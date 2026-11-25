//! Near-miss pinyin suggestions.
//!
//! Exact lookup stays exact: these suggestions run only after a lookup has
//! failed or after a ranked search has produced weak hits. The distance is the
//! optimal string alignment (Damerau–`Levenshtein` with adjacent transpositions),
//! computed over toneless pinyin keys such as `jia gen`.

use crate::dictionary::Dictionary;
use crate::dictionary::model::{LookupRef, Scope, Source, Suggestion};
use crate::dictionary::normalize::{contains_han, normalize};
use crate::dictionary::pinyin;

/// Up to `limit` suggestions of the form `jia gen (jia root)`.
pub(crate) fn pinyin_suggestions(
    dictionary: &Dictionary,
    query: &str,
    limit: usize,
) -> Vec<Suggestion> {
    if limit == 0 || contains_han(query) {
        return Vec::new();
    }
    let normalized = pinyin::toneless_key(query);
    let length = normalized.chars().count();
    if length < 4
        || !normalized
            .chars()
            .any(|character| "aeiouü".contains(character))
    {
        return Vec::new();
    }
    let allowed = if length <= 6 { 1 } else { 2 };
    let query_has_space = normalized.chars().any(char::is_whitespace);

    let mut candidates: Vec<(usize, usize, usize, String, String)> = Vec::new();
    for (key, references) in &dictionary.pinyin_toneless {
        if key == &normalized {
            continue;
        }
        let key_length = key.chars().count();
        let mut matched = None;
        if key_length.abs_diff(length) <= allowed {
            let distance = distance(&normalized, key);
            if distance > 0 && distance <= allowed {
                matched = Some((distance, false));
            }
        }
        // A typo that also drops the word space (`mahuag`, `mhuang`) only
        // reaches the key once its spaces are removed. Such a match is
        // preferred at equal distance: a space-free query is more likely the
        // squashed form of a two-word key than a typo of a shorter fragment.
        if matched.is_none() && !query_has_space && key.chars().any(char::is_whitespace) {
            let compact: String = key
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect();
            if compact.chars().count().abs_diff(length) <= allowed {
                let distance = distance(&normalized, &compact);
                if distance <= allowed {
                    matched = Some((distance, true));
                }
            }
        }
        let Some((distance, compact_match)) = matched else {
            continue;
        };
        let Some(target) = references.first() else {
            continue;
        };
        let Some(term) = term_for(dictionary, *target) else {
            continue;
        };
        let preference = usize::from(!compact_match);
        candidates.push((distance, preference, key_length, key.clone(), term));
    }

    candidates.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(left.1.cmp(&right.1))
            .then(left.2.cmp(&right.2))
            .then(left.3.cmp(&right.3))
    });
    candidates.dedup_by(|left, right| left.3 == right.3);
    candidates
        .into_iter()
        .take(limit)
        .map(|(_, _, _, key, term)| Suggestion {
            text: format!("{key} ({term})"),
            term,
            page: None,
        })
        .collect()
}

/// Up to `limit` near-miss English terms from headwords, appendix entries, and
/// weights titles. A single-word query may match one word of a multi-word term,
/// so `alpha sweling` reaches `alpha swelling`.
pub(crate) fn term_suggestions(
    dictionary: &Dictionary,
    query: &str,
    scope: Scope,
    limit: usize,
) -> Vec<Suggestion> {
    if limit == 0 || contains_han(query) || query.chars().count() < 4 {
        return Vec::new();
    }

    let mut candidates: Vec<(usize, usize, String)> = Vec::new();
    if scope.allows(Source::Dictionary) {
        for entry in &dictionary.entries {
            if let Some(score) = close(query, &entry.norm_headword) {
                candidates.push((score, entry.norm_headword.len(), entry.headword.clone()));
            }
        }
    }
    for reference in dictionary.appendix_entries(scope) {
        if let Some(entry) = dictionary.appendix_entry(reference) {
            let normalized = normalize(&entry.term);
            if let Some(score) = close(query, &normalized) {
                candidates.push((score, normalized.len(), entry.term.clone()));
            }
        }
    }
    if scope.allows(Source::Weights) {
        for section in &dictionary.weights {
            let normalized = normalize(&section.title);
            if let Some(score) = close(query, &normalized) {
                candidates.push((score, normalized.len(), section.title.clone()));
            }
        }
    }

    candidates.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(left.1.cmp(&right.1))
            .then(left.2.cmp(&right.2))
    });
    candidates.dedup_by(|left, right| left.2.eq_ignore_ascii_case(&right.2));
    candidates
        .into_iter()
        .take(limit)
        .map(|(_, _, term)| Suggestion {
            text: term.clone(),
            term,
            page: None,
        })
        .collect()
}

fn close(query: &str, term: &str) -> Option<usize> {
    let allowed = if query.chars().count() <= 6 { 1 } else { 2 };
    let length_ok = term.chars().count().abs_diff(query.chars().count()) <= allowed;
    if length_ok {
        let whole = distance(query, term);
        if whole <= allowed {
            return Some(whole);
        }
    }
    if query.contains(' ') {
        return None;
    }
    term.split_whitespace()
        .filter(|word| word.chars().count() >= 3)
        .filter(|word| word.chars().count().abs_diff(query.chars().count()) <= allowed)
        .map(|word| distance(query, word))
        .filter(|distance| *distance <= allowed)
        .min()
}

