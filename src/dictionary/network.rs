//! Appendix network: formula families, formula-medicinal composition, and
//! named acupuncture-point combinations.
//!
//! These relationships are not `<a href>` links. Formula families come from
//! explicit prose such as "This is X with the addition of ...", formula
//! ingredients from the `□` ingredient lines, and point combinations from the
//! named `combining ... points` entries. Every edge keeps a relation label and
//! a source line; none is a clinical equivalence.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::dictionary::Dictionary;
use crate::dictionary::model::{
    AppendixEntry, EdgeDirection, GraphDirection, NetworkDomain, NetworkNode, NetworkNodeType,
    NetworkRelation, NetworkRequest, NetworkResponse, Source,
};
use crate::dictionary::normalize::normalize;
use crate::dictionary::search;
use crate::error::{DictionaryError, DictionaryErrorCode};

const MAX_DEPTH: usize = 3;
const MAX_LIMIT: usize = 50;
const MAX_VISITED: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NodeKey {
    Formula(usize),
    Medicinal(usize),
    Point(usize),
    Combination(usize),
    Method(usize),
}

#[derive(Debug, Clone)]
struct Edge {
    from: NodeKey,
    to: NodeKey,
    relation: NetworkRelation,
    label: String,
}

#[derive(Debug, Clone)]
struct Combination {
    context: String,
    indication: String,
    label: String,
    file: String,
    line: usize,
}

#[derive(Debug, Default)]
pub(crate) struct Network {
    outgoing: HashMap<NodeKey, Vec<Edge>>,
    incoming: HashMap<NodeKey, Vec<Edge>>,
    formula_names: HashMap<String, usize>,
    medicinal_names: HashMap<String, usize>,
    point_names: HashMap<String, usize>,
    method_names: HashMap<String, usize>,
    combinations: Vec<Combination>,
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct NetworkStats {
    pub(crate) formula_edges: usize,
    pub(crate) formula_medicinal_edges: usize,
    pub(crate) point_combinations: usize,
}

impl Network {
    pub(crate) fn build(dictionary: &Dictionary) -> (Self, NetworkStats) {
        let mut network = Self::default();
        network.index_formulas(dictionary);
        network.index_medicinals(dictionary);
        network.index_points(dictionary);
        network.index_methods(dictionary);
        let formula_edges = network.build_formula_edges(dictionary);
        let formula_medicinal_edges = network.build_medicinal_edges(dictionary);
        let point_combinations = network.build_point_combinations(dictionary);
        (
            network,
            NetworkStats {
                formula_edges,
                formula_medicinal_edges,
                point_combinations,
            },
        )
    }

    fn index_formulas(&mut self, dictionary: &Dictionary) {
        for (index, formula) in dictionary.formulas.iter().enumerate() {
            add_appendix_names(&mut self.formula_names, formula, index);
        }
    }

    fn index_medicinals(&mut self, dictionary: &Dictionary) {
        for (index, medicinal) in dictionary.medicinals.iter().enumerate() {
            add_appendix_names(&mut self.medicinal_names, medicinal, index);
        }
    }

    fn index_points(&mut self, dictionary: &Dictionary) {
        for (id, index) in &dictionary.point_ids {
            self.point_names.insert(id.clone(), *index);
        }
        for (index, point) in dictionary.meridians.iter().enumerate() {
            add_appendix_names(&mut self.point_names, point, index);
        }
    }

    fn index_methods(&mut self, dictionary: &Dictionary) {
        for (index, entry) in dictionary.entries.iter().enumerate() {
            if entry.norm_headword.starts_with("combining ")
                && entry.norm_headword.contains("point")
            {
                self.method_names
                    .entry(entry.norm_headword.clone())
                    .or_insert(index);
                for alias in &entry.aliases {
                    for field in [
                        &alias.simplified,
                        &alias.traditional,
                        &alias.pinyin_toned,
                        &alias.pinyin_ascii,
                    ] {
                        let key = normalize(field);
                        if !key.is_empty() {
                            self.method_names.entry(key).or_insert(index);
                        }
                    }
                }
            }
        }
    }

