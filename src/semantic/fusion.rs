//! Lexical gating, reciprocal-rank fusion, and semantic hit construction.

use std::collections::{HashMap, HashSet};

use crate::dictionary::normalize::contains_han;
use crate::dictionary::pinyin;
use crate::dictionary::search::{first_paragraphs, search_all};
use crate::dictionary::{
    AppendixRef, Dictionary, LookupRef, MatchKind, Scope, SearchHit, SearchRequest, SearchResponse,
    Source,
};
use crate::error::DictionaryError;

use super::embedder::Embedder;
use super::index::{SemanticIndex, SemanticMatch};
use super::{HYBRID_GATE_SCORE, MIN_SIMILARITY, RRF_K, SEMANTIC_TOP_K};

/// Tuning for one hybrid query.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HybridOptions {
    pub min_similarity: f32,
    pub top_k: usize,
}

impl Default for HybridOptions {
    fn default() -> Self {
        Self {
            min_similarity: MIN_SIMILARITY,
            top_k: SEMANTIC_TOP_K,
        }
    }
}

/// Diagnostics from one gated search.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HybridTrace {
    pub gate_open: bool,
    pub embedding_ms: u128,
    pub semantic_candidates: usize,
    pub fallback: Option<String>,
}

/// Run lexical search and, only when it is weak, fuse semantic candidates.
pub fn search_hybrid(
    dictionary: &Dictionary,
    request: &SearchRequest,
    embedder: &dyn Embedder,
    index: &SemanticIndex,
    options: HybridOptions,
) -> Result<SearchResponse, DictionaryError> {
    Ok(search_hybrid_traced(dictionary, request, embedder, index, options)?.0)
}

/// The same search with a record of the gate, timing, candidates, and fallback.
pub fn search_hybrid_traced(
    dictionary: &Dictionary,
    request: &SearchRequest,
    embedder: &dyn Embedder,
    index: &SemanticIndex,
    options: HybridOptions,
) -> Result<(SearchResponse, HybridTrace), DictionaryError> {
    let mut trace = HybridTrace::default();
    // Fusion needs every lexical candidate, not only the response limit.
    // `search_all` ranks once, so the gate and fusion share the same work.
    let mut lexical = search_all(dictionary, request)?;
    if !should_embed(dictionary, request, &lexical) {
        lexical.hits.truncate(request.limit);
        trace.fallback = Some("lexical gate".to_owned());
        return Ok((lexical, trace));
    }
    trace.gate_open = true;

    let started = std::time::Instant::now();
    let query_vector = match embedder.embed_query(request.query.trim()) {
        Ok(vector) => vector,
        Err(error) => {
            trace.embedding_ms = started.elapsed().as_millis();
            lexical.hits.truncate(request.limit);
            trace.fallback = Some(format!("embedding failed: {error}"));
            return Ok((lexical, trace));
        }
    };
    trace.embedding_ms = started.elapsed().as_millis();

    let semantic = if request.scope == Scope::All {
        index.search(&query_vector, options.top_k, options.min_similarity)
    } else {
        index.search_scoped(
            &query_vector,
            options.top_k,
            options.min_similarity,
            request.scope,
        )
    };
    let semantic = match semantic {
        Ok(semantic) => semantic,
        Err(error) => {
            lexical.hits.truncate(request.limit);
            trace.fallback = Some(format!("index search failed: {error}"));
            return Ok((lexical, trace));
        }
    };
    trace.semantic_candidates = semantic.len();
    if semantic.is_empty() {
        lexical.hits.truncate(request.limit);
        trace.fallback = Some("no candidate above threshold".to_owned());
        return Ok((lexical, trace));
    }

    let response = if is_appendix_scope(request.scope) {
        fuse_conservative(dictionary, lexical, &semantic, request.limit)
    } else {
        fuse(dictionary, lexical, &semantic, request.limit)
    };
    Ok((response, trace))
}

/// True when the lexical gate is open and the query is not pure pinyin.
pub fn should_embed(
    dictionary: &Dictionary,
    request: &SearchRequest,
    lexical: &SearchResponse,
) -> bool {
    let best = lexical.hits.first().map(|hit| hit.score).unwrap_or(0);
    if best >= HYBRID_GATE_SCORE {
        return false;
    }
    let query = request.query.trim();
    if query.is_empty() || is_pure_pinyin(dictionary, query) {
        return false;
    }
    true
}

