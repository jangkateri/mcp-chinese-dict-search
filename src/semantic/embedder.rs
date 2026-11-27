//! The embedding interface and a deterministic fake for hermetic tests.

use std::sync::atomic::{AtomicUsize, Ordering};

/// One embedding model, addressed through a narrow blocking interface.
pub trait Embedder: Send + Sync {
    fn dimensions(&self) -> usize;
    fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String>;
    fn embed_query(&self, text: &str) -> Result<Vec<f32>, String>;
}

/// A fake embedder that maps each text to a stable unit vector.
///
/// The vector is derived from an FNV-1a hash of the text, so equal inputs
/// always produce equal vectors and different inputs almost always differ.
/// Tests use [`FakeEmbedder::query_calls`] to prove that the gate skipped or
/// opened embedding.
#[derive(Debug)]
pub struct FakeEmbedder {
    dimensions: usize,
    fail: bool,
    query_calls: AtomicUsize,
    document_calls: AtomicUsize,
}

impl FakeEmbedder {
    pub fn new(dimensions: usize) -> Self {
        assert!(dimensions > 0, "the fake embedder needs dimensions");
        Self {
            dimensions,
            fail: false,
            query_calls: AtomicUsize::new(0),
            document_calls: AtomicUsize::new(0),
        }
    }

    /// A fake that returns an error for every call.
    pub fn failing(dimensions: usize) -> Self {
        Self {
            fail: true,
            ..Self::new(dimensions)
        }
    }

    pub fn query_calls(&self) -> usize {
        self.query_calls.load(Ordering::Relaxed)
    }

    pub fn document_calls(&self) -> usize {
        self.document_calls.load(Ordering::Relaxed)
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        if self.fail {
            return Err("the fake embedder is configured to fail".to_owned());
        }
        Ok(fake_vector(text, self.dimensions))
    }
}

impl Embedder for FakeEmbedder {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        self.document_calls
            .fetch_add(texts.len(), Ordering::Relaxed);
        texts.iter().map(|text| self.embed(text)).collect()
    }

    fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
        self.query_calls.fetch_add(1, Ordering::Relaxed);
        self.embed(text)
    }
}

fn fake_vector(text: &str, dimensions: usize) -> Vec<f32> {
    let mut state = fnv1a(text.as_bytes());
    let mut vector = Vec::with_capacity(dimensions);
    for _ in 0..dimensions {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let unit = (state >> 11) as f64 / (1u64 << 53) as f64;
        vector.push((unit * 2.0 - 1.0) as f32);
    }
    normalize(&mut vector);
    vector
}

fn normalize(vector: &mut [f32]) {
    let norm = vector
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum::<f64>()
        .sqrt();
    if norm > 0.0 {
        for value in vector {
            *value = (*value as f64 / norm) as f32;
        }
    } else if let Some(first) = vector.first_mut() {
        *first = 1.0;
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fake_is_deterministic_and_normalized() {
        let embedder = FakeEmbedder::new(8);
        let first = embedder.embed_query("alpha").unwrap();
        let second = embedder.embed_query("alpha").unwrap();
        assert_eq!(first, second);
        assert_ne!(first, embedder.embed_query("beta").unwrap());
        assert_eq!(first.len(), 8);
        let norm = first.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn calls_are_counted() {
        let embedder = FakeEmbedder::new(4);
        assert_eq!(embedder.query_calls(), 0);
        embedder.embed_query("alpha").unwrap();
        assert_eq!(embedder.query_calls(), 1);
        embedder
            .embed_documents(&["alpha".to_owned(), "beta".to_owned()])
            .unwrap();
        assert_eq!(embedder.document_calls(), 2);
    }

    #[test]
    fn a_failing_fake_returns_an_error() {
        let embedder = FakeEmbedder::failing(4);
        assert!(embedder.embed_query("alpha").is_err());
        assert!(embedder.embed_documents(&["alpha".to_owned()]).is_err());
    }
}
