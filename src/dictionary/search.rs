//! Ranking, snippets, scope routing, exact-entry retrieval, and appendix links.

use std::collections::HashMap;

use crate::dictionary::Dictionary;
use crate::dictionary::fuzzy;
use crate::dictionary::model::{
    AppendixRef, Entry, EntryAliases, EntryRequest, EntryResponse, LookupRef, MatchKind,
    RelatedEntry, Scope, SearchHit, SearchRequest, SearchResponse, Source,
};
use crate::dictionary::normalize::{collapse_whitespace, contains_han, normalize};
use crate::dictionary::parse::plain_text;
use crate::dictionary::pinyin;
use crate::error::DictionaryError;

const CROSS_REFERENCE_LIMIT: usize = 10;

struct Candidate {
    score: u32,
    order: usize,
    hit: SearchHit,
}

/// Exact pinyin or Han matches for one query.
#[derive(Debug, Default)]
struct ExactLookup {
    /// True when the query is specific enough to answer from the index alone.
    shortcut: bool,
    entry_scores: HashMap<usize, u32>,
    appendix_scores: HashMap<AppendixRef, u32>,
    targets: Vec<(LookupRef, u32)>,
}

pub fn search(
    dictionary: &Dictionary,
    request: &SearchRequest,
) -> Result<SearchResponse, DictionaryError> {
    let query = request.query.trim();
    let normalized = normalize(query);
    if normalized.is_empty() {
        return Err(DictionaryError::new(
            "The \"query\" argument is empty after normalization.",
            crate::error::DictionaryErrorCode::InvalidRequest,
        ));
    }

    if request.scope.allows(Source::Dictionary)
        && let Some(index) = find_entry_by_anchor(dictionary, query)
    {
        let entry = &dictionary.entries[index];
        let (snippet, truncated, total_chars) = truncate(plain_text(&entry.body), 20_000);
        let snippet = if truncated {
            format!(
                "{snippet}\n[truncated: showing the first 20,000 of {total_chars} characters. Use get_entry \
                 with a larger max_chars to read further.]"
            )
        } else {
            snippet
        };
        return Ok(SearchResponse {
            query: query.to_owned(),
            scope: request.scope,
            hits: vec![SearchHit {
                source: Source::Dictionary,
                term: entry.headword.clone(),
                id: None,
                snippet,
                matched_in: MatchKind::Headword,
                score: 1000,
                file: entry.file.clone(),
                anchor: entry.anchor.clone(),
                redirect: is_redirect(&entry.body),
                source_file: dictionary.source_file(&entry.file),
                line: Some(entry.line),
            }],
            total: 1,
            suggestions: Vec::new(),
        });
    }

    let exact = exact_lookup(dictionary, query, request.scope);
    if let Some(exact) = &exact
        && exact.shortcut
    {
        let mut hits: Vec<SearchHit> = exact
            .targets
            .iter()
            .filter_map(|(target, score)| lookup_hit(dictionary, *target, *score, &normalized))
            .collect();
        let total = hits.len();
        hits.truncate(request.limit);
        return Ok(SearchResponse {
            query: query.to_owned(),
            scope: request.scope,
            hits,
            total,
            suggestions: Vec::new(),
        });
    }

    let tokens = tokenize(&normalized);
    let dictionary_body = normalized.chars().count() >= 3 || contains_han(&normalized);
    let appendix_body = request.scope != Scope::All;
    let weights_body =
        request.scope == Scope::Weights || is_measurement_query(&normalized, &tokens);

    let mut candidates = Vec::new();
    let mut order = 0usize;

    if request.scope.allows(Source::Dictionary) {
        for (index, entry) in dictionary.entries.iter().enumerate() {
            let exact_score = exact
                .as_ref()
                .and_then(|exact| exact.entry_scores.get(&index).copied());
            if let Some((score, kind)) =
                score_entry(entry, &normalized, &tokens, dictionary_body, exact_score)
            {
                let snippet = if kind == MatchKind::Body {
                    snippet_around(
                        &entry.body,
                        tokens.first().map(String::as_str).unwrap_or(&normalized),
                    )
                } else {
                    first_paragraphs(&entry.body, 500)
                };
                candidates.push(Candidate {
                    score,
                    order,
                    hit: SearchHit {
                        source: Source::Dictionary,
                        term: entry.headword.clone(),
                        id: None,
                        snippet,
                        matched_in: kind,
                        score,
                        file: entry.file.clone(),
                        anchor: entry.anchor.clone(),
                        redirect: is_redirect(&entry.body),
                        source_file: dictionary.source_file(&entry.file),
                        line: Some(entry.line),
                    },
                });
            }
            order += 1;
        }
    }

    for reference in dictionary.appendix_entries(request.scope) {
        let Some(entry) = dictionary.appendix_entry(reference) else {
            continue;
        };
        let exact_score = exact
            .as_ref()
            .and_then(|exact| exact.appendix_scores.get(&reference).copied());
        if let Some((score, kind)) =
            score_appendix(entry, &normalized, &tokens, appendix_body, exact_score)
        {
            let snippet = if kind == MatchKind::Body {
                snippet_around(
                    &entry.body,
                    tokens.first().map(String::as_str).unwrap_or(&normalized),
                )
            } else {
                first_paragraphs(&entry.body, 500)
            };
            candidates.push(Candidate {
                score,
                order,
                hit: SearchHit {
                    source: entry.source,
                    term: entry.term.clone(),
                    id: entry.id.clone(),
                    snippet,
                    matched_in: kind,
                    score,
                    file: entry.file.clone(),
                    anchor: None,
                    redirect: false,
                    source_file: dictionary.source_file(&entry.file),
                    line: Some(entry.line),
                },
            });
        }
        order += 1;
    }

    if request.scope.allows(Source::Weights) {
        for section in &dictionary.weights {
            if let Some((score, kind)) = score_section(section, &normalized, &tokens, weights_body)
            {
                let body = format!("{}\n\n{}", section.title, section.body);
                let snippet = if kind == MatchKind::Body {
                    snippet_around(
                        &section.body,
                        tokens.first().map(String::as_str).unwrap_or(&normalized),
                    )
                } else {
                    first_paragraphs(&body, 500)
                };
                candidates.push(Candidate {
                    score,
                    order,
                    hit: SearchHit {
                        source: Source::Weights,
                        term: section.title.clone(),
                        id: Some(section.number.clone()),
                        snippet,
                        matched_in: kind,
                        score,
                        file: section.file.clone(),
                        anchor: None,
                        redirect: false,
                        source_file: dictionary.source_file(&section.file),
                        line: None,
                    },
                });
            }
            order += 1;
        }
    }

    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.order.cmp(&right.order))
    });
    let weak_results = candidates
        .first()
        .is_none_or(|candidate| candidate.score < 300);

    let mut hits = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for candidate in candidates {
        let key = (
            candidate.hit.source,
            candidate.hit.term.clone(),
            candidate.hit.snippet.clone(),
        );
        if seen.insert(key) {
            hits.push(candidate.hit);
        }
    }

    let total = hits.len();
    hits.truncate(request.limit);

    let suggestions = if weak_results {
        let mut values = fuzzy::term_suggestions(dictionary, &normalized, request.scope, 3);
        for suggestion in fuzzy::pinyin_suggestions(dictionary, &normalized, 3) {
            if values.len() >= 5 {
                break;
            }
            if !values
                .iter()
                .any(|value| value.text.eq_ignore_ascii_case(&suggestion.text))
            {
                values.push(suggestion);
            }
        }
        values
    } else {
        Vec::new()
    };

    Ok(SearchResponse {
        query: query.to_owned(),
        scope: request.scope,
        hits,
        total,
        suggestions,
    })
}

