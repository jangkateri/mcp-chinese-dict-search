//! Command-line flags and environment configuration.
//!
//! Configuration comes from flags first, then environment variables, then
//! built-in defaults. The dictionary is local data, so the only required
//! setting is a path to the archive or its extracted directory.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::ConfigError;
use crate::server::{SERVER_NAME, SERVER_VERSION};

/// Parsed flags before validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub help: bool,
    pub version: bool,
    pub verbose: bool,
    pub build_indices: bool,
    pub build_linked_data: String,
    pub data: String,
    /// Display name used in citations; empty means "derive from the data path".
    pub title: String,
    pub log_file: String,
    pub cache_dir: String,
    pub pages_dir: String,
    pub ollama_url: String,
    pub embedding_model: String,
    pub embedding_profile: String,
    pub semantic_index: String,
    pub require_semantic: bool,
    pub build_semantic_index: bool,
    pub check_semantic: bool,
    pub prune_semantic_index: bool,
    pub yes: bool,
    pub errors: Vec<String>,
}

/// Fully resolved, validated configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub data: PathBuf,
    /// `None` means "derive from the data path".
    pub title: Option<String>,
    /// Empty means "stderr only".
    pub log_file: String,
    /// `None` means "beside the dictionary".
    pub cache_dir: Option<PathBuf>,
    /// `None` means "write no citation pages".
    pub pages_dir: Option<PathBuf>,
    /// `Some` means "build a linked copy here and exit without serving".
    pub build_linked_data: Option<PathBuf>,
    /// Loopback Ollama base URL.
    pub ollama_url: String,
    /// `Some` enables gated hybrid retrieval.
    pub embedding_model: Option<String>,
    pub embedding_profile: Option<PathBuf>,
    /// Override the derived digest-keyed semantic index path.
    pub semantic_index: Option<PathBuf>,
    pub require_semantic: bool,
    pub build_semantic_index: bool,
    pub check_semantic: bool,
    pub prune_semantic_index: bool,
    pub yes: bool,
    pub verbose: bool,
}

impl Config {
    pub fn log_file_path(&self) -> Option<&str> {
        (!self.log_file.is_empty()).then_some(self.log_file.as_str())
    }
}

/// `--help` text, printed to stdout before any MCP client is attached.
pub fn usage() -> String {
    format!(
        "{SERVER_NAME} {SERVER_VERSION} — MCP server for a local Markdown dictionary

Usage:
  {SERVER_NAME} --data <path> [options]

Options:
  --data <path>        Dictionary .zip or extracted directory (required;
                       env MCP_CHINESE_DICT_DATA)
  --title <text>       Display name used in citations (default: derived from the
                       data path, or a dictionary-title.txt in a linked copy;
                       env MCP_CHINESE_DICT_TITLE)
  --log-file <path>    Append diagnostics to this file as well as stderr
                       (env MCP_CHINESE_DICT_LOG_FILE); useful because MCP clients
                       usually hide a stdio server's stderr
  --cache-dir <path>   Directory for the generated lookup cache (default: beside
                       the dictionary; env MCP_CHINESE_DICT_CACHE_DIR)
  --pages-dir <path>   Write a Markdown page per get_entry result into this directory and
                       name it in the result, so the citation can be clicked; choose a
                       path without spaces (env MCP_CHINESE_DICT_PAGES_DIR)
  --build-indices      Build the pinyin and Han lookup indices, write the cache,
                       and exit without serving stdio
  --build-linked-data <dir>
                       Copy the Markdown set into <dir>, insert the index anchors
                       missing from the conversion, verify entries and links, and
                       exit without serving stdio; point --data at the copy
  --ollama-url <url>   Loopback Ollama base URL (default: http://127.0.0.1:11434;
                       env MCP_CHINESE_DICT_OLLAMA_URL)
  --embedding-model <name>
                       Ollama embedding model; supplying it enables gated hybrid
                       retrieval (env MCP_CHINESE_DICT_EMBEDDING_MODEL)
  --embedding-profile <path>
                       Optional JSON override for the model profile
                       (env MCP_CHINESE_DICT_EMBEDDING_PROFILE)
  --semantic-index <path>
                       Override the derived digest-keyed semantic index path
                       (env MCP_CHINESE_DICT_SEMANTIC_INDEX)
  --build-semantic-index
                       Build a semantic index for the configured model and exit;
                       requires --embedding-model
  --check-semantic     Report model, digest, dimensions, index path, and status;
                       never writes
  --prune-semantic-index
                       List or, with --yes, remove semantic indexes that do not
                       match the current model and digest
  --require-semantic   Fail startup when the model or index is unavailable
                       (env MCP_CHINESE_DICT_REQUIRE_SEMANTIC)
  --yes                Confirm a pruning operation
  --verbose            Log request handling, timing, and the load report
  -h, --help           Print this help and exit
  --version            Print the version and exit

Tools published over stdio:
  search_dictionary(query, scope?, limit?)   Ranked dictionary and appendix hits
  get_entry(term, source?, max_chars?)       One exact entry with cross-references
  related_entries(term, direction?, depth?, limit?)
                                             Follow resolved cross-references
  render_graph(term, direction?, depth?, limit?)
                                             Render the graph as a PNG image
  related_network(term, domain, direction?, depth?, limit?)
                                             Formula, medicinal, or point-combination network
  render_network(term, domain, direction?, depth?, limit?)
                                             Render an appendix network as a PNG"
    )
}

