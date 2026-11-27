//! Guards for the deployable Windows bundle in `dist/`.
//!
//! These are text/artifact checks, not runtime tests: they keep the deployment
//! guide's version in step with the crate and make sure dictionary data and
//! credentials are never shipped.

use std::path::Path;

#[test]
fn install_guide_matches_the_package_version() {
    let guide = include_str!("../dist/INSTALL.md");
    let expected = format!("**Version:** {}", env!("CARGO_PKG_VERSION"));
    assert!(
        guide.contains(&expected),
        "dist/INSTALL.md must state {expected}"
    );
}

#[test]
fn install_guide_covers_every_supported_client() {
    let guide = include_str!("../dist/INSTALL.md");
    for client in ["Claude Code", "OpenCode", "DeepSeek Harness"] {
        assert!(
            guide.contains(client),
            "dist/INSTALL.md must document {client}"
        );
    }
}

#[test]
fn bundle_ships_no_dictionary_data_or_credentials() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("dist");
    assert!(
        !dist.join(".credentials").exists(),
        "dist/ must never contain a live credential file"
    );

    for entry in std::fs::read_dir(&dist).expect("dist exists") {
        let entry = entry.expect("dist entry reads");
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let release_archive =
            name.starts_with("mcp-chinese-dict-search-") && name.ends_with(".zip");
        assert!(
            !name.ends_with(".epub") && (release_archive || !name.ends_with(".zip")),
            "dist/ must not ship dictionary archives: {name}"
        );
        assert!(
            !name.starts_with("images"),
            "dist/ must not ship dictionary images: {name}"
        );
        assert!(
            !name.contains(".lookup-cache-"),
            "dist/ must not ship a derived lookup cache: {name}"
        );
    }
}

#[test]
fn bundle_contains_the_binary_and_checksums() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("dist");
    assert!(
        dist.join("mcp-chinese-dict-search.exe").is_file(),
        "dist/ must contain the Windows x64 executable"
    );
    assert!(
        dist.join("evaluate-semantic.exe").is_file(),
        "dist/ must contain the evaluation executable"
    );
    let sums = include_str!("../dist/SHA256SUMS.txt");
    for file in [
        "mcp-chinese-dict-search.exe",
        "evaluate-semantic.exe",
        "install.ps1",
        "INSTALL.md",
        "mcp-config.example.json",
        "LICENSE",
    ] {
        assert!(sums.contains(file), "SHA256SUMS.txt must list {file}");
    }
}

#[test]
fn config_example_uses_placeholders_and_the_data_flag() {
    let config = include_str!("../dist/mcp-config.example.json");
    assert!(config.contains("C:/path/to/mcp-chinese-dict-search.exe"));
    assert!(config.contains("C:/path/to/dictionary.zip"));
    assert!(config.contains("--data"));
}
