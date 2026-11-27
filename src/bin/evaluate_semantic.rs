//! Offline lexical-versus-hybrid evaluation for a local query set.
//!
//! This is a standalone binary, not part of the MCP server. It loads the
//! dictionary and semantic index directly, so it needs no child process and no
//! Python runtime.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use mcp_chinese_dict_search::dictionary::{Dictionary, Scope, SearchRequest};
use mcp_chinese_dict_search::semantic::{
    HybridOptions, HybridTrace, IndexExpectation, OllamaClient, OllamaEmbedder, SEMANTIC_TOP_K,
    SemanticIndex, resolve_index_path, resolve_profile, search_hybrid_traced,
};

#[derive(Debug, Deserialize)]
struct QueryCase {
    query: String,
    expected: String,
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Both,
    Lexical,
    Hybrid,
}

impl Mode {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "both" => Ok(Self::Both),
            "lexical" => Ok(Self::Lexical),
            "hybrid" => Ok(Self::Hybrid),
            other => Err(format!(
                "invalid --mode {other:?}; expected both, lexical, or hybrid"
            )),
        }
    }
}

#[derive(Debug)]
struct Args {
    data: PathBuf,
    cache_dir: Option<PathBuf>,
    queries: PathBuf,
    semantic_index: Option<PathBuf>,
    embedding_model: Option<String>,
    embedding_profile: Option<PathBuf>,
    ollama_url: String,
    limit: usize,
    repeat: usize,
    mode: Mode,
    per_query: bool,
}

struct HybridContext {
    embedder: OllamaEmbedder,
    index: SemanticIndex,
    options: HybridOptions,
}

struct Case {
    query: String,
    expected: String,
    scope: Scope,
}

#[derive(Serialize)]
struct Metrics {
    queries: usize,
    recall_at_5: f64,
    mrr: f64,
    top1: f64,
    p50_ms: f64,
    p95_ms: f64,
}

#[derive(Serialize)]
struct QueryResult {
    query: String,
    expected: String,
    scope: String,
    rank: Option<usize>,
    matched_in: Option<String>,
    score: Option<u32>,
    top_term: Option<String>,
    top_score: u32,
    hits_total: usize,
    latency_ms: f64,
    gate_open: Option<bool>,
    embedding_ms: Option<u64>,
    semantic_candidates: Option<usize>,
    fallback: Option<String>,
}

