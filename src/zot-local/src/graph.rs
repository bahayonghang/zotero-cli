//! Local relationship knowledge-graph assembly and analysis.
//!
//! Pure computation over already-loaded [`Item`]s plus explicit Zotero
//! relations. Database access lives in [`crate::db`]; this module never opens
//! SQLite. Every algorithm here is deterministic — stable iteration order, no
//! randomness — so identical input always yields an identical
//! [`KnowledgeGraph`], which keeps the CLI output and the unit tests stable.

use std::collections::{BTreeMap, HashMap};

use zot_core::{
    CommunitySummary, EdgeRelation, GraphEdge, GraphMetrics, GraphNode, GraphOptions, Item,
    KnowledgeGraph, RankedNode, TagSummary,
};

const COAUTHOR_WEIGHT: i64 = 8;
const TAG_WEIGHT: i64 = 5;
const COLLECTION_WEIGHT: i64 = 1;
const RELATED_WEIGHT: i64 = 100;
const COMMUNITY_TOP_TAGS: usize = 3;
const MAX_LABEL_ROUNDS: usize = 20;

/// Per-pair accumulator collected from the inverted indexes before edges are
/// materialised.
#[derive(Default, Clone)]
struct PairAccum {
    coauthor: usize,
    tag: usize,
    collection: usize,
    related: bool,
}

/// Build a [`KnowledgeGraph`] from loaded items and explicit relation key
/// pairs `(source_key, target_key)`. Pure and deterministic.
pub(crate) fn assemble_graph(
    items: &[Item],
    explicit_relations: &[(String, String)],
    opts: &GraphOptions,
    scope: String,
) -> KnowledgeGraph {
    // Stable node order so indices, communities, and rankings are reproducible.
    let mut ordered: Vec<&Item> = items.iter().collect();
    ordered.sort_by(|a, b| a.key.cmp(&b.key));

    let index_of: HashMap<&str, usize> = ordered
        .iter()
        .enumerate()
        .map(|(idx, item)| (item.key.as_str(), idx))
        .collect();

    let mut pairs: HashMap<(usize, usize), PairAccum> = HashMap::new();

    if opts.relations.coauthor {
        for group in invert_authors(&ordered).values() {
            accumulate(group, opts.max_group_size, &mut pairs, |acc| acc.coauthor += 1);
        }
    }
    if opts.relations.tag {
        for group in invert_tags(&ordered).values() {
            accumulate(group, opts.max_group_size, &mut pairs, |acc| acc.tag += 1);
        }
    }
    if opts.relations.collection {
        for group in invert_collections(&ordered).values() {
            accumulate(group, opts.max_group_size, &mut pairs, |acc| {
                acc.collection += 1
            });
        }
    }
    if opts.relations.related {
        for (source, target) in explicit_relations {
            if let (Some(&i), Some(&j)) =
                (index_of.get(source.as_str()), index_of.get(target.as_str()))
                && i != j
            {
                pairs.entry(order_pair(i, j)).or_default().related = true;
            }
        }
    }

    // Materialise edges in a deterministic order.
    let mut pair_keys: Vec<(usize, usize)> = pairs.keys().copied().collect();
    pair_keys.sort_unstable();

    let node_count = ordered.len();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut degree = vec![0usize; node_count];
    let mut weighted = vec![0i64; node_count];
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); node_count];
    let mut edge_pairs: Vec<(usize, usize)> = Vec::new();

    for (i, j) in pair_keys {
        let Some(acc) = pairs.get(&(i, j)) else {
            continue;
        };
        let mut relations: Vec<EdgeRelation> = Vec::new();
        let mut weight: i64 = 0;
        if acc.related {
            relations.push(EdgeRelation::Related);
            weight += RELATED_WEIGHT;
        }
        if acc.coauthor > 0 {
            relations.push(EdgeRelation::Coauthor);
            weight += COAUTHOR_WEIGHT * acc.coauthor as i64;
        }
        if acc.tag >= opts.min_shared_tags && acc.tag > 0 {
            relations.push(EdgeRelation::Tag);
            weight += TAG_WEIGHT * acc.tag as i64;
        }
        if acc.collection > 0 {
            relations.push(EdgeRelation::Collection);
            weight += COLLECTION_WEIGHT * acc.collection as i64;
        }
        if relations.is_empty() {
            continue;
        }
        degree[i] += 1;
        degree[j] += 1;
        weighted[i] += weight;
        weighted[j] += weight;
        adjacency[i].push(j);
        adjacency[j].push(i);
        edge_pairs.push((i, j));
        edges.push(GraphEdge {
            source: ordered[i].key.clone(),
            target: ordered[j].key.clone(),
            weight,
            relations,
        });
    }

    let community = label_propagation(node_count, &adjacency);
    let connected_components = count_components(node_count, &edge_pairs);

    let nodes: Vec<GraphNode> = ordered
        .iter()
        .enumerate()
        .map(|(idx, item)| GraphNode {
            key: item.key.clone(),
            title: item.title.clone(),
            item_type: item.item_type.clone(),
            year: parse_year(item.date.as_deref()),
            first_author: item
                .creators
                .first()
                .map(|creator| creator.full_name())
                .filter(|name| !name.is_empty()),
            tags: item.tags.clone(),
            doi: item.doi.clone(),
            url: item.url.clone(),
            degree: degree[idx],
            weighted_degree: weighted[idx],
            community: community[idx],
        })
        .collect();

    let metrics = compute_metrics(&nodes, &ordered, &community, connected_components, opts.top_n);

    KnowledgeGraph {
        scope,
        nodes,
        edges,
        metrics,
    }
}

