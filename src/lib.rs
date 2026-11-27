//! An MCP (Model Context Protocol) stdio server for a local Markdown
//! dictionary.
//!
//! The crate is split into a library and a thin binary so the protocol surface
//! and the dictionary can be tested in-process without spawning a child
//! process:
//!
//! - [`config`] — flags, environment, data-path discovery;
//! - [`dictionary`] — zip/directory loading, parsing, indexing, search;
//! - [`logging`] — stderr and optional file diagnostics;
//! - [`pages`] — clickable citation pages written per retrieved entry;
//! - [`protocol`] — JSON-RPC classification, negotiation, response builders;
//! - [`prompts`] — the `citation` prompt;
//! - [`resources`] — addressable entries and a server overview;
//! - [`server`] — message dispatch and tool-call lifecycle;
//! - [`transport`] — newline-delimited JSON-RPC over stdio;
//! - [`tools`] — tool schemas, argument validation, text rendering;
//! - [`error`] — stable error vocabulary.

pub mod cli;
pub mod config;
pub mod dictionary;
pub mod error;
pub(crate) mod graph_image;
pub mod logging;
pub mod pages;
pub mod prompts;
pub mod protocol;
pub mod resources;
pub mod schema;
#[cfg(feature = "semantic")]
pub mod semantic;
pub mod server;
pub mod tools;
pub mod transport;
