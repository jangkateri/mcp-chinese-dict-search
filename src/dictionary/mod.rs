//! Dictionary loading, indexing, search, and exact-entry retrieval.

pub mod appendix;
mod fuzzy;
pub mod index;
pub mod linked;
mod lookup;
pub mod model;
pub mod normalize;
pub mod parse;
pub mod pinyin;
pub mod search;
pub mod source;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::error::DictionaryError;

pub use linked::{LinkReport, build_linked_data};
pub use model::{
    Alias, AnchorTarget, AppendixEntry, AppendixRef, Entry, EntryAliases, EntryRequest,
    EntryResponse, Link, LoadReport, LookupRef, MatchKind, RelatedEntry, Scope, SearchHit,
    SearchRequest, SearchResponse, Section, Source, Suggestion,
};

/// Sidecar written into a linked copy so citations keep the source title.
pub(crate) const TITLE_FILE: &str = "dictionary-title.txt";

/// An immutable in-memory dictionary.
#[derive(Debug, Default)]
pub struct Dictionary {
    /// Display name used in citations and server instructions.
    pub(crate) title: String,
    pub(crate) entries: Vec<Entry>,
    pub(crate) by_headword: HashMap<String, Vec<usize>>,
    pub(crate) anchors: HashMap<String, AnchorTarget>,
    pub(crate) meridians: Vec<AppendixEntry>,
    pub(crate) medicinals: Vec<AppendixEntry>,
    pub(crate) formulas: Vec<AppendixEntry>,
    pub(crate) weights: Vec<Section>,
    pub(crate) point_ids: HashMap<String, usize>,
    pub(crate) appendix_by_id: HashMap<String, AppendixRef>,
    pub(crate) appendix_by_alias: HashMap<String, Vec<AppendixRef>>,
    pub(crate) appendix_by_term: HashMap<String, Vec<AppendixRef>>,
    pub(crate) pinyin_toned: HashMap<String, Vec<LookupRef>>,
    pub(crate) pinyin_toneless: HashMap<String, Vec<LookupRef>>,
    pub(crate) han: HashMap<String, Vec<LookupRef>>,
    /// Absolute directory of an extracted source, for clickable file:line
    /// citations. `None` when the dictionary came from a zip.
    pub(crate) data_root: Option<PathBuf>,
    pub(crate) report: LoadReport,
}

impl Dictionary {
    /// An empty dictionary, used by tests and the protocol layer before loading.
    pub fn empty() -> Self {
        Self {
            title: "Dictionary".to_owned(),
            ..Self::default()
        }
    }

    /// Display name used in citations and server instructions.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Load and index the archive or extracted directory at `path`, caching the
    /// generated lookup indices beside the data.
    pub fn load(path: &Path) -> Result<Self, DictionaryError> {
        Self::load_with_cache(path, None)
    }

    /// Absolute path of a document for a terminal-clickable citation, when the
    /// source is an extracted directory.
    pub(crate) fn source_file(&self, file: &str) -> Option<String> {
        self.data_root
            .as_ref()
            .map(|root| root.join(file).to_string_lossy().into_owned())
    }

    /// Load with an optional cache directory. The cache filename is derived
    /// from the dictionary title and a content fingerprint.
    pub fn load_with_cache(path: &Path, cache_dir: Option<&Path>) -> Result<Self, DictionaryError> {
        Self::load_impl(path, cache_dir, None, false)
    }

    /// Load with an explicit display title and cache behaviour.
    pub fn load_with_options(
        path: &Path,
        cache_dir: Option<&Path>,
        title: Option<&str>,
        rebuild_cache: bool,
    ) -> Result<Self, DictionaryError> {
        Self::load_impl(path, cache_dir, title, rebuild_cache)
    }

    /// Load and rebuild the pinyin and Han lookup indices, replacing the cache
    /// file for the current fingerprint even when a usable one exists.
    pub fn load_rebuilding_cache(
        path: &Path,
        cache_dir: Option<&Path>,
    ) -> Result<Self, DictionaryError> {
        Self::load_impl(path, cache_dir, None, true)
    }

    fn load_impl(
        path: &Path,
        cache_dir: Option<&Path>,
        title: Option<&str>,
        rebuild_cache: bool,
    ) -> Result<Self, DictionaryError> {
        let mut sources = source::read_sources(path)?;
        let mut dictionary = Self {
            title: resolved_title(path, title),
            ..Self::default()
        };
        if path.is_dir() {
            dictionary.data_root =
                Some(std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()));
        }
        dictionary.report.warnings = std::mem::take(&mut sources.warnings);

