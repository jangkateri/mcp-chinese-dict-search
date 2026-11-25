//! Parse the four appendices.
//!
//! Appendix 1 is a list of acupuncture points; appendices 2 and 3 are numbered
//! monographs; appendix 4 is a set of tables. The grammars differ, and the
//! corpus carries `⁑` markers, nested parentheses, escaped brackets, and two
//! stray `No Match` suffixes. The parsers below tolerate all of them.

use crate::dictionary::model::{AppendixEntry, Section, Source};
use crate::dictionary::normalize::{collapse_whitespace, normalize, unescape_markdown};
use crate::dictionary::parse::{plain_text, split_at_first_han, split_at_first_latin};

/// Parse appendix 1 (meridians and acupuncture points).
pub fn parse_meridian(file: &str, content: &str) -> (Vec<AppendixEntry>, usize) {
    let lines: Vec<&str> = content.lines().collect();
    let mut entries = Vec::new();
    let mut failures = 0;
    let mut section = String::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        if let Some((number, title)) = numbered_heading(line) {
            section = format!("{number} {title}");
            index += 1;
            continue;
        }
        let Some(header) = point_header(line) else {
            if looks_like_point_start(line) {
                failures += 1;
            }
            index += 1;
            continue;
        };

        let body_start = index + 1;
        let mut body_end = body_start;
        while body_end < lines.len()
            && point_header(lines[body_end]).is_none()
            && numbered_heading(lines[body_end]).is_none()
        {
            body_end += 1;
        }
        let body = body_text(lines[body_start..body_end].iter().copied(), &header.rest);
        entries.push(AppendixEntry {
            source: Source::Meridian,
            file: file.to_owned(),
            line: index + 1,
            id: Some(header.id.clone()),
            section: section.clone(),
            subsection: None,
            norm_terms: normalized_terms(&header.aliases),
            term: header.term,
            simplified: header.simplified,
            traditional: header.traditional,
            pinyin_toned: header.pinyin_toned,
            pinyin_ascii: header.pinyin_ascii,
            aliases: header.aliases,
            norm_body: normalize(&plain_text(&body)),
            body,
        });
        index = body_end;
    }

    (entries, failures)
}

/// Parse appendix 2 (medicinal agents).
pub fn parse_medicinals(file: &str, content: &str) -> (Vec<AppendixEntry>, usize) {
    parse_monographs(file, content, Source::Medicinal)
}

/// Parse appendix 3 (herbal formulas).
pub fn parse_formulas(file: &str, content: &str) -> (Vec<AppendixEntry>, usize) {
    parse_monographs(file, content, Source::Formulas)
}

/// Parse appendix 4 (weights and measures). `file` is the source file name,
/// carried for citations.
pub fn parse_weights(file: &str, content: &str) -> Vec<Section> {
    let lines: Vec<&str> = content.lines().collect();
    let mut sections = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        let Some((number, title)) = weights_heading(line) else {
            index += 1;
            continue;
        };
        let start = index + 1;
        let mut end = start;
        while end < lines.len() && weights_heading(lines[end]).is_none() {
            end += 1;
        }
        let body = lines[start..end].join("\n").trim().to_owned();
        sections.push(Section {
            file: file.to_owned(),
            number,
            title,
            body,
        });
        index = end;
    }

    sections
}

fn parse_monographs(file: &str, content: &str, source: Source) -> (Vec<AppendixEntry>, usize) {
    let lines: Vec<&str> = content.lines().collect();
    let mut entries = Vec::new();
    let mut failures = 0;
    let mut section = String::new();
    let mut subsection: Option<String> = None;
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        if let Some((number, title)) = numbered_heading(line) {
            section = format!("{number} {title}");
            subsection = None;
            index += 1;
            continue;
        }

        let Some((id, header_line)) = monograph_line(line) else {
            if is_subsection_heading(line)
                && next_non_empty(&lines, index + 1)
                    .is_some_and(|next| monograph_line(next).is_some())
            {
                subsection = Some(clean(line));
            }
            index += 1;
            continue;
        };

        match monograph_header(&header_line) {
            Some(header) => {
                let body_start = index + 1;
                let mut body_end = body_start;
                while body_end < lines.len()
                    && monograph_line(lines[body_end]).is_none()
                    && numbered_heading(lines[body_end]).is_none()
                {
                    body_end += 1;
                }
                let body = body_text(lines[body_start..body_end].iter().copied(), &header.rest);
                entries.push(AppendixEntry {
                    source,
                    file: file.to_owned(),
                    line: index + 1,
                    id: Some(id),
                    section: section.clone(),
                    subsection: subsection.clone(),
                    norm_terms: normalized_terms(&header.aliases),
                    term: header.term,
                    simplified: header.simplified,
                    traditional: header.traditional,
                    pinyin_toned: header.pinyin_toned,
                    pinyin_ascii: header.pinyin_ascii,
                    aliases: header.aliases,
                    norm_body: normalize(&plain_text(&body)),
                    body,
                });
                index = body_end;
            }
            None => {
                failures += 1;
                index += 1;
            }
        }
    }

    (entries, failures)
}

