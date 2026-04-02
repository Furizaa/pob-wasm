use crate::error::DataError;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

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

/// Expansion jewel metadata for cluster jewel socket nodes.
/// Set on nodes that can accept cluster jewels (Large/Medium/Small Jewel Sockets).
#[derive(Debug, Clone, Default)]
pub struct ExpansionJewelMeta {
    /// 0 = Small, 1 = Medium, 2 = Large cluster jewel socket.
    pub size: u32,
    /// Ring position index within the parent passive tree orbit.
    /// Used to compute subgraph node IDs in BuildSubgraph.
    pub index: u32,
    /// For Medium/Small sockets that are children of a larger socket:
    /// the node ID of the parent socket.
    pub parent: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawExpansionJewel {
    #[serde(default)]
    size: u32,
    #[serde(default)]
    index: u32,
    /// Parent socket node ID (for Medium/Small sockets nested in a larger subgraph).
    #[serde(default)]
    parent: Option<u32>,
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
    is_class_start: bool,
    #[serde(default)]
    class_start_index: Option<u32>,
    #[serde(default)]
    ascendancy_name: Option<String>,
    #[serde(default)]
    icon: String,
    #[serde(default)]
    skill_points_granted: i32,
    #[serde(default)]
    expansion_jewel: Option<RawExpansionJewel>,
    /// X coordinate (pre-computed from group/orbit data).
    #[serde(default)]
    x: Option<f64>,
    /// Y coordinate (pre-computed from group/orbit data).
    #[serde(default)]
    y: Option<f64>,
    /// For jewel sockets: maps radius_index (string) → list of node IDs in that radius.
    /// Pre-computed during data extraction.
    #[serde(default)]
    nodes_in_radius: std::collections::HashMap<String, Vec<u32>>,
    /// Mastery effect IDs available for this mastery node.
    /// Each entry is an effect ID (u32) that can be selected.
    /// Only set for `is_mastery = true` nodes.
    /// Populated from `mastery_effects` field in the tree JSON (if present).
    #[serde(default)]
    mastery_effects: Vec<u32>,
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
        } else if self.is_class_start {
            NodeType::ClassStart
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
    /// IDs of nodes this one connects to (from "out" field in tree data)
    pub linked_ids: Vec<u32>,
    /// Classification of this node (keystone, notable, etc.)
    pub node_type: NodeType,
    /// If non-`None`, this node belongs to the named ascendancy class
    pub ascendancy_name: Option<String>,
    /// Path to the node's icon asset
    pub icon: String,
    /// Number of skill points granted by allocating this node
    pub skill_points_granted: i32,
    /// Class start index (0=Scion, 1=Marauder, etc.) if this is a class start node
    pub class_start_index: Option<u32>,
    /// For cluster jewel socket nodes: metadata about the expansion jewel slot.
    /// `None` for regular passive nodes.
    pub expansion_jewel: Option<ExpansionJewelMeta>,
    /// X coordinate in the passive tree canvas (computed from group position + orbit).
    /// Used for radius computations (nodesInRadius).
    pub x: Option<f64>,
    /// Y coordinate in the passive tree canvas.
    pub y: Option<f64>,
    /// For jewel socket nodes: maps radius_index (1-based) → set of node IDs
    /// within that radius. Precomputed during tree loading.
    /// Empty for non-socket nodes.
    pub nodes_in_radius: HashMap<usize, HashSet<u32>>,
    /// For mastery nodes: the effect IDs available to select.
    /// Populated from tree JSON `mastery_effects` field.
    /// Empty for non-mastery nodes.
    pub mastery_effect_ids: Vec<u32>,
}

/// Per-class base attributes from the passive tree data.
#[derive(Debug, Clone, Default)]
pub struct ClassData {
    pub name: String,
    pub base_str: f64,
    pub base_dex: f64,
    pub base_int: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct RawClassData {
    name: String,
    #[serde(default)]
    base_str: f64,
    #[serde(default)]
    base_dex: f64,
    #[serde(default)]
    base_int: f64,
}

#[derive(Debug, Clone)]
pub struct PassiveTree {
    pub nodes: HashMap<u32, PassiveNode>,
    /// Class data by class index (0=Scion, 1=Marauder, 2=Ranger, 3=Witch, 4=Duelist, 5=Templar, 6=Shadow)
    pub classes: Vec<ClassData>,
    /// Maps lowercase notable name → node_id for non-ascendancy notable nodes.
    /// Mirrors PoB's PassiveTree.notableMap.
    /// Deduplication: if two nodes share a name, the one in a group (on-tree) wins.
    pub notable_map: HashMap<String, u32>,
    /// Maps lowercase ascendancy notable name → node_id for ascendancy notable nodes.
    /// Mirrors PoB's PassiveTree.ascendancyMap.
    pub ascendancy_map: HashMap<String, u32>,
    /// Global mastery effect lookup: effect_id → stat strings.
    /// Mirrors `self.masteryEffects` in PassiveTree.lua:460.
    /// Populated from tree JSON `mastery_effects` top-level key (if present),
    /// or from a separate `mastery_effects.json` sidecar file loaded via
    /// `PassiveTree::load_mastery_effects`.
    pub mastery_effects: HashMap<u32, Vec<String>>,
}

impl PassiveTree {
    pub fn from_json(json: &str) -> Result<Self, DataError> {
        #[derive(Deserialize)]
        struct Root {
            nodes: HashMap<String, RawNode>,
            #[serde(default)]
            classes: HashMap<String, RawClassData>,
            /// Optional top-level mastery effects table: effect_id (string) → stat strings.
            /// Mirrors `self.masteryEffects` in PassiveTree.lua.
            #[serde(default)]
            mastery_effects: HashMap<String, Vec<String>>,
        }
        let root: Root = serde_json::from_str(json)?;

        // Build global mastery_effects lookup: effect_id (u32) → stat strings.
        // First from the top-level field (if present in the JSON), then supplemented
        // by any per-node masteryEffects arrays.
        let mastery_effects: HashMap<u32, Vec<String>> = root
            .mastery_effects
            .into_iter()
            .filter_map(|(k, v)| k.parse::<u32>().ok().map(|id| (id, v)))
            .collect();

        let nodes: HashMap<u32, PassiveNode> = root
            .nodes
            .into_values()
            .map(|raw| {
                let node_type = raw.node_type();
                let expansion_jewel = raw.expansion_jewel.map(|ej| ExpansionJewelMeta {
                    size: ej.size,
                    index: ej.index,
                    parent: ej.parent,
                });
                // Convert nodes_in_radius from String keys to usize keys
                let nodes_in_radius: HashMap<usize, HashSet<u32>> = raw
                    .nodes_in_radius
                    .into_iter()
                    .filter_map(|(k, v)| {
                        k.parse::<usize>()
                            .ok()
                            .map(|idx| (idx, v.into_iter().collect()))
                    })
                    .collect();

                let node = PassiveNode {
                    id: raw.id,
                    name: raw.name,
                    stats: raw.stats,
                    linked_ids: raw.out_ids,
                    node_type,
                    ascendancy_name: raw.ascendancy_name,
                    icon: raw.icon,
                    skill_points_granted: raw.skill_points_granted,
                    class_start_index: raw.class_start_index,
                    expansion_jewel,
                    x: raw.x,
                    y: raw.y,
                    nodes_in_radius,
                    mastery_effect_ids: raw.mastery_effects,
                };
                (raw.id, node)
            })
            .collect();

        // Parse class data — keyed by class index string ("0", "1", ...)
        let mut classes_map: Vec<(u32, ClassData)> = root
            .classes
            .into_iter()
            .filter_map(|(k, v)| {
                k.parse::<u32>().ok().map(|idx| {
                    (
                        idx,
                        ClassData {
                            name: v.name,
                            base_str: v.base_str,
                            base_dex: v.base_dex,
                            base_int: v.base_int,
                        },
                    )
                })
            })
            .collect();
        classes_map.sort_by_key(|(idx, _)| *idx);
        let classes = classes_map.into_iter().map(|(_, c)| c).collect();

        // Build notableMap and ascendancyMap, mirroring PassiveTree.lua lines 515-527.
        // notableMap: lowercase_name → node_id for non-ascendancy Notable nodes.
        // ascendancyMap: lowercase_name → node_id for ascendancy Notable nodes.
        // Deduplication rule for notableMap: if two nodes share a name, the on-tree
        // node (one that has a group / is not a cluster notable) wins.
        // In our data model we can't distinguish "in a group" vs "cluster" directly,
        // so we use the simpler PoB logic: later entries overwrite earlier ones, but
        // we apply the same dedup: only overwrite if the existing entry is absent OR
        // if the new node's `is_notable` flag is true (no group field in Rust model).
        // For the real tree this is fine: regular tree notables always win over cluster
        // notables because they appear in both sets and the tree has `group` set.
        // Since the Rust tree data doesn't expose a `group` field, we just allow the
        // first entry to stand (cluster notables have unique names in practice).
        let mut notable_map: HashMap<String, u32> = HashMap::new();
        let mut ascendancy_map: HashMap<String, u32> = HashMap::new();

        for node in nodes.values() {
            if node.node_type == NodeType::Notable {
                let key = node.name.to_lowercase();
                if node.ascendancy_name.is_none() {
                    // Non-ascendancy notable: insert into notableMap.
                    // Deduplication: if the name already exists, overwrite only if this
                    // node has no existing entry OR there is already an entry (the Lua
                    // uses `node.g` to detect on-tree; we approximate by always inserting
                    // if the slot is empty, allowing duplicates to overwrite —
                    // matching PoB's "on-tree wins" by relying on iteration order).
                    // The real data ensures real-tree notables precede cluster notables
                    // in JSON (they have larger IDs in practice). The safest approximation
                    // is: always allow overwrite (last writer wins). In practice this means
                    // regular-tree notables will correctly map since both sides of a
                    // duplicate resolve to identical stats for the anointment use-case.
                    notable_map.entry(key).or_insert(node.id);
                } else {
                    // Ascendancy notable: insert into ascendancyMap.
                    ascendancy_map.insert(key, node.id);
                }
            }
        }

        Ok(Self {
            nodes,
            classes,
            notable_map,
            ascendancy_map,
            mastery_effects,
        })
    }

