//! Relationship traversal over the cross-references the parser already resolved.
//!
//! The graph is built once from `Entry.links`. A link is an edge only when it is
//! resolved and its target anchor maps to a main entry. No relation is inferred
//! from prose.

use std::collections::{HashMap, VecDeque};

use crate::dictionary::Dictionary;
use crate::dictionary::model::{
    EdgeDirection, Entry, GraphDirection, GraphNode, GraphRequest, GraphResponse, Link, Source,
};
use crate::dictionary::normalize::normalize;
use crate::dictionary::search;
use crate::error::{DictionaryError, DictionaryErrorCode};

/// Maximum traversal depth accepted by the tool and the core.
pub(crate) const MAX_DEPTH: usize = 3;
/// Maximum number of returned nodes.
pub(crate) const MAX_LIMIT: usize = 50;
/// Safety cap on discovered nodes, independent of output truncation.
pub(crate) const MAX_VISITED: usize = 10_000;

/// One directed adjacency entry.
///
/// `owner` is the entry whose `Link` produced the edge; `link` indexes that
/// entry's `links`. Keeping indices instead of cloning text makes the graph
/// small and guarantees the displayed link text comes from the parsed source.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GraphEdge {
    pub(crate) neighbor: usize,
    pub(crate) owner: usize,
    pub(crate) link: usize,
}

/// Immutable outgoing and incoming adjacency lists.
#[derive(Debug, Default)]
pub(crate) struct Graph {
    pub(crate) outgoing: Vec<Vec<GraphEdge>>,
    pub(crate) incoming: Vec<Vec<GraphEdge>>,
}

/// Counts gathered while building the graph.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct GraphStats {
    pub(crate) nodes: usize,
    pub(crate) edges: usize,
    pub(crate) unmapped_links: usize,
}

impl Graph {
    /// Build adjacency from the final entry anchors and resolved links.
    pub(crate) fn build(entries: &[Entry]) -> (Self, GraphStats) {
        let mut graph = Self {
            outgoing: vec![Vec::new(); entries.len()],
            incoming: vec![Vec::new(); entries.len()],
        };
        let mut stats = GraphStats {
            nodes: entries.len(),
            ..GraphStats::default()
        };

        let mut entry_by_anchor: HashMap<&str, usize> = HashMap::new();
        let mut entry_by_term: HashMap<(String, String), usize> = HashMap::new();
        for (index, entry) in entries.iter().enumerate() {
            if let Some(anchor) = entry.anchor.as_deref()
                && !anchor.is_empty()
            {
                entry_by_anchor.entry(anchor).or_insert(index);
            }
            entry_by_term
                .entry((entry.file.clone(), entry.norm_headword.clone()))
                .or_insert(index);
        }

        for (owner, entry) in entries.iter().enumerate() {
            for (link_index, link) in entry.links.iter().enumerate() {
                if !link.resolved {
                    continue;
                }
                let anchor = link
                    .target_anchor
                    .as_deref()
                    .filter(|anchor| !anchor.is_empty());
                // Most links name an entry anchor. A subentry anchor such as
                // `a1SSD` names a paragraph inside an entry, so fall back to the
                // link's resolved term and file to find the containing entry.
                let neighbor = anchor
                    .and_then(|anchor| entry_by_anchor.get(anchor).copied())
                    .or_else(|| {
                        let term = link.target_term.as_deref()?;
                        let file = link.target_file.as_deref()?;
                        entry_by_term
                            .get(&(file.to_owned(), normalize(term)))
                            .copied()
                    });
                let Some(neighbor) = neighbor else {
                    stats.unmapped_links += 1;
                    continue;
                };
                graph.outgoing[owner].push(GraphEdge {
                    neighbor,
                    owner,
                    link: link_index,
                });
                graph.incoming[neighbor].push(GraphEdge {
                    neighbor: owner,
                    owner,
                    link: link_index,
                });
                stats.edges += 1;
            }
        }

        (graph, stats)
    }
}

#[derive(Debug)]
struct Discovered {
    index: usize,
    distance: usize,
    direction: EdgeDirection,
    via: String,
    link_text: String,
}

