//! Tool layer: the MCP tools this server publishes, argument validation,
//! execution against the dictionary, and model-facing text rendering.
//!
//! Schemas deliberately stay inside the narrow JSON Schema subset that harnesses
//! such as DeepSeek Harness enforce: `type` / `properties` / `required` /
//! `additionalProperties` / `items` / `enum` / `const`, plus `description`,
//! `title`, `default`, and `examples`. Bounds the subset cannot express are
//! stated in prose and enforced here in code.

use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

#[cfg(feature = "semantic")]
use crate::semantic::SemanticEngine;

use crate::dictionary::graph;
use crate::dictionary::{
    Dictionary, EntryRequest, EntryResponse, GraphDirection, GraphImageResponse, GraphNode,
    GraphRequest, GraphResponse, Link, MatchKind, NetworkDomain, NetworkImageResponse,
    NetworkRequest, NetworkResponse, Scope, SearchRequest, SearchResponse, Suggestion,
};
use crate::error::{DictionaryError, DictionaryErrorCode, ToolArgumentError, UnknownToolError};
use crate::graph_image;
use crate::pages::Pages;

pub const SEARCH_TOOL_NAME: &str = "search_dictionary";
pub const ENTRY_TOOL_NAME: &str = "get_entry";
pub const RELATED_TOOL_NAME: &str = "related_entries";
pub const RENDER_TOOL_NAME: &str = "render_graph";
pub const NETWORK_TOOL_NAME: &str = "related_network";
pub const RENDER_NETWORK_TOOL_NAME: &str = "render_network";

pub const DEFAULT_LIMIT: usize = 5;
pub const MAX_LIMIT: usize = 20;
pub const DEFAULT_MAX_CHARS: usize = 20_000;
pub const MAX_MAX_CHARS: usize = 100_000;
pub const DEFAULT_GRAPH_LIMIT: usize = 20;
pub const DEFAULT_GRAPH_DEPTH: usize = 1;

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

fn related_output_schema() -> Value {
    crate::schema::output_schema::<GraphResponse>()
}

fn render_output_schema() -> Value {
    crate::schema::output_schema::<GraphImageResponse>()
}

fn network_output_schema() -> Value {
    crate::schema::output_schema::<NetworkResponse>()
}

fn render_network_output_schema() -> Value {
    crate::schema::output_schema::<NetworkImageResponse>()
}

