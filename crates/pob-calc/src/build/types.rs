use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct Build {
    pub class_name: String,
    pub ascend_class_name: String,
    pub level: u8,
    pub bandit: String,
    pub target_version: String,
    pub passive_spec: PassiveSpec,
    pub skill_sets: Vec<SkillSet>,
    pub active_skill_set: usize,  // 0-based index
    pub main_socket_group: usize, // 0-based index
    pub item_sets: Vec<ItemSet>,
    pub active_item_set: usize,
    pub config: BuildConfig,
}

#[derive(Debug, Clone, Default)]
pub struct PassiveSpec {
    pub tree_version: String,
    pub allocated_nodes: HashSet<u32>,
    pub class_id: u32,
    pub ascend_class_id: u32,
}

#[derive(Debug, Clone)]
pub struct SkillSet {
    pub id: u32,
    pub skills: Vec<Skill>,
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub slot: String,
    pub enabled: bool,
    pub main_active_skill: usize, // 0-based index into gems
    pub gems: Vec<Gem>,
}

#[derive(Debug, Clone)]
pub struct Gem {
    pub skill_id: String,
    pub level: u8,
    pub quality: u8,
    pub enabled: bool,
    pub is_support: bool,
}

/// A fully resolved active skill, ready for offence calculations.
/// Mirrors POB's activeSkill table (env.player.mainSkill).
#[derive(Debug)]
pub struct ActiveSkill {
    /// The active gem's skill ID (e.g. "Fireball", "Cleave")
    pub skill_id: String,
    /// Gem level (1–20)
    pub level: u8,
    /// Combined modifier database for this skill's context
    pub skill_mod_db: crate::mod_db::ModDb,
    /// True if this is an attack (uses attack speed, weapon damage)
    pub is_attack: bool,
    /// True if this is a spell
    pub is_spell: bool,
    /// True if this skill uses melee mechanics
    pub is_melee: bool,
    /// True if the skill can crit
    pub can_crit: bool,
    /// Base critical strike chance (0.0–1.0). From gem data.
    pub base_crit_chance: f64,
    /// Base damage (min, max) per damage type, from gem level scaling.
    /// Key: "Physical" | "Lightning" | "Cold" | "Fire" | "Chaos"
    pub base_damage: std::collections::HashMap<String, (f64, f64)>,
    /// Attack speed (uses per second, for attacks)
    pub attack_speed_base: f64,
    /// Cast time in seconds (for spells)
    pub cast_time: f64,
}

#[derive(Debug, Clone, Default)]
pub struct ItemSet {
    pub id: u32,
    /// Map of slot name → item id
    pub slots: HashMap<String, u32>,
}

#[derive(Debug, Clone, Default)]
pub struct BuildConfig {
    pub numbers: HashMap<String, f64>,
    pub booleans: HashMap<String, bool>,
    pub strings: HashMap<String, String>,
}