pub fn entry(
    dictionary: &Dictionary,
    request: &EntryRequest,
) -> Result<EntryResponse, DictionaryError> {
    let term = request.term.trim();
    let normalized = normalize(term);
    if normalized.is_empty() {
        return Err(DictionaryError::new(
            "The \"term\" argument is empty after normalization.",
            crate::error::DictionaryErrorCode::InvalidRequest,
        ));
    }

    if request.source.allows(Source::Dictionary) {
        if let Some(index) = find_entry_by_anchor(dictionary, term) {
            return Ok(dictionary_response(dictionary, index, request));
        }
        if let Some(index) = dictionary
            .by_headword
            .get(&normalized)
            .and_then(|indices| indices.first())
        {
            return Ok(dictionary_response(dictionary, *index, request));
        }
        if let Some(index) = find_entry_by_lookup(dictionary, term) {
            return Ok(dictionary_response(dictionary, index, request));
        }
        if let Some(index) = find_entry(dictionary, &normalized) {
            return Ok(dictionary_response(dictionary, index, request));
        }
    }

    if let Some(reference) = find_appendix_ref_by_lookup(dictionary, term, request.source)
        .or_else(|| find_appendix_ref(dictionary, &normalized, request.source))
    {
        return Ok(appendix_response(dictionary, reference, request));
    }

    if request.source.allows(Source::Weights)
        && let Some(section) = find_weights_section(dictionary, &normalized)
    {
        return Ok(weights_response(dictionary, section, request));
    }

    let suggestions = suggestions(dictionary, &normalized, request.source);
    let mut error = DictionaryError::not_found(
        format!("No entry matches {term:?}."),
        suggestions.iter().map(|value| value.text.clone()).collect(),
    );
    error.suggestion_terms = suggestions.into_iter().map(|value| value.term).collect();
    Err(error)
}

