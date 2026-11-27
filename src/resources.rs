//! MCP resources: addressable dictionary entries plus a short server overview.
//!
//! A citation anchor can be read as an address (`dictionary://entry/a1ALP`) by
//! clients that support resources, including harnesses that do not support
//! prompts. The text is the same as `get_entry` returns.

use serde_json::{Value, json};

use crate::dictionary::{Dictionary, EntryRequest, Scope};
use crate::error::DictionaryError;
use crate::server::{SERVER_NAME, SERVER_VERSION};
use crate::tools::{DEFAULT_MAX_CHARS, render_entry_text};

/// The one statically listed resource.
pub const ABOUT_URI: &str = "dictionary://about";
/// Prefix of every entry resource; the rest is a percent-encoded reference.
pub const ENTRY_URI_PREFIX: &str = "dictionary://entry/";

/// One parsed resource URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reference {
    About,
    Entry(String),
}

/// Why `resources/read` failed.
#[derive(Debug)]
pub enum ResourceReadError {
    /// The URI is not part of this server's scheme.
    Unknown(String),
    /// The URI named an entry the dictionary does not contain.
    Dictionary(DictionaryError),
}

/// The `resources/list` result entries.
pub fn definitions() -> Value {
    json!([
        {
            "uri": ABOUT_URI,
            "name": "dictionary-about",
            "title": "About this dictionary server",
            "description": "What this server searches, the measured entry counts, its tools, and its entry-URI \
                scheme.",
            "mimeType": "text/markdown"
        }
    ])
}

/// The `resources/templates/list` result entries.
pub fn templates() -> Value {
    json!([
        {
            "uriTemplate": "dictionary://entry/{reference}",
            "name": "dictionary-entry",
            "title": "Dictionary entry by anchor or term",
            "description": "One entry with its source record. {reference} is a citation anchor such as a1ALP, or \
                an exact headword, pinyin form, or Chinese term; percent-encode spaces and non-ASCII characters.",
            "mimeType": "text/markdown"
        }
    ])
}

/// Parse a resource URI. Only this server's two shapes are accepted.
pub fn parse_uri(uri: &str) -> Option<Reference> {
    if uri == ABOUT_URI {
        return Some(Reference::About);
    }
    let encoded = uri.strip_prefix(ENTRY_URI_PREFIX)?;
    let reference = percent_decode(encoded)?;
    (!reference.trim().is_empty()).then_some(Reference::Entry(reference))
}

/// Read one resource; the value is the `resources/read` result object.
pub fn read(dictionary: &Dictionary, uri: &str) -> Result<Value, ResourceReadError> {
    let reference = parse_uri(uri).ok_or_else(|| ResourceReadError::Unknown(uri.to_owned()))?;
    let text = match reference {
        Reference::About => about_text(dictionary),
        Reference::Entry(reference) => {
            let response = dictionary
                .entry(&EntryRequest {
                    term: reference,
                    source: Scope::All,
                    max_chars: DEFAULT_MAX_CHARS,
                })
                .map_err(ResourceReadError::Dictionary)?;
            render_entry_text(&response)
        }
    };
    Ok(json!({
        "contents": [{ "uri": uri, "mimeType": "text/markdown", "text": text }]
    }))
}

fn about_text(dictionary: &Dictionary) -> String {
    let report = dictionary.report();
    format!(
        "# {SERVER_NAME} {SERVER_VERSION}\n\n\
         Searches the configured dictionary \"{}\" and its appendices from local files; the \
         server makes no network requests.\n\n\
         | Source | Entries |\n|---|---:|\n\
         | Dictionary | {} |\n\
         | Meridian points | {} |\n\
         | Medicinals | {} |\n\
         | Formulas | {} |\n\
         | Weights and measures | {} sections |\n\n\
         Tools: `search_dictionary`, `get_entry`, `related_entries`, `render_graph`, \
         `related_network`, `render_network`.\n\n\
         Resources: `dictionary://entry/<reference>`, where the reference is a citation anchor such as `a1ALP`, \
         a headword, pinyin (`jia3`, `jia gen`), or Chinese (`甲`), percent-encoded where it is not URI-safe.\n\n\
         Prompt: `citation` with a single `reference` argument.",
        dictionary.title(),
        report.main_entries,
        report.meridian_entries,
        report.medicinal_entries,
        report.formula_entries,
        report.weight_sections
    )
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_value(*bytes.get(index + 1)?)?;
            let low = hex_value(*bytes.get(index + 2)?)?;
            decoded.push(high << 4 | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_about_and_entry_uris() {
        assert_eq!(parse_uri(ABOUT_URI), Some(Reference::About));
        assert_eq!(
            parse_uri("dictionary://entry/a1ALP"),
            Some(Reference::Entry("a1ALP".to_owned()))
        );
        assert_eq!(
            parse_uri("dictionary://entry/alpha%20swelling"),
            Some(Reference::Entry("alpha swelling".to_owned()))
        );
        assert_eq!(
            parse_uri("dictionary://entry/%E6%B0%94"),
            Some(Reference::Entry("气".to_owned()))
        );
    }

    #[test]
    fn rejects_other_uris() {
        assert_eq!(parse_uri("dictionary://entry/"), None);
        assert_eq!(parse_uri("dictionary://entry/%20"), None);
        assert_eq!(parse_uri("dictionary://elsewhere/a1ALP"), None);
        assert_eq!(parse_uri("dictionary://entry/%zz"), None);
        assert_eq!(parse_uri("http://example.com/"), None);
    }

    #[test]
    fn about_reports_the_loaded_counts() {
        let text = about_text(&Dictionary::empty());
        assert!(text.contains(SERVER_NAME));
        assert!(text.contains("| Dictionary | 0 |"));
        assert!(text.contains(ENTRY_URI_PREFIX));
    }

    #[test]
    fn unknown_uris_and_missing_entries_are_distinguished() {
        let dictionary = Dictionary::empty();
        assert!(matches!(
            read(&dictionary, "dictionary://nope"),
            Err(ResourceReadError::Unknown(_))
        ));
        assert!(matches!(
            read(&dictionary, &format!("{ENTRY_URI_PREFIX}missing")),
            Err(ResourceReadError::Dictionary(_))
        ));
    }
}
