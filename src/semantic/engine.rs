//! The serving-side semantic engine: one index, one model, digest re-checks.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::dictionary::{Dictionary, SearchRequest, SearchResponse};
use crate::error::DictionaryError;
use crate::logging::Logger;

use super::embedder::Embedder;
use super::fusion::{HybridOptions, search_hybrid_traced};
use super::index::SemanticIndex;
use super::ollama::{OllamaClient, OllamaEmbedder};

const DIGEST_RECHECK_INTERVAL: Duration = Duration::from_secs(300);

pub struct SemanticEngine {
    embedder: OllamaEmbedder,
    index: SemanticIndex,
    options: HybridOptions,
    logger: Logger,
    client: OllamaClient,
    model: String,
    digest: String,
    last_digest_check: Mutex<Instant>,
    recheck_interval: Duration,
    disabled: AtomicBool,
}

impl SemanticEngine {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        embedder: OllamaEmbedder,
        index: SemanticIndex,
        options: HybridOptions,
        logger: Logger,
        client: OllamaClient,
        model: impl Into<String>,
        digest: impl Into<String>,
    ) -> Self {
        Self {
            embedder,
            index,
            options,
            logger,
            client,
            model: model.into(),
            digest: digest.into(),
            last_digest_check: Mutex::new(Instant::now()),
            recheck_interval: DIGEST_RECHECK_INTERVAL,
            disabled: AtomicBool::new(false),
        }
    }

    pub fn with_recheck_interval(mut self, interval: Duration) -> Self {
        self.recheck_interval = interval;
        self
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn dimensions(&self) -> usize {
        self.embedder.dimensions()
    }

    pub fn index(&self) -> &SemanticIndex {
        &self.index
    }

    pub fn is_active(&self) -> bool {
        !self.disabled.load(Ordering::Relaxed)
    }

    pub fn search(
        &self,
        dictionary: &Dictionary,
        request: &SearchRequest,
    ) -> Result<SearchResponse, DictionaryError> {
        if !self.is_active() || !self.digest_is_current() {
            return dictionary.search(request);
        }
        let (response, trace) = search_hybrid_traced(
            dictionary,
            request,
            &self.embedder,
            &self.index,
            self.options,
        )?;
        self.logger.debug(format!(
            "semantic: gate_open={} embedding_ms={} candidates={} fallback={}",
            trace.gate_open,
            trace.embedding_ms,
            trace.semantic_candidates,
            trace.fallback.as_deref().unwrap_or("none")
        ));
        Ok(response)
    }

    /// Re-check the model digest at most once per interval. A changed digest
    /// disables semantic retrieval for the rest of the process.
    fn digest_is_current(&self) -> bool {
        let now = Instant::now();
        {
            let last = self
                .last_digest_check
                .lock()
                .expect("digest mutex poisoned");
            if now.duration_since(*last) < self.recheck_interval {
                return true;
            }
        }
        *self
            .last_digest_check
            .lock()
            .expect("digest mutex poisoned") = now;
        match self.client.model_tag(&self.model) {
            Ok(tag) if tag.digest == self.digest => true,
            Ok(tag) => {
                self.disable(format!(
                    "the Ollama model {} now has digest {}, but the loaded index was built for \
                     {}; rebuild the semantic index",
                    self.model, tag.digest, self.digest
                ));
                false
            }
            Err(error) => {
                self.logger.warn(format!(
                    "cannot re-check the Ollama model digest for {}: {error}; keeping semantic \
                     retrieval active until the next check",
                    self.model
                ));
                true
            }
        }
    }

    fn disable(&self, reason: String) {
        if !self.disabled.swap(true, Ordering::Relaxed) {
            self.logger.warn(format!(
                "semantic retrieval disabled for this process: {reason}"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::{Entry, Scope};
    use crate::semantic::index::IndexMetadata;

    fn dictionary() -> Dictionary {
        let mut dictionary = Dictionary::empty();
        dictionary.entries.push(Entry {
            headword: "alpha".to_owned(),
            norm_headword: "alpha".to_owned(),
            file: "08-a.md".to_owned(),
            line: 1,
            anchor: None,
            aliases: Vec::new(),
            body: "A made-up entry.".to_owned(),
            norm_body: "a made-up entry.".to_owned(),
            links: Vec::new(),
        });
        for (index, entry) in dictionary.entries.iter().enumerate() {
            dictionary
                .by_headword
                .entry(entry.norm_headword.clone())
                .or_default()
                .push(index);
        }
        dictionary
    }

    fn engine() -> SemanticEngine {
        let client = OllamaClient::query("http://127.0.0.1:1").unwrap();
        let profile = crate::semantic::profiles::EmbeddingProfile::for_model("fake");
        let embedder = OllamaEmbedder::new(client.clone(), "fake", profile, 8);
        let index = SemanticIndex::new(
            IndexMetadata {
                package_version: env!("CARGO_PKG_VERSION").to_owned(),
                document_version: crate::semantic::DOCUMENT_VERSION,
                dictionary_fingerprint: String::new(),
                model_name: "fake".to_owned(),
                model_digest: "digest".to_owned(),
                dimensions: 8,
                profile_hash: "profile".to_owned(),
            },
            Vec::new(),
        )
        .unwrap();
        SemanticEngine::new(
            embedder,
            index,
            HybridOptions::default(),
            Logger::new(false),
            client,
            "fake",
            "digest",
        )
    }

    fn tags_server(digest: &'static str) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 4096];
            let _ = stream.read(&mut buffer);
            let body = format!(r#"{{"models":[{{"name":"fake","digest":"{digest}"}}]}}"#);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: \
                 {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        (url, handle)
    }

    #[test]
    fn a_changed_digest_disables_semantic_retrieval() {
        let (url, handle) = tags_server("changed");
        let client = OllamaClient::query(&url).unwrap();
        let profile = crate::semantic::profiles::EmbeddingProfile::for_model("fake");
        let embedder = OllamaEmbedder::new(client.clone(), "fake", profile, 8);
        let index = SemanticIndex::new(
            IndexMetadata {
                package_version: env!("CARGO_PKG_VERSION").to_owned(),
                document_version: crate::semantic::DOCUMENT_VERSION,
                dictionary_fingerprint: String::new(),
                model_name: "fake".to_owned(),
                model_digest: "expected".to_owned(),
                dimensions: 8,
                profile_hash: "profile".to_owned(),
            },
            Vec::new(),
        )
        .unwrap();
        let engine = SemanticEngine::new(
            embedder,
            index,
            HybridOptions::default(),
            Logger::new(false),
            client,
            "fake",
            "expected",
        )
        .with_recheck_interval(Duration::ZERO);
        let response = engine
            .search(
                &dictionary(),
                &SearchRequest {
                    query: "alpha".to_owned(),
                    scope: Scope::All,
                    limit: 5,
                },
            )
            .unwrap();
        assert!(!engine.is_active());
        assert_eq!(response.hits[0].term, "alpha");
        handle.join().unwrap();
    }

    #[test]
    fn a_disabled_engine_returns_the_lexical_response() {
        let engine = engine();
        engine.disable("test".to_owned());
        assert!(!engine.is_active());
        let response = engine
            .search(
                &dictionary(),
                &SearchRequest {
                    query: "alpha".to_owned(),
                    scope: Scope::All,
                    limit: 5,
                },
            )
            .unwrap();
        assert_eq!(response.hits[0].term, "alpha");
    }
}
