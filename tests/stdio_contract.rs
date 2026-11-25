//! Contract tests that drive the real executable over a real stdio pipe.
//!
//! These assert the two things unit tests cannot: stdout carries only
//! newline-delimited JSON-RPC, and the legacy handshake works in a child
//! process with no network or user configuration.

mod support;

use serde_json::json;
use support::{response_for, run_session, run_session_with, server_command};

#[test]
fn handshake_tools_list_and_discover_work_over_stdio() {
    let (responses, stderr) = run_session(&[
        support::initialize(1),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        json!({"jsonrpc":"2.0","id":3,"method":"server/discover","params":{}}),
        json!({"jsonrpc":"2.0","id":4,"method":"ping","params":{}}),
    ]);

    assert_eq!(responses.len(), 4, "one response per request");

    let initialize = &response_for(&responses, 1)["result"];
    assert_eq!(initialize["protocolVersion"], "2025-11-25");
    assert_eq!(initialize["serverInfo"]["name"], "mcp-chinese-dict-search");
    assert!(initialize["instructions"].is_string());

    let tools = response_for(&responses, 2)["result"]["tools"]
        .as_array()
        .expect("tools is an array");
    assert_eq!(tools.len(), 2);
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert_eq!(names, vec!["search_dictionary", "get_entry"]);

    // The deliberate legacy-era signal: a 2026-capable client falls back.
    assert_eq!(response_for(&responses, 3)["error"]["code"], -32601);
    assert_eq!(response_for(&responses, 4)["result"], json!({}));

    assert!(stderr.contains("initialize from integration-test"));
    assert!(stderr.contains("loaded 6 entries"));
}

#[test]
fn help_goes_to_stdout_and_errors_go_to_stderr() {
    let help = server_command().arg("--help").output().expect("help runs");
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("search_dictionary"));
    assert!(text.contains("get_entry"));
    assert!(text.contains("--data"));

    let failure = server_command()
        .arg("--not-a-flag")
        .output()
        .expect("bad arg runs");
    assert_eq!(failure.status.code(), Some(2));
    assert!(
        failure.stdout.is_empty(),
        "configuration errors stay off stdout"
    );
    assert!(String::from_utf8_lossy(&failure.stderr).contains("unknown argument"));
}

