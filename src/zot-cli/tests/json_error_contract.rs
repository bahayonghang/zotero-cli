use std::process::{Command, Output};

use serde_json::Value;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zot"))
        .args(args)
        .output()
        .expect("run zot")
}

fn parse_single_document(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must be exactly one JSON document: {error}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_error(output: &Output, expected_code: &str, expected_exit: i32) -> Value {
    assert_eq!(output.status.code(), Some(expected_exit));
    let value = parse_single_document(output);
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], expected_code);
    assert_eq!(value["meta"]["api_version"], 1);
    value
}

#[test]
fn every_top_level_command_group_has_a_single_document_error_contract() {
    let domain_failure_commands: &[(&str, &[&str])] = &[
        ("doctor", &["doctor"]),
        ("config", &["config", "show"]),
        ("library", &["library", "stats"]),
        ("item", &["item", "get", "ITEM0001"]),
        ("collection", &["collection", "list"]),
        ("graph", &["graph"]),
        ("workspace", &["workspace", "list"]),
        ("sync", &["sync", "update-status"]),
        ("mcp", &["mcp", "serve"]),
    ];

    for (group, command) in domain_failure_commands {
        let mut args = vec!["--json", "--library", "invalid"];
        args.extend_from_slice(command);
        let output = run(&args);
        let value = assert_error(&output, "invalid-library", 1);
        assert_eq!(
            value["error"]["message"], "Invalid library scope: invalid",
            "unexpected message for {group}"
        );
        assert!(output.stderr.is_empty(), "unexpected stderr for {group}");
    }

    let output = run(&["--json", "completions", "powershell"]);
    assert_error(&output, "json-protocol-unsupported", 1);
    assert!(output.stderr.is_empty());
}

#[test]
fn local_update_validation_runs_in_real_process_without_network() {
    // Exercises the nested item dispatcher on Windows' smaller main-thread stack.
    let output = run(&[
        "--json",
        "item",
        "update",
        "ITEMABCD",
        "--write-backend",
        "local",
        "--field",
        "DOI=",
    ]);
    assert_error(&output, "local-write-field", 1);
    assert!(output.stderr.is_empty());
}

#[test]
fn local_tag_validation_runs_in_real_process_without_network() {
    let web_confirm = run(&[
        "--json",
        "item",
        "tag",
        "add",
        "ITEMABCD",
        "--tag",
        "priority",
        "--confirm",
        "--expected-server-id",
        "server",
        "--expected-version",
        "7",
    ]);
    assert_error(&web_confirm, "local-write-options", 1);
    assert!(web_confirm.stderr.is_empty());

    let web_remove = run(&[
        "--json",
        "item",
        "tag",
        "remove",
        "ITEMABCD",
        "--tag",
        "priority",
        "--expected-version",
        "7",
        "--confirm",
        "--expected-server-id",
        "server",
    ]);
    assert_error(&web_remove, "local-write-options", 1);

    for command in ["add", "remove"] {
        let blank = run(&[
            "--json",
            "item",
            "tag",
            command,
            "ITEMABCD",
            "--tag",
            "   ",
            "--write-backend",
            "local",
        ]);
        assert_error(&blank, "local-write-tag", 1);
        assert!(blank.stderr.is_empty());
    }

    let missing = run(&[
        "--json",
        "item",
        "tag",
        "add",
        "ITEMABCD",
        "--write-backend",
        "local",
    ]);
    assert_error(&missing, "local-write-tag", 1);
}

#[test]
fn local_collection_create_validation_runs_in_real_process_without_network() {
    let web_confirm = run(&[
        "--json",
        "collection",
        "create",
        "待读",
        "--confirm",
        "--expected-server-id",
        "server",
        "--expected-library-version",
        "12",
    ]);
    assert_error(&web_confirm, "local-write-options", 1);
    assert!(web_confirm.stderr.is_empty());

    let blank = run(&[
        "--json",
        "collection",
        "create",
        "   ",
        "--write-backend",
        "local",
    ]);
    assert_error(&blank, "local-write-collection", 1);

    let parent = run(&[
        "--json",
        "collection",
        "create",
        "待读",
        "--parent",
        "bad",
        "--write-backend",
        "local",
    ]);
    assert_error(&parent, "local-api-key", 1);
}

#[test]
fn organize_apply_without_confirm_is_one_json_error_and_does_not_require_network() {
    let output = run(&[
        "--json",
        "library",
        "organize",
        "apply",
        "--plan",
        "missing-plan.json",
        "--report",
        "missing-report.json",
        "--write-backend",
        "local",
    ]);
    assert_error(&output, "local-organize-confirm", 1);
    assert!(output.stderr.is_empty());
}

#[test]
fn organize_hash_and_schema_fail_in_real_process_before_network() {
    use sha2::{Digest, Sha256};

    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("plan.json");
    let report = dir.path().join("report.json");
    let bytes = br#"{"schema_version":1,"backend":"local","server_id":"server","library":"user","profile":null,"max_affected":50,"item_count":0,"action_count":0,"changed_count":0,"unchanged_count":0,"items":[]}"#;
    std::fs::write(&path, bytes).expect("plan");
    let hash = format!("{:x}", Sha256::digest(bytes));
    for (expected, error) in [
        ("wrong", "local-organize-hash"),
        (hash.as_str(), "local-organize-schema"),
    ] {
        let output = run(&[
            "--json",
            "library",
            "organize",
            "apply",
            "--plan",
            path.to_str().expect("path"),
            "--report",
            report.to_str().expect("path"),
            "--write-backend",
            "local",
            "--confirm",
            "--expected-plan-sha256",
            expected,
        ]);
        assert_error(&output, error, 1);
        assert!(output.stderr.is_empty());
        assert!(!report.exists());
    }
    let request = dir.path().join("request.json");
    std::fs::write(&request, br#"{"schema_version":1,"items":[{"item_key":"ITEMABCD","add_tags":["review"],"library":"group:12"}]}"#).expect("request");
    let output = run(&[
        "--json",
        "library",
        "organize",
        "plan",
        "--file",
        request.to_str().expect("path"),
        "--output",
        report.to_str().expect("path"),
        "--write-backend",
        "local",
    ]);
    assert_error(&output, "local-organize-schema", 1);
    assert!(output.stderr.is_empty());
    assert!(!report.exists());
}

#[test]
fn json_parse_failure_is_enveloped_and_uses_exit_code_two() {
    let output = run(&["--json", "--not-a-real-option"]);
    let value = assert_error(&output, "cli-parse", 2);

    assert_eq!(value["error"]["message"], "Invalid command-line arguments");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Usage:"));
    assert!(output.stderr.is_empty());
}

#[test]
fn verbose_parse_detail_stays_on_stderr_and_stdout_is_unchanged() {
    let normal = run(&["--json", "--not-a-real-option"]);
    let verbose = run(&["--json", "--verbose", "--not-a-real-option"]);

    assert_error(&verbose, "cli-parse", 2);
    assert_eq!(normal.stdout, verbose.stdout);
    let stderr = String::from_utf8_lossy(&verbose.stderr);
    assert!(stderr.contains("Caused by:"));
    assert!(stderr.contains("--not-a-real-option"));
}

#[test]
fn graph_server_rejects_json_before_database_or_listener_output() {
    let output = run(&["--json", "graph", "serve", "--no-open", "--port", "0"]);
    let value = assert_error(&output, "json-protocol-unsupported", 1);

    assert_eq!(
        value["error"]["message"],
        "`graph serve` uses a long-running human output protocol"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("serving"));
    assert!(output.stderr.is_empty());
}
