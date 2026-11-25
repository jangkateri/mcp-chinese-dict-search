//! Verify that the generated lookup maps are reused across loads and rebuilt
//! when the dictionary content changes.

mod support;

use std::path::{Path, PathBuf};

use mcp_chinese_dict_search::dictionary::Dictionary;

fn copy_fixture(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("fixture destination is created");
    for entry in std::fs::read_dir(source).expect("fixture directory is readable") {
        let entry = entry.expect("fixture entry is readable");
        if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "md")
        {
            std::fs::copy(entry.path(), destination.join(entry.file_name()))
                .expect("fixture file is copied");
        }
    }
}

#[test]
fn lookup_cache_is_reused_until_the_dictionary_changes() {
    let base = std::env::temp_dir().join(format!(
        "mcp-chinese-dict-cache-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&base);
    let data = base.join("dictionary");
    let cache = base.join("lookup-cache");
    copy_fixture(&support::fixture_dir(), &data);

    let first = Dictionary::load_with_cache(&data, Some(&cache)).expect("first load succeeds");
    assert!(!first.report().lookup_cache_hit);
    let first_path: PathBuf = first
        .report()
        .lookup_cache_path
        .as_ref()
        .expect("cache path is recorded")
        .into();
    assert!(first_path.exists(), "cache is written: {first_path:?}");

    let second = Dictionary::load_with_cache(&data, Some(&cache)).expect("second load succeeds");
    assert!(second.report().lookup_cache_hit);
    assert_eq!(
        second.report().lookup_cache_path.as_deref(),
        first.report().lookup_cache_path.as_deref()
    );

    let main_file = data.join("08-a.md");
    let mut content = std::fs::read_to_string(&main_file).expect("fixture is readable");
    content.push_str("\n<!-- changed -->\n");
    std::fs::write(&main_file, content).expect("fixture is modified");

    let third = Dictionary::load_with_cache(&data, Some(&cache)).expect("third load succeeds");
    assert!(!third.report().lookup_cache_hit);
    assert_ne!(
        third.report().lookup_cache_path.as_deref(),
        first.report().lookup_cache_path.as_deref()
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn rebuilding_replaces_a_current_cache_and_stays_reusable() {
    let base = std::env::temp_dir().join(format!(
        "mcp-chinese-dict-rebuild-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&base);
    let data = base.join("dictionary");
    let cache = base.join("lookup-cache");
    copy_fixture(&support::fixture_dir(), &data);

    let first = Dictionary::load_with_cache(&data, Some(&cache)).expect("first load succeeds");
    assert!(!first.report().lookup_cache_hit);
    assert!(first.report().lookup_cache_ready);
    let path = first
        .report()
        .lookup_cache_path
        .clone()
        .expect("cache path is recorded");
    assert!(Path::new(&path).exists(), "cache is written: {path}");

    let rebuilt = Dictionary::load_rebuilding_cache(&data, Some(&cache)).expect("rebuild succeeds");
    assert!(
        !rebuilt.report().lookup_cache_hit,
        "an explicit rebuild never reuses the file it replaces"
    );
    assert!(rebuilt.report().lookup_cache_ready);
    assert_eq!(
        rebuilt.report().pinyin_toned_keys,
        first.report().pinyin_toned_keys
    );
    assert_eq!(
        rebuilt.report().pinyin_toneless_keys,
        first.report().pinyin_toneless_keys
    );
    assert_eq!(rebuilt.report().han_keys, first.report().han_keys);
    assert_eq!(rebuilt.report().lookup_cache_path.as_deref(), Some(&*path));

    let third = Dictionary::load_with_cache(&data, Some(&cache)).expect("third load succeeds");
    assert!(
        third.report().lookup_cache_hit,
        "the rebuilt file is usable by the next startup"
    );

    let _ = std::fs::remove_dir_all(&base);
}
