//! Generated pinyin- and Han-keyed lookup indices.
//!
//! The indices are built from the user-supplied archive and cached in a JSON
//! file named `<dictionary title>.lookup-cache-v2.<fingerprint>.json`. A later
//! startup reuses that file when the fingerprint, package version, and cache
//! version match; otherwise it rebuilds and replaces the cache. The cache is
//! derived from copyrighted source data and must stay out of git and `dist/`.
//!
//! The indices are the primary lookup path for pinyin and Chinese queries; the
//! ordinary ranked search remains the fallback for English text and for pinyin
//! that is not an exact alias.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::dictionary::model::{AppendixEntry, Entry, LookupRef, Source};
use crate::dictionary::pinyin;
use crate::dictionary::source::Sources;

/// Bump this when the index layout or key semantics change. It is part of the
/// cache filename, so an old cache is never silently reused.
const CACHE_VERSION: u32 = 2;
const CACHE_MAX_BYTES: u64 = 64 * 1024 * 1024;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x100_0000_01b3;

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct LookupIndexes {
    /// Exact tone-numbered pinyin (`wei4`, `jia3 gen1`) to entries.
    pub pinyin_toned: HashMap<String, Vec<LookupRef>>,
    /// Toneless pinyin (`wei`, `jia gen`) to entries.
    pub pinyin_toneless: HashMap<String, Vec<LookupRef>>,
    /// Exact Han string (`为`, `為`, `為根`) to entries.
    pub han: HashMap<String, Vec<LookupRef>>,
}

#[derive(Debug, Deserialize)]
struct LookupCache {
    version: u32,
    package_version: String,
    fingerprint: String,
    pinyin_toned: HashMap<String, Vec<LookupRef>>,
    pinyin_toneless: HashMap<String, Vec<LookupRef>>,
    han: HashMap<String, Vec<LookupRef>>,
}

#[derive(Serialize)]
struct LookupCacheRef<'a> {
    version: u32,
    package_version: &'a str,
    fingerprint: &'a str,
    pinyin_toned: &'a HashMap<String, Vec<LookupRef>>,
    pinyin_toneless: &'a HashMap<String, Vec<LookupRef>>,
    han: &'a HashMap<String, Vec<LookupRef>>,
}

/// A stable content fingerprint for the Markdown that produces the indices.
///
/// FNV-1a is used as a cache key, not as a security primitive. The cache is
/// regenerated whenever the fingerprint changes.
pub(crate) fn fingerprint(sources: &Sources) -> String {
    let mut documents: Vec<(&str, &str)> = Vec::new();
    for (name, content) in &sources.main {
        documents.push((name, content));
    }
    for (_, name, content) in &sources.appendices {
        documents.push((name, content));
    }
    if let Some((name, content)) = &sources.weights {
        documents.push((name, content));
    }
    if let Some((name, content)) = &sources.index {
        documents.push((name, content));
    }
    documents.sort_by(|left, right| left.0.cmp(right.0).then_with(|| left.1.cmp(right.1)));

    let mut hash = FNV_OFFSET;
    hash_bytes(
        &mut hash,
        format!("lookup-cache-v{CACHE_VERSION}").as_bytes(),
    );
    hash_bytes(&mut hash, &[0]);
    for (name, content) in documents {
        hash_bytes(&mut hash, name.as_bytes());
        hash_bytes(&mut hash, &[0]);
        hash_bytes(&mut hash, content.as_bytes());
        hash_bytes(&mut hash, &[0]);
    }
    format!("{hash:016x}")
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

/// The cache path is `<dictionary name>.lookup-cache-v2.<fingerprint>.json`,
/// placed beside the archive/directory unless a cache directory is configured.
pub(crate) fn cache_path(data: &Path, cache_dir: Option<&Path>, fingerprint: &str) -> PathBuf {
    let title = data
        .file_stem()
        .or_else(|| data.file_name())
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "dictionary".to_owned());
    let name = format!("{title}.lookup-cache-v{CACHE_VERSION}.{fingerprint}.json");
    match cache_dir {
        Some(directory) => directory.join(name),
        None => data.parent().unwrap_or_else(|| Path::new(".")).join(name),
    }
}