    fn build_formula_edges(&mut self, dictionary: &Dictionary) -> usize {
        let mut count = 0;
        let formula_names: Vec<(String, usize)> = self
            .formula_names
            .iter()
            .map(|(name, index)| (name.clone(), *index))
            .collect();
        for (index, formula) in dictionary.formulas.iter().enumerate() {
            let body = normalize(&formula.body);
            let Some(this_is) = body.find("this is ") else {
                continue;
            };
            let sentence_end = body[this_is..]
                .find('.')
                .map(|end| this_is + end)
                .unwrap_or(body.len());
            let combined = body.find("combined with ");
            let variant = body.find("variant of ");
            let mut seen = HashSet::new();
            for (name, target) in &formula_names {
                if *target == index || name.len() < 5 {
                    continue;
                }
                let Some(position) = body.find(name) else {
                    continue;
                };
                if position < this_is || position >= sentence_end {
                    if variant.is_some_and(|variant| position > variant && position < sentence_end)
                        && seen.insert((*target, NetworkRelation::HasVariant))
                    {
                        self.add_edge(
                            NodeKey::Formula(*target),
                            NodeKey::Formula(index),
                            NetworkRelation::HasVariant,
                            name.clone(),
                        );
                        count += 1;
                    }
                    continue;
                }
                let relation = if combined.is_some_and(|start| position > start) {
                    NetworkRelation::CombinedWith
                } else {
                    NetworkRelation::HasDerivative
                };
                if seen.insert((*target, relation)) {
                    self.add_edge(
                        NodeKey::Formula(*target),
                        NodeKey::Formula(index),
                        relation,
                        name.clone(),
                    );
                    count += 1;
                }
            }
        }
        count
    }

    fn build_medicinal_edges(&mut self, dictionary: &Dictionary) -> usize {
        let mut count = 0;
        let medicinal_names: Vec<(String, usize)> = self
            .medicinal_names
            .iter()
            .map(|(name, index)| (name.clone(), *index))
            .collect();
        for (formula_index, formula) in dictionary.formulas.iter().enumerate() {
            let mut seen = HashSet::new();
            for line in formula.body.lines() {
                let trimmed = line.trim_start();
                if trimmed.starts_with('•') {
                    break;
                }
                if !trimmed.starts_with('□') {
                    continue;
                }
                let line_norm = normalize(line);
                let mut matches: Vec<(usize, usize)> = medicinal_names
                    .iter()
                    .filter(|(name, _)| name.len() >= 4 && line_norm.contains(name.as_str()))
                    .map(|(name, index)| (name.len(), *index))
                    .collect();
                matches.sort_by_key(|left| std::cmp::Reverse(left.0));
                for (_, medicinal_index) in matches {
                    if !seen.insert(medicinal_index) {
                        continue;
                    }
                    self.add_edge(
                        NodeKey::Formula(formula_index),
                        NodeKey::Medicinal(medicinal_index),
                        NetworkRelation::Contains,
                        line.trim().to_owned(),
                    );
                    self.add_edge(
                        NodeKey::Medicinal(medicinal_index),
                        NodeKey::Formula(formula_index),
                        NetworkRelation::UsedIn,
                        formula.term.clone(),
                    );
                    count += 1;
                }
            }
        }
        count
    }

