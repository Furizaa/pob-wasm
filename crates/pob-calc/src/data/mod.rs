pub mod bases;
pub mod gems;
pub mod misc;

use crate::error::DataError;
use gems::GemsMap;
use misc::MiscData;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct RawGameData {
    gems: GemsMap,
    misc: MiscData,
}

/// Immutable game data shared across all calculations.
/// Loaded once at startup from the JSON files produced by data-extractor.
#[derive(Debug, Clone)]
pub struct GameData {
    pub gems: GemsMap,
    pub misc: Arc<MiscData>,
}

impl GameData {
    /// Parse a combined JSON string containing all game data sections.
    /// The JSON structure matches what `data-extractor` produces.
    pub fn from_json(json: &str) -> Result<Self, DataError> {
        let raw: RawGameData = serde_json::from_str(json)?;
        Ok(Self {
            gems: raw.gems,
            misc: Arc::new(raw.misc),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
