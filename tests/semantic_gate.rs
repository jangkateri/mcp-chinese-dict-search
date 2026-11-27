//! Feature-gated Ollama capability smoke test (phase S0).
//!
//! Compiled only with `--features semantic`. It runs against a local Ollama
//! server when `MCP_CHINESE_DICT_OLLAMA_URL` and
//! `MCP_CHINESE_DICT_EMBEDDING_MODEL` are both set, and prints a skip reason
//! otherwise so the default hermetic suite never needs a model.

#![cfg(feature = "semantic")]

use serde::Deserialize;

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
    digest: String,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

fn configured() -> Option<(String, String)> {
    let url = std::env::var("MCP_CHINESE_DICT_OLLAMA_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())?;
    let model = std::env::var("MCP_CHINESE_DICT_EMBEDDING_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty())?;
    Some((url.trim_end_matches('/').to_owned(), model))
}

#[test]
fn ollama_returns_finite_normalized_non_identical_vectors() {
    let Some((base, model)) = configured() else {
        eprintln!(
            "skipping Ollama smoke test: set MCP_CHINESE_DICT_OLLAMA_URL and \
             MCP_CHINESE_DICT_EMBEDDING_MODEL"
        );
        return;
    };

    let tags: TagsResponse = ureq::get(format!("{base}/api/tags"))
        .call()
        .expect("GET /api/tags succeeds")
        .body_mut()
        .read_json()
        .expect("GET /api/tags returns JSON");
    let tag = tags
        .models
        .iter()
        .find(|tag| tag.name == model)
        .unwrap_or_else(|| panic!("model {model} is not present in /api/tags"));
    assert!(!tag.digest.is_empty(), "the model digest is empty");

    let request = serde_json::json!({
        "model": model,
        "input": ["alpha beta", "gamma delta"],
        "truncate": true,
        "keep_alive": "30m",
        "options": { "num_ctx": 4096 }
    });
    let response: EmbedResponse = ureq::post(format!("{base}/api/embed"))
        .send_json(request)
        .expect("POST /api/embed succeeds")
        .body_mut()
        .read_json()
        .expect("POST /api/embed returns JSON");

    assert_eq!(response.embeddings.len(), 2, "one vector per input");
    for vector in &response.embeddings {
        assert!(!vector.is_empty(), "the embedding is empty");
        assert!(
            vector.iter().all(|value| value.is_finite()),
            "the embedding contains NaN or infinity"
        );
        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.01,
            "expected a unit vector, got norm {norm}"
        );
    }
    assert_ne!(
        response.embeddings[0], response.embeddings[1],
        "different inputs must not produce identical vectors"
    );
}
