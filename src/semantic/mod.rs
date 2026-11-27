//! Gated hybrid retrieval core.
//!
//! This module is compiled only with the `semantic` feature. It contains the
//! model-independent pieces: document construction, the embedder trait and its
//! deterministic fake, the flat cosine index, reciprocal-rank fusion, and the
//! loopback Ollama client, index build, and serving engine.

pub mod build;
pub mod document;
pub mod embedder;
pub mod engine;
#[cfg(test)]
mod eval;
pub mod fusion;
pub mod index;
pub mod ollama;
pub mod profiles;
pub mod startup;

/// Lexical score at or above which the embedding model is not consulted.
pub const HYBRID_GATE_SCORE: u32 = 300;

/// Reciprocal-rank-fusion constant.
pub const RRF_K: f64 = 60.0;

/// How many semantic candidates are considered before fusion.
pub const SEMANTIC_TOP_K: usize = 20;

/// Default minimum cosine similarity for a semantic candidate.
pub const MIN_SIMILARITY: f32 = 0.30;

pub use build::{BuildResult, DEFAULT_BATCH_SIZE, build_index};
pub use document::{DocumentProfile, SemanticDocument, build_documents};
pub use embedder::{Embedder, FakeEmbedder};
pub use engine::SemanticEngine;
pub use fusion::{
    HybridOptions, HybridTrace, fuse, search_hybrid, search_hybrid_traced, should_embed,
};
pub use index::{
    FORMAT_VERSION, IndexExpectation, IndexMetadata, SemanticIndex, SemanticMatch, SemanticRecord,
    semantic_cache_path,
};
pub use ollama::{ModelTag, OllamaClient, OllamaEmbedder, is_loopback_url};
pub use profiles::EmbeddingProfile;
pub use startup::{
    BuildReport, PruneReport, ServeOptions, build, check, load_engine, prune, resolve_index_path,
    resolve_profile,
};
