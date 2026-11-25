//! Pinyin and Han keys for the generated lookup indices.
//!
//! The source stores pinyin inconsistently: main entries use tone numbers
//! (`wei4`) plus a diacritic form (`wèi`), while the appendices use diacritics
//! (`JIǍ GĒN`) plus a toneless ASCII form (`jia gen`). These functions reduce
//! either spelling to the same two keys:
//!
//! - a tone-numbered key such as `wei4` or `jia3 gen1`;
//! - a toneless key such as `wei` or `jia gen`.
//!
//! The tone-numbered key is only produced when the input actually carries tone
//! information, so an English headword never enters the pinyin index.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::dictionary::normalize::is_han;

/// Convert a pinyin string to a tone-numbered key.
///
/// Returns `None` when the string has no vowel or carries no tone information.
pub fn toned_key(text: &str) -> Option<String> {
    let mut out = String::new();
    let mut syllable = String::new();
    let mut tone: Option<char> = None;
    let mut saw_tone = false;
    let mut saw_digit = false;
    let mut saw_vowel = false;

    for character in text.nfd() {
        match character {
            '\u{0304}' => {
                tone = Some('1');
                saw_tone = true;
            }
            '\u{0301}' => {
                tone = Some('2');
                saw_tone = true;
            }
            '\u{030C}' => {
                tone = Some('3');
                saw_tone = true;
            }
            '\u{0300}' => {
                tone = Some('4');
                saw_tone = true;
            }
            // Diaeresis distinguishes ü from u in display, but the lookup key
            // folds both to u (the appendices also spell it as plain u).
            '\u{0308}' => {}
            character if is_combining_mark(character) => {}
            character if character.is_ascii_alphabetic() => {
                let lower = character.to_ascii_lowercase();
                if matches!(lower, 'a' | 'e' | 'i' | 'o' | 'u' | 'v') {
                    saw_vowel = true;
                }
                syllable.push(if lower == 'v' { 'u' } else { lower });
            }
            character if character.is_ascii_digit() => {
                saw_digit = true;
                syllable.push(character);
            }
            character if is_separator(character) => {
                flush(&mut out, &mut syllable, &mut tone);
            }
            _ => {
                flush(&mut out, &mut syllable, &mut tone);
            }
        }
    }
    flush(&mut out, &mut syllable, &mut tone);

    (saw_vowel && (saw_tone || saw_digit)).then_some(out)
}

/// Convert a pinyin string to a toneless ASCII key.
pub fn toneless_key(text: &str) -> String {
    let mut out = String::new();
    let mut pending_space = false;

    for character in text.nfd() {
        if is_combining_mark(character) {
            continue;
        }
        if character.is_ascii_alphabetic() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            let lower = character.to_ascii_lowercase();
            out.push(if lower == 'v' { 'u' } else { lower });
        } else if character.is_ascii_digit() {
            // Tone numbers are deliberately dropped.
        } else {
            pending_space = !out.is_empty();
        }
    }

    out
}

/// Keep only Han characters, dropping spaces, punctuation, and Latin text.
///
/// This makes `为 滞`, `为滞`, and `为滞 (wei4 zhi4)` the same key.
pub fn han_key(text: &str) -> String {
    text.chars()
        .filter(|character| is_han(*character))
        .collect()
}

/// Split a string into its runs of Han characters.
///
/// `下甲 xià jiǎ` yields `["下甲"]`; `为根 / 為根` yields `["为根", "為根"]`.
pub fn han_runs(text: &str) -> Vec<String> {
    let mut runs = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if is_han(character) {
            current.push(character);
        } else if !current.is_empty() {
            runs.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
}

fn is_separator(character: char) -> bool {
    character.is_whitespace()
        || matches!(character, '\'' | '’' | '-' | '/' | '\\' | ',' | ';' | ':')
}

fn flush(out: &mut String, syllable: &mut String, tone: &mut Option<char>) {
    if syllable.is_empty() {
        *tone = None;
        return;
    }

    let repaired = repair_u9(syllable);
    syllable.clear();

    // A standalone tone digit (`qi 4`) belongs to the previous syllable.
    if !out.is_empty() && repaired.chars().all(|character| character.is_ascii_digit()) {
        if !out
            .chars()
            .next_back()
            .is_some_and(|character| character.is_ascii_digit())
        {
            out.push_str(&repaired);
        }
        *tone = None;
        return;
    }

    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(&repaired);
    if let Some(digit) = tone.take()
        && !repaired
            .chars()
            .next_back()
            .is_some_and(|character| character.is_ascii_digit())
    {
        out.push(digit);
    }
}

/// The source encodes `ü` as `u9` in a few tone-numbered aliases (`nu94` for
/// `nǜ`). Drop the `9` when it directly precedes a tone digit.
fn repair_u9(text: &str) -> String {
    let characters: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < characters.len() {
        if characters[index] == 'u'
            && index + 2 < characters.len()
            && characters[index + 1] == '9'
            && characters[index + 2].is_ascii_digit()
        {
            out.push('u');
            index += 2;
        } else {
            out.push(characters[index]);
            index += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_marks_become_tone_numbers() {
        assert_eq!(toned_key("wèi").as_deref(), Some("wei4"));
        assert_eq!(toned_key("wéi").as_deref(), Some("wei2"));
        assert_eq!(toned_key("jiǎ gēn").as_deref(), Some("jia3 gen1"));
        assert_eq!(toned_key("JIǍ GĒN").as_deref(), Some("jia3 gen1"));
        assert_eq!(
            toned_key("yǐ bǐng dīng").as_deref(),
            Some("yi3 bing3 ding1")
        );
    }

    #[test]
    fn tone_numbers_are_preserved_and_repaired() {
        assert_eq!(toned_key("wei4").as_deref(), Some("wei4"));
        assert_eq!(toned_key("jia3 gen1").as_deref(), Some("jia3 gen1"));
        assert_eq!(toned_key("nu94").as_deref(), Some("nu4"));
        assert_eq!(toned_key("nǜ").as_deref(), Some("nu4"));
        assert_eq!(toned_key("lv4").as_deref(), Some("lu4"));
    }

    #[test]
    fn toneless_forms_are_needed_for_the_fallback_key() {
        assert_eq!(toned_key("jia gen"), None);
        assert_eq!(toned_key("wèi wēi"), Some("wei4 wei1".to_owned()));
        assert_eq!(toneless_key("wèi"), "wei");
        assert_eq!(toneless_key("jiǎ gēn"), "jia gen");
        assert_eq!(toneless_key("nu94"), "nu");
        assert_eq!(toneless_key("lǜ"), "lu");
        assert_eq!(toneless_key("xi'an"), "xi an");
        assert_eq!(toneless_key("wèi wēi"), "wei wei");
    }

    #[test]
    fn han_keys_drop_spacing_and_latin_text() {
        assert_eq!(han_key("为 滞"), "为滞");
        assert_eq!(han_key("为滞 (wei4 zhi4)"), "为滞");
        assert_eq!(han_key("為滯"), "為滯");
        assert_eq!(han_runs("下甲 xià jiǎ"), vec!["下甲"]);
        assert_eq!(han_runs("为根 / 為根"), vec!["为根", "為根"]);
    }

    #[test]
    fn non_pinyin_text_is_not_given_a_toned_key() {
        assert_eq!(toned_key("ZZ-1"), None);
        assert_eq!(toned_key("alpha swelling"), None);
        assert!(toned_key("a1ALP").is_some());
    }
}