#[derive(Debug)]
struct ParsedHeader {
    id: String,
    term: String,
    simplified: String,
    traditional: String,
    pinyin_toned: String,
    pinyin_ascii: String,
    aliases: Vec<String>,
    rest: String,
}

fn point_header(line: &str) -> Option<ParsedHeader> {
    let line = line.trim().trim_end_matches("No Match").trim();
    let open = line.find(" (")?;
    let id = line[..open].trim();
    if !looks_like_point_id(id) {
        return None;
    }
    let close = matching_paren(line, open + 1)?;
    let inner = &line[open + 2..close];
    let rest = line[close + 1..].trim_start_matches(':').trim().to_owned();

    let parts: Vec<&str> = inner.splitn(3, " / ").collect();
    let (simplified, traditional, toned, ascii, english) = match parts.len() {
        3 => {
            let (traditional, toned) = split_at_first_latin(parts[1]);
            let (ascii, english) = parts[2].split_once(", ").unwrap_or((parts[2], parts[2]));
            (
                clean(parts[0]),
                clean(traditional),
                clean(toned),
                clean(ascii),
                clean(english),
            )
        }
        2 => {
            let (traditional, toned) = split_at_first_latin(parts[1]);
            (
                clean(parts[0]),
                clean(traditional),
                clean(toned),
                String::new(),
                clean(traditional),
            )
        }
        _ => (
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            clean(inner),
        ),
    };

    let term = if english.is_empty() {
        if !ascii.is_empty() {
            ascii.clone()
        } else {
            simplified.clone()
        }
    } else {
        english
    };
    let mut aliases = vec![
        id.to_owned(),
        term.clone(),
        simplified.clone(),
        traditional.clone(),
        toned.clone(),
        ascii.clone(),
    ];
    add_alias_text(&mut aliases, &rest);

    Some(ParsedHeader {
        id: id.to_owned(),
        term,
        simplified,
        traditional,
        pinyin_toned: toned,
        pinyin_ascii: ascii,
        aliases,
        rest,
    })
}

fn monograph_header(header: &str) -> Option<ParsedHeader> {
    let open = header.find(" (")?;
    let prefix = header[..open].trim();
    let close = matching_paren(header, open + 1)?;
    let inside = &header[open + 2..close];
    let rest = header[close + 1..]
        .trim_start_matches(':')
        .trim()
        .to_owned();

    let (upper, remainder) = prefix.split_once(" / ").unwrap_or((prefix, ""));
    let (lower, chinese) = split_at_first_han(remainder);
    let (simplified, traditional) = chinese.split_once(" / ").unwrap_or((chinese, chinese));

    let (latin, english) = inside.split_once(", ").unwrap_or(("", inside));
    let simplified = clean(simplified);
    let traditional = clean(traditional);
    let term = clean(english);
    let term = if term.is_empty() {
        simplified.clone()
    } else {
        term
    };

    let mut aliases = vec![
        clean(upper),
        clean(lower),
        simplified.clone(),
        traditional.clone(),
        clean(latin),
        term.clone(),
    ];
    add_alias_text(&mut aliases, &rest);

    Some(ParsedHeader {
        id: String::new(),
        term,
        simplified,
        traditional,
        pinyin_toned: clean(upper),
        pinyin_ascii: clean(lower),
        aliases,
        rest,
    })
}

fn body_text<'a>(lines: impl Iterator<Item = &'a str>, rest: &str) -> String {
    let mut body = String::new();
    if !rest.is_empty() {
        body.push_str(rest);
    }
    for line in lines {
        if body.is_empty() {
            body.push_str(line);
        } else {
            body.push('\n');
            body.push_str(line);
        }
    }
    body.trim().to_owned()
}

fn normalized_terms(aliases: &[String]) -> Vec<String> {
    let mut terms = Vec::new();
    for alias in aliases {
        let normalized = normalize(alias);
        if !normalized.is_empty() && !terms.contains(&normalized) {
            terms.push(normalized);
        }
    }
    terms
}

