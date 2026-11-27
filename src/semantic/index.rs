//! Flat cosine index, metadata header, and the digest-keyed cache path.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::dictionary::{LookupRef, Scope, Source};

pub const MAGIC: [u8; 8] = *b"MCDSEM01";
pub const FORMAT_VERSION: u32 = 1;
pub const MAX_INDEX_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_DIMENSIONS: usize = 4096;
pub const MAX_RECORDS: usize = 100_000;
pub const MAX_HEADER_BYTES: usize = 64 * 1024;
pub const MAX_STRING_BYTES: usize = 4096;
const NORM_TOLERANCE: f32 = 0.02;

/// The identity and dimensions an index was built with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexMetadata {
    /// Informational only; not part of index compatibility.
    pub package_version: String,
    pub document_version: u32,
    pub dictionary_fingerprint: String,
    pub model_name: String,
    pub model_digest: String,
    pub dimensions: usize,
    pub profile_hash: String,
}

/// The identity a serving process expects an index to have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexExpectation {
    pub document_version: u32,
    pub dictionary_fingerprint: String,
    pub model_name: String,
    pub model_digest: String,
    pub dimensions: usize,
    pub profile_hash: String,
}

/// One stored vector and the entry it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticRecord {
    pub reference: LookupRef,
    pub vector: Vec<f32>,
}

/// One cosine match.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticMatch {
    pub reference: LookupRef,
    pub similarity: f32,
}

/// A loaded, validated flat semantic index.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticIndex {
    pub metadata: IndexMetadata,
    pub records: Vec<SemanticRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HeaderJson {
    package_version: String,
    /// Missing in indexes written before 0.6.1; those were document version 1.
    #[serde(default = "default_document_version")]
    document_version: u32,
    dictionary_fingerprint: String,
    model_name: String,
    model_digest: String,
    dimensions: u32,
    count: u32,
    profile_hash: String,
}

fn default_document_version() -> u32 {
    1
}

impl SemanticIndex {
    pub fn new(metadata: IndexMetadata, records: Vec<SemanticRecord>) -> Result<Self, String> {
        let index = Self { metadata, records };
        index.validate()?;
        Ok(index)
    }

    pub fn build(
        metadata: IndexMetadata,
        documents: &[super::document::SemanticDocument],
        vectors: Vec<Vec<f32>>,
    ) -> Result<Self, String> {
        if documents.len() != vectors.len() {
            return Err(format!(
                "{} documents but {} vectors",
                documents.len(),
                vectors.len()
            ));
        }
        let records = documents
            .iter()
            .cloned()
            .zip(vectors)
            .map(|(document, vector)| SemanticRecord {
                reference: document.reference,
                vector,
            })
            .collect();
        Self::new(metadata, records)
    }

    pub fn validate(&self) -> Result<(), String> {
        let dimensions = self.metadata.dimensions;
        if !(1..=MAX_DIMENSIONS).contains(&dimensions) {
            return Err(format!(
                "dimensions {dimensions} are outside 1..={MAX_DIMENSIONS}"
            ));
        }
        if self.records.len() > MAX_RECORDS {
            return Err(format!(
                "{} records exceed the {MAX_RECORDS}-record limit",
                self.records.len()
            ));
        }
        for (name, value) in [
            ("package_version", &self.metadata.package_version),
            (
                "dictionary_fingerprint",
                &self.metadata.dictionary_fingerprint,
            ),
            ("model_name", &self.metadata.model_name),
            ("model_digest", &self.metadata.model_digest),
            ("profile_hash", &self.metadata.profile_hash),
        ] {
            validate_string(name, value)?;
        }

        let mut seen = HashSet::with_capacity(self.records.len());
        for record in &self.records {
            validate_vector(&record.vector, dimensions)?;
            if !seen.insert(record.reference) {
                return Err(format!(
                    "duplicate record for {}:{}",
                    record.reference.source.as_str(),
                    record.reference.index
                ));
            }
        }
        Ok(())
    }