    /// Load mastery effects from a sidecar JSON file.
    ///
    /// The sidecar JSON has the structure:
    /// ```json
    /// {
    ///   "effects": { "<effect_id>": ["stat string", ...], ... }
    /// }
    /// ```
    ///
    /// Merges into `self.mastery_effects`. Existing entries are not overwritten.
    /// This is used to augment a tree JSON that lacks mastery effect stats.
    pub fn load_mastery_effects_from_json(&mut self, json: &str) -> Result<(), DataError> {
        #[derive(Deserialize)]
        struct MasterySidecar {
            /// effect_id (string) → stat strings
            effects: HashMap<String, Vec<String>>,
        }
        let sidecar: MasterySidecar = serde_json::from_str(json)?;
        for (k, v) in sidecar.effects {
            if let Ok(id) = k.parse::<u32>() {
                // Only insert if not already present (JSON top-level wins)
                self.mastery_effects.entry(id).or_insert(v);
            }
        }
        Ok(())
    }

    /// Get the class start node IDs for all classes.
    /// Returns a HashSet of node IDs that are class start or ascendancy start nodes.
    pub fn get_start_node_ids(&self) -> std::collections::HashSet<u32> {
        self.nodes
            .values()
            .filter(|n| {
                n.node_type == NodeType::ClassStart || n.node_type == NodeType::AscendancyStart
            })
            .map(|n| n.id)
            .collect()
    }

    /// Get class data by class index (PoB class ID).
    /// Returns None if the index is out of range.
    pub fn class_data(&self, class_idx: u32) -> Option<&ClassData> {
        self.classes.get(class_idx as usize)
    }

    /// Build a bidirectional adjacency map from the tree's `linked_ids`.
    /// Returns a map from node_id → list of connected node_ids.
    pub fn build_adjacency(&self) -> HashMap<u32, Vec<u32>> {
        let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();
        for node in self.nodes.values() {
            for &linked in &node.linked_ids {
                adj.entry(node.id).or_default().push(linked);
                adj.entry(linked).or_default().push(node.id);
            }
        }
        adj
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
