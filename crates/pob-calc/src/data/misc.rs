use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct MiscData {
    pub game_constants: HashMap<String, f64>,
    pub character_constants: HashMap<String, f64>,
    pub monster_life_table: Vec<i32>,
    pub monster_damage_table: Vec<f64>,
    pub monster_evasion_table: Vec<i32>,
    pub monster_accuracy_table: Vec<i32>,
    pub monster_ally_life_table: Vec<i32>,
    pub monster_ally_damage_table: Vec<f64>,
    pub monster_ailment_threshold_table: Vec<i32>,
    pub monster_phys_conversion_multi_table: Vec<i32>,
    #[serde(default)]
    pub pob_misc: PobMisc,
}

/// PoB-specific constants from Data.lua `data.misc = { ... }`.
/// These are hardcoded values in PoB, not from the game data files.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PobMisc {
    #[serde(default = "default_server_tick_time")]
    pub server_tick_time: f64,
    #[serde(default = "default_server_tick_rate")]
    pub server_tick_rate: f64,
    #[serde(default = "default_stun_base_duration")]
    pub stun_base_duration: f64,
    #[serde(default = "default_stun_base_mult")]
    pub stun_base_mult: f64,
    #[serde(default = "default_stun_not_melee_damage_mult")]
    pub stun_not_melee_damage_mult: f64,
    #[serde(default = "default_min_stun_chance_needed")]
    pub min_stun_chance_needed: f64,
    #[serde(default = "default_impale_stored_damage_base")]
    pub impale_stored_damage_base: f64,
    #[serde(default = "default_std_boss_dps_mult")]
    pub std_boss_dps_mult: f64,
    #[serde(default = "default_pinnacle_boss_dps_mult")]
    pub pinnacle_boss_dps_mult: f64,
    #[serde(default = "default_pinnacle_boss_pen")]
    pub pinnacle_boss_pen: f64,
    #[serde(default = "default_uber_boss_dps_mult")]
    pub uber_boss_dps_mult: f64,
    #[serde(default = "default_uber_boss_pen")]
    pub uber_boss_pen: f64,
    #[serde(default = "default_ehp_calc_speed_up")]
    pub ehp_calc_speed_up: i32,
    #[serde(default = "default_ehp_calc_max_damage")]
    pub ehp_calc_max_damage: f64,
    #[serde(default = "default_ehp_calc_max_iterations")]
    pub ehp_calc_max_iterations_to_calc: i32,
    #[serde(default = "default_max_hit_smoothing_passes")]
    pub max_hit_smoothing_passes: i32,
    #[serde(default = "default_avoid_chance_cap")]
    pub avoid_chance_cap: f64,
}

impl Default for PobMisc {
    fn default() -> Self {
        Self {
            server_tick_time: default_server_tick_time(),
            server_tick_rate: default_server_tick_rate(),
            stun_base_duration: default_stun_base_duration(),
            stun_base_mult: default_stun_base_mult(),
            stun_not_melee_damage_mult: default_stun_not_melee_damage_mult(),
            min_stun_chance_needed: default_min_stun_chance_needed(),
            impale_stored_damage_base: default_impale_stored_damage_base(),
            std_boss_dps_mult: default_std_boss_dps_mult(),
            pinnacle_boss_dps_mult: default_pinnacle_boss_dps_mult(),
            pinnacle_boss_pen: default_pinnacle_boss_pen(),
            uber_boss_dps_mult: default_uber_boss_dps_mult(),
            uber_boss_pen: default_uber_boss_pen(),
            ehp_calc_speed_up: default_ehp_calc_speed_up(),
            ehp_calc_max_damage: default_ehp_calc_max_damage(),
            ehp_calc_max_iterations_to_calc: default_ehp_calc_max_iterations(),
            max_hit_smoothing_passes: default_max_hit_smoothing_passes(),
            avoid_chance_cap: default_avoid_chance_cap(),
        }
    }
}

fn default_server_tick_time() -> f64 { 0.033 }
fn default_server_tick_rate() -> f64 { 1.0 / 0.033 }
fn default_stun_base_duration() -> f64 { 0.35 }
fn default_stun_base_mult() -> f64 { 200.0 }
fn default_stun_not_melee_damage_mult() -> f64 { 0.75 }
fn default_min_stun_chance_needed() -> f64 { 20.0 }
fn default_impale_stored_damage_base() -> f64 { 0.1 }
fn default_std_boss_dps_mult() -> f64 { 4.0 / 4.40 }
fn default_pinnacle_boss_dps_mult() -> f64 { 8.0 / 4.40 }
fn default_pinnacle_boss_pen() -> f64 { 15.0 / 5.0 }
fn default_uber_boss_dps_mult() -> f64 { 10.0 / 4.25 }
fn default_uber_boss_pen() -> f64 { 40.0 / 5.0 }
fn default_ehp_calc_speed_up() -> i32 { 8 }
fn default_ehp_calc_max_damage() -> f64 { 100_000_000.0 }
fn default_ehp_calc_max_iterations() -> i32 { 50 }
fn default_max_hit_smoothing_passes() -> i32 { 8 }
fn default_avoid_chance_cap() -> f64 { 75.0 }
