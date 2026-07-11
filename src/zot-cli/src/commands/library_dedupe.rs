//! Batch duplicate cleanup engine for `zot library dedupe`.
//!
//! Keeper scoring, confidence marking, and plan building are pure local
//! functions so the default dry-run never touches the network. The apply
//! loop is generic over [`GroupMerger`] (the fake-seam pattern from
//! `commands/collection.rs`) so tests can inject per-group failures; the
//! production merger delegates each group to `merge_item_set`, the same
//! engine behind `item merge` and `library duplicates-merge`.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use anyhow::Result;
use zot_core::{
    DedupeApplyReport, DedupeConfidence, DedupeGroupFailure, DedupeGroupPlan, DedupeItemRef,
    DedupePlan, DuplicateGroup, Item, MergeApplyResult, MergeOperation, WriteBackend, ZotError,
    ZotResult,
};
use zot_local::AttachmentSource;

use crate::commands::item::merge::{MergeWriter, merge_item_set};

/// Build the dry-run cleanup plan: pick a keeper per group ("published
/// version first" policy) and mark low-confidence groups for review. Purely
/// local — the only I/O is `get_attachments` against the read-only SQLite.
///
/// Detection groups sharing an item key are first merged into one component
/// (see [`merge_overlapping_groups`]) so every key appears at most once in
/// the plan and no group can elect a keeper that an earlier group trashes.
pub(crate) fn build_dedupe_plan(
    library: &impl AttachmentSource,
    groups: &[DuplicateGroup],
    write_backend: WriteBackend,
    include_low_confidence: bool,
) -> ZotResult<DedupePlan> {
    let mut plans = Vec::new();
    for group in merge_overlapping_groups(groups) {
        let match_type = group
            .match_types
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("+");
        let mut scored = Vec::new();
        for item in &group.items {
            scored.push(ScoredItem {
                type_rank: type_rank(&item.item_type),
                field_count: metadata_field_count(item),
                attachment_count: library.get_attachments(&item.key)?.len(),
                item,
            });
        }
        scored.sort_by(keeper_ordering);
        let [keeper, runner_up, ..] = scored.as_slice() else {
            continue;
        };
        let (confidence, confidence_note) = group_confidence(&group.items);
        plans.push(DedupeGroupPlan {
            match_type,
            confidence,
            confidence_note,
            keeper: item_ref(keeper.item),
            reason: keeper_reason(keeper, runner_up),
            absorb: scored[1..]
                .iter()
                .map(|entry| item_ref(entry.item))
                .collect(),
        });
    }
    Ok(DedupePlan {
        write_backend,
        include_low_confidence,
        total_groups: plans.len(),
        groups: plans,
        confirm_required: true,
    })
}

/// One post-merge duplicate component: the union of detection groups
/// connected through shared item keys.
struct MergedGroup {
    match_types: BTreeSet<String>,
    keys: BTreeSet<String>,
    items: Vec<Item>,
}

/// Merge detection groups that share an item key into connected components.
///
/// With `--method both` the same item can land in a DOI group and a title
/// group (e.g. {A,B} and {B,C}). Planning those separately is a correctness
/// hazard, not just wasted API calls: applying the first group trashes B,
/// and if B then wins keeper selection in the second group, C gets absorbed
/// into an item already sitting in the trash. Components keep every key
/// unique across the plan; first-appearance order is preserved so
/// single-source groups come out exactly as detected.
fn merge_overlapping_groups(groups: &[DuplicateGroup]) -> Vec<MergedGroup> {
    let mut merged: Vec<MergedGroup> = Vec::new();
    for group in groups {
        // `find_duplicates` only emits groups of >= 2; skip anything smaller
        // instead of planning a group with nothing to absorb.
        if group.items.len() < 2 {
            continue;
        }
        let overlapping = merged
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                group
                    .items
                    .iter()
                    .any(|item| entry.keys.contains(&item.key))
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let target_index = match overlapping.split_first() {
            None => {
                merged.push(MergedGroup {
                    match_types: BTreeSet::new(),
                    keys: BTreeSet::new(),
                    items: Vec::new(),
                });
                merged.len() - 1
            }
            Some((&first, rest)) => {
                // Fold later components into the first: `rest` is ascending
                // and every index is greater than `first`, so each removal
                // only shifts entries behind those already handled
                // (compensated via the enumeration count).
                for (removed, &index) in rest.iter().enumerate() {
                    let entry = merged.remove(index - removed);
                    let target = &mut merged[first];
                    target.match_types.extend(entry.match_types);
                    for item in entry.items {
                        if target.keys.insert(item.key.clone()) {
                            target.items.push(item);
                        }
                    }
                }
                first
            }
        };
        let target = &mut merged[target_index];
        target.match_types.insert(group.match_type.clone());
        for item in &group.items {
            if target.keys.insert(item.key.clone()) {
                target.items.push(item.clone());
            }
        }
    }
    merged
}

