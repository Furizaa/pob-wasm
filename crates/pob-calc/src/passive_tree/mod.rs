use crate::error::DataError;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
struct RawNode {
    id: u32,
    name: String,
    #[serde(default)]
    stats: Vec<String>,
    #[serde(rename = "out", default)]
    out_ids: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct PassiveNode {
    pub id: u32,
    pub name: String,
    /// Human-readable stat descriptions, e.g. ["+10 to maximum Life"]
    pub stats: Vec<String>,
    /// IDs of nodes this one connects to
    pub linked_ids: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct PassiveTree {
    pub nodes: HashMap<u32, PassiveNode>,
}

impl PassiveTree {
    pub fn from_json(json: &str) -> Result<Self, DataError> {
        #[derive(Deserialize)]
        struct Root {
            nodes: HashMap<String, RawNode>,
        }
        let root: Root = serde_json::from_str(json)?;
        let nodes = root
            .nodes
            .into_values()
            .map(|raw| {
                let node = PassiveNode {
                    id: raw.id,
                    name: raw.name,
                    stats: raw.stats,
                    linked_ids: raw.out_ids,
                };
                (raw.id, node)
            })
            .collect();
        Ok(Self { nodes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_TREE_JSON: &str = r#"{
        "nodes": {
            "50459": { "id": 50459, "name": "Thick Skin", "stats": ["+10 to maximum Life"], "out": [47175] },
            "47175": { "id": 47175, "name": "Quick Recovery", "stats": [], "out": [] }
        }
    }"#;

    #[test]
    fn loads_nodes_from_json() {
        let tree = PassiveTree::from_json(MINIMAL_TREE_JSON).unwrap();
        assert_eq!(tree.nodes.len(), 2);
        let node = tree.nodes.get(&50459).unwrap();
        assert_eq!(node.name, "Thick Skin");
        assert!(node.linked_ids.contains(&47175));
    }
}