/// Resolve a citation anchor such as `a1ALP` to its exact entry.
///
/// An anchor is only five characters and starts with `a1`; checking that shape
/// avoids scanning the entry list for ordinary search terms.
fn find_entry_by_anchor(dictionary: &Dictionary, term: &str) -> Option<usize> {
    if term.len() != 5
        || !term
            .get(..2)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("a1"))
    {
        return None;
    }
    dictionary.entries.iter().position(|entry| {
        entry
            .anchor
            .as_deref()
            .is_some_and(|anchor| anchor.eq_ignore_ascii_case(term))
    })
}

/// Resolve a pinyin or Han query through the generated lookup indices.
///
/// Tone-numbered and tone-marked pinyin, and Chinese characters, are specific
/// enough to answer from the index alone. A toneless pinyin query still gets
/// the exact entries first, but ordinary text search continues below them.
fn exact_lookup(dictionary: &Dictionary, query: &str, scope: Scope) -> Option<ExactLookup> {
    let han = pinyin::han_key(query);
    let mut matches = Vec::new();
    let shortcut;

    if !han.is_empty() {
        add_lookup_matches(&mut matches, dictionary.han.get(&han), 1000);
        shortcut = true;
    } else if let Some(toned) = pinyin::toned_key(query) {
        let toned_refs = dictionary.pinyin_toned.get(&toned);
        let had_toned = toned_refs.is_some_and(|references| !references.is_empty());
        shortcut = had_toned;
        add_lookup_matches(&mut matches, toned_refs, 1000);
        if !had_toned {
            // Only fall back to the toneless key when the tone-numbered form
            // is absent, so `wei2` does not also return `wei4` entries.
            add_lookup_matches(
                &mut matches,
                dictionary.pinyin_toneless.get(&pinyin::toneless_key(query)),
                950,
            );
        }
    } else {
        let toneless = pinyin::toneless_key(query);
        add_lookup_matches(&mut matches, dictionary.pinyin_toneless.get(&toneless), 950);
        shortcut = false;
    }

    if matches.is_empty() {
        return None;
    }

    let mut best: HashMap<LookupRef, u32> = HashMap::new();
    for (target, base) in matches {
        if !scope.allows(target.source) {
            continue;
        }
        let score = lookup_score(dictionary, target, base, query);
        best.entry(target)
            .and_modify(|current| *current = (*current).max(score))
            .or_insert(score);
    }
    if best.is_empty() {
        return None;
    }

    let mut targets: Vec<(LookupRef, u32)> = best.into_iter().collect();
    targets.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| lookup_order(left.0).cmp(&lookup_order(right.0)))
    });

    let mut entry_scores = HashMap::new();
    let mut appendix_scores = HashMap::new();
    for (target, score) in &targets {
        match target.source {
            Source::Dictionary => {
                entry_scores.insert(target.index, *score);
            }
            source => {
                appendix_scores.insert(
                    AppendixRef {
                        source,
                        index: target.index,
                    },
                    *score,
                );
            }
        }
    }

    Some(ExactLookup {
        shortcut,
        entry_scores,
        appendix_scores,
        targets,
    })
}

fn add_lookup_matches(
    matches: &mut Vec<(LookupRef, u32)>,
    references: Option<&Vec<LookupRef>>,
    score: u32,
) {
    if let Some(references) = references {
        matches.extend(references.iter().map(|target| (*target, score)));
    }
}

/// Prefer a pinyin-like headword over a redirect when a toneless query is
/// ambiguous (`qi` can mean qì or qí).
fn lookup_score(dictionary: &Dictionary, target: LookupRef, base: u32, query: &str) -> u32 {
    let mut score = base;
    if target.source == Source::Dictionary
        && let Some(entry) = dictionary.entries.get(target.index)
    {
        let root = pinyin::toneless_key(query);
        if !root.is_empty() && entry.norm_headword.starts_with(&root) {
            score += 40;
        }
        if is_redirect(&entry.body) {
            score = score.saturating_sub(10);
        }
    }
    score
}

fn lookup_order(target: LookupRef) -> (u8, usize) {
    let source_rank = match target.source {
        Source::Dictionary => 0,
        Source::Meridian => 1,
        Source::Medicinal => 2,
        Source::Formulas => 3,
        Source::Weights => 4,
    };
    (source_rank, target.index)
}