fn order_pair(i: usize, j: usize) -> (usize, usize) {
    if i <= j { (i, j) } else { (j, i) }
}

/// Emit every unordered pair inside `group`, skipping oversized groups so a
/// single popular tag/collection cannot create a dense hairball.
fn accumulate(
    group: &[usize],
    max_group_size: usize,
    pairs: &mut HashMap<(usize, usize), PairAccum>,
    mut bump: impl FnMut(&mut PairAccum),
) {
    if group.len() < 2 || group.len() > max_group_size {
        return;
    }
    for a in 0..group.len() {
        for b in (a + 1)..group.len() {
            bump(pairs.entry(order_pair(group[a], group[b])).or_default());
        }
    }
}

fn invert_authors(ordered: &[&Item]) -> BTreeMap<String, Vec<usize>> {
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, item) in ordered.iter().enumerate() {
        for creator in &item.creators {
            let name = creator.full_name().trim().to_lowercase();
            if !name.is_empty() {
                push_unique(groups.entry(name).or_default(), idx);
            }
        }
    }
    groups
}

fn invert_tags(ordered: &[&Item]) -> BTreeMap<String, Vec<usize>> {
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, item) in ordered.iter().enumerate() {
        for tag in &item.tags {
            groups.entry(tag.clone()).or_default().push(idx);
        }
    }
    groups
}

fn invert_collections(ordered: &[&Item]) -> BTreeMap<String, Vec<usize>> {
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, item) in ordered.iter().enumerate() {
        for collection in &item.collections {
            groups.entry(collection.clone()).or_default().push(idx);
        }
    }
    groups
}

/// Append `idx` only if it is not already the last pushed value. Items are
/// visited in increasing index order, so this keeps each group sorted and free
/// of duplicates (e.g. an author credited twice on one paper).
fn push_unique(group: &mut Vec<usize>, idx: usize) {
    if group.last() != Some(&idx) {
        group.push(idx);
    }
}

