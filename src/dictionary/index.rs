//! Parse `38-index.md` into an anchor → term map and resolve entry links.
//!
//! The Markdown conversion stripped every `id="a1XXXX"` definition. The index
//! is the authoritative mapping, but it omits a few entry anchors. Those gaps
//! were checked against the exact source EPUB and are filled by
//! [`apply_verified_corrections`].

use std::collections::{HashMap, HashSet};

use crate::dictionary::model::{AnchorTarget, Entry};
use crate::dictionary::normalize::{collapse_whitespace, normalize, unescape_markdown};
use crate::dictionary::parse::strip_tags;

/// The anchor map and the term → anchors map derived from the index.
///
/// A term can have several anchors. The index gives numbered sub-anchors for
/// duplicate headwords (for example, the two `wei2` entries), and the anchors
/// occur in the same order as the entries.
#[derive(Debug, Default)]
pub struct IndexData {
    pub anchors: HashMap<String, AnchorTarget>,
    pub term_anchors: HashMap<(String, String), Vec<String>>,
}

/// Anchors that the source EPUB defines on an entry but `38-index.md` omits.
///
/// Verified on 2026-11-24 against the exact unencrypted EPUB: each anchor is
/// the `id` of the `div` that contains the named entry. The four entries keep
/// their proper citation and their inbound links resolve.
const MISSING_ENTRY_ANCHORS: &[(&str, &str, &str)] = &[
    ("23-p.md", "provisioning", "a1UN9"),
    ("23-p.md", "provisioning qi4", "a1PDE"),
    ("23-p.md", "provisioning-aspect pattern", "a1PB6"),
    ("23-p.md", "provisioning-defense disharmony", "a1R3X"),
];

/// Anchors that the EPUB defines on a paragraph inside an entry rather than on
/// an entry container.
///
/// `a1SSD` is the first sense paragraph of `wind stroke`; the EPUB uses it as
/// the target of every "wind-like stroke" reference. The paragraph is part of
/// the `wind stroke` entry, so links resolve to that entry.
const SUBENTRY_TARGETS: &[(&str, &str, &str)] = &[("a1SSD", "30-w.md", "wind stroke")];

/// Counts returned by [`apply_verified_corrections`].
#[derive(Debug, Default, Clone, Copy)]
pub struct CorrectionCounts {
    pub entry_anchors: usize,
    pub link_targets: usize,
}

/// Apply the EPUB-verified corrections for anchors the Markdown index omits.
///
/// Corrections apply only when the named file and headword are present, so
/// synthetic fixtures are unaffected.
pub fn apply_verified_corrections(index: &mut IndexData, entries: &[Entry]) -> CorrectionCounts {
    let mut counts = CorrectionCounts::default();

    for &(file, headword, anchor) in MISSING_ENTRY_ANCHORS {
        let norm = normalize(headword);
        let Some(entry) = entries
            .iter()
            .find(|entry| entry.file == file && entry.norm_headword == norm)
        else {
            continue;
        };
        let mut added = !index.anchors.contains_key(anchor);
        index
            .anchors
            .entry(anchor.to_owned())
            .or_insert_with(|| AnchorTarget {
                file: file.to_owned(),
                term: entry.headword.clone(),
                norm_term: norm.clone(),
            });
        let anchors = index
            .term_anchors
            .entry((file.to_owned(), norm))
            .or_default();
        if !anchors.iter().any(|existing| existing == anchor) {
            anchors.push(anchor.to_owned());
            added = true;
        }
        if added {
            counts.entry_anchors += 1;
        }
    }

    for &(anchor, file, term) in SUBENTRY_TARGETS {
        let norm = normalize(term);
        let Some(entry) = entries
            .iter()
            .find(|entry| entry.file == file && entry.norm_headword == norm)
        else {
            continue;
        };
        let added = !index.anchors.contains_key(anchor);
        index
            .anchors
            .entry(anchor.to_owned())
            .or_insert_with(|| AnchorTarget {
                file: file.to_owned(),
                term: entry.headword.clone(),
                norm_term: norm.clone(),
            });
        if added {
            counts.link_targets += 1;
        }
    }

    counts
}

