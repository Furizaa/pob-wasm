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
    /// Create a minimal valid GameData with reasonable defaults for testing.
    /// Game constants use real PoE values; all collections are empty.
    #[cfg(test)]
    pub fn default_for_test() -> Self {
        let json = r#"{
            "gems": {},
            "misc": {
                "game_constants": {
                    "base_maximum_all_resistances_%": 75,
                    "maximum_block_%": 75,
                    "base_maximum_spell_block_%": 75,
                    "max_power_charges": 3,
                    "max_frenzy_charges": 3,
                    "max_endurance_charges": 3,
                    "maximum_life_leech_rate_%_per_minute": 20,
                    "maximum_mana_leech_rate_%_per_minute": 20,
                    "maximum_life_leech_amount_per_leech_%_max_life": 10,
                    "maximum_mana_leech_amount_per_leech_%_max_mana": 10,
                    "maximum_energy_shield_leech_amount_per_leech_%_max_energy_shield": 10,
                    "base_number_of_totems_allowed": 1,
                    "impaled_debuff_number_of_reflected_hits": 8,
                    "soul_eater_maximum_stacks": 40
                },
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
            "tree": { "nodes": {} }
        }"#;
        Self::from_json(json).expect("default_for_test JSON must be valid")
    }

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
    fn default_for_test_creates_valid_game_data() {
        let data = GameData::default_for_test();
        // Collections are empty
        assert_eq!(data.gems.len(), 0);
        assert!(data.passive_tree.nodes.is_empty());
        assert!(data.bases.is_empty());
        assert!(data.uniques.is_empty());
        // Game constants have real PoE values
        assert_eq!(
            data.misc
                .game_constants
                .get("base_maximum_all_resistances_%"),
            Some(&75.0)
        );
        assert_eq!(data.misc.game_constants.get("maximum_block_%"), Some(&75.0));
        assert_eq!(
            data.misc.game_constants.get("max_power_charges"),
            Some(&3.0)
        );
        assert_eq!(
            data.misc.game_constants.get("max_frenzy_charges"),
            Some(&3.0)
        );
        assert_eq!(
            data.misc.game_constants.get("max_endurance_charges"),
            Some(&3.0)
        );
        assert_eq!(
            data.misc
                .game_constants
                .get("maximum_life_leech_rate_%_per_minute"),
            Some(&20.0)
        );
        assert_eq!(
            data.misc
                .game_constants
                .get("base_number_of_totems_allowed"),
            Some(&1.0)
        );
        assert_eq!(
            data.misc
                .game_constants
                .get("impaled_debuff_number_of_reflected_hits"),
            Some(&8.0)
        );
        assert_eq!(
            data.misc.game_constants.get("soul_eater_maximum_stacks"),
            Some(&40.0)
        );
        // Monster tables are empty
        assert!(data.misc.monster_life_table.is_empty());
        assert!(data.misc.monster_damage_table.is_empty());
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