/// Seam between the apply loop and the actual per-group merge, so tests can
/// drive multi-group failure handling without a network stack.
pub(crate) trait GroupMerger {
    async fn merge_group(
        &self,
        keeper_key: &str,
        source_keys: &[String],
    ) -> Result<MergeApplyResult>;
}

/// Production merger: one confirmed `merge_item_set` call per group over the
/// Zotero Web API.
pub(crate) struct WriterGroupMerger<'a>(pub(crate) &'a dyn MergeWriter);

impl GroupMerger for WriterGroupMerger<'_> {
    async fn merge_group(
        &self,
        keeper_key: &str,
        source_keys: &[String],
    ) -> Result<MergeApplyResult> {
        match merge_item_set(self.0, keeper_key, source_keys, true).await? {
            MergeOperation::Applied(result) => Ok(result),
            // Unreachable with confirm=true; mapped defensively instead of
            // panicking per workspace lint policy.
            MergeOperation::Preview(_) => Err(ZotError::InvalidInput {
                code: "library-dedupe".to_string(),
                message: "Merge engine returned a preview during apply".to_string(),
                hint: None,
            }
            .into()),
        }
    }
}

/// Apply every planned group. A failing group is recorded and the loop moves
/// on: the Web API has no transactions, and each group's merge is
/// independently safe to re-run (fill only writes empty fields, relations
/// dedupe, trash is idempotent), so partial progress converges on retry.
pub(crate) async fn apply_dedupe_plan<M: GroupMerger>(
    merger: &M,
    plan: &DedupePlan,
) -> DedupeApplyReport {
    let mut applied = Vec::new();
    let mut failed = Vec::new();
    let mut skipped_low_confidence = Vec::new();
    for group in &plan.groups {
        if group.confidence == DedupeConfidence::Low && !plan.include_low_confidence {
            skipped_low_confidence.push(group.clone());
            continue;
        }
        let sources = group
            .absorb
            .iter()
            .map(|entry| entry.key.clone())
            .collect::<Vec<_>>();
        match merger.merge_group(&group.keeper.key, &sources).await {
            Ok(result) => applied.push(result),
            Err(err) => failed.push(DedupeGroupFailure {
                keeper: group.keeper.key.clone(),
                sources,
                error: failure_message(&err),
            }),
        }
    }
    DedupeApplyReport {
        write_backend: plan.write_backend,
        total_groups: plan.groups.len(),
        eligible_groups: plan.groups.len() - skipped_low_confidence.len(),
        applied_groups: applied.len(),
        failed_groups: failed.len(),
        skipped_low_confidence_groups: skipped_low_confidence.len(),
        applied,
        failed,
        skipped_low_confidence,
    }
}

/// One group member with the precomputed signals keeper selection sorts on.
struct ScoredItem<'a> {
    item: &'a Item,
    type_rank: u8,
    field_count: usize,
    attachment_count: usize,
}

/// "Published version first" type priority (rank 1 wins). Journal and
/// conference papers tie on purpose; anything unknown ranks last.
fn type_rank(item_type: &str) -> u8 {
    match item_type {
        "journalArticle" | "conferencePaper" => 1,
        "book" | "bookSection" => 2,
        "thesis" => 3,
        "report" => 4,
        "preprint" => 5,
        "document" => 6,
        _ => 7,
    }
}

/// Count the non-empty dimensions of the compact local model
/// (title/abstract/date/url/doi/creators/tags/extra). Used as the first
/// tie-break: more populated metadata makes a better keeper.
fn metadata_field_count(item: &Item) -> usize {
    let mut count = usize::from(!item.title.trim().is_empty());
    for text in [&item.abstract_note, &item.date, &item.url, &item.doi] {
        if text
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            count += 1;
        }
    }
    count += usize::from(!item.creators.is_empty());
    count += usize::from(!item.tags.is_empty());
    count += usize::from(!item.extra.is_empty());
    count
}

