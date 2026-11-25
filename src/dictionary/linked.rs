//! One-time build of a linked copy of the Markdown dictionary.
//!
//! The EPUB-to-Markdown conversion dropped the `id` attributes that the book's
//! own `file.md#a1XXXX` links use as targets; the anchors survive only in
//! `38-index.md`. This build writes a copy of every Markdown document with one
//! `<a id="…"></a>` line before each entry, so those links resolve, and then
//! re-reads the copy to prove that the entries and the link targets are intact.
//! The source archive or directory is never modified; the server is pointed at
//! the copy instead.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::dictionary::parse::is_anchor_marker;
use crate::dictionary::{Dictionary, TITLE_FILE, source};
use crate::error::DictionaryError;

/// What the linked-copy build did and verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkReport {
    pub files: usize,
    pub entries: usize,
    pub anchors_inserted: usize,
    pub anchorless_entries: usize,
    pub content_mismatches: usize,
    pub fragment_links: usize,
    pub resolved_fragments: usize,
    pub unresolved_fragments: usize,
    pub unresolved_in_letters: usize,
    pub unresolved_examples: Vec<String>,
}

/// Write a linked copy of the dictionary at `source_path` into `destination`,
/// then verify it. Both a `.zip` archive and an extracted directory are
/// accepted as the source.
pub fn build_linked_data(
    source_path: &Path,
    destination: &Path,
) -> Result<LinkReport, DictionaryError> {
    let source_absolute = std::path::absolute(source_path).map_err(unreadable)?;
    let destination_absolute = std::path::absolute(destination).map_err(unreadable)?;
    if destination_absolute.starts_with(&source_absolute) {
        return Err(DictionaryError::unreadable(format!(
            "The linked copy must be outside the source: {} is inside {}.",
            destination_absolute.display(),
            source_absolute.display()
        )));
    }

    let dictionary = Dictionary::load(source_path)?;
    let documents = source::read_all_documents(source_path)?;
    fs::create_dir_all(&destination_absolute).map_err(unreadable)?;
    fs::write(
        destination_absolute.join(TITLE_FILE),
        format!("{}\n", dictionary.title()),
    )
    .map_err(unreadable)?;

    let mut anchors_by_file: HashMap<String, Vec<(usize, String)>> = HashMap::new();
    let mut anchorless_entries = 0;
    for entry in &dictionary.entries {
        match &entry.anchor {
            Some(anchor) => anchors_by_file
                .entry(entry.file.clone())
                .or_default()
                .push((entry.line, anchor.clone())),
            None => anchorless_entries += 1,
        }
    }
    for positions in anchors_by_file.values_mut() {
        positions.sort_by_key(|(line, _)| *line);
    }

    let mut report = LinkReport {
        files: documents.len(),
        entries: dictionary.entries.len(),
        anchors_inserted: 0,
        anchorless_entries,
        content_mismatches: 0,
        fragment_links: 0,
        resolved_fragments: 0,
        unresolved_fragments: 0,
        unresolved_in_letters: 0,
        unresolved_examples: Vec::new(),
    };

    let mut copied: HashMap<String, String> = HashMap::new();
    for (name, content) in documents {
        let Some(file_name) = Path::new(&name)
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
        else {
            return Err(DictionaryError::unreadable(format!(
                "The archive contains an unnamed document ({name})."
            )));
        };
        if copied.contains_key(&file_name) {
            return Err(DictionaryError::unreadable(format!(
                "The archive contains two documents named {file_name}."
            )));
        }

        let text = insert_anchors(&content, anchors_by_file.get(&file_name), &mut report)?;
        let text = unescape_brackets(&text);
        let path = destination_absolute.join(&file_name);
        fs::write(&path, text.as_bytes()).map_err(unreadable)?;
        copied.insert(file_name, text);
    }

    verify_entries(&dictionary, &destination_absolute, &mut report)?;
    verify_links(&copied, &mut report);

    if report.content_mismatches > 0 {
        return Err(DictionaryError::unreadable(format!(
            "The linked copy changed {0} entries; it was written to {1} for inspection.",
            report.content_mismatches,
            destination_absolute.display()
        )));
    }
    Ok(report)
}

/// The conversion escaped literal brackets as `\[` and `\]`. In a Markdown
/// preview with math enabled, `\[` opens display math, so the 833 monograph
/// headings render as LaTeX and the line's text disappears. The copy drops the
/// backslashes; the parser accepts both forms and the server output is
/// unchanged because it unescaped them already.
fn unescape_brackets(text: &str) -> String {
    text.replace("\\[", "[").replace("\\]", "]")
}

