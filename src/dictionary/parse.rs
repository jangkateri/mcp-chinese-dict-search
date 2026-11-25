//! Parse main-dictionary letter files into entries, aliases, and links.

use crate::dictionary::model::{Alias, Entry, Link};
use crate::dictionary::normalize::{
    collapse_whitespace, contains_han, normalize, unescape_markdown,
};

/// Parse one `NN-x.md` letter file.
pub fn parse_entries(file: &str, content: &str) -> (Vec<Entry>, usize) {
    let lines: Vec<&str> = content.lines().collect();
    let starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("\\*"))
        .map(|(index, _)| index)
        .collect();

    let mut entries = Vec::with_capacity(starts.len());
    let mut alias_failures = 0;

    for (position, &start) in starts.iter().enumerate() {
        let end = starts.get(position + 1).copied().unwrap_or(lines.len());
        let headword = lines[start][2..].trim().to_owned();
        let mut cursor = start + 1;

        while cursor < end && lines[cursor].trim().is_empty() {
            cursor += 1;
        }

        let mut aliases = Vec::new();
        while cursor < end {
            let line = lines[cursor];
            if line.trim().is_empty() {
                cursor += 1;
                continue;
            }
            match parse_alias_line(line) {
                Some(alias) => {
                    aliases.push(alias);
                    cursor += 1;
                }
                None => {
                    if line.contains('|') && line.contains("\\*") {
                        alias_failures += 1;
                    }
                    break;
                }
            }
        }

        // A linked copy puts `<a id="…"></a>` on its own line before a headword.
        // That line falls at the end of the previous entry's range; dropping it
        // keeps the body, and therefore the entry text, identical to the source.
        let mut body_lines: Vec<&str> = lines[cursor..end]
            .iter()
            .copied()
            .filter(|line| !is_anchor_marker(line))
            .collect();
        while body_lines
            .first()
            .is_some_and(|line| line.trim().is_empty())
        {
            body_lines.remove(0);
        }
        while body_lines.last().is_some_and(|line| line.trim().is_empty()) {
            body_lines.pop();
        }
        let body = body_lines.join("\n");

        entries.push(Entry {
            norm_headword: normalize(&headword),
            headword,
            file: file.to_owned(),
            line: start + 1,
            anchor: None,
            aliases,
            norm_body: normalize(&plain_text(&body)),
            links: parse_links(&body),
            body,
        });
    }

    (entries, alias_failures)
}

/// A standalone anchor target written by the linked-copy build, for example
/// `<a id="a1ASW"></a>`. It carries no entry text.
pub(crate) fn is_anchor_marker(line: &str) -> bool {
    let line = line.trim();
    line.starts_with("<a id=\"") && line.ends_with("\"></a>")
}

/// Parse one alias line, tolerating the corpus's known irregularities.
pub fn parse_alias_line(line: &str) -> Option<Alias> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    if let Some(star) = line.find("\\*") {
        let left = unescape_markdown(line[..star].trim());
        let right = unescape_markdown(line[star + 2..].trim());
        let (simplified, traditional) = split_han_pair(&left);
        if !contains_han(simplified) && !contains_han(traditional) {
            return None;
        }
        let (pinyin_toned, pinyin_ascii) = split_pinyin_pair(&right);
        return Some(Alias {
            simplified: clean(simplified),
            traditional: clean(traditional),
            pinyin_toned: clean(pinyin_toned),
            pinyin_ascii: clean(pinyin_ascii),
        });
    }

    // `片 pian4` (`tablet`): Han characters followed by pinyin, no `|` or `\*`.
    if contains_han(line) {
        let (han, latin) = split_at_first_latin(line);
        if !han.trim().is_empty() && !latin.trim().is_empty() {
            let (simplified, traditional) = split_han_pair(han);
            return Some(Alias {
                simplified: clean(simplified),
                traditional: clean(traditional),
                pinyin_toned: clean(latin),
                pinyin_ascii: String::new(),
            });
        }
    }

    None
}

fn split_han_pair(text: &str) -> (&str, &str) {
    match text.split_once('|') {
        Some((simplified, traditional)) => (simplified.trim(), traditional.trim()),
        None => (text.trim(), ""),
    }
}

fn split_pinyin_pair(text: &str) -> (&str, &str) {
    if let Some((toned, ascii)) = text.split_once(" / ") {
        (toned.trim(), ascii.trim())
    } else if let Some((toned, ascii)) = text.split_once('/') {
        (toned.trim(), ascii.trim())
    } else {
        (text.trim(), "")
    }
}

fn clean(text: &str) -> String {
    collapse_whitespace(&text.replace('\u{a0}', " "))
}

/// Split at the first ASCII alphabetic character.
pub fn split_at_first_latin(text: &str) -> (&str, &str) {
    let split = text
        .char_indices()
        .find(|(_, character)| character.is_ascii_alphabetic())
        .map(|(index, _)| index)
        .unwrap_or(text.len());
    (&text[..split], &text[split..])
}

/// Split at the first Han character.
pub fn split_at_first_han(text: &str) -> (&str, &str) {
    let split = text
        .char_indices()
        .find(|(_, character)| super::normalize::is_han(*character))
        .map(|(index, _)| index)
        .unwrap_or(text.len());
    (&text[..split], &text[split..])
}