        for (file, content) in &sources.main {
            let (mut parsed, alias_failures) = parse::parse_entries(file, content);
            dictionary.report.alias_parse_failures += alias_failures;
            dictionary.entries.append(&mut parsed);
        }

        let main_files: HashSet<String> =
            sources.main.iter().map(|(file, _)| file.clone()).collect();
        let mut index_data = sources
            .index
            .as_ref()
            .map(|(_, content)| index::parse_index(content, &main_files))
            .unwrap_or_default();
        dictionary.report.index_anchors = index_data.anchors.len();
        let corrections = index::apply_verified_corrections(&mut index_data, &dictionary.entries);
        dictionary.report.recovered_entry_anchors = corrections.entry_anchors;
        dictionary.report.recovered_link_targets = corrections.link_targets;
        let (resolved, unresolved, anchorless) =
            index::resolve_entries(&mut dictionary.entries, &index_data);
        dictionary.report.resolved_links = resolved;
        dictionary.report.unresolved_links = unresolved;
        dictionary.report.anchorless_entries = anchorless;
        dictionary.anchors = index_data.anchors;

        let anchorless_examples: Vec<String> = dictionary
            .entries
            .iter()
            .filter(|entry| entry.anchor.is_none())
            .take(12)
            .map(|entry| format!("{}:{}", entry.file, entry.headword))
            .collect();
        if !anchorless_examples.is_empty() {
            dictionary.report.warnings.push(format!(
                "{} entries have no index anchor; examples: {}",
                anchorless,
                anchorless_examples.join("; ")
            ));
        }

        for (source, file, content) in &sources.appendices {
            let (entries, failures) = match source {
                Source::Meridian => appendix::parse_meridian(file, content),
                Source::Medicinal => appendix::parse_medicinals(file, content),
                Source::Formulas => appendix::parse_formulas(file, content),
                Source::Weights => (Vec::new(), 0),
                Source::Dictionary => (Vec::new(), 0),
            };
            dictionary.report.appendix_header_failures += failures;
            match source {
                Source::Meridian => dictionary.meridians.extend(entries),
                Source::Medicinal => dictionary.medicinals.extend(entries),
                Source::Formulas => dictionary.formulas.extend(entries),
                _ => {}
            }
        }
        if let Some((file, content)) = &sources.weights {
            dictionary.weights = appendix::parse_weights(file, content);
        }

        dictionary.report.main_entries = dictionary.entries.len();
        dictionary.report.distinct_headwords = dictionary.by_headword.len();
        dictionary.report.meridian_entries = dictionary.meridians.len();
        dictionary.report.medicinal_entries = dictionary.medicinals.len();
        dictionary.report.formula_entries = dictionary.formulas.len();
        dictionary.report.weight_sections = dictionary.weights.len();

        dictionary.build_indexes();
        dictionary.report.distinct_headwords = dictionary.by_headword.len();
        dictionary.report.duplicate_headwords = dictionary
            .report
            .main_entries
            .saturating_sub(dictionary.by_headword.len());

        if dictionary.entries.is_empty() {
            return Err(DictionaryError::no_entries(
                "The archive contained no parseable dictionary entries.",
            ));
        }