/// Parse command-line arguments.
pub fn parse_args(args: &[String], env: &HashMap<String, String>) -> Options {
    let mut options = Options {
        help: false,
        version: false,
        verbose: false,
        build_indices: false,
        build_linked_data: String::new(),
        data: env
            .get("MCP_CHINESE_DICT_DATA")
            .map(|value| value.trim().to_owned())
            .unwrap_or_default(),
        title: env
            .get("MCP_CHINESE_DICT_TITLE")
            .map(|value| value.trim().to_owned())
            .unwrap_or_default(),
        log_file: env
            .get("MCP_CHINESE_DICT_LOG_FILE")
            .map(|value| value.trim().to_owned())
            .unwrap_or_default(),
        cache_dir: env
            .get("MCP_CHINESE_DICT_CACHE_DIR")
            .map(|value| value.trim().to_owned())
            .unwrap_or_default(),
        pages_dir: env
            .get("MCP_CHINESE_DICT_PAGES_DIR")
            .map(|value| value.trim().to_owned())
            .unwrap_or_default(),
        ollama_url: env
            .get("MCP_CHINESE_DICT_OLLAMA_URL")
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "http://127.0.0.1:11434".to_owned()),
        embedding_model: env
            .get("MCP_CHINESE_DICT_EMBEDDING_MODEL")
            .map(|value| value.trim().to_owned())
            .unwrap_or_default(),
        embedding_profile: env
            .get("MCP_CHINESE_DICT_EMBEDDING_PROFILE")
            .map(|value| value.trim().to_owned())
            .unwrap_or_default(),
        semantic_index: env
            .get("MCP_CHINESE_DICT_SEMANTIC_INDEX")
            .map(|value| value.trim().to_owned())
            .unwrap_or_default(),
        require_semantic: env
            .get("MCP_CHINESE_DICT_REQUIRE_SEMANTIC")
            .is_some_and(|value| truthy(value)),
        build_semantic_index: false,
        check_semantic: false,
        prune_semantic_index: false,
        yes: false,
        errors: Vec::new(),
    };

    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        match argument {
            "-h" | "--help" => options.help = true,
            "--version" => options.version = true,
            "--verbose" => options.verbose = true,
            "--build-indices" => options.build_indices = true,
            "--build-linked-data" => match args.get(index + 1) {
                Some(value) => {
                    options.build_linked_data = value.clone();
                    index += 1;
                }
                None => options
                    .errors
                    .push("--build-linked-data requires a value".to_owned()),
            },
            "--data" => match args.get(index + 1) {
                Some(value) => {
                    options.data = value.clone();
                    index += 1;
                }
                None => options.errors.push("--data requires a value".to_owned()),
            },
            "--title" => match args.get(index + 1) {
                Some(value) => {
                    options.title = value.clone();
                    index += 1;
                }
                None => options.errors.push("--title requires a value".to_owned()),
            },
            "--log-file" => match args.get(index + 1) {
                Some(value) => {
                    options.log_file = value.clone();
                    index += 1;
                }
                None => options
                    .errors
                    .push("--log-file requires a value".to_owned()),
            },
            "--cache-dir" => match args.get(index + 1) {
                Some(value) => {
                    options.cache_dir = value.clone();
                    index += 1;
                }
                None => options
                    .errors
                    .push("--cache-dir requires a value".to_owned()),
            },
            "--pages-dir" => match args.get(index + 1) {
                Some(value) => {
                    options.pages_dir = value.clone();
                    index += 1;
                }
                None => options
                    .errors
                    .push("--pages-dir requires a value".to_owned()),
            },
            "--ollama-url" => match args.get(index + 1) {
                Some(value) => {
                    options.ollama_url = value.clone();
                    index += 1;
                }
                None => options
                    .errors
                    .push("--ollama-url requires a value".to_owned()),
            },
            "--embedding-model" => match args.get(index + 1) {
                Some(value) => {
                    options.embedding_model = value.clone();
                    index += 1;
                }
                None => options
                    .errors
                    .push("--embedding-model requires a value".to_owned()),
            },
            "--embedding-profile" => match args.get(index + 1) {
                Some(value) => {
                    options.embedding_profile = value.clone();
                    index += 1;
                }
                None => options
                    .errors
                    .push("--embedding-profile requires a value".to_owned()),
            },
            "--semantic-index" => match args.get(index + 1) {
                Some(value) => {
                    options.semantic_index = value.clone();
                    index += 1;
                }
                None => options
                    .errors
                    .push("--semantic-index requires a value".to_owned()),
            },
            "--require-semantic" => options.require_semantic = true,
            "--build-semantic-index" => options.build_semantic_index = true,
            "--check-semantic" => options.check_semantic = true,
            "--prune-semantic-index" => options.prune_semantic_index = true,
            "--yes" => options.yes = true,
            other => options.errors.push(format!("unknown argument {other:?}")),
        }
        index += 1;
    }

    options
}