fn lookup_hit(
    dictionary: &Dictionary,
    target: LookupRef,
    score: u32,
    normalized_query: &str,
) -> Option<SearchHit> {
    match target.source {
        Source::Dictionary => {
            let entry = dictionary.entries.get(target.index)?;
            Some(SearchHit {
                source: Source::Dictionary,
                term: entry.headword.clone(),
                id: None,
                snippet: first_paragraphs(&entry.body, 500),
                matched_in: if entry.norm_headword == normalized_query {
                    MatchKind::Headword
                } else {
                    MatchKind::Alias
                },
                score,
                file: entry.file.clone(),
                anchor: entry.anchor.clone(),
                redirect: is_redirect(&entry.body),
                source_file: dictionary.source_file(&entry.file),
                line: Some(entry.line),
            })
        }
        source => {
            let entry = dictionary.appendix_entry(AppendixRef {
                source,
                index: target.index,
            })?;
            Some(SearchHit {
                source: entry.source,
                term: entry.term.clone(),
                id: entry.id.clone(),
                snippet: first_paragraphs(&entry.body, 500),
                matched_in: MatchKind::Alias,
                score,
                file: entry.file.clone(),
                anchor: None,
                redirect: false,
                source_file: dictionary.source_file(&entry.file),
                line: Some(entry.line),
            })
        }
    }
}

fn find_entry_by_lookup(dictionary: &Dictionary, term: &str) -> Option<usize> {
    exact_lookup(dictionary, term, Scope::Dictionary)?
        .targets
        .iter()
        .find_map(|(target, _)| (target.source == Source::Dictionary).then_some(target.index))
}

fn find_appendix_ref_by_lookup(
    dictionary: &Dictionary,
    term: &str,
    scope: Scope,
) -> Option<AppendixRef> {
    exact_lookup(dictionary, term, scope)?
        .targets
        .iter()
        .find_map(|(target, _)| match target.source {
            Source::Dictionary => None,
            source => Some(AppendixRef {
                source,
                index: target.index,
            }),
        })
}

fn find_entry(dictionary: &Dictionary, normalized: &str) -> Option<usize> {
    if let Some(indices) = dictionary.by_headword.get(normalized)
        && let Some(index) = indices.first()
    {
        return Some(*index);
    }

    for (index, entry) in dictionary.entries.iter().enumerate() {
        if alias_matches(entry, normalized, AliasMatch::Exact) {
            return Some(index);
        }
    }
    for (index, entry) in dictionary.entries.iter().enumerate() {
        if entry.norm_headword.starts_with(normalized)
            || alias_matches(entry, normalized, AliasMatch::Prefix)
        {
            return Some(index);
        }
    }
    for (index, entry) in dictionary.entries.iter().enumerate() {
        if entry.norm_headword.contains(normalized)
            || alias_matches(entry, normalized, AliasMatch::Contains)
        {
            return Some(index);
        }
    }
    None
}

fn find_appendix_ref(
    dictionary: &Dictionary,
    normalized: &str,
    scope: Scope,
) -> Option<AppendixRef> {
    if let Some(reference) = dictionary.appendix_by_id.get(normalized)
        && scope.allows(reference.source)
    {
        return Some(*reference);
    }
    if let Some(references) = dictionary.appendix_by_alias.get(normalized)
        && let Some(reference) = references
            .iter()
            .copied()
            .find(|reference| scope.allows(reference.source))
    {
        return Some(reference);
    }
    if let Some(references) = dictionary.appendix_by_term.get(normalized)
        && let Some(reference) = references
            .iter()
            .copied()
            .find(|reference| scope.allows(reference.source))
    {
        return Some(reference);
    }

    for reference in dictionary.appendix_entries(scope) {
        let Some(entry) = dictionary.appendix_entry(reference) else {
            continue;
        };
        if entry
            .id
            .as_ref()
            .is_some_and(|id| normalize(id).starts_with(normalized))
            || normalize(&entry.term).starts_with(normalized)
            || entry
                .norm_terms
                .iter()
                .any(|alias| alias.starts_with(normalized))
        {
            return Some(reference);
        }
    }
    for reference in dictionary.appendix_entries(scope) {
        let Some(entry) = dictionary.appendix_entry(reference) else {
            continue;
        };
        if entry
            .id
            .as_ref()
            .is_some_and(|id| normalize(id).contains(normalized))
            || normalize(&entry.term).contains(normalized)
            || entry
                .norm_terms
                .iter()
                .any(|alias| alias.contains(normalized))
        {
            return Some(reference);
        }
    }
    None
}

fn find_weights_section(dictionary: &Dictionary, normalized: &str) -> Option<usize> {
    dictionary
        .weights
        .iter()
        .position(|section| {
            normalize(&section.number) == normalized || normalize(&section.title) == normalized
        })
        .or_else(|| {
            dictionary.weights.iter().position(|section| {
                normalize(&section.title).starts_with(normalized)
                    || normalize(&section.title).contains(normalized)
            })
        })
        .or_else(|| {
            dictionary
                .weights
                .iter()
                .position(|section| normalize(&plain_text(&section.body)).contains(normalized))
        })
}