/// Traverse from one main dictionary entry.
pub(crate) fn related(
    dictionary: &Dictionary,
    request: &GraphRequest,
) -> Result<GraphResponse, DictionaryError> {
    let term = request.term.trim();
    if term.is_empty() {
        return Err(DictionaryError::new(
            "The \"term\" argument is empty after normalization.",
            DictionaryErrorCode::InvalidRequest,
        ));
    }
    if request.depth == 0 || request.depth > MAX_DEPTH {
        return Err(DictionaryError::new(
            format!(
                "\"depth\" must be between 1 and {MAX_DEPTH}, got {}.",
                request.depth
            ),
            DictionaryErrorCode::InvalidRequest,
        ));
    }
    if request.limit == 0 || request.limit > MAX_LIMIT {
        return Err(DictionaryError::new(
            format!(
                "\"limit\" must be between 1 and {MAX_LIMIT}, got {}.",
                request.limit
            ),
            DictionaryErrorCode::InvalidRequest,
        ));
    }

    let Some(start) = search::resolve_dictionary_entry(dictionary, term) else {
        return Err(DictionaryError::not_found(
            format!(
                "No main dictionary entry matches {term:?}. Relationship traversal covers main \
                 dictionary entries; use get_entry for an appendix entry."
            ),
            Vec::new(),
        ));
    };

    let mut visited = vec![false; dictionary.entries.len()];
    visited[start] = true;
    let mut queue = VecDeque::from([(start, 0usize)]);
    let mut discovered = Vec::new();
    let mut cap_hit = false;

    'traversal: while let Some((current, distance)) = queue.pop_front() {
        if distance >= request.depth {
            continue;
        }

        let mut edge_lists: Vec<(EdgeDirection, &Vec<GraphEdge>)> = Vec::with_capacity(2);
        if matches!(
            request.direction,
            GraphDirection::Outgoing | GraphDirection::Both
        ) {
            edge_lists.push((EdgeDirection::Outgoing, &dictionary.graph.outgoing[current]));
        }
        if matches!(
            request.direction,
            GraphDirection::Incoming | GraphDirection::Both
        ) {
            edge_lists.push((EdgeDirection::Incoming, &dictionary.graph.incoming[current]));
        }

        for (direction, edges) in edge_lists {
            for edge in edges {
                if visited[edge.neighbor] {
                    continue;
                }
                if discovered.len() >= MAX_VISITED {
                    cap_hit = true;
                    break 'traversal;
                }
                visited[edge.neighbor] = true;
                let owner = &dictionary.entries[edge.owner];
                let link = owner.links.get(edge.link);
                let link_text = link
                    .map(|link: &Link| link.text.clone())
                    .filter(|text| !text.is_empty())
                    .unwrap_or_else(|| dictionary.entries[edge.neighbor].headword.clone());
                discovered.push(Discovered {
                    index: edge.neighbor,
                    distance: distance + 1,
                    direction,
                    via: owner.headword.clone(),
                    link_text,
                });
                queue.push_back((edge.neighbor, distance + 1));
            }
        }
    }

    discovered.sort_by(|left, right| {
        left.distance
            .cmp(&right.distance)
            .then_with(|| left.index.cmp(&right.index))
    });
    let total = discovered.len();
    let truncated = cap_hit || total > request.limit;
    let nodes = discovered
        .into_iter()
        .take(request.limit)
        .map(|node| graph_node(dictionary, node))
        .collect();

    let start_entry = &dictionary.entries[start];
    Ok(GraphResponse {
        query: term.to_owned(),
        term: start_entry.headword.clone(),
        anchor: start_entry.anchor.clone(),
        direction: request.direction,
        depth: request.depth,
        nodes,
        total,
        truncated,
    })
}

