//! Build, load, check, and prune the semantic index for the CLI.

use std::path::{Path, PathBuf};

use crate::dictionary::Dictionary;
use crate::logging::Logger;

use super::SEMANTIC_TOP_K;
use super::build::{BuildResult, DEFAULT_BATCH_SIZE, build_index};
use super::engine::SemanticEngine;
use super::fusion::HybridOptions;
use super::index::{IndexExpectation, SemanticIndex, semantic_cache_path};
use super::ollama::{OllamaClient, OllamaEmbedder};
use super::profiles::EmbeddingProfile;

#[derive(Debug, Clone)]
pub struct ServeOptions<'a> {
    pub data: &'a Path,
    pub cache_dir: Option<&'a Path>,
    pub model: &'a str,
    pub ollama_url: &'a str,
    pub index_override: Option<&'a Path>,
    pub profile_path: Option<&'a Path>,
    pub require: bool,
}

#[derive(Debug, Clone)]
pub struct BuildReport {
    pub model: String,
    pub digest: String,
    pub dimensions: usize,
    pub documents: usize,
    pub long_documents: usize,
    pub index_path: PathBuf,
    pub bytes: u64,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Default)]
pub struct PruneReport {
    pub removed: Vec<PathBuf>,
    /// Present when the command was a dry run.
    pub planned: Vec<PathBuf>,
    pub kept: Vec<PathBuf>,
}

fn rebuild_command(model: &str, profile_path: Option<&Path>) -> String {
    let mut command = format!("--embedding-model {model} --build-semantic-index");
    if let Some(path) = profile_path {
        command.push_str(" --embedding-profile ");
        command.push_str(&path.display().to_string());
    }
    command
}

pub fn resolve_profile(
    model: &str,
    profile_path: Option<&Path>,
) -> Result<EmbeddingProfile, String> {
    let mut profile = EmbeddingProfile::for_model(model);
    if let Some(path) = profile_path {
        profile.apply_override(path)?;
    }
    profile.validate()?;
    Ok(profile)
}

pub fn resolve_index_path(
    data: &Path,
    cache_dir: Option<&Path>,
    dictionary: &Dictionary,
    model: &str,
    digest: &str,
    override_path: Option<&Path>,
) -> PathBuf {
    match override_path {
        Some(path) => path.to_path_buf(),
        None => semantic_cache_path(
            data,
            cache_dir,
            dictionary.content_fingerprint(),
            model,
            digest,
        ),
    }
}

pub fn load_engine(
    dictionary: &Dictionary,
    options: &ServeOptions<'_>,
    logger: &Logger,
) -> Result<Option<SemanticEngine>, String> {
    match load_engine_inner(dictionary, options, logger) {
        Ok(engine) => Ok(Some(engine)),
        Err(error) => {
            if options.require {
                Err(error)
            } else {
                logger.warn(format!(
                    "semantic retrieval is disabled: {error}; continuing with lexical search"
                ));
                Ok(None)
            }
        }
    }
}

