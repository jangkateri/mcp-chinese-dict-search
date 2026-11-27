//! Known embedding-model profiles and a small JSON override.

use std::path::Path;

use serde::Deserialize;

const MAX_PREFIX_CHARS: usize = 4096;
const MIN_CONTEXT: u32 = 256;
const MAX_CONTEXT: u32 = 131_072;
const MAX_INPUT_CHARS_LIMIT: usize = 1_000_000;

/// Embedding-affecting formatting for one model.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingProfile {
    /// A short family name used in logs, not the Ollama model name.
    pub family: String,
    pub query_prefix: String,
    pub document_prefix: String,
    pub query_instruction: Option<String>,
    pub min_similarity: f32,
    pub num_ctx: u32,
    pub max_input_chars: usize,
}

impl EmbeddingProfile {
    pub fn for_model(model: &str) -> Self {
        let lower = model.to_ascii_lowercase();
        if lower.contains("qwen3-embedding") {
            return Self {
                family: "qwen3-embedding".to_owned(),
                query_prefix: "Query:".to_owned(),
                document_prefix: String::new(),
                query_instruction: Some(
                    "Given a web search query, retrieve relevant passages that answer the query"
                        .to_owned(),
                ),
                min_similarity: 0.30,
                num_ctx: 4096,
                max_input_chars: 20_000,
            };
        }
        if lower.contains("embeddinggemma") {
            return Self {
                family: "embeddinggemma".to_owned(),
                query_prefix: String::new(),
                document_prefix: String::new(),
                query_instruction: None,
                min_similarity: 0.30,
                num_ctx: 2048,
                max_input_chars: 20_000,
            };
        }
        if lower.contains("bge-m3") {
            return Self {
                family: "bge-m3".to_owned(),
                query_prefix: String::new(),
                document_prefix: String::new(),
                query_instruction: None,
                min_similarity: 0.30,
                num_ctx: 8192,
                max_input_chars: 20_000,
            };
        }
        if lower.contains("nomic-embed-text") {
            return Self {
                family: "nomic-embed-text".to_owned(),
                query_prefix: "search_query: ".to_owned(),
                document_prefix: "search_document: ".to_owned(),
                query_instruction: None,
                min_similarity: 0.30,
                num_ctx: 8192,
                max_input_chars: 20_000,
            };
        }
        if lower.contains("mxbai-embed-large") {
            return Self {
                family: "mxbai-embed-large".to_owned(),
                query_prefix: "Represent this sentence for searching relevant passages: "
                    .to_owned(),
                document_prefix: String::new(),
                query_instruction: None,
                min_similarity: 0.30,
                num_ctx: 512,
                max_input_chars: 20_000,
            };
        }
        Self {
            family: "default".to_owned(),
            query_prefix: String::new(),
            document_prefix: String::new(),
            query_instruction: None,
            min_similarity: 0.30,
            num_ctx: 4096,
            max_input_chars: 20_000,
        }
    }

    pub fn query_text(&self, text: &str) -> String {
        let mut output = String::new();
        if let Some(instruction) = &self.query_instruction {
            output.push_str("Instruct: ");
            output.push_str(instruction);
            output.push('\n');
        }
        output.push_str(&self.query_prefix);
        output.push_str(text);
        output
    }

    pub fn document_text(&self, text: &str) -> String {
        let mut output = String::new();
        output.push_str(&self.document_prefix);
        output.push_str(text);
        output
    }

