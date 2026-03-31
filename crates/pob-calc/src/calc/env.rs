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

/// A buff/debuff entry with its name and the list of mods it provides.
#[derive(Debug, Clone)]
pub struct BuffEntry {
    pub name: String,
    pub skill_name: Option<String>,
    pub mods: Vec<crate::mod_db::types::Mod>,
    pub active: bool,
}

/// A curse entry with priority and slot tracking.
#[derive(Debug, Clone)]
pub struct CurseEntry {
    pub name: String,
    pub skill_name: Option<String>,
    pub mods: Vec<crate::mod_db::types::Mod>,
    pub priority: f64,
    pub is_mark: bool,
    pub active: bool,
}

/// An actor (player or minion) in a calculation environment.
pub struct Actor {
    pub mod_db: ModDb,
    pub output: OutputTable,
    pub breakdown: BreakdownTable,
    pub minion: Option<Box<Actor>>,
    pub main_skill: Option<crate::build::types::ActiveSkill>,
    pub weapon_data1: Option<crate::build::types::ItemWeaponData>,
    pub weapon_data2: Option<crate::build::types::ItemWeaponData>,
    pub has_shield: bool,
    pub dual_wield: bool,
    pub active_skill_list: Vec<crate::build::types::ActiveSkill>,

    // Reservation tracking
    pub reserved_life: f64,
    pub reserved_life_percent: f64,
    pub reserved_mana: f64,
    pub reserved_mana_percent: f64,

    // Buff/debuff/curse lists
    pub buffs: Vec<BuffEntry>,
    pub guards: Vec<BuffEntry>,
    pub debuffs: Vec<BuffEntry>,
    pub curses: Vec<CurseEntry>,

    // Action speed
    pub action_speed_mod: f64,

    /// Damage shift table: [source_type][dest_type] = percentage.
    /// Indices: 0=Physical, 1=Lightning, 2=Cold, 3=Fire, 4=Chaos.
    /// Initialised as identity (100% stays as original type).
    pub damage_shift_table: [[f64; 5]; 5],
}

impl Actor {
    pub fn new(mod_db: ModDb) -> Self {
        Self {
            mod_db,
            output: HashMap::new(),
            breakdown: HashMap::new(),
            minion: None,
            main_skill: None,
            weapon_data1: None,
            weapon_data2: None,
            has_shield: false,
            dual_wield: false,
            active_skill_list: Vec::new(),
            reserved_life: 0.0,
            reserved_life_percent: 0.0,
            reserved_mana: 0.0,
            reserved_mana_percent: 0.0,
            buffs: Vec::new(),
            guards: Vec::new(),
            debuffs: Vec::new(),
            curses: Vec::new(),
            action_speed_mod: 1.0,
            damage_shift_table: [
                [100.0, 0.0, 0.0, 0.0, 0.0],
                [0.0, 100.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 100.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 100.0, 0.0],
                [0.0, 0.0, 0.0, 0.0, 100.0],
            ],
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

/// Get a numeric output value, returning 0.0 if absent or not a number.
pub fn get_output_f64(output: &OutputTable, key: &str) -> f64 {
    output
        .get(key)
        .and_then(|v| {
            if let OutputValue::Number(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .unwrap_or(0.0)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_new_reservation_defaults_are_zero() {
        let actor = Actor::new(ModDb::new());
        assert_eq!(actor.reserved_life, 0.0);
        assert_eq!(actor.reserved_life_percent, 0.0);
        assert_eq!(actor.reserved_mana, 0.0);
        assert_eq!(actor.reserved_mana_percent, 0.0);
    }

    #[test]
    fn actor_new_buff_lists_are_empty() {
        let actor = Actor::new(ModDb::new());
        assert!(actor.buffs.is_empty());
        assert!(actor.guards.is_empty());
        assert!(actor.debuffs.is_empty());
        assert!(actor.curses.is_empty());
    }

    #[test]
    fn actor_new_action_speed_mod_defaults_to_one() {
        let actor = Actor::new(ModDb::new());
        assert_eq!(actor.action_speed_mod, 1.0);
    }

    #[test]
    fn buff_entry_can_be_constructed() {
        let entry = BuffEntry {
            name: "Anger".into(),
            skill_name: Some("Anger".into()),
            mods: Vec::new(),
            active: true,
        };
        assert_eq!(entry.name, "Anger");
        assert!(entry.active);
        assert!(entry.mods.is_empty());
    }

    #[test]
    fn curse_entry_can_be_constructed() {
        let entry = CurseEntry {
            name: "Vulnerability".into(),
            skill_name: Some("Vulnerability".into()),
            mods: Vec::new(),
            priority: 1.0,
            is_mark: false,
            active: true,
        };
        assert_eq!(entry.name, "Vulnerability");
        assert_eq!(entry.priority, 1.0);
        assert!(!entry.is_mark);
        assert!(entry.active);
    }
}
