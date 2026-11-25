//! Read the dictionary from a zip archive or an extracted directory.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::dictionary::model::Source;
use crate::error::DictionaryError;

/// Raw Markdown documents grouped by role.
#[derive(Debug, Default)]
pub struct Sources {
    pub main: Vec<(String, String)>,
    pub appendices: Vec<(Source, String, String)>,
    pub weights: Option<(String, String)>,
    pub index: Option<(String, String)>,
    pub warnings: Vec<String>,
}

/// Read every relevant Markdown document from `path`.
pub fn read_sources(path: &Path) -> Result<Sources, DictionaryError> {
    let documents = if path.is_dir() {
        read_directory(path)?
    } else if path.is_file() {
        read_zip(path)?
    } else {
        return Err(DictionaryError::unreadable(format!(
            "Cannot read the dictionary at {}: the path does not exist.",
            path.display()
        )));
    };

    let mut sources = Sources::default();
    for (name, content) in documents {
        let file_name = Path::new(&name)
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or(name.clone());
        if is_main_name(&file_name) {
            sources.main.push((file_name, content));
        } else if let Some(source) = appendix_source(&file_name) {
            if source == Source::Weights {
                sources.weights = Some((file_name, content));
            } else {
                sources.appendices.push((source, file_name, content));
            }
        } else if file_name.starts_with("38-") {
            sources.index = Some((file_name, content));
        }
    }

    sources.main.sort_by(|left, right| left.0.cmp(&right.0));
    if sources.main.is_empty() {
        return Err(DictionaryError::no_entries(format!(
            "No letter files (08-a.md … 33-z.md) were found in {}.",
            path.display()
        )));
    }

    if sources.index.is_none() {
        sources
            .warnings
            .push("38-index.md is missing; cross-references will not resolve.".to_owned());
    }
    if !sources
        .appendices
        .iter()
        .any(|(source, _, _)| *source == Source::Meridian)
    {
        sources
            .warnings
            .push("Appendix 1 (Meridian) was not found.".to_owned());
    }
    if !sources
        .appendices
        .iter()
        .any(|(source, _, _)| *source == Source::Medicinal)
    {
        sources
            .warnings
            .push("Appendix 2 (Medicinal) was not found.".to_owned());
    }
    if !sources
        .appendices
        .iter()
        .any(|(source, _, _)| *source == Source::Formulas)
    {
        sources
            .warnings
            .push("Appendix 3 (Formulas) was not found.".to_owned());
    }
    if sources.weights.is_none() {
        sources
            .warnings
            .push("Appendix 4 (Weights and Measures) was not found.".to_owned());
    }

    Ok(sources)
}

/// Every Markdown document in the archive, front matter included, in file-name
/// order. Used by the one-time linked-copy build, which must preserve the whole
/// set rather than only the files the server parses.
pub fn read_all_documents(path: &Path) -> Result<Vec<(String, String)>, DictionaryError> {
    let mut documents = if path.is_dir() {
        read_directory(path)?
    } else if path.is_file() {
        read_zip(path)?
    } else {
        return Err(DictionaryError::unreadable(format!(
            "Cannot read the dictionary at {}: the path does not exist.",
            path.display()
        )));
    };
    documents.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(documents)
}

fn read_directory(path: &Path) -> Result<Vec<(String, String)>, DictionaryError> {
    let mut documents = Vec::new();
    let entries = std::fs::read_dir(path).map_err(|error| {
        DictionaryError::unreadable(format!(
            "Cannot read the directory {}: {error}",
            path.display()
        ))
    })?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    paths.sort();

    for file_path in paths {
        let name = file_path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        let content = std::fs::read_to_string(&file_path).map_err(|error| {
            DictionaryError::unreadable(format!("Cannot read {}: {error}", file_path.display()))
        })?;
        documents.push((name, content));
    }
    Ok(documents)
}

fn read_zip(path: &Path) -> Result<Vec<(String, String)>, DictionaryError> {
    let file = File::open(path).map_err(|error| {
        DictionaryError::unreadable(format!("Cannot open {}: {error}", path.display()))
    })?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        DictionaryError::unreadable(format!(
            "Cannot read {} as a zip archive: {error}",
            path.display()
        ))
    })?;

    let mut documents = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            DictionaryError::unreadable(format!("Cannot read zip entry {index}: {error}"))
        })?;
        if entry.is_dir() {
            continue;
        }
        let name = entry
            .name()
            .map_err(|error| {
                DictionaryError::unreadable(format!("Cannot read zip entry {index}: {error}"))
            })?
            .into_owned();
        if !name.ends_with(".md") {
            continue;
        }
        let mut content = String::new();
        entry.read_to_string(&mut content).map_err(|error| {
            DictionaryError::unreadable(format!("Cannot read {name} as UTF-8 text: {error}"))
        })?;
        documents.push((name, content));
    }
    Ok(documents)
}

fn is_main_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() == 7
        && bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && bytes[2] == b'-'
        && bytes[3].is_ascii_lowercase()
        && &bytes[4..] == b".md"
}

fn appendix_source(name: &str) -> Option<Source> {
    if name.starts_with("34-") {
        Some(Source::Meridian)
    } else if name.starts_with("35-") {
        Some(Source::Medicinal)
    } else if name.starts_with("36-") {
        Some(Source::Formulas)
    } else if name.starts_with("37-") {
        Some(Source::Weights)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_letter_files_only() {
        assert!(is_main_name("08-a.md"));
        assert!(is_main_name("33-z.md"));
        assert!(!is_main_name("38-index.md"));
        assert!(!is_main_name("34-appendix-1-Meridian.md"));
        assert!(!is_main_name("08-a.MD"));
    }

    #[test]
    fn classifies_appendix_prefixes() {
        assert_eq!(
            appendix_source("34-appendix-1-Meridian.md"),
            Some(Source::Meridian)
        );
        assert_eq!(
            appendix_source("35-appendix-2-Medicinal.md"),
            Some(Source::Medicinal)
        );
        assert_eq!(
            appendix_source("36-appendix-3-Formulas.md"),
            Some(Source::Formulas)
        );
        assert_eq!(
            appendix_source("37-appendix-4-Weights-Measurements.md"),
            Some(Source::Weights)
        );
        assert_eq!(appendix_source("03-table-of-contents.md"), None);
    }

    #[test]
    fn invalid_utf8_and_empty_directories_are_reported() {
        let base = std::env::temp_dir().join(format!(
            "mcp-chinese-dict-source-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("temp directory is created");

        let error = read_sources(&base).unwrap_err();
        assert!(error.to_string().contains("No letter files"));

        std::fs::write(base.join("08-a.md"), [0xff, 0xfe]).expect("invalid file is written");
        let error = read_sources(&base).unwrap_err();
        assert!(error.to_string().contains("Cannot read"));

        let _ = std::fs::remove_dir_all(&base);
    }
}