#[derive(Serialize)]
struct ModeResult {
    metrics: Metrics,
    queries: Vec<QueryResult>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("evaluate-semantic: {error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let cases = load_queries(&args.queries)?;
    let dictionary =
        Dictionary::load_with_options(&args.data, args.cache_dir.as_deref(), None, false)
            .map_err(|error| error.to_string())?;
    let hybrid = if matches!(args.mode, Mode::Both | Mode::Hybrid) {
        Some(load_hybrid(&args, &dictionary)?)
    } else {
        None
    };

    let mut output = serde_json::Map::new();
    if matches!(args.mode, Mode::Both | Mode::Lexical) {
        let result = run_mode(Mode::Lexical, &args, &dictionary, &cases, None)?;
        output.insert("lexical".to_owned(), mode_value(&result, args.per_query)?);
    }
    if matches!(args.mode, Mode::Both | Mode::Hybrid) {
        let result = run_mode(Mode::Hybrid, &args, &dictionary, &cases, hybrid.as_ref())?;
        output.insert("hybrid".to_owned(), mode_value(&result, args.per_query)?);
    }

    let value = serde_json::Value::Object(output);
    println!(
        "{}",
        serde_json::to_string(&value).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn mode_value(result: &ModeResult, per_query: bool) -> Result<serde_json::Value, String> {
    let value = if per_query {
        serde_json::to_value(result)
    } else {
        serde_json::to_value(&result.metrics)
    };
    value.map_err(|error| error.to_string())
}

fn load_hybrid(args: &Args, dictionary: &Dictionary) -> Result<HybridContext, String> {
    let model = args
        .embedding_model
        .as_deref()
        .ok_or_else(|| "--embedding-model is required for hybrid mode".to_owned())?;
    let profile = resolve_profile(model, args.embedding_profile.as_deref())?;
    let client = OllamaClient::query(&args.ollama_url)?;
    let tag = client.model_tag(model)?;
    let path = resolve_index_path(
        &args.data,
        args.cache_dir.as_deref(),
        dictionary,
        model,
        &tag.digest,
        args.semantic_index.as_deref(),
    );
    if !path.exists() {
        return Err(format!(
            "semantic index is missing at {}; rebuild with --embedding-model {model} \
             --build-semantic-index",
            path.display()
        ));
    }
    let index = SemanticIndex::load(&path)?;
    let dimensions = client.dimensions(model, &profile)?;
    index.validate_expectation(&IndexExpectation {
        document_version: mcp_chinese_dict_search::semantic::DOCUMENT_VERSION,
        dictionary_fingerprint: dictionary.content_fingerprint().to_owned(),
        model_name: model.to_owned(),
        model_digest: tag.digest.clone(),
        dimensions,
        profile_hash: profile.hash(),
    })?;
    let min_similarity = profile.min_similarity;
    let embedder = OllamaEmbedder::new(client, model, profile, dimensions);
    Ok(HybridContext {
        embedder,
        index,
        options: HybridOptions {
            min_similarity,
            top_k: SEMANTIC_TOP_K,
        },
    })
}

fn run_mode(
    mode: Mode,
    args: &Args,
    dictionary: &Dictionary,
    cases: &[Case],
    hybrid: Option<&HybridContext>,
) -> Result<ModeResult, String> {
    let mut queries = Vec::with_capacity(cases.len());
    let mut latencies = Vec::with_capacity(cases.len() * args.repeat);
    for case in cases {
        let mut latest: Option<(
            mcp_chinese_dict_search::dictionary::SearchResponse,
            Option<HybridTrace>,
            f64,
        )> = None;
        for _ in 0..args.repeat {
            let request = SearchRequest {
                query: case.query.clone(),
                scope: case.scope,
                limit: args.limit,
            };
            let started = Instant::now();
            let (response, trace) = match mode {
                Mode::Lexical => (
                    dictionary
                        .search(&request)
                        .map_err(|error| error.to_string())?,
                    None,
                ),
                Mode::Hybrid => {
                    let context = hybrid
                        .ok_or_else(|| "hybrid mode requires an embedding model".to_owned())?;
                    let (response, trace) = search_hybrid_traced(
                        dictionary,
                        &request,
                        &context.embedder,
                        &context.index,
                        context.options,
                    )
                    .map_err(|error| error.to_string())?;
                    (response, Some(trace))
                }
                Mode::Both => return Err("internal error: Both passed to run_mode".to_owned()),
            };
            let elapsed = started.elapsed().as_secs_f64() * 1000.0;
            latencies.push(elapsed);
            latest = Some((response, trace, elapsed));
        }
        let (response, trace, latency) =
            latest.ok_or_else(|| "--repeat must be at least 1".to_owned())?;
        let top = response.hits.first();
        let found = response
            .hits
            .iter()
            .enumerate()
            .find(|(_, hit)| hit.term == case.expected);
        let (rank, matched_in, score) = match found {
            Some((index, hit)) => (
                Some(index + 1),
                serde_json::to_value(hit.matched_in)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned)),
                Some(hit.score),
            ),
            None => (None, None, None),
        };
        let (gate_open, embedding_ms, semantic_candidates, fallback) = match trace {
            Some(trace) => (
                Some(trace.gate_open),
                Some(u64::try_from(trace.embedding_ms).unwrap_or(u64::MAX)),
                Some(trace.semantic_candidates),
                trace.fallback,
            ),
            None => (None, None, None, None),
        };
        queries.push(QueryResult {
            query: case.query.clone(),
            expected: case.expected.clone(),
            scope: case.scope.as_str().to_owned(),
            rank,
            matched_in,
            score,
            top_term: top.map(|hit| hit.term.clone()),
            top_score: top.map(|hit| hit.score).unwrap_or(0),
            hits_total: response.total,
            latency_ms: latency,
            gate_open,
            embedding_ms,
            semantic_candidates,
            fallback,
        });
    }
    let metrics = metrics(cases.len(), &queries, &latencies);
    Ok(ModeResult { metrics, queries })
}

fn metrics(count: usize, queries: &[QueryResult], latencies: &[f64]) -> Metrics {
    if count == 0 {
        return Metrics {
            queries: 0,
            recall_at_5: 0.0,
            mrr: 0.0,
            top1: 0.0,
            p50_ms: 0.0,
            p95_ms: 0.0,
        };
    }
    let count = count as f64;
    let recall = queries
        .iter()
        .filter(|query| query.rank.is_some_and(|rank| rank <= 5))
        .count() as f64
        / count;
    let top1 = queries.iter().filter(|query| query.rank == Some(1)).count() as f64 / count;
    let reciprocal = queries
        .iter()
        .filter_map(|query| query.rank)
        .map(|rank| 1.0 / rank as f64)
        .sum::<f64>()
        / count;
    let mut sorted = latencies.to_vec();
    sorted.sort_by(f64::total_cmp);
    Metrics {
        queries: count as usize,
        recall_at_5: recall,
        mrr: reciprocal,
        top1,
        p50_ms: median(&sorted),
        p95_ms: percentile(&sorted, 0.95),
    }
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values[middle]
    } else {
        (values[middle - 1] + values[middle]) / 2.0
    }
}

fn percentile(values: &[f64], fraction: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let position = ((fraction * (values.len() - 1) as f64).round() as usize).min(values.len() - 1);
    values[position]
}