fn network_input_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "term": {
                "type": "string",
                "description": "Formula, medicinal, acupuncture point, or named point-combination \
                    entry to start from."
            },
            "domain": {
                "type": "string",
                "enum": ["formula", "medicinal", "point_combination"],
                "description": "Which appendix network to query: formula-to-formula, \
                    formula-to-medicinal, or point-to-combination."
            },
            "direction": {
                "type": "string",
                "enum": ["outgoing", "incoming", "both"],
                "description": "Follow network edges in one direction or both. Defaults to both.",
                "default": "both"
            },
            "depth": {
                "type": "integer",
                "description": format!(
                    "How many network hops to follow, 1-{}. Defaults to {DEFAULT_GRAPH_DEPTH}; \
                     a value outside the range is an error.",
                    graph::MAX_DEPTH
                ),
                "default": DEFAULT_GRAPH_DEPTH
            },
            "limit": {
                "type": "integer",
                "description": format!(
                    "How many network nodes to return, 1-{}. Defaults to {DEFAULT_GRAPH_LIMIT}; \
                     a value outside the range is an error.",
                    graph::MAX_LIMIT
                ),
                "default": DEFAULT_GRAPH_LIMIT
            }
        },
        "required": ["term", "domain"],
        "additionalProperties": false
    })
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
        },
        {
            "name": RELATED_TOOL_NAME,
            "title": "Follow dictionary cross-references",
            "description": "Follow the resolved cross-references of one main dictionary entry. Use \
                outgoing for entries that this entry references, incoming for entries that reference \
                it, and both for either. Returns up to three hops with the citation, direction, and \
                link text that reached each entry. This tool covers main dictionary entries, not \
                appendix entries.",
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
            "inputSchema": {
                "type": "object",
                "properties": {
                    "term": {
                        "type": "string",
                        "description": "Main dictionary headword, alias, pinyin, Chinese characters, \
                            or citation anchor to start from."
                    },
                    "direction": {
                        "type": "string",
                        "enum": ["outgoing", "incoming", "both"],
                        "description": "Follow links this entry makes, links into it, or both. \
                            Defaults to both.",
                        "default": "both"
                    },
                    "depth": {
                        "type": "integer",
                        "description": format!(
                            "How many hops to follow, 1-{}. Defaults to {DEFAULT_GRAPH_DEPTH}; a \
                             value outside the range is an error.",
                            graph::MAX_DEPTH
                        ),
                        "default": DEFAULT_GRAPH_DEPTH
                    },
                    "limit": {
                        "type": "integer",
                        "description": format!(
                            "How many related entries to return, 1-{}. Defaults to \
                             {DEFAULT_GRAPH_LIMIT}; a value outside the range is an error.",
                            graph::MAX_LIMIT
                        ),
                        "default": DEFAULT_GRAPH_LIMIT
                    }
                },
                "required": ["term"],
                "additionalProperties": false
            },
            "outputSchema": related_output_schema(),
        },
        {
            "name": RENDER_TOOL_NAME,
            "title": "Render a graph neighborhood as a PNG",
            "description": "Render the related-entry graph as a PNG image and return it with an \
                absolute file path. Uses the same term, direction, depth, and limit as \
                related_entries. The image shows the start entry, labelled nodes, and green \
                outgoing / blue incoming edges. It is written to --pages-dir when configured, \
                otherwise to the system temporary directory.",
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
            "inputSchema": {
                "type": "object",
                "properties": {
                    "term": {
                        "type": "string",
                        "description": "Main dictionary headword, alias, pinyin, Chinese characters, \
                            or citation anchor to start from."
                    },
                    "direction": {
                        "type": "string",
                        "enum": ["outgoing", "incoming", "both"],
                        "description": "Follow links this entry makes, links into it, or both. \
                            Defaults to both.",
                        "default": "both"
                    },
                    "depth": {
                        "type": "integer",
                        "description": format!(
                            "How many hops to draw, 1-{}. Defaults to {DEFAULT_GRAPH_DEPTH}; a \
                             value outside the range is an error.",
                            graph::MAX_DEPTH
                        ),
                        "default": DEFAULT_GRAPH_DEPTH
                    },
                    "limit": {
                        "type": "integer",
                        "description": format!(
                            "How many related entries to draw, 1-{}. Defaults to \
                             {DEFAULT_GRAPH_LIMIT}; a value outside the range is an error.",
                            graph::MAX_LIMIT
                        ),
                        "default": DEFAULT_GRAPH_LIMIT
                    }
                },
                "required": ["term"],
                "additionalProperties": false
            },
            "outputSchema": render_output_schema(),
        },
        {
            "name": NETWORK_TOOL_NAME,
            "title": "Follow an appendix network",
            "description": "Follow appendix relationships: formula-to-formula derivation, \
                formula-to-medicinal composition, or point-to-named-combination membership. \
                Edges carry a relation and a source line. The point domain covers named \
                combining-point entries, not arbitrary co-occurrence in prose.",
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
            "inputSchema": network_input_schema(),
            "outputSchema": network_output_schema(),
        },
        {
            "name": RENDER_NETWORK_TOOL_NAME,
            "title": "Render an appendix network as a PNG",
            "description": "Render an appendix network as a PNG using the same arguments as \
                related_network. The file is written to --pages-dir when configured, otherwise \
                the system temporary directory.",
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
            "inputSchema": network_input_schema(),
            "outputSchema": render_network_output_schema(),
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

fn to_network_domain(value: Option<&Value>) -> Result<NetworkDomain, ToolArgumentError> {
    let value = value.ok_or_else(|| {
        ToolArgumentError(
            "\"domain\" is required (formula, medicinal, or point_combination).".to_owned(),
        )
    })?;
    let text = value.as_str().ok_or_else(|| {
        ToolArgumentError(
            "\"domain\" must be one of: formula, medicinal, point_combination.".to_owned(),
        )
    })?;
    NetworkDomain::parse(text).ok_or_else(|| {
        ToolArgumentError(format!(
            "\"domain\" must be one of: formula, medicinal, point_combination; got {text:?}."
        ))
    })
}

fn to_graph_direction(
    value: Option<&Value>,
    field: &str,
    default_value: GraphDirection,
) -> Result<GraphDirection, ToolArgumentError> {
    let Some(value) = value else {
        return Ok(default_value);
    };
    if value.is_null() {
        return Ok(default_value);
    }
    let text = value.as_str().ok_or_else(|| {
        ToolArgumentError(format!(
            "\"{field}\" must be one of: outgoing, incoming, both."
        ))
    })?;
    GraphDirection::parse(text).ok_or_else(|| {
        ToolArgumentError(format!(
            "\"{field}\" must be one of: outgoing, incoming, both; got {text:?}."
        ))
    })
}

enum SearchBackend {
    Lexical,
    #[cfg(feature = "semantic")]
    Semantic(Arc<SemanticEngine>),
}

async fn run_search(
    dictionary: &Arc<Dictionary>,
    backend: SearchBackend,
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
    let request = SearchRequest {
        query,
        scope,
        limit,
    };
    let mut response = match backend {
        SearchBackend::Lexical => dictionary.search(&request)?,
        #[cfg(feature = "semantic")]
        SearchBackend::Semantic(engine) => {
            let dictionary = Arc::clone(dictionary);
            let cancel_task = cancel.clone();
            let handle = tokio::task::spawn_blocking(move || engine.search(&dictionary, &request));
            tokio::select! {
                result = handle => match result {
                    Ok(response) => response?,
                    Err(error) => {
                        return Err(DictionaryError::new(
                            format!("semantic search task failed: {error}"),
                            DictionaryErrorCode::InvalidRequest,
                        )
                        .into());
                    }
                },
                _ = cancel_task.cancelled() => return Err(DictionaryError::aborted().into()),
            }
        }
    };
    if cancel.is_cancelled() {
        return Err(DictionaryError::aborted().into());
    }
    if let Some(pages) = pages {
        for suggestion in &mut response.suggestions {
            if let Some(path) = pages.page_for(dictionary.as_ref(), &suggestion.term) {
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

async fn run_related(
    dictionary: &Dictionary,
    args: &Map<String, Value>,
    cancel: CancellationToken,
) -> Result<Value, CallToolError> {
    let term = to_required_string(
        args.get("term"),
        "term",
        Some("Example: {\"term\":\"alpha swelling\"}."),
    )?;
    let direction = to_graph_direction(args.get("direction"), "direction", GraphDirection::Both)?;
    let depth = to_integer(
        args.get("depth"),
        "depth",
        DEFAULT_GRAPH_DEPTH,
        1,
        graph::MAX_DEPTH,
    )?;
    let limit = to_integer(
        args.get("limit"),
        "limit",
        DEFAULT_GRAPH_LIMIT,
        1,
        graph::MAX_LIMIT,
    )?;

    if cancel.is_cancelled() {
        return Err(DictionaryError::aborted().into());
    }
    let response = dictionary.related(&GraphRequest {
        term,
        direction,
        depth,
        limit,
    })?;
    if cancel.is_cancelled() {
        return Err(DictionaryError::aborted().into());
    }

    let text = render_related_text(&response);
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": response,
    }))
}

async fn run_render(
    dictionary: &Dictionary,
    args: &Map<String, Value>,
    cancel: CancellationToken,
    pages: Option<&Pages>,
) -> Result<Value, CallToolError> {
    let term = to_required_string(
        args.get("term"),
        "term",
        Some("Example: {\"term\":\"spleen\",\"depth\":2}."),
    )?;
    let direction = to_graph_direction(args.get("direction"), "direction", GraphDirection::Both)?;
    let depth = to_integer(
        args.get("depth"),
        "depth",
        DEFAULT_GRAPH_DEPTH,
        1,
        graph::MAX_DEPTH,
    )?;
    let limit = to_integer(
        args.get("limit"),
        "limit",
        DEFAULT_GRAPH_LIMIT,
        1,
        graph::MAX_LIMIT,
    )?;

    if cancel.is_cancelled() {
        return Err(DictionaryError::aborted().into());
    }
    let response = dictionary.related(&GraphRequest {
        term,
        direction,
        depth,
        limit,
    })?;
    if cancel.is_cancelled() {
        return Err(DictionaryError::aborted().into());
    }

    let directory = pages
        .map(|pages| pages.directory().to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("mcp-chinese-dict-graphs"));
    let rendered =
        graph_image::render(&response, &directory).map_err(DictionaryError::unreadable)?;
    let path = std::path::absolute(&rendered.path).unwrap_or(rendered.path);
    let text = format!(
        "{}\n\nImage: {}\n",
        render_related_text(&response),
        path.display()
    );
    let encoded = STANDARD.encode(&rendered.bytes);
    Ok(json!({
        "content": [
            { "type": "text", "text": text },
            { "type": "image", "data": encoded, "mimeType": "image/png" }
        ],
        "structuredContent": GraphImageResponse {
            graph: response,
            image_path: path.display().to_string(),
            mime_type: "image/png".to_owned(),
            width: rendered.width,
            height: rendered.height,
        }
    }))
}

async fn run_network(
    dictionary: &Dictionary,
    args: &Map<String, Value>,
    cancel: CancellationToken,
) -> Result<Value, CallToolError> {
    let term = to_required_string(
        args.get("term"),
        "term",
        Some("Example: {\"term\":\"Four Gentlemen Decoction\",\"domain\":\"formula\"}."),
    )?;
    let domain = to_network_domain(args.get("domain"))?;
    let direction = to_graph_direction(args.get("direction"), "direction", GraphDirection::Both)?;
    let depth = to_integer(
        args.get("depth"),
        "depth",
        DEFAULT_GRAPH_DEPTH,
        1,
        graph::MAX_DEPTH,
    )?;
    let limit = to_integer(
        args.get("limit"),
        "limit",
        DEFAULT_GRAPH_LIMIT,
        1,
        graph::MAX_LIMIT,
    )?;

    if cancel.is_cancelled() {
        return Err(DictionaryError::aborted().into());
    }
    let response = dictionary.related_network(&NetworkRequest {
        term,
        domain,
        direction,
        depth,
        limit,
    })?;
    if cancel.is_cancelled() {
        return Err(DictionaryError::aborted().into());
    }
    let text = render_network_text(&response);
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": response,
    }))
}

