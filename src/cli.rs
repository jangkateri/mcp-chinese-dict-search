//! Command-line entry point for the Chinese medicine dictionary MCP server.
//!
//! Runs nothing but the stdio transport: stdout carries JSON-RPC messages, and
//! every diagnostic goes to stderr.

use std::collections::HashMap;
use std::sync::Arc;

use crate::config::{parse_args, resolve_config, usage};
use crate::dictionary::{Dictionary, TITLE_FILE, build_linked_data};
use crate::logging::Logger;
use crate::pages::Pages;
use crate::server::{McpServer, SERVER_NAME, SERVER_VERSION};
use crate::transport;

const EXIT_OK: i32 = 0;
const EXIT_FAILURE: i32 = 1;
const EXIT_CONFIG: i32 = 2;

/// Install the signal handler that aborts in-flight calls when the client or a
/// terminal interrupts us.
fn spawn_signal_handler(server: Arc<McpServer>, logger: Logger) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            let mut terminate =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("cannot install a SIGTERM handler");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        logger.debug("signal received; aborting in-flight calls");
        server.abort_all();
    });
}

/// Run the server until stdin closes.
pub async fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let env: HashMap<String, String> = std::env::vars().collect();
    run_with(args, env).await
}

/// Testable form of [`run`]: arguments and environment are injected.
pub async fn run_with(args: Vec<String>, env: HashMap<String, String>) -> i32 {
    let options = parse_args(&args, &env);

    if options.help {
        println!("{}", usage());
        return EXIT_OK;
    }
    if options.version {
        println!("{SERVER_VERSION}");
        return EXIT_OK;
    }
    if !options.errors.is_empty() {
        for message in &options.errors {
            eprintln!("{SERVER_NAME}: {message}");
        }
        eprintln!("{SERVER_NAME}: run with --help for usage");
        return EXIT_CONFIG;
    }

    let config = match resolve_config(&options) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{SERVER_NAME}: {error}");
            return EXIT_CONFIG;
        }
    };

    // An MCP client owns this process's stderr and usually discards it, so an
    // optional log file is often the only place to see what happened.
    let logger = match config.log_file_path() {
        Some(path) => match Logger::with_log_file(config.verbose, path) {
            Ok(logger) => logger,
            Err(error) => {
                eprintln!("{SERVER_NAME}: cannot open the log file {path}: {error}");
                Logger::new(config.verbose)
            }
        },
        None => Logger::new(config.verbose),
    };

    if let Some(destination) = &config.build_linked_data {
        return match build_linked_data(&config.data, destination) {
            Ok(report) => {
                if let Some(title) = &config.title
                    && let Err(error) =
                        std::fs::write(destination.join(TITLE_FILE), format!("{title}\n"))
                {
                    logger.warn(format!(
                        "cannot write the title sidecar in {}: {error}",
                        destination.display()
                    ));
                }
                logger.info(format!(
                    "built linked copy: {} files, {} entries ({} anchors inserted, {} entries \
                     without an anchor); fragment links: {} resolved, {} unresolved; copy: {}",
                    report.files,
                    report.entries,
                    report.anchors_inserted,
                    report.anchorless_entries,
                    report.resolved_fragments,
                    report.unresolved_fragments,
                    destination.display()
                ));
                if report.unresolved_fragments > 0 {
                    logger.warn(format!(
                        "{} fragment links remain unresolved, {} of them into letter files; these \
                         target conversion-dropped ids that belong to the index, the table of \
                         contents, the introduction, or a paragraph rather than an entry. \
                         Examples: {}",
                        report.unresolved_fragments,
                        report.unresolved_in_letters,
                        report.unresolved_examples.join("; ")
                    ));
                }
                EXIT_OK
            }
            Err(error) => {
                eprintln!("{SERVER_NAME}: {error}");
                EXIT_FAILURE
            }
        };
    }

    let started = std::time::Instant::now();
    let loaded = Dictionary::load_with_options(
        &config.data,
        config.cache_dir.as_deref(),
        config.title.as_deref(),
        options.build_indices,
    );
    let dictionary = match loaded {
        Ok(dictionary) => dictionary,
        Err(error) => {
            eprintln!("{SERVER_NAME}: {error}");
            return EXIT_CONFIG;
        }
    };
    let report = dictionary.report();
    logger.info(format!(
        "loaded {} entries, {} points, {} medicinals, {} formulas, {} weight sections from {} in {} ms",
        report.main_entries,
        report.meridian_entries,
        report.medicinal_entries,
        report.formula_entries,
        report.weight_sections,
        config.data.display(),
        started.elapsed().as_millis()
    ));
    logger.debug(format!(
        "index: {} anchors, {} recovered entry anchors, {} recovered link targets, {} pinyin tone \
         keys, {} pinyin toneless keys, {} Han keys, {} resolved links, {} unresolved, {} graph \
         nodes, {} graph edges, {} unmapped graph links, {} formula edges, {} formula-medicinal \
         edges, {} point combinations, {} anchorless entries, {} duplicate headwords, {} alias \
         failures, {} appendix header failures",
        report.index_anchors,
        report.recovered_entry_anchors,
        report.recovered_link_targets,
        report.pinyin_toned_keys,
        report.pinyin_toneless_keys,
        report.han_keys,
        report.resolved_links,
        report.unresolved_links,
        report.graph_nodes,
        report.graph_edges,
        report.graph_unmapped_links,
        report.network_formula_edges,
        report.network_formula_medicinal_edges,
        report.network_point_combinations,
        report.anchorless_entries,
        report.duplicate_headwords,
        report.alias_parse_failures,
        report.appendix_header_failures
    ));
    logger.debug(format!(
        "lookup cache: {} at {}",
        if report.lookup_cache_hit {
            "hit"
        } else if report.lookup_cache_ready {
            "miss (built and saved)"
        } else {
            "not written"
        },
        report.lookup_cache_path.as_deref().unwrap_or("none")
    ));
    for warning in &report.warnings {
        logger.warn(warning.clone());
    }

    if options.build_indices {
        let path = report.lookup_cache_path.as_deref().unwrap_or("none");
        if !report.lookup_cache_ready {
            logger.error(format!("could not write the lookup indices to {path}"));
            return EXIT_FAILURE;
        }
        logger.info(format!(
            "built {} pinyin tone keys, {} pinyin toneless keys, {} Han keys from {} in {} ms; \
             cache: {path}",
            report.pinyin_toned_keys,
            report.pinyin_toneless_keys,
            report.han_keys,
            config.data.display(),
            started.elapsed().as_millis()
        ));
        return EXIT_OK;
    }

    #[cfg(feature = "semantic")]
    {
        let model = config.embedding_model.as_deref().unwrap_or_default();
        if config.build_semantic_index {
            return match crate::semantic::startup::build(
                &dictionary,
                &config.data,
                config.cache_dir.as_deref(),
                model,
                &config.ollama_url,
                config.embedding_profile.as_deref(),
                config.semantic_index.as_deref(),
                &logger,
            ) {
                Ok(report) => {
                    logger.info(format!(
                        "semantic index ready: {} records, {} dimensions, {} bytes at {}",
                        report.documents,
                        report.dimensions,
                        report.bytes,
                        report.index_path.display()
                    ));
                    EXIT_OK
                }
                Err(error) => {
                    eprintln!("{SERVER_NAME}: {error}");
                    EXIT_FAILURE
                }
            };
        }
        if config.check_semantic {
            return match crate::semantic::startup::check(
                &dictionary,
                &config.data,
                config.cache_dir.as_deref(),
                model,
                &config.ollama_url,
                config.embedding_profile.as_deref(),
                config.semantic_index.as_deref(),
            ) {
                Ok(report) => {
                    println!("{report}");
                    EXIT_OK
                }
                Err(error) => {
                    eprintln!("{SERVER_NAME}: {error}");
                    EXIT_FAILURE
                }
            };
        }
        if config.prune_semantic_index {
            return match crate::semantic::startup::prune(
                &dictionary,
                &config.data,
                config.cache_dir.as_deref(),
                model,
                &config.ollama_url,
                config.embedding_profile.as_deref(),
                config.semantic_index.as_deref(),
                config.yes,
            ) {
                Ok(report) => {
                    for path in &report.kept {
                        println!("kept {}", path.display());
                    }
                    for path in &report.removed {
                        println!("removed {}", path.display());
                    }
                    for path in &report.planned {
                        println!("would remove {} (pass --yes to delete)", path.display());
                    }
                    if report.removed.is_empty() && report.planned.is_empty() {
                        println!("no stale semantic indexes to remove");
                    }
                    EXIT_OK
                }
                Err(error) => {
                    eprintln!("{SERVER_NAME}: {error}");
                    EXIT_FAILURE
                }
            };
        }
    }

    #[cfg(feature = "semantic")]
    let semantic = if let Some(model) = config.embedding_model.as_deref() {
        let options = crate::semantic::ServeOptions {
            data: &config.data,
            cache_dir: config.cache_dir.as_deref(),
            model,
            ollama_url: &config.ollama_url,
            index_override: config.semantic_index.as_deref(),
            profile_path: config.embedding_profile.as_deref(),
            require: config.require_semantic,
        };
        match crate::semantic::startup::load_engine(&dictionary, &options, &logger) {
            Ok(engine) => engine.map(Arc::new),
            Err(error) => {
                eprintln!("{SERVER_NAME}: {error}");
                return EXIT_CONFIG;
            }
        }
    } else {
        None
    };

    let pages = match config.pages_dir.as_deref() {
        Some(directory) => match Pages::new(directory) {
            Ok(pages) => {
                if pages.directory().to_string_lossy().contains(' ') {
                    logger.warn(format!(
                        "citation page directory {} contains a space; VS Code's terminal may stop the link at \
                         the space. Choose a path without spaces.",
                        pages.directory().display()
                    ));
                }
                logger.debug(format!("citation pages: {}", pages.directory().display()));
                Some(pages)
            }
            Err(error) => {
                logger.warn(format!(
                    "Cannot create the citation page directory {}: {error}",
                    directory.display()
                ));
                None
            }
        },
        None => None,
    };

    let dictionary = Arc::new(dictionary);
    let mut server = McpServer::new(Arc::clone(&dictionary), logger.clone());
    #[cfg(feature = "semantic")]
    {
        server = server.with_semantic(semantic);
    }
    if let Some(pages) = pages {
        server = server.with_pages(pages);
    }
    let server = Arc::new(server);
    spawn_signal_handler(Arc::clone(&server), logger.clone());

    let result = transport::serve(
        server,
        tokio::io::stdin(),
        tokio::io::stdout(),
        logger.clone(),
    )
    .await;

    match result {
        Ok(()) => {
            logger.debug("stdio closed; exiting");
            EXIT_OK
        }
        Err(error) => {
            logger.error(format!("stdio transport failed: {error}"));
            EXIT_FAILURE
        }
    }
}
