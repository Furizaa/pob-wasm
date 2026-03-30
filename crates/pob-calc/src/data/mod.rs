pub mod bases;
pub mod gems;
pub mod misc;
pub mod uniques;

use crate::error::DataError;
use crate::passive_tree::PassiveTree;
use bases::{BaseItemData, BaseItemMap};
use gems::GemsMap;
use misc::MiscData;
use serde::Deserialize;
use std::sync::Arc;
use uniques::{UniqueItemData, UniqueItemMap};

#[derive(Deserialize)]
struct RawGameData {
    gems: GemsMap,
    misc: MiscData,
    #[serde(default)]
    tree: Option<serde_json::Value>,
    #[serde(default)]
    bases: Option<Vec<BaseItemData>>,
    #[serde(default)]
    uniques: Option<Vec<UniqueItemData>>,
}

/// Immutable game data shared across all calculations.
/// Loaded once at startup from the JSON files produced by data-extractor.
#[derive(Debug, Clone)]
pub struct GameData {
    pub gems: GemsMap,
    pub misc: Arc<MiscData>,
    pub passive_tree: PassiveTree,
    pub bases: BaseItemMap,
    pub uniques: UniqueItemMap,
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
        let bases = BaseItemMap::from_vec(raw.bases.unwrap_or_default());
        let uniques = UniqueItemMap::from_vec(raw.uniques.unwrap_or_default());
        Ok(Self {
            gems: raw.gems,
            misc: Arc::new(raw.misc),
            passive_tree,
            bases,
            uniques,
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

    #[test]
    fn missing_bases_and_uniques_default_to_empty() {
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
        assert!(data.bases.is_empty());
        assert!(data.uniques.is_empty());
    }

    #[test]
    fn bases_and_uniques_load_when_present() {
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
            "bases": [
                {
                    "name": "Vaal Regalia",
                    "item_type": "Armour",
                    "socket_limit": 6
                },
                {
                    "name": "Harbinger Bow",
                    "item_type": "Weapon"
                }
            ],
            "uniques": [
                {
                    "name": "Headhunter",
                    "base_type": "Leather Belt",
                    "explicits": ["+60 to Strength"]
                },
                {
                    "name": "Tabula Rasa",
                    "base_type": "Simple Robe"
                }
            ]
        }"#;
        let data = GameData::from_json(json).unwrap();
        assert_eq!(data.bases.len(), 2);
        assert!(data.bases.get("Vaal Regalia").is_some());
        assert!(data.bases.get("Harbinger Bow").is_some());
        assert_eq!(data.uniques.len(), 2);
        assert!(data.uniques.get("Headhunter").is_some());
        assert_eq!(
            data.uniques.get("Headhunter").unwrap().base_type,
            "Leather Belt"
        );
        assert!(data.uniques.get("Tabula Rasa").is_some());
    }
}
