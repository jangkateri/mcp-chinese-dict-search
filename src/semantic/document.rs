//! Deterministic document construction for the semantic index.
//!
//! One document is produced per searchable entry, in the order main entries,
//! meridian appendix entries, medicinal appendix entries, formula appendix
//! entries, and weights sections. The order is part of the index contract.

use crate::dictionary::parse::plain_text;
use crate::dictionary::{Dictionary, LookupRef, Source};

/// Embedding-affecting formatting applied to every document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentProfile {
    /// Prefix prepended to each document, for models that use one.
    pub document_prefix: String,
}

/// One document and the reference it will be stored under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticDocument {
    pub reference: LookupRef,
    pub text: String,
}

pub fn build_documents(
    dictionary: &Dictionary,
    profile: &DocumentProfile,
) -> Vec<SemanticDocument> {
    let mut documents = Vec::new();

    for (index, entry) in dictionary.entries.iter().enumerate() {
        let mut parts = vec![entry.headword.clone()];
        for alias in &entry.aliases {
            push_unique(&mut parts, &alias.simplified);
            push_unique(&mut parts, &alias.traditional);
            push_unique(&mut parts, &alias.pinyin_toned);
            push_unique(&mut parts, &alias.pinyin_ascii);
        }
        parts.push(plain_text(&entry.body));
        documents.push(SemanticDocument {
            reference: LookupRef {
                source: Source::Dictionary,
                index,
            },
            text: with_prefix(&profile.document_prefix, &parts),
        });
    }

    for (source, entries) in [
        (Source::Meridian, &dictionary.meridians),
        (Source::Medicinal, &dictionary.medicinals),
        (Source::Formulas, &dictionary.formulas),
    ] {
        for (index, entry) in entries.iter().enumerate() {
            let mut parts = vec![entry.term.clone()];
            if let Some(id) = &entry.id {
                push_unique(&mut parts, id);
            }
            push_unique(&mut parts, &entry.simplified);
            push_unique(&mut parts, &entry.traditional);
            push_unique(&mut parts, &entry.pinyin_toned);
            push_unique(&mut parts, &entry.pinyin_ascii);
            for alias in &entry.aliases {
                push_unique(&mut parts, alias);
            }
            parts.push(plain_text(&entry.body));
            documents.push(SemanticDocument {
                reference: LookupRef { source, index },
                text: with_prefix(&profile.document_prefix, &parts),
            });
        }
    }

    for (index, section) in dictionary.weights.iter().enumerate() {
        let parts = vec![
            section.number.clone(),
            section.title.clone(),
            plain_text(&section.body),
        ];
        documents.push(SemanticDocument {
            reference: LookupRef {
                source: Source::Weights,
                index,
            },
            text: with_prefix(&profile.document_prefix, &parts),
        });
    }

    documents
}

fn push_unique(parts: &mut Vec<String>, value: &str) {
    let value = value.trim();
    if !value.is_empty() && !parts.iter().any(|existing| existing == value) {
        parts.push(value.to_owned());
    }
}

fn with_prefix(prefix: &str, parts: &[String]) -> String {
    let body = parts
        .iter()
        .filter(|part| !part.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    if prefix.is_empty() {
        body
    } else {
        format!("{prefix}{body}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::{Alias, AppendixEntry, Entry, Section};

    fn entry(headword: &str, body: &str) -> Entry {
        Entry {
            headword: headword.to_owned(),
            norm_headword: headword.to_ascii_lowercase(),
            file: "08-a.md".to_owned(),
            line: 1,
            anchor: None,
            aliases: vec![Alias {
                simplified: "甲".to_owned(),
                traditional: "甲".to_owned(),
                pinyin_toned: "jia3".to_owned(),
                pinyin_ascii: "jiǎ".to_owned(),
            }],
            body: body.to_owned(),
            norm_body: body.to_ascii_lowercase(),
            links: Vec::new(),
        }
    }

    fn appendix(source: Source, term: &str, body: &str) -> AppendixEntry {
        AppendixEntry {
            source,
            file: "34-appendix-1-Meridian.md".to_owned(),
            line: 1,
            id: Some("AA-1".to_owned()),
            section: "Test".to_owned(),
            subsection: None,
            term: term.to_owned(),
            simplified: "甲".to_owned(),
            traditional: "甲".to_owned(),
            pinyin_toned: "jia3".to_owned(),
            pinyin_ascii: "jiǎ".to_owned(),
            aliases: vec!["alpha alias".to_owned()],
            body: body.to_owned(),
            norm_terms: Vec::new(),
            norm_body: body.to_ascii_lowercase(),
        }
    }

    #[test]
    fn documents_follow_the_documented_order_and_apply_the_prefix() {
        let mut dictionary = Dictionary::empty();
        dictionary.entries.push(entry("alpha", "A made-up entry."));
        dictionary.meridians.push(appendix(
            Source::Meridian,
            "alpha point",
            "A made-up point.",
        ));
        dictionary.medicinals.push(appendix(
            Source::Medicinal,
            "alpha root",
            "A made-up medicinal.",
        ));
        dictionary.formulas.push(appendix(
            Source::Formulas,
            "alpha decoction",
            "A made-up formula.",
        ));
        dictionary.weights.push(Section {
            file: "37-appendix-4-Weights.md".to_owned(),
            number: "1".to_owned(),
            title: "Length".to_owned(),
            body: "A made-up measure.".to_owned(),
        });

        let documents = build_documents(
            &dictionary,
            &DocumentProfile {
                document_prefix: "document: ".to_owned(),
            },
        );

        assert_eq!(documents.len(), 5);
        assert_eq!(
            documents
                .iter()
                .map(|document| document.reference.source)
                .collect::<Vec<_>>(),
            vec![
                Source::Dictionary,
                Source::Meridian,
                Source::Medicinal,
                Source::Formulas,
                Source::Weights,
            ]
        );
        assert_eq!(documents[0].reference.index, 0);
        assert!(documents[0].text.starts_with("document: alpha"));
        assert!(documents[0].text.contains("A made-up entry."));
        assert!(documents[4].text.contains("Length"));
    }

    #[test]
    fn duplicate_alias_values_are_emitted_once() {
        let mut dictionary = Dictionary::empty();
        dictionary.entries.push(entry("alpha", "A made-up entry."));
        let documents = build_documents(&dictionary, &DocumentProfile::default());
        assert_eq!(documents[0].text.matches("甲").count(), 1);
        assert_eq!(documents[0].text.matches("jia3").count(), 1);
    }
}
