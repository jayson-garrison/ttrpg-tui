//! Stage 3: validate the reference graph to a fixpoint (D-20, D-21, D-16).
//!
//! Each pass handles one class of problem and starts over, so removing a user
//! item (which restores the built-in it replaced) is re-checked from scratch.

use std::collections::{BTreeMap, BTreeSet};

use super::load::{Candidate, Key, Layers};
use super::{ItemKind, LoadWarning, WarningKind};
use crate::error::MAX_DEPTH;

type Edges = BTreeMap<Key, Vec<Key>>;

/// Returns generators removed for a missing reference, mapped to the missing id.
pub(super) fn validate(
    layers: &mut Layers,
    warnings: &mut Vec<LoadWarning>,
) -> BTreeMap<String, String> {
    let mut excluded = BTreeMap::new();
    loop {
        let edges = effective_edges(layers);

        // 1. Dangling references.
        let dangling: Vec<(Key, Key)> = edges
            .iter()
            .filter_map(|(k, deps)| {
                deps.iter()
                    .find(|d| !edges.contains_key(*d))
                    .map(|d| (k.clone(), d.clone()))
            })
            .collect();
        if !dangling.is_empty() {
            for (key, missing) in dangling {
                if key.0 == ItemKind::Generator {
                    excluded.insert(key.1.clone(), missing.1.clone());
                }
                let msg = format!("refers to {:?}, which does not exist", missing.1);
                remove(layers, &key, WarningKind::MissingReference, msg, warnings);
            }
            continue;
        }

        // 2. Cycles: drop user members if any, otherwise every member.
        let cycles = cyclic_components(&edges);
        if !cycles.is_empty() {
            for members in cycles {
                let users: Vec<&Key> = members.iter().filter(|k| is_user(layers, k)).collect();
                let targets = if users.is_empty() {
                    members.iter().collect()
                } else {
                    users
                };
                let names: Vec<&str> = members.iter().map(|k| k.1.as_str()).collect();
                let msg = format!("is part of a reference cycle ({})", names.join(" -> "));
                for key in targets {
                    remove(layers, key, WarningKind::Cycle, msg.clone(), warnings);
                }
            }
            continue;
        }

        // 3. Nesting depth (same-kind references; table depth restarts per slot).
        if let Some(targets) = too_deep(&edges, layers) {
            let msg = format!("is nested more than {MAX_DEPTH} levels deep");
            for t in &targets {
                remove(layers, t, WarningKind::DepthExceeded, msg.clone(), warnings);
            }
            continue;
        }
        return excluded;
    }
}

fn effective<'a>(layers: &'a Layers, key: &Key) -> Option<&'a Candidate> {
    layers.get(key).and_then(|s| s.last())
}

fn is_user(layers: &Layers, key: &Key) -> bool {
    effective(layers, key).is_some_and(|c| c.user)
}

fn effective_edges(layers: &Layers) -> Edges {
    layers
        .iter()
        .filter_map(|(k, stack)| stack.last().map(|c| (k.clone(), c.edges())))
        .collect()
}

/// Remove the effective item. A user item takes all user layers of that key
/// with it, which restores the built-in beneath (if any).
fn remove(
    layers: &mut Layers,
    key: &Key,
    kind: WarningKind,
    message: String,
    warnings: &mut Vec<LoadWarning>,
) {
    let Some(stack) = layers.get_mut(key) else {
        return;
    };
    let Some(top) = stack.last() else {
        return;
    };
    warnings.push(LoadWarning {
        origin: top.origin.clone(),
        item: Some(key.clone()),
        kind,
        message,
    });
    if top.user {
        stack.retain(|c| !c.user);
    } else {
        stack.clear();
    }
    if stack.is_empty() {
        layers.remove(key);
    }
}

/// Nodes reachable from `start` by one or more edges.
fn reach(edges: &Edges, start: &Key) -> BTreeSet<Key> {
    let mut seen = BTreeSet::new();
    let mut stack: Vec<&Key> = edges
        .get(start)
        .map(|d| d.iter().collect())
        .unwrap_or_default();
    while let Some(n) = stack.pop() {
        if seen.insert(n.clone()) {
            if let Some(deps) = edges.get(n) {
                stack.extend(deps);
            }
        }
    }
    seen
}

/// Strongly connected components that contain a cycle (size > 1 or a self-loop).
fn cyclic_components(edges: &Edges) -> Vec<Vec<Key>> {
    let reaches: BTreeMap<&Key, BTreeSet<Key>> = edges
        .keys()
        .map(|k| (k, reach(edges, k)))
        .filter(|(k, r)| r.contains(*k))
        .collect();
    let mut done: BTreeSet<&Key> = BTreeSet::new();
    let mut out = Vec::new();
    for (&k, r) in &reaches {
        if done.contains(k) {
            continue;
        }
        let members: Vec<Key> = r
            .iter()
            .filter(|m| reaches.get(m).is_some_and(|rm| rm.contains(k)))
            .cloned()
            .collect();
        done.extend(reaches.keys().filter(|m| members.contains(m)));
        out.push(members);
    }
    out
}

/// The first node (in key order) whose same-kind nesting height exceeds the
/// limit, with the items to remove: user items below it if any, else itself.
fn too_deep(edges: &Edges, layers: &Layers) -> Option<Vec<Key>> {
    let same_kind: Edges = edges
        .iter()
        .map(|(k, deps)| {
            (
                k.clone(),
                deps.iter().filter(|d| d.0 == k.0).cloned().collect(),
            )
        })
        .collect();
    let heights = heights(&same_kind);
    let key = same_kind
        .keys()
        .find(|k| heights.get(*k).copied().unwrap_or(0) > MAX_DEPTH)?
        .clone();
    let mut below: BTreeSet<Key> = reach(&same_kind, &key);
    below.insert(key.clone());
    let users: Vec<Key> = below.into_iter().filter(|k| is_user(layers, k)).collect();
    let targets = if users.is_empty() {
        vec![key.clone()]
    } else {
        users
    };
    Some(targets)
}

/// Longest chain of references below each node. The graph must be acyclic.
fn heights(edges: &Edges) -> BTreeMap<Key, usize> {
    let mut h: BTreeMap<Key, usize> = BTreeMap::new();
    for start in edges.keys() {
        let mut stack = vec![start];
        while let Some(&node) = stack.last() {
            let deps = edges.get(node).map(Vec::as_slice).unwrap_or_default();
            let pending: Vec<&Key> = deps.iter().filter(|d| !h.contains_key(*d)).collect();
            if pending.is_empty() {
                let height = deps
                    .iter()
                    .filter_map(|d| h.get(d))
                    .map(|x| x + 1)
                    .max()
                    .unwrap_or(0);
                h.insert(node.clone(), height);
                stack.pop();
            } else {
                stack.extend(pending);
            }
        }
    }
    h
}