    /// The embedding-affecting fields, excluding `min_similarity` and `family`.
    pub fn hash(&self) -> String {
        let num_ctx = self.num_ctx.to_string();
        let max_input_chars = self.max_input_chars.to_string();
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for value in [
            "embedding-profile-v1",
            self.query_prefix.as_str(),
            self.document_prefix.as_str(),
            self.query_instruction.as_deref().unwrap_or(""),
            num_ctx.as_str(),
            max_input_chars.as_str(),
        ] {
            for byte in value.as_bytes() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x100_0000_01b3);
            }
            hash ^= 0;
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
        format!("{hash:016x}")
    }

    pub fn apply_override(&mut self, path: &Path) -> Result<(), String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let parsed: ProfileOverride = serde_json::from_str(&text)
            .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
        if let Some(value) = parsed.query_prefix {
            self.query_prefix = value;
        }
        if let Some(value) = parsed.document_prefix {
            self.document_prefix = value;
        }
        if let Some(value) = parsed.query_instruction {
            self.query_instruction = value;
        }
        if let Some(value) = parsed.min_similarity {
            self.min_similarity = value;
        }
        if let Some(value) = parsed.num_ctx {
            self.num_ctx = value;
        }
        if let Some(value) = parsed.max_input_chars {
            self.max_input_chars = value;
        }
        self.validate()
    }

    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("query_prefix", &self.query_prefix),
            ("document_prefix", &self.document_prefix),
        ] {
            if value.chars().count() > MAX_PREFIX_CHARS {
                return Err(format!(
                    "{name} is longer than {MAX_PREFIX_CHARS} characters"
                ));
            }
        }
        if let Some(instruction) = &self.query_instruction
            && instruction.chars().count() > MAX_PREFIX_CHARS
        {
            return Err(format!(
                "query_instruction is longer than {MAX_PREFIX_CHARS} characters"
            ));
        }
        if !(0.0..=1.0).contains(&self.min_similarity) {
            return Err(format!(
                "min_similarity {} is outside 0.0..=1.0",
                self.min_similarity
            ));
        }
        if !(MIN_CONTEXT..=MAX_CONTEXT).contains(&self.num_ctx) {
            return Err(format!(
                "num_ctx {} is outside {MIN_CONTEXT}..={MAX_CONTEXT}",
                self.num_ctx
            ));
        }
        if !(1..=MAX_INPUT_CHARS_LIMIT).contains(&self.max_input_chars) {
            return Err(format!(
                "max_input_chars {} is outside 1..={MAX_INPUT_CHARS_LIMIT}",
                self.max_input_chars
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileOverride {
    query_prefix: Option<String>,
    document_prefix: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    query_instruction: Option<Option<String>>,
    min_similarity: Option<f32>,
    num_ctx: Option<u32>,
    max_input_chars: Option<usize>,
}

fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qwen_uses_an_instruction_on_queries_only() {
        let profile = EmbeddingProfile::for_model("qwen3-embedding:0.6b");
        assert_eq!(
            profile.query_text("qi deficiency"),
            "Instruct: Given a web search query, retrieve relevant passages that answer the \
             query\nQuery:qi deficiency"
        );
        assert_eq!(profile.document_text("qi deficiency"), "qi deficiency");
        assert_eq!(profile.num_ctx, 4096);
    }

    #[test]
    fn other_models_use_their_documented_prefixes() {
        let nomic = EmbeddingProfile::for_model("nomic-embed-text");
        assert_eq!(nomic.query_text("qi"), "search_query: qi");
        assert_eq!(nomic.document_text("qi"), "search_document: qi");

        let gemma = EmbeddingProfile::for_model("embeddinggemma:latest");
        assert_eq!(gemma.query_text("qi"), "qi");
        assert_eq!(gemma.document_text("qi"), "qi");
        assert_eq!(gemma.num_ctx, 2048);

        let bge = EmbeddingProfile::for_model("bge-m3:latest");
        assert_eq!(bge.num_ctx, 8192);
    }

    #[test]
    fn the_profile_hash_tracks_embedding_affecting_fields() {
        let profile = EmbeddingProfile::for_model("qwen3-embedding:0.6b");
        let mut changed = profile.clone();
        assert_eq!(profile.hash(), changed.hash());
        changed.num_ctx = 8192;
        assert_ne!(profile.hash(), changed.hash());
        changed = profile.clone();
        changed.min_similarity = 0.5;
        assert_eq!(profile.hash(), changed.hash());
        changed = profile.clone();
        changed.query_instruction = None;
        assert_ne!(profile.hash(), changed.hash());
    }

    #[test]
    fn an_override_replaces_fields_and_validates_ranges() {
        let directory =
            std::env::temp_dir().join(format!("mcp-chinese-dict-profile-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("profile.json");
        std::fs::write(
            &path,
            r#"{"query_prefix":"Q: ","query_instruction":null,"num_ctx":2048,"min_similarity":0.4}"#,
        )
        .unwrap();
        let mut profile = EmbeddingProfile::for_model("qwen3-embedding:0.6b");
        profile.apply_override(&path).unwrap();
        assert_eq!(profile.query_prefix, "Q: ");
        assert_eq!(profile.query_instruction, None);
        assert_eq!(profile.num_ctx, 2048);
        assert_eq!(profile.min_similarity, 0.4);

        std::fs::write(&path, r#"{"num_ctx":1}"#).unwrap();
        assert!(profile.apply_override(&path).is_err());
        let _ = std::fs::remove_dir_all(&directory);
    }
}