fn is_pure_pinyin(dictionary: &Dictionary, query: &str) -> bool {
    if query.is_empty() || contains_han(query) {
        return false;
    }
    if pinyin::toned_key(query).is_some() {
        return true;
    }
    let toneless = pinyin::toneless_key(query);
    !toneless.is_empty() && dictionary.pinyin_toneless.contains_key(&toneless)
}

#[derive(Debug)]
struct Merged {
    reference: LookupRef,
    lexical_rank: Option<usize>,
    semantic_rank: Option<usize>,
    lexical_hit: Option<SearchHit>,
    similarity: Option<f32>,
}

impl Merged {
    fn rrf(&self) -> f64 {
        let lexical = self
            .lexical_rank
            .map(|rank| 1.0 / (RRF_K + rank as f64))
            .unwrap_or(0.0);
        let semantic = self
            .semantic_rank
            .map(|rank| 1.0 / (RRF_K + rank as f64))
            .unwrap_or(0.0);
        lexical + semantic
    }

    fn lexical_score(&self) -> u32 {
        self.lexical_hit.as_ref().map(|hit| hit.score).unwrap_or(0)
    }

    fn pinned(&self) -> bool {
        self.lexical_score() >= 600
    }
}

/// Merge lexical and semantic candidates with reciprocal-rank fusion.
pub fn fuse(
    dictionary: &Dictionary,
    lexical: SearchResponse,
    semantic: &[SemanticMatch],
    limit: usize,
) -> SearchResponse {
    let mut merged: HashMap<LookupRef, Merged> = HashMap::new();
    let mut unmapped: Vec<SearchHit> = Vec::new();
    let resolver = HitResolver::new(dictionary);

    for (rank, hit) in lexical.hits.iter().enumerate() {
        let Some(reference) = resolver.reference_for_hit(hit) else {
            unmapped.push(hit.clone());
            continue;
        };
        let entry = merged.entry(reference).or_insert_with(|| Merged {
            reference,
            lexical_rank: None,
            semantic_rank: None,
            lexical_hit: None,
            similarity: None,
        });
        if entry.lexical_rank.is_none() {
            entry.lexical_rank = Some(rank + 1);
            entry.lexical_hit = Some(hit.clone());
        }
    }

    for (rank, candidate) in semantic.iter().enumerate() {
        let entry = merged.entry(candidate.reference).or_insert_with(|| Merged {
            reference: candidate.reference,
            lexical_rank: None,
            semantic_rank: None,
            lexical_hit: None,
            similarity: None,
        });
        if entry.semantic_rank.is_none() {
            entry.semantic_rank = Some(rank + 1);
            entry.similarity = Some(candidate.similarity);
        }
    }

    let mut entries: Vec<Merged> = merged.into_values().collect();
    entries.sort_by(|left, right| {
        right
            .pinned()
            .cmp(&left.pinned())
            .then_with(|| right.rrf().total_cmp(&left.rrf()))
            .then_with(|| right.lexical_score().cmp(&left.lexical_score()))
            .then_with(|| {
                right
                    .similarity
                    .unwrap_or(0.0)
                    .total_cmp(&left.similarity.unwrap_or(0.0))
            })
            .then_with(|| lookup_order(left.reference).cmp(&lookup_order(right.reference)))
    });

    let total = entries.len() + unmapped.len();
    let mut hits = Vec::with_capacity(entries.len() + unmapped.len());
    for entry in entries {
        if let Some(hit) = entry.lexical_hit {
            hits.push(hit);
        } else if let Some(hit) = semantic_hit(
            dictionary,
            entry.reference,
            semantic_score(entry.similarity.unwrap_or(0.0)),
        ) {
            hits.push(hit);
        }
    }
    hits.extend(unmapped);
    hits.truncate(limit);

    SearchResponse {
        query: lexical.query,
        scope: lexical.scope,
        hits,
        total,
        suggestions: lexical.suggestions,
    }
}

fn is_appendix_scope(scope: Scope) -> bool {
    matches!(
        scope,
        Scope::Meridian | Scope::Medicinal | Scope::Formulas | Scope::Weights
    )
}