async fn run_render_network(
    dictionary: &Dictionary,
    args: &Map<String, Value>,
    cancel: CancellationToken,
    pages: Option<&Pages>,
) -> Result<Value, CallToolError> {
    let term = to_required_string(
        args.get("term"),
        "term",
        Some("Example: {\"term\":\"Four Gentlemen Decoction\",\"domain\":\"formula\"}."),
    )?;
    let domain = to_network_domain(args.get("domain"))?;
    let direction = to_graph_direction(args.get("direction"), "direction", GraphDirection::Both)?;
    let depth = to_integer(
        args.get("depth"),
        "depth",
        DEFAULT_GRAPH_DEPTH,
        1,
        graph::MAX_DEPTH,
    )?;
    let limit = to_integer(
        args.get("limit"),
        "limit",
        DEFAULT_GRAPH_LIMIT,
        1,
        graph::MAX_LIMIT,
    )?;

    if cancel.is_cancelled() {
        return Err(DictionaryError::aborted().into());
    }
    let network = dictionary.related_network(&NetworkRequest {
        term,
        domain,
        direction,
        depth,
        limit,
    })?;
    if cancel.is_cancelled() {
        return Err(DictionaryError::aborted().into());
    }
    let graph = network_to_graph(&network);
    let directory = pages
        .map(|pages| pages.directory().to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("mcp-chinese-dict-graphs"));
    let rendered = graph_image::render(&graph, &directory).map_err(DictionaryError::unreadable)?;
    let path = std::path::absolute(&rendered.path).unwrap_or(rendered.path);
    let text = format!(
        "{}\n\nImage: {}\n",
        render_network_text(&network),
        path.display()
    );
    let encoded = STANDARD.encode(&rendered.bytes);
    Ok(json!({
        "content": [
            { "type": "text", "text": text },
            { "type": "image", "data": encoded, "mimeType": "image/png" }
        ],
        "structuredContent": NetworkImageResponse {
            network,
            image_path: path.display().to_string(),
            mime_type: "image/png".to_owned(),
            width: rendered.width,
            height: rendered.height,
        }
    }))
}