fn dictionary_response(
    dictionary: &Dictionary,
    index: usize,
    request: &EntryRequest,
) -> EntryResponse {
    let mut current = index;
    let mut redirected_from = None;
    let mut entry = &dictionary.entries[current];

    if is_redirect(&entry.body)
        && let Some(target) = entry.links.iter().find(|link| link.resolved)
        && let Some(target_term) = &target.target_term
        && let Some(indices) = dictionary.by_headword.get(&normalize(target_term))
        && let Some(target_index) = indices.first()
        && *target_index != current
    {
        redirected_from = Some(entry.headword.clone());
        current = *target_index;
        entry = &dictionary.entries[current];
    }

    let alias = entry.aliases.first().cloned().unwrap_or_default();
    let content = plain_text(&entry.body);
    let (content, truncated, total_chars) = truncate(content, request.max_chars);

    EntryResponse {
        source: Source::Dictionary,
        term: entry.headword.clone(),
        id: None,
        aliases: EntryAliases {
            simplified: alias.simplified,
            traditional: alias.traditional,
            pinyin_toned: alias.pinyin_toned,
            pinyin_ascii: alias.pinyin_ascii,
            english: entry.headword.clone(),
        },
        content,
        cross_references: entry.links.clone(),
        related: related_points(dictionary, &entry.body, CROSS_REFERENCE_LIMIT),
        redirected_from,
        citation: citation(dictionary, entry),
        source_file: dictionary.source_file(&entry.file),
        line: Some(entry.line),
        truncated,
        total_chars,
    }
}

fn appendix_response(
    dictionary: &Dictionary,
    reference: AppendixRef,
    request: &EntryRequest,
) -> EntryResponse {
    let Some(entry) = dictionary.appendix_entry(reference) else {
        return EntryResponse {
            source: reference.source,
            term: String::new(),
            id: None,
            aliases: EntryAliases::default(),
            content: String::new(),
            cross_references: Vec::new(),
            related: Vec::new(),
            redirected_from: None,
            citation: String::new(),
            source_file: None,
            line: None,
            truncated: false,
            total_chars: 0,
        };
    };

    let content = plain_text(&entry.body);
    let (content, truncated, total_chars) = truncate(content, request.max_chars);
    let mut citation = format!("{}, {}", dictionary.title(), entry.file);
    if let Some(id) = &entry.id {
        citation.push(':');
        citation.push_str(id);
    }

    EntryResponse {
        source: entry.source,
        term: entry.term.clone(),
        id: entry.id.clone(),
        aliases: appendix_aliases(entry),
        content,
        cross_references: Vec::new(),
        related: Vec::new(),
        redirected_from: None,
        citation,
        source_file: dictionary.source_file(&entry.file),
        line: Some(entry.line),
        truncated,
        total_chars,
    }
}

fn weights_response(
    dictionary: &Dictionary,
    index: usize,
    request: &EntryRequest,
) -> EntryResponse {
    let section = &dictionary.weights[index];
    let content = format!("{}\n\n{}", section.title, plain_text(&section.body));
    let (content, truncated, total_chars) = truncate(content, request.max_chars);

    EntryResponse {
        source: Source::Weights,
        term: section.title.clone(),
        id: Some(section.number.clone()),
        aliases: EntryAliases::default(),
        content,
        cross_references: Vec::new(),
        related: Vec::new(),
        redirected_from: None,
        citation: format!(
            "{}, {}:{}",
            dictionary.title(),
            section.file,
            section.number
        ),
        source_file: dictionary.source_file(&section.file),
        line: None,
        truncated,
        total_chars,
    }
}