#[test]
fn version_is_printed_on_stdout() {
    let output = server_command()
        .arg("--version")
        .output()
        .expect("version runs");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn missing_data_is_a_configuration_error() {
    let output = server_command()
        .env(
            "MCP_CHINESE_DICT_DATA",
            "C:/definitely/not/here/dictionary.zip",
        )
        .output()
        .expect("missing data runs");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not exist"));
}

#[test]
fn build_indices_writes_the_cache_and_exits_without_serving() {
    let cache = std::env::temp_dir().join(format!(
        "mcp-chinese-dict-build-indices-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&cache);

    let output = server_command()
        .arg("--build-indices")
        .env("MCP_CHINESE_DICT_CACHE_DIR", &cache)
        .output()
        .expect("build runs");

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "build failed: {stderr}");
    assert!(output.stdout.is_empty(), "build mode keeps stdout clean");
    assert!(
        stderr.contains("built "),
        "build counts are reported: {stderr}"
    );
    assert!(
        stderr.contains("Han keys"),
        "build counts are reported: {stderr}"
    );

    let written: Vec<String> = std::fs::read_dir(&cache)
        .expect("cache directory exists")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        written
            .iter()
            .any(|name| name.contains(".lookup-cache-v") && name.ends_with(".json")),
        "a cache file is written: {written:?}"
    );

    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn build_linked_data_writes_a_verified_copy_that_serves() {
    let base = std::env::temp_dir().join(format!(
        "mcp-chinese-dict-linked-e2e-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&base);
    let copy = base.join("linked");

    let output = server_command()
        .env("MCP_CHINESE_DICT_DATA", support::fixture_source_dir())
        .arg("--build-linked-data")
        .arg(&copy)
        .output()
        .expect("build runs");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "build failed: {stderr}");
    assert!(output.stdout.is_empty(), "build mode keeps stdout clean");
    assert!(stderr.contains("built linked copy"), "{stderr}");
    assert!(
        stderr.contains("6 anchors inserted"),
        "every indexed fixture entry gets an anchor: {stderr}"
    );

    let page = std::fs::read_to_string(copy.join("08-a.md")).expect("the copy is written");
    assert!(
        page.contains("<a id=\"a1ASW\"></a>\n\\*alpha swelling"),
        "the entry anchor is inserted: {page}"
    );

    let (responses, stderr) = run_session_with(
        {
            let mut command = server_command();
            command.env("MCP_CHINESE_DICT_DATA", &copy);
            command
        },
        &[
            support::initialize(1),
            support::call_tool(2, "get_entry", json!({"term":"alpha swelling"})),
        ],
    );
    assert_eq!(
        response_for(&responses, 2)["result"]["structuredContent"]["citation"],
        "Test Dictionary, 08-a.md#a1ASW"
    );
    assert!(stderr.contains("loaded 6 entries"), "{stderr}");

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn the_served_fixture_is_the_linked_copy_production_uses() {
    let linked = support::fixture_dir();

    let letter = std::fs::read_to_string(linked.join("08-a.md")).expect("the linked letter file");
    assert!(
        letter.contains("<a id=\"a1ASW\"></a>\n\\*alpha swelling"),
        "the served copy carries the entry anchors: {letter}"
    );

    let appendix =
        std::fs::read_to_string(linked.join("35-appendix-2-Medicinal.md")).expect("the appendix");
    assert!(
        appendix.contains("[01.01.01] JIǍ GĒN"),
        "the served copy unescapes monograph brackets: {appendix}"
    );
    assert!(
        !appendix.contains("\\[") && !appendix.contains("\\]"),
        "the served copy keeps no conversion escapes: {appendix}"
    );
}

#[test]
fn misspellings_return_english_and_pinyin_suggestions() {
    let (responses, _) = run_session(&[
        support::initialize(1),
        support::call_tool(2, "get_entry", json!({"term":"alpha sweling"})),
        support::call_tool(3, "search_dictionary", json!({"query":"alpha sweling"})),
        support::call_tool(4, "get_entry", json!({"term":"jia3 zhon3"})),
        support::call_tool(5, "search_dictionary", json!({"query":"jiazhong"})),
    ]);

    let entry = response_for(&responses, 2)["result"]["content"][0]["text"]
        .as_str()
        .expect("entry text");
    assert!(entry.contains("Did you mean"), "{entry}");
    assert!(entry.contains("alpha swelling"), "{entry}");

    let search = response_for(&responses, 3)["result"]["content"][0]["text"]
        .as_str()
        .expect("search text");
    assert!(search.contains("Did you mean"), "{search}");
    assert!(search.contains("alpha swelling"), "{search}");

    let pinyin = response_for(&responses, 4)["result"]["content"][0]["text"]
        .as_str()
        .expect("entry text");
    assert!(
        pinyin.contains("jia zhong (alpha swelling)"),
        "a toneless near key is suggested: {pinyin}"
    );

    let compact = response_for(&responses, 5)["result"]["content"][0]["text"]
        .as_str()
        .expect("search text");
    assert!(
        compact.contains("jia zhong (alpha swelling)"),
        "a space-free typo reaches the two-word key: {compact}"
    );
}

#[test]
fn suggestions_name_a_clickable_page_when_pages_are_enabled() {
    let base = std::env::temp_dir().join(format!(
        "mcp-chinese-dict-suggestion-pages-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&base);
    let pages = base.join("pages");

    let (responses, _) = run_session_with(
        {
            let mut command = server_command();
            command.arg("--pages-dir").arg(&pages);
            command
        },
        &[
            support::initialize(1),
            support::call_tool(2, "get_entry", json!({"term":"alpha sweling"})),
            support::call_tool(3, "search_dictionary", json!({"query":"alpha sweling"})),
        ],
    );

    let page_path = pages.join("dictionary-a1ASW.md");
    let entry = response_for(&responses, 2)["result"]["content"][0]["text"]
        .as_str()
        .expect("entry text");
    assert!(
        entry.contains(&format!("alpha swelling\" — {}", page_path.display()))
            || entry.contains(&format!("alpha swelling — {}", page_path.display())),
        "the suggestion names its page: {entry}"
    );
    assert!(page_path.exists(), "the suggestion page is written");

    let search = response_for(&responses, 3)["result"]["content"][0]["text"]
        .as_str()
        .expect("search text");
    assert!(
        search.contains(&page_path.display().to_string()),
        "a weak search result names the suggestion page: {search}"
    );

    let page = std::fs::read_to_string(&page_path).expect("the page is readable");
    assert!(page.starts_with("# alpha swelling\n"));

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn resources_and_the_citation_prompt_serve_fixture_entries() {
    let (responses, stderr) = run_session(&[
        support::initialize(1),
        json!({"jsonrpc":"2.0","id":2,"method":"resources/list","params":{}}),
        json!({"jsonrpc":"2.0","id":3,"method":"resources/templates/list","params":{}}),
        json!({"jsonrpc":"2.0","id":4,"method":"resources/read",
               "params":{"uri":"dictionary://entry/a1ASW"}}),
        json!({"jsonrpc":"2.0","id":5,"method":"resources/read",
               "params":{"uri":"dictionary://entry/alpha%20swelling"}}),
        json!({"jsonrpc":"2.0","id":6,"method":"resources/read",
               "params":{"uri":"dictionary://nope"}}),
        json!({"jsonrpc":"2.0","id":7,"method":"prompts/list","params":{}}),
        json!({"jsonrpc":"2.0","id":8,"method":"prompts/get",
               "params":{"name":"citation","arguments":{"reference":"a1ASW"}}}),
    ]);

    assert_eq!(
        response_for(&responses, 2)["result"]["resources"][0]["uri"],
        "dictionary://about"
    );
    assert!(
        response_for(&responses, 3)["result"]["resourceTemplates"][0]["uriTemplate"]
            .as_str()
            .unwrap()
            .starts_with("dictionary://entry/")
    );

    let by_anchor = response_for(&responses, 4)["result"]["contents"][0]["text"]
        .as_str()
        .expect("entry text");
    assert!(by_anchor.contains("alpha swelling"));
    assert!(by_anchor.contains("08-a.md"));

    let by_term = response_for(&responses, 5)["result"]["contents"][0]["text"]
        .as_str()
        .expect("entry text");
    assert_eq!(by_term, by_anchor, "percent-encoded references decode");

    assert_eq!(response_for(&responses, 6)["error"]["code"], -32002);

    assert_eq!(
        response_for(&responses, 7)["result"]["prompts"][0]["name"],
        "citation"
    );
    let message = response_for(&responses, 8)["result"]["messages"][0]["content"]["text"]
        .as_str()
        .expect("prompt text");
    assert!(message.contains("alpha swelling"));
    assert!(message.contains("08-a.md"));

    assert!(stderr.contains("loaded 6 entries"));
}

#[test]
fn pages_dir_writes_a_clickable_entry_page() {
    let base =
        std::env::temp_dir().join(format!("mcp-chinese-dict-pages-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let pages = base.join("pages");

    let (responses, stderr) = run_session_with(
        {
            let mut command = server_command();
            command.arg("--pages-dir").arg(&pages);
            command
        },
        &[
            support::initialize(1),
            support::call_tool(
                2,
                "get_entry",
                json!({"term":"alpha swelling","max_chars":20}),
            ),
        ],
    );

    let text = response_for(&responses, 2)["result"]["content"][0]["text"]
        .as_str()
        .expect("entry text");
    assert!(
        text.contains("[truncated"),
        "the tool result honours max_chars: {text}"
    );
    let page_path = pages.join("dictionary-a1ASW.md");
    assert!(
        text.contains(&format!("Entry page: {}", page_path.display())),
        "result names the page: {text}"
    );

    let link_line = text
        .lines()
        .find(|line| line.starts_with("Clickable link: "))
        .expect("the result carries a link");
    let url = link_line
        .split("](")
        .nth(1)
        .and_then(|rest| rest.strip_suffix(')'))
        .expect("the link carries a URL");
    assert!(url.starts_with("file:///"), "got {url}");
    assert!(!url.contains(' '), "link URLs are encoded: {url}");

    let page = std::fs::read_to_string(&page_path).expect("the page is written");
    assert!(page.starts_with("# alpha swelling\n"));
    assert!(page.contains("`Test Dictionary, 08-a.md#a1ASW`"));
    assert!(page.contains("Generated by mcp-chinese-dict-search"));
    assert!(
        page.contains("- [clear heat](dictionary-a1CHT.md)"),
        "the cross-reference links to a page in the same directory: {page}"
    );
    let target = std::fs::read_to_string(pages.join("dictionary-a1CHT.md"))
        .expect("the cross-reference page is written");
    assert!(target.starts_with("# clear heat\n"));
    assert!(
        target.contains("A made-up treatment method that clears heat."),
        "the linked page holds the target entry: {target}"
    );
    assert!(
        !page.contains("[truncated"),
        "the page holds the full entry: {page}"
    );
    assert!(
        page.chars().count() > 120,
        "the page is longer than the requested max_chars"
    );
    assert!(
        page.contains("See clear heat."),
        "the page holds text the truncated result omitted"
    );

    assert!(stderr.contains("loaded 6 entries"));
    let _ = std::fs::remove_dir_all(&base);
}