fn network_to_graph(network: &NetworkResponse) -> GraphResponse {
    GraphResponse {
        query: network.query.clone(),
        term: network.term.clone(),
        anchor: None,
        direction: network.direction,
        depth: network.depth,
        nodes: network
            .nodes
            .iter()
            .map(|node| GraphNode {
                source: node.source,
                term: node.term.clone(),
                id: node.id.clone(),
                anchor: None,
                file: node.file.clone(),
                citation: node.citation.clone(),
                source_file: node.source_file.clone(),
                line: node.line,
                distance: node.distance,
                edge_direction: node.edge_direction,
                via: node.via.clone(),
                link_text: format!("{}: {}", node.relation.as_str(), node.label),
                redirect: false,
            })
            .collect(),
        total: network.total,
        truncated: network.truncated,
    }
}

fn render_network_text(response: &NetworkResponse) -> String {
    if response.nodes.is_empty() {
        return format!(
            "No related {} nodes for {:?} in direction {} within depth {}.",
            response.domain.as_str(),
            response.term,
            response.direction.as_str(),
            response.depth
        );
    }
    let mut lines = vec![format!(
        "{} related {} node{} for {:?} ({} · depth {}):",
        response.nodes.len(),
        response.node_type.as_str(),
        if response.nodes.len() == 1 { "" } else { "s" },
        response.term,
        response.direction.as_str(),
        response.depth
    )];
    lines.push(String::new());
    for (index, node) in response.nodes.iter().enumerate() {
        let mut header = format!(
            "{}. {} [{}] — {} · {}",
            index + 1,
            node.term,
            node.node_type.as_str(),
            node.source.as_str(),
            node.citation
        );
        if let Some(id) = &node.id {
            header.push_str(&format!(" · {id}"));
        }
        lines.push(header);
        if let Some(path) = &node.source_file {
            match node.line {
                Some(line) => lines.push(format!("   Source: {path}:{line}")),
                None => lines.push(format!("   Source: {path}")),
            }
        }
        lines.push(format!(
            "   {} · via {} · {}",
            node.edge_direction.as_str(),
            node.via,
            node.relation.as_str()
        ));
        if !node.label.is_empty() {
            lines.push(format!("   {}", node.label));
        }
        lines.push(String::new());
    }
    if response.truncated || response.total > response.nodes.len() {
        lines.push(format!(
            "(Showing {} of {} reachable nodes; lower depth or raise limit.)",
            response.nodes.len(),
            response.total
        ));
    }
    lines.join("\n").trim_end().to_owned()
}