        dictionary.load_lookup_indexes(path, &sources, cache_dir, rebuild_cache);
        Ok(dictionary)
    }

    fn build_indexes(&mut self) {
        for (index, entry) in self.entries.iter().enumerate() {
            self.by_headword
                .entry(entry.norm_headword.clone())
                .or_default()
                .push(index);
        }

        for (index, entry) in self.meridians.iter().enumerate() {
            if let Some(id) = &entry.id {
                self.point_ids
                    .entry(normalize::normalize(id))
                    .or_insert(index);
            }
        }

        let appendix_refs: Vec<AppendixRef> = self
            .meridians
            .iter()
            .enumerate()
            .map(|(index, _)| AppendixRef {
                source: Source::Meridian,
                index,
            })
            .chain(
                self.medicinals
                    .iter()
                    .enumerate()
                    .map(|(index, _)| AppendixRef {
                        source: Source::Medicinal,
                        index,
                    }),
            )
            .chain(
                self.formulas
                    .iter()
                    .enumerate()
                    .map(|(index, _)| AppendixRef {
                        source: Source::Formulas,
                        index,
                    }),
            )
            .collect();

        for reference in appendix_refs {
            let Some(entry) = self.appendix_entry(reference) else {
                continue;
            };
            let id = entry.id.clone();
            let term = normalize::normalize(&entry.term);
            let aliases = entry.norm_terms.clone();

            if let Some(id) = id {
                self.appendix_by_id
                    .entry(normalize::normalize(&id))
                    .or_insert(reference);
            }
            if !term.is_empty() {
                self.appendix_by_term
                    .entry(term)
                    .or_default()
                    .push(reference);
            }
            for alias in aliases {
                if !alias.is_empty() {
                    let refs = self.appendix_by_alias.entry(alias).or_default();
                    if !refs.contains(&reference) {
                        refs.push(reference);
                    }
                }
            }
        }
    }

    /// Load the lookup maps from a fingerprinted cache, or build and write one.
    fn load_lookup_indexes(
        &mut self,
        data_path: &Path,
        sources: &source::Sources,
        cache_dir: Option<&Path>,
        rebuild: bool,
    ) {
        let fingerprint = lookup::fingerprint(sources);
        let cache_path = lookup::cache_path(data_path, cache_dir, &fingerprint);
        self.report.lookup_cache_path = Some(cache_path.display().to_string());

        let cached = if rebuild {
            Ok(None)
        } else {
            lookup::load_cache(&cache_path, &fingerprint)
        };
        let indexes = match cached {
            Ok(Some(indexes)) => {
                self.report.lookup_cache_hit = true;
                self.report.lookup_cache_ready = true;
                indexes
            }
            result => {
                if let Err(error) = result {
                    self.report.warnings.push(format!(
                        "Cannot use lookup cache {}; rebuilding: {error}",
                        cache_path.display()
                    ));
                }
                let indexes = lookup::build(
                    &self.entries,
                    &self.meridians,
                    &self.medicinals,
                    &self.formulas,
                );
                match lookup::save_cache(&cache_path, &fingerprint, &indexes, rebuild) {
                    Ok(()) => self.report.lookup_cache_ready = true,
                    Err(error) => self.report.warnings.push(format!(
                        "Cannot write lookup cache {}: {error}",
                        cache_path.display()
                    )),
                }
                indexes
            }
        };

        self.report.pinyin_toned_keys = indexes.pinyin_toned.len();
        self.report.pinyin_toneless_keys = indexes.pinyin_toneless.len();
        self.report.han_keys = indexes.han.len();
        self.pinyin_toned = indexes.pinyin_toned;
        self.pinyin_toneless = indexes.pinyin_toneless;
        self.han = indexes.han;
    }

    pub fn report(&self) -> &LoadReport {
        &self.report
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn appendix_entry(&self, reference: AppendixRef) -> Option<&AppendixEntry> {
        match reference.source {
            Source::Meridian => self.meridians.get(reference.index),
            Source::Medicinal => self.medicinals.get(reference.index),
            Source::Formulas => self.formulas.get(reference.index),
            _ => None,
        }
    }

    pub(crate) fn appendix_entries(&self, scope: Scope) -> Vec<AppendixRef> {
        let mut refs = Vec::new();
        if matches!(scope, Scope::All | Scope::Meridian) {
            refs.extend((0..self.meridians.len()).map(|index| AppendixRef {
                source: Source::Meridian,
                index,
            }));
        }
        if matches!(scope, Scope::All | Scope::Medicinal) {
            refs.extend((0..self.medicinals.len()).map(|index| AppendixRef {
                source: Source::Medicinal,
                index,
            }));
        }
        if matches!(scope, Scope::All | Scope::Formulas) {
            refs.extend((0..self.formulas.len()).map(|index| AppendixRef {
                source: Source::Formulas,
                index,
            }));
        }
        refs
    }

    pub fn search(&self, request: &SearchRequest) -> Result<SearchResponse, DictionaryError> {
        search::search(self, request)
    }

    pub fn entry(&self, request: &EntryRequest) -> Result<EntryResponse, DictionaryError> {
        search::entry(self, request)
    }
}

/// The display title: explicit override, then a linked copy's sidecar, then the
/// data path's own name.
pub(crate) fn resolved_title(path: &Path, override_title: Option<&str>) -> String {
    if let Some(title) = override_title
        .map(str::trim)
        .filter(|title| !title.is_empty())
    {
        return title.to_owned();
    }
    if path.is_dir()
        && let Ok(text) = std::fs::read_to_string(path.join(TITLE_FILE))
    {
        let text = text.trim();
        if !text.is_empty() {
            return text.to_owned();
        }
    }
    let name = if path.is_dir() {
        path.file_name()
    } else {
        path.file_stem().or_else(|| path.file_name())
    };
    name.map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Dictionary".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_dictionary_has_a_zeroed_report() {
        let dictionary = Dictionary::empty();
        assert_eq!(dictionary.entry_count(), 0);
        assert_eq!(dictionary.report().main_entries, 0);
    }
}
