//! Tool layer: the two MCP tools this server publishes, argument validation,
//! execution against the dictionary, and model-facing text rendering.
//!
//! Schemas deliberately stay inside the narrow JSON Schema subset that harnesses
//! such as DeepSeek Harness enforce: `type` / `properties` / `required` /
//! `additionalProperties` / `items` / `enum` / `const`, plus `description`,
//! `title`, `default`, and `examples`. Bounds the subset cannot express are
//! stated in prose and enforced here in code.

use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

use crate::dictionary::{
    Dictionary, EntryRequest, EntryResponse, Link, MatchKind, Scope, SearchRequest, SearchResponse,
    Suggestion,
};
use crate::error::{DictionaryError, DictionaryErrorCode, ToolArgumentError, UnknownToolError};
use crate::pages::Pages;

pub const SEARCH_TOOL_NAME: &str = "search_dictionary";
pub const ENTRY_TOOL_NAME: &str = "get_entry";

pub const DEFAULT_LIMIT: usize = 5;
pub const MAX_LIMIT: usize = 20;
pub const DEFAULT_MAX_CHARS: usize = 20_000;
pub const MAX_MAX_CHARS: usize = 100_000;

/// Failure while handling `tools/call`, before a tool result is built.
#[derive(Debug, thiserror::Error)]
pub enum CallToolError {
    #[error(transparent)]
    Argument(#[from] ToolArgumentError),
    #[error(transparent)]
    Unknown(#[from] UnknownToolError),
    #[error(transparent)]
    Dictionary(#[from] DictionaryError),
}

/// Output schema derived from the serialized response type, so a field cannot
/// be added to a payload without appearing in the contract.
fn search_output_schema() -> Value {
    crate::schema::output_schema::<SearchResponse>()
}

fn entry_output_schema() -> Value {
    crate::schema::output_schema::<EntryResponse>()
}

/// The tool list advertised by `tools/list`.
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": SEARCH_TOOL_NAME,
            "title": "Search the Chinese medicine dictionary",
            "description": "Search the configured dictionary: English headwords, Chinese characters, pinyin, \
                definitions, acupuncture-point identifiers, medicinal monographs, formula monographs, and \
                weights/measures tables. Pinyin may be tone-numbered (jia3), tone-marked (jiǎ), or toneless \
                (qi); Chinese may be simplified (气) or traditional (氣). Exact pinyin and Chinese keys are \
                resolved through generated lookup indices. Results are ranked and carry a file citation. Use \
                get_entry when the exact term is known.",
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "English term; pinyin in tone-numbered (jia3), tone-marked (jiǎ), or toneless \
                            (qi) form; simplified or traditional Chinese characters; acupuncture-point identifier \
                            such as AA-1; herb name; formula name; or measurement term."
                    },
                    "scope": {
                        "type": "string",
                        "enum": ["all", "dictionary", "meridian", "medicinal", "formulas", "weights"],
                        "description": "Restrict the search to one part of the book. Defaults to all.",
                        "default": "all"
                    },
                    "limit": {
                        "type": "integer",
                        "description": format!(
                            "How many hits to return, 1-{MAX_LIMIT}. Defaults to {DEFAULT_LIMIT}; a value outside \
                             that range is an error, not clamped silently."
                        ),
                        "default": DEFAULT_LIMIT
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            },
            "outputSchema": search_output_schema(),
        },
        {
            "name": ENTRY_TOOL_NAME,
            "title": "Read a dictionary or appendix entry",
            "description": "Return one exact entry from the main dictionary or an appendix: its Chinese and pinyin \
                aliases, full text, resolved cross-references, related acupuncture-point monographs, and a citation. \
                Accepts a headword; pinyin in tone-numbered (jia3), tone-marked (jiǎ), or toneless (jia) form; \
                simplified or traditional Chinese characters; point identifier; herb name or identifier; formula \
                name or identifier; weights/measures section; or a citation anchor such as a1ALP.",
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
            "inputSchema": {
                "type": "object",
                "properties": {
                    "term": {
                        "type": "string",
                        "description": "Exact headword; pinyin in tone-numbered (jia3), tone-marked (jiǎ), or toneless \
                            (qi) form; simplified or traditional Chinese characters; point identifier such as \
                            AA-1; herb or formula name; weights/measures section title; or citation anchor such as \
                            a1ALP."
                    },
                    "source": {
                        "type": "string",
                        "enum": ["all", "dictionary", "meridian", "medicinal", "formulas", "weights"],
                        "description": "Restrict the lookup to one part of the book. Defaults to all.",
                        "default": "all"
                    },
                    "max_chars": {
                        "type": "integer",
                        "description": format!(
                            "Truncate the entry text to this many characters. Defaults to {DEFAULT_MAX_CHARS}; \
                             values above {MAX_MAX_CHARS} are an error."
                        ),
                        "default": DEFAULT_MAX_CHARS
                    }
                },
                "required": ["term"],
                "additionalProperties": false
            },
            "outputSchema": entry_output_schema(),
        }
    ])
}