/// Deterministic label propagation. Labels start as each node's own index;
/// each round every node (in index order) adopts the neighbour label with the
/// highest count, breaking ties toward the smallest label. Returns community
/// ids normalised to a dense `0..k` range by first appearance.
fn label_propagation(node_count: usize, adjacency: &[Vec<usize>]) -> Vec<usize> {
    let mut labels: Vec<usize> = (0..node_count).collect();
    for _ in 0..MAX_LABEL_ROUNDS {
        let mut changed = false;
        for node in 0..node_count {
            let neighbours = &adjacency[node];
            if neighbours.is_empty() {
                continue;
            }
            let mut counts: BTreeMap<usize, usize> = BTreeMap::new();
            for &neighbour in neighbours {
                *counts.entry(labels[neighbour]).or_default() += 1;
            }
            // BTreeMap iterates labels ascending, so `>` keeps the smallest
            // label on ties.
            let mut best_label = labels[node];
            let mut best_count = 0usize;
            for (&label, &count) in &counts {
                if count > best_count {
                    best_count = count;
                    best_label = label;
                }
            }
            if best_label != labels[node] {
                labels[node] = best_label;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    normalise_labels(&labels)
}

fn normalise_labels(labels: &[usize]) -> Vec<usize> {
    let mut mapping: HashMap<usize, usize> = HashMap::new();
    let mut next = 0usize;
    labels
        .iter()
        .map(|&label| {
            *mapping.entry(label).or_insert_with(|| {
                let id = next;
                next += 1;
                id
            })
        })
        .collect()
}

/// Count connected components with union-find over the emitted edges.
fn count_components(node_count: usize, edge_pairs: &[(usize, usize)]) -> usize {
    if node_count == 0 {
        return 0;
    }
    let mut parent: Vec<usize> = (0..node_count).collect();
    for &(i, j) in edge_pairs {
        union(&mut parent, i, j);
    }
    let mut roots = 0usize;
    for node in 0..node_count {
        if find(&mut parent, node) == node {
            roots += 1;
        }
    }
    roots
}

fn find(parent: &mut [usize], node: usize) -> usize {
    let mut root = node;
    while parent[root] != root {
        root = parent[root];
    }
    // Path compression.
    let mut current = node;
    while parent[current] != root {
        let next = parent[current];
        parent[current] = root;
        current = next;
    }
    root
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra != rb {
        // Attach the larger root under the smaller for stability.
        if ra < rb {
            parent[rb] = ra;
        } else {
            parent[ra] = rb;
        }
    }
}

fn parse_year(date: Option<&str>) -> Option<String> {
    let date = date?;
    let mut run = String::new();
    for ch in date.chars() {
        if ch.is_ascii_digit() {
            run.push(ch);
            if run.len() == 4 {
                return Some(run);
            }
        } else if !run.is_empty() {
            run.clear();
        }
    }
    None
}

fn compute_metrics(
    nodes: &[GraphNode],
    ordered: &[&Item],
    community: &[usize],
    connected_components: usize,
    top_n: usize,
) -> GraphMetrics {
    let edge_count: usize = nodes.iter().map(|node| node.degree).sum::<usize>() / 2;

    let top_by_degree = rank_nodes(nodes, top_n, |node| node.degree as i64);
    let top_by_weighted_degree = rank_nodes(nodes, top_n, |node| node.weighted_degree);

    GraphMetrics {
        node_count: nodes.len(),
        edge_count,
        connected_components,
        top_by_degree,
        top_by_weighted_degree,
        communities: summarise_communities(ordered, community),
        top_authors: top_counts(ordered.iter().flat_map(|item| {
            item.creators
                .iter()
                .map(|creator| creator.full_name())
                .filter(|name| !name.is_empty())
        }), top_n),
        top_tags: top_counts(
            ordered.iter().flat_map(|item| item.tags.iter().cloned()),
            top_n,
        ),
    }
}

fn rank_nodes(nodes: &[GraphNode], top_n: usize, score: impl Fn(&GraphNode) -> i64) -> Vec<RankedNode> {
    let mut ranked: Vec<RankedNode> = nodes
        .iter()
        .map(|node| RankedNode {
            key: node.key.clone(),
            title: node.title.clone(),
            score: score(node),
        })
        .filter(|node| node.score > 0)
        .collect();
    ranked.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.key.cmp(&b.key)));
    ranked.truncate(top_n);
    ranked
}

fn summarise_communities(ordered: &[&Item], community: &[usize]) -> Vec<CommunitySummary> {
    let mut members: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (idx, &id) in community.iter().enumerate() {
        members.entry(id).or_default().push(idx);
    }
    let mut summaries: Vec<CommunitySummary> = members
        .into_iter()
        .map(|(id, idxs)| {
            let tags = top_counts(
                idxs.iter()
                    .filter_map(|&idx| ordered.get(idx))
                    .flat_map(|item| item.tags.iter().cloned()),
                COMMUNITY_TOP_TAGS,
            );
            CommunitySummary {
                id,
                size: idxs.len(),
                top_tags: tags.into_iter().map(|tag| tag.name).collect(),
            }
        })
        .collect();
    summaries.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.id.cmp(&b.id)));
    summaries
}

/// Count occurrences of each string and return the Top-N as `TagSummary`,
/// ordered by count desc then name asc.
fn top_counts(values: impl Iterator<Item = String>, top_n: usize) -> Vec<TagSummary> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    let mut ranked: Vec<TagSummary> = counts
        .into_iter()
        .map(|(name, count)| TagSummary { name, count })
        .collect();
    ranked.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    ranked.truncate(top_n);
    ranked
}