/// Insert one anchor line before each entry in `content`.
fn insert_anchors(
    content: &str,
    anchors: Option<&Vec<(usize, String)>>,
    report: &mut LinkReport,
) -> Result<String, DictionaryError> {
    let Some(anchors) = anchors else {
        return Ok(content.to_owned());
    };
    let lines: Vec<&str> = content.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + anchors.len());
    let mut next = 0;

    for (index, line) in lines.iter().enumerate() {
        while next < anchors.len() && anchors[next].0.saturating_sub(1) == index {
            let anchor = &anchors[next].1;
            let already_present = index > 0
                && is_anchor_marker(lines[index - 1])
                && lines[index - 1].contains(&format!("id=\"{anchor}\""));
            if line.starts_with("\\*") && !already_present {
                out.push(format!("<a id=\"{anchor}\"></a>"));
                report.anchors_inserted += 1;
            }
            next += 1;
        }
        out.push((*line).to_owned());
    }

    if next < anchors.len() {
        return Err(DictionaryError::unreadable(format!(
            "{} anchors point past the end of the file; the parsed lines and the copy disagree.",
            anchors.len() - next
        )));
    }
    Ok(out.join("\n") + "\n")
}

fn verify_entries(
    original: &Dictionary,
    destination: &Path,
    report: &mut LinkReport,
) -> Result<(), DictionaryError> {
    let copy = Dictionary::load(destination)?;
    if copy.entries.len() != original.entries.len() {
        report.content_mismatches += original.entries.len().abs_diff(copy.entries.len());
        return Ok(());
    }
    for (left, right) in original.entries.iter().zip(&copy.entries) {
        let same = unescape_brackets(&left.headword) == unescape_brackets(&right.headword)
            && left.anchor == right.anchor
            && unescape_brackets(&left.body) == unescape_brackets(&right.body)
            && left.aliases.len() == right.aliases.len()
            && left.aliases.iter().zip(&right.aliases).all(|(a, b)| {
                unescape_brackets(&a.simplified) == unescape_brackets(&b.simplified)
                    && unescape_brackets(&a.traditional) == unescape_brackets(&b.traditional)
                    && unescape_brackets(&a.pinyin_toned) == unescape_brackets(&b.pinyin_toned)
                    && unescape_brackets(&a.pinyin_ascii) == unescape_brackets(&b.pinyin_ascii)
            });
        if !same {
            report.content_mismatches += 1;
        }
    }
    if copy.meridians.len() != original.meridians.len()
        || copy.medicinals.len() != original.medicinals.len()
        || copy.formulas.len() != original.formulas.len()
        || copy.weights.len() != original.weights.len()
    {
        report.content_mismatches += 1;
    }
    Ok(())
}

fn verify_links(documents: &HashMap<String, String>, report: &mut LinkReport) {
    let ids: HashMap<&str, HashSet<String>> = documents
        .iter()
        .map(|(name, content)| (name.as_str(), ids_in(content)))
        .collect();

    let mut unresolved: Vec<(bool, String)> = Vec::new();
    for (name, content) in documents {
        let mut rest = content.as_str();
        while let Some(start) = rest.find("href=\"") {
            rest = &rest[start + 6..];
            let Some(end) = rest.find('"') else { break };
            let href = &rest[..end];
            rest = &rest[end + 1..];
            let Some((target, anchor)) = href.split_once('#') else {
                continue;
            };
            if anchor.is_empty() {
                continue;
            }
            report.fragment_links += 1;
            let target = if target.is_empty() {
                name.as_str()
            } else {
                target
            };
            let resolved = ids
                .get(target)
                .is_some_and(|anchors| anchors.contains(anchor));
            if resolved {
                report.resolved_fragments += 1;
            } else {
                let in_letters = target.len() == 7
                    && target.as_bytes().get(2) == Some(&b'-')
                    && target.ends_with(".md");
                if in_letters {
                    report.unresolved_in_letters += 1;
                }
                if unresolved.len() < 8 {
                    unresolved.push((in_letters, format!("{name} → {href}")));
                }
            }
        }
    }
    report.unresolved_fragments = report.fragment_links - report.resolved_fragments;
    report.unresolved_examples = unresolved.into_iter().map(|(_, example)| example).collect();
}

fn ids_in(content: &str) -> HashSet<String> {
    let mut ids = HashSet::new();
    let mut rest = content;
    while let Some(start) = rest.find("id=\"") {
        rest = &rest[start + 4..];
        let Some(end) = rest.find('"') else { break };
        ids.insert(rest[..end].to_owned());
        rest = &rest[end + 1..];
    }
    ids
}

fn unreadable(error: std::io::Error) -> DictionaryError {
    DictionaryError::unreadable(error.to_string())
}

