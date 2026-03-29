use crate::data::GameData;
use crate::mod_db::ModDb;
use std::collections::HashMap;
use std::sync::Arc;

/// Final computed stat values. Keys match POB's env.player.output table names.
pub type OutputTable = HashMap<String, OutputValue>;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum OutputValue {
    Number(f64),
    Bool(bool),
    Str(String),
}

/// Per-stat formula breakdown data.
/// Keys match OutputTable keys — only stats with non-trivial breakdowns have entries.
pub type BreakdownTable = HashMap<String, BreakdownData>;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct BreakdownData {
    /// Ordered formula step strings, e.g. ["4000 (base)", "x 1.85 (increased/reduced)", "= 7400"]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<String>,

    /// Per-item-slot contribution rows (Armour, Evasion, ES)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<SlotRow>,

    /// Per-damage-type conversion rows
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub damage_types: Vec<DamageTypeRow>,

    /// Mana/life reservation rows
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reservations: Vec<ReservationRow>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SlotRow {
    pub base: f64,
    pub inc: Option<String>,
    pub more: Option<String>,
    pub total: String,
    pub source: String,
    pub source_name: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DamageTypeRow {
    pub source: String,
    pub base: String,
    pub inc: String,
    pub more: String,
    pub total: String,
    pub conv_dst: String,
    pub gain_dst: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReservationRow {
    pub skill_name: String,
    pub base: String,
    pub mult: String,
    pub more: String,
    pub inc: String,
    pub efficiency: String,
    pub efficiency_more: String,
    pub total: String,
}

/// An actor (player or minion) in a calculation environment.
pub struct Actor {
    pub mod_db: ModDb,
    pub output: OutputTable,
    pub breakdown: BreakdownTable,
    pub minion: Option<Box<Actor>>,
    pub main_skill: Option<crate::build::types::ActiveSkill>,
}

impl Actor {
    pub fn new(mod_db: ModDb) -> Self {
        Self {
            mod_db,
            output: HashMap::new(),
            breakdown: HashMap::new(),
            minion: None,
            main_skill: None,
        }
    }

    /// Write a numeric output value.
    pub fn set_output(&mut self, key: &str, value: f64) {
        self.output
            .insert(key.to_string(), OutputValue::Number(value));
    }

    /// Write a boolean output value.
    pub fn set_output_bool(&mut self, key: &str, value: bool) {
        self.output
            .insert(key.to_string(), OutputValue::Bool(value));
    }

    /// Set breakdown lines for a stat.
    pub fn set_breakdown_lines(&mut self, key: &str, lines: Vec<String>) {
        self.breakdown.entry(key.to_string()).or_default().lines = lines;
    }

    /// Add a slot row to a stat's breakdown.
    pub fn push_breakdown_slot(&mut self, key: &str, row: SlotRow) {
        self.breakdown
            .entry(key.to_string())
            .or_default()
            .slots
            .push(row);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalcMode {
    /// Normal build calculation
    Normal,
    /// Calculator mode (for gear/node comparisons)
    Calculator,
}

/// The full calculation environment for one pass.
/// Mirrors POB's `env` table from CalcSetup.lua.
pub struct CalcEnv {
    pub player: Actor,
    pub enemy: Actor,
    pub mode: CalcMode,
    pub data: Arc<GameData>,
}

impl CalcEnv {
    pub fn new(player_mod_db: ModDb, enemy_mod_db: ModDb, data: Arc<GameData>) -> Self {
        Self {
            player: Actor::new(player_mod_db),
            enemy: Actor::new(enemy_mod_db),
            mode: CalcMode::Normal,
            data,
        }
    }
}
