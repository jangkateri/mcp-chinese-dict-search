//! Hermetic synthetic evaluation of lexical versus gated hybrid retrieval.
//!
//! This is a mechanism test, not the real-data acceptance measurement. It uses
//! an invented corpus and a topic embedder whose vectors are built from
//! invented keyword groups, so it can run without a model.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::dictionary::{Dictionary, Entry, Scope, SearchRequest};
    use crate::semantic::{
        Embedder, HybridOptions, IndexMetadata, SemanticIndex, build_documents, search_hybrid,
    };

    const TOPICS: &[&[&str]] = &[
        &["sweat", "sweating", "perspiration", "perspire"],
        &["heat", "hot", "burning"],
        &["cold", "chilled", "shiver", "trembling"],
        &["digestion", "stool", "stools", "appetite", "bowel"],
        &["cough", "lung", "breath", "chest"],
        &["sleep", "insomnia", "night", "waking"],
        &["pain", "ache", "sore", "painful"],
        &["swelling", "swollen", "edema", "puffy", "bloated"],
    ];

    struct TopicEmbedder;

    impl Embedder for TopicEmbedder {
        fn dimensions(&self) -> usize {
            TOPICS.len() + 1
        }

        fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            Ok(texts.iter().map(|text| self.vector(text)).collect())
        }

        fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
            Ok(self.vector(text))
        }
    }

    impl TopicEmbedder {
        fn vector(&self, text: &str) -> Vec<f32> {
            let lower = text.to_ascii_lowercase();
            let mut vector = Vec::with_capacity(TOPICS.len() + 1);
            for keywords in TOPICS {
                let count = keywords
                    .iter()
                    .map(|keyword| lower.matches(keyword).count())
                    .sum::<usize>();
                vector.push(count as f32);
            }
            vector.push(0.05);
            let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
            vector.into_iter().map(|value| value / norm).collect()
        }
    }

    fn entry(headword: &str, line: usize, body: &str) -> Entry {
        Entry {
            headword: headword.to_owned(),
            norm_headword: headword.to_owned(),
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
        let bodies = [
            ("alpha", "profuse perspiration after minor exertion"),
            ("beta", "a burning sensation in the chest"),
            ("gamma", "loose stools and poor appetite"),
            ("delta", "a dry cough that is worse at night"),
            ("epsilon", "aching joints and sore muscles"),
            ("zeta", "swollen ankles and puffiness"),
            ("eta", "chills and shivering with a mild fever"),
            ("theta", "waking repeatedly and unable to sleep"),
        ];
        let mut dictionary = Dictionary::empty();
        for (index, (headword, body)) in bodies.iter().enumerate() {
            dictionary.entries.push(entry(headword, index + 1, body));
            dictionary
                .by_headword
                .entry((*headword).to_owned())
                .or_default()
                .push(index);
        }
        dictionary
    }

    fn queries() -> Vec<(&'static str, &'static str)> {
        vec![
            ("sweating too much", "alpha"),
            ("feeling hot inside", "beta"),
            ("watery bowel movements", "gamma"),
            ("a tickle in the chest at bedtime", "delta"),
            ("limbs are painful", "epsilon"),
            ("feet look bloated", "zeta"),
            ("uncontrollable trembling", "eta"),
            ("waking through the night", "theta"),
        ]
    }

    fn recall_at_five(hits: &[String], expected: &str) -> f64 {
        f64::from(hits.iter().take(5).any(|term| term == expected))
    }

    fn reciprocal_rank(hits: &[String], expected: &str) -> f64 {
        hits.iter()
            .position(|term| term == expected)
            .map(|index| 1.0 / (index as f64 + 1.0))
            .unwrap_or(0.0)
    }

    #[test]
    fn hybrid_recall_beats_lexical_on_invented_paraphrases() {
        let dictionary = dictionary();
        let embedder = TopicEmbedder;
        let documents = build_documents(&dictionary, &Default::default());
        let texts: Vec<String> = documents
            .iter()
            .map(|document| document.text.clone())
            .collect();
        let vectors = embedder.embed_documents(&texts).unwrap();
        let index = SemanticIndex::build(
            IndexMetadata {
                package_version: "0.5.0".to_owned(),
                dictionary_fingerprint: dictionary.content_fingerprint().to_owned(),
                model_name: "topic".to_owned(),
                model_digest: "invented".to_owned(),
                dimensions: embedder.dimensions(),
                profile_hash: "topic".to_owned(),
            },
            &documents,
            vectors,
        )
        .unwrap();

        let mut lexical_hits: HashMap<&str, Vec<String>> = HashMap::new();
        let mut hybrid_hits: HashMap<&str, Vec<String>> = HashMap::new();
        for (query, expected) in queries() {
            let request = SearchRequest {
                query: query.to_owned(),
                scope: Scope::All,
                limit: 5,
            };
            let lexical = dictionary.search(&request).unwrap();
            lexical_hits.insert(
                expected,
                lexical.hits.into_iter().map(|hit| hit.term).collect(),
            );
            let hybrid = search_hybrid(
                &dictionary,
                &request,
                &embedder,
                &index,
                HybridOptions {
                    min_similarity: 0.30,
                    top_k: 20,
                },
            )
            .unwrap();
            hybrid_hits.insert(
                expected,
                hybrid.hits.into_iter().map(|hit| hit.term).collect(),
            );
        }

        let count = queries().len() as f64;
        let lexical_recall = queries()
            .iter()
            .map(|(_, expected)| recall_at_five(&lexical_hits[expected], expected))
            .sum::<f64>()
            / count;
        let hybrid_recall = queries()
            .iter()
            .map(|(_, expected)| recall_at_five(&hybrid_hits[expected], expected))
            .sum::<f64>()
            / count;
        let lexical_mrr = queries()
            .iter()
            .map(|(_, expected)| reciprocal_rank(&lexical_hits[expected], expected))
            .sum::<f64>()
            / count;
        let hybrid_mrr = queries()
            .iter()
            .map(|(_, expected)| reciprocal_rank(&hybrid_hits[expected], expected))
            .sum::<f64>()
            / count;

        eprintln!(
            "synthetic eval: lexical recall@5 {lexical_recall:.2}, MRR {lexical_mrr:.2}; hybrid \
             recall@5 {hybrid_recall:.2}, MRR {hybrid_mrr:.2}"
        );
        assert!(
            hybrid_recall > lexical_recall,
            "hybrid recall {hybrid_recall} did not beat lexical {lexical_recall}"
        );
        assert!(
            hybrid_recall >= 0.50,
            "hybrid recall {hybrid_recall} is too low"
        );
    }
}
