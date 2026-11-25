//! End-to-end tests for both tools over the real stdio transport, using the
//! synthetic fixture dictionary.

mod support;

use serde_json::{Value, json};
use support::{call_tool, initialize, response_for, run_session};

fn run(calls: Vec<Value>) -> (Vec<Value>, String) {
    let mut messages = vec![initialize(1)];
    messages.extend(calls);
    run_session(&messages)
}

fn structured(response: &Value) -> &Value {
    &response["result"]["structuredContent"]
}

#[test]
fn search_finds_dictionary_entries_and_appendix_entries() {
    let (responses, stderr) = run(vec![
        call_tool(
            2,
            "search_dictionary",
            json!({"query":"alpha swelling","limit":3}),
        ),
        call_tool(3, "search_dictionary", json!({"query":"AA-1","limit":3})),
        call_tool(
            4,
            "search_dictionary",
            json!({"query":"jia gen","scope":"medicinal","limit":2}),
        ),
        call_tool(
            5,
            "search_dictionary",
            json!({"query":"Jia Decoction","scope":"formulas","limit":2}),
        ),
        call_tool(
            6,
            "search_dictionary",
            json!({"query":"jin","scope":"weights","limit":2}),
        ),
    ]);
    assert!(stderr.contains("loaded 6 entries"));

    let dictionary = structured(response_for(&responses, 2));
    assert_eq!(dictionary["hits"][0]["source"], "dictionary");
    assert_eq!(dictionary["hits"][0]["term"], "alpha swelling");
    assert_eq!(dictionary["hits"][0]["matched_in"], "headword");
    assert_eq!(dictionary["hits"][0]["anchor"], "a1ASW");

    let point = structured(response_for(&responses, 3));
    assert_eq!(point["hits"][0]["source"], "meridian");
    assert_eq!(point["hits"][0]["id"], "AA-1");
    assert_eq!(point["hits"][0]["matched_in"], "id");

    let medicinal = structured(response_for(&responses, 4));
    assert_eq!(medicinal["hits"][0]["source"], "medicinal");
    assert_eq!(medicinal["hits"][0]["id"], "01.01.01");
    assert_eq!(medicinal["hits"][0]["term"], "jia root");

    let formula = structured(response_for(&responses, 5));
    assert_eq!(formula["hits"][0]["source"], "formulas");
    assert_eq!(formula["hits"][0]["term"], "Jia Decoction");

    let weights = structured(response_for(&responses, 6));
    assert_eq!(weights["hits"][0]["source"], "weights");
    assert!(
        weights["hits"][0]["term"]
            .as_str()
            .unwrap()
            .contains("Weights")
    );
}

#[test]
fn pinyin_and_chinese_lookup_use_generated_indices() {
    let (responses, _) = run(vec![
        call_tool(
            2,
            "search_dictionary",
            json!({"query":"jia3","scope":"dictionary","limit":3}),
        ),
        call_tool(
            3,
            "search_dictionary",
            json!({"query":"jia","scope":"dictionary","limit":3}),
        ),
        call_tool(
            4,
            "search_dictionary",
            json!({"query":"甲肿","scope":"dictionary","limit":3}),
        ),
        call_tool(
            5,
            "search_dictionary",
            json!({"query":"甲腫","scope":"dictionary","limit":3}),
        ),
        call_tool(6, "get_entry", json!({"term":"qīng jiǎ","max_chars":1000})),
        call_tool(7, "get_entry", json!({"term":"清甲","max_chars":1000})),
        call_tool(
            8,
            "get_entry",
            json!({"term":"jia3 san1 li3","source":"meridian","max_chars":1000}),
        ),
        call_tool(
            9,
            "get_entry",
            json!({"term":"下甲","source":"meridian","max_chars":1000}),
        ),
        call_tool(
            10,
            "search_dictionary",
            json!({"query":"甲根","scope":"medicinal","limit":3}),
        ),
    ]);

    let toned = structured(response_for(&responses, 2));
    assert_eq!(toned["hits"][0]["term"], "alpha");
    assert_eq!(toned["hits"][0]["matched_in"], "alias");

    let toneless = structured(response_for(&responses, 3));
    assert_eq!(toneless["hits"][0]["term"], "alpha");

    let simplified = structured(response_for(&responses, 4));
    assert_eq!(simplified["hits"][0]["term"], "alpha swelling");
    assert_eq!(simplified["hits"][0]["anchor"], "a1ASW");

    let traditional = structured(response_for(&responses, 5));
    assert_eq!(traditional["hits"][0]["term"], "alpha swelling");

    let toned_entry = structured(response_for(&responses, 6));
    assert_eq!(toned_entry["term"], "clear heat");

    let traditional_entry = structured(response_for(&responses, 7));
    assert_eq!(traditional_entry["term"], "clear heat");

    let point_pinyin = structured(response_for(&responses, 8));
    assert_eq!(point_pinyin["id"], "AA-1");

    let point_alias = structured(response_for(&responses, 9));
    assert_eq!(point_alias["id"], "AA-1");

    let medicinal = structured(response_for(&responses, 10));
    assert_eq!(medicinal["hits"][0]["source"], "medicinal");
    assert_eq!(medicinal["hits"][0]["id"], "01.01.01");
}