fn add_alias_text(aliases: &mut Vec<String>, text: &str) {
    let text = clean(text);
    if text.is_empty() {
        return;
    }
    if !aliases.contains(&text) {
        aliases.push(text.clone());
    }
    for part in text.split([';', ',']) {
        let part = clean(part);
        if part.is_empty() {
            continue;
        }
        if !aliases.contains(&part) {
            aliases.push(part.clone());
        }
        for run in han_runs(&part) {
            if !aliases.contains(&run) {
                aliases.push(run);
            }
        }
    }
}

fn clean(text: &str) -> String {
    let text = unescape_markdown(text).replace('\u{a0}', " ");
    collapse_whitespace(&text.replace('⁑', ""))
}

fn han_runs(text: &str) -> Vec<String> {
    let mut runs = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if super::normalize::is_han(character) {
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

fn matching_paren(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, character) in text[open..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn numbered_heading(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    let split = line.find(' ')?;
    let prefix = &line[..split];
    // Section headings are `1.1 …`, not numbered-list items such as `1. …`.
    let (left, right) = prefix.split_once('.')?;
    if left.is_empty()
        || right.is_empty()
        || !left.chars().all(|character| character.is_ascii_digit())
        || !right.chars().all(|character| character.is_ascii_digit())
    {
        return None;
    }
    Some((prefix.to_owned(), line[split + 1..].trim().to_owned()))
}

fn weights_heading(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    let split = line.find(' ')?;
    let prefix = &line[..split];
    let number = prefix
        .strip_suffix("\\.")
        .or_else(|| prefix.strip_suffix('.'))?;
    if number.is_empty() || !number.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    Some((number.to_owned(), line[split + 1..].trim().to_owned()))
}

fn monograph_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    // The source archive escapes the brackets (`\[01.01.01\]`); the linked copy
    // unescapes them so a math-enabled Markdown preview does not read the line
    // as LaTeX display math. Accept either form.
    let rest = line
        .strip_prefix("\\[")
        .or_else(|| line.strip_prefix('['))?;
    let (end, skip) = match rest.find("\\]") {
        Some(position) => (position, 2),
        None => (rest.find(']')?, 1),
    };
    let id = rest[..end].trim();
    if id.is_empty() {
        return None;
    }
    Some((id.to_owned(), rest[end + skip..].trim().to_owned()))
}

fn looks_like_point_start(line: &str) -> bool {
    let line = line.trim();
    line.find(" (")
        .is_some_and(|open| looks_like_point_id(line[..open].trim()))
}

fn looks_like_point_id(id: &str) -> bool {
    !id.is_empty()
        && id.contains('-')
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
        && id
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase())
}

fn next_non_empty<'a>(lines: &'a [&'a str], start: usize) -> Option<&'a str> {
    lines[start..]
        .iter()
        .find(|line| !line.trim().is_empty())
        .copied()
}

fn is_subsection_heading(line: &str) -> bool {
    let line = line.trim();
    if line.is_empty()
        || line.starts_with('•')
        || line.starts_with('□')
        || line.starts_with("\\[")
        || line.starts_with('|')
        || line.contains("<a ")
        || line.len() > 90
    {
        return false;
    }
    !line.ends_with(['.', ':', ';', '。', '!', '?'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_point_with_alt_names_and_stray_suffix() {
        let content = "\
1.3 AA, Made-up Channel\r
\r
AA-1 (甲三里 / 甲三里 jiǎ sān lǐ / jia san li, Jia Three Li): Alt. Names: 下甲 xià jiǎ, Lower Jia.\r
\r
• Location: On the made-up limb.\r
\r
• Actions: Fortifies the liver.\r
";
        let (entries, failures) = parse_meridian("34-appendix-1-Meridian.md", content);
        assert_eq!(failures, 0);
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.id.as_deref(), Some("AA-1"));
        assert_eq!(entry.term, "Jia Three Li");
        assert!(entry.aliases.iter().any(|alias| alias == "甲三里"));
        assert!(entry.aliases.iter().any(|alias| alias == "jia san li"));
        assert!(entry.aliases.iter().any(|alias| alias == "下甲"));
        assert!(entry.body.contains("• Location"));
        assert_eq!(entry.section, "1.3 AA, Made-up Channel");

        let content =
            "BB-2 (下乙 / 下乙 xià yǐ / xia yi, Lower Yi)No Match\r\n\r\n• Location: Sacrum.\r\n";
        let (entries, _) = parse_meridian("34-appendix-1-Meridian.md", content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].term, "Lower Yi");
    }

    #[test]
    fn parses_a_medicinal_header_with_marker_and_nested_parentheses() {
        let content = "\
2.1 Made-up Agents\r
\r
Warm Made-up Agents\r
\r
\\[01.01.01\\] JIǍ GĒN / jia gen 甲根 / 甲根 (Jiae Herba, jia root): Alt. Eng.: jiagen. Category: Made-up agent.\r
\r
• Properties: Bland, warm.\r
\r
\\[02.01.05.A\\] YǏ JĪ / yi ji 乙己⁑ / 乙己⁑ (Yijiae Folium, yi leaf): Alt. Eng.: yi leaf⁑.\r
\r
• Properties: Sour, cool.\r
\r
\\[06.02.14\\] BǏNG CǍO / bing cao 丙草 / 丙草 (Bingiae Herba (cum Radice), bing grass): Category: Made-up agent.\r
";
        let (entries, failures) = parse_medicinals("35-appendix-2-Medicinal.md", content);
        assert_eq!(failures, 0);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].id.as_deref(), Some("01.01.01"));
        assert_eq!(entries[0].term, "jia root");
        assert!(entries[0].aliases.iter().any(|alias| alias == "JIǍ GĒN"));
        assert!(entries[0].aliases.iter().any(|alias| alias == "甲根"));
        assert_eq!(
            entries[0].subsection.as_deref(),
            Some("Warm Made-up Agents")
        );
        assert!(entries[1].aliases.iter().any(|alias| alias == "乙己"));
        assert!(!entries[1].aliases.iter().any(|alias| alias.contains('⁑')));
        assert_eq!(entries[2].term, "bing grass");
        assert!(
            entries[2]
                .aliases
                .iter()
                .any(|alias| alias.contains("cum Radice"))
        );
    }

    #[test]
    fn accepts_unescaped_monograph_brackets_from_the_linked_copy() {
        let content = "\
2.1 Made-up Agents\r
\r
[01.01.01] JIǍ GĒN / jia gen 甲根 / 甲根 (Jiae Herba, jia root): Alt. Eng.: jiagen. Category: Made-up agent.\r
\r
• Properties: Bland, warm.\r
";
        let (entries, failures) = parse_medicinals("35-appendix-2-Medicinal.md", content);
        assert_eq!(failures, 0);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id.as_deref(), Some("01.01.01"));
        assert_eq!(entries[0].term, "jia root");
    }

    #[test]
    fn parses_a_formula_with_a_single_chinese_form() {
        let content = "\
3.1 Made-up Formulas\r
\r
\\[01.01.01\\] JIǍ GĒN TĀNG / jia gen tang 甲根汤 (Jia Decoction): Made-up warm formula.\r
\r
□ Jiǎ gēn / jia gen 甲根 (Jiae Herba, jia root) 9g\r
\r
\\[01.01.01.C\\] DÀ BǏNG TĀNG / da bing tang 大丙汤 (Major Bing Decoction): Variant.\r
";
        let (entries, failures) = parse_formulas("36-appendix-3-Formulas.md", content);
        assert_eq!(failures, 0);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].source, Source::Formulas);
        assert_eq!(entries[0].term, "Jia Decoction");
        assert!(entries[0].aliases.iter().any(|alias| alias == "甲根汤"));
        assert_eq!(entries[1].id.as_deref(), Some("01.01.01.C"));
    }

    #[test]
    fn parses_weights_sections() {
        let content = "\
21\\. Weights in Different Dynasties\r
\r
| Dynasty | Grams/jin |\r
|---|---|\r
| Zhou | 14.18 |\r
\r
22\\. Measures in Different Dynasties\r
\r
| Dynasty | Ml./dou |\r
|---|---|\r
\r
23\\. Weights Conversion Table\r
\r
| Unit | Value |\r
";
        let sections = parse_weights("37-appendix-4-Weights-Measurements.md", content);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].file, "37-appendix-4-Weights-Measurements.md");
        assert_eq!(sections[0].number, "21");
        assert_eq!(sections[0].title, "Weights in Different Dynasties");
        assert!(sections[0].body.contains("14.18"));
        assert_eq!(sections[2].number, "23");
    }

    #[test]
    fn malformed_appendix_input_does_not_panic() {
        let (entries, failures) = parse_meridian("34-appendix-1-Meridian.md", "AA-1 (甲三里");
        assert!(entries.is_empty());
        assert_eq!(failures, 1);

        let (entries, failures) =
            parse_medicinals("35-appendix-2-Medicinal.md", "\\[01.01.01\\] broken");
        assert!(entries.is_empty());
        assert_eq!(failures, 1);

        let (entries, failures) = parse_formulas("36-appendix-3-Formulas.md", "garbage\n");
        assert!(entries.is_empty());
        assert_eq!(failures, 0);

        let sections = parse_weights("37-appendix-4.md", "garbage\n\n21\\. Title\n| x |");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].file, "37-appendix-4.md");
    }
}
