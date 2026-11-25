//! Shared integration-test support: a hermetic server command, a stdio session
//! helper, and a locator for the real archive.

// Each integration-test binary includes this module but uses only a subset of
// the helpers; unused items in one binary are used by another.
#![allow(dead_code)]

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use mcp_chinese_dict_search::dictionary::build_linked_data;
use serde_json::{Value, json};

mod fixture;

/// The invented source dictionary: the input the linked build consumes,
/// shaped like the archive a user supplies (no entry anchors, escaped
/// brackets). It is written to a temporary directory, so the repository holds
/// no dictionary data.
pub fn fixture_source_dir() -> PathBuf {
    static SOURCE: OnceLock<PathBuf> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            let destination = std::env::temp_dir().join(format!(
                "mcp-chinese-dict-search-synthetic-source-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&destination);
            fixture::write(&destination);
            destination
        })
        .clone()
}

/// The dictionary the tests actually serve: the one-time linked copy, built
/// once per test process just as production builds it at install time. It has
/// the index anchors inserted and the conversion's escaped brackets removed.
pub fn fixture_dir() -> PathBuf {
    static LINKED: OnceLock<PathBuf> = OnceLock::new();
    LINKED
        .get_or_init(|| {
            let destination = std::env::temp_dir().join(format!(
                "mcp-chinese-dict-search-linked-fixture-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&destination);
            build_linked_data(&fixture_source_dir(), &destination)
                .expect("the fixture linked copy builds");
            destination
        })
        .clone()
}

/// A command for the real server binary, pointed at the linked fixture copy and
/// stripped of ambient configuration.
pub fn server_command() -> Command {
    let isolated_home = std::env::temp_dir().join(format!(
        "mcp-chinese-dict-search-test-home-{}",
        std::process::id()
    ));

    let mut command = Command::new(env!("CARGO_BIN_EXE_mcp-chinese-dict-search"));
    command
        .env("MCP_CHINESE_DICT_DATA", fixture_dir())
        .env("MCP_CHINESE_DICT_TITLE", "Test Dictionary")
        .env("USERPROFILE", &isolated_home)
        .env("HOME", &isolated_home)
        .env(
            "MCP_CHINESE_DICT_CACHE_DIR",
            isolated_home.join("lookup-cache"),
        )
        .env_remove("MCP_CHINESE_DICT_LOG_FILE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

/// Run one complete stdio session.
///
/// Writes every message, closes stdin, waits for the child to exit, and returns
/// the parsed JSON-RPC responses plus stderr.
pub fn run_session(messages: &[Value]) -> (Vec<Value>, String) {
    run_session_with(server_command(), messages)
}

/// Run one stdio session with a prepared command.
pub fn run_session_with(mut command: Command, messages: &[Value]) -> (Vec<Value>, String) {
    let mut child = command.spawn().expect("server binary starts");
    {
        let stdin = child.stdin.as_mut().expect("stdin is piped");
        for message in messages {
            let line = serde_json::to_string(message).expect("message serializes");
            writeln!(stdin, "{line}").expect("request writes");
        }
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("server exits");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "server failed: {stderr}");

    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    let responses = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("stdout carries only JSON-RPC"))
        .collect();
    (responses, stderr)
}

/// Find the response for a request id.
pub fn response_for(responses: &[Value], id: i64) -> &Value {
    responses
        .iter()
        .find(|response| response["id"].as_i64() == Some(id))
        .unwrap_or_else(|| panic!("no response for id {id}: {responses:?}"))
}

/// Build a `tools/call` request.
pub fn call_tool(id: i64, name: &str, arguments: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    })
}

/// The standard handshake used by tool tests.
pub fn initialize(id: i64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "clientInfo": { "name": "integration-test" }
        }
    })
}

/// Locate the real archive or extracted directory, if present.
///
/// Only an explicitly configured dictionary is used; nothing is discovered by
/// filename, and no dictionary data is committed.
pub fn real_data_path() -> Option<PathBuf> {
    let value = std::env::var("MCP_CHINESE_DICT_DATA").ok()?;
    let path = PathBuf::from(value);
    path.exists().then_some(path)
}
