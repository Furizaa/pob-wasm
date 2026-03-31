use crate::error::DataError;
use serde::Deserialize;
use std::collections::HashMap;

/// Classification of a passive-tree node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Small,
    Notable,
    Keystone,
    JewelSocket,
    Mastery,
    AscendancyStart,
    ClassStart,
}

#[derive(Debug, Clone, Deserialize)]
struct RawNode {
    id: u32,
    name: String,
    #[serde(default)]
    stats: Vec<String>,
    #[serde(rename = "out", default)]
    out_ids: Vec<u32>,
    #[serde(default)]
    is_keystone: bool,
    #[serde(default)]
    is_notable: bool,
    #[serde(default)]
    is_jewel_socket: bool,
    #[serde(default)]
    is_mastery: bool,
    #[serde(default)]
    is_ascendancy_start: bool,
    #[serde(default)]
    ascendancy_name: Option<String>,
    #[serde(default)]
    icon: String,
    #[serde(default)]
    skill_points_granted: i32,
}

impl RawNode {
    fn node_type(&self) -> NodeType {
        if self.is_keystone {
            NodeType::Keystone
        } else if self.is_notable {
            NodeType::Notable
        } else if self.is_jewel_socket {
            NodeType::JewelSocket
        } else if self.is_mastery {
            NodeType::Mastery
        } else if self.is_ascendancy_start {
            NodeType::AscendancyStart
        } else {
            NodeType::Small
        }
    }
}

#[derive(Debug, Clone)]
pub struct PassiveNode {
    pub id: u32,
    pub name: String,
    /// Human-readable stat descriptions, e.g. ["+10 to maximum Life"]
    pub stats: Vec<String>,
    /// IDs of nodes this one connects to
    pub linked_ids: Vec<u32>,
    /// Classification of this node (keystone, notable, etc.)
    pub node_type: NodeType,
    /// If non-`None`, this node belongs to the named ascendancy class
    pub ascendancy_name: Option<String>,
    /// Path to the node's icon asset
    pub icon: String,
    /// Number of skill points granted by allocating this node
    pub skill_points_granted: i32,
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
                let node_type = raw.node_type();
                let node = PassiveNode {
                    id: raw.id,
                    name: raw.name,
                    stats: raw.stats,
                    linked_ids: raw.out_ids,
                    node_type,
                    ascendancy_name: raw.ascendancy_name,
                    icon: raw.icon,
                    skill_points_granted: raw.skill_points_granted,
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
        // Old-format JSON should default new fields gracefully
        assert_eq!(node.node_type, NodeType::Small);
        assert!(node.ascendancy_name.is_none());
        assert_eq!(node.icon, "");
        assert_eq!(node.skill_points_granted, 0);
    }

    #[test]
    fn node_types_parsed_from_json() {
        let json = r#"{
            "nodes": {
                "57279": {
                    "id": 57279, "name": "Blood Magic",
                    "stats": ["Removes all mana"],
                    "out": [],
                    "is_keystone": true, "is_notable": false,
                    "is_jewel_socket": false, "is_mastery": false,
                    "is_ascendancy_start": false, "ascendancy_name": null,
                    "icon": "Art/2DArt/SkillIcons/passives/BloodMagicKeystone.png",
                    "skill_points_granted": 1
                },
                "40867": {
                    "id": 40867, "name": "Bastion of Hope",
                    "stats": ["+5% Chance to Block Attack Damage"],
                    "out": [],
                    "is_keystone": false, "is_notable": true,
                    "is_jewel_socket": false, "is_mastery": false,
                    "is_ascendancy_start": false,
                    "ascendancy_name": "Guardian",
                    "icon": "", "skill_points_granted": 1
                },
                "26725": {
                    "id": 26725, "name": "",
                    "stats": [],
                    "out": [57279],
                    "is_keystone": false, "is_notable": false,
                    "is_jewel_socket": true, "is_mastery": false,
                    "is_ascendancy_start": false,
                    "ascendancy_name": null,
                    "icon": "", "skill_points_granted": 0
                }
            }
        }"#;

        let tree = PassiveTree::from_json(json).unwrap();
        assert_eq!(tree.nodes.len(), 3);

        // Blood Magic → Keystone
        let blood_magic = tree.nodes.get(&57279).unwrap();
        assert_eq!(blood_magic.node_type, NodeType::Keystone);
        assert_eq!(blood_magic.name, "Blood Magic");
        assert!(blood_magic.ascendancy_name.is_none());
        assert_eq!(
            blood_magic.icon,
            "Art/2DArt/SkillIcons/passives/BloodMagicKeystone.png"
        );
        assert_eq!(blood_magic.skill_points_granted, 1);

        // Bastion of Hope → Notable with ascendancy "Guardian"
        let bastion = tree.nodes.get(&40867).unwrap();
        assert_eq!(bastion.node_type, NodeType::Notable);
        assert_eq!(bastion.ascendancy_name.as_deref(), Some("Guardian"));
        assert_eq!(bastion.skill_points_granted, 1);

        // 26725 → JewelSocket
        let jewel = tree.nodes.get(&26725).unwrap();
        assert_eq!(jewel.node_type, NodeType::JewelSocket);
        assert!(jewel.ascendancy_name.is_none());
        assert_eq!(jewel.skill_points_granted, 0);
    }
}
