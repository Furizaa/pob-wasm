use serde::Deserialize;
use std::collections::HashMap;

/// A single stat attached to a gem (constant or quality).
#[derive(Debug, Clone, Deserialize)]
pub struct GemStatEntry {
    #[serde(default)]
    pub stat_id: String,
    #[serde(default)]
    pub value: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GemLevelData {
    pub level: u8,
    #[serde(default)]
    pub level_requirement: u32,
    #[serde(default)]
    pub stat_values: Vec<f64>,
    // Legacy damage fields (old JSON format)
    #[serde(default)]
    pub phys_min: f64,
    #[serde(default)]
    pub phys_max: f64,
    #[serde(default)]
    pub fire_min: f64,
    #[serde(default)]
    pub fire_max: f64,
    #[serde(default)]
    pub cold_min: f64,
    #[serde(default)]
    pub cold_max: f64,
    #[serde(default)]
    pub lightning_min: f64,
    #[serde(default)]
    pub lightning_max: f64,
    #[serde(default)]
    pub chaos_min: f64,
    #[serde(default)]
    pub chaos_max: f64,
    // Fields present in both old and new format
    #[serde(default)]
    pub crit_chance: f64,
    #[serde(default)]
    pub damage_effectiveness: f64,
    #[serde(default)]
    pub attack_speed_mult: f64,
    // New fields from the extractor
    #[serde(default)]
    pub cast_time: f64,
    #[serde(default)]
    pub mana_cost: f64,
    #[serde(default)]
    pub life_cost: f64,
    #[serde(default)]
    pub mana_multiplier: f64,
    #[serde(default)]
    pub stored_uses: u32,
    #[serde(default)]
    pub cooldown: f64,
    #[serde(default)]
    pub duration: f64,
    // Reservation fields (auras, heralds, etc.)
    #[serde(default)]
    pub mana_reservation_flat: f64,
    #[serde(default)]
    pub mana_reservation_percent: f64,
    #[serde(default)]
    pub life_reservation_flat: f64,
    #[serde(default)]
    pub life_reservation_percent: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GemData {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub is_support: bool,
    /// Skill types — may be numeric IDs (old format) or string names (new format).
    /// Use `#[serde(deserialize_with)]` would be complex; instead we keep both
    /// possible representations via an untagged enum helper.
    #[serde(default)]
    pub skill_types: Vec<String>,
    #[serde(default)]
    pub levels: Vec<GemLevelData>,
    // New fields from the extractor
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub cast_time: f64,
    #[serde(default)]
    pub base_effectiveness: f64,
    #[serde(default)]
    pub incremental_effectiveness: f64,
    #[serde(default)]
    pub base_flags: Vec<String>,
    #[serde(default)]
    pub mana_multiplier_at_20: f64,
    // Support gem matching fields
    #[serde(default)]
    pub require_skill_types: Vec<String>,
    #[serde(default)]
    pub add_skill_types: Vec<String>,
    #[serde(default)]
    pub exclude_skill_types: Vec<String>,
    // Stat definitions
    #[serde(default)]
    pub constant_stats: Vec<GemStatEntry>,
    #[serde(default)]
    pub quality_stats: Vec<GemStatEntry>,
    #[serde(default)]
    pub stats: Vec<String>,
}

pub type GemsMap = HashMap<String, GemData>;

/// Mirrors POB's SkillType constants (Common.lua).
/// Used to determine how a skill interacts with the mod system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillTypeFlags(pub u64);

impl SkillTypeFlags {
    pub const NONE: Self = SkillTypeFlags(0);
    pub const ATTACK: Self = SkillTypeFlags(1 << 0);
    pub const SPELL: Self = SkillTypeFlags(1 << 1);
    pub const PROJECTILE: Self = SkillTypeFlags(1 << 2);
    pub const AREA: Self = SkillTypeFlags(1 << 3);
    pub const DURATION: Self = SkillTypeFlags(1 << 4);
    pub const MELEE: Self = SkillTypeFlags(1 << 5);
    pub const DAMAGE: Self = SkillTypeFlags(1 << 6);

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    /// Build a `SkillTypeFlags` from a list of string skill-type names.
    pub fn from_vec(names: &[String]) -> Self {
        let mut bits = 0u64;
        for name in names {
            let flag = match name.to_lowercase().as_str() {
                "attack" => Self::ATTACK.0,
                "spell" => Self::SPELL.0,
                "projectile" => Self::PROJECTILE.0,
                "area" => Self::AREA.0,
                "duration" => Self::DURATION.0,
                "melee" => Self::MELEE.0,
                "damage" => Self::DAMAGE.0,
                _ => 0,
            };
            bits |= flag;
        }
        SkillTypeFlags(bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_format_deserializes() {
        let json = r#"{
            "id": "Fireball",
            "display_name": "Fireball",
            "is_support": false,
            "color": "Int",
            "skill_types": ["Spell", "Projectile", "Area"],
            "cast_time": 0.75,
            "base_effectiveness": 1.6,
            "incremental_effectiveness": 0.034,
            "base_flags": ["spell", "projectile", "area"],
            "mana_multiplier_at_20": 0.0,
            "require_skill_types": [],
            "add_skill_types": [],
            "exclude_skill_types": [],
            "constant_stats": [
                {"stat_id": "base_is_projectile", "value": 1.0}
            ],
            "quality_stats": [
                {"stat_id": "base_projectile_speed_+%", "value": 1.0}
            ],
            "stats": ["spell_minimum_base_fire_damage", "spell_maximum_base_fire_damage"],
            "levels": [
                {
                    "level": 20,
                    "level_requirement": 70,
                    "stat_values": [165.0, 248.0],
                    "crit_chance": 6.0,
                    "damage_effectiveness": 2.4,
                    "attack_speed_mult": 0.0,
                    "mana_cost": 26.0,
                    "life_cost": 0.0,
                    "mana_multiplier": 0.0,
                    "stored_uses": 0,
                    "cooldown": 0.0,
                    "duration": 0.0
                }
            ]
        }"#;
        let gem: GemData = serde_json::from_str(json).unwrap();
        assert_eq!(gem.id, "Fireball");
        assert_eq!(gem.color, Some("Int".to_string()));
        assert_eq!(gem.skill_types, vec!["Spell", "Projectile", "Area"]);
        assert!((gem.cast_time - 0.75).abs() < f64::EPSILON);
        assert!((gem.base_effectiveness - 1.6).abs() < f64::EPSILON);
        assert!((gem.incremental_effectiveness - 0.034).abs() < f64::EPSILON);
        assert_eq!(gem.base_flags, vec!["spell", "projectile", "area"]);
        assert_eq!(gem.constant_stats.len(), 1);
        assert_eq!(gem.constant_stats[0].stat_id, "base_is_projectile");
        assert_eq!(gem.quality_stats.len(), 1);
        assert_eq!(gem.stats.len(), 2);

        let lvl = &gem.levels[0];
        assert_eq!(lvl.level, 20);
        assert_eq!(lvl.level_requirement, 70);
        assert_eq!(lvl.stat_values, vec![165.0, 248.0]);
        assert!((lvl.crit_chance - 6.0).abs() < f64::EPSILON);
        assert!((lvl.damage_effectiveness - 2.4).abs() < f64::EPSILON);
        assert!((lvl.mana_cost - 26.0).abs() < f64::EPSILON);
    }

    #[test]
    fn old_format_still_works() {
        let json = r#"{
            "id": "Fireball",
            "display_name": "Fireball",
            "is_support": false,
            "skill_types": [],
            "levels": [
                {
                    "level": 1,
                    "fire_min": 10.0,
                    "fire_max": 15.0,
                    "crit_chance": 6.0,
                    "cast_time": 0.75,
                    "attack_speed_mult": 0.0
                }
            ]
        }"#;
        let gem: GemData = serde_json::from_str(json).unwrap();
        assert_eq!(gem.id, "Fireball");
        assert!(!gem.is_support);
        assert!(gem.color.is_none());
        assert_eq!(gem.base_flags.len(), 0);
        assert_eq!(gem.constant_stats.len(), 0);

        let lvl = &gem.levels[0];
        assert!((lvl.fire_min - 10.0).abs() < f64::EPSILON);
        assert!((lvl.fire_max - 15.0).abs() < f64::EPSILON);
        assert_eq!(lvl.level_requirement, 0);
        assert!(lvl.stat_values.is_empty());
    }

    #[test]
    fn skill_type_flags_from_vec() {
        let names = vec![
            "Attack".to_string(),
            "Melee".to_string(),
            "Duration".to_string(),
        ];
        let flags = SkillTypeFlags::from_vec(&names);
        assert!(flags.contains(SkillTypeFlags::ATTACK));
        assert!(flags.contains(SkillTypeFlags::MELEE));
        assert!(flags.contains(SkillTypeFlags::DURATION));
        assert!(!flags.contains(SkillTypeFlags::SPELL));
    }

    #[test]
    fn skill_type_flags_from_vec_unknown_ignored() {
        let names = vec!["UnknownType".to_string(), "Spell".to_string()];
        let flags = SkillTypeFlags::from_vec(&names);
        assert!(flags.contains(SkillTypeFlags::SPELL));
        assert!(!flags.contains(SkillTypeFlags::ATTACK));
    }
}