/// Parse every `<a href="file#anchor">text</a>` link in an entry body.
pub fn parse_links(body: &str) -> Vec<Link> {
    let mut links = Vec::new();
    let mut rest = body;

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
        let raw_text = &after[text_start..text_start + end];
        let (target_file, target_anchor) = match href.split_once('#') {
            Some((file, anchor)) if !file.is_empty() => {
                (Some(file.to_owned()), Some(anchor.to_owned()))
            }
            Some((_, anchor)) => (None, Some(anchor.to_owned())),
            None => (Some(href.to_owned()), None),
        };
        links.push(Link {
            text: clean(&strip_tags(raw_text)),
            target_file,
            target_anchor,
            target_term: None,
            resolved: false,
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

/// Render body markdown as plain text for display and matching.
pub fn plain_text(body: &str) -> String {
    let text = body
        .replace("</td>", " | ")
        .replace("</th>", " | ")
        .replace("</tr>", "\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n");
    unescape_markdown(&strip_tags(&text)).trim().to_owned()
}

pub fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0usize;
    for character in text.chars() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(character),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\\*alpha\r\n\r\n甲 | 甲  \\*jia3 / jiǎ\r\n\r\nAlso alpha alias. See <a href=\"14-g.md#a1BET\" class=\"class_s1NTM-1\">greater alpha</a>.\r\n\r\n\\*alpha swelling\r\n\r\n甲肿 | 甲腫  \\*jia3 zhong3 / jiǎ zhǒng\r\n甲胀 | 甲脹  \\*jia3 zhang4 / jiǎ zhàng\r\n\r\nA made-up feeling of swelling.\r\n\r\n\\*empty\r\n";

    #[test]
    fn parses_entries_aliases_and_body() {
        let (entries, failures) = parse_entries("08-a.md", SAMPLE);
        assert_eq!(failures, 0);
        assert_eq!(entries.len(), 3);

        assert_eq!(entries[0].headword, "alpha");
        assert_eq!(entries[0].aliases.len(), 1);
        assert_eq!(entries[0].aliases[0].simplified, "甲");
        assert_eq!(entries[0].aliases[0].traditional, "甲");
        assert_eq!(entries[0].aliases[0].pinyin_toned, "jia3");
        assert_eq!(entries[0].aliases[0].pinyin_ascii, "jiǎ");
        assert!(entries[0].body.contains("Also alpha alias."));
        assert_eq!(entries[0].line, 1);

        assert_eq!(entries[1].headword, "alpha swelling");
        assert_eq!(entries[1].aliases.len(), 2);

        assert_eq!(entries[2].headword, "empty");
        assert!(entries[2].aliases.is_empty());
        assert!(entries[2].body.is_empty());
    }

    #[test]
    fn parses_links_and_splits_href() {
        let (entries, _) = parse_entries("08-a.md", SAMPLE);
        let links = &entries[0].links;
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].text, "greater alpha");
        assert_eq!(links[0].target_file.as_deref(), Some("14-g.md"));
        assert_eq!(links[0].target_anchor.as_deref(), Some("a1BET"));
        assert!(!links[0].resolved);
    }

    #[test]
    fn tolerates_escaped_pipes_and_missing_delimiters() {
        let alias = parse_alias_line("二阴 \\| 二陰  \\*er4 yin1 /èr yīn").unwrap();
        assert_eq!(alias.simplified, "二阴");
        assert_eq!(alias.traditional, "二陰");
        assert_eq!(alias.pinyin_toned, "er4 yin1");
        assert_eq!(alias.pinyin_ascii, "èr yīn");

        let alias = parse_alias_line("片 pian4").unwrap();
        assert_eq!(alias.simplified, "片");
        assert_eq!(alias.pinyin_toned, "pian4");
        assert!(alias.pinyin_ascii.is_empty());
    }

    #[test]
    fn variant_pinyin_stays_in_the_alias() {
        let alias = parse_alias_line(
            "爪切进针法 \\| 爪切進針法 \u{a0}\\*zhao3 qie4 jin4 zhen1 fa3 / zhǎo qiè jìn zhēn fǎ \\[\\*zhua3 qie4 jin4 zhen1 fa3 / zhuǎ qiè jìn zhēn fǎ\\]",
        )
        .unwrap();
        assert_eq!(alias.pinyin_toned, "zhao3 qie4 jin4 zhen1 fa3");
        assert!(alias.pinyin_ascii.contains("zhuǎ qiè"));
    }

    #[test]
    fn rejects_non_alias_paragraphs() {
        assert!(parse_alias_line("See <a href=\"10-c.md#a1CHT\">clear heat</a>.").is_none());
        assert!(parse_alias_line("PATTERNS").is_none());
    }

    #[test]
    fn malformed_letter_input_does_not_panic() {
        let (entries, _) = parse_entries("08-a.md", "");
        assert!(entries.is_empty());

        let (entries, _) = parse_entries("08-a.md", "\\*truncated\n\nabc |");
        assert_eq!(entries.len(), 1);
        assert!(entries[0].aliases.is_empty());
        assert!(entries[0].body.contains("abc |"));

        let (entries, _) = parse_entries("08-a.md", "\\*only headword");
        assert_eq!(entries.len(), 1);
        assert!(entries[0].body.is_empty());
    }
}