/// Keep the lexical head, then let semantic candidates fill the last slot.
///
/// Appendix entries are short and structured; scoped lexical ranking is
/// already strong. Reserving one slot for semantic retrieval prevents the
/// fusion from demoting correct lexical hits while still allowing a semantic
/// discovery to enter the response.
fn fuse_conservative(
    dictionary: &Dictionary,
    lexical: SearchResponse,
    semantic: &[SemanticMatch],
    limit: usize,
) -> SearchResponse {
    let SearchResponse {
        query,
        scope,
        hits: mut lexical_hits,
        total: lexical_total,
        suggestions,
    } = lexical;
    let resolver = HitResolver::new(dictionary);
    let lexical_refs: HashSet<LookupRef> = lexical_hits
        .iter()
        .filter_map(|hit| resolver.reference_for_hit(hit))
        .collect();
    let semantic_only_count = semantic
        .iter()
        .filter(|candidate| !lexical_refs.contains(&candidate.reference))
        .count();

    let mut lexical_ranks: HashMap<LookupRef, usize> = HashMap::new();
    for (rank, hit) in lexical_hits.iter().enumerate() {
        if let Some(reference) = resolver.reference_for_hit(hit) {
            lexical_ranks.entry(reference).or_insert(rank + 1);
        }
    }

    let pin = if limit == 0 {
        0
    } else {
        limit.saturating_sub(1).max(1)
    }
    .min(lexical_hits.len());
    let remaining = lexical_hits.split_off(pin);
    let mut hits = lexical_hits;
    let mut used: HashSet<LookupRef> = hits
        .iter()
        .filter_map(|hit| resolver.reference_for_hit(hit))
        .collect();

    if hits.len() < limit {
        let mut best: Option<(f64, f32, LookupRef)> = None;
        for (semantic_rank, candidate) in semantic.iter().enumerate() {
            if used.contains(&candidate.reference) {
                continue;
            }
            let lexical = lexical_ranks
                .get(&candidate.reference)
                .map(|rank| 1.0 / (RRF_K + *rank as f64))
                .unwrap_or(0.0);
            let semantic_score = 1.0 / (RRF_K + semantic_rank as f64 + 1.0);
            let score = lexical + semantic_score;
            let better = best.is_none_or(|(best_score, best_similarity, best_reference)| {
                score
                    .total_cmp(&best_score)
                    .then_with(|| candidate.similarity.total_cmp(&best_similarity))
                    .then_with(|| {
                        lookup_order(best_reference).cmp(&lookup_order(candidate.reference))
                    })
                    .is_gt()
            });
            if better {
                best = Some((score, candidate.similarity, candidate.reference));
            }
        }
        if let Some((_, similarity, reference)) = best
            && let Some(hit) = semantic_hit(dictionary, reference, semantic_score(similarity))
        {
            used.insert(reference);
            hits.push(hit);
        }
    }

    for hit in remaining {
        if hits.len() >= limit {
            break;
        }
        if let Some(reference) = resolver.reference_for_hit(&hit) {
            if used.contains(&reference) {
                continue;
            }
            used.insert(reference);
        }
        hits.push(hit);
    }

    SearchResponse {
        query,
        scope,
        hits,
        total: lexical_total + semantic_only_count,
        suggestions,
    }
}

fn semantic_score(similarity: f32) -> u32 {
    let score = 200.0 + 99.0 * similarity.clamp(0.0, 1.0);
    (score.round() as u32).min(299)
}

fn lookup_order(reference: LookupRef) -> (u8, usize) {
    let source_rank = match reference.source {
        Source::Dictionary => 0,
        Source::Meridian => 1,
        Source::Medicinal => 2,
        Source::Formulas => 3,
        Source::Weights => 4,
    };
    (source_rank, reference.index)
}