    /// Reject an index built for another source, model, document version, or profile.
    pub fn validate_expectation(&self, expected: &IndexExpectation) -> Result<(), String> {
        let actual = &self.metadata;
        for (name, actual, expected) in [
            (
                "dictionary fingerprint",
                actual.dictionary_fingerprint.as_str(),
                expected.dictionary_fingerprint.as_str(),
            ),
            (
                "model name",
                actual.model_name.as_str(),
                expected.model_name.as_str(),
            ),
            (
                "model digest",
                actual.model_digest.as_str(),
                expected.model_digest.as_str(),
            ),
            (
                "profile hash",
                actual.profile_hash.as_str(),
                expected.profile_hash.as_str(),
            ),
        ] {
            if actual != expected {
                return Err(format!(
                    "the index {name} is {actual:?}, but this server expects {expected:?}; rebuild \
                     the semantic index"
                ));
            }
        }
        if self.metadata.document_version != expected.document_version {
            return Err(format!(
                "the index document version is {}, but this server expects {}; rebuild the \
                 semantic index",
                self.metadata.document_version, expected.document_version
            ));
        }
        if self.metadata.dimensions != expected.dimensions {
            return Err(format!(
                "the index has {} dimensions, but this server expects {}; rebuild the semantic \
                 index",
                self.metadata.dimensions, expected.dimensions
            ));
        }
        Ok(())
    }

    /// Return the highest-cosine records above `min_similarity`, deterministically ordered.
    pub fn search(
        &self,
        query: &[f32],
        limit: usize,
        min_similarity: f32,
    ) -> Result<Vec<SemanticMatch>, String> {
        self.search_filtered(query, limit, min_similarity, |_| true)
    }

    /// Search only records whose source is allowed by `scope`.
    ///
    /// Filtering happens before top-k selection, so a scoped query returns the
    /// best candidates from that source rather than whatever remains of a
    /// global top-k list.
    pub fn search_scoped(
        &self,
        query: &[f32],
        limit: usize,
        min_similarity: f32,
        scope: Scope,
    ) -> Result<Vec<SemanticMatch>, String> {
        self.search_filtered(query, limit, min_similarity, |reference| {
            scope.allows(reference.source)
        })
    }