fn graph_node(dictionary: &Dictionary, discovered: Discovered) -> GraphNode {
    let entry = &dictionary.entries[discovered.index];
    GraphNode {
        source: Source::Dictionary,
        term: entry.headword.clone(),
        id: None,
        anchor: entry.anchor.clone(),
        file: entry.file.clone(),
        citation: search::entry_citation(dictionary, entry),
        source_file: dictionary.source_file(&entry.file),
        line: Some(entry.line),
        distance: discovered.distance,
        edge_direction: discovered.direction,
        via: discovered.via,
        link_text: discovered.link_text,
        redirect: search::is_redirect(&entry.body),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn link_to(text: &str, anchor: &str, target_term: &str) -> Link {
        Link {
            text: text.to_owned(),
            target_file: Some("08-a.md".to_owned()),
            target_anchor: Some(anchor.to_owned()),
            target_term: Some(target_term.to_owned()),
            resolved: true,
        }
    }

    fn link(text: &str, anchor: &str) -> Link {
        link_to(text, anchor, text)
    }

    fn entry(headword: &str, anchor: Option<&str>, links: Vec<Link>) -> Entry {
        Entry {
            headword: headword.to_owned(),
            norm_headword: normalize(headword),
            file: "08-a.md".to_owned(),
            line: 1,
            anchor: anchor.map(str::to_owned),
            aliases: Vec::new(),
            body: String::new(),
            norm_body: String::new(),
            links,
        }
    }

    fn dictionary_from(entries: Vec<Entry>) -> Dictionary {
        let (graph, _) = Graph::build(&entries);
        let mut by_headword = HashMap::new();
        for (index, entry) in entries.iter().enumerate() {
            by_headword
                .entry(entry.norm_headword.clone())
                .or_insert_with(Vec::new)
                .push(index);
        }
        let mut dictionary = Dictionary::empty();
        dictionary.entries = entries;
        dictionary.by_headword = by_headword;
        dictionary.graph = graph;
        dictionary
    }

    fn dictionary() -> Dictionary {
        dictionary_from(vec![
            entry("alpha", Some("a1ALP"), vec![link("greater alpha", "a1BET")]),
            entry("greater alpha", Some("a1BET"), vec![link("alpha", "a1ALP")]),
            entry("alpha alias", Some("a1AAL"), vec![link("alpha", "a1ALP")]),
        ])
    }

    fn request(term: &str, direction: GraphDirection, depth: usize, limit: usize) -> GraphRequest {
        GraphRequest {
            term: term.to_owned(),
            direction,
            depth,
            limit,
        }
    }

    #[test]
    fn build_maps_resolved_links_and_counts_unmapped_ones() {
        let entries = vec![
            entry(
                "alpha",
                Some("a1ALP"),
                vec![link("greater alpha", "a1BET"), link("missing", "a1ZZZ")],
            ),
            entry("greater alpha", Some("a1BET"), Vec::new()),
        ];
        let (graph, stats) = Graph::build(&entries);
        assert_eq!(stats.nodes, 2);
        assert_eq!(stats.edges, 1);
        assert_eq!(stats.unmapped_links, 1);
        assert_eq!(graph.outgoing[0].len(), 1);
        assert_eq!(graph.incoming[1].len(), 1);
    }

    #[test]
    fn outgoing_and_incoming_are_directional() {
        let dictionary = dictionary();
        let outgoing = related(
            &dictionary,
            &request("alpha", GraphDirection::Outgoing, 1, 20),
        )
        .unwrap();
        assert_eq!(outgoing.nodes.len(), 1);
        assert_eq!(outgoing.nodes[0].term, "greater alpha");
        assert_eq!(outgoing.nodes[0].edge_direction, EdgeDirection::Outgoing);

        let incoming = related(
            &dictionary,
            &request("alpha", GraphDirection::Incoming, 1, 20),
        )
        .unwrap();
        assert_eq!(incoming.nodes.len(), 2);
        assert_eq!(incoming.nodes[0].term, "greater alpha");
        assert_eq!(incoming.nodes[1].term, "alpha alias");
        assert_eq!(incoming.nodes[0].edge_direction, EdgeDirection::Incoming);

        let both = related(&dictionary, &request("alpha", GraphDirection::Both, 1, 20)).unwrap();
        assert_eq!(both.nodes.len(), 2);
        assert_eq!(both.nodes[0].term, "greater alpha");
        assert_eq!(both.nodes[1].term, "alpha alias");
    }

    #[test]
    fn a_second_hop_is_reached_with_its_via_entry() {
        let dictionary = dictionary_from(vec![
            entry("alpha", Some("a1ALP"), vec![link("greater alpha", "a1BET")]),
            entry(
                "greater alpha",
                Some("a1BET"),
                vec![link("clear heat", "a1CHT")],
            ),
            entry("clear heat", Some("a1CHT"), Vec::new()),
        ]);
        let response = related(
            &dictionary,
            &request("alpha", GraphDirection::Outgoing, 2, 20),
        )
        .unwrap();
        assert_eq!(response.total, 2);
        assert_eq!(response.nodes[0].term, "greater alpha");
        assert_eq!(response.nodes[0].distance, 1);
        assert_eq!(response.nodes[0].via, "alpha");
        assert_eq!(response.nodes[1].term, "clear heat");
        assert_eq!(response.nodes[1].distance, 2);
        assert_eq!(response.nodes[1].via, "greater alpha");
    }

    #[test]
    fn subentry_anchor_maps_to_its_entry_and_redirect_is_marked() {
        let mut dictionary = dictionary_from(vec![
            entry(
                "alpha",
                Some("a1ALP"),
                vec![link_to("wind-like stroke", "a1SSD", "wind stroke")],
            ),
            entry("wind stroke", Some("a1R4B"), Vec::new()),
        ]);
        dictionary.entries[1].body = "See alpha.".to_owned();
        let response = related(
            &dictionary,
            &request("alpha", GraphDirection::Outgoing, 1, 20),
        )
        .unwrap();
        assert_eq!(response.nodes[0].term, "wind stroke");
        assert_eq!(response.nodes[0].anchor.as_deref(), Some("a1R4B"));
        assert_eq!(response.nodes[0].link_text, "wind-like stroke");
        assert!(response.nodes[0].redirect);
    }

    #[test]
    fn cycles_terminate_and_distance_is_shortest() {
        let dictionary = dictionary();
        let response =
            related(&dictionary, &request("alpha", GraphDirection::Both, 3, 20)).unwrap();
        assert_eq!(response.total, 2);
        assert_eq!(
            response
                .nodes
                .iter()
                .filter(|node| node.term == "alpha")
                .count(),
            0,
            "the start entry is not returned as a neighbour"
        );
    }

    #[test]
    fn depth_and_limit_are_enforced() {
        let dictionary = dictionary();
        let too_deep =
            related(&dictionary, &request("alpha", GraphDirection::Both, 4, 20)).unwrap_err();
        assert_eq!(too_deep.code, DictionaryErrorCode::InvalidRequest);

        let no_limit =
            related(&dictionary, &request("alpha", GraphDirection::Both, 1, 0)).unwrap_err();
        assert_eq!(no_limit.code, DictionaryErrorCode::InvalidRequest);

        let too_many = related(
            &dictionary,
            &request("alpha", GraphDirection::Both, 1, MAX_LIMIT + 1),
        )
        .unwrap_err();
        assert_eq!(too_many.code, DictionaryErrorCode::InvalidRequest);

        let one = related(&dictionary, &request("alpha", GraphDirection::Both, 1, 1)).unwrap();
        assert_eq!(one.nodes.len(), 1);
        assert_eq!(one.total, 2);
        assert!(one.truncated);
    }

    #[test]
    fn unknown_term_and_empty_term_do_not_panic() {
        let dictionary = dictionary();
        let unknown = related(
            &dictionary,
            &request("not present", GraphDirection::Both, 1, 20),
        )
        .unwrap_err();
        assert_eq!(unknown.code, DictionaryErrorCode::NotFound);
        assert!(unknown.message.contains("main dictionary entry"));

        let empty = related(&dictionary, &request("  ", GraphDirection::Both, 1, 20)).unwrap_err();
        assert_eq!(empty.code, DictionaryErrorCode::InvalidRequest);
    }

    #[test]
    fn anchorless_start_still_resolves_by_headword_and_cites_file_line() {
        let mut dictionary = dictionary();
        dictionary
            .entries
            .push(entry("anchorless", None, Vec::new()));
        let index = dictionary.entries.len() - 1;
        dictionary
            .by_headword
            .insert("anchorless".to_owned(), vec![index]);
        let (graph, _) = Graph::build(&dictionary.entries);
        dictionary.graph = graph;
        let response = related(
            &dictionary,
            &request("anchorless", GraphDirection::Both, 1, 20),
        )
        .unwrap();
        assert_eq!(response.term, "anchorless");
        assert_eq!(response.anchor, None);
    }
}
