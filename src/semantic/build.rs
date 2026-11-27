//! Deterministic semantic-index construction.

use crate::dictionary::Dictionary;

use super::document::{DocumentProfile, build_documents};
use super::embedder::Embedder;
use super::index::{IndexMetadata, SemanticIndex};
use super::profiles::EmbeddingProfile;

pub const DEFAULT_BATCH_SIZE: usize = 16;

#[derive(Debug, Clone, PartialEq)]
pub struct BuildResult {
    pub index: SemanticIndex,
    pub documents: usize,
    pub dimensions: usize,
    pub long_documents: usize,
    pub elapsed_ms: u128,
}

pub fn build_index(
    dictionary: &Dictionary,
    embedder: &dyn Embedder,
    model_name: &str,
    model_digest: &str,
    profile: &EmbeddingProfile,
    batch_size: usize,
    mut progress: impl FnMut(usize, usize),
) -> Result<BuildResult, String> {
    if batch_size == 0 {
        return Err("the index batch size must be at least 1".to_owned());
    }
    profile.validate()?;
    let started = std::time::Instant::now();
    let documents = build_documents(dictionary, &DocumentProfile::default());
    if documents.is_empty() {
        return Err("the dictionary produced no searchable documents".to_owned());
    }
    let long_documents = documents
        .iter()
        .filter(|document| document.text.chars().count() > profile.max_input_chars)
        .count();

    let total = documents.len();
    let mut vectors = Vec::with_capacity(total);
    let mut dimensions = None;
    progress(0, total);

    for chunk in documents.chunks(batch_size) {
        let texts: Vec<String> = chunk.iter().map(|document| document.text.clone()).collect();
        let batch = embedder.embed_documents(&texts)?;
        if batch.len() != texts.len() {
            return Err(format!(
                "the embedder returned {} vectors for {} documents",
                batch.len(),
                texts.len()
            ));
        }
        for vector in batch {
            if vector.is_empty() {
                return Err("the embedder returned an empty vector".to_owned());
            }
            if let Some(expected) = dimensions {
                if vector.len() != expected {
                    return Err(format!(
                        "the embedder returned inconsistent dimensions: {expected} then {}",
                        vector.len()
                    ));
                }
            } else {
                dimensions = Some(vector.len());
            }
            vectors.push(vector);
        }
        progress(vectors.len(), total);
    }

    let dimensions = dimensions.ok_or_else(|| "the embedder returned no vectors".to_owned())?;
    if embedder.dimensions() != 0 && embedder.dimensions() != dimensions {
        return Err(format!(
            "the embedder declares {} dimensions but returned {dimensions}",
            embedder.dimensions()
        ));
    }
    let metadata = IndexMetadata {
        package_version: env!("CARGO_PKG_VERSION").to_owned(),
        dictionary_fingerprint: dictionary.content_fingerprint().to_owned(),
        model_name: model_name.to_owned(),
        model_digest: model_digest.to_owned(),
        dimensions,
        profile_hash: profile.hash(),
    };
    let index = SemanticIndex::build(metadata, &documents, vectors)?;
    Ok(BuildResult {
        index,
        documents: total,
        dimensions,
        long_documents,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::{Entry, Source};
    use crate::semantic::embedder::FakeEmbedder;
    use crate::semantic::index::IndexExpectation;

    fn dictionary() -> Dictionary {
        let mut dictionary = Dictionary::empty();
        dictionary.entries.push(Entry {
            headword: "alpha".to_owned(),
            norm_headword: "alpha".to_owned(),
            file: "08-a.md".to_owned(),
            line: 1,
            anchor: None,
            aliases: Vec::new(),
            body: "A made-up first entry.".to_owned(),
            norm_body: "a made-up first entry.".to_owned(),
            links: Vec::new(),
        });
        dictionary.entries.push(Entry {
            headword: "beta".to_owned(),
            norm_headword: "beta".to_owned(),
            file: "08-a.md".to_owned(),
            line: 2,
            anchor: None,
            aliases: Vec::new(),
            body: "A made-up second entry.".to_owned(),
            norm_body: "a made-up second entry.".to_owned(),
            links: Vec::new(),
        });
        dictionary
    }

    #[test]
    fn a_build_is_deterministic_and_reports_progress() {
        let dictionary = dictionary();
        let profile = EmbeddingProfile::for_model("fake");
        let embedder = FakeEmbedder::new(8);
        let mut progress = Vec::new();
        let first = build_index(
            &dictionary,
            &embedder,
            "fake",
            "digest",
            &profile,
            1,
            |done, total| progress.push((done, total)),
        )
        .unwrap();
        assert_eq!(first.documents, 2);
        assert_eq!(first.dimensions, 8);
        assert_eq!(first.index.records.len(), 2);
        assert_eq!(progress, vec![(0, 2), (1, 2), (2, 2)]);
        assert_eq!(first.index.metadata.dictionary_fingerprint, "");
        assert_eq!(first.index.metadata.profile_hash, profile.hash());

        let second = build_index(
            &dictionary,
            &embedder,
            "fake",
            "digest",
            &profile,
            2,
            |_, _| {},
        )
        .unwrap();
        assert_eq!(first.index, second.index);
    }

    #[test]
    fn identity_can_be_validated_after_a_build() {
        let dictionary = dictionary();
        let profile = EmbeddingProfile::for_model("fake");
        let result = build_index(
            &dictionary,
            &FakeEmbedder::new(8),
            "fake",
            "digest",
            &profile,
            16,
            |_, _| {},
        )
        .unwrap();
        result
            .index
            .validate_expectation(&IndexExpectation {
                package_version: env!("CARGO_PKG_VERSION").to_owned(),
                dictionary_fingerprint: String::new(),
                model_name: "fake".to_owned(),
                model_digest: "digest".to_owned(),
                dimensions: 8,
                profile_hash: profile.hash(),
            })
            .unwrap();
    }

    #[test]
    fn inconsistent_vectors_are_rejected() {
        struct BadEmbedder;
        impl Embedder for BadEmbedder {
            fn dimensions(&self) -> usize {
                8
            }
            fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
                Ok(texts.iter().map(|_| vec![1.0, 0.0]).collect())
            }
            fn embed_query(&self, _: &str) -> Result<Vec<f32>, String> {
                Ok(vec![1.0, 0.0])
            }
        }
        let profile = EmbeddingProfile::for_model("fake");
        let error = build_index(
            &dictionary(),
            &BadEmbedder,
            "fake",
            "digest",
            &profile,
            16,
            |_, _| {},
        )
        .unwrap_err();
        assert!(error.contains("dimensions"));
    }

    #[test]
    fn a_zero_batch_size_is_rejected() {
        let profile = EmbeddingProfile::for_model("fake");
        let error = build_index(
            &dictionary(),
            &FakeEmbedder::new(8),
            "fake",
            "digest",
            &profile,
            0,
            |_, _| {},
        )
        .unwrap_err();
        assert!(error.contains("batch size"));
    }

    #[test]
    fn the_source_type_is_part_of_each_record() {
        let dictionary = dictionary();
        let result = build_index(
            &dictionary,
            &FakeEmbedder::new(4),
            "fake",
            "digest",
            &EmbeddingProfile::for_model("fake"),
            16,
            |_, _| {},
        )
        .unwrap();
        assert!(
            result
                .index
                .records
                .iter()
                .all(|record| record.reference.source == Source::Dictionary)
        );
    }
}
