use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct MiscData {
    pub game_constants: HashMap<String, f64>,
    pub character_constants: HashMap<String, f64>,
    pub monster_life_table: Vec<i32>,
    pub monster_damage_table: Vec<i32>,
    pub monster_evasion_table: Vec<i32>,
    pub monster_accuracy_table: Vec<i32>,
    pub monster_ally_life_table: Vec<i32>,
    pub monster_ally_damage_table: Vec<i32>,
    pub monster_ailment_threshold_table: Vec<i32>,
    pub monster_phys_conversion_multi_table: Vec<f32>,
}
