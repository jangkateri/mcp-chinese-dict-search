//! Data types shared by the loader, the search engine, and the tool layer.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which part of the source a result comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(rename_all = "lowercase")]
pub enum Source {
    Dictionary,
    Meridian,
    Medicinal,
    Formulas,
    Weights,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dictionary => "dictionary",
            Self::Meridian => "meridian",
            Self::Medicinal => "medicinal",
            Self::Formulas => "formulas",
            Self::Weights => "weights",
        }
    }
}

/// Query domain filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(rename_all = "lowercase")]
pub enum Scope {
    All,
    Dictionary,
    Meridian,
    Medicinal,
    Formulas,
    Weights,
}

impl Scope {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "all" => Some(Self::All),
            "dictionary" => Some(Self::Dictionary),
            "meridian" => Some(Self::Meridian),
            "medicinal" => Some(Self::Medicinal),
            "formulas" => Some(Self::Formulas),
            "weights" => Some(Self::Weights),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Dictionary => "dictionary",
            Self::Meridian => "meridian",
            Self::Medicinal => "medicinal",
            Self::Formulas => "formulas",
            Self::Weights => "weights",
        }
    }

    pub fn allows(self, source: Source) -> bool {
        match self {
            Self::All => true,
            Self::Dictionary => source == Source::Dictionary,
            Self::Meridian => source == Source::Meridian,
            Self::Medicinal => source == Source::Medicinal,
            Self::Formulas => source == Source::Formulas,
            Self::Weights => source == Source::Weights,
        }
    }
}

/// One Chinese/pinyin/English alias line attached to a main entry.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Alias {
    pub simplified: String,
    pub traditional: String,
    pub pinyin_toned: String,
    pub pinyin_ascii: String,
}

/// A parsed `<a href="file#anchor">text</a>` link.
#[derive(Debug, Clone, Default, Serialize, JsonSchema)]
pub struct Link {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_anchor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_term: Option<String>,
    pub resolved: bool,
}

/// One main-dictionary entry.
#[derive(Debug, Clone)]
pub struct Entry {
    pub headword: String,
    pub norm_headword: String,
    pub file: String,
    pub line: usize,
    pub anchor: Option<String>,
    pub aliases: Vec<Alias>,
    pub body: String,
    pub norm_body: String,
    pub links: Vec<Link>,
}

/// One appendix monograph or point entry.
#[derive(Debug, Clone)]
pub struct AppendixEntry {
    pub source: Source,
    pub file: String,
    pub line: usize,
    pub id: Option<String>,
    pub section: String,
    pub subsection: Option<String>,
    pub term: String,
    pub simplified: String,
    pub traditional: String,
    pub pinyin_toned: String,
    pub pinyin_ascii: String,
    pub aliases: Vec<String>,
    pub body: String,
    pub norm_terms: Vec<String>,
    pub norm_body: String,
}

/// One appendix-4 weights/measures table.
#[derive(Debug, Clone)]
pub struct Section {
    /// The Markdown file the table was read from, used verbatim in citations.
    pub file: String,
    pub number: String,
    pub title: String,
    pub body: String,
}

/// Where an index anchor points.
#[derive(Debug, Clone)]
pub struct AnchorTarget {
    pub file: String,
    pub term: String,
    pub norm_term: String,
}

/// Reference to an appendix entry stored in one of the per-kind vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppendixRef {
    pub source: Source,
    pub index: usize,
}

/// Reference to a main-dictionary or appendix entry in a generated index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LookupRef {
    pub source: Source,
    pub index: usize,
}

/// Counts gathered while loading.
#[derive(Debug, Clone, Default)]
pub struct LoadReport {
    pub main_entries: usize,
    pub distinct_headwords: usize,
    pub duplicate_headwords: usize,
    pub meridian_entries: usize,
    pub medicinal_entries: usize,
    pub formula_entries: usize,
    pub weight_sections: usize,
    pub index_anchors: usize,
    pub recovered_entry_anchors: usize,
    pub recovered_link_targets: usize,
    pub pinyin_toned_keys: usize,
    pub pinyin_toneless_keys: usize,
    pub han_keys: usize,
    pub lookup_cache_hit: bool,
    /// True when a cache file for this fingerprint is usable: it was reused or
    /// it was written during this load.
    pub lookup_cache_ready: bool,
    pub lookup_cache_path: Option<String>,
    pub resolved_links: usize,
    pub unresolved_links: usize,
    pub anchorless_entries: usize,
    pub alias_parse_failures: usize,
    pub appendix_header_failures: usize,
    pub warnings: Vec<String>,
}

/// A search request from the tool layer.
#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub query: String,
    pub scope: Scope,
    pub limit: usize,
}

/// How a search hit matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(rename_all = "lowercase")]
pub enum MatchKind {
    Headword,
    Alias,
    Body,
    Id,
}

/// One ranked search result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SearchHit {
    pub source: Source,
    pub term: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub snippet: String,
    pub matched_in: MatchKind,
    pub score: u32,
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    pub redirect: bool,
    /// Absolute source file and line when the data is an extracted directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

/// One near-miss suggestion, with the term that resolves it and, when the tool
/// layer has written one, the page a terminal can open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Suggestion {
    /// Display text, for example `jia gen (jia root)`.
    pub text: String,
    /// Term that resolves the entry, for example `jia root`.
    pub term: String,
    /// Absolute page path, filled by the tool layer when pages are enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

/// Search results returned to the tool layer.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SearchResponse {
    pub query: String,
    pub scope: Scope,
    pub hits: Vec<SearchHit>,
    pub total: usize,
    /// Near-miss suggestions when no exact key matched and the ranked hits are
    /// weak, for example `jia gen (jia root)`.
    pub suggestions: Vec<Suggestion>,
}

/// An exact-entry request from the tool layer.
#[derive(Debug, Clone)]
pub struct EntryRequest {
    pub term: String,
    pub source: Scope,
    pub max_chars: usize,
}

/// Flattened alias display for one entry.
#[derive(Debug, Clone, Default, Serialize, JsonSchema)]
pub struct EntryAliases {
    pub simplified: String,
    pub traditional: String,
    pub pinyin_toned: String,
    pub pinyin_ascii: String,
    pub english: String,
}

/// A compact appendix summary attached to an entry.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RelatedEntry {
    pub source: Source,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub term: String,
    pub chinese: String,
    pub summary: String,
}

/// An exact-entry response.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct EntryResponse {
    pub source: Source,
    pub term: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub aliases: EntryAliases,
    pub content: String,
    pub cross_references: Vec<Link>,
    pub related: Vec<RelatedEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirected_from: Option<String>,
    pub citation: String,
    /// Absolute source file when the data is an extracted directory. A terminal
    /// can open `source_file:line`; a bare `10-c.md#a1RMS` cannot, because VS
    /// Code resolves it against the working directory and then finds nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
    /// 1-based line of the entry in `source_file`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    pub truncated: bool,
    pub total_chars: usize,
}
