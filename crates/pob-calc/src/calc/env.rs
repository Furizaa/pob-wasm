use crate::build::types::ItemArmourData;
use crate::data::GameData;
use crate::mod_db::types::Mod;
use crate::mod_db::ModDb;
use std::collections::{HashMap, HashSet};
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
    /// Uncancellable life reservation percentage (from HasUncancellableReservation skills).
    /// Mirrors env.player["uncancellable_LifeReservation"] in Lua.
    pub uncancellable_life_reservation: f64,
    /// Uncancellable mana reservation percentage (from HasUncancellableReservation skills).
    /// Mirrors env.player["uncancellable_ManaReservation"] in Lua.
    pub uncancellable_mana_reservation: f64,

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

    /// Reservation efficiency values, stored for Arcane Cloak calculations.
    /// Mirrors env.player["ManaEfficiency"] / env.player["LifeEfficiency"] in Lua.
    pub mana_efficiency: f64,
    pub life_efficiency: f64,

    /// Per-slot armour item base data, populated by setup.rs.
    ///
    /// Mirrors `actor.itemList[slot].armourData` from CalcDefence.lua:843-923.
    /// The slot name is one of: "Helmet", "Gloves", "Boots", "Body Armour",
    /// "Weapon 2", "Weapon 3".  For players "Weapon 3" is always empty.
    ///
    /// These base values are **not** added to mod_db as global BASE mods —
    /// instead defence.rs iterates this list and applies per-slot INC/MORE.
    pub gear_slot_armour: Vec<(String, ItemArmourData)>,
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
            uncancellable_life_reservation: 0.0,
            uncancellable_mana_reservation: 0.0,
            mana_efficiency: 0.0,
            life_efficiency: 0.0,
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
            gear_slot_armour: Vec::new(),
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

/// A single entry in the requirements table.
/// Mirrors one entry in PoB's `env.requirementsTable`.
#[derive(Debug, Clone)]
pub struct RequirementEntry {
    /// Strength requirement (0 if none)
    pub str_req: f64,
    /// Dexterity requirement (0 if none)
    pub dex_req: f64,
    /// Intelligence requirement (0 if none)
    pub int_req: f64,
    /// Human-readable source name for display
    pub source_name: String,
}

/// Classification of a radius jewel's per-node callback type.
/// Mirrors PoB's funcList `type` field on jewel data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadiusJewelType {
    /// Fires for any node in radius, allocated or not.
    Threshold,
    /// Fires only when the node is **allocated** (env.allocNodes[node.id] is set).
    SelfAlloc,
    /// Fires only when the node is **not allocated**.
    SelfUnalloc,
    /// Runs in the first pass, before effect scaling.
    /// Used by timeless jewels (Glorious Vanity in Maraketh mode, etc.).
    Other,
}

/// A single entry in the radius jewel list.
/// Mirrors one element of `env.radiusJewelList` in CalcSetup.lua.
///
/// Each entry represents one jewel socket with a radius effect, and holds:
/// - The set of passive node IDs within that jewel's radius.
/// - A callback function that runs per-node (and once with node=None to finalise).
/// - Mutable per-pass accumulator data (e.g. total Str/Dex/Int tallied in radius).
pub struct RadiusJewelEntry {
    /// Set of passive node IDs within this jewel's radius.
    /// Mirrors `rad.nodes` — keyed by node_id.
    pub nodes: HashSet<u32>,
    /// The per-node callback. Signature:
    ///   `(node_id: Option<u32>, node_mods: &mut Vec<Mod>, data: &mut HashMap<String, f64>, mod_acc: &mut Vec<Mod>)`
    ///
    /// - `node_id`: `Some(nid)` for per-node calls, `None` for the finalize call.
    /// - `node_mods`: The current node's mod list (per-node) OR the main accumulator (finalize).
    /// - `data`: Numeric per-pass accumulator (e.g. attribute tallies).
    /// - `mod_acc`: Mod accumulator for callbacks that collect mods across nodes
    ///   (e.g. "Grants all bonuses of Unallocated Small Passive Skills in Radius").
    ///   During finalize, drain this into `node_mods` (the main accumulator).
    ///
    /// When `node_id` is None (finalize), this is the "finalize" call (after all nodes processed).
    pub func: Box<
        dyn Fn(Option<u32>, &mut Vec<Mod>, &mut HashMap<String, f64>, &mut Vec<Mod>) + Send + Sync,
    >,
    /// Whether this is a Threshold, Self (allocated), SelfUnalloc, or Other jewel.
    pub jewel_type: RadiusJewelType,
    /// The passive tree socket node ID where this jewel is socketed.
    /// Used to set `rad.data.modSource = "Tree:{node_id}"`.
    pub node_id: u32,
    /// Mutable per-pass accumulator for numeric tallies
    /// (e.g. `{"Str": 45, "Dex": 30, "Int": 20}`).
    pub data: HashMap<String, f64>,
    /// Mutable per-pass mod accumulator for callbacks that collect mods across nodes
    /// (e.g. "Grants all bonuses of Unallocated Small Passive Skills in Radius").
    /// Populated during per-node calls (node_id = Some); drained to main modList
    /// during the finalise call (node_id = None).
    ///
    /// Mirrors `rad.data.modList` in the Lua jewelSelfUnallocFuncs callbacks.
    pub mod_accumulator: Vec<Mod>,
}

/// The full calculation environment for one pass.
/// Mirrors POB's `env` table from CalcSetup.lua.
pub struct CalcEnv {
    pub player: Actor,
    pub enemy: Actor,
    pub mode: CalcMode,
    pub data: Arc<GameData>,