/// Load a cache file. `Ok(None)` means the file is absent or for another
/// version/content; `Err` means a cache exists but could not be used.
pub(crate) fn load_cache(path: &Path, fingerprint: &str) -> Result<Option<LookupIndexes>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Cannot inspect {}: {error}", path.display()))?;
    if metadata.len() > CACHE_MAX_BYTES {
        return Err(format!(
            "Cache {} is larger than {} bytes.",
            path.display(),
            CACHE_MAX_BYTES
        ));
    }
    let bytes =
        fs::read(path).map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
    let cache: LookupCache = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Cannot parse {}: {error}", path.display()))?;

    if cache.version != CACHE_VERSION
        || cache.package_version != env!("CARGO_PKG_VERSION")
        || cache.fingerprint != fingerprint
    {
        return Ok(None);
    }

    Ok(Some(LookupIndexes {
        pinyin_toned: cache.pinyin_toned,
        pinyin_toneless: cache.pinyin_toneless,
        han: cache.han,
    }))
}

/// Write the cache atomically and prune older fingerprint files for the same
/// dictionary title and cache version. With `replace`, an existing file for
/// this fingerprint is overwritten even when it is still usable.
pub(crate) fn save_cache(
    path: &Path,
    fingerprint: &str,
    indexes: &LookupIndexes,
    replace: bool,
) -> Result<(), String> {
    let cache = LookupCacheRef {
        version: CACHE_VERSION,
        package_version: env!("CARGO_PKG_VERSION"),
        fingerprint,
        pinyin_toned: &indexes.pinyin_toned,
        pinyin_toneless: &indexes.pinyin_toneless,
        han: &indexes.han,
    };
    let bytes = serde_json::to_vec(&cache)
        .map_err(|error| format!("Cannot serialize lookup cache: {error}"))?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Cannot create {}: {error}", parent.display()))?;
    }
    let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
    fs::write(&temporary, &bytes)
        .map_err(|error| format!("Cannot write {}: {error}", temporary.display()))?;

    if path.exists() {
        if !replace && matches!(load_cache(path, fingerprint), Ok(Some(_))) {
            // Another process won the race; its file is equivalent.
            let _ = fs::remove_file(&temporary);
            return Ok(());
        }
        // Replace an invalid, stale, or deliberately rebuilt cache.
        fs::remove_file(path)
            .map_err(|error| format!("Cannot replace {}: {error}", path.display()))?;
    }
    match fs::rename(&temporary, path) {
        Ok(()) => {}
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(format!("Cannot install {}: {error}", path.display()));
        }
    }

    prune_old_caches(path);
    Ok(())
}

fn prune_old_caches(current: &Path) {
    let Some(parent) = current.parent() else {
        return;
    };
    let Some(name) = current.file_name().and_then(|value| value.to_str()) else {
        return;
    };
    let Some(title) = name.split(".lookup-cache-v").next() else {
        return;
    };
    let prefix = format!("{title}.lookup-cache-v{CACHE_VERSION}.");
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == current {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if file_name.starts_with(&prefix) && file_name.ends_with(".json") {
            let _ = fs::remove_file(path);
        }
    }
}