/// Full deterministic keeper ordering: type rank, then more fields, more
/// attachments, earlier dateAdded, and finally key order so equal candidates
/// still sort stably.
fn keeper_ordering(a: &ScoredItem, b: &ScoredItem) -> Ordering {
    a.type_rank
        .cmp(&b.type_rank)
        .then_with(|| b.field_count.cmp(&a.field_count))
        .then_with(|| b.attachment_count.cmp(&a.attachment_count))
        .then_with(|| date_added_key(a.item).cmp(&date_added_key(b.item)))
        .then_with(|| a.item.key.cmp(&b.item.key))
}

/// Sort key for "earlier dateAdded wins": items without a date sort last.
/// Zotero's `YYYY-MM-DD HH:MM:SS` strings order chronologically as text.
fn date_added_key(item: &Item) -> (bool, &str) {
    match item.date_added.as_deref() {
        Some(date) if !date.is_empty() => (false, date),
        _ => (true, ""),
    }
}

fn date_added_display(item: &Item) -> &str {
    item.date_added
        .as_deref()
        .filter(|date| !date.is_empty())
        .unwrap_or("unknown")
}

/// Explain the keeper choice against the runner-up, walking the comparison
/// layers in order and stopping at the first decisive one; layers that tied
/// on the way are shown with `=` so the decision stays auditable.
fn keeper_reason(keeper: &ScoredItem, runner_up: &ScoredItem) -> String {
    if keeper.type_rank != runner_up.type_rank {
        return format!(
            "type: {}({}) > {}({})",
            keeper.item.item_type, keeper.type_rank, runner_up.item.item_type, runner_up.type_rank
        );
    }
    let prefix = format!(
        "type: {}({}) = {}({}); tie-break: ",
        keeper.item.item_type, keeper.type_rank, runner_up.item.item_type, runner_up.type_rank
    );
    let mut segments = Vec::new();
    if keeper.field_count != runner_up.field_count {
        segments.push(format!(
            "fields {}>{}",
            keeper.field_count, runner_up.field_count
        ));
        return prefix + &segments.join(", ");
    }
    segments.push(format!(
        "fields {}={}",
        keeper.field_count, runner_up.field_count
    ));
    if keeper.attachment_count != runner_up.attachment_count {
        segments.push(format!(
            "attachments {}>{}",
            keeper.attachment_count, runner_up.attachment_count
        ));
        return prefix + &segments.join(", ");
    }
    segments.push(format!(
        "attachments {}={}",
        keeper.attachment_count, runner_up.attachment_count
    ));
    if date_added_key(keeper.item) != date_added_key(runner_up.item) {
        segments.push(format!(
            "dateAdded {} < {}",
            date_added_display(keeper.item),
            date_added_display(runner_up.item)
        ));
        return prefix + &segments.join(", ");
    }
    segments.push(format!(
        "dateAdded {} = {}",
        date_added_display(keeper.item),
        date_added_display(runner_up.item)
    ));
    segments.push(format!("key {} < {}", keeper.item.key, runner_up.item.key));
    prefix + &segments.join(", ")
}

/// Mark a group `low` confidence when its members spread more than one year
/// apart or carry two or more distinct DOIs. Deliberately weaker than
/// Zotero's own "different DOI vetoes the match" rule: an arXiv DOI always
/// differs from the published one, and preprint↔published cleanup is this
/// command's primary scenario, so the group stays in the plan and only gets
/// flagged for review.
fn group_confidence(items: &[Item]) -> (DedupeConfidence, Option<String>) {
    let mut notes = Vec::new();
    let years = items
        .iter()
        .filter_map(|item| extract_year(item.date.as_deref()?))
        .collect::<Vec<_>>();
    if let (Some(min), Some(max)) = (years.iter().min(), years.iter().max())
        && max - min > 1
    {
        notes.push(format!("year spread {min}\u{2194}{max} (>1)"));
    }
    let dois = items
        .iter()
        .filter_map(|item| item.doi.as_deref())
        .map(|doi| doi.trim().to_lowercase())
        .filter(|doi| !doi.is_empty())
        .collect::<BTreeSet<_>>();
    if dois.len() >= 2 {
        notes.push("differing DOIs".to_string());
    }
    if notes.is_empty() {
        (DedupeConfidence::Normal, None)
    } else {
        (DedupeConfidence::Low, Some(notes.join("; ")))
    }
}