/// Parse the index, ignoring links that do not target a main letter file.
pub fn parse_index(content: &str, main_files: &HashSet<String>) -> IndexData {
    let mut data = IndexData::default();

    for line in content.lines() {
        let links = links_in_line(line);
        let eligible: Vec<&IndexLink> = links
            .iter()
            .filter(|link| {
                link.target_file
                    .as_ref()
                    .is_some_and(|file| main_files.contains(file))
                    && link.anchor.is_some()
            })
            .collect();
        if eligible.is_empty() {
            continue;
        }

        let prefix = clean(&line[..eligible[0].start]);
        let all_numbered = eligible.iter().all(|link| {
            link.text
                .chars()
                .all(|character| character.is_ascii_digit())
        });

        for link in eligible {
            let term = if all_numbered && !prefix.is_empty() {
                prefix.clone()
            } else if !link.text.is_empty() {
                link.text.clone()
            } else if !prefix.is_empty() {
                prefix.clone()
            } else {
                continue;
            };
            let Some(file) = &link.target_file else {
                continue;
            };
            let Some(anchor) = &link.anchor else {
                continue;
            };
            let norm_term = normalize(&term);
            data.anchors
                .entry(anchor.clone())
                .or_insert_with(|| AnchorTarget {
                    file: file.clone(),
                    term: term.clone(),
                    norm_term: norm_term.clone(),
                });
            let term_anchors = data
                .term_anchors
                .entry((file.clone(), norm_term))
                .or_default();
            if !term_anchors.iter().any(|existing| existing == anchor) {
                term_anchors.push(anchor.clone());
            }
        }
    }

    data
}

/// Attach anchors to entries and resolve their internal links.
pub fn resolve_entries(entries: &mut [Entry], index: &IndexData) -> (usize, usize, usize) {
    let mut resolved = 0;
    let mut unresolved = 0;
    let mut anchorless = 0;
    let mut cursors: HashMap<(String, String), usize> = HashMap::new();

    for entry in entries.iter_mut() {
        let key = (entry.file.clone(), entry.norm_headword.clone());
        let position = {
            let cursor = cursors.entry(key.clone()).or_insert(0);
            let position = *cursor;
            *cursor += 1;
            position
        };
        entry.anchor = index
            .term_anchors
            .get(&key)
            .and_then(|anchors| anchors.get(position))
            .cloned();
        if entry.anchor.is_none() {
            anchorless += 1;
        }

        for link in &mut entry.links {
            let Some(anchor) = &link.target_anchor else {
                continue;
            };
            match index.anchors.get(anchor) {
                Some(target) => {
                    link.target_term = Some(target.term.clone());
                    link.resolved = true;
                    resolved += 1;
                }
                None => {
                    unresolved += 1;
                }
            }
        }
    }

    (resolved, unresolved, anchorless)
}

struct IndexLink {
    start: usize,
    target_file: Option<String>,
    anchor: Option<String>,
    text: String,
}

fn links_in_line(line: &str) -> Vec<IndexLink> {
    let mut links = Vec::new();
    let mut rest = line;

    while let Some(start) = rest.find("<a ") {
        let after = &rest[start..];
        let Some(gt) = after.find('>') else {
            break;
        };
        let attributes = &after[..gt];
        let Some(href) = attribute(attributes, "href") else {
            rest = &after[gt + 1..];
            continue;
        };
        let text_start = gt + 1;
        let Some(end) = after[text_start..].find("</a>") else {
            break;
        };
        let text = clean(&strip_tags(&after[text_start..text_start + end]));
        let (target_file, anchor) = match href.split_once('#') {
            Some((file, anchor)) => (
                (!file.is_empty()).then(|| file.to_owned()),
                Some(anchor.to_owned()),
            ),
            None => (Some(href.to_owned()), None),
        };
        links.push(IndexLink {
            start,
            target_file,
            anchor,
            text,
        });
        rest = &after[text_start + end + 4..];
    }

    links
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut search = tag;
    let needle_double = format!("{name}=\"");
    let needle_single = format!("{name}='");
    while let Some(index) = search.find(' ') {
        search = &search[index + 1..];
        if let Some(value) = search.strip_prefix(&needle_double) {
            return value.find('"').map(|end| &value[..end]);
        }
        if let Some(value) = search.strip_prefix(&needle_single) {
            return value.find('\'').map(|end| &value[..end]);
        }
    }
    None
}

