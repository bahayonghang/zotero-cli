use std::collections::HashSet;

use anyhow::Result;
use serde::Serialize;
use zot_core::ErrorPayload;
use zot_local::SearchOptions;
use zot_remote::ZoteroRemote;

use crate::app_error::AppError;
use crate::cli::{ItemTagBatchArgs, ItemTagCommand};
use crate::context::AppContext;
use crate::output::CommandOutput;
use crate::util::require_item;

const PREVIEW_SAMPLE_LIMIT: usize = 10;

pub(crate) async fn handle(ctx: &AppContext, command: ItemTagCommand) -> Result<CommandOutput> {
    match command {
        ItemTagCommand::List(args) => {
            let item = require_item(&ctx.local_library()?, &args.key)?;
            CommandOutput::new(ctx, item.tags, None, |tags| {
                for tag in tags {
                    println!("{tag}");
                }
            })
        }
        ItemTagCommand::Add(args) => {
            ctx.remote()?.add_tags(&args.key, &args.tags).await?;
            let payload = serde_json::json!({ "key": args.key, "added": args.tags });
            CommandOutput::new(ctx, payload, None, |_| println!("Tags added."))
        }
        ItemTagCommand::Remove(args) => {
            ctx.remote()?.remove_tags(&args.key, &args.tags).await?;
            let payload = serde_json::json!({ "key": args.key, "removed": args.tags });
            CommandOutput::new(ctx, payload, None, |_| println!("Tags removed."))
        }
        ItemTagCommand::Batch(args) => {
            let payload = batch_update_tags(ctx, args).await?;
            CommandOutput::new(ctx, payload, None, |payload| {
                println!(
                    "{}",
                    serde_json::to_string_pretty(payload).expect("serialize batch tags")
                );
            })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum BatchTagState {
    Preview,
    Applied,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum TagOperation {
    Add,
    Remove,
}

#[derive(Debug, Serialize)]
struct BatchTagSuccess {
    key: String,
    operation: TagOperation,
}

#[derive(Debug, Serialize)]
struct BatchTagFailure {
    key: String,
    operation: TagOperation,
    error: ErrorPayload,
}

#[derive(Debug, Serialize)]
struct BatchTagReport {
    state: BatchTagState,
    matched: usize,
    affected: usize,
    truncated: bool,
    sample_keys: Vec<String>,
    added: Vec<String>,
    removed: Vec<String>,
    max_affected: usize,
    exceeds_max_affected: bool,
    attempted_operations: usize,
    succeeded_operations: usize,
    failed_operations: usize,
    successful: Vec<BatchTagSuccess>,
    failed: Vec<BatchTagFailure>,
}

#[derive(Debug)]
struct BatchTagPlan {
    keys: Vec<String>,
    report: BatchTagReport,
}

impl BatchTagPlan {
    fn new(
        matched: usize,
        keys: Vec<String>,
        added: Vec<String>,
        removed: Vec<String>,
        max_affected: usize,
    ) -> Self {
        let affected = keys.len();
        Self {
            report: BatchTagReport {
                state: BatchTagState::Preview,
                matched,
                affected,
                truncated: matched > affected,
                sample_keys: keys.iter().take(PREVIEW_SAMPLE_LIMIT).cloned().collect(),
                added,
                removed,
                max_affected,
                exceeds_max_affected: affected > max_affected,
                attempted_operations: 0,
                succeeded_operations: 0,
                failed_operations: 0,
                successful: Vec::new(),
                failed: Vec::new(),
            },
            keys,
        }
    }

    fn enforce_max_affected(&self) -> Result<()> {
        if self.report.exceeds_max_affected {
            return Err(zot_core::ZotError::InvalidInput {
                code: "batch-tags-max-affected".to_string(),
                message: format!(
                    "Batch tag plan affects {} items, exceeding --max-affected {}",
                    self.report.affected, self.report.max_affected
                ),
                hint: Some(
                    "Narrow the filters or explicitly raise --max-affected after reviewing the preview"
                        .to_string(),
                ),
            }
            .into());
        }
        Ok(())
    }
}

trait BatchTagWriter {
    async fn add_tags(&self, key: &str, tags: &[String]) -> Result<()>;
    async fn remove_tags(&self, key: &str, tags: &[String]) -> Result<()>;
}

struct WebBatchTagWriter {
    remote: ZoteroRemote,
}

impl BatchTagWriter for WebBatchTagWriter {
    async fn add_tags(&self, key: &str, tags: &[String]) -> Result<()> {
        Ok(self.remote.add_tags(key, tags).await?)
    }

    async fn remove_tags(&self, key: &str, tags: &[String]) -> Result<()> {
        Ok(self.remote.remove_tags(key, tags).await?)
    }
}

async fn batch_update_tags(ctx: &AppContext, args: ItemTagBatchArgs) -> Result<BatchTagReport> {
    validate_batch_args(&args)?;
    let confirm = args.confirm;
    let result = ctx.local_library()?.search(SearchOptions {
        query: args.query,
        tag: args.tag,
        limit: args.limit,
        ..SearchOptions::default()
    })?;
    let keys = result
        .items
        .into_iter()
        .map(|item| item.key)
        .collect::<Vec<_>>();
    let plan = BatchTagPlan::new(
        result.total,
        keys,
        args.add_tags,
        args.remove_tags,
        args.max_affected,
    );
    execute_batch_tag_plan(plan, confirm, || {
        Ok(WebBatchTagWriter {
            remote: ctx.remote()?,
        })
    })
    .await
}

fn validate_batch_args(args: &ItemTagBatchArgs) -> Result<()> {
    if args.query.trim().is_empty() && args.tag.is_none() {
        return Err(invalid_input(
            "batch-tags-filter",
            "Provide --query and/or --tag",
            None,
        ));
    }
    if args.tag.as_deref().is_some_and(|tag| tag.trim().is_empty()) {
        return Err(invalid_input(
            "batch-tags-filter",
            "--tag must not be blank",
            None,
        ));
    }
    if args.add_tags.is_empty() && args.remove_tags.is_empty() {
        return Err(invalid_input(
            "batch-tags-op",
            "Provide --add-tag and/or --remove-tag",
            None,
        ));
    }
    if args
        .add_tags
        .iter()
        .chain(&args.remove_tags)
        .any(|tag| tag.trim().is_empty())
    {
        return Err(invalid_input(
            "batch-tags-op",
            "Tag mutations must not contain blank values",
            None,
        ));
    }
    let added = args.add_tags.iter().collect::<HashSet<_>>();
    if let Some(conflict) = args.remove_tags.iter().find(|tag| added.contains(tag)) {
        return Err(invalid_input(
            "batch-tags-conflict",
            &format!("Tag `{conflict}` cannot be both added and removed"),
            Some("Choose either --add-tag or --remove-tag for each tag"),
        ));
    }
    if args.limit == 0 {
        return Err(invalid_input(
            "batch-tags-limit",
            "--limit must be greater than zero",
            None,
        ));
    }
    if args.max_affected == 0 {
        return Err(invalid_input(
            "batch-tags-max-affected",
            "--max-affected must be greater than zero",
            None,
        ));
    }
    Ok(())
}

fn invalid_input(code: &str, message: &str, hint: Option<&str>) -> anyhow::Error {
    zot_core::ZotError::InvalidInput {
        code: code.to_string(),
        message: message.to_string(),
        hint: hint.map(str::to_string),
    }
    .into()
}

async fn execute_batch_tag_plan<W, F>(
    plan: BatchTagPlan,
    confirm: bool,
    make_writer: F,
) -> Result<BatchTagReport>
where
    W: BatchTagWriter,
    F: FnOnce() -> Result<W>,
{
    if !confirm {
        return Ok(plan.report);
    }
    plan.enforce_max_affected()?;
    if plan.keys.is_empty() {
        let mut report = plan.report;
        report.state = BatchTagState::Applied;
        return Ok(report);
    }
    let writer = make_writer()?;
    Ok(apply_batch_tag_plan(&writer, plan).await)
}

async fn apply_batch_tag_plan<W: BatchTagWriter>(writer: &W, plan: BatchTagPlan) -> BatchTagReport {
    let BatchTagPlan { keys, mut report } = plan;
    for key in keys {
        if !report.added.is_empty() {
            let result = writer.add_tags(&key, &report.added).await;
            record_outcome(&mut report, key.clone(), TagOperation::Add, result);
        }
        if !report.removed.is_empty() {
            let result = writer.remove_tags(&key, &report.removed).await;
            record_outcome(&mut report, key, TagOperation::Remove, result);
        }
    }
    report.state = if report.failed_operations == 0 {
        BatchTagState::Applied
    } else if report.succeeded_operations == 0 {
        BatchTagState::Failed
    } else {
        BatchTagState::Partial
    };
    report
}

fn record_outcome(
    report: &mut BatchTagReport,
    key: String,
    operation: TagOperation,
    result: Result<()>,
) {
    report.attempted_operations += 1;
    match result {
        Ok(()) => {
            report.succeeded_operations += 1;
            report.successful.push(BatchTagSuccess { key, operation });
        }
        Err(error) => {
            report.failed_operations += 1;
            report.failed.push(BatchTagFailure {
                key,
                operation,
                error: AppError::runtime(error).payload(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::Mutex;

    use anyhow::anyhow;

    use super::*;

    fn args() -> ItemTagBatchArgs {
        ItemTagBatchArgs {
            query: "transformer".to_string(),
            tag: None,
            add_tags: vec!["reviewed".to_string()],
            remove_tags: Vec::new(),
            limit: 50,
            max_affected: 50,
            confirm: false,
        }
    }

    fn error_code(error: anyhow::Error) -> String {
        error
            .downcast_ref::<zot_core::ZotError>()
            .expect("typed ZotError")
            .payload()
            .code
    }

    #[test]
    fn validation_rejects_unsafe_inputs_with_stable_codes() {
        let mut value = args();
        value.query.clear();
        assert_eq!(
            error_code(validate_batch_args(&value).expect_err("missing filter")),
            "batch-tags-filter"
        );

        let mut value = args();
        value.query.clear();
        value.tag = Some("   ".to_string());
        assert_eq!(
            error_code(validate_batch_args(&value).expect_err("blank filter tag")),
            "batch-tags-filter"
        );

        let mut value = args();
        value.add_tags.clear();
        assert_eq!(
            error_code(validate_batch_args(&value).expect_err("missing operation")),
            "batch-tags-op"
        );

        let mut value = args();
        value.add_tags = vec!["  ".to_string()];
        assert_eq!(
            error_code(validate_batch_args(&value).expect_err("blank mutation tag")),
            "batch-tags-op"
        );

        let mut value = args();
        value.add_tags = vec!["same".to_string()];
        value.remove_tags = vec!["same".to_string()];
        assert_eq!(
            error_code(validate_batch_args(&value).expect_err("conflicting tag")),
            "batch-tags-conflict"
        );

        let mut value = args();
        value.limit = 0;
        assert_eq!(
            error_code(validate_batch_args(&value).expect_err("zero limit")),
            "batch-tags-limit"
        );

        let mut value = args();
        value.max_affected = 0;
        assert_eq!(
            error_code(validate_batch_args(&value).expect_err("zero ceiling")),
            "batch-tags-max-affected"
        );
    }

    #[tokio::test]
    async fn preview_reports_total_selection_and_sample_without_constructing_writer() {
        let keys = (0..15).map(|index| format!("KEY{index:02}")).collect();
        let plan = BatchTagPlan::new(30, keys, vec!["reviewed".to_string()], Vec::new(), 10);
        let writer_created = Cell::new(false);

        let report = execute_batch_tag_plan(plan, false, || {
            writer_created.set(true);
            Ok(FakeWriter::default())
        })
        .await
        .expect("preview");

        assert!(!writer_created.get());
        assert_eq!(report.state, BatchTagState::Preview);
        assert_eq!(report.matched, 30);
        assert_eq!(report.affected, 15);
        assert!(report.truncated);
        assert_eq!(report.sample_keys.len(), PREVIEW_SAMPLE_LIMIT);
        assert!(report.exceeds_max_affected);
        assert_eq!(report.attempted_operations, 0);
        let json = serde_json::to_value(&report).expect("serialize preview");
        assert_eq!(json["state"], "preview");
        assert_eq!(json["matched"], 30);
        assert_eq!(json["affected"], 15);
        assert_eq!(
            json["sample_keys"].as_array().map(Vec::len),
            Some(PREVIEW_SAMPLE_LIMIT)
        );
    }

    #[tokio::test]
    async fn over_ceiling_confirm_fails_before_constructing_writer() {
        let plan = BatchTagPlan::new(
            3,
            vec!["A".to_string(), "B".to_string(), "C".to_string()],
            vec!["reviewed".to_string()],
            Vec::new(),
            2,
        );
        let writer_created = Cell::new(false);

        let error = execute_batch_tag_plan(plan, true, || {
            writer_created.set(true);
            Ok(FakeWriter::default())
        })
        .await
        .expect_err("ceiling must block");

        assert!(!writer_created.get());
        assert_eq!(error_code(error), "batch-tags-max-affected");
    }

    #[tokio::test]
    async fn empty_confirm_is_applied_without_constructing_writer() {
        let plan = BatchTagPlan::new(0, Vec::new(), vec!["reviewed".to_string()], Vec::new(), 50);
        let writer_created = Cell::new(false);

        let report = execute_batch_tag_plan(plan, true, || {
            writer_created.set(true);
            Ok(FakeWriter::default())
        })
        .await
        .expect("empty apply");

        assert!(!writer_created.get());
        assert_eq!(report.state, BatchTagState::Applied);
        assert_eq!(report.attempted_operations, 0);
    }

    #[derive(Default)]
    struct FakeWriter {
        calls: Mutex<Vec<(String, TagOperation)>>,
        fail_all: bool,
    }

    impl BatchTagWriter for FakeWriter {
        async fn add_tags(&self, key: &str, _tags: &[String]) -> Result<()> {
            self.calls
                .lock()
                .expect("calls lock")
                .push((key.to_string(), TagOperation::Add));
            if self.fail_all || key == "A" {
                return Err(anyhow!("add transport failed"));
            }
            Ok(())
        }

        async fn remove_tags(&self, key: &str, _tags: &[String]) -> Result<()> {
            self.calls
                .lock()
                .expect("calls lock")
                .push((key.to_string(), TagOperation::Remove));
            if self.fail_all || key == "B" {
                return Err(zot_core::ZotError::Remote {
                    code: "remove-tags-test".to_string(),
                    message: "remove failed".to_string(),
                    hint: Some("retry later".to_string()),
                    status: Some(503),
                }
                .into());
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn apply_continues_all_operations_and_preserves_nested_error_codes() {
        let writer = FakeWriter::default();
        let plan = BatchTagPlan::new(
            2,
            vec!["A".to_string(), "B".to_string()],
            vec!["new".to_string()],
            vec!["old".to_string()],
            2,
        );

        let report = apply_batch_tag_plan(&writer, plan).await;

        assert_eq!(report.state, BatchTagState::Partial);
        assert_eq!(report.attempted_operations, 4);
        assert_eq!(report.succeeded_operations, 2);
        assert_eq!(report.failed_operations, 2);
        assert_eq!(report.failed[0].error.code, "runtime-error");
        assert_eq!(report.failed[1].error.code, "remove-tags-test");
        assert_eq!(
            *writer.calls.lock().expect("calls lock"),
            [
                ("A".to_string(), TagOperation::Add),
                ("A".to_string(), TagOperation::Remove),
                ("B".to_string(), TagOperation::Add),
                ("B".to_string(), TagOperation::Remove),
            ]
        );
    }

    #[tokio::test]
    async fn report_distinguishes_applied_and_failed_states() {
        let applied = apply_batch_tag_plan(
            &FakeWriter::default(),
            BatchTagPlan::new(
                1,
                vec!["B".to_string()],
                vec!["new".to_string()],
                Vec::new(),
                1,
            ),
        )
        .await;
        assert_eq!(applied.state, BatchTagState::Applied);

        let failed = apply_batch_tag_plan(
            &FakeWriter {
                calls: Mutex::default(),
                fail_all: true,
            },
            BatchTagPlan::new(
                1,
                vec!["A".to_string()],
                vec!["new".to_string()],
                vec!["old".to_string()],
                1,
            ),
        )
        .await;
        assert_eq!(failed.state, BatchTagState::Failed);
        assert_eq!(failed.failed_operations, 2);
        assert_eq!(failed.succeeded_operations, 0);
    }
}