/// First standalone run of exactly four digits in a Zotero date string
/// ("2021-05-01", "May 2021", "2021" all yield 2021).
fn extract_year(date: &str) -> Option<i32> {
    let bytes = date.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_digit() {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index - start == 4 {
                return date[start..index].parse().ok();
            }
        } else {
            index += 1;
        }
    }
    None
}

fn item_ref(item: &Item) -> DedupeItemRef {
    DedupeItemRef {
        key: item.key.clone(),
        item_type: item.item_type.clone(),
        title: item.title.clone(),
    }
}

/// Flatten a group failure into the mapped `code: message` form so the
/// report carries the structured `ZotError` info (per the zot-remote error
/// mapping) as one reviewable string.
fn failure_message(err: &anyhow::Error) -> String {
    match err.downcast_ref::<ZotError>() {
        Some(zot) => {
            let payload = zot.payload();
            format!("{}: {}", payload.code, payload.message)
        }
        None => err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::sync::Arc;

    use zot_core::{AppConfig, Attachment, LibraryScope};
    use zot_local::PdfiumBackend;
    use zot_remote::HttpRuntime;

    use super::*;
    use crate::context::AppContext;
    use crate::output::CommandOutput;

    fn json_ctx() -> AppContext {
        AppContext {
            json: true,
            profile: Some("default".to_string()),
            scope: LibraryScope::User,
            config: AppConfig::default(),
            http: Arc::new(HttpRuntime::default()),
            pdf: Arc::new(PdfiumBackend),
        }
    }

    fn item(key: &str, item_type: &str, title: &str) -> Item {
        Item {
            key: key.to_string(),
            item_type: item_type.to_string(),
            title: title.to_string(),
            creators: vec![],
            abstract_note: None,
            date: None,
            url: None,
            doi: None,
            tags: vec![],
            collections: vec![],
            date_added: None,
            date_modified: None,
            extra: Default::default(),
        }
    }

    fn group(match_type: &str, items: Vec<Item>) -> DuplicateGroup {
        DuplicateGroup {
            match_type: match_type.to_string(),
            score: 0.92,
            items,
        }
    }

    fn build_plan(
        library: &impl AttachmentSource,
        groups: &[DuplicateGroup],
    ) -> ZotResult<DedupePlan> {
        build_dedupe_plan(library, groups, WriteBackend::Web, false)
    }

    /// Attachment counts per item key; the planner only calls
    /// `get_attachments`, the rest of the trait is inert.
    struct FakeAttachments(Vec<(&'static str, usize)>);

    fn no_attachments() -> FakeAttachments {
        FakeAttachments(Vec::new())
    }

    impl AttachmentSource for FakeAttachments {
        fn get_attachments(&self, key: &str) -> ZotResult<Vec<Attachment>> {
            let count = self
                .0
                .iter()
                .find(|(entry, _)| *entry == key)
                .map(|(_, count)| *count)
                .unwrap_or(0);
            Ok((0..count)
                .map(|index| Attachment {
                    key: format!("{key}-A{index}"),
                    parent_key: key.to_string(),
                    filename: "paper.pdf".to_string(),
                    content_type: "application/pdf".to_string(),
                })
                .collect())
        }
        fn get_attachment_by_key(&self, _key: &str) -> ZotResult<Option<Attachment>> {
            Ok(None)
        }
        fn get_pdf_attachment(&self, _key: &str) -> ZotResult<Option<Attachment>> {
            Ok(None)
        }
        fn attachment_path(&self, _attachment: &Attachment) -> PathBuf {
            PathBuf::new()
        }
        fn pdf_path(&self, _attachment: &Attachment) -> PathBuf {
            PathBuf::new()
        }
    }

    #[test]
    fn keeper_prefers_published_type_over_preprint() {
        // Passing the preprint first proves selection reorders the group.
        let prep = item("PREP0001", "preprint", "Attention Is All You Need");
        let conf = item("CONF0001", "conferencePaper", "Attention Is All You Need");
        let plan =
            build_plan(&no_attachments(), &[group("title", vec![prep, conf])]).expect("build plan");

        let group = &plan.groups[0];
        assert_eq!(group.keeper.key, "CONF0001");
        assert_eq!(group.reason, "type: conferencePaper(1) > preprint(5)");
        assert_eq!(group.absorb.len(), 1);
        assert_eq!(group.absorb[0].key, "PREP0001");
    }

    #[test]
    fn keeper_tie_breaks_on_metadata_field_count() {
        let mut rich = item("RICH0001", "journalArticle", "Same Title");
        rich.doi = Some("10.1000/x".to_string());
        rich.date = Some("2021-05-01".to_string());
        let poor = item("POOR0001", "journalArticle", "Same Title");
        let plan =
            build_plan(&no_attachments(), &[group("title", vec![poor, rich])]).expect("build plan");

        let group = &plan.groups[0];
        assert_eq!(group.keeper.key, "RICH0001");
        assert_eq!(
            group.reason,
            "type: journalArticle(1) = journalArticle(1); tie-break: fields 3>1"
        );
    }

    #[test]
    fn keeper_tie_breaks_on_attachment_count() {
        let with_pdf = item("PDFS0001", "journalArticle", "Same Title");
        let without = item("NONE0001", "journalArticle", "Same Title");
        let library = FakeAttachments(vec![("PDFS0001", 2), ("NONE0001", 0)]);
        let plan =
            build_plan(&library, &[group("title", vec![without, with_pdf])]).expect("build plan");

        let group = &plan.groups[0];
        assert_eq!(group.keeper.key, "PDFS0001");
        assert_eq!(
            group.reason,
            "type: journalArticle(1) = journalArticle(1); tie-break: fields 1=1, attachments 2>0"
        );
    }

    #[test]
    fn keeper_tie_breaks_on_earlier_date_added() {
        let mut old = item("OLDR0001", "journalArticle", "Same Title");
        old.date_added = Some("2020-01-05 09:00:00".to_string());
        let mut new = item("NEWR0001", "journalArticle", "Same Title");
        new.date_added = Some("2022-03-01 09:00:00".to_string());
        let plan =
            build_plan(&no_attachments(), &[group("title", vec![new, old])]).expect("build plan");

        let group = &plan.groups[0];
        assert_eq!(group.keeper.key, "OLDR0001");
        assert_eq!(
            group.reason,
            "type: journalArticle(1) = journalArticle(1); tie-break: fields 1=1, \
             attachments 0=0, dateAdded 2020-01-05 09:00:00 < 2022-03-01 09:00:00"
        );
    }

    #[test]
    fn keeper_falls_back_to_key_order_for_determinism() {
        let mut first = item("AAAA0001", "journalArticle", "Same Title");
        first.date_added = Some("2024-01-01 00:00:00".to_string());
        let mut second = item("BBBB0002", "journalArticle", "Same Title");
        second.date_added = Some("2024-01-01 00:00:00".to_string());
        let plan = build_plan(&no_attachments(), &[group("title", vec![second, first])])
            .expect("build plan");

        let group = &plan.groups[0];
        assert_eq!(group.keeper.key, "AAAA0001");
        assert_eq!(
            group.reason,
            "type: journalArticle(1) = journalArticle(1); tie-break: fields 1=1, \
             attachments 0=0, dateAdded 2024-01-01 00:00:00 = 2024-01-01 00:00:00, \
             key AAAA0001 < BBBB0002"
        );
    }

    #[test]
    fn partially_overlapping_groups_merge_into_one_component() {
        // DOI group {A,B} + title group {B,C}. Planned separately, applying
        // the DOI group first trashes B, and B (preprint outranks document)
        // would then win keeper selection in {B,C} — absorbing C into a
        // trashed item. The merged component elects exactly one keeper.
        let article = item("ARTC0001", "journalArticle", "Same Paper");
        let preprint = item("PREP0001", "preprint", "Same Paper");
        let document = item("DOCU0001", "document", "Same Paper");

        let plan = build_plan(
            &no_attachments(),
            &[
                group("doi", vec![article.clone(), preprint.clone()]),
                group("title", vec![preprint, document]),
            ],
        )
        .expect("build plan");

        assert_eq!(plan.total_groups, 1);
        let merged = &plan.groups[0];
        assert_eq!(merged.match_type, "doi+title");
        assert_eq!(merged.keeper.key, "ARTC0001");
        assert_eq!(
            merged
                .absorb
                .iter()
                .map(|entry| entry.key.as_str())
                .collect::<Vec<_>>(),
            vec!["PREP0001", "DOCU0001"]
        );
    }

    #[test]
    fn fully_overlapping_groups_collapse_to_combined_match_type() {
        // The same pair matched by both DOI and title collapses into one
        // planned group instead of two redundant merges.
        let article = item("ARTC0001", "journalArticle", "Same Paper");
        let preprint = item("PREP0001", "preprint", "Same Paper");

        let plan = build_plan(
            &no_attachments(),
            &[
                group("doi", vec![article.clone(), preprint.clone()]),
                group("title", vec![preprint, article]),
            ],
        )
        .expect("build plan");

        assert_eq!(plan.total_groups, 1);
        let merged = &plan.groups[0];
        assert_eq!(merged.match_type, "doi+title");
        assert_eq!(merged.keeper.key, "ARTC0001");
        assert_eq!(merged.absorb.len(), 1);
        assert_eq!(merged.absorb[0].key, "PREP0001");
    }

    #[test]
    fn bridging_group_folds_two_existing_components_into_one() {
        // {A,B} and {C,D} first form two separate components; a later {B,C}
        // group bridges them. This exercises the fold branch of
        // `merge_overlapping_groups` (removing later components with index
        // compensation), which two-group overlaps never reach.
        let article = item("ARTC0001", "journalArticle", "Same Paper");
        let preprint = item("PREP0001", "preprint", "Same Paper");
        let document = item("DOCU0001", "document", "Same Paper");
        let report = item("REPT0001", "report", "Same Paper");

        let plan = build_plan(
            &no_attachments(),
            &[
                group("doi", vec![article.clone(), preprint.clone()]),
                group("title", vec![document.clone(), report]),
                group("title", vec![preprint, document]),
            ],
        )
        .expect("build plan");

        assert_eq!(plan.total_groups, 1);
        let merged = &plan.groups[0];
        assert_eq!(merged.match_type, "doi+title");
        assert_eq!(merged.keeper.key, "ARTC0001");
        // Absorb order follows keeper scoring: report(4) > preprint(5) >
        // document(6) on the type-rank ladder.
        assert_eq!(
            merged
                .absorb
                .iter()
                .map(|entry| entry.key.as_str())
                .collect::<Vec<_>>(),
            vec!["REPT0001", "PREP0001", "DOCU0001"]
        );
    }

    #[test]
    fn year_spread_over_one_marks_low_confidence() {
        let mut early = item("ERLY0001", "conferencePaper", "Same Title");
        early.date = Some("2021-06-01".to_string());
        let mut late = item("LATE0001", "preprint", "Same Title");
        late.date = Some("2023".to_string());

        let (confidence, note) = group_confidence(&[early, late]);
        assert_eq!(confidence, DedupeConfidence::Low);
        assert_eq!(note.as_deref(), Some("year spread 2021\u{2194}2023 (>1)"));
    }

    #[test]
    fn differing_dois_mark_low_confidence() {
        // The arXiv DOI vs published DOI case: flagged, never excluded.
        let mut published = item("PUBL0001", "journalArticle", "Same Title");
        published.doi = Some("10.1109/tit.2021.1234".to_string());
        let mut preprint = item("PREP0001", "preprint", "Same Title");
        preprint.doi = Some("10.48550/arXiv.2101.00001".to_string());

        let (confidence, note) = group_confidence(&[published, preprint]);
        assert_eq!(confidence, DedupeConfidence::Low);
        assert_eq!(note.as_deref(), Some("differing DOIs"));
    }

    #[test]
    fn same_doi_and_year_stay_normal_confidence() {
        // Case-insensitive DOI equality collapses to one DOI; one-year
        // spread is within tolerance.
        let mut a = item("ITEM0001", "journalArticle", "Same Title");
        a.doi = Some("10.1000/X".to_string());
        a.date = Some("2021-12-01".to_string());
        let mut b = item("ITEM0002", "journalArticle", "Same Title");
        b.doi = Some("10.1000/x".to_string());
        b.date = Some("2022-01-15".to_string());

        let (confidence, note) = group_confidence(&[a, b]);
        assert_eq!(confidence, DedupeConfidence::Normal);
        assert_eq!(note, None);
    }

    #[test]
    fn dedupe_plan_serializes_design_contract_shape() {
        let mut conf = item("CONF0001", "conferencePaper", "Attention Is All You Need");
        conf.date = Some("2021-06-01".to_string());
        let mut prep = item("PREP0001", "preprint", "Attention Is All You Need");
        prep.date = Some("2023".to_string());
        let mut doi_a = item("DOIA0001", "journalArticle", "Another Paper");
        doi_a.doi = Some("10.1000/y".to_string());
        let mut doi_b = item("DOIB0002", "journalArticle", "Another Paper");
        doi_b.doi = Some("10.1000/y".to_string());

        let plan = build_plan(
            &no_attachments(),
            &[
                group("title", vec![prep, conf]),
                group("doi", vec![doi_a, doi_b]),
            ],
        )
        .expect("build plan");
        let json = serde_json::to_value(&plan).expect("serialize plan");

        assert_eq!(json["write_backend"], "web");
        assert_eq!(json["include_low_confidence"], false);
        assert_eq!(json["total_groups"], 2);
        assert_eq!(json["confirm_required"], true);
        let low = &json["groups"][0];
        assert_eq!(low["match_type"], "title");
        assert_eq!(low["confidence"], "low");
        assert_eq!(low["confidence_note"], "year spread 2021\u{2194}2023 (>1)");
        assert_eq!(low["keeper"]["key"], "CONF0001");
        assert_eq!(low["keeper"]["item_type"], "conferencePaper");
        assert_eq!(low["keeper"]["title"], "Attention Is All You Need");
        assert_eq!(low["reason"], "type: conferencePaper(1) > preprint(5)");
        assert_eq!(low["absorb"][0]["key"], "PREP0001");
        assert_eq!(low["absorb"][0]["item_type"], "preprint");
        let normal = &json["groups"][1];
        assert_eq!(normal["match_type"], "doi");
        assert_eq!(normal["confidence"], "normal");
        // `confidence_note` is omitted entirely for normal groups.
        assert!(normal.get("confidence_note").is_none());
    }

    #[test]
    fn dedupe_plan_envelope_carries_plan_fields() {
        let conf = item("CONF0001", "conferencePaper", "Attention Is All You Need");
        let prep = item("PREP0001", "preprint", "Attention Is All You Need");
        let plan =
            build_plan(&no_attachments(), &[group("title", vec![prep, conf])]).expect("build plan");

        let out =
            CommandOutput::new(&json_ctx(), plan, None, |_| unreachable!()).expect("build output");
        let json = out.as_json().expect("json payload");
        assert!(json.contains("\"ok\": true"));
        assert!(json.contains("\"confirm_required\": true"));
        assert!(json.contains("\"reason\": \"type: conferencePaper(1) > preprint(5)\""));
    }

    /// Records call order; fails exactly one keeper to prove the loop
    /// continues past a failed group.
    struct FakeMerger {
        fail_keeper: &'static str,
        calls: RefCell<Vec<String>>,
    }

    impl GroupMerger for FakeMerger {
        async fn merge_group(
            &self,
            keeper_key: &str,
            source_keys: &[String],
        ) -> Result<MergeApplyResult> {
            self.calls.borrow_mut().push(keeper_key.to_string());
            if keeper_key == self.fail_keeper {
                return Err(ZotError::Remote {
                    code: "update-item-value".to_string(),
                    message: "Zotero API request failed with status 412".to_string(),
                    hint: None,
                    status: Some(412),
                }
                .into());
            }
            Ok(MergeApplyResult {
                write_backend: WriteBackend::Web,
                already_applied: false,
                keeper_key: keeper_key.to_string(),
                source_keys_trashed: source_keys.to_vec(),
                metadata_fields_filled: vec![],
                skipped_incompatible_fields: vec![],
                tags_added: vec![],
                collections_added: vec![],
                relations_to_add: vec![],
                children_reparented: 0,
                skipped_duplicate_attachments: 0,
            })
        }
    }

    fn group_plan(keeper: &str, source: &str) -> DedupeGroupPlan {
        DedupeGroupPlan {
            match_type: "doi".to_string(),
            confidence: DedupeConfidence::Normal,
            confidence_note: None,
            keeper: DedupeItemRef {
                key: keeper.to_string(),
                item_type: "journalArticle".to_string(),
                title: "Title".to_string(),
            },
            reason: "type: journalArticle(1) > preprint(5)".to_string(),
            absorb: vec![DedupeItemRef {
                key: source.to_string(),
                item_type: "preprint".to_string(),
                title: "Title".to_string(),
            }],
        }
    }

    #[tokio::test]
    async fn apply_continues_after_single_group_failure() {
        let plan = DedupePlan {
            write_backend: WriteBackend::Web,
            include_low_confidence: false,
            groups: vec![
                group_plan("KEEP0001", "DUPE0001"),
                group_plan("KEEP0002", "DUPE0002"),
                group_plan("KEEP0003", "DUPE0003"),
            ],
            total_groups: 3,
            confirm_required: true,
        };
        let merger = FakeMerger {
            fail_keeper: "KEEP0002",
            calls: RefCell::new(Vec::new()),
        };

        let report = apply_dedupe_plan(&merger, &plan).await;

        // Every group was attempted, in plan order, despite the failure.
        assert_eq!(
            *merger.calls.borrow(),
            vec!["KEEP0001", "KEEP0002", "KEEP0003"]
        );
        assert_eq!(report.total_groups, 3);
        assert_eq!(report.applied_groups, 2);
        assert_eq!(report.failed_groups, 1);
        assert_eq!(
            report
                .applied
                .iter()
                .map(|result| result.keeper_key.as_str())
                .collect::<Vec<_>>(),
            vec!["KEEP0001", "KEEP0003"]
        );
        assert_eq!(report.applied[0].source_keys_trashed, vec!["DUPE0001"]);
        assert_eq!(report.failed[0].keeper, "KEEP0002");
        assert_eq!(report.failed[0].sources, vec!["DUPE0002"]);
        // The mapped ZotError code/message survive into the report string.
        assert_eq!(
            report.failed[0].error,
            "update-item-value: Zotero API request failed with status 412"
        );
    }

    #[tokio::test]
    async fn apply_skips_low_confidence_before_calling_writer_by_default() {
        let mut low = group_plan("KEEPLOW1", "DUPELOW1");
        low.confidence = DedupeConfidence::Low;
        low.confidence_note = Some("differing DOIs".to_string());
        let plan = DedupePlan {
            write_backend: WriteBackend::Desktop,
            include_low_confidence: false,
            groups: vec![group_plan("KEEP0001", "DUPE0001"), low],
            total_groups: 2,
            confirm_required: true,
        };
        let merger = FakeMerger {
            fail_keeper: "",
            calls: RefCell::new(Vec::new()),
        };

        let report = apply_dedupe_plan(&merger, &plan).await;

        assert_eq!(*merger.calls.borrow(), vec!["KEEP0001"]);
        assert_eq!(report.write_backend, WriteBackend::Desktop);
        assert_eq!(report.total_groups, 2);
        assert_eq!(report.eligible_groups, 1);
        assert_eq!(report.applied_groups, 1);
        assert_eq!(report.failed_groups, 0);
        assert_eq!(report.skipped_low_confidence_groups, 1);
        assert_eq!(report.skipped_low_confidence[0].keeper.key, "KEEPLOW1");
    }

    #[tokio::test]
    async fn include_low_confidence_calls_writer_for_every_group() {
        let mut low = group_plan("KEEPLOW1", "DUPELOW1");
        low.confidence = DedupeConfidence::Low;
        low.confidence_note = Some("differing DOIs".to_string());
        let plan = DedupePlan {
            write_backend: WriteBackend::Desktop,
            include_low_confidence: true,
            groups: vec![group_plan("KEEP0001", "DUPE0001"), low],
            total_groups: 2,
            confirm_required: true,
        };
        let merger = FakeMerger {
            fail_keeper: "",
            calls: RefCell::new(Vec::new()),
        };

        let report = apply_dedupe_plan(&merger, &plan).await;

        assert_eq!(*merger.calls.borrow(), vec!["KEEP0001", "KEEPLOW1"]);
        assert_eq!(report.eligible_groups, 2);
        assert_eq!(report.applied_groups, 2);
        assert_eq!(report.skipped_low_confidence_groups, 0);
        assert!(report.skipped_low_confidence.is_empty());
    }
}