fn clean(text: &str) -> String {
    collapse_whitespace(&unescape_markdown(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::parse::parse_entries;
    use std::collections::HashSet;

    fn main_files() -> HashSet<String> {
        [
            "08-a.md".to_owned(),
            "10-c.md".to_owned(),
            "14-g.md".to_owned(),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn maps_plain_and_numbered_index_entries() {
        let index = "\
<a href=\"08-a.md#a1ASW\" class=\"class_s1NTM-0\">alpha swelling</a>

clear heat <a href=\"10-c.md#a1CH1\">1</a> <a href=\"10-c.md#a1CH2\">2</a>

<a href=\"03-table-of-contents.md#nav\">Navigation</a>
";
        let data = parse_index(index, &main_files());
        assert_eq!(data.anchors.len(), 3);
        assert_eq!(data.anchors["a1ASW"].term, "alpha swelling");
        assert_eq!(data.anchors["a1CH1"].term, "clear heat");
        assert_eq!(data.anchors["a1CH2"].term, "clear heat");
        assert!(!data.anchors.contains_key("nav"));
        assert_eq!(
            data.term_anchors
                .get(&("08-a.md".to_owned(), "alpha swelling".to_owned()))
                .map(Vec::as_slice),
            Some(["a1ASW".to_owned()].as_slice())
        );
        assert_eq!(
            data.term_anchors
                .get(&("10-c.md".to_owned(), "clear heat".to_owned()))
                .map(Vec::as_slice),
            Some(["a1CH1".to_owned(), "a1CH2".to_owned()].as_slice())
        );
    }

    #[test]
    fn numbered_subanchors_follow_entry_order() {
        let mut entries = parse_entries(
            "10-c.md",
            "\\*same\r\n\r\nFirst sense.\r\n\r\n\\*same\r\n\r\nSecond sense.\r\n",
        )
        .0;
        let index = parse_index(
            "same <a href=\"10-c.md#a1S01\">1</a> <a href=\"10-c.md#a1S02\">2</a>\n",
            &main_files(),
        );
        let (_, _, anchorless) = resolve_entries(&mut entries, &index);
        assert_eq!(anchorless, 0);
        assert_eq!(entries[0].anchor.as_deref(), Some("a1S01"));
        assert_eq!(entries[1].anchor.as_deref(), Some("a1S02"));
    }

    #[test]
    fn verified_corrections_resolve_missing_entry_and_subentry_anchors() {
        let mut entries = parse_entries(
            "23-p.md",
            "\\*provisioning\r\n\r\nSee <a href=\"23-p.md#a1UN9\">provisioning</a>.\r\n",
        )
        .0;
        entries.extend(
            parse_entries(
                "30-w.md",
                "\\*wind stroke\r\n\r\nSee <a href=\"30-w.md#a1SSD\">wind-like stroke</a>.\r\n",
            )
            .0,
        );
        let files: HashSet<String> = ["23-p.md".to_owned(), "30-w.md".to_owned()]
            .into_iter()
            .collect();
        let mut index = parse_index("<a href=\"30-w.md#a1R4B\">wind stroke</a>\n", &files);
        let counts = apply_verified_corrections(&mut index, &entries);
        assert_eq!(counts.entry_anchors, 1);
        assert_eq!(counts.link_targets, 1);

        let (resolved, unresolved, anchorless) = resolve_entries(&mut entries, &index);
        assert_eq!(resolved, 2);
        assert_eq!(unresolved, 0);
        assert_eq!(anchorless, 0);
        assert_eq!(entries[0].anchor.as_deref(), Some("a1UN9"));
        assert_eq!(entries[1].anchor.as_deref(), Some("a1R4B"));
        assert_eq!(
            entries[0].links[0].target_term.as_deref(),
            Some("provisioning")
        );
        assert_eq!(
            entries[1].links[0].target_term.as_deref(),
            Some("wind stroke")
        );
    }

    #[test]
    fn resolves_entry_links_and_counts_unresolved() {
        let mut entries = parse_entries(
            "08-a.md",
            "\\*abdomen\r\n\r\n腹 | 腹  \\*fu4 / fù\r\n\r\nSee <a href=\"14-g.md#a1S36\">greater abdomen</a> and <a href=\"10-c.md#missing\">nope</a>.\r\n",
        )
        .0;
        let index = parse_index(
            "<a href=\"14-g.md#a1S36\">greater abdomen</a>\n",
            &main_files(),
        );
        let (resolved, unresolved, anchorless) = resolve_entries(&mut entries, &index);
        assert_eq!(resolved, 1);
        assert_eq!(unresolved, 1);
        assert_eq!(anchorless, 1);
        assert!(entries[0].links[0].resolved);
        assert_eq!(
            entries[0].links[0].target_term.as_deref(),
            Some("greater abdomen")
        );
        assert!(!entries[0].links[1].resolved);
    }
}