    fn build_point_combinations(&mut self, dictionary: &Dictionary) -> usize {
        let methods: Vec<usize> = self
            .method_names
            .values()
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let mut count = 0;
        for method_index in methods {
            let Some(method) = dictionary.entries.get(method_index) else {
                continue;
            };
            let mut context = String::new();
            let mut current_role = String::new();
            let mut pending: Vec<(String, String, usize)> = Vec::new();
            let mut group_start_line: Option<usize> = None;
            let mut in_list = false;
            for (offset, line) in method.body.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let line_number = method.line + offset;
                let normalized = normalize(trimmed);
                if is_list_heading(trimmed) {
                    in_list = true;
                    context.clear();
                    current_role.clear();
                    pending.clear();
                    group_start_line = None;
                    continue;
                }
                if !in_list {
                    continue;
                }
                let points = find_point_ids(dictionary, &normalized);
                let role = role_prefix(trimmed);
                if role.is_some() || !points.is_empty() {
                    if let Some(role) = role {
                        current_role = role.to_owned();
                    }
                    if group_start_line.is_none() {
                        group_start_line = Some(line_number);
                    }
                    for (id, point_index) in points {
                        pending.push((current_role.clone(), id, point_index));
                    }
                    continue;
                }
                if pending.is_empty() {
                    if is_group_heading(trimmed) {
                        context = trimmed.to_owned();
                    }
                    continue;
                }
                if is_group_heading(trimmed) {
                    count += usize::from(self.push_combination(
                        method_index,
                        &method.file,
                        &method.headword,
                        &context,
                        &context,
                        &pending,
                        group_start_line.unwrap_or(method.line),
                    ));
                    pending.clear();
                    current_role.clear();
                    group_start_line = None;
                    context = trimmed.to_owned();
                } else {
                    count += usize::from(self.push_combination(
                        method_index,
                        &method.file,
                        &method.headword,
                        &context,
                        trimmed,
                        &pending,
                        group_start_line.unwrap_or(method.line),
                    ));
                    pending.clear();
                    current_role.clear();
                    group_start_line = None;
                    context.clear();
                }
            }
            if !pending.is_empty() {
                count += usize::from(self.push_combination(
                    method_index,
                    &method.file,
                    &method.headword,
                    &context,
                    &context,
                    &pending,
                    group_start_line.unwrap_or(method.line),
                ));
            }
        }
        count
    }

    #[allow(clippy::too_many_arguments)]
    fn push_combination(
        &mut self,
        method_index: usize,
        file: &str,
        method_headword: &str,
        context: &str,
        indication: &str,
        pending: &[(String, String, usize)],
        line: usize,
    ) -> bool {
        let mut seen = HashSet::new();
        let mut points: Vec<(String, String, usize)> = Vec::new();
        for point in pending {
            if seen.insert(point.2) {
                points.push(point.clone());
            }
        }
        if points.is_empty() {
            return false;
        }
        let label = format!(
            "{}: {}",
            if context.is_empty() {
                "combination"
            } else {
                context
            },
            points
                .iter()
                .map(|(role, id, _)| if role.is_empty() {
                    id.clone()
                } else {
                    format!("{role} {id}")
                })
                .collect::<Vec<_>>()
                .join(" + ")
        );
        let combination_index = self.combinations.len();
        self.combinations.push(Combination {
            context: context.to_owned(),
            indication: indication.to_owned(),
            label: label.clone(),
            file: file.to_owned(),
            line,
        });
        self.add_edge(
            NodeKey::Method(method_index),
            NodeKey::Combination(combination_index),
            NetworkRelation::HasCombination,
            label.clone(),
        );
        self.add_edge(
            NodeKey::Combination(combination_index),
            NodeKey::Method(method_index),
            NetworkRelation::Method,
            method_headword.to_owned(),
        );
        for (role, id, point_index) in &points {
            let point_key = NodeKey::Point(*point_index);
            let descriptor = if role.is_empty() {
                id.clone()
            } else {
                format!("{role} {id}")
            };
            self.add_edge(
                NodeKey::Combination(combination_index),
                point_key,
                NetworkRelation::UsesPoint,
                descriptor,
            );
            self.add_edge(
                point_key,
                NodeKey::Combination(combination_index),
                NetworkRelation::UsedInCombination,
                label.clone(),
            );
        }
        true
    }

    fn add_edge(&mut self, from: NodeKey, to: NodeKey, relation: NetworkRelation, label: String) {
        let edge = Edge {
            from,
            to,
            relation,
            label,
        };
        let outgoing = self.outgoing.entry(from).or_default();
        if !outgoing
            .iter()
            .any(|existing| existing.to == to && existing.relation == relation)
        {
            outgoing.push(edge.clone());
        }
        let incoming = self.incoming.entry(to).or_default();
        if !incoming
            .iter()
            .any(|existing| existing.from == from && existing.relation == relation)
        {
            incoming.push(edge);
        }
    }