/// Normalize and validate the parsed options, resolving the data path.
pub fn resolve_config(options: &Options) -> Result<Config, ConfigError> {
    let build_modes = usize::from(options.build_indices)
        + usize::from(!options.build_linked_data.trim().is_empty())
        + usize::from(options.build_semantic_index);
    if build_modes > 1 {
        return Err(ConfigError(
            "Choose one build mode: --build-indices, --build-linked-data, or \
             --build-semantic-index."
                .to_owned(),
        ));
    }

    let embedding_model = (!options.embedding_model.trim().is_empty())
        .then(|| options.embedding_model.trim().to_owned());
    if options.build_semantic_index && embedding_model.is_none() {
        return Err(ConfigError(
            "--build-semantic-index requires --embedding-model.".to_owned(),
        ));
    }
    if (options.check_semantic || options.prune_semantic_index) && embedding_model.is_none() {
        return Err(ConfigError(
            "This semantic operation requires --embedding-model.".to_owned(),
        ));
    }
    if options.require_semantic && embedding_model.is_none() {
        return Err(ConfigError(
            "--require-semantic requires --embedding-model.".to_owned(),
        ));
    }
    let uses_semantic = embedding_model.is_some()
        || options.build_semantic_index
        || options.check_semantic
        || options.prune_semantic_index;
    if uses_semantic && !cfg!(feature = "semantic") {
        return Err(ConfigError(
            "This binary was built without the semantic feature. Rebuild with `cargo build \
             --release --features semantic` to use --embedding-model, --build-semantic-index, \
             --check-semantic, or --prune-semantic-index."
                .to_owned(),
        ));
    }
    if embedding_model.is_some() && !is_loopback_url(&options.ollama_url) {
        return Err(ConfigError(format!(
            "The Ollama URL {:?} is not loopback. Only http://127.0.0.1, http://localhost, or \
             http://[::1] is accepted.",
            options.ollama_url
        )));
    }

    if options.data.trim().is_empty() {
        return Err(ConfigError(
            "No dictionary. Pass --data <zip-or-directory> or set MCP_CHINESE_DICT_DATA."
                .to_owned(),
        ));
    }
    let data = PathBuf::from(options.data.trim());

    if !data.exists() {
        return Err(ConfigError(format!(
            "Cannot read the dictionary at {}: the path does not exist.",
            data.display()
        )));
    }
    if !data.is_file() && !data.is_dir() {
        return Err(ConfigError(format!(
            "Cannot read the dictionary at {}: expected a .zip file or a directory.",
            data.display()
        )));
    }

    let cache_dir =
        (!options.cache_dir.trim().is_empty()).then(|| PathBuf::from(options.cache_dir.trim()));
    let pages_dir =
        (!options.pages_dir.trim().is_empty()).then(|| PathBuf::from(options.pages_dir.trim()));
    let build_linked_data = (!options.build_linked_data.trim().is_empty())
        .then(|| PathBuf::from(options.build_linked_data.trim()));

    let title = (!options.title.trim().is_empty()).then(|| options.title.trim().to_owned());
    let embedding_profile = (!options.embedding_profile.trim().is_empty())
        .then(|| PathBuf::from(options.embedding_profile.trim()));
    let semantic_index = (!options.semantic_index.trim().is_empty())
        .then(|| PathBuf::from(options.semantic_index.trim()));

    Ok(Config {
        data,
        title,
        log_file: options.log_file.clone(),
        cache_dir,
        pages_dir,
        build_linked_data,
        ollama_url: options.ollama_url.trim().trim_end_matches('/').to_owned(),
        embedding_model,
        embedding_profile,
        semantic_index,
        require_semantic: options.require_semantic,
        build_semantic_index: options.build_semantic_index,
        check_semantic: options.check_semantic,
        prune_semantic_index: options.prune_semantic_index,
        yes: options.yes,
        verbose: options.verbose,
    })
}