fn score_entry(
    entry: &Entry,
    query: &str,
    tokens: &[String],
    body_allowed: bool,
    exact_alias_score: Option<u32>,
) -> Option<(u32, MatchKind)> {
    let mut best: Option<(u32, MatchKind)> = None;
    if entry.norm_headword == query {
        consider(&mut best, 1000, MatchKind::Headword);
    } else if entry.norm_headword.starts_with(query) {
        consider(&mut best, 800, MatchKind::Headword);
    } else if entry.norm_headword.contains(query) {
        consider(&mut best, 700, MatchKind::Headword);
    }

    if let Some(score) = exact_alias_score {
        consider(&mut best, score, MatchKind::Alias);
    }

    for alias in &entry.aliases {
        for field in [
            &alias.simplified,
            &alias.traditional,
            &alias.pinyin_toned,
            &alias.pinyin_ascii,
        ] {
            let field = normalize(field);
            if field.is_empty() {
                continue;
            }
            if field == query {
                consider(&mut best, 900, MatchKind::Alias);
            } else if field.starts_with(query) {
                consider(&mut best, 750, MatchKind::Alias);
            } else if field.contains(query) {
                consider(&mut best, 650, MatchKind::Alias);
            }
        }
    }

    let names = entry
        .aliases
        .iter()
        .flat_map(|alias| {
            [
                normalize(&alias.simplified),
                normalize(&alias.traditional),
                normalize(&alias.pinyin_toned),
                normalize(&alias.pinyin_ascii),
            ]
        })
        .collect::<Vec<_>>()
        .join(" ");
    let name_text = format!("{} {names}", entry.norm_headword);
    let name_coverage = coverage(&name_text, tokens);
    if name_coverage >= 1.0 {
        consider(&mut best, 600, MatchKind::Headword);
    } else if name_coverage > 0.0 {
        consider(
            &mut best,
            (100.0 * name_coverage) as u32,
            MatchKind::Headword,
        );
    }

    if body_allowed {
        let count = entry.norm_body.matches(query).count();
        if count > 0 {
            consider(
                &mut best,
                500 + u32::try_from(count.min(20)).unwrap_or(20) * 5,
                MatchKind::Body,
            );
        } else {
            let body_coverage = coverage(&entry.norm_body, tokens);
            if body_coverage >= 1.0 {
                consider(&mut best, 400, MatchKind::Body);
            } else if body_coverage > 0.0 {
                consider(&mut best, (100.0 * body_coverage) as u32, MatchKind::Body);
            }
        }
    }

    best
}

fn score_appendix(
    entry: &crate::dictionary::model::AppendixEntry,
    query: &str,
    tokens: &[String],
    body_allowed: bool,
    exact_alias_score: Option<u32>,
) -> Option<(u32, MatchKind)> {
    let mut best: Option<(u32, MatchKind)> = None;

    if let Some(score) = exact_alias_score {
        consider(&mut best, score, MatchKind::Alias);
    }

    if let Some(id) = &entry.id {
        let id = normalize(id);
        if id == query {
            consider(&mut best, 1000, MatchKind::Id);
        } else if id.starts_with(query) {
            consider(&mut best, 900, MatchKind::Id);
        } else if id.contains(query) {
            consider(&mut best, 800, MatchKind::Id);
        }
    }

    let term = normalize(&entry.term);
    if term == query {
        consider(&mut best, 950, MatchKind::Headword);
    } else if term.starts_with(query) {
        consider(&mut best, 850, MatchKind::Headword);
    } else if term.contains(query) {
        consider(&mut best, 750, MatchKind::Headword);
    }

    for alias in &entry.norm_terms {
        if alias == query {
            consider(&mut best, 900, MatchKind::Alias);
        } else if alias.starts_with(query) {
            consider(&mut best, 800, MatchKind::Alias);
        } else if alias.contains(query) {
            consider(&mut best, 700, MatchKind::Alias);
        }
    }

    if body_allowed {
        let count = entry.norm_body.matches(query).count();
        if count > 0 {
            consider(
                &mut best,
                500 + u32::try_from(count.min(20)).unwrap_or(20) * 5,
                MatchKind::Body,
            );
        } else {
            let body_coverage = coverage(&entry.norm_body, tokens);
            if body_coverage >= 1.0 {
                consider(&mut best, 400, MatchKind::Body);
            } else if body_coverage > 0.0 {
                consider(&mut best, (100.0 * body_coverage) as u32, MatchKind::Body);
            }
        }
    }

    best
}

fn score_section(
    section: &crate::dictionary::model::Section,
    query: &str,
    tokens: &[String],
    body_allowed: bool,
) -> Option<(u32, MatchKind)> {
    let mut best: Option<(u32, MatchKind)> = None;
    if normalize(&section.number) == query {
        consider(&mut best, 1000, MatchKind::Id);
    }
    let title = normalize(&section.title);
    if title == query {
        consider(&mut best, 950, MatchKind::Headword);
    } else if title.starts_with(query) {
        consider(&mut best, 850, MatchKind::Headword);
    } else if title.contains(query) {
        consider(&mut best, 750, MatchKind::Headword);
    }
    if body_allowed {
        let body = normalize(&plain_text(&section.body));
        let count = body.matches(query).count();
        if count > 0 {
            consider(
                &mut best,
                500 + u32::try_from(count.min(20)).unwrap_or(20) * 5,
                MatchKind::Body,
            );
        } else {
            let body_coverage = coverage(&body, tokens);
            if body_coverage >= 1.0 {
                consider(&mut best, 400, MatchKind::Body);
            } else if body_coverage > 0.0 {
                consider(&mut best, (100.0 * body_coverage) as u32, MatchKind::Body);
            }
        }
    }
    best
}