    pub(crate) fn related(
        &self,
        dictionary: &Dictionary,
        request: &NetworkRequest,
    ) -> Result<NetworkResponse, DictionaryError> {
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
        let normalized = normalize(term);
        let Some(start) = self.resolve(request.domain, &normalized) else {
            return Err(DictionaryError::not_found(
                format!(
                    "No {} entry matches {term:?}.",
                    request.domain.as_str().replace('_', " ")
                ),
                Vec::new(),
            ));
        };

        let mut visited = HashSet::new();
        visited.insert(start);
        let mut queue = VecDeque::from([(start, 0usize)]);
        let mut found: Vec<(NodeKey, usize, EdgeDirection, Edge)> = Vec::new();
        let mut cap_hit = false;
        'traversal: while let Some((current, distance)) = queue.pop_front() {
            if distance >= request.depth {
                continue;
            }
            let mut edge_lists: Vec<(EdgeDirection, &Vec<Edge>)> = Vec::with_capacity(2);
            if matches!(
                request.direction,
                GraphDirection::Outgoing | GraphDirection::Both
            ) && let Some(edges) = self.outgoing.get(&current)
            {
                edge_lists.push((EdgeDirection::Outgoing, edges));
            }
            if matches!(
                request.direction,
                GraphDirection::Incoming | GraphDirection::Both
            ) && let Some(edges) = self.incoming.get(&current)
            {
                edge_lists.push((EdgeDirection::Incoming, edges));
            }
            for (direction, edges) in edge_lists {
                for edge in edges {
                    let neighbor = match direction {
                        EdgeDirection::Outgoing => edge.to,
                        EdgeDirection::Incoming => edge.from,
                    };
                    if !allowed(request.domain, neighbor)
                        || !allowed_edge(request.domain, edge)
                        || visited.contains(&neighbor)
                    {
                        continue;
                    }
                    if found.len() >= MAX_VISITED {
                        cap_hit = true;
                        break 'traversal;
                    }
                    visited.insert(neighbor);
                    found.push((neighbor, distance + 1, direction, edge.clone()));
                    queue.push_back((neighbor, distance + 1));
                }
            }
        }
        found.sort_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| node_order(left.0).cmp(&node_order(right.0)))
        });
        let total = found.len();
        let truncated = cap_hit || total > request.limit;
        let nodes = found
            .into_iter()
            .take(request.limit)
            .map(|(key, distance, direction, edge)| {
                self.node(dictionary, key, distance, direction, &edge)
            })
            .collect();
        let start_node = self.node(
            dictionary,
            start,
            0,
            EdgeDirection::Outgoing,
            &Edge {
                from: start,
                to: start,
                relation: NetworkRelation::Method,
                label: String::new(),
            },
        );
        Ok(NetworkResponse {
            query: term.to_owned(),
            term: start_node.term,
            node_type: start_node.node_type,
            domain: request.domain,
            direction: request.direction,
            depth: request.depth,
            nodes,
            total,
            truncated,
        })
    }

    fn resolve(&self, domain: NetworkDomain, normalized: &str) -> Option<NodeKey> {
        match domain {
            NetworkDomain::Formula => self
                .formula_names
                .get(normalized)
                .copied()
                .map(NodeKey::Formula),
            NetworkDomain::Medicinal => self
                .medicinal_names
                .get(normalized)
                .copied()
                .map(NodeKey::Medicinal)
                .or_else(|| {
                    self.formula_names
                        .get(normalized)
                        .copied()
                        .map(NodeKey::Formula)
                }),
            NetworkDomain::PointCombination => self
                .point_names
                .get(normalized)
                .copied()
                .map(NodeKey::Point)
                .or_else(|| {
                    self.method_names
                        .get(normalized)
                        .copied()
                        .map(NodeKey::Method)
                }),
        }
    }

    fn node(
        &self,
        dictionary: &Dictionary,
        key: NodeKey,
        distance: usize,
        edge_direction: EdgeDirection,
        edge: &Edge,
    ) -> NetworkNode {
        match key {
            NodeKey::Formula(index) => {
                let formula = &dictionary.formulas[index];
                appendix_node(
                    dictionary,
                    formula,
                    Source::Formulas,
                    NetworkNodeType::Formula,
                    distance,
                    edge_direction,
                    edge,
                    &self.node_term(dictionary, edge.from),
                )
            }
            NodeKey::Medicinal(index) => {
                let medicinal = &dictionary.medicinals[index];
                appendix_node(
                    dictionary,
                    medicinal,
                    Source::Medicinal,
                    NetworkNodeType::Medicinal,
                    distance,
                    edge_direction,
                    edge,
                    &self.node_term(dictionary, edge.from),
                )
            }
            NodeKey::Point(index) => {
                let point = &dictionary.meridians[index];
                appendix_node(
                    dictionary,
                    point,
                    Source::Meridian,
                    NetworkNodeType::Point,
                    distance,
                    edge_direction,
                    edge,
                    &self.node_term(dictionary, edge.from),
                )
            }
            NodeKey::Method(index) => {
                let entry = &dictionary.entries[index];
                NetworkNode {
                    node_type: NetworkNodeType::Method,
                    source: Source::Dictionary,
                    term: entry.headword.clone(),
                    id: None,
                    citation: search::entry_citation(dictionary, entry),
                    file: entry.file.clone(),
                    source_file: dictionary.source_file(&entry.file),
                    line: Some(entry.line),
                    distance,
                    edge_direction,
                    via: self.node_term(dictionary, edge.from),
                    relation: if edge_direction == EdgeDirection::Incoming {
                        edge.relation.inverse()
                    } else {
                        edge.relation
                    },
                    label: edge.label.clone(),
                }
            }
            NodeKey::Combination(index) => {
                let combination = &self.combinations[index];
                NetworkNode {
                    node_type: NetworkNodeType::Combination,
                    source: Source::Dictionary,
                    term: combination.label.clone(),
                    id: None,
                    citation: format!(
                        "{}, {}:{}",
                        dictionary.title(),
                        combination.file,
                        combination.line
                    ),
                    file: combination.file.clone(),
                    source_file: dictionary.source_file(&combination.file),
                    line: Some(combination.line),
                    distance,
                    edge_direction,
                    via: self.node_term(dictionary, edge.from),
                    relation: if edge_direction == EdgeDirection::Incoming {
                        edge.relation.inverse()
                    } else {
                        edge.relation
                    },
                    label: if combination.indication.is_empty() {
                        combination.context.clone()
                    } else {
                        combination.indication.clone()
                    },
                }
            }
        }
    }

    fn node_term(&self, dictionary: &Dictionary, key: NodeKey) -> String {
        match key {
            NodeKey::Formula(index) => dictionary
                .formulas
                .get(index)
                .map(|entry| entry.term.clone())
                .unwrap_or_default(),
            NodeKey::Medicinal(index) => dictionary
                .medicinals
                .get(index)
                .map(|entry| entry.term.clone())
                .unwrap_or_default(),
            NodeKey::Point(index) => dictionary
                .meridians
                .get(index)
                .map(|entry| entry.term.clone())
                .unwrap_or_default(),
            NodeKey::Combination(index) => self
                .combinations
                .get(index)
                .map(|combination| combination.label.clone())
                .unwrap_or_default(),
            NodeKey::Method(index) => dictionary
                .entries
                .get(index)
                .map(|entry| entry.headword.clone())
                .unwrap_or_default(),
        }
    }
}

