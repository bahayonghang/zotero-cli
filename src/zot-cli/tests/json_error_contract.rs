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