/// Coerce an optional integer argument, tolerating numeric strings some models
/// emit, and enforce the documented range.
pub fn to_integer(
    value: Option<&Value>,
    field: &str,
    default_value: usize,
    min: usize,
    max: usize,
) -> Result<usize, ToolArgumentError> {
    let Some(value) = value else {
        return Ok(default_value);
    };
    if value.is_null() {
        return Ok(default_value);
    }

    let numeric = match value {
        Value::String(text) if text.trim().is_empty() => return Ok(default_value),
        Value::String(text) => text.trim().parse::<f64>().map_err(|_| {
            ToolArgumentError(format!("\"{field}\" must be a number, got {value}."))
        })?,
        Value::Number(number) => number.as_f64().ok_or_else(|| {
            ToolArgumentError(format!("\"{field}\" must be a number, got {value}."))
        })?,
        other => {
            return Err(ToolArgumentError(format!(
                "\"{field}\" must be a number, got {other}."
            )));
        }
    };

    if !numeric.is_finite() {
        return Err(ToolArgumentError(format!(
            "\"{field}\" must be a finite number, got {value}."
        )));
    }

    let truncated = numeric.trunc();
    if truncated < min as f64 || truncated > max as f64 {
        return Err(ToolArgumentError(format!(
            "\"{field}\" must be between {min} and {max}, got {truncated}."
        )));
    }
    Ok(truncated as usize)
}

fn to_required_string(
    value: Option<&Value>,
    field: &str,
    hint: Option<&str>,
) -> Result<String, ToolArgumentError> {
    match value {
        Some(Value::String(text)) if !text.trim().is_empty() => Ok(text.trim().to_owned()),
        _ => {
            let hint = hint.map(|text| format!(" {text}")).unwrap_or_default();
            Err(ToolArgumentError(format!(
                "\"{field}\" is required and must be a non-empty string.{hint}"
            )))
        }
    }
}

fn to_scope(
    value: Option<&Value>,
    field: &str,
    default_value: Scope,
) -> Result<Scope, ToolArgumentError> {
    let Some(value) = value else {
        return Ok(default_value);
    };
    if value.is_null() {
        return Ok(default_value);
    }
    let text = value.as_str().ok_or_else(|| {
        ToolArgumentError(format!(
            "\"{field}\" must be one of: all, dictionary, meridian, medicinal, formulas, weights."
        ))
    })?;
    Scope::parse(text).ok_or_else(|| {
        ToolArgumentError(format!(
            "\"{field}\" must be one of: all, dictionary, meridian, medicinal, formulas, weights; got {text:?}."
        ))
    })
}

