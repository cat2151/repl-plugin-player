use super::projection::array;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

// Only a single audio path justifies arrows. Event routing is independent of
// audio routing; it must not turn an ordinary instrument chain into a branch.
pub(super) fn project(track: &Value) -> (Vec<String>, bool) {
    let plugins: Vec<_> = array(&track["nodes"])
        .iter()
        .filter(|n| n["isPlugin"] == true)
        .collect();
    let fallback = || {
        (
            plugins
                .iter()
                .filter_map(|n| n["displayName"].as_str().map(String::from))
                .collect(),
            false,
        )
    };
    let Some(order) = serial_order(track) else {
        return fallback();
    };
    let mut ordered = Vec::new();
    for id in order {
        for plugin in &plugins {
            if plugin["nodeId"].as_str() == Some(id.as_str()) {
                let Some(name) = plugin["displayName"].as_str() else {
                    return fallback();
                };
                ordered.push(name.to_owned());
            }
        }
    }
    if ordered.len() != plugins.len() {
        return fallback();
    }
    (ordered, true)
}

fn endpoint<'a>(edge: &'a Value, nodes: &'a [Value]) -> Option<&'a str> {
    match edge["type"].as_str()? {
        "graphInput" => Some("graph:input"),
        "graphOutput" => Some("graph:output"),
        "plugin" => edge["nodeId"]
            .as_str()
            .filter(|id| !id.is_empty())
            .or_else(|| {
                nodes.iter().find_map(|node| {
                    (node["isPlugin"] == true && node["instanceId"] == edge["instanceId"])
                        .then(|| node["nodeId"].as_str())
                        .flatten()
                })
            }),
        _ => None,
    }
}

fn serial_order(track: &Value) -> Option<Vec<String>> {
    if track["graph"]["success"] != true {
        return None;
    }
    let nodes = array(&track["nodes"]);
    let mut edges = BTreeMap::<&str, BTreeSet<&str>>::new();
    let mut incoming = BTreeMap::<&str, BTreeSet<&str>>::new();
    for edge in array(&track["graph"]["connections"]) {
        if edge["busType"] != "audio" {
            continue;
        }
        let source = endpoint(&edge["source"], nodes)?;
        let target = endpoint(&edge["target"], nodes)?;
        edges.entry(source).or_default().insert(target);
        incoming.entry(target).or_default().insert(source);
    }
    if edges.values().any(|targets| targets.len() != 1)
        || incoming.values().any(|sources| sources.len() != 1)
    {
        return None;
    }
    let roots: Vec<_> = edges
        .keys()
        .filter(|id| !incoming.contains_key(*id))
        .collect();
    if roots.len() != 1 {
        return None;
    }
    let mut current = *roots[0];
    let mut visited = BTreeSet::new();
    let mut order = Vec::new();
    loop {
        if !visited.insert(current) {
            return None;
        }
        order.push(current.to_owned());
        match edges.get(current) {
            Some(targets) => current = targets.first()?,
            None => break,
        }
    }
    // A disconnected component or cycle cannot be represented by this path.
    (visited.len()
        == edges
            .keys()
            .chain(incoming.keys())
            .collect::<BTreeSet<_>>()
            .len())
    .then_some(order)
}

#[cfg(test)]
#[path = "plugin_names_tests.rs"]
mod tests;