/// The path the copy is written to, resolved for display.
pub fn destination_path(destination: &Path) -> PathBuf {
    std::path::absolute(destination).unwrap_or_else(|_| destination.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    /// An invented source dictionary, written at test time. It is not derived
    /// from any published work; only the structural shapes matter here.
    fn fixture() -> PathBuf {
        static SOURCE: OnceLock<PathBuf> = OnceLock::new();
        SOURCE
            .get_or_init(|| {
                let directory = std::env::temp_dir().join(format!(
                    "mcp-chinese-dict-linked-source-{}",
                    std::process::id()
                ));
                let _ = fs::remove_dir_all(&directory);
                fs::create_dir_all(&directory).expect("the source directory is created");
                for (name, text) in [
                    (
                        "08-a.md",
                        "\\*alpha\n\n甲 | 甲  \\*jia3 / jiǎ\n\n\
                         Also alpha alias. See <a href=\"14-g.md#a1BET\">greater alpha</a>.\n\n\
                         \\*alpha swelling\n\n甲肿 | 甲腫  \\*jia3 zhong3 / jiǎ zhǒng\n\n\
                         A made-up feeling of swelling. See <a href=\"10-c.md#a1CHT\">clear heat</a>.\n",
                    ),
                    (
                        "10-c.md",
                        "\\*clear heat\n\n清甲 | 清甲  \\*qing1 jia3 / qīng jiǎ\n\n\
                         A made-up treatment method that clears heat.\n",
                    ),
                    (
                        "14-g.md",
                        "\\*greater alpha\n\n大甲 | 大甲  \\*da4 jia3 / dà jiǎ\n\n\
                         The upper part of the alpha region.\n",
                    ),
                    (
                        "35-appendix-2-Medicinal.md",
                        "\\[01.01.01\\] JIǍ GĒN / jia gen 甲根 / 甲根 (Jiae Herba, jia root): \
                         Alt. Eng.: jiagen.\n\n• Properties: Bland, warm.\n",
                    ),
                    (
                        "38-index.md",
                        "Index\n\n\
                         <a href=\"08-a.md#a1ALP\" class=\"class_s1NTM-0\">alpha</a>\n\n\
                         <a href=\"08-a.md#a1ASW\" class=\"class_s1NTM-0\">alpha swelling</a>\n\n\
                         <a href=\"14-g.md#a1BET\" class=\"class_s1NTM-0\">greater alpha</a>\n\n\
                         <a href=\"10-c.md#a1CHT\" class=\"class_s1NTM-0\">clear heat</a>\n",
                    ),
                ] {
                    fs::write(directory.join(name), text).expect("a source file is written");
                }
                directory
            })
            .clone()
    }

    fn destination(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "mcp-chinese-dict-linked-{}-{label}",
            std::process::id()
        ))
    }

    #[test]
    fn anchors_are_inserted_before_entries_without_changing_content() {
        let destination = destination("basic");
        let _ = fs::remove_dir_all(&destination);

        let report = build_linked_data(&fixture(), &destination).expect("the copy builds");
        assert_eq!(report.files, 5);
        assert_eq!(report.anchors_inserted, 4, "got {report:?}");
        assert_eq!(report.content_mismatches, 0);
        assert_eq!(report.unresolved_in_letters, 0, "got {report:?}");
        assert!(report.resolved_fragments > 0);

        let text = fs::read_to_string(destination.join("08-a.md")).expect("the copy exists");
        assert!(text.contains("<a id=\"a1ASW\"></a>\n\\*alpha swelling"));
        let clearing = fs::read_to_string(destination.join("10-c.md")).expect("the copy exists");
        assert!(clearing.contains("<a id=\"a1CHT\"></a>\n\\*clear heat"));

        let _ = fs::remove_dir_all(&destination);
    }

    #[test]
    fn building_from_a_linked_copy_adds_nothing() {
        let first_directory = destination("twice");
        let second_directory = destination("twice-second");
        let _ = fs::remove_dir_all(&first_directory);
        let _ = fs::remove_dir_all(&second_directory);

        let first = build_linked_data(&fixture(), &first_directory).expect("the copy builds");
        assert!(first.anchors_inserted > 0);
        let text = fs::read_to_string(first_directory.join("08-a.md")).expect("the copy exists");
        assert_eq!(text.matches("<a id=\"a1ASW\"></a>").count(), 1);

        // A copy is already linked: rebuilding from it must insert nothing.
        let second =
            build_linked_data(&first_directory, &second_directory).expect("the second copy builds");
        assert_eq!(second.anchors_inserted, 0, "got {second:?}");
        assert_eq!(second.content_mismatches, 0);

        let _ = fs::remove_dir_all(&first_directory);
        let _ = fs::remove_dir_all(&second_directory);
    }

    #[test]
    fn escaped_brackets_are_unescaped_in_the_copy() {
        let destination = destination("brackets");
        let _ = fs::remove_dir_all(&destination);

        build_linked_data(&fixture(), &destination).expect("the copy builds");
        let text = fs::read_to_string(destination.join("35-appendix-2-Medicinal.md"))
            .expect("the copy exists");
        assert!(
            text.contains("[01.01.01] JIǍ GĒN"),
            "the monograph heading is preview-safe: {text}"
        );
        assert!(!text.contains("\\["), "no math delimiter remains: {text}");

        let _ = fs::remove_dir_all(&destination);
    }

    #[test]
    fn a_copy_inside_the_source_is_refused() {
        let error = build_linked_data(&fixture(), &fixture().join("copy")).unwrap_err();
        assert!(error.to_string().contains("outside the source"));
    }
}