#[test]
fn get_entry_returns_content_aliases_cross_references_and_redirects() {
    let (responses, _) = run(vec![
        call_tool(
            2,
            "get_entry",
            json!({"term":"alpha swelling","max_chars":2000}),
        ),
        call_tool(
            3,
            "get_entry",
            json!({"term":"AA-1","source":"meridian","max_chars":2000}),
        ),
        call_tool(
            4,
            "get_entry",
            json!({"term":"alpha alias","max_chars":1000}),
        ),
        call_tool(
            5,
            "get_entry",
            json!({"term":"jin","source":"weights","max_chars":1000}),
        ),
    ]);

    let entry = structured(response_for(&responses, 2));
    assert_eq!(entry["source"], "dictionary");
    assert_eq!(entry["aliases"]["simplified"], "甲肿");
    assert_eq!(entry["aliases"]["pinyin_ascii"], "jiǎ zhǒng");
    assert!(
        entry["content"]
            .as_str()
            .unwrap()
            .contains("made-up feeling")
    );
    assert!(
        entry["citation"]
            .as_str()
            .unwrap()
            .contains("08-a.md#a1ASW")
    );
    let references = entry["cross_references"].as_array().unwrap();
    assert!(references.iter().any(|link| {
        link["text"] == "clear heat"
            && link["target_term"] == "clear heat"
            && link["resolved"] == true
    }));

    let point = structured(response_for(&responses, 3));
    assert_eq!(point["source"], "meridian");
    assert_eq!(point["id"], "AA-1");
    assert_eq!(point["aliases"]["simplified"], "甲三里");
    assert_eq!(point["aliases"]["pinyin_ascii"], "jia san li");
    assert!(
        point["content"]
            .as_str()
            .unwrap()
            .contains("Fortifies the liver")
    );

    let redirect = structured(response_for(&responses, 4));
    assert_eq!(redirect["term"], "alpha");
    assert_eq!(redirect["redirected_from"], "alpha alias");
    assert!(
        redirect["content"]
            .as_str()
            .unwrap()
            .contains("Also alpha alias")
    );

    let weights = structured(response_for(&responses, 5));
    assert_eq!(weights["source"], "weights");
    assert!(weights["content"].as_str().unwrap().contains("Grams/jin"));
}

#[test]
fn scope_filters_and_not_found_results_are_handled() {
    let (responses, _) = run(vec![
        call_tool(
            2,
            "search_dictionary",
            json!({"query":"alpha swelling","scope":"meridian","limit":3}),
        ),
        call_tool(3, "get_entry", json!({"term":"no such term"})),
    ]);

    let filtered = structured(response_for(&responses, 2));
    assert_eq!(filtered["total"], 0);
    assert!(filtered["hits"].as_array().unwrap().is_empty());

    let not_found = &response_for(&responses, 3)["result"];
    assert_eq!(not_found["isError"], true);
    assert!(
        not_found["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("No entry matches")
    );
}

#[test]
fn argument_and_tool_faults_are_jsonrpc_errors() {
    let (responses, _) = run(vec![
        call_tool(2, "search_dictionary", json!({"query":"x","limit":21})),
        call_tool(3, "get_entry", json!({"term":"x","max_chars":0})),
        call_tool(4, "not_a_tool", json!({})),
        call_tool(5, "search_dictionary", json!({})),
    ]);

    let limit = response_for(&responses, 2);
    assert_eq!(limit["error"]["code"], -32602);
    assert!(
        limit["error"]["message"]
            .as_str()
            .unwrap()
            .contains("between 1 and 20")
    );

    let chars = response_for(&responses, 3);
    assert_eq!(chars["error"]["code"], -32602);

    let unknown = response_for(&responses, 4);
    assert_eq!(unknown["error"]["code"], -32602);
    assert!(
        unknown["error"]["message"]
            .as_str()
            .unwrap()
            .contains("search_dictionary")
    );

    let missing = response_for(&responses, 5);
    assert_eq!(missing["error"]["code"], -32602);
    assert!(
        missing["error"]["message"]
            .as_str()
            .unwrap()
            .contains("\"query\"")
    );
}
