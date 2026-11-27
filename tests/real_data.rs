//! Optional structural checks against a real dictionary.
//!
//! Nothing is discovered by filename: set `MCP_CHINESE_DICT_DATA` to a `.zip`
//! or extracted directory to run the test; it prints a skip reason otherwise.

mod support;

use mcp_chinese_dict_search::dictionary::{
    Dictionary, EntryRequest, GraphDirection, GraphRequest, Scope, SearchRequest,
};

#[test]
fn a_configured_dictionary_loads_and_cites_its_own_title() {
    let Some(path) = support::real_data_path() else {
        eprintln!(
            "skipping real-data test: set MCP_CHINESE_DICT_DATA to a dictionary .zip or directory"
        );
        return;
    };

    let started = std::time::Instant::now();
    let dictionary = Dictionary::load(&path).expect("the configured dictionary loads");
    let report = dictionary.report();
    eprintln!(
        "loaded {} entries from {} in {:?}",
        report.main_entries,
        dictionary.title(),
        started.elapsed()
    );

    assert!(report.main_entries > 0, "the dictionary has no entries");
    assert!(
        !dictionary.title().is_empty(),
        "the dictionary title is empty"
    );
    assert_eq!(
        report.graph_nodes, report.main_entries,
        "every main entry must have a graph node"
    );
    assert_eq!(
        report.graph_edges + report.graph_unmapped_links,
        report.resolved_links,
        "every resolved link is either a graph edge or explicitly unmapped"
    );

    let search = dictionary
        .search(&SearchRequest {
            query: "a".to_owned(),
            scope: Scope::Dictionary,
            limit: 1,
        })
        .expect("search succeeds");
    assert_eq!(search.query, "a");

    if let Some(term) = search.hits.first().map(|hit| hit.term.clone()) {
        let related = dictionary
            .related(&GraphRequest {
                term: term.clone(),
                direction: GraphDirection::Both,
                depth: 1,
                limit: 5,
            })
            .expect("graph traversal succeeds for the first dictionary hit");
        assert!(!related.term.is_empty());
        assert!(related.total <= report.main_entries);
        let entry = dictionary
            .entry(&EntryRequest {
                term,
                source: Scope::All,
                max_chars: 20_000,
            })
            .expect("the first hit resolves");
        assert!(
            entry.citation.starts_with(dictionary.title()),
            "citation {:?} does not start with the title {:?}",
            entry.citation,
            dictionary.title()
        );
    }
}