fn term_for(dictionary: &Dictionary, target: LookupRef) -> Option<String> {
    match target.source {
        Source::Dictionary => dictionary
            .entries
            .get(target.index)
            .map(|entry| entry.headword.clone()),
        Source::Meridian | Source::Medicinal | Source::Formulas => dictionary
            .appendix_entry(crate::dictionary::model::AppendixRef {
                source: target.source,
                index: target.index,
            })
            .map(|entry| entry.term.clone()),
        Source::Weights => None,
    }
}

/// Optimal string alignment distance, counting an adjacent transposition as
/// one edit. Keys are short, so the quadratic table is cheap.
pub(crate) fn distance(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    if left.is_empty() {
        return right.len();
    }
    if right.is_empty() {
        return left.len();
    }
    if left.len().abs_diff(right.len()) > 2 {
        return left.len().abs_diff(right.len());
    }

    let mut table = vec![vec![0usize; right.len() + 1]; left.len() + 1];
    for (index, row) in table.iter_mut().enumerate() {
        row[0] = index;
    }
    for (index, cell) in table[0].iter_mut().enumerate() {
        *cell = index;
    }

    for i in 1..=left.len() {
        for j in 1..=right.len() {
            let substitution = usize::from(left[i - 1] != right[j - 1]);
            let mut value = (table[i - 1][j] + 1)
                .min(table[i][j - 1] + 1)
                .min(table[i - 1][j - 1] + substitution);
            if i > 1 && j > 1 && left[i - 1] == right[j - 2] && left[i - 2] == right[j - 1] {
                value = value.min(table[i - 2][j - 2] + 1);
            }
            table[i][j] = value;
        }
    }
    table[left.len()][right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::model::Entry;
    use std::collections::HashMap;

    fn entry(headword: &str, line: usize) -> Entry {
        Entry {
            headword: headword.to_owned(),
            norm_headword: headword.to_lowercase(),
            file: "08-a.md".to_owned(),
            line,
            anchor: None,
            aliases: Vec::new(),
            body: String::new(),
            norm_body: String::new(),
            links: Vec::new(),
        }
    }

    fn dictionary() -> Dictionary {
        let mut dictionary = Dictionary::empty();
        dictionary.entries = vec![entry("jia root", 9), entry("alpha swelling", 27)];
        dictionary.pinyin_toneless = HashMap::from([(
            "jia gen".to_owned(),
            vec![LookupRef {
                source: Source::Dictionary,
                index: 0,
            }],
        )]);
        dictionary
    }

    #[test]
    fn distance_counts_substitutions_insertions_and_transpositions() {
        assert_eq!(distance("jia ge", "jia gen"), 1);
        assert_eq!(distance("jia zhon", "jia zhong"), 1);
        assert_eq!(distance("bing ca", "bing cao"), 1);
        assert_eq!(distance("jiagen", "jia gen"), 1);
        assert_eq!(distance("ang", "gnah"), 3);
        assert_eq!(distance("same", "same"), 0);
    }

    fn texts(values: Vec<Suggestion>) -> Vec<String> {
        values.into_iter().map(|value| value.text).collect()
    }

    #[test]
    fn a_misspelled_syllable_finds_the_key_and_its_term() {
        let dictionary = dictionary();
        let suggestions = pinyin_suggestions(&dictionary, "jia ge", 3);
        assert_eq!(
            texts(suggestions.clone()),
            vec!["jia gen (jia root)".to_owned()]
        );
        assert_eq!(suggestions[0].term, "jia root");
        assert_eq!(suggestions[0].page, None);
    }

    #[test]
    fn a_space_free_typo_reaches_a_two_word_key() {
        let dictionary = dictionary();
        for query in ["jiage", "jigen", "jiagen"] {
            assert_eq!(
                texts(pinyin_suggestions(&dictionary, query, 3)),
                vec!["jia gen (jia root)".to_owned()],
                "query {query}"
            );
        }
    }

    #[test]
    fn a_compact_match_outranks_a_shorter_fragment() {
        let mut dictionary = dictionary();
        let index = dictionary.entries.len();
        dictionary.entries.push(entry("igen", 40));
        dictionary.pinyin_toneless.insert(
            "igen".to_owned(),
            vec![LookupRef {
                source: Source::Dictionary,
                index,
            }],
        );
        let suggestions = pinyin_suggestions(&dictionary, "jigen", 3);
        assert_eq!(
            texts(suggestions),
            vec!["jia gen (jia root)".to_owned(), "igen (igen)".to_owned()]
        );
    }

    #[test]
    fn an_english_misspelling_finds_the_headword() {
        let dictionary = dictionary();
        let suggestions = term_suggestions(&dictionary, "jia roott", Scope::All, 3);
        assert_eq!(texts(suggestions.clone()), vec!["jia root".to_owned()]);
        assert_eq!(suggestions[0].term, "jia root");
        assert_eq!(
            texts(term_suggestions(
                &dictionary,
                "alpha sweling",
                Scope::All,
                3
            )),
            vec!["alpha swelling".to_owned()]
        );
        assert!(term_suggestions(&dictionary, "zzzzz", Scope::All, 3).is_empty());
    }

    #[test]
    fn short_or_non_pinyin_queries_produce_nothing() {
        let dictionary = dictionary();
        assert!(pinyin_suggestions(&dictionary, "jia", 3).is_empty());
        assert!(pinyin_suggestions(&dictionary, "甲", 3).is_empty());
        assert!(pinyin_suggestions(&dictionary, "a1ALP", 3).is_empty());
        assert!(pinyin_suggestions(&dictionary, "zzzzz", 3).is_empty());
    }
}