    fn search_filtered(
        &self,
        query: &[f32],
        limit: usize,
        min_similarity: f32,
        allowed: impl Fn(LookupRef) -> bool,
    ) -> Result<Vec<SemanticMatch>, String> {
        if query.len() != self.metadata.dimensions {
            return Err(format!(
                "query has {} dimensions but the index has {}",
                query.len(),
                self.metadata.dimensions
            ));
        }
        if !query.iter().all(|value| value.is_finite()) {
            return Err("the query contains NaN or infinity".to_owned());
        }
        let query_norm = query
            .iter()
            .map(|value| f64::from(*value) * f64::from(*value))
            .sum::<f64>()
            .sqrt();
        if query_norm == 0.0 {
            return Err("the query vector has zero norm".to_owned());
        }

        let mut matches: Vec<SemanticMatch> = self
            .records
            .iter()
            .filter(|record| allowed(record.reference))
            .filter_map(|record| {
                let dot = record
                    .vector
                    .iter()
                    .zip(query)
                    .map(|(left, right)| f64::from(*left) * f64::from(*right))
                    .sum::<f64>();
                let similarity = (dot / query_norm) as f32;
                (similarity.is_finite() && similarity >= min_similarity).then_some(SemanticMatch {
                    reference: record.reference,
                    similarity,
                })
            })
            .collect();

        matches.sort_by(|left, right| {
            right
                .similarity
                .partial_cmp(&left.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| lookup_order(left.reference).cmp(&lookup_order(right.reference)))
        });
        matches.truncate(limit);
        Ok(matches)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let header = HeaderJson {
            package_version: self.metadata.package_version.clone(),
            document_version: self.metadata.document_version,
            dictionary_fingerprint: self.metadata.dictionary_fingerprint.clone(),
            model_name: self.metadata.model_name.clone(),
            model_digest: self.metadata.model_digest.clone(),
            dimensions: u32::try_from(self.metadata.dimensions)
                .map_err(|_| "dimensions do not fit in u32".to_owned())?,
            count: u32::try_from(self.records.len())
                .map_err(|_| "record count does not fit in u32".to_owned())?,
            profile_hash: self.metadata.profile_hash.clone(),
        };
        let header_bytes = serde_json::to_vec(&header)
            .map_err(|error| format!("cannot serialize the index header: {error}"))?;
        if header_bytes.len() > MAX_HEADER_BYTES {
            return Err(format!(
                "the index header is {} bytes, over the {MAX_HEADER_BYTES}-byte limit",
                header_bytes.len()
            ));
        }

        let record_size = record_size(self.metadata.dimensions)?;
        let total = 8usize
            .checked_add(4)
            .and_then(|value| value.checked_add(4))
            .and_then(|value| value.checked_add(header_bytes.len()))
            .and_then(|value| value.checked_add(record_size.checked_mul(self.records.len())?))
            .ok_or_else(|| "the index size overflows usize".to_owned())?;
        ensure_size(total as u64)?;

        let mut bytes = Vec::with_capacity(total);
        bytes.extend_from_slice(&MAGIC);
        bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        bytes.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&header_bytes);
        for record in &self.records {
            bytes.push(source_byte(record.reference.source));
            bytes.extend_from_slice(&(record.reference.index as u32).to_le_bytes());
            for value in &record.vector {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        ensure_size(bytes.len() as u64)?;
        let mut cursor = Cursor::new(bytes);
        if cursor.read_exact(8)? != MAGIC {
            return Err("the index magic does not match".to_owned());
        }
        let version = cursor.read_u32()?;
        if version != FORMAT_VERSION {
            return Err(format!(
                "unsupported index format version {version}; expected {FORMAT_VERSION}"
            ));
        }
        let header_len = cursor.read_u32()? as usize;
        if header_len > MAX_HEADER_BYTES {
            return Err(format!(
                "the index header is {header_len} bytes, over the {MAX_HEADER_BYTES}-byte limit"
            ));
        }
        let header_bytes = cursor.read_exact(header_len)?;
        let header: HeaderJson = serde_json::from_slice(header_bytes)
            .map_err(|error| format!("cannot parse the index header: {error}"))?;
        validate_string("package_version", &header.package_version)?;
        validate_string("dictionary_fingerprint", &header.dictionary_fingerprint)?;
        validate_string("model_name", &header.model_name)?;
        validate_string("model_digest", &header.model_digest)?;
        validate_string("profile_hash", &header.profile_hash)?;

        let dimensions = header.dimensions as usize;
        if !(1..=MAX_DIMENSIONS).contains(&dimensions) {
            return Err(format!(
                "dimensions {dimensions} are outside 1..={MAX_DIMENSIONS}"
            ));
        }
        let count = header.count as usize;
        if count > MAX_RECORDS {
            return Err(format!(
                "{count} records exceed the {MAX_RECORDS}-record limit"
            ));
        }
        let record_size = record_size(dimensions)?;
        let expected = cursor
            .position()
            .checked_add(
                record_size
                    .checked_mul(count)
                    .ok_or_else(|| "the record count overflows the index size".to_owned())?,
            )
            .ok_or_else(|| "the index size overflows usize".to_owned())?;
        if expected != bytes.len() {
            return Err(format!(
                "the index header declares {count} records, but the file length is {} bytes",
                bytes.len()
            ));
        }

        let mut records = Vec::with_capacity(count);
        for _ in 0..count {
            let source = source_from_byte(cursor.read_u8()?)?;
            let index = cursor.read_u32()? as usize;
            let mut vector = Vec::with_capacity(dimensions);
            for _ in 0..dimensions {
                vector.push(cursor.read_f32()?);
            }
            records.push(SemanticRecord {
                reference: LookupRef { source, index },
                vector,
            });
        }
        if !cursor.is_empty() {
            return Err("the index has trailing bytes".to_owned());
        }

        Self::new(
            IndexMetadata {
                package_version: header.package_version,
                document_version: header.document_version,
                dictionary_fingerprint: header.dictionary_fingerprint,
                model_name: header.model_name,
                model_digest: header.model_digest,
                dimensions,
                profile_hash: header.profile_hash,
            },
            records,
        )
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let metadata = fs::metadata(path)
            .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
        ensure_size(metadata.len()).map_err(|error| format!("{}: {error}", path.display()))?;
        let bytes =
            fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        Self::from_bytes(&bytes).map_err(|error| format!("cannot load {}: {error}", path.display()))
    }

    /// Write atomically: a temporary file in the same directory is renamed over
    /// the target, and an existing target is kept if the rename fails.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let bytes = self.to_bytes()?;
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }

