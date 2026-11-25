//! Matching normalization.
//!
//! Display text is left untouched; this module produces the case-, diacritic-,
//! escape-, and whitespace-insensitive form used for lookup and ranking.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Lowercase, strip diacritics, fold non-breaking spaces, unescape Markdown,
/// and collapse whitespace.
pub fn normalize(text: &str) -> String {
    let unescaped = unescape_markdown(text);
    let folded: String = unescaped
        .nfkd()
        .filter(|character| !is_combining_mark(*character))
        .collect();

    let mut out = String::with_capacity(folded.len());
    let mut pending_space = false;
    for character in folded.chars() {
        if character.is_whitespace() || character == '\u{a0}' {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        for lower in character.to_lowercase() {
            out.push(lower);
        }
    }
    out
}

/// Remove the backslash Markdown escaping that the ebook conversion inserted
/// before punctuation, while preserving backslashes before letters.
pub fn unescape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\\'
            && let Some(&next) = characters.peek()
            && matches!(next, '[' | ']' | '*' | '|' | '.' | '_' | '<' | '>' | '/')
        {
            out.push(next);
            characters.next();
            continue;
        }
        out.push(character);
    }
    out
}

/// Whether a string contains at least one Han character.
pub fn contains_han(text: &str) -> bool {
    text.chars().any(is_han)
}

pub fn is_han(character: char) -> bool {
    matches!(character as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF)
}

/// Whether a string contains at least one ASCII alphabetic character.
pub fn contains_latin(text: &str) -> bool {
    text.chars()
        .any(|character| character.is_ascii_alphabetic())
}

/// Collapse whitespace for display, without changing case or diacritics.
pub fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_diacritics_and_escapes_fold() {
        assert_eq!(normalize("JIǍ GĒN"), "jia gen");
        assert_eq!(normalize("甲肿 | 甲腫"), "甲肿 | 甲腫");
        assert_eq!(normalize(r"a made-up \[note\]"), "a made-up [note]");
        assert_eq!(normalize("jia3   zhong3"), "jia3 zhong3");
    }

    #[test]
    fn non_breaking_spaces_become_regular_spaces() {
        assert_eq!(normalize("甲肿\u{a0}\\*jia3 / jiǎ"), "甲肿 *jia3 / jia");
    }

    #[test]
    fn han_detection_distinguishes_scripts() {
        assert!(contains_han("甲三里"));
        assert!(!contains_han("Jia Three Li"));
        assert!(contains_latin("aa-1"));
        assert!(!contains_latin("甲三里"));
    }

    #[test]
    fn whitespace_collapses_for_display() {
        assert_eq!(collapse_whitespace("  a \n\t b  "), "a b");
    }
}
