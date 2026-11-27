//! The MCP server itself: a JSON-RPC 2.0 dispatcher.
//!
//! Everything protocol-facing lives in [`McpServer::handle_message`], which is
//! a pure async function of one message plus the injected dictionary, so the
//! whole surface is testable in-process without pipes. The stdio plumbing lives
//! in [`transport`](crate::transport).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

use crate::dictionary::Dictionary;
use crate::error::DictionaryErrorCode;
use crate::logging::Logger;
use crate::pages::Pages;
use crate::prompts::{CITATION_PROMPT, PromptError};
use crate::protocol::{
    Classified, RequestId, classify_message, error_response, negotiate_protocol_version,
    result_response,
};
use crate::resources::{ABOUT_URI, ENTRY_URI_PREFIX, ResourceReadError};
#[cfg(not(feature = "semantic"))]
use crate::tools::call_tool;
#[cfg(feature = "semantic")]
use crate::tools::call_tool_with_semantic;
use crate::tools::{CallToolError, error_message, error_tool_result, tool_definitions};

pub use crate::protocol::error_code as ErrorCode;

pub const SERVER_NAME: &str = "mcp-chinese-dict-search";
pub const SERVER_TITLE: &str = "Chinese Medicine Dictionary Search";
/// Kept in step with `Cargo.toml` by `CARGO_PKG_VERSION`, so the wire version
/// never drifts from the release.
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Text published to clients that support server instructions. Harnesses such
/// as DeepSeek Harness place this in the system prompt, so it stays short.
pub fn server_instructions(title: &str) -> String {
    format!(
        "Search the configured local dictionary \"{title}\" and its appendices. \
     Use search_dictionary for English terms; pinyin in tone-numbered (jia3), tone-marked (jiǎ), or toneless (jia) \
     form; simplified or traditional Chinese characters; acupuncture-point identifiers; herbs; formulas; or \
     measurements. Use get_entry when the exact term or identifier is known, including a citation anchor such as \
     a1ALP. Use related_entries to follow resolved cross-references from a main dictionary entry: outgoing for \
     entries it references, incoming for entries that reference it, and both for either, up to three hops. Use \
     render_graph when the user wants to see that neighborhood as a PNG; it returns an image block and an \
     absolute file path. Use related_network with domain formula, medicinal, or point_combination for \
     appendix relationships, and render_network for their PNG. When the user writes \"tcm citation <anchor>\", \
     call get_entry with that anchor to return the entry text and its full source record. Any entry is also available as the resource \
     dictionary://entry/<anchor>, and the citation prompt returns one entry per reference. Whenever you present a \
     citation anchor, resolve it with get_entry and include the entry text; never leave a bare anchor. Cite the \
     returned file and anchor. Do not read the dictionary directory with the client's own file, shell, grep, or \
     search tools; they cannot resolve the dictionary's aliases, anchors, appendices, or suggestions, and they \
     produce citations that cannot be opened. When you answer, copy the absolute `Source: <path>:<line>` line \
     and any `Entry page: <absolute path>` line from the tool result verbatim. Never write a bare relative \
     citation such as `35-appendix-2.md` or `35-appendix-2.md#a1XXXX`; a terminal resolves those against its \
     working directory and fails."
    )
}

/// One MCP server instance bound to a loaded dictionary.
pub struct McpServer {
    dictionary: Arc<Dictionary>,
    #[cfg(feature = "semantic")]
    semantic: Option<Arc<crate::semantic::SemanticEngine>>,
    logger: Logger,
    pages: Option<Pages>,
    protocol_version: Mutex<Option<String>>,
    initialized: AtomicBool,
    in_flight: Mutex<HashMap<RequestId, CancellationToken>>,
    instructions: String,
}

impl McpServer {
    pub fn new(dictionary: Arc<Dictionary>, logger: Logger) -> Self {
        let instructions = server_instructions(dictionary.title());
        Self::with_instructions(dictionary, logger, instructions)
    }

    /// Pass an empty string to publish no instructions.
    pub fn with_instructions(
        dictionary: Arc<Dictionary>,
        logger: Logger,
        instructions: impl Into<String>,
    ) -> Self {
        Self {
            dictionary,
            #[cfg(feature = "semantic")]
            semantic: None,
            logger,
            pages: None,
            protocol_version: Mutex::new(None),
            initialized: AtomicBool::new(false),
            in_flight: Mutex::new(HashMap::new()),
            instructions: instructions.into(),
        }
    }