fn load_queries(path: &Path) -> Result<Vec<Case>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let cases: Vec<QueryCase> = serde_json::from_str(&text)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
    if cases.is_empty() {
        return Err("the query file must contain a non-empty JSON array".to_owned());
    }
    let mut queries = Vec::with_capacity(cases.len());
    for (index, case) in cases.into_iter().enumerate() {
        if case.query.trim().is_empty() {
            return Err(format!("query {} has an empty query", index + 1));
        }
        if case.expected.trim().is_empty() {
            return Err(format!("query {} has an empty expected term", index + 1));
        }
        let scope = match case.scope.as_deref() {
            None => Scope::All,
            Some(value) => Scope::parse(value)
                .ok_or_else(|| format!("query {} has an invalid scope {value:?}", index + 1))?,
        };
        queries.push(Case {
            query: case.query,
            expected: case.expected,
            scope,
        });
    }
    Ok(queries)
}

fn parse_args() -> Result<Args, String> {
    let mut data = None;
    let mut cache_dir = None;
    let mut queries = None;
    let mut semantic_index = None;
    let mut embedding_model = None;
    let mut embedding_profile = None;
    let mut ollama_url = "http://127.0.0.1:11434".to_owned();
    let mut limit = 5usize;
    let mut repeat = 1usize;
    let mut mode = None;
    let mut per_query = false;

    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--data" => data = Some(PathBuf::from(value(&mut args, "--data")?)),
            "--cache-dir" => {
                cache_dir = Some(PathBuf::from(value(&mut args, "--cache-dir")?));
            }
            "--queries" => queries = Some(PathBuf::from(value(&mut args, "--queries")?)),
            "--semantic-index" => {
                semantic_index = Some(PathBuf::from(value(&mut args, "--semantic-index")?));
            }
            "--embedding-model" => {
                embedding_model = Some(value(&mut args, "--embedding-model")?);
            }
            "--embedding-profile" => {
                embedding_profile = Some(PathBuf::from(value(&mut args, "--embedding-profile")?));
            }
            "--ollama-url" => ollama_url = value(&mut args, "--ollama-url")?,
            "--limit" => {
                let value = value(&mut args, "--limit")?;
                limit = value
                    .parse()
                    .map_err(|_| format!("invalid --limit {value:?}"))?;
            }
            "--repeat" => {
                let value = value(&mut args, "--repeat")?;
                repeat = value
                    .parse()
                    .map_err(|_| format!("invalid --repeat {value:?}"))?;
            }
            "--mode" => mode = Some(Mode::parse(&value(&mut args, "--mode")?)?),
            "--per-query" => per_query = true,
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }

    if !(1..=20).contains(&limit) {
        return Err("--limit must be between 1 and 20".to_owned());
    }
    if repeat < 1 {
        return Err("--repeat must be at least 1".to_owned());
    }
    let data = data.ok_or_else(|| "--data is required".to_owned())?;
    let queries = queries.ok_or_else(|| "--queries is required".to_owned())?;
    let mode = mode.unwrap_or(if embedding_model.is_some() {
        Mode::Both
    } else {
        Mode::Lexical
    });
    if matches!(mode, Mode::Both | Mode::Hybrid) && embedding_model.is_none() {
        return Err("--embedding-model is required for hybrid mode".to_owned());
    }

    Ok(Args {
        data,
        cache_dir,
        queries,
        semantic_index,
        embedding_model,
        embedding_profile,
        ollama_url,
        limit,
        repeat,
        mode,
        per_query,
    })
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn print_help() {
    println!(
        "evaluate-semantic — compare lexical and gated hybrid retrieval\n\
         \n\
         Usage:\n\
           evaluate-semantic --data <path> --queries <path> [options]\n\
         \n\
         Options:\n\
           --cache-dir <path>          Lookup cache and semantic index directory\n\
           --semantic-index <path>     Exact semantic index override\n\
           --embedding-model <name>    Ollama model; enables hybrid mode\n\
           --embedding-profile <path>  JSON profile override\n\
           --ollama-url <url>          Loopback Ollama URL\n\
           --limit <n>                 Hits per query, 1–20 (default 5)\n\
           --repeat <n>                Repeats per query (default 1)\n\
           --mode both|lexical|hybrid  Default: both with a model, else lexical\n\
           --per-query                 Include per-query rank and trace fields\n\
           -h, --help                  Show this help\n\
         \n\
         The query file is a JSON array of objects with \"query\", \"expected\",\n\
         and optional \"scope\" (all, dictionary, meridian, medicinal, formulas,\n\
         weights)."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_and_median_handle_edges() {
        assert_eq!(percentile(&[], 0.95), 0.0);
        assert_eq!(median(&[]), 0.0);
        assert_eq!(percentile(&[10.0], 0.95), 10.0);
        assert_eq!(median(&[10.0, 20.0]), 15.0);
        assert_eq!(percentile(&[10.0, 20.0], 0.95), 20.0);
    }

    #[test]
    fn mode_parsing_is_explicit() {
        assert_eq!(Mode::parse("both").unwrap(), Mode::Both);
        assert!(Mode::parse("nope").is_err());
    }
}
