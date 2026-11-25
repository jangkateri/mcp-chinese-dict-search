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
  --verbose            Log request handling, timing, and the load report
  -h, --help           Print this help and exit
  --version            Print the version and exit

Tools published over stdio:
  search_dictionary(query, scope?, limit?)   Ranked dictionary and appendix hits
  get_entry(term, source?, max_chars?)       One exact entry with cross-references"
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
            other => options.errors.push(format!("unknown argument {other:?}")),
        }
        index += 1;
    }

    options
}

/// Normalize and validate the parsed options, resolving the data path.
pub fn resolve_config(options: &Options) -> Result<Config, ConfigError> {
    if options.build_indices && !options.build_linked_data.trim().is_empty() {
        return Err(ConfigError(
            "Choose one build mode: --build-indices or --build-linked-data.".to_owned(),
        ));
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

    Ok(Config {
        data,
        title,
        log_file: options.log_file.clone(),
        cache_dir,
        pages_dir,
        build_linked_data,
        verbose: options.verbose,
    })
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
        assert!(text.contains("--data"));
        assert!(text.contains("--title"));
        assert!(text.contains("--cache-dir"));
        assert!(text.contains("--pages-dir"));
        assert!(text.contains("--build-indices"));
        assert!(text.contains("--build-linked-data"));
        assert!(text.contains("MCP_CHINESE_DICT_DATA"));
        assert!(text.contains("MCP_CHINESE_DICT_CACHE_DIR"));
        assert!(text.contains("MCP_CHINESE_DICT_PAGES_DIR"));
    }
}