fn truthy(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "no" | "off"
    )
}

fn is_loopback_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    let Some(rest) = lower
        .strip_prefix("http://")
        .or_else(|| lower.strip_prefix("https://"))
    else {
        return false;
    };
    if rest.starts_with("[::1]") {
        return true;
    }
    let host = rest.split([':', '/']).next().unwrap_or("");
    matches!(host, "127.0.0.1" | "localhost")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn defaults_come_from_the_environment() {
        let mut env = HashMap::new();
        env.insert("MCP_CHINESE_DICT_DATA".to_owned(), "dictionary".to_owned());
        env.insert(
            "MCP_CHINESE_DICT_TITLE".to_owned(),
            "My Dictionary".to_owned(),
        );
        env.insert(
            "MCP_CHINESE_DICT_LOG_FILE".to_owned(),
            "server.log".to_owned(),
        );
        env.insert("MCP_CHINESE_DICT_CACHE_DIR".to_owned(), "cache".to_owned());
        env.insert("MCP_CHINESE_DICT_PAGES_DIR".to_owned(), "pages".to_owned());

        let options = parse_args(&[], &env);
        assert_eq!(options.data, "dictionary");
        assert_eq!(options.title, "My Dictionary");
        assert_eq!(options.log_file, "server.log");
        assert_eq!(options.cache_dir, "cache");
        assert_eq!(options.pages_dir, "pages");
        assert!(options.errors.is_empty());
    }

    #[test]
    fn flags_override_the_environment() {
        let mut env = HashMap::new();
        env.insert("MCP_CHINESE_DICT_DATA".to_owned(), "old".to_owned());
        let options = parse_args(
            &args(&[
                "--data",
                "new",
                "--title",
                "New Dictionary",
                "--log-file",
                "x.log",
                "--cache-dir",
                "cache",
                "--pages-dir",
                "pages",
                "--verbose",
            ]),
            &env,
        );
        assert_eq!(options.data, "new");
        assert_eq!(options.title, "New Dictionary");
        assert_eq!(options.log_file, "x.log");
        assert_eq!(options.cache_dir, "cache");
        assert_eq!(options.pages_dir, "pages");
        assert!(options.verbose);
        assert!(options.errors.is_empty());
    }

    #[test]
    fn invalid_values_are_collected_not_panicked_on() {
        let options = parse_args(&args(&["--data"]), &HashMap::new());
        assert_eq!(options.errors.len(), 1);
        assert!(options.errors[0].contains("--data requires a value"));

        let options = parse_args(&args(&["--wat"]), &HashMap::new());
        assert_eq!(options.errors.len(), 1);
        assert!(options.errors[0].contains("unknown argument"));

        let options = parse_args(&args(&["--cache-dir"]), &HashMap::new());
        assert_eq!(options.errors.len(), 1);
        assert!(options.errors[0].contains("--cache-dir requires a value"));

        let options = parse_args(&args(&["--pages-dir"]), &HashMap::new());
        assert_eq!(options.errors.len(), 1);
        assert!(options.errors[0].contains("--pages-dir requires a value"));

        let options = parse_args(&args(&["--title"]), &HashMap::new());
        assert_eq!(options.errors.len(), 1);
        assert!(options.errors[0].contains("--title requires a value"));
    }

    #[test]
    fn a_missing_data_path_is_a_configuration_error() {
        let error = resolve_config(&parse_args(&[], &HashMap::new())).unwrap_err();
        assert!(error.to_string().contains("No dictionary"));
    }

    #[test]
    fn an_explicit_missing_data_path_fails_startup() {
        let options = parse_args(
            &args(&["--data", "C:/definitely/not/here/dict.zip"]),
            &HashMap::new(),
        );
        let error = resolve_config(&options).unwrap_err();
        assert!(error.to_string().contains("does not exist"));
    }

    #[test]
    fn build_indices_is_a_mode_flag() {
        let options = parse_args(&args(&["--build-indices"]), &HashMap::new());
        assert!(options.build_indices);
        assert!(!options.help);
        assert!(!options.version);
        assert!(options.errors.is_empty());
    }

    #[test]
    fn build_linked_data_takes_a_destination_and_excludes_build_indices() {
        let options = parse_args(
            &args(&["--build-linked-data", "C:/mcp-dict-data"]),
            &HashMap::new(),
        );
        assert_eq!(options.build_linked_data, "C:/mcp-dict-data");
        assert!(!options.build_indices);
        assert!(options.errors.is_empty());

        let missing = parse_args(&args(&["--build-linked-data"]), &HashMap::new());
        assert!(
            missing
                .errors
                .iter()
                .any(|error| error.contains("requires a value"))
        );

        let both = parse_args(
            &args(&["--build-indices", "--build-linked-data", "C:/copy"]),
            &HashMap::new(),
        );
        let error = resolve_config(&both).expect_err("both build modes are rejected");
        assert!(error.to_string().contains("Choose one build mode"));
    }

    #[test]
    fn usage_names_both_tools_and_the_data_flag() {
        let text = usage();
        assert!(text.contains("search_dictionary"));
        assert!(text.contains("get_entry"));
        assert!(text.contains("related_entries"));
        assert!(text.contains("render_graph"));
        assert!(text.contains("related_network"));
        assert!(text.contains("render_network"));
        assert!(text.contains("--data"));
        assert!(text.contains("--title"));
        assert!(text.contains("--cache-dir"));
        assert!(text.contains("--pages-dir"));
        assert!(text.contains("--build-indices"));
        assert!(text.contains("--build-linked-data"));
        assert!(text.contains("MCP_CHINESE_DICT_DATA"));
        assert!(text.contains("MCP_CHINESE_DICT_CACHE_DIR"));
        assert!(text.contains("MCP_CHINESE_DICT_PAGES_DIR"));
        assert!(text.contains("--embedding-model"));
        assert!(text.contains("--build-semantic-index"));
        assert!(text.contains("--check-semantic"));
        assert!(text.contains("--prune-semantic-index"));
        assert!(text.contains("MCP_CHINESE_DICT_OLLAMA_URL"));
    }

    #[test]
    fn semantic_flags_and_environment_parse() {
        let mut env = HashMap::new();
        env.insert(
            "MCP_CHINESE_DICT_OLLAMA_URL".to_owned(),
            "http://localhost:11434".to_owned(),
        );
        env.insert(
            "MCP_CHINESE_DICT_EMBEDDING_MODEL".to_owned(),
            "qwen3-embedding:0.6b".to_owned(),
        );
        env.insert(
            "MCP_CHINESE_DICT_REQUIRE_SEMANTIC".to_owned(),
            "1".to_owned(),
        );
        let options = parse_args(
            &args(&[
                "--semantic-index",
                "cache/index.bin",
                "--embedding-profile",
                "profile.json",
            ]),
            &env,
        );
        assert_eq!(options.ollama_url, "http://localhost:11434");
        assert_eq!(options.embedding_model, "qwen3-embedding:0.6b");
        assert_eq!(options.embedding_profile, "profile.json");
        assert_eq!(options.semantic_index, "cache/index.bin");
        assert!(options.require_semantic);
        assert!(options.errors.is_empty());

        let flags = parse_args(
            &args(&[
                "--build-semantic-index",
                "--check-semantic",
                "--prune-semantic-index",
                "--yes",
            ]),
            &HashMap::new(),
        );
        assert!(flags.build_semantic_index);
        assert!(flags.check_semantic);
        assert!(flags.prune_semantic_index);
        assert!(flags.yes);
    }

    #[test]
    fn semantic_build_modes_and_pruning_are_validated() {
        let both = parse_args(
            &args(&[
                "--build-indices",
                "--build-semantic-index",
                "--embedding-model",
                "model",
            ]),
            &HashMap::new(),
        );
        let error = resolve_config(&both).unwrap_err();
        assert!(error.to_string().contains("Choose one build mode"));

        let no_model = parse_args(&args(&["--build-semantic-index"]), &HashMap::new());
        let error = resolve_config(&no_model).unwrap_err();
        assert!(error.to_string().contains("requires --embedding-model"));
    }

    #[test]
    fn loopback_urls_are_recognized() {
        assert!(is_loopback_url("http://127.0.0.1:11434"));
        assert!(is_loopback_url("http://localhost:11434/"));
        assert!(is_loopback_url("http://[::1]:11434"));
        assert!(!is_loopback_url("https://ollama.com"));
        assert!(!is_loopback_url("http://192.168.1.10:11434"));
    }

    #[cfg(not(feature = "semantic"))]
    #[test]
    fn semantic_flags_require_the_feature() {
        let options = parse_args(
            &args(&["--data", ".", "--embedding-model", "model"]),
            &HashMap::new(),
        );
        let error = resolve_config(&options).unwrap_err();
        assert!(error.to_string().contains("semantic feature"));
    }

    #[cfg(feature = "semantic")]
    #[test]
    fn a_remote_ollama_url_is_rejected() {
        let options = parse_args(
            &args(&[
                "--data",
                ".",
                "--embedding-model",
                "model",
                "--ollama-url",
                "https://ollama.com",
            ]),
            &HashMap::new(),
        );
        let error = resolve_config(&options).unwrap_err();
        assert!(error.to_string().contains("not loopback"));
    }
}