fn allowed(domain: NetworkDomain, key: NodeKey) -> bool {
    match domain {
        NetworkDomain::Formula => matches!(key, NodeKey::Formula(_)),
        NetworkDomain::Medicinal => matches!(key, NodeKey::Formula(_) | NodeKey::Medicinal(_)),
        NetworkDomain::PointCombination => {
            matches!(
                key,
                NodeKey::Point(_) | NodeKey::Combination(_) | NodeKey::Method(_)
            )
        }
    }
}

fn allowed_edge(domain: NetworkDomain, edge: &Edge) -> bool {
    match domain {
        NetworkDomain::Formula => {
            matches!(edge.from, NodeKey::Formula(_)) && matches!(edge.to, NodeKey::Formula(_))
        }
        NetworkDomain::Medicinal => matches!(
            (edge.from, edge.to),
            (NodeKey::Formula(_), NodeKey::Medicinal(_))
                | (NodeKey::Medicinal(_), NodeKey::Formula(_))
        ),
        NetworkDomain::PointCombination => true,
    }
}

fn node_order(key: NodeKey) -> (u8, usize) {
    match key {
        NodeKey::Formula(index) => (0, index),
        NodeKey::Medicinal(index) => (1, index),
        NodeKey::Point(index) => (2, index),
        NodeKey::Method(index) => (3, index),
        NodeKey::Combination(index) => (4, index),
    }
}