/// Hash lookups from a search hit back to the dictionary storage it came from.
///
/// A weak query can produce thousands of lexical candidates; scanning every
/// entry for each one dominates hybrid latency. The maps are built once per
/// fusion and preserve the previous first-match semantics.
struct HitResolver<'a> {
    entries_by_file_line: HashMap<(&'a str, usize), usize>,
    entries_by_term_file: HashMap<(&'a str, &'a str), usize>,
    appendix_by_file_line: HashMap<(Source, &'a str, usize), AppendixRef>,
    appendix_by_term_file: HashMap<(Source, &'a str, &'a str), AppendixRef>,
    weights_by_file_title: HashMap<(&'a str, &'a str), usize>,
    weights_by_file_number: HashMap<(&'a str, &'a str), usize>,
}

impl<'a> HitResolver<'a> {
    fn new(dictionary: &'a Dictionary) -> Self {
        let mut entries_by_file_line = HashMap::new();
        let mut entries_by_term_file = HashMap::new();
        for (index, entry) in dictionary.entries.iter().enumerate() {
            entries_by_file_line
                .entry((entry.file.as_str(), entry.line))
                .or_insert(index);
            entries_by_term_file
                .entry((entry.headword.as_str(), entry.file.as_str()))
                .or_insert(index);
        }

        let mut appendix_by_file_line = HashMap::new();
        let mut appendix_by_term_file = HashMap::new();
        for (source, entries) in [
            (Source::Meridian, &dictionary.meridians),
            (Source::Medicinal, &dictionary.medicinals),
            (Source::Formulas, &dictionary.formulas),
        ] {
            for (index, entry) in entries.iter().enumerate() {
                let reference = AppendixRef { source, index };
                appendix_by_file_line
                    .entry((source, entry.file.as_str(), entry.line))
                    .or_insert(reference);
                appendix_by_term_file
                    .entry((source, entry.term.as_str(), entry.file.as_str()))
                    .or_insert(reference);
            }
        }

        let mut weights_by_file_title = HashMap::new();
        let mut weights_by_file_number = HashMap::new();
        for (index, section) in dictionary.weights.iter().enumerate() {
            weights_by_file_title
                .entry((section.file.as_str(), section.title.as_str()))
                .or_insert(index);
            weights_by_file_number
                .entry((section.file.as_str(), section.number.as_str()))
                .or_insert(index);
        }

        Self {
            entries_by_file_line,
            entries_by_term_file,
            appendix_by_file_line,
            appendix_by_term_file,
            weights_by_file_title,
            weights_by_file_number,
        }
    }

    fn reference_for_hit(&self, hit: &SearchHit) -> Option<LookupRef> {
        match hit.source {
            Source::Dictionary => {
                let index = hit
                    .line
                    .and_then(|line| {
                        self.entries_by_file_line
                            .get(&(hit.file.as_str(), line))
                            .copied()
                    })
                    .or_else(|| {
                        self.entries_by_term_file
                            .get(&(hit.term.as_str(), hit.file.as_str()))
                            .copied()
                    })?;
                Some(LookupRef {
                    source: Source::Dictionary,
                    index,
                })
            }
            Source::Meridian | Source::Medicinal | Source::Formulas => {
                let reference = hit
                    .line
                    .and_then(|line| {
                        self.appendix_by_file_line
                            .get(&(hit.source, hit.file.as_str(), line))
                            .copied()
                    })
                    .or_else(|| {
                        self.appendix_by_term_file
                            .get(&(hit.source, hit.term.as_str(), hit.file.as_str()))
                            .copied()
                    })?;
                Some(LookupRef {
                    source: reference.source,
                    index: reference.index,
                })
            }
            Source::Weights => {
                let index = self
                    .weights_by_file_title
                    .get(&(hit.file.as_str(), hit.term.as_str()))
                    .copied()
                    .or_else(|| {
                        hit.id.as_deref().and_then(|number| {
                            self.weights_by_file_number
                                .get(&(hit.file.as_str(), number))
                                .copied()
                        })
                    })?;
                Some(LookupRef {
                    source: Source::Weights,
                    index,
                })
            }
        }
    }
}

fn semantic_hit(dictionary: &Dictionary, reference: LookupRef, score: u32) -> Option<SearchHit> {
    match reference.source {
        Source::Dictionary => {
            let entry = dictionary.entries.get(reference.index)?;
            Some(SearchHit {
                source: Source::Dictionary,
                term: entry.headword.clone(),
                id: None,
                snippet: first_paragraphs(&entry.body, 500),
                matched_in: MatchKind::Semantic,
                score,
                file: entry.file.clone(),
                anchor: entry.anchor.clone(),
                redirect: crate::dictionary::search::is_redirect(&entry.body),
                source_file: dictionary.source_file(&entry.file),
                line: Some(entry.line),
            })
        }
        Source::Meridian | Source::Medicinal | Source::Formulas => {
            let entry = dictionary.appendix_entry(AppendixRef {
                source: reference.source,
                index: reference.index,
            })?;
            Some(SearchHit {
                source: entry.source,
                term: entry.term.clone(),
                id: entry.id.clone(),
                snippet: first_paragraphs(&entry.body, 500),
                matched_in: MatchKind::Semantic,
                score,
                file: entry.file.clone(),
                anchor: None,
                redirect: false,
                source_file: dictionary.source_file(&entry.file),
                line: Some(entry.line),
            })
        }
        Source::Weights => {
            let section = dictionary.weights.get(reference.index)?;
            Some(SearchHit {
                source: Source::Weights,
                term: section.title.clone(),
                id: Some(section.number.clone()),
                snippet: first_paragraphs(&section.body, 500),
                matched_in: MatchKind::Semantic,
                score,
                file: section.file.clone(),
                anchor: None,
                redirect: false,
                source_file: dictionary.source_file(&section.file),
                line: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::{AppendixEntry, Entry, Scope, SearchRequest, Section, Suggestion};
    use crate::semantic::document::{DocumentProfile, build_documents};
    use crate::semantic::embedder::FakeEmbedder;
    use crate::semantic::index::{IndexMetadata, SemanticIndex};

    fn entry(headword: &str, line: usize, body: &str) -> Entry {
        Entry {
            headword: headword.to_owned(),
            norm_headword: headword.to_ascii_lowercase(),
            file: "08-a.md".to_owned(),
            line,
            anchor: None,
            aliases: Vec::new(),
            body: body.to_owned(),
            norm_body: body.to_ascii_lowercase(),
            links: Vec::new(),
        }
    }

    fn dictionary() -> Dictionary {
        let mut dictionary = Dictionary::empty();
        dictionary
            .entries
            .push(entry("alpha", 1, "A made-up first entry."));
        dictionary
            .entries
            .push(entry("beta", 2, "A made-up second entry."));
        dictionary
            .entries
            .push(entry("gamma", 3, "A made-up third entry."));
        for (index, entry) in dictionary.entries.iter().enumerate() {
            dictionary
                .by_headword
                .entry(entry.norm_headword.clone())
                .or_default()
                .push(index);
        }
        dictionary.weights.push(Section {
            file: "37-appendix-4-Weights.md".to_owned(),
            number: "1".to_owned(),
            title: "Length".to_owned(),
            body: "A made-up measure.".to_owned(),
        });
        dictionary
    }

    fn hit(source: Source, term: &str, line: usize, score: u32) -> SearchHit {
        SearchHit {
            source,
            term: term.to_owned(),
            id: None,
            snippet: String::new(),
            matched_in: MatchKind::Headword,
            score,
            file: if source == Source::Weights {
                "37-appendix-4-Weights.md".to_owned()
            } else {
                "08-a.md".to_owned()
            },
            anchor: None,
            redirect: false,
            source_file: None,
            line: (source != Source::Weights).then_some(line),
        }
    }

    fn lexical(hits: Vec<SearchHit>, total: usize) -> SearchResponse {
        SearchResponse {
            query: "query".to_owned(),
            scope: Scope::All,
            hits,
            total,
            suggestions: Vec::new(),
        }
    }

    fn reference(source: Source, index: usize) -> LookupRef {
        LookupRef { source, index }
    }

    #[test]
    fn rrf_lets_a_strong_semantic_rank_improve_a_weak_lexical_hit() {
        let dictionary = dictionary();
        let response = fuse(
            &dictionary,
            lexical(
                vec![
                    hit(Source::Dictionary, "alpha", 1, 100),
                    hit(Source::Dictionary, "beta", 2, 90),
                ],
                2,
            ),
            &[SemanticMatch {
                reference: reference(Source::Dictionary, 1),
                similarity: 0.9,
            }],
            10,
        );
        assert_eq!(response.hits[0].term, "beta");
        assert_eq!(response.hits[1].term, "alpha");
        assert_eq!(response.total, 2);
    }

    #[test]
    fn a_semantic_only_hit_is_labelled_and_scored_as_a_fallback() {
        let dictionary = dictionary();
        let response = fuse(
            &dictionary,
            lexical(vec![hit(Source::Dictionary, "alpha", 1, 100)], 1),
            &[SemanticMatch {
                reference: reference(Source::Dictionary, 2),
                similarity: 0.8,
            }],
            10,
        );
        let gamma = response
            .hits
            .iter()
            .find(|hit| hit.term == "gamma")
            .expect("the semantic-only hit is present");
        assert_eq!(gamma.matched_in, MatchKind::Semantic);
        assert!((200..=299).contains(&gamma.score));
        assert_eq!(response.total, 2);
    }

    #[test]
    fn a_hit_in_both_lists_is_deduplicated_and_keeps_lexical_provenance() {
        let dictionary = dictionary();
        let response = fuse(
            &dictionary,
            lexical(vec![hit(Source::Dictionary, "beta", 2, 120)], 1),
            &[SemanticMatch {
                reference: reference(Source::Dictionary, 1),
                similarity: 0.9,
            }],
            10,
        );
        assert_eq!(response.total, 1);
        assert_eq!(response.hits.len(), 1);
        assert_eq!(response.hits[0].term, "beta");
        assert_eq!(response.hits[0].matched_in, MatchKind::Headword);
        assert_eq!(response.hits[0].score, 120);
    }

    #[test]
    fn a_pinned_lexical_hit_stays_ahead_of_a_semantic_only_hit() {
        let dictionary = dictionary();
        let response = fuse(
            &dictionary,
            lexical(vec![hit(Source::Dictionary, "alpha", 1, 650)], 1),
            &[SemanticMatch {
                reference: reference(Source::Dictionary, 2),
                similarity: 0.99,
            }],
            10,
        );
        assert_eq!(response.hits[0].term, "alpha");
        assert_eq!(response.hits[0].score, 650);
    }

    #[test]
    fn fusion_respects_the_limit_and_weight_hits() {
        let dictionary = dictionary();
        let response = fuse(
            &dictionary,
            lexical(Vec::new(), 0),
            &[
                SemanticMatch {
                    reference: reference(Source::Weights, 0),
                    similarity: 0.5,
                },
                SemanticMatch {
                    reference: reference(Source::Dictionary, 0),
                    similarity: 0.4,
                },
            ],
            1,
        );
        assert_eq!(response.hits.len(), 1);
        assert_eq!(response.hits[0].source, Source::Weights);
        assert_eq!(response.hits[0].term, "Length");
        assert_eq!(response.hits[0].id.as_deref(), Some("1"));
        assert_eq!(response.total, 2);
    }

    #[test]
    fn appendix_hits_are_constructed_for_each_appendix_source() {
        let mut dictionary = dictionary();
        for (source, term, line) in [
            (Source::Meridian, "alpha point", 1),
            (Source::Medicinal, "alpha root", 2),
            (Source::Formulas, "alpha decoction", 3),
        ] {
            let entries = match source {
                Source::Meridian => &mut dictionary.meridians,
                Source::Medicinal => &mut dictionary.medicinals,
                Source::Formulas => &mut dictionary.formulas,
                _ => unreachable!(),
            };
            entries.push(AppendixEntry {
                source,
                file: "34-appendix-1-Meridian.md".to_owned(),
                line,
                id: Some("AA-1".to_owned()),
                section: "Test".to_owned(),
                subsection: None,
                term: term.to_owned(),
                simplified: String::new(),
                traditional: String::new(),
                pinyin_toned: String::new(),
                pinyin_ascii: String::new(),
                aliases: Vec::new(),
                body: "A made-up appendix entry.".to_owned(),
                norm_terms: Vec::new(),
                norm_body: "a made-up appendix entry.".to_owned(),
            });
        }
        let response = fuse(
            &dictionary,
            lexical(Vec::new(), 0),
            &[
                SemanticMatch {
                    reference: reference(Source::Meridian, 0),
                    similarity: 0.5,
                },
                SemanticMatch {
                    reference: reference(Source::Medicinal, 0),
                    similarity: 0.4,
                },
                SemanticMatch {
                    reference: reference(Source::Formulas, 0),
                    similarity: 0.3,
                },
            ],
            10,
        );
        assert_eq!(response.hits.len(), 3);
        assert!(
            response
                .hits
                .iter()
                .all(|hit| hit.matched_in == MatchKind::Semantic)
        );
        assert_eq!(response.hits[0].source, Source::Meridian);
        assert_eq!(response.hits[1].source, Source::Medicinal);
        assert_eq!(response.hits[2].source, Source::Formulas);
    }

    #[test]
    fn suggestions_survive_fusion() {
        let dictionary = dictionary();
        let mut lexical = lexical(vec![hit(Source::Dictionary, "alpha", 1, 100)], 1);
        lexical.suggestions.push(Suggestion {
            text: "alpha (alpha)".to_owned(),
            term: "alpha".to_owned(),
            page: None,
        });
        let response = fuse(
            &dictionary,
            lexical,
            &[SemanticMatch {
                reference: reference(Source::Dictionary, 2),
                similarity: 0.8,
            }],
            10,
        );
        assert_eq!(response.suggestions.len(), 1);
    }

    #[test]
    fn the_gate_opens_only_for_weak_non_pinyin_queries() {
        let mut dictionary = dictionary();
        let request = SearchRequest {
            query: "alpha".to_owned(),
            scope: Scope::All,
            limit: 10,
        };
        assert!(!should_embed(
            &dictionary,
            &request,
            &lexical(vec![hit(Source::Dictionary, "alpha", 1, 300)], 1)
        ));
        assert!(should_embed(
            &dictionary,
            &request,
            &lexical(vec![hit(Source::Dictionary, "alpha", 1, 299)], 1)
        ));
        assert!(should_embed(&dictionary, &request, &lexical(Vec::new(), 0)));
        let empty = SearchRequest {
            query: "   ".to_owned(),
            ..request.clone()
        };
        assert!(!should_embed(&dictionary, &empty, &lexical(Vec::new(), 0)));

        dictionary
            .pinyin_toneless
            .insert("qi xu".to_owned(), vec![reference(Source::Dictionary, 0)]);
        let pinyin_request = SearchRequest {
            query: "qi xu".to_owned(),
            ..request.clone()
        };
        assert!(!should_embed(
            &dictionary,
            &pinyin_request,
            &lexical(Vec::new(), 0)
        ));
        let toned = SearchRequest {
            query: "qi4 xu1".to_owned(),
            ..request
        };
        assert!(!should_embed(&dictionary, &toned, &lexical(Vec::new(), 0)));
    }

    #[test]
    fn hybrid_search_embeds_once_and_returns_semantic_hits() {
        let dictionary = dictionary();
        let embedder = FakeEmbedder::new(8);
        let documents = build_documents(&dictionary, &DocumentProfile::default());
        let texts = documents
            .iter()
            .map(|document| document.text.clone())
            .collect::<Vec<_>>();
        let vectors = embedder.embed_documents(&texts).unwrap();
        let index = SemanticIndex::build(
            IndexMetadata {
                package_version: "0.5.0".to_owned(),
                dictionary_fingerprint: "fingerprint".to_owned(),
                model_name: "fake".to_owned(),
                model_digest: "0123456789abcdef".to_owned(),
                dimensions: 8,
                profile_hash: "profile".to_owned(),
            },
            &documents,
            vectors,
        )
        .unwrap();
        let request = SearchRequest {
            query: "zzz".to_owned(),
            scope: Scope::All,
            limit: 10,
        };
        let response = search_hybrid(
            &dictionary,
            &request,
            &embedder,
            &index,
            HybridOptions {
                min_similarity: -1.0,
                top_k: 10,
            },
        )
        .unwrap();
        assert_eq!(embedder.query_calls(), 1);
        assert!(!response.hits.is_empty());
        assert!(
            response
                .hits
                .iter()
                .all(|hit| hit.matched_in == MatchKind::Semantic)
        );
    }

    #[test]
    fn hybrid_search_does_not_embed_a_strong_lexical_query() {
        let dictionary = dictionary();
        let embedder = FakeEmbedder::new(8);
        let documents = build_documents(&dictionary, &DocumentProfile::default());
        let texts = documents
            .iter()
            .map(|document| document.text.clone())
            .collect::<Vec<_>>();
        let vectors = embedder.embed_documents(&texts).unwrap();
        let index = SemanticIndex::build(
            IndexMetadata {
                package_version: "0.5.0".to_owned(),
                dictionary_fingerprint: "fingerprint".to_owned(),
                model_name: "fake".to_owned(),
                model_digest: "0123456789abcdef".to_owned(),
                dimensions: 8,
                profile_hash: "profile".to_owned(),
            },
            &documents,
            vectors,
        )
        .unwrap();
        let request = SearchRequest {
            query: "alpha".to_owned(),
            scope: Scope::All,
            limit: 10,
        };
        let response = search_hybrid(
            &dictionary,
            &request,
            &embedder,
            &index,
            HybridOptions::default(),
        )
        .unwrap();
        assert_eq!(embedder.query_calls(), 0);
        assert_eq!(response.hits[0].matched_in, MatchKind::Headword);
    }

    #[test]
    fn appendix_scopes_use_the_conservative_policy() {
        assert!(is_appendix_scope(Scope::Meridian));
        assert!(is_appendix_scope(Scope::Medicinal));
        assert!(is_appendix_scope(Scope::Formulas));
        assert!(is_appendix_scope(Scope::Weights));
        assert!(!is_appendix_scope(Scope::All));
        assert!(!is_appendix_scope(Scope::Dictionary));
    }

    #[test]
    fn conservative_fusion_keeps_the_lexical_head_and_adds_one_semantic_tail() {
        let mut dictionary = dictionary();
        dictionary
            .entries
            .push(entry("delta", 4, "A made-up fourth entry."));
        dictionary
            .entries
            .push(entry("epsilon", 5, "A made-up fifth entry."));
        let lexical = lexical(
            vec![
                hit(Source::Dictionary, "alpha", 1, 100),
                hit(Source::Dictionary, "beta", 2, 90),
                hit(Source::Dictionary, "gamma", 3, 80),
                hit(Source::Dictionary, "delta", 4, 70),
            ],
            4,
        );
        let response = fuse_conservative(
            &dictionary,
            lexical,
            &[SemanticMatch {
                reference: reference(Source::Dictionary, 4),
                similarity: 0.9,
            }],
            5,
        );
        let terms: Vec<&str> = response.hits.iter().map(|hit| hit.term.as_str()).collect();
        assert_eq!(terms, vec!["alpha", "beta", "gamma", "delta", "epsilon"]);
        assert_eq!(response.hits[4].matched_in, MatchKind::Semantic);
        assert_eq!(response.total, 5);
    }

    #[test]
    fn conservative_fusion_does_not_demote_a_lexical_hit_already_in_the_head() {
        let dictionary = dictionary();
        let lexical = lexical(
            vec![
                hit(Source::Dictionary, "alpha", 1, 100),
                hit(Source::Dictionary, "beta", 2, 90),
                hit(Source::Dictionary, "gamma", 3, 80),
            ],
            3,
        );
        let response = fuse_conservative(
            &dictionary,
            lexical,
            &[SemanticMatch {
                reference: reference(Source::Dictionary, 0),
                similarity: 0.99,
            }],
            5,
        );
        assert_eq!(response.hits.len(), 3);
        assert_eq!(response.hits[0].term, "alpha");
        assert_eq!(response.total, 3);
    }

    #[test]
    fn conservative_fusion_reserves_no_semantic_slot_at_limit_one() {
        let dictionary = dictionary();
        let response = fuse_conservative(
            &dictionary,
            lexical(
                vec![
                    hit(Source::Dictionary, "alpha", 1, 100),
                    hit(Source::Dictionary, "beta", 2, 90),
                ],
                2,
            ),
            &[SemanticMatch {
                reference: reference(Source::Dictionary, 2),
                similarity: 0.99,
            }],
            1,
        );
        assert_eq!(response.hits.len(), 1);
        assert_eq!(response.hits[0].term, "alpha");
    }

    #[test]
    fn hybrid_search_falls_back_to_lexical_when_embedding_fails() {
        let dictionary = dictionary();
        let failing = FakeEmbedder::failing(8);
        let index = SemanticIndex::new(
            IndexMetadata {
                package_version: "0.5.0".to_owned(),
                dictionary_fingerprint: "fingerprint".to_owned(),
                model_name: "fake".to_owned(),
                model_digest: "0123456789abcdef".to_owned(),
                dimensions: 8,
                profile_hash: "profile".to_owned(),
            },
            Vec::new(),
        )
        .unwrap();
        let request = SearchRequest {
            query: "zzz".to_owned(),
            scope: Scope::All,
            limit: 10,
        };
        let response = search_hybrid(
            &dictionary,
            &request,
            &failing,
            &index,
            HybridOptions::default(),
        )
        .unwrap();
        assert_eq!(failing.query_calls(), 1);
        assert!(response.hits.is_empty());
    }
}
