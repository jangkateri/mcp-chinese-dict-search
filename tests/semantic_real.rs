//! Optional real-data semantic smoke test.
//!
//! Runs only with `--features semantic` and when `MCP_CHINESE_DICT_DATA`,
//! `MCP_CHINESE_DICT_OLLAMA_URL`, and `MCP_CHINESE_DICT_EMBEDDING_MODEL` are
//! set. It loads an existing index when `MCP_CHINESE_DICT_SEMANTIC_INDEX`
//! points at one, and otherwise builds one in a temporary directory.

#![cfg(feature = "semantic")]

use std::path::PathBuf;

use mcp_chinese_dict_search::dictionary::{Dictionary, Scope, SearchRequest};
use mcp_chinese_dict_search::semantic::{
    EmbeddingProfile, HybridOptions, IndexExpectation, OllamaClient, OllamaEmbedder, SemanticIndex,
    build_index, semantic_cache_path,
};

fn configured() -> Option<(PathBuf, String, String)> {
    let data = std::env::var("MCP_CHINESE_DICT_DATA").ok()?;
    let url = std::env::var("MCP_CHINESE_DICT_OLLAMA_URL").ok()?;
    let model = std::env::var("MCP_CHINESE_DICT_EMBEDDING_MODEL").ok()?;
    Some((PathBuf::from(data), url, model))
}

#[test]
fn a_real_index_loads_and_answers_a_weak_query() {
    let Some((data, url, model)) = configured() else {
        eprintln!(
            "skipping real semantic test: set MCP_CHINESE_DICT_DATA, \
             MCP_CHINESE_DICT_OLLAMA_URL, and MCP_CHINESE_DICT_EMBEDDING_MODEL"
        );
        return;
    };
    let dictionary = Dictionary::load(&data).expect("the configured dictionary loads");
    let profile = EmbeddingProfile::for_model(&model);
    let client = OllamaClient::query(&url).expect("the Ollama client is loopback");
    let tag = client.model_tag(&model).expect("the model is installed");
    let path = std::env::var("MCP_CHINESE_DICT_SEMANTIC_INDEX")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            semantic_cache_path(
                &data,
                Some(&std::env::temp_dir()),
                dictionary.content_fingerprint(),
                &model,
                &tag.digest,
            )
        });

    let index = if path.exists() {
        SemanticIndex::load(&path).expect("the configured index loads")
    } else {
        let embedder = OllamaEmbedder::connect(client.clone(), &model, profile.clone())
            .expect("the model probes");
        let result = build_index(
            &dictionary,
            &embedder,
            &model,
            &tag.digest,
            &profile,
            16,
            |_, _| {},
        )
        .expect("the index builds");
        result.index.save(&path).expect("the index is written");
        result.index
    };
    let dimensions = client
        .dimensions(&model, &profile)
        .expect("the model probes");
    index
        .validate_expectation(&IndexExpectation {
            package_version: env!("CARGO_PKG_VERSION").to_owned(),
            dictionary_fingerprint: dictionary.content_fingerprint().to_owned(),
            model_name: model.clone(),
            model_digest: tag.digest.clone(),
            dimensions,
            profile_hash: profile.hash(),
        })
        .expect("the index matches the model and source");

    let embedder = OllamaEmbedder::new(client, &model, profile, dimensions);
    let request = SearchRequest {
        query: "a feeling of being unwell without a clear cause".to_owned(),
        scope: Scope::All,
        limit: 5,
    };
    let response = mcp_chinese_dict_search::semantic::search_hybrid(
        &dictionary,
        &request,
        &embedder,
        &index,
        HybridOptions::default(),
    )
    .expect("the weak query succeeds");
    eprintln!(
        "weak query returned {} hits ({} total)",
        response.hits.len(),
        response.total
    );
    assert!(response.hits.len() <= 5);
}
