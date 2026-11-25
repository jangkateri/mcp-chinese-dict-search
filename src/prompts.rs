//! MCP prompts: one `citation` prompt that returns an entry for a reference.
//!
//! Clients that expose MCP prompts as commands (Claude Code, OpenCode) surface
//! it as `/mcp__<server>__citation`; the result is injected into the
//! conversation. Harnesses without prompt support (DeepSeek Harness, Codex)
//! ignore the capability and keep using the tools and resources.

use serde_json::{Map, Value, json};

use crate::dictionary::{Dictionary, EntryRequest, Scope};
use crate::error::DictionaryError;
use crate::tools::{DEFAULT_MAX_CHARS, render_entry_text};

pub const CITATION_PROMPT: &str = "citation";

/// Why `prompts/get` failed.
#[derive(Debug)]
pub enum PromptError {
    /// No prompt with this name is published.
    Unknown(String),
    /// The prompt was called without its required argument.
    Argument(String),
    /// The reference named no entry.
    Dictionary(DictionaryError),
}

/// The `prompts/list` result entries.
pub fn definitions() -> Value {
    json!([
        {
            "name": CITATION_PROMPT,
            "title": "Show a dictionary entry by citation",
            "description": "Resolve a citation anchor or exact term and return the entry text with its source \
                record.",
            "arguments": [
                {
                    "name": "reference",
                    "description": "Citation anchor (a1ALP), headword, pinyin (jia3), or Chinese term. One token: \
                        clients split arguments on whitespace, so use an anchor or a single word.",
                    "required": true
                }
            ]
        }
    ])
}

/// Resolve one `prompts/get`; the value is the `prompts/get` result object.
pub fn get(
    dictionary: &Dictionary,
    name: &str,
    arguments: &Map<String, Value>,
) -> Result<Value, PromptError> {
    if name != CITATION_PROMPT {
        return Err(PromptError::Unknown(name.to_owned()));
    }
    let reference = arguments
        .get("reference")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            PromptError::Argument(
                "The \"citation\" prompt requires a \"reference\" argument, for example \
                 {\"reference\":\"a1ALP\"}."
                    .to_owned(),
            )
        })?;

    let response = dictionary
        .entry(&EntryRequest {
            term: reference.to_owned(),
            source: Scope::All,
            max_chars: DEFAULT_MAX_CHARS,
        })
        .map_err(PromptError::Dictionary)?;

    Ok(json!({
        "description": format!("Dictionary entry for {reference:?}"),
        "messages": [{
            "role": "user",
            "content": {
                "type": "text",
                "text": format!(
                    "Show this entry from \"{}\" and cite its file and anchor.\n\n{}",
                    dictionary.title(),
                    render_entry_text(&response)
                )
            }
        }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_requires_a_single_reference_argument() {
        let definitions = definitions();
        assert_eq!(definitions[0]["name"], CITATION_PROMPT);
        assert_eq!(definitions[0]["arguments"][0]["name"], "reference");
        assert_eq!(definitions[0]["arguments"][0]["required"], true);
    }

    #[test]
    fn a_missing_or_blank_reference_is_an_argument_error() {
        let dictionary = Dictionary::empty();
        assert!(matches!(
            get(&dictionary, CITATION_PROMPT, &Map::new()),
            Err(PromptError::Argument(_))
        ));
        let blank = json!({ "reference": "  " });
        assert!(matches!(
            get(&dictionary, CITATION_PROMPT, blank.as_object().unwrap()),
            Err(PromptError::Argument(_))
        ));
    }

    #[test]
    fn unknown_prompts_are_named() {
        let error = get(&Dictionary::empty(), "nope", &Map::new()).unwrap_err();
        assert!(matches!(error, PromptError::Unknown(name) if name == "nope"));
    }

    #[test]
    fn a_reference_with_no_entry_is_a_dictionary_error() {
        let arguments = json!({ "reference": "a1ALP" });
        assert!(matches!(
            get(
                &Dictionary::empty(),
                CITATION_PROMPT,
                arguments.as_object().unwrap()
            ),
            Err(PromptError::Dictionary(_))
        ));
    }
}