    // ── SETUP-16: Special unique item mod lists ───────────────────────────────
    // Mirrors CalcSetup.lua lines 961–1049.
    // These are populated during add_item_mods() when the corresponding special
    // unique item is equipped. Downstream calculations (animated weapons, minions)
    // read from these lists instead of / in addition to the player mod_db.
    /// Necromantic Aegis: shield mods that redirect to minions.
    /// Populated when a shield is equipped AND the Necromantic Aegis keystone
    /// (node 45175) is allocated. Non-SocketedIn mods go here instead of player.
    /// Mirrors `env.aegisModList` (CalcSetup.lua line 963).
    pub aegis_mod_list: Option<ModDb>,

    /// The Iron Mass: animated weapon mods.
    /// Populated when "The Iron Mass, Gladius" is in Weapon 1.
    /// Non-SocketedIn mods go here AND to the player mod_db.
    /// Mirrors `env.theIronMass` (CalcSetup.lua line 1016).
    pub the_iron_mass: Option<ModDb>,

    /// Dancing Dervish: animated weapon mods.
    /// Populated when a weapon with UniqueAnimateWeapon granted skill is in Weapon 1.
    /// Non-SocketedIn mods go here; SocketedIn mods go to player.
    /// (Non-SocketedIn do NOT go to player — unlike The Iron Mass.)
    /// Mirrors `env.weaponModList1` (CalcSetup.lua line 1034).
    pub weapon_mod_list1: Option<ModDb>,

    // ── Buff-mode flags (CalcSetup.lua lines 444–467) ──────────────────────
    // Set by the buff-mode dispatch based on the requested BuffMode.
    // For oracle builds (always "EFFECTIVE"), all three are true.
    //
    // mode_buffs:     buffs/auras are active (gates warcry exert, aura application)
    // mode_combat:    in combat (gates flask/tincture, on-hit/on-kill events)
    // mode_effective: enemy is present (gates enemy-dependent effects, curses, DPS)
    //
    // These are also mirrored into mod_db.conditions["Buffed"/"Combat"/"Effective"]
    // so that Condition-tag mods with var = "Buffed"/"Combat"/"Effective" evaluate
    // correctly (CalcSetup.lua lines 108–110 inside calcs.initModDB).
    pub mode_buffs: bool,
    pub mode_combat: bool,
    pub mode_effective: bool,
    /// Item and gem attribute requirements. Populated during setup.
    pub requirements_table: Vec<RequirementEntry>,
    /// Mirrors env.allocNodes: set of node IDs that are "allocated" for this env
    /// (includes both passive spec nodes and nodes granted by anointments/Forbidden jewels).
    pub alloc_nodes: HashSet<u32>,
    /// Mirrors env.grantedPassives: set of node IDs that were granted via anointments
    /// or Forbidden Flesh/Flame (i.e. NOT part of the original passive spec allocation).
    pub granted_passives: HashSet<u32>,
    /// Mirrors env.radiusJewelList: list of radius jewel descriptors.
    /// Populated during jewel slot processing in init_env.
    /// Each entry has a callback that is called for each passive node in its radius.
    pub radius_jewel_list: Vec<RadiusJewelEntry>,
    /// Mirrors env.extraRadiusNodeList: unallocated nodes that are in the radius of
    /// non-Self jewels (Threshold, SelfUnalloc). These nodes' mods are processed by
    /// buildModListForNodeList when finishJewels=true, but do NOT contribute to the
    /// player's modDB directly.
    pub extra_radius_node_list: HashSet<u32>,
    /// Mirrors env.keystonesAdded: dedup set of keystone names already merged this pass.
    /// Reset at the start of each perform::run() call (CalcPerform.lua line 1096).
    /// Guards against double-applying a keystone when merge_keystones() is called multiple
    /// times within the same perform pass (e.g. after flask application, after aura/buff
    /// application).
    pub keystones_added: HashSet<String>,
    /// Mirrors `env.configInput`: the build's Configuration tab inputs.
    /// Numeric values: `config_numbers["enemyPhysicalDamage"]` etc.
    /// String values: `config_strings["enemyIsBoss"]` etc.
    /// Boolean values: `config_booleans["conditionLowLife"]` etc.
    pub config_numbers: HashMap<String, f64>,
    pub config_strings: HashMap<String, String>,
    pub config_booleans: HashMap<String, bool>,
    /// Enemy level (Lua: env.enemyLevel), from config or default 84.
    pub enemy_level: usize,
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
            requirements_table: Vec::new(),
            alloc_nodes: HashSet::new(),
            granted_passives: HashSet::new(),
            radius_jewel_list: Vec::new(),
            extra_radius_node_list: HashSet::new(),
            keystones_added: HashSet::new(),
            // SETUP-16: Special unique mod lists — None until populated by add_item_mods().
            aegis_mod_list: None,
            the_iron_mass: None,
            weapon_mod_list1: None,
            // Buff-mode flags: default to EFFECTIVE (all three true).
            // This matches the oracle build path where mode = "MAIN" → buffMode = "EFFECTIVE".
            // CalcSetup.lua lines 444–467: for mode != "CALCS", buffMode = "EFFECTIVE",
            // which sets all three flags to true.
            mode_buffs: true,
            mode_combat: true,
            mode_effective: true,
            config_numbers: HashMap::new(),
            config_strings: HashMap::new(),
            config_booleans: HashMap::new(),
            enemy_level: 84,
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