        let temporary = temporary_path(path);
        fs::write(&temporary, &bytes)
            .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;

        if !path.exists() {
            return fs::rename(&temporary, path).map_err(|error| {
                let _ = fs::remove_file(&temporary);
                format!("cannot rename the index into place: {error}")
            });
        }

        let backup = backup_path(path);
        let _ = fs::remove_file(&backup);
        if let Err(error) = fs::rename(path, &backup) {
            let _ = fs::remove_file(&temporary);
            return Err(format!(
                "cannot move the old index aside at {}: {error}",
                path.display()
            ));
        }
        match fs::rename(&temporary, path) {
            Ok(()) => {
                let _ = fs::remove_file(&backup);
                Ok(())
            }
            Err(error) => {
                let _ = fs::rename(&backup, path);
                let _ = fs::remove_file(&temporary);
                Err(format!("cannot rename the index into place: {error}"))
            }
        }
    }
}

/// The digest-keyed cache path for one model and dictionary fingerprint.
pub fn semantic_cache_path(
    data: &Path,
    cache_dir: Option<&Path>,
    fingerprint: &str,
    model_name: &str,
    model_digest: &str,
) -> PathBuf {
    let stem = data
        .file_stem()
        .or_else(|| data.file_name())
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "dictionary".to_owned());
    let model = sanitize_component(model_name);
    let digest_source = model_digest.rsplit(':').next().unwrap_or(model_digest);
    let digest: String = digest_source
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .take(16)
        .collect();
    let digest = if digest.is_empty() {
        "nohash".to_owned()
    } else {
        digest
    };
    let name =
        format!("{stem}.semantic-cache-v{FORMAT_VERSION}.{fingerprint}.{model}.{digest}.bin");
    match cache_dir {
        Some(directory) => directory.join(name),
        None => data.parent().unwrap_or_else(|| Path::new(".")).join(name),
    }
}

fn ensure_size(length: u64) -> Result<(), String> {
    if length > MAX_INDEX_BYTES {
        Err(format!(
            "the index is {length} bytes, over the {MAX_INDEX_BYTES}-byte limit"
        ))
    } else {
        Ok(())
    }
}

fn record_size(dimensions: usize) -> Result<usize, String> {
    1usize
        .checked_add(4)
        .and_then(|value| value.checked_add(dimensions.checked_mul(4)?))
        .ok_or_else(|| "the record size overflows usize".to_owned())
}

fn validate_string(name: &str, value: &str) -> Result<(), String> {
    if value.len() > MAX_STRING_BYTES {
        return Err(format!(
            "{name} is {} bytes, over the {MAX_STRING_BYTES}-byte limit",
            value.len()
        ));
    }
    Ok(())
}