pub(crate) fn build(
    entries: &[Entry],
    meridians: &[AppendixEntry],
    medicinals: &[AppendixEntry],
    formulas: &[AppendixEntry],
) -> LookupIndexes {
    let mut indexes = LookupIndexes::default();

    for (index, entry) in entries.iter().enumerate() {
        let target = LookupRef {
            source: Source::Dictionary,
            index,
        };
        for alias in &entry.aliases {
            add_pinyin(&mut indexes, &alias.pinyin_toned, target);
            add_pinyin(&mut indexes, &alias.pinyin_ascii, target);
            add_han_field(&mut indexes, &alias.simplified, target);
            add_han_field(&mut indexes, &alias.traditional, target);
        }
        // A few headwords are themselves pinyin (`wei2`, `Wei2`). Index only those,
        // so English headwords do not pollute the pinyin keys and turn up as
        // `constipation (constipation)` suggestions.
        if pinyin::toned_key(&entry.headword).is_some() {
            add_pinyin(&mut indexes, &entry.headword, target);
        }
    }

    for (source, entries) in [
        (Source::Meridian, meridians),
        (Source::Medicinal, medicinals),
        (Source::Formulas, formulas),
    ] {
        for (index, entry) in entries.iter().enumerate() {
            let target = LookupRef { source, index };
            add_pinyin(&mut indexes, &entry.pinyin_toned, target);
            add_pinyin(&mut indexes, &entry.pinyin_ascii, target);
            add_han_field(&mut indexes, &entry.simplified, target);
            add_han_field(&mut indexes, &entry.traditional, target);
            for alias in &entry.aliases {
                add_han_alias(&mut indexes, alias, target);
            }
        }
    }

    indexes
}

fn add_pinyin(indexes: &mut LookupIndexes, text: &str, target: LookupRef) {
    if text.is_empty() {
        return;
    }
    let toneless = pinyin::toneless_key(text);
    if let Some(toned) = pinyin::toned_key(text) {
        push(&mut indexes.pinyin_toned, toned, target);
        push(&mut indexes.pinyin_toneless, toneless, target);
    } else if !toneless.is_empty() {
        push(&mut indexes.pinyin_toneless, toneless, target);
    }
}

fn add_han_field(indexes: &mut LookupIndexes, text: &str, target: LookupRef) {
    let key = pinyin::han_key(text);
    if !key.is_empty() {
        push(&mut indexes.han, key, target);
    }
}

fn add_han_alias(indexes: &mut LookupIndexes, text: &str, target: LookupRef) {
    for run in pinyin::han_runs(text) {
        push(&mut indexes.han, run, target);
    }
}