    /// Write a page for every `get_entry` result into `pages`, so the citation
    /// can be opened with one click.
    pub fn with_pages(mut self, pages: Pages) -> Self {
        self.pages = Some(pages);
        self
    }

    #[cfg(feature = "semantic")]
    pub fn with_semantic(mut self, semantic: Option<Arc<crate::semantic::SemanticEngine>>) -> Self {
        self.semantic = semantic;
        self
    }

    /// The protocol revision negotiated by the last `initialize`, if any.
    pub fn protocol_version(&self) -> Option<String> {
        self.protocol_version
            .lock()
            .expect("protocol version mutex poisoned")
            .clone()
    }

    /// Whether the client has completed the `initialize` handshake.
    pub fn initialized(&self) -> bool {
        self.initialized.load(Ordering::Relaxed)
    }

    /// Number of tool calls currently running; exposed for tests and diagnostics.
    pub fn in_flight_count(&self) -> usize {
        self.in_flight
            .lock()
            .expect("in-flight mutex poisoned")
            .len()
    }

    /// Abort every running tool call; used when the transport goes away.
    pub fn abort_all(&self) {
        let mut in_flight = self.in_flight.lock().expect("in-flight mutex poisoned");
        for token in in_flight.values() {
            token.cancel();
        }
        in_flight.clear();
    }

    /// Handle one decoded JSON-RPC message.
    ///
    /// Returns the response to send, or `None` for notifications and
    /// unsolicited responses.
    pub async fn handle_message(&self, message: &Value) -> Option<Value> {
        match classify_message(message) {
            Classified::Invalid { id, reason } => {
                self.logger.warn(format!("invalid message: {reason}"));
                Some(error_response(
                    id,
                    ErrorCode::INVALID_REQUEST,
                    format!("Invalid JSON-RPC message: {reason}"),
                ))
            }
            Classified::Response { id } => {
                // This server never sends requests, so a response is unsolicited.
                if let Some(id) = id {
                    self.logger
                        .debug(format!("ignoring unsolicited response {id}"));
                }
                None
            }
            Classified::Notification { method, params } => {
                self.handle_notification(&method, &params);
                None
            }
            Classified::Request { id, method, params } => {
                Some(self.handle_request(id, &method, &params).await)
            }
        }
    }

    /// Parse and handle one raw line of the stdio transport.
    pub async fn handle_line(&self, line: &str) -> Option<Value> {
        match serde_json::from_str::<Value>(line) {
            Ok(message) => self.handle_message(&message).await,
            Err(error) => {
                self.logger.warn(format!("unparseable message: {error}"));
                Some(error_response(
                    None,
                    ErrorCode::PARSE_ERROR,
                    format!("Parse error: {error}"),
                ))
            }
        }
    }

    fn handle_notification(&self, method: &str, params: &Value) {
        match method {
            "notifications/initialized" => {
                self.initialized.store(true, Ordering::Relaxed);
                self.logger.debug("client completed initialization");
            }
            "notifications/cancelled" => {
                let Some(request_id) = params
                    .get("requestId")
                    .and_then(|value| serde_json::from_value::<RequestId>(value.clone()).ok())
                else {
                    return;
                };
                let in_flight = self.in_flight.lock().expect("in-flight mutex poisoned");
                if let Some(token) = in_flight.get(&request_id) {
                    self.logger
                        .debug(format!("cancelling request {request_id}"));
                    token.cancel();
                }
            }
            other => self.logger.debug(format!("ignoring notification {other}")),
        }
    }

    async fn handle_request(&self, id: RequestId, method: &str, params: &Value) -> Value {
        match method {
            "initialize" => self.initialize(id, params),
            "ping" => result_response(Some(id), json!({})),
            "tools/list" => result_response(Some(id), json!({ "tools": tool_definitions() })),
            "tools/call" => self.tools_call(id, params).await,
            "resources/list" => result_response(
                Some(id),
                json!({ "resources": crate::resources::definitions() }),
            ),
            "resources/templates/list" => result_response(
                Some(id),
                json!({ "resourceTemplates": crate::resources::templates() }),
            ),
            "resources/read" => self.resources_read(id, params),
            "prompts/list" => result_response(
                Some(id),
                json!({ "prompts": crate::prompts::definitions() }),
            ),
            "prompts/get" => self.prompts_get(id, params),
            // Deprecated, but some clients still send it; accepting keeps them quiet.
            "logging/setLevel" => result_response(Some(id), json!({})),
            other => {
                self.logger.debug(format!("unsupported method {other}"));
                // `server/discover` deliberately lands here: a 2026-era client
                // uses Method Not Found as the signal to fall back to `initialize`.
                error_response(
                    Some(id),
                    ErrorCode::METHOD_NOT_FOUND,
                    format!(
                        "Method not found: {other}. This server implements initialize, ping, tools/list, \
                         tools/call, resources/list, resources/templates/list, resources/read, prompts/list, \
                         prompts/get and logging/setLevel."
                    ),
                )
            }
        }
    }

