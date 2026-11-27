//! Error vocabulary shared by the dictionary, the tool layer, and startup.
//!
//! Codes are stable strings so callers and tests can branch on a code instead
//! of matching human-readable text.

use std::fmt;

/// Stable error classifications for dictionary work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictionaryErrorCode {
    /// The data path could not be opened or read.
    DataUnreadable,
    /// The archive or directory contained no usable letter files.
    NoEntries,
    /// A requested term or identifier was not found.
    NotFound,
    /// A request was malformed beyond tool-argument validation.
    InvalidRequest,
    /// The caller cancelled the request.
    Aborted,
}

impl DictionaryErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DataUnreadable => "DICTIONARY_DATA_UNREADABLE",
            Self::NoEntries => "DICTIONARY_NO_ENTRIES",
            Self::NotFound => "DICTIONARY_NOT_FOUND",
            Self::InvalidRequest => "DICTIONARY_INVALID_REQUEST",
            Self::Aborted => "DICTIONARY_ABORTED",
        }
    }
}

impl fmt::Display for DictionaryErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Error raised while loading or querying the dictionary.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct DictionaryError {
    pub message: String,
    pub code: DictionaryErrorCode,
    /// Near matches offered when a lookup fails.
    pub suggestions: Vec<String>,
    /// Resolvable term behind each suggestion, aligned by index with
    /// `suggestions`; the tool layer uses it to write a page for the click.
    pub suggestion_terms: Vec<String>,
}

impl DictionaryError {
    pub fn new(message: impl Into<String>, code: DictionaryErrorCode) -> Self {
        Self {
            message: message.into(),
            code,
            suggestions: Vec::new(),
            suggestion_terms: Vec::new(),
        }
    }

    pub fn unreadable(message: impl Into<String>) -> Self {
        Self::new(message, DictionaryErrorCode::DataUnreadable)
    }

    pub fn no_entries(message: impl Into<String>) -> Self {
        Self::new(message, DictionaryErrorCode::NoEntries)
    }

    pub fn not_found(message: impl Into<String>, suggestions: Vec<String>) -> Self {
        let mut error = Self::new(message, DictionaryErrorCode::NotFound);
        error.suggestions = suggestions;
        error
    }

    pub fn aborted() -> Self {
        Self::new("Request cancelled", DictionaryErrorCode::Aborted)
    }
}

/// Raised when a tool argument is missing or malformed; mapped to JSON-RPC Invalid Params.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ToolArgumentError(pub String);

/// Raised when `tools/call` names a tool this server does not publish.
#[derive(Debug, thiserror::Error)]
#[error(
    "Unknown tool: {name:?}. Available tools: search_dictionary, get_entry, related_entries, \
     render_graph, related_network, render_network."
)]
pub struct UnknownToolError {
    pub name: String,
}

/// Raised when configuration cannot be resolved before serving starts.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ConfigError(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_display_actionable_messages() {
        let error =
            DictionaryError::not_found("No entry matches \"x\".".to_owned(), vec!["y".to_owned()]);
        assert_eq!(error.to_string(), "No entry matches \"x\".");
        assert_eq!(error.code.as_str(), "DICTIONARY_NOT_FOUND");
        assert_eq!(error.suggestions, vec!["y"]);

        assert_eq!(ToolArgumentError("bad".to_owned()).to_string(), "bad");
        assert!(
            UnknownToolError {
                name: "nope".to_owned()
            }
            .to_string()
            .contains("search_dictionary")
        );
        assert_eq!(ConfigError("config".to_owned()).to_string(), "config");
    }
}