fn load_engine_inner(
    dictionary: &Dictionary,
    options: &ServeOptions<'_>,
    logger: &Logger,
) -> Result<SemanticEngine, String> {
    let profile = resolve_profile(options.model, options.profile_path)?;
    let client = OllamaClient::query(options.ollama_url)?;
    let tag = client.model_tag(options.model)?;
    let path = resolve_index_path(
        options.data,
        options.cache_dir,
        dictionary,
        options.model,
        &tag.digest,
        options.index_override,
    );
    if !path.exists() {
        return Err(format!(
            "the semantic index for {} digest {} is missing at {}; build it with `{}`",
            options.model,
            tag.digest,
            path.display(),
            rebuild_command(options.model, options.profile_path)
        ));
    }
    let index = SemanticIndex::load(&path)?;
    let dimensions = client.dimensions(options.model, &profile)?;
    index.validate_expectation(&IndexExpectation {
        package_version: env!("CARGO_PKG_VERSION").to_owned(),
        dictionary_fingerprint: dictionary.content_fingerprint().to_owned(),
        model_name: options.model.to_owned(),
        model_digest: tag.digest.clone(),
        dimensions,
        profile_hash: profile.hash(),
    })?;
    logger.info(format!(
        "semantic: model {}, digest {}, {} dimensions, {} records, index {}",
        options.model,
        tag.digest,
        dimensions,
        index.records.len(),
        path.display()
    ));
    let embedder = OllamaEmbedder::new(client.clone(), options.model, profile.clone(), dimensions);
    Ok(SemanticEngine::new(
        embedder,
        index,
        HybridOptions {
            min_similarity: profile.min_similarity,
            top_k: SEMANTIC_TOP_K,
        },
        logger.clone(),
        client,
        options.model,
        tag.digest,
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn build(
    dictionary: &Dictionary,
    data: &Path,
    cache_dir: Option<&Path>,
    model: &str,
    ollama_url: &str,
    profile_path: Option<&Path>,
    index_override: Option<&Path>,
    logger: &Logger,
) -> Result<BuildReport, String> {
    let profile = resolve_profile(model, profile_path)?;
    let client = OllamaClient::build(ollama_url)?;
    let tag = client.model_tag(model)?;
    let embedder = OllamaEmbedder::connect(client, model, profile.clone())?;
    let result: BuildResult = build_index(
        dictionary,
        &embedder,
        model,
        &tag.digest,
        &profile,
        DEFAULT_BATCH_SIZE,
        |done, total| {
            if done == 0 || done == total || done % 256 == 0 {
                logger.info(format!("semantic index: {done}/{total} documents"));
            }
        },
    )?;
    let path = resolve_index_path(
        data,
        cache_dir,
        dictionary,
        model,
        &tag.digest,
        index_override,
    );
    result.index.save(&path)?;
    let bytes = std::fs::metadata(&path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    logger.info(format!(
        "built semantic index: model {}, digest {}, {} dimensions, {} records, {} bytes, {} ms, \
         index {}",
        model,
        tag.digest,
        result.dimensions,
        result.documents,
        bytes,
        result.elapsed_ms,
        path.display()
    ));
    if result.long_documents > 0 {
        logger.warn(format!(
            "{} documents exceed the profile's {} character guard; Ollama will truncate what \
             exceeds their context window",
            result.long_documents, profile.max_input_chars
        ));
    }
    Ok(BuildReport {
        model: model.to_owned(),
        digest: tag.digest,
        dimensions: result.dimensions,
        documents: result.documents,
        long_documents: result.long_documents,
        index_path: path,
        bytes,
        elapsed_ms: result.elapsed_ms,
    })
}

pub fn check(
    dictionary: &Dictionary,
    data: &Path,
    cache_dir: Option<&Path>,
    model: &str,
    ollama_url: &str,
    profile_path: Option<&Path>,
    index_override: Option<&Path>,
) -> Result<String, String> {
    let profile = resolve_profile(model, profile_path)?;
    let client = OllamaClient::query(ollama_url)?;
    let tag = client.model_tag(model)?;
    let path = resolve_index_path(
        data,
        cache_dir,
        dictionary,
        model,
        &tag.digest,
        index_override,
    );
    let mut lines = vec![
        format!("model: {model}"),
        format!("digest: {}", tag.digest),
        format!("profile: {} (hash {})", profile.family, profile.hash()),
        format!("expected index: {}", path.display()),
    ];
    if !path.exists() {
        lines.push("index: missing".to_owned());
        lines.push(format!(
            "remediation: run {}",
            rebuild_command(model, profile_path)
        ));
        return Ok(lines.join("\n"));
    }
    let index = SemanticIndex::load(&path)?;
    lines.push(format!("index records: {}", index.records.len()));
    lines.push(format!("index dimensions: {}", index.metadata.dimensions));
    match client.dimensions(model, &profile) {
        Ok(dimensions) => {
            lines.push(format!("model dimensions: {dimensions}"));
            match index.validate_expectation(&IndexExpectation {
                package_version: env!("CARGO_PKG_VERSION").to_owned(),
                dictionary_fingerprint: dictionary.content_fingerprint().to_owned(),
                model_name: model.to_owned(),
                model_digest: tag.digest.clone(),
                dimensions,
                profile_hash: profile.hash(),
            }) {
                Ok(()) => lines.push("status: active".to_owned()),
                Err(error) => {
                    lines.push("status: stale".to_owned());
                    lines.push(format!(
                        "remediation: rebuild the index for this model and profile: {error}"
                    ));
                }
            }
        }
        Err(error) => {
            lines.push(format!("model probe: failed: {error}"));
            lines.push("status: degraded".to_owned());
            lines.push(format!(
                "remediation: check the Ollama server and run `ollama pull {model}`"
            ));
        }
    }
    Ok(lines.join("\n"))
}

#[allow(clippy::too_many_arguments)]
pub fn prune(
    dictionary: &Dictionary,
    data: &Path,
    cache_dir: Option<&Path>,
    model: &str,
    ollama_url: &str,
    probe_profile: Option<&Path>,
    index_override: Option<&Path>,
    yes: bool,
) -> Result<PruneReport, String> {
    let _ = probe_profile;
    let client = OllamaClient::query(ollama_url)?;
    // The current digest identifies the only index this command may keep.
    let tag = client.model_tag(model)?;
    let current = resolve_index_path(
        data,
        cache_dir,
        dictionary,
        model,
        &tag.digest,
        index_override,
    );
    let directory = current.parent().unwrap_or_else(|| Path::new("."));
    let stem = data
        .file_stem()
        .or_else(|| data.file_name())
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "dictionary".to_owned());
    let prefix = format!("{stem}.semantic-cache-v");
    let mut report = PruneReport::default();
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("cannot list {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read an index entry: {error}"))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(&prefix) || !name.ends_with(".bin") {
            continue;
        }
        if path == current {
            report.kept.push(path);
        } else if yes {
            std::fs::remove_file(&path)
                .map_err(|error| format!("cannot remove {}: {error}", path.display()))?;
            report.removed.push(path);
        } else {
            report.planned.push(path);
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::Dictionary;
    use crate::logging::Logger;

    fn tags_server(
        model: &'static str,
        digest: &'static str,
    ) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 4096];
            let _ = stream.read(&mut buffer);
            let body = format!(r#"{{"models":[{{"name":"{model}","digest":"{digest}"}}]}}"#);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: \
                 {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        });
        (url, handle)
    }

    #[test]
    fn pruning_lists_by_default_and_deletes_only_with_yes() {
        let directory =
            std::env::temp_dir().join(format!("mcp-chinese-dict-prune-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let data = directory.join("dictionary");
        std::fs::write(&data, b"x").unwrap();
        let current = semantic_cache_path(&data, Some(&directory), "", "model", "digest");
        let other = directory.join("dictionary.semantic-cache-v1.other.model.other.bin");
        std::fs::write(&current, b"current").unwrap();
        std::fs::write(&other, b"other").unwrap();

        let (url, handle) = tags_server("model", "digest");
        let dictionary = Dictionary::empty();
        let planned = prune(
            &dictionary,
            &data,
            Some(&directory),
            "model",
            &url,
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(planned.planned, vec![other.clone()]);
        assert!(planned.removed.is_empty());
        assert!(other.exists());

        let (url, handle_two) = tags_server("model", "digest");
        let removed = prune(
            &dictionary,
            &data,
            Some(&directory),
            "model",
            &url,
            None,
            None,
            true,
        )
        .unwrap();
        assert_eq!(removed.removed, vec![other.clone()]);
        assert!(!other.exists());
        assert!(current.exists());
        handle.join().unwrap();
        handle_two.join().unwrap();
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_missing_index_degrades_or_fails_with_a_rebuild_hint() {
        let dictionary = Dictionary::empty();
        let logger = Logger::new(false);
        let (url, first_handle) = tags_server("model", "digest");
        let options = ServeOptions {
            data: Path::new("dictionary"),
            cache_dir: None,
            model: "model",
            ollama_url: &url,
            index_override: Some(Path::new("definitely-missing-index.bin")),
            profile_path: None,
            require: false,
        };
        assert!(
            load_engine(&dictionary, &options, &logger)
                .unwrap()
                .is_none()
        );
        first_handle.join().unwrap();

        let (url, second_handle) = tags_server("model", "digest");
        let required = ServeOptions {
            data: Path::new("dictionary"),
            cache_dir: None,
            model: "model",
            ollama_url: &url,
            index_override: Some(Path::new("definitely-missing-index.bin")),
            profile_path: None,
            require: true,
        };
        let error = match load_engine(&dictionary, &required, &logger) {
            Ok(_) => panic!("a required missing index must be an error"),
            Err(error) => error,
        };
        assert!(error.contains("build it with"), "{error}");
        second_handle.join().unwrap();
    }

    #[test]
    fn unreachable_ollama_degrades_by_default_and_fails_when_required() {
        let dictionary = Dictionary::empty();
        let logger = Logger::new(false);
        let options = ServeOptions {
            data: Path::new("dictionary"),
            cache_dir: None,
            model: "model",
            ollama_url: "http://127.0.0.1:1",
            index_override: None,
            profile_path: None,
            require: false,
        };
        assert!(
            load_engine(&dictionary, &options, &logger)
                .unwrap()
                .is_none()
        );
        let required = ServeOptions {
            require: true,
            ..options
        };
        assert!(load_engine(&dictionary, &required, &logger).is_err());
    }
}