fn consider(best: &mut Option<(u32, MatchKind)>, score: u32, kind: MatchKind) {
    if best.is_none_or(|(current, _)| score > current) {
        *best = Some((score, kind));
    }
}

#[derive(Debug, Clone, Copy)]
enum AliasMatch {
    Exact,
    Prefix,
    Contains,
}

fn alias_matches(entry: &Entry, query: &str, mode: AliasMatch) -> bool {
    entry.aliases.iter().any(|alias| {
        [
            &alias.simplified,
            &alias.traditional,
            &alias.pinyin_toned,
            &alias.pinyin_ascii,
        ]
        .iter()
        .map(|field| normalize(field))
        .any(|field| match mode {
            AliasMatch::Exact => field == query,
            AliasMatch::Prefix => field.starts_with(query),
            AliasMatch::Contains => field.contains(query),
        })
    })
}

fn coverage(haystack: &str, tokens: &[String]) -> f32 {
    if tokens.is_empty() {
        return 0.0;
    }
    let hits = tokens
        .iter()
        .filter(|token| haystack.contains(token.as_str()))
        .count();
    hits as f32 / tokens.len() as f32
}

fn tokenize(query: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in query.chars() {
        if character.is_alphanumeric() || character == '-' || character == '.' {
            for lower in character.to_lowercase() {
                current.push(lower);
            }
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens = tokens
        .into_iter()
        .map(|token| token.trim_matches(['-', '.']).to_owned())
        .filter(|token| !token.is_empty())
        .collect();
    tokens.sort();
    tokens.dedup();
    tokens
}

fn is_measurement_query(query: &str, tokens: &[String]) -> bool {
    const UNITS: &[&str] = &[
        "liang",
        "cun",
        "fen",
        "sheng",
        "dou",
        "tael",
        "dosage",
        "dose",
        "weights",
        "measures",
        "conversion",
    ];
    tokens.iter().any(|token| UNITS.contains(&token.as_str()))
        || ["两", "寸", "分", "升", "斗"]
            .iter()
            .any(|unit| query.contains(unit))
}

fn first_paragraphs(body: &str, max: usize) -> String {
    let mut paragraphs = Vec::new();
    for paragraph in body.split("\n\n") {
        let paragraph = collapse_whitespace(&plain_text(paragraph));
        if paragraph.is_empty() {
            continue;
        }
        paragraphs.push(paragraph);
        if paragraphs.len() == 2 {
            break;
        }
    }
    truncate_chars(&paragraphs.join(" "), max)
}

fn snippet_around(body: &str, needle: &str) -> String {
    let text = collapse_whitespace(&plain_text(body));
    let lower = text.to_lowercase();
    let needle_lower = needle.to_lowercase();
    let position = lower
        .find(needle_lower.as_str())
        .or_else(|| {
            needle_lower
                .split_whitespace()
                .next()
                .and_then(|token| lower.find(token))
        })
        .unwrap_or(0);
    let characters: Vec<char> = text.chars().collect();
    let position_chars = text[..position].chars().count();
    let needle_chars = needle.chars().count();
    let start = position_chars.saturating_sub(250);
    let end = (position_chars + needle_chars + 250).min(characters.len());
    let mut snippet: String = characters[start..end].iter().collect();
    if start > 0 {
        snippet.insert(0, '…');
    }
    if end < characters.len() {
        snippet.push('…');
    }
    snippet
}

fn truncate(text: String, max: usize) -> (String, bool, usize) {
    let total = text.chars().count();
    if total <= max {
        (text, false, total)
    } else {
        (text.chars().take(max).collect(), true, total)
    }
}

fn truncate_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn is_redirect(body: &str) -> bool {
    body.trim_start().to_ascii_lowercase().starts_with("see ")
}

fn citation(dictionary: &Dictionary, entry: &Entry) -> String {
    let mut citation = format!("{}, {}", dictionary.title(), entry.file);
    if let Some(anchor) = &entry.anchor {
        citation.push('#');
        citation.push_str(anchor);
    } else {
        citation.push(':');
        citation.push_str(&entry.line.to_string());
    }
    citation
}

fn appendix_aliases(entry: &crate::dictionary::model::AppendixEntry) -> EntryAliases {
    EntryAliases {
        simplified: entry.simplified.clone(),
        traditional: entry.traditional.clone(),
        pinyin_toned: entry.pinyin_toned.clone(),
        pinyin_ascii: entry.pinyin_ascii.clone(),
        english: entry.term.clone(),
    }
}

fn related_points(dictionary: &Dictionary, body: &str, max: usize) -> Vec<RelatedEntry> {
    let mut related = Vec::new();
    for point in &dictionary.meridians {
        let Some(id) = &point.id else {
            continue;
        };
        if !mentions(body, id) {
            continue;
        }
        related.push(RelatedEntry {
            source: Source::Meridian,
            id: Some(id.clone()),
            term: point.term.clone(),
            chinese: point
                .aliases
                .iter()
                .find(|alias| contains_han(alias))
                .cloned()
                .unwrap_or_default(),
            summary: first_paragraphs(&point.body, 200),
        });
        if related.len() >= max {
            break;
        }
    }
    related
}

fn mentions(body: &str, id: &str) -> bool {
    body.match_indices(id).any(|(index, _)| {
        let before = body[..index].chars().next_back();
        let after = body[index + id.len()..].chars().next();
        !before.is_some_and(is_identifier_character) && !after.is_some_and(is_identifier_character)
    })
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '-'
}

fn suggestions(
    dictionary: &Dictionary,
    query: &str,
    scope: Scope,
) -> Vec<crate::dictionary::Suggestion> {
    let mut values = Vec::new();
    if scope.allows(Source::Dictionary) {
        for entry in &dictionary.entries {
            if entry.norm_headword.starts_with(query) || entry.norm_headword.contains(query) {
                values.push((entry.norm_headword.len(), entry.headword.clone()));
            }
        }
    }
    for reference in dictionary.appendix_entries(scope) {
        if let Some(entry) = dictionary.appendix_entry(reference) {
            let term = normalize(&entry.term);
            if term.starts_with(query) || term.contains(query) {
                values.push((term.len(), entry.term.clone()));
            }
        }
    }
    if scope.allows(Source::Weights) {
        for section in &dictionary.weights {
            let title = normalize(&section.title);
            if title.starts_with(query) || title.contains(query) {
                values.push((title.len(), section.title.clone()));
            }
        }
    }
    values.sort();
    values.dedup_by(|left, right| left.1 == right.1);
    let mut results: Vec<crate::dictionary::Suggestion> = values
        .into_iter()
        .take(5)
        .map(|(_, value)| crate::dictionary::Suggestion {
            text: value.clone(),
            term: value,
            page: None,
        })
        .collect();

    for suggestion in fuzzy::term_suggestions(dictionary, query, scope, 3) {
        if results.len() >= 8 {
            break;
        }
        if !results
            .iter()
            .any(|value| value.text.eq_ignore_ascii_case(&suggestion.text))
        {
            results.push(suggestion);
        }
    }
    for suggestion in fuzzy::pinyin_suggestions(dictionary, query, 3) {
        if results.len() >= 8 {
            break;
        }
        if !results
            .iter()
            .any(|value| value.text.eq_ignore_ascii_case(&suggestion.text))
        {
            results.push(suggestion);
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::Dictionary;
    use crate::dictionary::model::Section;

    #[test]
    fn weights_cite_the_discovered_file_name() {
        let mut dictionary = Dictionary::empty();
        dictionary.weights = vec![Section {
            file: "37-appendix-4.md".to_owned(),
            number: "21".to_owned(),
            title: "Weights in Different Dynasties".to_owned(),
            body: "| Dynasty | Grams/jin |\n|---|---|\n| Zhou | 14.18 |".to_owned(),
        }];

        let entry = dictionary
            .entry(&EntryRequest {
                term: "jin".to_owned(),
                source: Scope::Weights,
                max_chars: 20_000,
            })
            .expect("the weights section resolves");
        assert_eq!(entry.citation, "Dictionary, 37-appendix-4.md:21");

        let hits = dictionary
            .search(&SearchRequest {
                query: "jin".to_owned(),
                scope: Scope::Weights,
                limit: 5,
            })
            .expect("the weights section is searchable");
        assert_eq!(hits.hits[0].file, "37-appendix-4.md");
    }

    #[test]
    fn tokenizer_keeps_point_ids_and_drops_punctuation() {
        assert_eq!(tokenize("aa-1"), vec!["aa-1"]);
        assert_eq!(tokenize("Jia, Three Li."), vec!["jia", "li", "three"]);
    }

    #[test]
    fn snippets_stay_within_the_limit() {
        let body = "one two three four five six seven eight nine ten";
        let snippet = snippet_around(body, "five");
        assert!(snippet.contains("five"));
        assert!(snippet.chars().count() <= body.chars().count() + 2);
    }

    #[test]
    fn empty_dictionary_searches_cleanly() {
        let dictionary = Dictionary::empty();
        let response = search(
            &dictionary,
            &SearchRequest {
                query: "alpha swelling".to_owned(),
                scope: Scope::All,
                limit: 5,
            },
        )
        .unwrap();
        assert!(response.hits.is_empty());
        assert_eq!(response.total, 0);
    }
}