async fn run_search(
    dictionary: &Dictionary,
    args: &Map<String, Value>,
    cancel: CancellationToken,
    pages: Option<&Pages>,
) -> Result<Value, CallToolError> {
    let query = to_required_string(
        args.get("query"),
        "query",
        Some("Example: {\"query\":\"alpha swelling\"}."),
    )?;
    let scope = to_scope(args.get("scope"), "scope", Scope::All)?;
    let limit = to_integer(args.get("limit"), "limit", DEFAULT_LIMIT, 1, MAX_LIMIT)?;

    if cancel.is_cancelled() {
        return Err(DictionaryError::aborted().into());
    }
    let mut response = dictionary.search(&SearchRequest {
        query,
        scope,
        limit,
    })?;
    if cancel.is_cancelled() {
        return Err(DictionaryError::aborted().into());
    }
    if let Some(pages) = pages {
        for suggestion in &mut response.suggestions {
            if let Some(path) = pages.page_for(dictionary, &suggestion.term) {
                suggestion.page = Some(path.display().to_string());
            }
        }
    }

    let text = render_search_text(&response);
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": response,
    }))
}

async fn run_entry(
    dictionary: &Dictionary,
    args: &Map<String, Value>,
    cancel: CancellationToken,
    pages: Option<&Pages>,
) -> Result<Value, CallToolError> {
    let term = to_required_string(
        args.get("term"),
        "term",
        Some("Example: {\"term\":\"alpha swelling\"}."),
    )?;
    let source = to_scope(args.get("source"), "source", Scope::All)?;
    let max_chars = to_integer(
        args.get("max_chars"),
        "max_chars",
        DEFAULT_MAX_CHARS,
        1,
        MAX_MAX_CHARS,
    )?;

    if cancel.is_cancelled() {
        return Err(DictionaryError::aborted().into());
    }
    let response = dictionary.entry(&EntryRequest {
        term,
        source,
        max_chars,
    })?;
    if cancel.is_cancelled() {
        return Err(DictionaryError::aborted().into());
    }

    let mut text = render_entry_text(&response);
    if let Some(pages) = pages {
        // A clicked page must be readable in full, whatever `max_chars` the
        // caller asked for, so re-read a truncated entry at the default bound.
        let page = if response.truncated {
            dictionary
                .entry(&EntryRequest {
                    term: response.term.clone(),
                    source,
                    max_chars: DEFAULT_MAX_CHARS,
                })
                .unwrap_or_else(|_| response.clone())
        } else {
            response.clone()
        };
        text.push_str("\n\n");
        text.push_str(&pages.attach(dictionary, &page));
    }
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": response,
    }))
}

/// Execute one `tools/call`.
pub async fn call_tool(
    dictionary: &Dictionary,
    name: &str,
    args: &Map<String, Value>,
    cancel: CancellationToken,
    pages: Option<&Pages>,
) -> Result<Value, CallToolError> {
    match name {
        SEARCH_TOOL_NAME => run_search(dictionary, args, cancel, pages).await,
        ENTRY_TOOL_NAME => run_entry(dictionary, args, cancel, pages).await,
        other => Err(UnknownToolError {
            name: other.to_owned(),
        }
        .into()),
    }
}

/// Turn an execution failure into an MCP `isError` tool result, so the model
/// sees the reason and can adapt instead of the call vanishing into a protocol
/// error.
pub fn error_tool_result(
    dictionary: &Dictionary,
    error: &DictionaryError,
    pages: Option<&Pages>,
) -> Value {
    let text = match pages {
        Some(pages) => error_text(error, &|term| {
            pages
                .page_for(dictionary, term)
                .map(|path| path.display().to_string())
        }),
        None => error_message(error),
    };
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": true,
    })
}

/// The model-facing text for a dictionary failure. Tool results, resource
/// reads, and prompts share it so the wording never drifts.
pub fn error_message(error: &DictionaryError) -> String {
    error_text(error, &|_| None)
}