    fn initialize(&self, id: RequestId, params: &Value) -> Value {
        let requested = params.get("protocolVersion").and_then(Value::as_str);
        let negotiated = negotiate_protocol_version(requested);

        let mut result = json!({
            "protocolVersion": negotiated,
            "capabilities": {
                "tools": { "listChanged": false },
                "resources": { "subscribe": false, "listChanged": false },
                "prompts": { "listChanged": false },
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "title": SERVER_TITLE,
                "version": SERVER_VERSION,
            },
        });
        if !self.instructions.trim().is_empty() {
            result["instructions"] = json!(self.instructions);
        }

        *self
            .protocol_version
            .lock()
            .expect("protocol version mutex poisoned") = Some(negotiated.to_owned());

        let client_name = params
            .get("clientInfo")
            .and_then(|info| info.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("unknown client");
        self.logger.info(format!(
            "initialize from {client_name} (requested {}, using {negotiated}, {} entries)",
            requested.unwrap_or("none"),
            self.dictionary.entry_count()
        ));

        result_response(Some(id), result)
    }

    async fn tools_call(&self, id: RequestId, params: &Value) -> Value {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return error_response(
                Some(id),
                ErrorCode::INVALID_PARAMS,
                "tools/call requires a string \"name\".",
            );
        };
        if name.is_empty() {
            return error_response(
                Some(id),
                ErrorCode::INVALID_PARAMS,
                "tools/call requires a string \"name\".",
            );
        }

        let arguments = match params.get("arguments") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(map)) => map.clone(),
            Some(_) => {
                return error_response(
                    Some(id),
                    ErrorCode::INVALID_PARAMS,
                    "tools/call \"arguments\" must be a JSON object.",
                );
            }
        };

        let cancel = CancellationToken::new();
        self.in_flight
            .lock()
            .expect("in-flight mutex poisoned")
            .insert(id.clone(), cancel.clone());

        let started = std::time::Instant::now();
        #[cfg(feature = "semantic")]
        let outcome = call_tool_with_semantic(
            &self.dictionary,
            self.semantic.clone(),
            name,
            &arguments,
            cancel,
            self.pages.as_ref(),
        )
        .await;
        #[cfg(not(feature = "semantic"))]
        let outcome = call_tool(
            &self.dictionary,
            name,
            &arguments,
            cancel,
            self.pages.as_ref(),
        )
        .await;

        let result = match outcome {
            Ok(result) => {
                self.logger.debug(format!(
                    "tools/call {name} completed in {} ms",
                    started.elapsed().as_millis()
                ));
                result_response(Some(id.clone()), result)
            }
            Err(CallToolError::Argument(error)) => {
                self.logger.warn(format!("tools/call rejected: {error}"));
                error_response(
                    Some(id.clone()),
                    ErrorCode::INVALID_PARAMS,
                    error.to_string(),
                )
            }
            Err(CallToolError::Unknown(error)) => {
                self.logger.warn(format!("tools/call rejected: {error}"));
                error_response(
                    Some(id.clone()),
                    ErrorCode::INVALID_PARAMS,
                    error.to_string(),
                )
            }
            Err(CallToolError::Dictionary(error)) => {
                self.logger.warn(format!(
                    "tools/call {name} failed: {} ({})",
                    error.message, error.code
                ));
                result_response(
                    Some(id.clone()),
                    error_tool_result(&self.dictionary, &error, self.pages.as_ref()),
                )
            }
        };

        self.in_flight
            .lock()
            .expect("in-flight mutex poisoned")
            .remove(&id);
        result
    }

    fn resources_read(&self, id: RequestId, params: &Value) -> Value {
        let Some(uri) = params.get("uri").and_then(Value::as_str) else {
            return error_response(
                Some(id),
                ErrorCode::INVALID_PARAMS,
                "resources/read requires a string \"uri\".",
            );
        };

        match crate::resources::read(&self.dictionary, uri) {
            Ok(result) => {
                self.logger.debug(format!("resources/read {uri}"));
                result_response(Some(id), result)
            }
            Err(ResourceReadError::Unknown(uri)) => {
                self.logger.warn(format!("unknown resource URI {uri}"));
                error_response(
                    Some(id),
                    ErrorCode::RESOURCE_NOT_FOUND,
                    format!(
                        "Unknown resource URI {uri:?}. Use {ABOUT_URI} or {ENTRY_URI_PREFIX}<anchor-or-term>."
                    ),
                )
            }
            Err(ResourceReadError::Dictionary(error)) => {
                let code = if error.code == DictionaryErrorCode::NotFound {
                    ErrorCode::RESOURCE_NOT_FOUND
                } else {
                    ErrorCode::INTERNAL_ERROR
                };
                self.logger
                    .warn(format!("resources/read {uri} failed: {}", error.message));
                error_response(Some(id), code, error_message(&error))
            }
        }
    }

    fn prompts_get(&self, id: RequestId, params: &Value) -> Value {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return error_response(
                Some(id),
                ErrorCode::INVALID_PARAMS,
                "prompts/get requires a string \"name\".",
            );
        };
        let arguments = match params.get("arguments") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(map)) => map.clone(),
            Some(_) => {
                return error_response(
                    Some(id),
                    ErrorCode::INVALID_PARAMS,
                    "prompts/get \"arguments\" must be a JSON object.",
                );
            }
        };

        match crate::prompts::get(&self.dictionary, name, &arguments) {
            Ok(result) => {
                self.logger.debug(format!("prompts/get {name}"));
                result_response(Some(id), result)
            }
            Err(PromptError::Unknown(name)) => {
                self.logger.warn(format!("unknown prompt {name}"));
                error_response(
                    Some(id),
                    ErrorCode::INVALID_PARAMS,
                    format!("Unknown prompt {name:?}. Available prompts: {CITATION_PROMPT}."),
                )
            }
            Err(PromptError::Argument(message)) => {
                self.logger.warn(format!("prompts/get rejected: {message}"));
                error_response(Some(id), ErrorCode::INVALID_PARAMS, message)
            }
            Err(PromptError::Dictionary(error)) => {
                self.logger
                    .warn(format!("prompts/get {name} failed: {}", error.message));
                error_response(Some(id), ErrorCode::INVALID_PARAMS, error_message(&error))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> McpServer {
        McpServer::new(Arc::new(Dictionary::empty()), Logger::new(false))
    }

    async fn handle(server: &McpServer, message: Value) -> Value {
        server
            .handle_message(&message)
            .await
            .expect("request should produce a response")
    }

    #[tokio::test]
    async fn initialize_negotiates_and_publishes_instructions() {
        let server = server();
        let response = handle(
            &server,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "clientInfo": { "name": "test-client" }
                }
            }),
        )
        .await;

        assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(response["result"]["serverInfo"]["name"], SERVER_NAME);
        assert!(response["result"]["instructions"].is_string());
        assert_eq!(server.protocol_version().as_deref(), Some("2025-06-18"));
    }

    #[tokio::test]
    async fn discover_is_method_not_found_so_modern_clients_fall_back() {
        let server = server();
        let response = handle(
            &server,
            json!({"jsonrpc":"2.0","id":2,"method":"server/discover","params":{}}),
        )
        .await;
        assert_eq!(response["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn ping_and_tools_list_work_before_initialize() {
        let server = server();
        let ping = handle(
            &server,
            json!({"jsonrpc":"2.0","id":1,"method":"ping","params":{}}),
        )
        .await;
        assert_eq!(ping["result"], json!({}));

        let list = handle(
            &server,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        )
        .await;
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 6);
    }

    #[tokio::test]
    async fn unknown_tools_are_invalid_params() {
        let server = server();
        let response = handle(
            &server,
            json!({
                "jsonrpc":"2.0","id":5,"method":"tools/call",
                "params":{"name":"nope","arguments":{}}
            }),
        )
        .await;
        assert_eq!(response["error"]["code"], -32602);
        assert!(
            response["error"]["message"]
                .as_str()
                .unwrap()
                .contains("search_dictionary")
        );
    }

    #[tokio::test]
    async fn not_found_becomes_an_iserror_tool_result() {
        let server = server();
        let response = handle(
            &server,
            json!({
                "jsonrpc":"2.0","id":6,"method":"tools/call",
                "params":{"name":"get_entry","arguments":{"term":"unknown term"}}
            }),
        )
        .await;
        assert_eq!(response["result"]["isError"], true);
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("No entry matches")
        );
        assert_eq!(server.in_flight_count(), 0);
    }

    #[tokio::test]
    async fn initialize_advertises_tools_resources_and_prompts() {
        let server = server();
        let response = handle(
            &server,
            json!({"jsonrpc":"2.0","id":9,"method":"initialize","params":{}}),
        )
        .await;
        let capabilities = &response["result"]["capabilities"];
        assert_eq!(capabilities["tools"]["listChanged"], false);
        assert_eq!(capabilities["resources"]["listChanged"], false);
        assert_eq!(capabilities["resources"]["subscribe"], false);
        assert_eq!(capabilities["prompts"]["listChanged"], false);
    }

    #[tokio::test]
    async fn resources_are_listed_templated_and_readable() {
        let server = server();
        let list = handle(
            &server,
            json!({"jsonrpc":"2.0","id":1,"method":"resources/list","params":{}}),
        )
        .await;
        assert_eq!(list["result"]["resources"][0]["uri"], ABOUT_URI);

        let templates = handle(
            &server,
            json!({"jsonrpc":"2.0","id":2,"method":"resources/templates/list","params":{}}),
        )
        .await;
        let template = templates["result"]["resourceTemplates"][0]["uriTemplate"]
            .as_str()
            .expect("a template URI");
        assert!(template.starts_with(ENTRY_URI_PREFIX));

        let read = handle(
            &server,
            json!({"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri": ABOUT_URI}}),
        )
        .await;
        let text = read["result"]["contents"][0]["text"]
            .as_str()
            .expect("resource text");
        assert!(text.contains(SERVER_NAME));
        assert_eq!(read["result"]["contents"][0]["uri"], ABOUT_URI);
    }

    #[tokio::test]
    async fn unknown_resource_uris_are_resource_not_found() {
        let server = server();
        let response = handle(
            &server,
            json!({"jsonrpc":"2.0","id":4,"method":"resources/read","params":{"uri":"dictionary://nope"}}),
        )
        .await;
        assert_eq!(response["error"]["code"], -32002);
        assert!(
            response["error"]["message"]
                .as_str()
                .unwrap()
                .contains(ENTRY_URI_PREFIX)
        );
    }

    #[tokio::test]
    async fn prompts_are_listed_and_validated() {
        let server = server();
        let list = handle(
            &server,
            json!({"jsonrpc":"2.0","id":1,"method":"prompts/list","params":{}}),
        )
        .await;
        assert_eq!(list["result"]["prompts"][0]["name"], CITATION_PROMPT);

        let missing = handle(
            &server,
            json!({
                "jsonrpc":"2.0","id":2,"method":"prompts/get",
                "params":{"name":CITATION_PROMPT,"arguments":{}}
            }),
        )
        .await;
        assert_eq!(missing["error"]["code"], -32602);
        assert!(
            missing["error"]["message"]
                .as_str()
                .unwrap()
                .contains("reference")
        );

        let unknown = handle(
            &server,
            json!({"jsonrpc":"2.0","id":3,"method":"prompts/get","params":{"name":"nope","arguments":{}}}),
        )
        .await;
        assert_eq!(unknown["error"]["code"], -32602);
        assert!(
            unknown["error"]["message"]
                .as_str()
                .unwrap()
                .contains(CITATION_PROMPT)
        );
    }

    #[tokio::test]
    async fn notifications_are_silent_and_mark_initialization() {
        let server = server();
        assert!(
            server
                .handle_message(&json!({
                    "jsonrpc":"2.0","method":"notifications/initialized"
                }))
                .await
                .is_none()
        );
        assert!(server.initialized());
    }

    #[tokio::test]
    async fn parse_errors_are_reported_with_a_null_id() {
        let server = server();
        let response = server.handle_line("{not json").await.unwrap();
        assert_eq!(response["error"]["code"], -32700);
        assert!(response["id"].is_null());
    }
}