/// Execute one `tools/call`.
pub async fn call_tool(
    dictionary: &Arc<Dictionary>,
    name: &str,
    args: &Map<String, Value>,
    cancel: CancellationToken,
    pages: Option<&Pages>,
) -> Result<Value, CallToolError> {
    call_tool_inner(
        dictionary,
        SearchBackend::Lexical,
        name,
        args,
        cancel,
        pages,
    )
    .await
}

#[cfg(feature = "semantic")]
pub async fn call_tool_with_semantic(
    dictionary: &Arc<Dictionary>,
    semantic: Option<Arc<SemanticEngine>>,
    name: &str,
    args: &Map<String, Value>,
    cancel: CancellationToken,
    pages: Option<&Pages>,
) -> Result<Value, CallToolError> {
    let backend = match semantic {
        Some(engine) => SearchBackend::Semantic(engine),
        None => SearchBackend::Lexical,
    };
    call_tool_inner(dictionary, backend, name, args, cancel, pages).await
}

async fn call_tool_inner(
    dictionary: &Arc<Dictionary>,
    backend: SearchBackend,
    name: &str,
    args: &Map<String, Value>,
    cancel: CancellationToken,
    pages: Option<&Pages>,
) -> Result<Value, CallToolError> {
    let dictionary_ref = dictionary.as_ref();
    match name {
        SEARCH_TOOL_NAME => run_search(dictionary, backend, args, cancel, pages).await,
        ENTRY_TOOL_NAME => run_entry(dictionary_ref, args, cancel, pages).await,
        RELATED_TOOL_NAME => run_related(dictionary_ref, args, cancel).await,
        RENDER_TOOL_NAME => run_render(dictionary_ref, args, cancel, pages).await,
        NETWORK_TOOL_NAME => run_network(dictionary_ref, args, cancel).await,
        RENDER_NETWORK_TOOL_NAME => run_render_network(dictionary_ref, args, cancel, pages).await,
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

fn render_related_text(response: &GraphResponse) -> String {
    if response.nodes.is_empty() {
        return format!(
            "No related entries for {:?} in direction {} within depth {}.",
            response.term,
            response.direction.as_str(),
            response.depth
        );
    }

    let mut lines = vec![format!(
        "{} related entr{} for {:?} ({} · depth {}):",
        response.nodes.len(),
        if response.nodes.len() == 1 {
            "y"
        } else {
            "ies"
        },
        response.term,
        response.direction.as_str(),
        response.depth
    )];
    lines.push(String::new());

    for (index, node) in response.nodes.iter().enumerate() {
        let mut header = format!("{}. {}", index + 1, node.term);
        if let Some(id) = &node.id {
            header.push_str(&format!(" [{id}]"));
        }
        header.push_str(" — dictionary · ");
        match (&node.source_file, node.line) {
            (Some(path), Some(line)) => header.push_str(&format!("{path}:{line}")),
            (Some(path), None) => header.push_str(path),
            _ => header.push_str(&node.citation),
        }
        if let Some(anchor) = &node.anchor {
            header.push_str(&format!(" · anchor {anchor}"));
        }
        lines.push(header);

        if let Some(path) = &node.source_file {
            match node.line {
                Some(line) => lines.push(format!("   Source: {path}:{line}")),
                None => lines.push(format!("   Source: {path}")),
            }
        }
        lines.push(format!(
            "   {} · via {} · link {:?}",
            node.edge_direction.as_str(),
            node.via,
            node.link_text
        ));
        if node.redirect {
            lines.push("   redirect entry".to_owned());
        }
        lines.push(String::new());
    }

    if response.truncated || response.total > response.nodes.len() {
        lines.push(format!(
            "(Showing {} of {} reachable entries; lower depth or raise limit.)",
            response.nodes.len(),
            response.total
        ));
        lines.push(String::new());
    }
    lines.push("To read one, call get_entry with its term or anchor.".to_owned());
    lines.join("\n").trim_end().to_owned()
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
                MatchKind::Semantic => "semantic",
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

    #[tokio::test]
    async fn a_cancelled_search_returns_aborted() {
        let dictionary = Arc::new(Dictionary::empty());
        let mut arguments = Map::new();
        arguments.insert("query".to_owned(), json!("alpha"));
        let cancel = CancellationToken::new();
        cancel.cancel();
        let error = call_tool(&dictionary, SEARCH_TOOL_NAME, &arguments, cancel, None)
            .await
            .unwrap_err();
        match error {
            CallToolError::Dictionary(error) => {
                assert_eq!(error.code, DictionaryErrorCode::Aborted)
            }
            other => panic!("expected an aborted dictionary error, got {other:?}"),
        }
    }

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
        assert_eq!(tools.len(), 6);
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect();
        assert_eq!(
            names,
            vec![
                SEARCH_TOOL_NAME,
                ENTRY_TOOL_NAME,
                RELATED_TOOL_NAME,
                RENDER_TOOL_NAME,
                NETWORK_TOOL_NAME,
                RENDER_NETWORK_TOOL_NAME
            ]
        );
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
        assert!(error.to_string().contains(
            "search_dictionary, get_entry, related_entries, render_graph, \
                     related_network, render_network"
        ));
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

    #[cfg(feature = "semantic")]
    #[tokio::test]
    async fn a_cancelled_semantic_search_returns_aborted() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::Duration;

        use crate::logging::Logger;
        use crate::semantic::{
            EmbeddingProfile, HybridOptions, IndexMetadata, OllamaClient, OllamaEmbedder,
            SemanticEngine, SemanticIndex,
        };

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 8192];
            let _ = stream.read(&mut buffer);
            std::thread::sleep(Duration::from_millis(500));
            let body = r#"{"embeddings":[[1.0,0.0]]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: \
                 {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        });
        let client = OllamaClient::query(&url).unwrap();
        let profile = EmbeddingProfile::for_model("fake");
        let embedder = OllamaEmbedder::new(client.clone(), "fake", profile, 2);
        let index = SemanticIndex::new(
            IndexMetadata {
                package_version: env!("CARGO_PKG_VERSION").to_owned(),
                dictionary_fingerprint: String::new(),
                model_name: "fake".to_owned(),
                model_digest: "digest".to_owned(),
                dimensions: 2,
                profile_hash: "profile".to_owned(),
            },
            Vec::new(),
        )
        .unwrap();
        let engine = SemanticEngine::new(
            embedder,
            index,
            HybridOptions::default(),
            Logger::new(false),
            client,
            "fake",
            "digest",
        );

        let dictionary = Arc::new(Dictionary::empty());
        let arguments = json!({"query": "zzz", "limit": 5});
        let cancel = CancellationToken::new();
        let cancel_task = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            cancel_task.cancel();
        });
        let error = run_search(
            &dictionary,
            SearchBackend::Semantic(Arc::new(engine)),
            arguments.as_object().unwrap(),
            cancel,
            None,
        )
        .await
        .unwrap_err();
        match error {
            CallToolError::Dictionary(error) => {
                assert_eq!(error.code, DictionaryErrorCode::Aborted)
            }
            other => panic!("expected an aborted dictionary error, got {other:?}"),
        }
        handle.join().unwrap();
    }
}