fn error_text(error: &DictionaryError, page_for: &dyn Fn(&str) -> Option<String>) -> String {
    match error.code {
        DictionaryErrorCode::NotFound => {
            let mut text = error.message.clone();
            if !error.suggestions.is_empty() {
                text.push_str(" Did you mean: ");
                text.push_str(
                    &error
                        .suggestions
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            match error
                                .suggestion_terms
                                .get(index)
                                .and_then(|term| page_for(term))
                            {
                                Some(page) => format!("\"{value}\" — {page}"),
                                None => format!("\"{value}\""),
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                text.push_str("? Call get_entry again with one of these terms.");
            }
            text
        }
        DictionaryErrorCode::Aborted => "Request cancelled.".to_owned(),
        _ => format!("Dictionary error ({}): {}", error.code, error.message),
    }
}

fn render_suggestions(suggestions: &[Suggestion]) -> String {
    suggestions
        .iter()
        .map(|suggestion| match &suggestion.page {
            Some(page) => format!("{} — {page}", suggestion.text),
            None => suggestion.text.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_search_text(response: &SearchResponse) -> String {
    if response.hits.is_empty() {
        let mut text = format!(
            "No matches for {:?} in scope {}. Try a headword, pinyin, Chinese characters, or an appendix identifier.",
            response.query,
            response.scope.as_str()
        );
        if !response.suggestions.is_empty() {
            text.push_str(&format!(
                "\nDid you mean: {}?",
                render_suggestions(&response.suggestions)
            ));
        }
        return text;
    }

    let mut lines = vec![format!(
        "{} result{} for {:?} (scope {}):",
        response.hits.len(),
        if response.hits.len() == 1 { "" } else { "s" },
        response.query,
        response.scope.as_str()
    )];
    lines.push(String::new());

    for (index, hit) in response.hits.iter().enumerate() {
        let mut header = format!("{}. {}", index + 1, hit.term);
        if let Some(id) = &hit.id {
            header.push_str(&format!(" [{id}]"));
        }
        match &hit.source_file {
            Some(path) => {
                match hit.line {
                    Some(line) => {
                        header.push_str(&format!(" — {} · {path}:{line}", hit.source.as_str()));
                    }
                    None => header.push_str(&format!(" — {} · {path}", hit.source.as_str())),
                }
                if let Some(anchor) = &hit.anchor {
                    header.push_str(&format!(" · anchor {anchor}"));
                }
            }
            None => {
                // Zip sources have no file on disk; keep the canonical citation.
                header.push_str(&format!(" — {} · {}", hit.source.as_str(), hit.file));
                if let Some(anchor) = &hit.anchor {
                    header.push('#');
                    header.push_str(anchor);
                }
            }
        }
        lines.push(header);
        lines.push(format!(
            "   matched in {} · score {}",
            match hit.matched_in {
                MatchKind::Headword => "headword",
                MatchKind::Alias => "alias",
                MatchKind::Body => "body",
                MatchKind::Id => "identifier",
            },
            hit.score
        ));
        if !hit.snippet.is_empty() {
            lines.push(format!("   {}", hit.snippet));
        }
        lines.push(String::new());
    }

    if response.total > response.hits.len() {
        lines.push(format!(
            "(Showing {} of {} matches; raise limit or narrow the query.)",
            response.hits.len(),
            response.total
        ));
    }
    if !response.suggestions.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "Did you mean: {}?",
            render_suggestions(&response.suggestions)
        ));
    }
    if response.hits.iter().any(|hit| hit.anchor.is_some()) {
        lines.push(String::new());
        lines.push(
            "To reopen a citation, ask: tcm citation <anchor> (for example, tcm citation a1ALP)."
                .to_owned(),
        );
    }
    lines.join("\n").trim_end().to_owned()
}

fn citation_tag(response: &EntryResponse) -> Option<String> {
    if let Some(id) = &response.id {
        return Some(id.clone());
    }
    response
        .citation
        .rsplit_once('#')
        .map(|(_, anchor)| anchor.to_owned())
}

pub(crate) fn render_entry_text(response: &EntryResponse) -> String {
    render_entry(response, true, None)
}

/// The same text without the "reopen this citation" instruction, for the page
/// written beside the lookup cache. Cross-references stay plain: the page has
/// no sibling page for them to link to.
pub(crate) fn render_entry_page(response: &EntryResponse) -> String {
    render_entry(response, false, None)
}

/// Resolves a cross-reference to the file name of a page written for it.
type PageLink<'a> = &'a dyn Fn(&Link) -> Option<String>;

/// Page text whose cross-references link to sibling pages that were written
/// for them. A link that resolves to a name that does not exist is exactly the
/// failure VS Code's Markdown preview shows, so callers pass only real pages.
pub(crate) fn render_entry_page_linked(response: &EntryResponse, link: PageLink<'_>) -> String {
    render_entry(response, false, Some(link))
}

fn render_entry(response: &EntryResponse, reopen_hint: bool, link: Option<PageLink<'_>>) -> String {
    let mut lines = vec![format!("# {}", response.term)];
    // Tool text is read in a terminal, where a bare `10-c.md#a1RMS` is resolved
    // against the working directory and fails. With an extracted source the
    // header names the absolute file and line instead. Pages are read in a
    // Markdown preview, which resolves relative to the page, so they keep the
    // canonical citation as code.
    let header = if reopen_hint {
        match &response.source_file {
            Some(path) => {
                let title = response
                    .citation
                    .rsplit_once(", ")
                    .map(|(title, _)| title)
                    .unwrap_or(response.citation.as_str());
                let location = match response.line {
                    Some(line) => format!("{path}:{line}"),
                    None => path.clone(),
                };
                let mut text =
                    format!("{} · {title}\nSource: {location}", response.source.as_str());
                if let Some((_, anchor)) = response.citation.rsplit_once('#') {
                    text.push_str(&format!(" · anchor {anchor}"));
                }
                text
            }
            None => format!("{} · {}", response.source.as_str(), response.citation),
        }
    } else {
        format!("{} · `{}`", response.source.as_str(), response.citation)
    };
    lines.push(header);
    if let Some(redirected) = &response.redirected_from {
        lines.push(format!("(Redirected from: {redirected})"));
    }

    let aliases = &response.aliases;
    let mut alias_parts = Vec::new();
    if !aliases.simplified.is_empty() {
        alias_parts.push(aliases.simplified.clone());
    }
    if !aliases.traditional.is_empty() && aliases.traditional != aliases.simplified {
        alias_parts.push(aliases.traditional.clone());
    }
    if !aliases.pinyin_toned.is_empty() {
        alias_parts.push(format!("({})", aliases.pinyin_toned));
    }
    if !aliases.pinyin_ascii.is_empty() {
        alias_parts.push(format!("({})", aliases.pinyin_ascii));
    }
    if !alias_parts.is_empty() {
        lines.push(format!("Aliases: {}", alias_parts.join(" · ")));
    }
    lines.push(String::new());
    lines.push(response.content.clone());

    if !response.cross_references.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "Cross-references ({}):",
            response.cross_references.len()
        ));
        for cross_reference in &response.cross_references {
            if let Some(term) = &cross_reference.target_term {
                if let Some(name) = link.and_then(|resolve| resolve(cross_reference)) {
                    lines.push(format!("- [{}]({})", cross_reference.text, name));
                } else if let Some(anchor) = &cross_reference.target_anchor {
                    // Tool text is terminal-facing: a bare `file.md#anchor` is
                    // resolved against the working directory and fails, so name
                    // only the anchor the caller can pass back to `get_entry`.
                    lines.push(format!(
                        "- {} → {} (anchor {anchor})",
                        cross_reference.text, term
                    ));
                } else {
                    lines.push(format!("- {} → {}", cross_reference.text, term));
                }
            } else {
                lines.push(format!(
                    "- {} (target not found in the index)",
                    cross_reference.text
                ));
            }
        }
    }

    if !response.related.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "Related appendix entries ({}):",
            response.related.len()
        ));
        for related in &response.related {
            let id = related.id.as_deref().unwrap_or("");
            let mut line = format!("- {} {}", id, related.term).trim().to_owned();
            if !related.chinese.is_empty() {
                line.push_str(&format!(" ({})", related.chinese));
            }
            if !related.summary.is_empty() {
                line.push_str(&format!(": {}", related.summary));
            }
            lines.push(line);
        }
    }

    if response.truncated {
        lines.push(String::new());
        lines.push(format!(
            "[truncated: showing the first {} of {} characters. Call {} again with a larger max_chars to read \
             further.]",
            response.content.chars().count(),
            response.total_chars,
            ENTRY_TOOL_NAME
        ));
    }

    if reopen_hint && let Some(tag) = citation_tag(response) {
        lines.push(String::new());
        lines.push(format!("To reopen this citation, ask: tcm citation {tag}"));
    }

    lines.join("\n").trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(value: Value) -> Map<String, Value> {
        value.as_object().cloned().expect("test args are an object")
    }

    #[test]
    fn numeric_arguments_accept_numbers_and_numeric_strings() {
        assert_eq!(to_integer(Some(&json!(4)), "limit", 5, 1, 20).unwrap(), 4);
        assert_eq!(to_integer(Some(&json!("7")), "limit", 5, 1, 20).unwrap(), 7);
        assert_eq!(
            to_integer(Some(&json!(null)), "limit", 5, 1, 20).unwrap(),
            5
        );
        assert_eq!(to_integer(None, "limit", 5, 1, 20).unwrap(), 5);
    }

    #[test]
    fn numeric_arguments_reject_out_of_range_values() {
        let error = to_integer(Some(&json!(21)), "limit", 5, 1, 20).unwrap_err();
        assert_eq!(
            error.to_string(),
            "\"limit\" must be between 1 and 20, got 21."
        );
    }

    #[test]
    fn required_strings_are_trimmed() {
        assert_eq!(
            to_required_string(Some(&json!("  hello ")), "query", None).unwrap(),
            "hello"
        );
        assert!(to_required_string(Some(&json!("  ")), "query", None).is_err());
        assert!(to_required_string(None, "query", None).is_err());
    }

    #[test]
    fn scopes_parse_and_reject_unknown_values() {
        assert_eq!(
            to_scope(Some(&json!("meridian")), "scope", Scope::All).unwrap(),
            Scope::Meridian
        );
        assert!(to_scope(Some(&json!("nope")), "scope", Scope::All).is_err());
        assert!(to_scope(Some(&json!(3)), "scope", Scope::All).is_err());
    }

    #[test]
    fn tool_definitions_stay_inside_the_supported_schema_subset() {
        let definitions = tool_definitions();
        let tools = definitions.as_array().expect("tools is an array");
        assert_eq!(tools.len(), 2);
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect();
        assert_eq!(names, vec![SEARCH_TOOL_NAME, ENTRY_TOOL_NAME]);
        for tool in tools {
            assert!(tool.get("inputSchema").is_some());
            assert!(tool.get("outputSchema").is_some());
        }
    }

    #[test]
    fn unknown_tool_names_are_reported_with_the_available_set() {
        let error = UnknownToolError {
            name: "nope".to_owned(),
        };
        assert!(error.to_string().contains("search_dictionary, get_entry"));
    }

    #[test]
    fn argument_object_shape_is_preserved_for_tests() {
        assert!(args(json!({"query": "x"})).contains_key("query"));
    }

    #[test]
    fn not_found_results_list_suggestions() {
        let error = DictionaryError::not_found(
            "No entry matches \"alpha\".".to_owned(),
            vec!["alpha swelling".to_owned()],
        );
        let result = error_tool_result(&Dictionary::empty(), &error, None);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Did you mean"));
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn source_names_are_stable() {
        assert_eq!(crate::dictionary::Source::Dictionary.as_str(), "dictionary");
        assert_eq!(crate::dictionary::Source::Weights.as_str(), "weights");
    }

    #[test]
    fn entry_text_explains_how_to_reopen_an_anchor() {
        let response = EntryResponse {
            source: crate::dictionary::Source::Dictionary,
            term: "alpha swelling".to_owned(),
            id: None,
            aliases: Default::default(),
            content: "A made-up alpha definition.".to_owned(),
            cross_references: Vec::new(),
            related: Vec::new(),
            redirected_from: None,
            citation: "Test Dictionary, 08-a.md#a1ASW".to_owned(),
            source_file: None,
            line: None,
            truncated: false,
            total_chars: 26,
        };
        let text = render_entry_text(&response);
        assert!(text.contains("A made-up alpha definition."));
        assert!(text.contains("tcm citation a1ASW"));
    }

    #[test]
    fn a_directory_source_renders_an_absolute_source_line() {
        let response = EntryResponse {
            source: crate::dictionary::Source::Dictionary,
            term: "alpha swelling".to_owned(),
            id: None,
            aliases: Default::default(),
            content: "A made-up feeling of swelling in the alpha region.".to_owned(),
            cross_references: Vec::new(),
            related: Vec::new(),
            redirected_from: None,
            citation: "Test Dictionary, 08-a.md#a1ASW".to_owned(),
            source_file: Some(r"C:\mcp-dict-data\08-a.md".to_owned()),
            line: Some(42),
            truncated: false,
            total_chars: 50,
        };

        let text = render_entry_text(&response);
        assert!(
            text.contains(r"Source: C:\mcp-dict-data\08-a.md:42"),
            "the tool header names a clickable absolute file and line: {text}"
        );
        assert!(text.contains("anchor a1ASW"), "{text}");
        assert!(
            !text.contains("08-a.md#a1ASW"),
            "no bare relative citation remains in tool text: {text}"
        );

        let page = render_entry_page(&response);
        assert!(
            page.contains("`Test Dictionary, 08-a.md#a1ASW`"),
            "pages keep the canonical citation as code: {page}"
        );
    }

    #[test]
    fn pages_link_only_the_cross_references_that_have_a_sibling_page() {
        let response = EntryResponse {
            source: crate::dictionary::Source::Dictionary,
            term: "alpha swelling".to_owned(),
            id: None,
            aliases: Default::default(),
            content: "A made-up feeling of swelling in the alpha region. See clear heat."
                .to_owned(),
            cross_references: vec![
                crate::dictionary::Link {
                    text: "clear heat".to_owned(),
                    target_file: Some("10-c.md".to_owned()),
                    target_anchor: Some("a1CHT".to_owned()),
                    target_term: Some("clear heat".to_owned()),
                    resolved: true,
                },
                crate::dictionary::Link {
                    text: "unresolved".to_owned(),
                    target_file: Some("10-c.md".to_owned()),
                    target_anchor: Some("a1ZZZ".to_owned()),
                    target_term: Some("unresolved".to_owned()),
                    resolved: true,
                },
            ],
            related: Vec::new(),
            redirected_from: None,
            citation: "Test Dictionary, 08-a.md#a1ASW".to_owned(),
            source_file: None,
            line: None,
            truncated: false,
            total_chars: 60,
        };

        let plain = render_entry_page(&response);
        assert!(!plain.contains("]("), "no link without a page: {plain}");
        assert!(
            plain.contains("(anchor a1CHT)"),
            "the anchor stays visible without a path: {plain}"
        );
        assert!(
            !plain.contains("10-c.md#"),
            "tool text has no bare relative citation: {plain}"
        );
        assert!(
            plain.contains("`Test Dictionary, 08-a.md#a1ASW`"),
            "the citation is code in a page: {plain}"
        );

        let linked = render_entry_page_linked(&response, &|link| {
            (link.target_anchor.as_deref() == Some("a1CHT"))
                .then(|| "dictionary-a1CHT.md".to_owned())
        });
        assert!(
            linked.contains("- [clear heat](dictionary-a1CHT.md)"),
            "a written sibling page is linked: {linked}"
        );
        assert!(
            linked.contains("- unresolved → unresolved (anchor a1ZZZ)"),
            "a target without a page stays anchor-only: {linked}"
        );
        assert!(
            !linked.contains("(dictionary-a1ZZZ.md)"),
            "no link is invented: {linked}"
        );
    }
}
