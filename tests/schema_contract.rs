//! The executable contract between `tools/list` and `tools/call`.
//!
//! Every structured payload must validate against the schema the server
//! advertises for that tool, and the advertised list is checked against a
//! committed snapshot so an intentional schema change is visible in review.

mod support;

use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};
use support::{call_tool, initialize, response_for, run_session};

const SEARCH: &str = "search_dictionary";
const ENTRY: &str = "get_entry";
const RELATED: &str = "related_entries";
const RENDER: &str = "render_graph";
const NETWORK: &str = "related_network";
const RENDER_NETWORK: &str = "render_network";

fn snapshot_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tools-list.json")
}

fn tool<'a>(tools: &'a [Value], name: &str) -> &'a Value {
    tools
        .iter()
        .find(|tool| tool["name"] == name)
        .unwrap_or_else(|| panic!("tools/list does not publish {name}: {tools:?}"))
}

fn assert_narrow(schema: &Value, name: &str) {
    let text = schema.to_string();
    for unsupported in ["\"$ref\"", "\"$defs\"", "\"anyOf\"", "\"null\""] {
        assert!(
            !text.contains(unsupported),
            "{name} outputSchema uses unsupported keyword {unsupported}: {schema}"
        );
    }
}

#[test]
fn structured_results_validate_against_the_advertised_schemas() {
    let calls = [
        (2, SEARCH, json!({"query": "alpha swelling", "limit": 3})),
        (
            3,
            ENTRY,
            json!({"term": "alpha swelling", "source": "dictionary"}),
        ),
        (4, ENTRY, json!({"term": "jia root", "source": "medicinal"})),
        (5, SEARCH, json!({"query": "zzzzzz", "limit": 2})),
        (
            6,
            ENTRY,
            json!({"term": "not-a-real-entry-zzz", "source": "all"}),
        ),
        (
            7,
            RELATED,
            json!({"term": "alpha", "direction": "both", "depth": 2}),
        ),
        (
            8,
            RENDER,
            json!({"term": "alpha", "direction": "both", "depth": 1, "limit": 2}),
        ),
        (
            9,
            NETWORK,
            json!({"term": "Jia Decoction", "domain": "formula", "depth": 1}),
        ),
        (
            10,
            NETWORK,
            json!({"term": "Jia Decoction", "domain": "medicinal", "depth": 1}),
        ),
        (
            11,
            RENDER_NETWORK,
            json!({"term": "Jia Decoction", "domain": "medicinal", "depth": 1}),
        ),
    ];
    let mut messages = vec![
        initialize(1),
        json!({"jsonrpc": "2.0", "id": 100, "method": "tools/list"}),
    ];
    for (id, name, arguments) in &calls {
        messages.push(call_tool(*id, name, arguments.clone()));
    }

    let (responses, stderr) = run_session(&messages);
    let listed = response_for(&responses, 100)["result"]["tools"]
        .as_array()
        .expect("tools is an array")
        .clone();

    for (id, name, _) in &calls {
        let schema = &tool(&listed, name)["outputSchema"];
        assert_narrow(schema, name);

        let result = &response_for(&responses, *id)["result"];
        let structured = &result["structuredContent"];
        if structured.is_null() {
            assert_eq!(
                result["isError"], true,
                "{name} returned neither text nor a structured payload: {result}"
            );
            continue;
        }
        if let Err(error) = jsonschema::validate(schema, structured) {
            panic!(
                "{name} structuredContent violates its outputSchema: {error}\n\
                 payload: {structured}\nstderr: {stderr}"
            );
        }

        // Prove the validator is not vacuous: an undeclared field must fail.
        let mut broken = structured.clone();
        broken["not_in_the_schema"] = json!(true);
        assert!(
            jsonschema::validate(schema, &broken).is_err(),
            "{name} outputSchema accepts undeclared fields"
        );
    }

    let missing = &response_for(&responses, 6)["result"];
    assert_eq!(missing["isError"], true, "an unknown term must be an error");
}

#[test]
fn the_advertised_tool_list_matches_the_checked_in_snapshot() {
    let (responses, stderr) = run_session(&[
        initialize(1),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    ]);
    let tools = &response_for(&responses, 2)["result"]["tools"];
    let path = snapshot_path();

    if std::env::var_os("MCP_UPDATE_TOOLS_LIST").is_some() {
        let mut text = serde_json::to_string_pretty(tools).expect("tools serialize");
        text.push('\n');
        fs::write(&path, text).expect("the snapshot is written");
        return;
    }

    let text = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {}: {error}\n\
             run MCP_UPDATE_TOOLS_LIST=1 cargo test --test schema_contract to create it",
            path.display()
        )
    });
    let expected: Value = serde_json::from_str(&text).expect("the snapshot is valid JSON");
    assert_eq!(
        tools, &expected,
        "tools/list changed. Review the diff, then run \
         MCP_UPDATE_TOOLS_LIST=1 cargo test --test schema_contract to accept it.\nstderr: {stderr}"
    );
}