fn push(map: &mut HashMap<String, Vec<LookupRef>>, key: String, target: LookupRef) {
    if key.is_empty() {
        return;
    }
    let references = map.entry(key).or_default();
    if !references.contains(&target) {
        references.push(target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::model::Alias;

    fn entry(headword: &str, aliases: Vec<Alias>) -> Entry {
        Entry {
            headword: headword.to_owned(),
            norm_headword: headword.to_ascii_lowercase(),
            file: "08-a.md".to_owned(),
            line: 1,
            anchor: None,
            aliases,
            body: String::new(),
            norm_body: String::new(),
            links: Vec::new(),
        }
    }

    fn alias(simplified: &str, traditional: &str, toned: &str, ascii: &str) -> Alias {
        Alias {
            simplified: simplified.to_owned(),
            traditional: traditional.to_owned(),
            pinyin_toned: toned.to_owned(),
            pinyin_ascii: ascii.to_owned(),
        }
    }

    #[test]
    fn pinyin_and_han_aliases_are_indexed_both_ways() {
        let indexes = build(
            &[entry("wei2", vec![alias("为", "為", "wei2", "wéi")])],
            &[],
            &[],
            &[],
        );

        let target = LookupRef {
            source: Source::Dictionary,
            index: 0,
        };
        assert_eq!(indexes.pinyin_toned["wei2"], vec![target]);
        assert_eq!(indexes.pinyin_toneless["wei"], vec![target]);
        assert_eq!(indexes.han["为"], vec![target]);
        assert_eq!(indexes.han["為"], vec![target]);
    }

    #[test]
    fn english_headwords_stay_out_of_the_pinyin_index() {
        let indexes = build(&[entry("constipation", Vec::new())], &[], &[], &[]);
        assert!(indexes.pinyin_toned.is_empty());
        assert!(indexes.pinyin_toneless.is_empty());
    }

    #[test]
    fn fingerprint_tracks_content_and_ignores_appendix_order() {
        let first = Sources {
            main: vec![(
                "08-a.md".to_owned(),
                "\\*one\n\n为 | 為 *wei2 / wéi\n".to_owned(),
            )],
            appendices: vec![
                (
                    Source::Meridian,
                    "34-appendix-1-Meridian.md".to_owned(),
                    "AA-1".to_owned(),
                ),
                (
                    Source::Medicinal,
                    "35-appendix-2-Medicinal.md".to_owned(),
                    "甲根".to_owned(),
                ),
            ],
            ..Sources::default()
        };
        let same = Sources {
            main: first.main.clone(),
            appendices: first.appendices.iter().rev().cloned().collect(),
            ..Sources::default()
        };
        let changed = Sources {
            main: vec![(
                "08-a.md".to_owned(),
                "\\*one\n\n为 | 為 *wei3 / wěi\n".to_owned(),
            )],
            appendices: first.appendices.clone(),
            ..Sources::default()
        };

        assert_eq!(fingerprint(&first), fingerprint(&same));
        assert_ne!(fingerprint(&first), fingerprint(&changed));
        assert_eq!(fingerprint(&first).len(), 16);
    }

    #[test]
    fn cache_path_uses_the_dictionary_title_and_fingerprint() {
        let beside = cache_path(Path::new("dictionary.zip"), None, "deadbeef");
        assert_eq!(
            beside.file_name().unwrap().to_string_lossy(),
            "dictionary.lookup-cache-v2.deadbeef.json"
        );

        let configured = cache_path(Path::new("dict.zip"), Some(Path::new("cache")), "deadbeef");
        assert_eq!(
            configured,
            PathBuf::from("cache").join("dict.lookup-cache-v2.deadbeef.json")
        );
    }

    #[test]
    fn cache_round_trip_preserves_all_three_indices() {
        let directory = std::env::temp_dir().join(format!(
            "mcp-chinese-dict-lookup-cache-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("temp cache directory is created");

        let indexes = build(
            &[entry("wei2", vec![alias("为", "為", "wei2", "wéi")])],
            &[],
            &[],
            &[],
        );
        let path = directory.join("dictionary.lookup-cache-v2.test.json");
        save_cache(&path, "test", &indexes, false).expect("cache is written");
        let loaded = load_cache(&path, "test")
            .expect("cache is readable")
            .expect("cache is present");

        assert_eq!(loaded.pinyin_toned.len(), indexes.pinyin_toned.len());
        assert_eq!(loaded.pinyin_toneless.len(), indexes.pinyin_toneless.len());
        assert_eq!(loaded.han.len(), indexes.han.len());
        assert_eq!(loaded.han["为"], indexes.han["为"]);

        fs::write(&path, b"not a cache").expect("corrupt cache is written");
        save_cache(&path, "test", &indexes, false).expect("corrupt cache is replaced");
        assert!(
            load_cache(&path, "test")
                .expect("replaced cache is readable")
                .is_some()
        );

        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn toneless_appendices_do_not_create_a_toned_key() {
        let entry = AppendixEntry {
            source: Source::Medicinal,
            file: "35-appendix-2-Medicinal.md".to_owned(),
            line: 1,
            id: Some("01.01.01".to_owned()),
            section: String::new(),
            subsection: None,
            term: "wei root".to_owned(),
            simplified: "为根".to_owned(),
            traditional: "為根".to_owned(),
            pinyin_toned: "WÉI GĒN".to_owned(),
            pinyin_ascii: "wei gen".to_owned(),
            aliases: vec!["为根".to_owned()],
            body: String::new(),
            norm_terms: Vec::new(),
            norm_body: String::new(),
        };
        let indexes = build(&[], &[], &[entry], &[]);
        let target = LookupRef {
            source: Source::Medicinal,
            index: 0,
        };
        assert_eq!(indexes.pinyin_toned["wei2 gen1"], vec![target]);
        assert_eq!(indexes.pinyin_toneless["wei gen"], vec![target]);
        assert_eq!(indexes.han["为根"], vec![target]);
        assert_eq!(indexes.han["為根"], vec![target]);
    }
}