fn validate_vector(vector: &[f32], dimensions: usize) -> Result<(), String> {
    if vector.len() != dimensions {
        return Err(format!(
            "vector has {} dimensions but the index declares {dimensions}",
            vector.len()
        ));
    }
    if !vector.iter().all(|value| value.is_finite()) {
        return Err("the vector contains NaN or infinity".to_owned());
    }
    let norm = vector
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || (norm - 1.0).abs() > f64::from(NORM_TOLERANCE) {
        return Err(format!("the vector norm is {norm}, not close to 1"));
    }
    Ok(())
}

fn source_byte(source: Source) -> u8 {
    match source {
        Source::Dictionary => 0,
        Source::Meridian => 1,
        Source::Medicinal => 2,
        Source::Formulas => 3,
        Source::Weights => 4,
    }
}

fn source_from_byte(byte: u8) -> Result<Source, String> {
    match byte {
        0 => Ok(Source::Dictionary),
        1 => Ok(Source::Meridian),
        2 => Ok(Source::Medicinal),
        3 => Ok(Source::Formulas),
        4 => Ok(Source::Weights),
        _ => Err(format!("unknown source byte {byte}")),
    }
}

fn lookup_order(reference: LookupRef) -> (u8, usize) {
    (source_byte(reference.source), reference.index)
}