fn add_appendix_names(map: &mut HashMap<String, usize>, entry: &AppendixEntry, index: usize) {
    for name in std::iter::once(&entry.term)
        .chain(entry.norm_terms.iter())
        .chain(std::iter::once(&entry.pinyin_ascii))
        .chain(std::iter::once(&entry.pinyin_toned))
    {
        let key = normalize(name);
        if !key.is_empty() {
            map.entry(key).or_insert(index);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn appendix_node(
    dictionary: &Dictionary,
    entry: &AppendixEntry,
    source: Source,
    node_type: NetworkNodeType,
    distance: usize,
    edge_direction: EdgeDirection,
    edge: &Edge,
    via: &str,
) -> NetworkNode {
    let mut citation = format!("{}, {}", dictionary.title(), entry.file);
    if let Some(id) = &entry.id {
        citation.push(':');
        citation.push_str(id);
    } else {
        citation.push(':');
        citation.push_str(&entry.line.to_string());
    }
    NetworkNode {
        node_type,
        source,
        term: entry.term.clone(),
        id: entry.id.clone(),
        citation,
        file: entry.file.clone(),
        source_file: dictionary.source_file(&entry.file),
        line: Some(entry.line),
        distance,
        edge_direction,
        via: via.to_owned(),
        relation: if edge_direction == EdgeDirection::Incoming {
            edge.relation.inverse()
        } else {
            edge.relation
        },
        label: edge.label.clone(),
    }
}

fn is_list_heading(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.ends_with('.') {
        return false;
    }
    let normalized = normalize(trimmed);
    normalized.starts_with("combining ") && normalized.ends_with("points")
}

fn is_group_heading(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.contains(',')
        || trimmed.contains('.')
        || trimmed.contains(';')
        || trimmed.contains(':')
    {
        return false;
    }
    trimmed.split_whitespace().count() <= 3
}

fn role_prefix(line: &str) -> Option<&'static str> {
    let mut chars = line.chars();
    let marker = chars.next()?;
    if chars.next() != Some('.') {
        return None;
    }
    match marker {
        'S' => Some("S"),
        'N' => Some("N"),
        'T' => Some("T"),
        'A' => Some("A"),
        'L' => Some("L"),
        'D' => Some("D"),
        _ => None,
    }
}

fn find_point_ids(dictionary: &Dictionary, normalized_line: &str) -> Vec<(String, usize)> {
    let mut matches: Vec<(usize, String, usize)> = dictionary
        .point_ids
        .iter()
        .filter(|(id, _)| id.len() >= 3 && contains_id(normalized_line, id))
        .map(|(id, index)| {
            let display = dictionary
                .meridians
                .get(*index)
                .and_then(|point| point.id.clone())
                .unwrap_or_else(|| id.clone());
            (display.len(), display, *index)
        })
        .collect();
    matches.sort_by_key(|left| std::cmp::Reverse(left.0));
    matches.dedup_by(|left, right| left.2 == right.2);
    matches
        .into_iter()
        .map(|(_, id, index)| (id, index))
        .collect()
}

fn contains_id(line: &str, id: &str) -> bool {
    let mut start = 0;
    while let Some(offset) = line[start..].find(id) {
        let position = start + offset;
        let before = line[..position].chars().next_back();
        let after = line[position + id.len()..].chars().next();
        let boundary = |character: Option<char>| {
            !character
                .is_some_and(|character| character.is_ascii_alphanumeric() || character == '-')
        };
        if boundary(before) && boundary(after) {
            return true;
        }
        start = position + id.len();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::model::{Alias, AppendixEntry, Entry, GraphDirection, NetworkDomain};
    use crate::dictionary::normalize::normalize;

    fn appendix(
        source: Source,
        term: &str,
        id: Option<&str>,
        body: &str,
        file: &str,
        line: usize,
    ) -> AppendixEntry {
        AppendixEntry {
            source,
            file: file.to_owned(),
            line,
            id: id.map(str::to_owned),
            section: String::new(),
            subsection: None,
            term: term.to_owned(),
            simplified: String::new(),
            traditional: String::new(),
            pinyin_toned: String::new(),
            pinyin_ascii: String::new(),
            aliases: Vec::new(),
            body: body.to_owned(),
            norm_terms: vec![normalize(term)],
            norm_body: normalize(body),
        }
    }

    fn method_entry() -> Entry {
        Entry {
            headword: "combining source and network points".to_owned(),
            norm_headword: "combining source and network points".to_owned(),
            file: "10-c.md".to_owned(),
            line: 10,
            anchor: None,
            aliases: Vec::<Alias>::new(),
            body: "Combining Source and Network Points\n\nLung\n\nS. LU-9 (Great Abyss)\n\nN. LI-16 (Great Bone)\n\nSore throat.\n\nSpleen\n\nS. SP-3 (Supreme White)\n\nN. ST-40 (Bountiful Bulge)\n\nStiff tongue.".to_owned(),
            norm_body: String::new(),
            links: Vec::new(),
        }
    }

    fn local_distant_method_entry() -> Entry {
        Entry {
            headword: "combining local and distant points".to_owned(),
            norm_headword: "combining local and distant points".to_owned(),
            file: "10-c.md".to_owned(),
            line: 30,
            anchor: None,
            aliases: Vec::<Alias>::new(),
            body: "Combined needling of points close to the affected area with others at a distance from it.\n\nCombining Local (L) and Distant (D) Points\n\nForehead\n\nL. Hall of Impression (印堂 yin4 tang2)\n\nGB-14 (Yang2 White, 阳白 yang2 bai2)\n\nD. LI-4 (Union Valley, 合谷 he2 gu3)\n\nST-44 (Inner Court, 内庭 nei4 ting2)\n\nTemples\n\nL. Greater Yang2 (太阳 tai4 yang2)".to_owned(),
            norm_body: String::new(),
            links: Vec::new(),
        }
    }

    fn dictionary() -> Dictionary {
        let formulas = vec![
            appendix(
                Source::Formulas,
                "Base Decoction",
                Some("1"),
                "Qi formula.",
                "36-appendix-3.md",
                1,
            ),
            appendix(
                Source::Formulas,
                "Derived Decoction",
                Some("2"),
                "This is Base Decoction with the addition of ginger.\n\n□ Ginger 3g",
                "36-appendix-3.md",
                10,
            ),
            appendix(
                Source::Formulas,
                "Combined Decoction",
                Some("3"),
                "This is Base Decoction combined with Other Decoction.",
                "36-appendix-3.md",
                20,
            ),
            appendix(
                Source::Formulas,
                "Other Decoction",
                Some("4"),
                "Other formula.",
                "36-appendix-3.md",
                30,
            ),
        ];
        let medicinals = vec![
            appendix(
                Source::Medicinal,
                "ginger",
                None,
                "Warm medicinal.",
                "35-appendix-2.md",
                1,
            ),
            appendix(
                Source::Medicinal,
                "licorice",
                None,
                "Sweet medicinal.",
                "35-appendix-2.md",
                10,
            ),
        ];
        let meridians = vec![
            appendix(
                Source::Meridian,
                "Great Abyss",
                Some("LU-9"),
                "Point.",
                "34-appendix-1.md",
                1,
            ),
            appendix(
                Source::Meridian,
                "Great Bone",
                Some("LI-16"),
                "Point.",
                "34-appendix-1.md",
                2,
            ),
            appendix(
                Source::Meridian,
                "Supreme White",
                Some("SP-3"),
                "Point.",
                "34-appendix-1.md",
                3,
            ),
            appendix(
                Source::Meridian,
                "Bountiful Bulge",
                Some("ST-40"),
                "Point.",
                "34-appendix-1.md",
                4,
            ),
            appendix(
                Source::Meridian,
                "Union Valley",
                Some("LI-4"),
                "Point.",
                "34-appendix-1.md",
                5,
            ),
            appendix(
                Source::Meridian,
                "Inner Court",
                Some("ST-44"),
                "Point.",
                "34-appendix-1.md",
                6,
            ),
            appendix(
                Source::Meridian,
                "Yang White",
                Some("GB-14"),
                "Point.",
                "34-appendix-1.md",
                7,
            ),
        ];
        let mut dictionary = Dictionary::empty();
        dictionary.formulas = formulas;
        dictionary.medicinals = medicinals;
        dictionary.meridians = meridians;
        dictionary.entries.push(method_entry());
        dictionary.entries.push(local_distant_method_entry());
        for (id, index) in [
            ("lu-9", 0),
            ("li-16", 1),
            ("sp-3", 2),
            ("st-40", 3),
            ("li-4", 4),
            ("st-44", 5),
            ("gb-14", 6),
        ] {
            dictionary.point_ids.insert(id.to_owned(), index);
        }
        dictionary
    }

    fn request(term: &str, domain: NetworkDomain) -> NetworkRequest {
        NetworkRequest {
            term: term.to_owned(),
            domain,
            direction: GraphDirection::Both,
            depth: 1,
            limit: 50,
        }
    }

    #[test]
    fn formula_relations_are_built_from_explicit_prose() {
        let dictionary = dictionary();
        let (network, _) = Network::build(&dictionary);
        let response = network
            .related(
                &dictionary,
                &request("Base Decoction", NetworkDomain::Formula),
            )
            .unwrap();
        assert_eq!(response.total, 2);
        assert!(response.nodes.iter().any(|node| {
            node.term == "Derived Decoction" && node.relation == NetworkRelation::HasDerivative
        }));
        assert!(response.nodes.iter().any(|node| {
            node.term == "Combined Decoction" && node.relation == NetworkRelation::HasDerivative
        }));
    }

    #[test]
    fn formula_medicinal_edges_come_from_ingredient_lines() {
        let dictionary = dictionary();
        let (network, _) = Network::build(&dictionary);
        let response = network
            .related(
                &dictionary,
                &request("Derived Decoction", NetworkDomain::Medicinal),
            )
            .unwrap();
        assert_eq!(response.total, 1);
        assert_eq!(response.nodes[0].term, "ginger");
        assert_eq!(response.nodes[0].relation, NetworkRelation::Contains);
    }

    #[test]
    fn point_combinations_are_grouped_and_resolve_by_point() {
        let dictionary = dictionary();
        let (network, _) = Network::build(&dictionary);
        let response = network
            .related(
                &dictionary,
                &request("LU-9", NetworkDomain::PointCombination),
            )
            .unwrap();
        assert_eq!(response.total, 1);
        assert!(response.nodes[0].term.contains("Lung"));
        assert!(response.nodes[0].term.contains("LU-9"));
        assert!(response.nodes[0].term.contains("LI-16"));
        assert_eq!(
            response.nodes[0].relation,
            NetworkRelation::UsedInCombination
        );
    }

    #[test]
    fn local_and_distant_combinations_keep_the_region_as_context() {
        let dictionary = dictionary();
        let (network, stats) = Network::build(&dictionary);
        assert_eq!(stats.point_combinations, 3);
        let response = network
            .related(
                &dictionary,
                &request("LI-4", NetworkDomain::PointCombination),
            )
            .unwrap();
        assert_eq!(response.total, 1);
        let node = &response.nodes[0];
        assert_eq!(node.node_type, NetworkNodeType::Combination);
        assert_eq!(node.term, "Forehead: L GB-14 + D LI-4 + D ST-44");
        assert_eq!(node.label, "Forehead");
        assert_eq!(node.relation, NetworkRelation::UsedInCombination);
    }

    #[test]
    fn point_id_matching_respects_boundaries() {
        assert!(contains_id("n. li-16 (great bone)", "li-16"));
        assert!(!contains_id("n. li-16 (great bone)", "li-1"));
        assert!(contains_id("s. lu-9 (great abyss)", "lu-9"));
    }
}
