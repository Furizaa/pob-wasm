pub mod bases;
pub mod gems;
pub mod misc;

use crate::error::DataError;
use crate::passive_tree::PassiveTree;
use gems::GemsMap;
use misc::MiscData;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct RawGameData {
    gems: GemsMap,
    misc: MiscData,
    #[serde(default)]
    tree: Option<serde_json::Value>,
}

/// Immutable game data shared across all calculations.
/// Loaded once at startup from the JSON files produced by data-extractor.
#[derive(Debug, Clone)]
pub struct GameData {
    pub gems: GemsMap,
    pub misc: Arc<MiscData>,
    pub passive_tree: PassiveTree,
}

impl GameData {
    /// Parse a combined JSON string containing all game data sections.
    /// The JSON structure matches what `data-extractor` produces.
    pub fn from_json(json: &str) -> Result<Self, DataError> {
        let raw: RawGameData = serde_json::from_str(json)?;
        let passive_tree = if let Some(tree_val) = raw.tree {
            let tree_json = serde_json::to_string(&tree_val)?;
            PassiveTree::from_json(&tree_json)?
        } else {
            PassiveTree {
                nodes: std::collections::HashMap::new(),
            }
        };
        Ok(Self {
            gems: raw.gems,
            misc: Arc::new(raw.misc),
            passive_tree,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_data_includes_passive_tree() {
        let json = r#"{
            "gems": {},
            "misc": {
                "game_constants": {},
                "character_constants": {},
                "monster_life_table": [],
                "monster_damage_table": [],
                "monster_evasion_table": [],
                "monster_accuracy_table": [],
                "monster_ally_life_table": [],
                "monster_ally_damage_table": [],
                "monster_ailment_threshold_table": [],
                "monster_phys_conversion_multi_table": []
            },
            "tree": {
                "nodes": {
                    "50459": { "id": 50459, "name": "Thick Skin", "stats": ["+10 to maximum Life"], "out": [] }
                }
            }
        }"#;
        let data = GameData::from_json(json).unwrap();
        assert!(data.passive_tree.nodes.contains_key(&50459));
    }

    #[test]
    fn load_from_json_stub_parses() {
        let json = r#"{
            "gems": {},
            "misc": {
                "game_constants": {},
                "character_constants": {},
                "monster_life_table": [],
                "monster_damage_table": [],
                "monster_evasion_table": [],
                "monster_accuracy_table": [],
                "monster_ally_life_table": [],
                "monster_ally_damage_table": [],
                "monster_ailment_threshold_table": [],
                "monster_phys_conversion_multi_table": []
            }
        }"#;
        let data = GameData::from_json(json).unwrap();
        assert_eq!(data.gems.len(), 0);
    }
}