fn sanitize_component(value: &str) -> String {
    let mut out = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            out.push(character.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_owned();
    if out.is_empty() {
        "model".to_owned()
    } else {
        out
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension(format!("bin.{}.tmp", std::process::id()))
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension(format!("bin.{}.bak", std::process::id()))
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn position(&self) -> usize {
        self.position
    }

    fn is_empty(&self) -> bool {
        self.position == self.bytes.len()
    }

    fn read_exact(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| "the index offset overflows usize".to_owned())?;
        if end > self.bytes.len() {
            return Err("the index is truncated".to_owned());
        }
        let slice = &self.bytes[self.position..end];
        self.position = end;
        Ok(slice)
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        let bytes = self.read_exact(1)?;
        Ok(bytes[0])
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes(
            bytes.try_into().map_err(|_| "short u32".to_owned())?,
        ))
    }

    fn read_f32(&mut self) -> Result<f32, String> {
        let bytes = self.read_exact(4)?;
        Ok(f32::from_le_bytes(
            bytes.try_into().map_err(|_| "short f32".to_owned())?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::LookupRef;
    use crate::semantic::DOCUMENT_VERSION;
    use std::path::Path;

    fn metadata(dimensions: usize) -> IndexMetadata {
        IndexMetadata {
            package_version: "0.5.0".to_owned(),
            document_version: DOCUMENT_VERSION,
            dictionary_fingerprint: "fingerprint".to_owned(),
            model_name: "test-model".to_owned(),
            model_digest: "sha256:0123456789abcdef".to_owned(),
            dimensions,
            profile_hash: "profile".to_owned(),
        }
    }

    fn record(source: Source, index: usize, vector: Vec<f32>) -> SemanticRecord {
        SemanticRecord {
            reference: LookupRef { source, index },
            vector,
        }
    }

    fn unit(values: &[f32]) -> Vec<f32> {
        let norm = values.iter().map(|v| v * v).sum::<f32>().sqrt();
        values.iter().map(|v| v / norm).collect()
    }

    fn sample() -> SemanticIndex {
        SemanticIndex::new(
            metadata(2),
            vec![
                record(Source::Dictionary, 0, unit(&[1.0, 0.0])),
                record(Source::Medicinal, 0, unit(&[0.0, 1.0])),
            ],
        )
        .unwrap()
    }

    #[test]
    fn cosine_search_orders_by_similarity_then_source() {
        let index = sample();
        let matches = index.search(&unit(&[0.9, 0.1]), 10, 0.0).unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].reference.source, Source::Dictionary);
        assert!(matches[0].similarity > matches[1].similarity);
    }

    #[test]
    fn search_respects_the_threshold_and_limit() {
        let index = sample();
        let matches = index.search(&unit(&[1.0, 0.0]), 10, 0.5).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].similarity, 1.0);
        assert_eq!(index.search(&unit(&[1.0, 0.0]), 1, 0.0).unwrap().len(), 1);
    }

    #[test]
    fn scoped_search_filters_records_before_top_k() {
        let index = sample();
        let matches = index
            .search_scoped(&unit(&[1.0, 0.0]), 10, 0.0, Scope::Medicinal)
            .unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].reference.source, Source::Medicinal);
        assert!(
            index
                .search_scoped(&unit(&[1.0, 0.0]), 10, 0.0, Scope::Meridian)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn round_trip_preserves_header_and_records() {
        let index = sample();
        let bytes = index.to_bytes().unwrap();
        let loaded = SemanticIndex::from_bytes(&bytes).unwrap();
        assert_eq!(index, loaded);
    }

    #[test]
    fn save_writes_atomically_and_leaves_no_temporary_file() {
        let directory =
            std::env::temp_dir().join(format!("mcp-chinese-dict-semantic-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("index.bin");
        sample().save(&path).unwrap();
        assert!(path.exists());
        assert!(!temporary_path(&path).exists());
        assert!(!backup_path(&path).exists());
        assert_eq!(SemanticIndex::load(&path).unwrap(), sample());
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_new_digest_keyed_path_leaves_the_old_index_loadable() {
        let directory = std::env::temp_dir().join(format!(
            "mcp-chinese-dict-semantic-two-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let old = directory.join("old-model-index.bin");
        let new = directory.join("new-model-index.bin");
        sample().save(&old).unwrap();
        sample().save(&new).unwrap();
        assert_eq!(SemanticIndex::load(&old).unwrap(), sample());
        assert_eq!(SemanticIndex::load(&new).unwrap(), sample());
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_failed_serialization_leaves_the_old_index_intact() {
        let directory = std::env::temp_dir().join(format!(
            "mcp-chinese-dict-semantic-fail-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("index.bin");
        sample().save(&path).unwrap();
        let old = fs::read(&path).unwrap();

        let invalid = SemanticIndex {
            metadata: metadata(2),
            records: vec![record(Source::Dictionary, 0, vec![f32::NAN, 0.0])],
        };
        assert!(invalid.save(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), old);
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn corrupt_headers_and_records_are_rejected() {
        let index = sample();
        let bytes = index.to_bytes().unwrap();

        let mut wrong_magic = bytes.clone();
        wrong_magic[0] = b'X';
        assert!(SemanticIndex::from_bytes(&wrong_magic).is_err());

        let mut wrong_version = bytes.clone();
        wrong_version[8..12].copy_from_slice(&99u32.to_le_bytes());
        assert!(SemanticIndex::from_bytes(&wrong_version).is_err());

        assert!(SemanticIndex::from_bytes(&bytes[..bytes.len() - 1]).is_err());

        let mut oversized_header = bytes.clone();
        oversized_header[12..16].copy_from_slice(&(MAX_HEADER_BYTES as u32 + 1).to_le_bytes());
        assert!(SemanticIndex::from_bytes(&oversized_header).is_err());

        let mut wrong_source = bytes.clone();
        let header_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let first_record = 16 + header_len;
        wrong_source[first_record] = 99;
        assert!(SemanticIndex::from_bytes(&wrong_source).is_err());

        let mut nan = bytes.clone();
        nan[first_record + 5..first_record + 9].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(SemanticIndex::from_bytes(&nan).is_err());
    }

    #[test]
    fn invalid_vectors_are_rejected() {
        assert!(
            SemanticIndex::new(
                metadata(2),
                vec![record(Source::Dictionary, 0, vec![1.0, 0.0, 0.0])]
            )
            .is_err()
        );
        assert!(
            SemanticIndex::new(
                metadata(2),
                vec![record(Source::Dictionary, 0, vec![0.0, 0.0])]
            )
            .is_err()
        );
        assert!(
            SemanticIndex::new(
                metadata(2),
                vec![record(Source::Dictionary, 0, vec![f32::INFINITY, 0.0])]
            )
            .is_err()
        );
        assert!(
            SemanticIndex::new(
                metadata(2),
                vec![
                    record(Source::Dictionary, 0, vec![1.0, 0.0]),
                    record(Source::Dictionary, 0, vec![0.0, 1.0]),
                ]
            )
            .is_err()
        );
    }

    fn expectation() -> IndexExpectation {
        IndexExpectation {
            document_version: DOCUMENT_VERSION,
            dictionary_fingerprint: "fingerprint".to_owned(),
            model_name: "test-model".to_owned(),
            model_digest: "sha256:0123456789abcdef".to_owned(),
            dimensions: 2,
            profile_hash: "profile".to_owned(),
        }
    }

    #[test]
    fn file_size_and_dimension_bounds_are_rejected() {
        assert!(ensure_size(MAX_INDEX_BYTES).is_ok());
        assert!(ensure_size(MAX_INDEX_BYTES + 1).is_err());
        assert!(SemanticIndex::new(metadata(0), Vec::new()).is_err());
        assert!(SemanticIndex::new(metadata(MAX_DIMENSIONS + 1), Vec::new()).is_err());
    }

    #[test]
    fn identity_mismatches_are_rejected_with_a_rebuild_hint() {
        let index = sample();
        assert!(index.validate_expectation(&expectation()).is_ok());

        let mutations: [fn(&mut IndexExpectation); 6] = [
            |expected: &mut IndexExpectation| {
                expected.dictionary_fingerprint = "other".to_owned();
            },
            |expected: &mut IndexExpectation| {
                expected.model_name = "other".to_owned();
            },
            |expected: &mut IndexExpectation| {
                expected.model_digest = "other".to_owned();
            },
            |expected: &mut IndexExpectation| {
                expected.dimensions = 3;
            },
            |expected: &mut IndexExpectation| {
                expected.profile_hash = "other".to_owned();
            },
            |expected: &mut IndexExpectation| {
                expected.document_version = DOCUMENT_VERSION + 1;
            },
        ];
        for mutate in mutations {
            let mut expected = expectation();
            mutate(&mut expected);
            let error = index.validate_expectation(&expected).unwrap_err();
            assert!(error.contains("rebuild"));
        }
    }

    #[test]
    fn package_version_is_diagnostic_only() {
        let index = sample();
        assert_ne!(index.metadata.package_version, env!("CARGO_PKG_VERSION"));
        assert!(index.validate_expectation(&expectation()).is_ok());
    }

    #[test]
    fn a_header_count_that_does_not_match_the_records_is_rejected() {
        let index = sample();
        let mut bytes = index.to_bytes().unwrap();
        let header_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let header_start = 16;
        let header = std::str::from_utf8(&bytes[header_start..header_start + header_len])
            .unwrap()
            .to_owned();
        let changed = header.replace("\"count\":2", "\"count\":3");
        assert_ne!(header, changed);
        bytes[header_start..header_start + header_len].copy_from_slice(changed.as_bytes());
        assert!(SemanticIndex::from_bytes(&bytes).is_err());
    }

    #[test]
    fn cache_path_includes_the_model_name_and_digest_prefix() {
        let path = semantic_cache_path(
            Path::new("dictionary.zip"),
            None,
            "deadbeef",
            "Qwen3-Embedding:0.6b",
            "sha256:0123456789abcdef0123",
        );
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            "dictionary.semantic-cache-v1.deadbeef.qwen3-embedding-0-6b.0123456789abcdef.bin"
        );
        let configured = semantic_cache_path(
            Path::new("dict.zip"),
            Some(Path::new("cache")),
            "deadbeef",
            "model",
            "sha256:0123",
        );
        assert_eq!(
            configured,
            Path::new("cache").join("dict.semantic-cache-v1.deadbeef.model.0123.bin")
        );
    }
}
