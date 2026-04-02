use super::env::CalcEnv;
use crate::{
    build::types::ItemSlot,
    build::Build,
    data::GameData,
    error::CalcError,
    mod_db::{
        types::{KeywordFlags, Mod, ModFlags, ModSource, ModTag, ModType, ModValue},
        ModDb,
    },
};
use std::collections::HashMap;
use std::sync::Arc;

// ─────────────────────────────────────────────────────────────────────────────
// Bandit & Pantheon mod injection  (CalcSetup.lua lines 531–553)
// ─────────────────────────────────────────────────────────────────────────────

/// Inject bandit reward mods into the player ModDb.
///
/// Mirrors CalcSetup.lua lines 531–540:
/// ```lua
/// if env.configInput.bandit == "Alira" then
///     modDB:NewMod("ElementalResist", "BASE", 15, "Bandit")
/// elseif env.configInput.bandit == "Kraityn" then
///     modDB:NewMod("MovementSpeed", "INC", 8, "Bandit")
/// elseif env.configInput.bandit == "Oak" then
///     modDB:NewMod("Life", "BASE", 40, "Bandit")
/// else
///     modDB:NewMod("ExtraPoints", "BASE", 1, "Bandit")
/// end
/// ```
///
/// The source category is "Bandit" in all cases; the name is the bandit's
/// name for the three rewarded choices, or "Kill All" for the default path.
fn add_bandit_mods(build: &Build, db: &mut ModDb) {
    match build.bandit.as_str() {
        "Alira" => {
            // +15 to all elemental resistances
            let src = ModSource::new("Bandit", "Alira");
            db.add(Mod::new_base("ElementalResist", 15.0, src));
        }
        "Kraityn" => {
            // 8% increased Movement Speed
            let src = ModSource::new("Bandit", "Kraityn");
            db.add(Mod {
                name: "MovementSpeed".to_string(),
                mod_type: ModType::Inc,
                value: ModValue::Number(8.0),
                flags: ModFlags::NONE,
                keyword_flags: KeywordFlags::NONE,
                tags: vec![],
                source: src,
            });
        }
        "Oak" => {
            // +40 to maximum Life
            let src = ModSource::new("Bandit", "Oak");
            db.add(Mod::new_base("Life", 40.0, src));
        }
        _ => {
            // Kill all bandits (bandit == "None" / nil / any other string).
            // Grants 1 ExtraPoints via the "Bandit" source.
            // PoB adds 1 here (not 2): the additional passive point grant from
            // killing bandits is modelled as 1 ExtraPoints mod.
            let src = ModSource::new("Bandit", "Kill All");
            db.add(Mod::new_base("ExtraPoints", 1.0, src));
        }
    }
}

/// Inject pantheon god mods into the player ModDb.
///
/// Mirrors CalcSetup.lua lines 542–553 and PantheonTools.lua lines 1–19
/// (`pantheon.applySoulMod`).
///
/// For each of the selected major and minor gods:
/// 1. Skip if the key is "None" or the god is not found in the data.
/// 2. Iterate all soul tiers for that god.
/// 3. For each soul mod line, parse it with `parse_mod`.
/// 4. Set the source on each parsed mod to "Pantheon:<primary_soul_name>"
///    where `primary_soul_name` is `god.souls[0].name` (the first soul tier).
/// 5. Add all parsed mods to the player ModDb.
///
/// Silent discard: lines that parse to zero mods are silently ignored,
/// matching `if modList and not extra then` guard in PantheonTools.lua.
fn add_pantheon_mods(build: &Build, db: &mut ModDb, data: &GameData) {
    for god_key in &[&build.pantheon_major_god, &build.pantheon_minor_god] {
        if god_key.as_str() == "None" {
            continue;
        }
        let Some(god) = data.pantheons.get(god_key.as_str()) else {
            continue;
        };
        // The primary soul name is always souls[0].name (Lua: god.souls[1].name).
        // ALL mods from this god use this name as the source, regardless of soul tier.
        let Some(primary_soul) = god.souls.first() else {
            continue;
        };
        let god_name = primary_soul.name.clone();
        let source = ModSource::new("Pantheon", &god_name);

        for soul in &god.souls {
            for soul_mod in &soul.mods {
                let parsed = crate::build::mod_parser::parse_mod(&soul_mod.line, source.clone());
                // Silent discard: if parse_mod returns empty Vec, skip this line.
                // Mirrors `if modList and not extra then` in PantheonTools.lua.
                for m in parsed {
                    db.add(m);
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cluster-jewel regex patterns (compiled once at startup)
// ─────────────────────────────────────────────────────────────────────────────
use once_cell::sync::Lazy;
use regex::Regex;

/// Matches "Adds N Passive Skills" (node count line).
static RE_CLUSTER_NODE_COUNT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^adds (\d+) passive skills?$").unwrap());

/// Matches "N Added Passive Skills are Jewel Sockets" (socket count line, multi).
static RE_CLUSTER_SOCKET_COUNT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^(\d+) added passive skills? are jewel sockets?$").unwrap());

/// Matches "Added Passive Skill is a Jewel Socket" or "1 Added Passive Skill is a Jewel Socket" (socket count = 1).
static RE_CLUSTER_SOCKET_ONE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^(?:1 )?added passive skill is a jewel socket$").unwrap());

/// Matches "1 Added Passive Skill is {Notable Name}" (notable assignment).
static RE_CLUSTER_NOTABLE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^1 added passive skill is (.+)$").unwrap());

/// Matches "Added Small Passive Skills grant: ..." (primary enchant = skill_id selection).
static RE_CLUSTER_SKILL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^added small passive skills grant: (.+)$").unwrap());

/// Matches "Added Small Passive Skills have N% increased Effect" (inc_effect).
static RE_CLUSTER_INC_EFFECT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^added small passive skills have (\d+)% increased effect$").unwrap()
});

/// Build the CalcEnv for a build.
/// Mirrors calcs.initEnv() in CalcSetup.lua.
pub fn init_env(build: &Build, data: Arc<GameData>) -> Result<CalcEnv, CalcError> {
    let mut player_db = ModDb::new();
    let enemy_db = ModDb::new();

    // Add base constants (mirrors calcs.initModDB())
    add_base_constants(&mut player_db, &data);

    // Add class base stats
    add_class_base_stats(build, &mut player_db, &data);

    // Add config conditions
    add_config_conditions(build, &mut player_db);

    // Add bandit reward mods (CalcSetup.lua lines 531-540)
    add_bandit_mods(build, &mut player_db);

    // Add pantheon god mods (CalcSetup.lua lines 542-553 + PantheonTools.lua)
    add_pantheon_mods(build, &mut player_db, &data);

    // Create the env first so add_item_mods can populate weapon data on the Actor
    let mut env = CalcEnv::new(player_db, enemy_db, data);

    // Add item mods from equipped items and extract weapon data
    // (CalcSetup.lua line 1228: mergeDB(env.modDB, env.itemModDB))
    add_item_mods(build, &mut env);

    // Add jewel mods from jewel slots
    add_jewel_mods(build, &mut env);

    // Add cluster jewel mods (synthetic passive nodes from cluster jewels)
    add_cluster_jewel_mods(build, &mut env);

    // Add flask mods (only if conditionUsingFlask is true)
    add_flask_mods(build, &mut env);

    // SETUP-07: Process anointments and Forbidden Flesh/Flame granted passives.
    // Must run AFTER add_item_mods (GrantedPassive mods come from items)
    // and BEFORE add_passive_mods (granted nodes must be in alloc_nodes when
    // passive mods are applied).
    // Mirrors CalcSetup.lua lines 1230-1258.
    apply_granted_passives(build, &mut env);

    // Build the radius jewel list from jewel slots.
    // Mirrors CalcSetup.lua lines 751-808: for each jewel slot with a radius,
    // add entries to env.radiusJewelList and populate env.extraRadiusNodeList.
    build_radius_jewel_list(build, &mut env);

    // Add passive tree node mods (including granted nodes from anointments).
    // Mirrors CalcSetup.lua lines 1260-1265: buildModListForNodeList(env, env.allocNodes, true).
    apply_passive_mods(build, &mut env);

    // Initialize enemy ModDb
    init_enemy_db(build, &mut env.enemy.mod_db, &env.data.clone());

    // Build attribute requirements table
    build_requirements_table(build, &mut env);

    // ── Mirror buff-mode flags into modDB.conditions ──────────────────────
    // Mirrors CalcSetup.lua lines 108–110 inside calcs.initModDB():
    //   modDB.conditions["Buffed"]    = env.mode_buffs
    //   modDB.conditions["Combat"]    = env.mode_combat
    //   modDB.conditions["Effective"] = env.mode_effective
    //
    // Must be called after all other setup (including add_passive_mods which
    // may add mods that consume these conditions), and after env.mode_* fields
    // are set. Since CalcEnv::new() defaults all three to true (EFFECTIVE mode),
    // this call unconditionally enables all three conditions for oracle builds.
    //
    // These conditions gate Condition-tag mods with var = "Buffed"/"Combat"/"Effective"
    // so that condition-gated mods from items, passives, etc. evaluate correctly.
    env.player.mod_db.set_condition("Buffed", env.mode_buffs);
    env.player.mod_db.set_condition("Combat", env.mode_combat);
    env.player
        .mod_db
        .set_condition("Effective", env.mode_effective);

    Ok(env)
}

/// Merge keystone modifiers into the player's mod_db.
///
/// Mirrors `modLib.mergeKeystones(env, modDB)` from ModTools.lua lines 225–237.
///
/// Algorithm:
/// 1. Collect all "Keystone" LIST mods from `env.player.mod_db`.
/// 2. For each mod whose value is a string keystone name:
///    - Skip if already in `env.keystones_added` (dedup guard).
///    - Look up the keystone node by name in the passive tree.
///    - If not found (no such keystone in this tree version), skip.
///    - Mark the keystone as added.
///    - Determine if the granting source is NOT from the tree:
///       `override_source = !source.category.to_lowercase().contains("tree")
///                          && !source.name.to_lowercase().contains("tree")`
///    - For each stat on the keystone node:
///        - Parse it into Mod objects.
///        - If `override_source`, replace each mod's source with the granting mod's source.
///        - Add to player mod_db.
///
/// Call sites:
/// - `perform::run()`: once at the start (after clearing `keystones_added`).
/// - `perform::run()`: again after aura/buff application (CalcPerform.lua:3257).
/// - `perform::run()`: again after flask application when mode_combat (CalcPerform.lua:1779).
pub fn merge_keystones(env: &mut super::env::CalcEnv) {
    use crate::mod_db::types::ModValue;

    let empty_output = super::env::OutputTable::new();
    let data = env.data.clone();
    let tree = &data.passive_tree;

    // Collect all "Keystone" LIST mods from player mod_db.
    // We clone to avoid holding an immutable borrow while mutating the mod_db below.
    let keystone_mods: Vec<crate::mod_db::types::Mod> = env
        .player
        .mod_db
        .list("Keystone", None, &empty_output)
        .into_iter()
        .cloned()
        .collect();

    for granting_mod in keystone_mods {
        // The keystone name is stored as ModValue::String.
        let keystone_name = match &granting_mod.value {
            ModValue::String(s) => s.clone(),
            _ => continue, // Number(0.0) stub — skip silently
        };

        // Dedup guard: skip if already processed this pass.
        // Mirrors: if not env.keystonesAdded[modObj.value]
        if env.keystones_added.contains(&keystone_name) {
            continue;
        }

        // Look up the keystone node in the tree.
        // Mirrors: env.spec.tree.keystoneMap[modObj.value]
        let keystone_node = match tree.keystone_by_name(&keystone_name) {
            Some(n) => n,
            None => continue, // Keystone not in this tree version — skip.
        };

        // Mark as processed for this pass.
        env.keystones_added.insert(keystone_name.clone());

        // Determine source override:
        // `fromTree` in Lua = modObj.mod.source exists AND does NOT contain "tree"
        // (case-insensitive). When fromTree is true (non-tree source), we override
        // each keystone mod's source with the granting item's source.
        //
        // In Rust:
        //   override_source = source.category does NOT contain "tree"
        //                   AND source.name does NOT contain "tree"
        let cat_lower = granting_mod.source.category.to_lowercase();
        let name_lower_src = granting_mod.source.name.to_lowercase();
        let override_source = !cat_lower.contains("tree") && !name_lower_src.contains("tree");

        // Parse each stat of the keystone node and add to player mod_db.
        // Mirrors: for _, mod in ipairs(env.spec.tree.keystoneMap[modObj.value].modList)
        // The source for the keystone mods: use "Passive:<keystone_name>" as the
        // default (same as add_passive_mods).
        let default_source =
            crate::mod_db::types::ModSource::new("Passive", keystone_name.as_str());

        // Collect the stats first (to avoid borrow issue with tree while we mutate mod_db).
        let stats: Vec<String> = keystone_node.stats.clone();

        for stat_text in &stats {
            let parsed_mods =
                crate::build::mod_parser::parse_mod(stat_text, default_source.clone());
            for mut keystone_mod in parsed_mods {
                if override_source {
                    // Replace source with granting item's source.
                    // Mirrors: modLib.setSource(mod, modObj.mod.source)
                    keystone_mod.source = granting_mod.source.clone();
                }
                env.player.mod_db.add(keystone_mod);
            }
        }
    }
}

/// Helper: look up a game constant or use a default.
fn gc_or(gc: &std::collections::HashMap<String, f64>, key: &str, default: f64) -> f64 {
    gc.get(key).copied().unwrap_or(default)
}

/// Mirrors calcs.initModDB() in CalcSetup.lua (lines 18-111).
/// Adds all ~50 base constants to the player ModDb.
fn add_base_constants(db: &mut ModDb, data: &GameData) {
    let gc = &data.misc.game_constants;
    let src = ModSource::new("Base", "game constants");

    // --- Resist caps ---
    let resist_max = gc_or(gc, "base_maximum_all_resistances_%", 75.0);
    for name in &[
        "FireResistMax",
        "ColdResistMax",
        "LightningResistMax",
        "ChaosResistMax",
    ] {
        db.add(Mod::new_base(*name, resist_max, src.clone()));
    }

    // --- Block caps ---
    let block_max = gc_or(gc, "maximum_block_%", 75.0);
    db.add(Mod::new_base("BlockChanceMax", block_max, src.clone()));

    let spell_block_max = gc_or(gc, "base_maximum_spell_block_%", 75.0);
    db.add(Mod::new_base(
        "SpellBlockChanceMax",
        spell_block_max,
        src.clone(),
    ));

    // --- Charge maxes ---
    let power_max = gc_or(gc, "max_power_charges", 3.0);
    db.add(Mod::new_base("PowerChargesMax", power_max, src.clone()));

    let frenzy_max = gc_or(gc, "max_frenzy_charges", 3.0);
    db.add(Mod::new_base("FrenzyChargesMax", frenzy_max, src.clone()));

    let endurance_max = gc_or(gc, "max_endurance_charges", 3.0);
    db.add(Mod::new_base(
        "EnduranceChargesMax",
        endurance_max,
        src.clone(),
    ));

    // --- Leech rates ---
    let max_life_leech_rate = gc_or(gc, "maximum_life_leech_rate_%_per_minute", 20.0);
    db.add(Mod::new_base(
        "MaxLifeLeechRate",
        max_life_leech_rate,
        src.clone(),
    ));

    let max_mana_leech_rate = gc_or(gc, "maximum_mana_leech_rate_%_per_minute", 20.0);
    db.add(Mod::new_base(
        "MaxManaLeechRate",
        max_mana_leech_rate,
        src.clone(),
    ));

    // --- Leech instance caps ---
    let max_life_leech_inst = gc_or(gc, "maximum_life_leech_amount_per_leech_%_max_life", 10.0);
    db.add(Mod::new_base(
        "MaxLifeLeechInstance",
        max_life_leech_inst,
        src.clone(),
    ));

    let max_mana_leech_inst = gc_or(gc, "maximum_mana_leech_amount_per_leech_%_max_mana", 10.0);
    db.add(Mod::new_base(
        "MaxManaLeechInstance",
        max_mana_leech_inst,
        src.clone(),
    ));

    let max_es_leech_inst = gc_or(
        gc,
        "maximum_energy_shield_leech_amount_per_leech_%_max_energy_shield",
        10.0,
    );
    db.add(Mod::new_base(
        "MaxEnergyShieldLeechInstance",
        max_es_leech_inst,
        src.clone(),
    ));

    // --- Active limits ---
    let active_totem = gc_or(gc, "base_number_of_totems_allowed", 1.0);
    db.add(Mod::new_base("ActiveTotemLimit", active_totem, src.clone()));
    db.add(Mod::new_base("ActiveMineLimit", 15.0, src.clone()));
    db.add(Mod::new_base("ActiveTrapLimit", 15.0, src.clone()));
    db.add(Mod::new_base("ActiveBrandLimit", 3.0, src.clone()));

    // --- Crit ---
    db.add(Mod::new_base("CritChanceCap", 100.0, src.clone()));
    db.add(Mod::new_base("CritMultiplier", 150.0, src.clone()));

    // --- Charge durations ---
    db.add(Mod::new_base("PowerChargesDuration", 10.0, src.clone()));
    db.add(Mod::new_base("FrenzyChargesDuration", 10.0, src.clone()));
    db.add(Mod::new_base("EnduranceChargesDuration", 10.0, src.clone()));

    // --- Trap/Mine/Totem/Warcry timing ---
    db.add(Mod::new_base("TrapThrowTime", 0.6, src.clone()));
    db.add(Mod::new_base("MineLayingTime", 0.3, src.clone()));
    db.add(Mod::new_base("TotemPlacementTime", 0.6, src.clone()));
    db.add(Mod::new_base("WarcryCastTime", 0.8, src.clone()));

    // --- Totem resistances ---
    db.add(Mod::new_base("TotemFireResist", 40.0, src.clone()));
    db.add(Mod::new_base("TotemColdResist", 40.0, src.clone()));
    db.add(Mod::new_base("TotemLightningResist", 40.0, src.clone()));
    db.add(Mod::new_base("TotemChaosResist", 20.0, src.clone()));

    // --- Ailment stacks ---
    db.add(Mod::new_base("MaxShockStacks", 1.0, src.clone()));
    db.add(Mod::new_base("MaxScorchStacks", 1.0, src.clone()));
    db.add(Mod::new_base("MaxBrittleStacks", 1.0, src.clone()));
    db.add(Mod::new_base("MaxSapStacks", 1.0, src.clone()));

    // --- Impale / Wither ---
    let impale_max = gc_or(gc, "impaled_debuff_number_of_reflected_hits", 5.0);
    db.add(Mod::new_base("ImpaleStacksMax", impale_max, src.clone()));
    db.add(Mod::new_base("WitherStacksMax", 15.0, src.clone()));

    // --- DoT durations ---
    db.add(Mod::new_base("BleedDurationBase", 4.0, src.clone()));
    db.add(Mod::new_base("IgniteDurationBase", 4.0, src.clone()));
    db.add(Mod::new_base("PoisonDurationBase", 2.0, src.clone()));

    // --- Soul Eater ---
    let soul_eater_max = gc_or(gc, "soul_eater_maximum_stacks", 40.0);
    db.add(Mod::new_base("SoulEaterMax", soul_eater_max, src.clone()));

    // --- Conditional mods ---
    // Maimed: -30% inc MovementSpeed
    db.add(Mod {
        name: "MovementSpeed".to_string(),
        mod_type: ModType::Inc,
        value: ModValue::Number(-30.0),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags: vec![ModTag::Condition {
            var: "Maimed".to_string(),
            neg: false,
        }],
        source: src.clone(),
    });

    // Intimidated: 10% inc DamageTaken
    db.add(Mod {
        name: "DamageTaken".to_string(),
        mod_type: ModType::Inc,
        value: ModValue::Number(10.0),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags: vec![ModTag::Condition {
            var: "Intimidated".to_string(),
            neg: false,
        }],
        source: src.clone(),
    });

    // Unnerved: 10% inc DamageTaken (spell only)
    db.add(Mod {
        name: "DamageTaken".to_string(),
        mod_type: ModType::Inc,
        value: ModValue::Number(10.0),
        flags: ModFlags::SPELL,
        keyword_flags: KeywordFlags::NONE,
        tags: vec![ModTag::Condition {
            var: "Unnerved".to_string(),
            neg: false,
        }],
        source: src.clone(),
    });
}

/// Per-class base attributes: (Str, Dex, Int).
/// Mirrors CalcSetup.lua initEnv() class stat lookup.
fn class_base_attributes(class_name: &str) -> (f64, f64, f64) {
    match class_name {
        "Marauder" => (32.0, 14.0, 14.0),
        "Ranger" => (14.0, 32.0, 14.0),
        "Witch" => (14.0, 14.0, 32.0),
        "Duelist" => (23.0, 23.0, 14.0),
        "Templar" => (23.0, 14.0, 23.0),
        "Shadow" => (14.0, 23.0, 23.0),
        "Scion" => (20.0, 20.0, 20.0),
        _ => (20.0, 20.0, 20.0), // fallback to Scion
    }
}

/// Mirrors calcs.initEnv() in CalcSetup.lua.
/// Adds class base stats, resistance penalty, accuracy, and evasion.
fn add_class_base_stats(build: &Build, db: &mut ModDb, _data: &GameData) {
    let level = build.level as f64;
    let src = ModSource::new("Base", format!("{} base stats", build.class_name));

    // Base life: 38 + 12 * level (simplified; full table loaded later)
    let base_life = 38.0 + 12.0 * level;
    db.add(Mod::new_base("Life", base_life, src.clone()));

    // Base mana: 34 + 6 * level
    let base_mana = 34.0 + 6.0 * level;
    db.add(Mod::new_base("Mana", base_mana, src.clone()));

    // Per-class Str/Dex/Int
    let (str_base, dex_base, int_base) = class_base_attributes(&build.class_name);
    db.add(Mod::new_base("Str", str_base, src.clone()));
    db.add(Mod::new_base("Dex", dex_base, src.clone()));
    db.add(Mod::new_base("Int", int_base, src.clone()));

    // Resistance penalty (act 10): -60 to elemental resists
    let penalty_src = ModSource::new("Base", "resistance penalty");
    db.add(Mod::new_base("FireResist", -60.0, penalty_src.clone()));
    db.add(Mod::new_base("ColdResist", -60.0, penalty_src.clone()));
    db.add(Mod::new_base("LightningResist", -60.0, penalty_src.clone()));

    // Base accuracy: 2 * level
    let acc_src = ModSource::new("Base", "base accuracy");
    db.add(Mod::new_base("Accuracy", 2.0 * level, acc_src));

    // Base evasion: 53 + 3 * level
    let eva_src = ModSource::new("Base", "base evasion");
    db.add(Mod::new_base("Evasion", 53.0 + 3.0 * level, eva_src));
}

/// Process anointments (GrantedPassive mods) and Forbidden Flesh/Flame
/// (GrantedAscendancyNode mods) to populate `env.alloc_nodes` and
/// `env.granted_passives`.
///
/// Mirrors CalcSetup.lua lines 1230-1258.
///
/// Must be called AFTER item mods have been added to `env.player.mod_db`
/// (so that `GrantedPassive` and `GrantedAscendancyNode` mods from amulets
/// and jewels are present) and BEFORE `add_passive_mods_with_granted`
/// (so that granted node IDs are available for mod application).
fn apply_granted_passives(build: &Build, env: &mut CalcEnv) {
    let data = env.data.clone();
    let tree = data.tree_for_version(&build.passive_spec.tree_version);

    // Empty output table (these mods have no tags requiring output lookups).
    let empty_output = crate::calc::env::OutputTable::new();

    // ── Part 1: Amulet anointments (GrantedPassive) ──────────────────────────
    // Mirrors CalcSetup.lua lines 1231-1239.
    //
    // `env.modDB:List(nil, "GrantedPassive")` returns a list of string values
    // where each value is the lowercase notable name (e.g. "corruption").
    // We look up the node in `tree.notable_map` (keyed by lowercase name).
    // If found, add the node ID to alloc_nodes and granted_passives.
    for m in env
        .player
        .mod_db
        .list("GrantedPassive", None, &empty_output)
    {
        let ModValue::String(passive_name) = &m.value else {
            continue;
        };
        // notableMap lookup: key is already lowercase (both the stored name and the map key).
        if let Some(&node_id) = tree.notable_map.get(passive_name.as_str()) {
            // Insert the node into the allocated set.
            // env.allocNodes[node.id] = env.spec.nodes[node.id] or node
            // In Rust, conquered-node handling is done by SETUP-06; we just track the ID.
            env.alloc_nodes.insert(node_id);
            // env.grantedPassives[node.id] = true
            env.granted_passives.insert(node_id);
            // env.extraRadiusNodeList[node.id] = nil — SETUP-08 concern, skip for now
        }
        // If not found, skip silently (stale build / passive renamed between versions).
    }

    // ── Part 2: Forbidden Flesh/Flame (GrantedAscendancyNode) ────────────────
    // Mirrors CalcSetup.lua lines 1242-1258.
    //
    // Both jewels must be equipped and name the SAME ascendancy notable.
    // The mod value is encoded as "<side>:<name>" (e.g. "flesh:heavy hitter").
    // We accumulate entries and only grant the node when BOTH sides are seen.
    //
    // matchedName = { name → (side, matched) }
    let mut matched: HashMap<String, (String, bool)> = HashMap::new();

    for m in env
        .player
        .mod_db
        .list("GrantedAscendancyNode", None, &empty_output)
    {
        let ModValue::String(encoded) = &m.value else {
            continue;
        };
        // encoded = "<side>:<name>", e.g. "flesh:heavy hitter"
        let Some((side, name)) = encoded.split_once(':') else {
            continue;
        };

        if let Some(entry) = matched.get_mut(name) {
            // Second jewel seen: check it's the opposite side and not yet matched.
            // matchedName[name].side ~= ascTbl.side and matchedName[name].matched == false
            if entry.0 != side && !entry.1 {
                entry.1 = true; // matchedName[name].matched = true

                // Look up in ascendancyMap, fall back to latest tree.
                // env.spec.tree.ascendancyMap[name] or build.latestTree.ascendancyMap[name]
                let node_id = tree.ascendancy_map.get(name).copied().or_else(|| {
                    // Fallback: try the default (latest) tree's ascendancyMap.
                    data.passive_tree.ascendancy_map.get(name).copied()
                });

                if let Some(node_id) = node_id {
                    // Extra guard: BOTH jewels must match the current character's class.
                    // env.itemModDB.conditions["ForbiddenFlesh"] == env.spec.curClassName
                    // and env.itemModDB.conditions["ForbiddenFlame"] == env.spec.curClassName
                    //
                    // In Rust, the ForbiddenFlesh/ForbiddenFlame conditions are stored as
                    // Condition mods in the player mod_db (set by item mod parser).
                    // For now: check if the node's ascendancy matches the build's ascendancy.
                    // This is the practical equivalent: Forbidden jewels only work if both
                    // jewels' ascendancy matches the current character's ascendancy class.
                    let node_ascendancy = tree
                        .nodes
                        .get(&node_id)
                        .and_then(|n| n.ascendancy_name.as_deref())
                        .unwrap_or("");

                    // The class check: node must belong to the build's current ascendancy.
                    // build.ascend_class_name contains the current ascendancy (e.g. "Deadeye").
                    // This matches env.spec.curClassName in PoB.
                    if node_ascendancy == build.ascend_class_name
                        || build.ascend_class_name == "None"
                        || build.ascend_class_name.is_empty()
                        || node_ascendancy.is_empty()
                    {
                        // env.allocNodes[node.id] = node
                        env.alloc_nodes.insert(node_id);
                        // env.grantedPassives[node.id] = true
                        env.granted_passives.insert(node_id);
                    }
                }
            }
        } else {
            // First time we see this name: record which side (Flesh or Flame) this jewel is.
            // matchedName[name] = { side = ascTbl.side, matched = false }
            matched.insert(name.to_string(), (side.to_string(), false));
        }
    }
}

/// Apply passive tree mods to the player ModDb, implementing the full two-pass
/// radius jewel framework from CalcSetup.lua.
///
/// Also handles mastery node stat substitution (SETUP-09) and writes the
/// Multiplier:AllocatedMastery* mods (CalcSetup.lua:664-671).
///
/// Mirrors calcs.buildModListForNodeList(env, env.allocNodes, true).
fn apply_passive_mods(build: &Build, env: &mut super::env::CalcEnv) {
    use crate::passive_tree::NodeType;

    let data = env.data.clone();
    let tree = data.tree_for_version(&build.passive_spec.tree_version);

    // Apply timeless jewel node replacements (SETUP-06).
    let timeless_overrides = crate::timeless_jewels::apply_timeless_jewels(build, tree, &data);

    // Find reachable (connected) nodes, respecting mastery gate.
    // Mirrors PassiveSpec.lua ImportFromNodeList:266-277.
    let reachable = connected_passive_nodes(build, tree);

    // Combine reachable nodes with explicitly granted nodes.
    let granted_nodes = env.granted_passives.clone();
    let all_alloc_nodes: std::collections::HashSet<u32> = reachable
        .iter()
        .copied()
        .chain(granted_nodes.iter().copied())
        .collect();

    // ── NOTABLE AND KEYSTONE COUNTING (SETUP-14) ─────────────────────────────
    // Mirrors PassiveSpec.lua:BuildAllDependsAndPaths lines 1312-1362 (the
    // elseif node.type == "Notable"/"Keystone" branches).
    // Counts are written as Multiplier:AllocatedNotable / Multiplier:AllocatedKeystone
    // in CalcSetup.lua lines 658-663.
    let mut allocated_notable_count: u32 = 0;
    let mut allocated_keystone_count: u32 = 0;

    // ── TATTOO TYPE COUNTING (SETUP-14) ──────────────────────────────────────
    // Mirrors PassiveSpec.lua:BuildAllDependsAndPaths lines 1348-1362:
    //   if node.isTattoo and node.alloc and node.overrideType then
    //       allocatedTattooTypes[node.overrideType] += 1
    // and CalcSetup.lua lines 607-311:
    //   if allocatedTattooTypes then
    //       for type, count in pairs(allocatedTattooTypes) do
    //           env.modDB.multipliers[type] = count
    //
    // A node is tattooed if it has an entry in hash_overrides with is_tattoo = true.
    // The counting uses `override_type` from the TattooOverrideNode.
    // Nodes with an empty `override_type` (no tattoo data loaded) are skipped.
    //
    // NOTE: This counts nodes in all_alloc_nodes (before mastery filtering).
    // The Lua does the same: the loop at line 1312 iterates self.nodes, checking
    // `node.alloc` which is set by the allocation process.
    let mut alloc_tattoo_types: HashMap<String, u32> = HashMap::new();

    for &nid in &all_alloc_nodes {
        match tree.nodes.get(&nid).map(|n| n.node_type) {
            Some(NodeType::Notable) => allocated_notable_count += 1,
            Some(NodeType::Keystone) => allocated_keystone_count += 1,
            _ => {}
        }

        // Count tattoo types for allocated nodes that have a hash override.
        // Mirrors: if node.isTattoo and node.alloc and node.overrideType then
        if let Some(override_node) = build.passive_spec.hash_overrides.get(&nid) {
            if override_node.is_tattoo && !override_node.override_type.is_empty() {
                *alloc_tattoo_types
                    .entry(override_node.override_type.clone())
                    .or_insert(0) += 1;
            }
        }
    }

    // ── MASTERY COUNTING (SETUP-09) ──────────────────────────────────────────
    // Mirrors PassiveSpec.lua:BuildAllDependsAndPaths lines 1307-1363.
    // For each allocated mastery node with a valid selection:
    //   - Count total allocated masteries
    //   - Count distinct mastery types (by node name)
    // These are written as Multiplier mods after all passive mods are applied.
    let mut allocated_mastery_count: u32 = 0;
    let mut allocated_mastery_type_count: u32 = 0;
    let mut allocated_mastery_types: HashMap<String, u32> = HashMap::new();

    // Build the effective mastery selections for this pass:
    // Only count mastery nodes that are both allocated AND have a recognized effect.
    // Unrecognized effects (effect not in tree.mastery_effects) dealloc the node (skip it).
    // Mirrors PassiveSpec.lua:1320-1347.
    let mut effective_mastery_selections: HashMap<u32, Vec<String>> = HashMap::new();
    for (&node_id, &effect_id) in &build.passive_spec.mastery_selections {
        if !all_alloc_nodes.contains(&node_id) {
            // Not allocated — skip (mirrors "else" branch at line 1343)
            continue;
        }
        // Look up the effect in the global mastery effects table
        let effect_stats = tree.mastery_effects.get(&effect_id);
        let Some(stats) = effect_stats else {
            // Unrecognized effect: skip (mirrors lines 1343-1347)
            continue;
        };
        // Node is allocated and has a recognized effect: apply it
        effective_mastery_selections.insert(node_id, stats.clone());

        // Count mastery for Multiplier mods.
        // Mirrors PassiveSpec.lua:1331-1341.
        allocated_mastery_count += 1;

        // Count distinct mastery types by node name (e.g. "Life Mastery").
        let node_name = tree
            .nodes
            .get(&node_id)
            .map(|n| n.name.as_str())
            .unwrap_or("");

        let type_count = allocated_mastery_types
            .entry(node_name.to_string())
            .or_insert(0);
        if *type_count == 0 {
            // First allocation of this mastery type (or re-added after being zeroed)
            allocated_mastery_type_count += 1;
        }
        *type_count += 1;
    }

    // Update env.alloc_nodes to include all allocated nodes (for radius jewel checks).
    // Remove mastery nodes that have no valid selection (mirrors line 1344-1346).
    let final_alloc_nodes: std::collections::HashSet<u32> = all_alloc_nodes
        .iter()
        .copied()
        .filter(|&nid| {
            // Keep non-mastery nodes always
            let node_type = tree.nodes.get(&nid).map(|n| n.node_type);
            if node_type != Some(NodeType::Mastery) {
                return true;
            }
            // Keep mastery nodes only if they have an effective selection
            effective_mastery_selections.contains_key(&nid)
        })
        .collect();

    env.alloc_nodes = final_alloc_nodes.clone();

    // Reset radius jewel accumulators and set modSource.
    // Mirrors CalcSetup.lua: for _, rad in pairs(env.radiusJewelList) do wipeTable(rad.data); ...
    for rad in &mut env.radius_jewel_list {
        rad.data.clear();
        rad.data.insert("modSource".to_string(), 0.0); // placeholder; modSource is string in Lua
    }

    // Collect mods from all allocated nodes using the two-pass radius jewel framework.
    let mut all_mods: Vec<crate::mod_db::types::Mod> = Vec::new();

    // Process all allocated nodes.
    for &node_id in &final_alloc_nodes {
        let node_mods = build_mod_list_for_node(
            node_id,
            true, // is_allocated
            tree,
            &timeless_overrides,
            &effective_mastery_selections,
            &mut env.radius_jewel_list,
            &env.alloc_nodes,
        );
        all_mods.extend(node_mods);
    }

    // Process extra radius nodes (unallocated nodes near non-Self jewels).
    // These nodes' mods do NOT go into the player ModDb — they are only processed
    // so radius jewel callbacks can read their stats.
    let extra_nodes = env.extra_radius_node_list.clone();
    for &node_id in &extra_nodes {
        // Only process if not already in alloc_nodes (to avoid double-processing).
        if final_alloc_nodes.contains(&node_id) {
            continue;
        }
        build_mod_list_for_node(
            node_id,
            false, // is_allocated
            tree,
            &timeless_overrides,
            &effective_mastery_selections,
            &mut env.radius_jewel_list,
            &env.alloc_nodes,
        );
        // Note: we discard the returned mods — extra radius nodes don't contribute to modDB.
    }

    // Finalise radius jewels: call func(None, &mut mods, &mut data) for each.
    // Mirrors: for _, rad in pairs(env.radiusJewelList) do rad.func(nil, modList, rad.data) end
    for rad in &mut env.radius_jewel_list {
        (rad.func)(None, &mut all_mods, &mut rad.data);
    }

    // Add all collected mods to the player ModDb.
    for m in all_mods {
        env.player.mod_db.add(m);
    }

    // ── WRITE MULTIPLIER MODS (SETUP-09 + SETUP-14) ─────────────────────────
    // Mirrors CalcSetup.lua:658-676.
    let passive_src = ModSource::new("Passive", "allocated node type counts");

    // Multiplier:AllocatedNotable (SETUP-14)
    // Mirrors CalcSetup.lua:658-660.
    if allocated_notable_count > 0 {
        env.player.mod_db.add(Mod {
            name: "Multiplier:AllocatedNotable".to_string(),
            mod_type: crate::mod_db::types::ModType::Base,
            value: crate::mod_db::types::ModValue::Number(allocated_notable_count as f64),
            flags: crate::mod_db::types::ModFlags::NONE,
            keyword_flags: crate::mod_db::types::KeywordFlags::NONE,
            tags: vec![],
            source: passive_src.clone(),
        });
    }

    // Multiplier:AllocatedKeystone (SETUP-14)
    // Mirrors CalcSetup.lua:661-663.
    if allocated_keystone_count > 0 {
        env.player.mod_db.add(Mod {
            name: "Multiplier:AllocatedKeystone".to_string(),
            mod_type: crate::mod_db::types::ModType::Base,
            value: crate::mod_db::types::ModValue::Number(allocated_keystone_count as f64),
            flags: crate::mod_db::types::ModFlags::NONE,
            keyword_flags: crate::mod_db::types::KeywordFlags::NONE,
            tags: vec![],
            source: passive_src.clone(),
        });
    }

    let mastery_src = ModSource::new("Mastery", "allocated mastery nodes");

    if allocated_mastery_count > 0 {
        env.player.mod_db.add(Mod {
            name: "Multiplier:AllocatedMastery".to_string(),
            mod_type: crate::mod_db::types::ModType::Base,
            value: crate::mod_db::types::ModValue::Number(allocated_mastery_count as f64),
            flags: crate::mod_db::types::ModFlags::NONE,
            keyword_flags: crate::mod_db::types::KeywordFlags::NONE,
            tags: vec![],
            source: mastery_src.clone(),
        });
    }

    if allocated_mastery_type_count > 0 {
        env.player.mod_db.add(Mod {
            name: "Multiplier:AllocatedMasteryType".to_string(),
            mod_type: crate::mod_db::types::ModType::Base,
            value: crate::mod_db::types::ModValue::Number(allocated_mastery_type_count as f64),
            flags: crate::mod_db::types::ModFlags::NONE,
            keyword_flags: crate::mod_db::types::KeywordFlags::NONE,
            tags: vec![],
            source: mastery_src.clone(),
        });
    }

    // "Life Mastery" gets a specific multiplier.
    // Mirrors CalcSetup.lua:670-672.
    if let Some(&life_mastery_count) = allocated_mastery_types.get("Life Mastery") {
        if life_mastery_count > 0 {
            env.player.mod_db.add(Mod {
                name: "Multiplier:AllocatedLifeMastery".to_string(),
                mod_type: crate::mod_db::types::ModType::Base,
                value: crate::mod_db::types::ModValue::Number(life_mastery_count as f64),
                flags: crate::mod_db::types::ModFlags::NONE,
                keyword_flags: crate::mod_db::types::KeywordFlags::NONE,
                tags: vec![],
                source: mastery_src,
            });
        }
    }

    // ── WRITE TATTOO TYPE MULTIPLIERS (SETUP-14) ─────────────────────────────
    // Mirrors CalcSetup.lua:307-311:
    //   if allocatedTattooTypes then
    //       for type, count in pairs(allocatedTattooTypes) do
    //           env.modDB.multipliers[type] = count
    //
    // IMPORTANT: These use `set_multiplier` (direct write to `modDB.multipliers`),
    // NOT `NewMod` / `db.add`. This mirrors the Lua which writes directly to
    // `env.modDB.multipliers[type]` bypassing the NewMod pipeline.
    // See "What's missing" section in SETUP-14 reference doc.
    for (tattoo_type, &count) in &alloc_tattoo_types {
        env.player.mod_db.set_multiplier(tattoo_type, count as f64);
    }
}

/// Build the mod list for a single passive node, implementing the two-pass
/// radius jewel dispatch, PassiveSkillEffect scaling, and suppression checks.
///
/// Mirrors calcs.buildModListForNode(env, node) from CalcSetup.lua lines 113-167.
/// Also handles mastery node stat substitution (SETUP-09):
///   if the node is a mastery with a selection, use the effect's stats instead of node.stats.
///
/// Returns the final Vec<Mod> for this node.
fn build_mod_list_for_node(
    node_id: u32,
    is_allocated: bool,
    tree: &crate::passive_tree::PassiveTree,
    timeless_overrides: &HashMap<u32, Vec<String>>,
    // mastery_stats: mastery node_id → effective stat strings (already looked up from mastery_effects).
    // Mastery nodes NOT in this map are skipped (they have no allocated effect).
    mastery_stats: &HashMap<u32, Vec<String>>,
    radius_jewel_list: &mut Vec<super::env::RadiusJewelEntry>,
    _alloc_nodes: &std::collections::HashSet<u32>,
) -> Vec<crate::mod_db::types::Mod> {
    use crate::calc::env::RadiusJewelType;
    use crate::mod_db::types::{ModType, ModValue};
    use crate::passive_tree::NodeType;

    let Some(node) = tree.nodes.get(&node_id) else {
        return Vec::new();
    };

    // Mastery node handling (SETUP-09):
    // Mirrors PassiveSpec.lua:BuildAllDependsAndPaths lines 1320-1347.
    // If the node has an effective selection in mastery_stats, use those stats.
    // If not in mastery_stats, skip (no selection or unrecognized effect).
    if node.node_type == NodeType::Mastery {
        let Some(effect_stats) = mastery_stats.get(&node_id) else {
            // No selection or unrecognized effect: skip this node entirely.
            return Vec::new();
        };
        // Apply mastery effect stats (node.sd = effect.sd in Lua).
        let source = ModSource::new("Passive", &node.name);
        let mut mod_list: Vec<crate::mod_db::types::Mod> = Vec::new();
        for stat_text in effect_stats {
            let parsed = crate::build::mod_parser::parse_mod(stat_text, source.clone());
            mod_list.extend(parsed);
        }
        return mod_list;
    }

    let source = ModSource::new("Passive", &node.name);

    // Collect base mods from the node's stats (or timeless overrides).
    let stats: &[String] = if let Some(override_stats) = timeless_overrides.get(&node_id) {
        override_stats
    } else {
        &node.stats
    };

    let mut mod_list: Vec<crate::mod_db::types::Mod> = Vec::new();
    for stat_text in stats {
        let parsed = crate::build::mod_parser::parse_mod(stat_text, source.clone());
        mod_list.extend(parsed);
    }

    // ── FIRST PASS: "Other"-type radius jewels ─────────────────────────────
    // Only fires for non-Mastery nodes in the jewel's radius.
    // Mirrors: for _, rad in pairs(env.radiusJewelList) do
    //   if rad.type == "Other" and rad.nodes[node.id] and ... then
    //     rad.func(node, modList, rad.data)
    for rad in radius_jewel_list.iter_mut() {
        if rad.jewel_type == RadiusJewelType::Other && rad.nodes.contains(&node_id) {
            (rad.func)(Some(node_id), &mut mod_list, &mut rad.data);
        }
    }

    // ── SUPPRESSION CHECK ───────────────────────────────────────────────────
    // PassiveSkillHasNoEffect: wipe all mods (node contributes nothing).
    // AllocatedPassiveSkillHasNoEffect: wipe only if this node is allocated.
    // Mirrors: if modList:Flag(nil, "PassiveSkillHasNoEffect") or
    //   (env.allocNodes[node.id] and modList:Flag(nil, "AllocatedPassiveSkillHasNoEffect"))
    let has_no_effect = mod_list_flag(&mod_list, "PassiveSkillHasNoEffect");
    let has_alloc_no_effect =
        is_allocated && mod_list_flag(&mod_list, "AllocatedPassiveSkillHasNoEffect");

    if has_no_effect || has_alloc_no_effect {
        mod_list.clear();
    }

    // ── EFFECT SCALING ─────────────────────────────────────────────────────
    // PassiveSkillEffect multiplier from INC+MORE mods on this node's modList.
    // Mirrors: local scale = calcLib.mod(modList, nil, "PassiveSkillEffect")
    //   which expands to: (1 + sum_inc/100) * product_more
    let scale = mod_list_calc_mod(&mod_list, "PassiveSkillEffect");
    if (scale - 1.0).abs() > 1e-9 {
        // ScaleAddList: multiply all BASE values by scale.
        for m in &mut mod_list {
            if m.mod_type == ModType::Base {
                if let ModValue::Number(ref mut v) = m.value {
                    *v *= scale;
                }
            }
        }
    }

    // ── SECOND PASS: Threshold / SelfAlloc / SelfUnalloc jewels ────────────
    // Mirrors: for _, rad in pairs(env.radiusJewelList) do
    //   if rad.nodes[node.id] and ... then rad.func(node, modList, rad.data) end
    for rad in radius_jewel_list.iter_mut() {
        if !rad.nodes.contains(&node_id) {
            continue;
        }
        let fires = match rad.jewel_type {
            RadiusJewelType::Threshold => true,
            RadiusJewelType::SelfAlloc => is_allocated,
            RadiusJewelType::SelfUnalloc => !is_allocated,
            RadiusJewelType::Other => false, // already handled in first pass
        };
        if fires {
            (rad.func)(Some(node_id), &mut mod_list, &mut rad.data);
        }
    }

    // ── PassiveSkillHasOtherEffect ──────────────────────────────────────────
    // Replaces the entire modList with NodeModifier LIST entries.
    // Mirrors: if modList:Flag(nil, "PassiveSkillHasOtherEffect") then
    //   for i, mod in ipairs(modList:List(skillCfg, "NodeModifier")) do
    //     if i == 1 then wipeTable(modList) end
    //     modList:AddMod(mod.mod)
    //   end
    if mod_list_flag(&mod_list, "PassiveSkillHasOtherEffect") {
        let node_modifier_mods: Vec<crate::mod_db::types::Mod> = mod_list
            .iter()
            .filter(|m| m.name == "NodeModifier" && m.mod_type == ModType::List)
            .filter_map(|_m| {
                // NodeModifier LIST entries store the replacement mod as their value.
                // In Rust this is not yet fully implemented; skip for now.
                // In Lua: mod.mod contains the replacement mod to add.
                None::<crate::mod_db::types::Mod>
            })
            .collect();
        if !node_modifier_mods.is_empty() {
            mod_list.clear();
            mod_list.extend(node_modifier_mods);
        } else {
            // If no NodeModifier entries (because this isn't implemented yet),
            // clear the modList to suppress the original node stats.
            // This is the correct behavior: PassiveSkillHasOtherEffect means
            // "replace with something else", so the original stats must be gone.
            mod_list.clear();
        }
    }

    mod_list
}

/// Compute the sum of INC mods for a given stat in a local mod list.
fn mod_list_sum_inc(mods: &[crate::mod_db::types::Mod], stat: &str) -> f64 {
    use crate::mod_db::types::ModType;
    mods.iter()
        .filter(|m| m.name == stat && m.mod_type == ModType::Inc && m.tags.is_empty())
        .map(|m| m.value.as_f64())
        .sum()
}

/// Compute the product of MORE mods for a given stat in a local mod list.
fn mod_list_more(mods: &[crate::mod_db::types::Mod], stat: &str) -> f64 {
    use crate::mod_db::types::ModType;
    mods.iter()
        .filter(|m| m.name == stat && m.mod_type == ModType::More && m.tags.is_empty())
        .fold(1.0_f64, |acc, m| acc * (1.0 + m.value.as_f64() / 100.0))
}

/// Compute calcLib.mod equivalent for a local mod list:
/// (1 + sum_inc/100) * product_more.
/// Returns 1.0 when no mods affect the named stat.
fn mod_list_calc_mod(mods: &[crate::mod_db::types::Mod], stat: &str) -> f64 {
    let inc = mod_list_sum_inc(mods, stat);
    let more = mod_list_more(mods, stat);
    (1.0 + inc / 100.0) * more
}

/// Check if a FLAG mod with given name is present in a local mod list.
fn mod_list_flag(mods: &[crate::mod_db::types::Mod], stat: &str) -> bool {
    use crate::mod_db::types::{ModType, ModValue};
    mods.iter()
        .filter(|m| m.name == stat && m.mod_type == ModType::Flag && m.tags.is_empty())
        .any(|m| matches!(&m.value, ModValue::Bool(true)))
}

/// Build the radius jewel list for all jewel slots in the active item set.
/// Mirrors CalcSetup.lua lines 751-808.
fn build_radius_jewel_list(build: &Build, env: &mut super::env::CalcEnv) {
    use crate::build::types::ItemSlot;
    use crate::calc::env::{RadiusJewelEntry, RadiusJewelType};

    let item_set = match build.item_sets.get(build.active_item_set) {
        Some(set) => set,
        None => return,
    };

    let data = env.data.clone();
    let tree = data.tree_for_version(&build.passive_spec.tree_version);

    // Build reverse lookup: item_id → socket_node_id from passive spec jewels.
    // env.spec.nodes[slot.nodeId] in Lua maps to the socket's node in the tree.
    // build.passive_spec.jewels maps socket_node_id → item_id.
    let mut item_to_socket: HashMap<u32, u32> = HashMap::new();
    for (&socket_node_id, &item_id) in &build.passive_spec.jewels {
        item_to_socket.insert(item_id, socket_node_id);
    }

    for (slot_name, &item_id) in &item_set.slots {
        let slot = match ItemSlot::from_str(slot_name) {
            Some(s) => s,
            None => continue,
        };

        if !slot.is_jewel() {
            continue;
        }

        let item = match build.items.get(&item_id) {
            Some(i) => i,
            None => continue,
        };

        // Check if this item has a radius.
        let radius_index = extract_radius_index(item);
        let radius_idx = match radius_index {
            Some(idx) => idx,
            None => continue,
        };

        // Find the socket node ID for this jewel item.
        // Mirrors env.spec.nodes[slot.nodeId] in Lua.
        let socket_node_id = match item_to_socket.get(&item_id) {
            Some(&id) => id,
            None => continue, // jewel not socketed in the passive tree
        };

        // Get the socket node from the tree.
        let socket_node = match tree.nodes.get(&socket_node_id) {
            Some(n) => n,
            None => continue,
        };

        // Get nodes in radius for this jewel.
        // Mirrors: node.nodesInRadius and node.nodesInRadius[item.jewelRadiusIndex] or {}
        // Note: nodes_in_radius is empty in current tree data (no x/y coords extracted yet).
        let nodes_in_radius: std::collections::HashSet<u32> = socket_node
            .nodes_in_radius
            .get(&radius_idx)
            .cloned()
            .unwrap_or_default();

        // Determine the jewel type from the item's name.
        let jewel_type = determine_jewel_type(item);

        // Default callback: tally Str/Dex/Int in radius.
        // Mirrors PoB's default funcList entry for jewels without custom funcList.
        let func = Box::new(
            move |node_id: Option<u32>,
                  mod_list: &mut Vec<crate::mod_db::types::Mod>,
                  data: &mut HashMap<String, f64>| {
                if node_id.is_none() {
                    // Finalise call — default func does nothing on finalise.
                    return;
                }
                // Per-node: tally Str/Dex/Int BASE mods from the node's modList.
                // Mirrors PoB's default: data[stat] = (data[stat] or 0) + out:Sum("BASE", nil, stat)
                for stat in &["Str", "Dex", "Int"] {
                    let sum: f64 = mod_list
                        .iter()
                        .filter(|m| {
                            m.name == *stat
                                && m.mod_type == crate::mod_db::types::ModType::Base
                                && m.tags.is_empty()
                        })
                        .map(|m| m.value.as_f64())
                        .sum();
                    if sum != 0.0 {
                        let entry = data.entry(stat.to_string()).or_insert(0.0);
                        *entry += sum;
                    }
                }
            },
        );

        // Add non-SelfAlloc jewels' unallocated nodes to extraRadiusNodeList.
        // Mirrors: if func.type ~= "Self" and node.nodesInRadius then ...
        if jewel_type != RadiusJewelType::SelfAlloc {
            for &nid in &nodes_in_radius {
                if !env.alloc_nodes.contains(&nid) {
                    env.extra_radius_node_list.insert(nid);
                }
            }
        }

        env.radius_jewel_list.push(RadiusJewelEntry {
            nodes: nodes_in_radius,
            func,
            jewel_type,
            node_id: socket_node_id,
            data: HashMap::new(),
        });
    }
}

/// Extract the radius index from an item's mod lines.
/// Returns None if the item has no radius.
/// Returns Some(index) where index is 1-based: Small=1, Medium=2, Large=3, VeryLarge=4, Massive=5.
fn extract_radius_index(item: &crate::build::types::Item) -> Option<usize> {
    // Check all mod lines for a "Radius:" or "Only affects Passives in X Ring" line.
    // First try explicit "Radius: X" from item text (stored in item.base_type or implicits).
    // In the build XML, the radius is encoded as "Radius: Variable" or "Radius: Large" etc.
    // as a property line on the item.

    // Check the item's `radius` field if present (set by XML parser).
    // For now check the implicits/explicits for radius-related text.
    let all_lines = item
        .implicits
        .iter()
        .chain(item.explicits.iter())
        .chain(item.crafted_mods.iter());

    for line in all_lines {
        let lower = line.to_lowercase();
        // Thread of Hope: "Only affects Passives in X Ring"
        if lower.contains("only affects passives in small ring") {
            return Some(1); // Small radius index
        }
        if lower.contains("only affects passives in medium ring") {
            return Some(2);
        }
        if lower.contains("only affects passives in large ring") {
            return Some(3);
        }
        if lower.contains("only affects passives in very large ring") {
            return Some(4);
        }
        if lower.contains("only affects passives in massive ring") {
            return Some(5);
        }
    }

    // Check item radius field from XML parser (stored as a property).
    // This is the "Radius: Small" etc. line from the item description.
    match item.radius.as_deref() {
        Some("Small") => Some(1),
        Some("Medium") => Some(2),
        Some("Large") => Some(3),
        Some("Very Large") | Some("VeryLarge") => Some(4),
        Some("Massive") => Some(5),
        Some("Variable") => {
            // Variable radius: needs special handling (Thread of Hope's ring variants).
            // The ring determines which radius band applies.
            // Default to the "Variable Medium" band (index 7 in 3_16 data for medium band).
            // For now, return None until full variable radius support is implemented.
            None
        }
        _ => None,
    }
}

/// Determine the RadiusJewelType for an item.
/// Thread of Hope → SelfUnalloc (allows allocating unconnected nodes).
/// Intuitive Leap → SelfAlloc (allocated nodes in radius count without connectivity).
/// Timeless jewels → Other (handled by SETUP-06, but registered here for PassiveSkillHasNoEffect).
/// Default → SelfAlloc.
fn determine_jewel_type(item: &crate::build::types::Item) -> super::env::RadiusJewelType {
    use super::env::RadiusJewelType;

    let name_lower = item.name.to_lowercase();
    let _base_lower = item.base_type.to_lowercase();

    // Check by item name for known unique jewels.
    if name_lower.contains("thread of hope") {
        return RadiusJewelType::SelfUnalloc;
    }
    if name_lower.contains("intuitive leap") {
        return RadiusJewelType::SelfAlloc;
    }
    if name_lower.contains("impossible escape") {
        return RadiusJewelType::SelfAlloc;
    }
    // Unnatural Instinct: grants all bonuses of unallocated small passives in radius,
    // allocated small passives grant nothing.
    if name_lower.contains("unnatural instinct") {
        return RadiusJewelType::SelfUnalloc;
    }

    // Default for regular jewels: SelfAlloc (only fires when node is allocated).
    RadiusJewelType::SelfAlloc
}

/// Compute the set of passive node IDs that are reachable from a class or
/// ascendancy start node, traversing only through the allocated node set.
///
/// Mirrors PoB's `BuildAllDependsAndPaths` which prunes "orphan" nodes —
/// allocated nodes that can't be traced back to the character's start.
///
/// Also enforces the mastery gate from `ImportFromNodeList`:
///   mastery nodes are only considered allocated if they have a selection in
///   `build.passive_spec.mastery_selections`.
fn connected_passive_nodes(
    build: &Build,
    tree: &crate::passive_tree::PassiveTree,
) -> std::collections::HashSet<u32> {
    use crate::passive_tree::NodeType;
    use std::collections::{HashSet, VecDeque};

    // Gate: mastery nodes are only allocated if they have a selection.
    // Mirrors PassiveSpec.lua ImportFromNodeList:269-273.
    let allocated: HashSet<u32> = build
        .passive_spec
        .allocated_nodes
        .iter()
        .copied()
        .filter(|&nid| {
            // Check if this is a mastery node
            match tree.nodes.get(&nid) {
                Some(node) if node.node_type == NodeType::Mastery => {
                    // Mastery nodes require a selection to be considered allocated
                    build.passive_spec.mastery_selections.contains_key(&nid)
                }
                _ => true, // Non-mastery nodes are always allocated
            }
        })
        .collect();

    if allocated.is_empty() {
        return HashSet::new();
    }

    // Build bidirectional adjacency restricted to allocated nodes (using tree out links)
    let mut adj: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    for &nid in &allocated {
        if let Some(node) = tree.nodes.get(&nid) {
            for &linked in &node.linked_ids {
                if allocated.contains(&linked) {
                    adj.entry(nid).or_default().push(linked);
                    adj.entry(linked).or_default().push(nid);
                }
            }
        }
    }

    // Find start nodes: class start nodes and ascendancy start nodes in the tree
    // that are in the allocated set OR are the expected class/ascendancy starts.
    let mut start_ids: HashSet<u32> = HashSet::new();
    for (&nid, node) in &tree.nodes {
        if matches!(
            node.node_type,
            NodeType::ClassStart | NodeType::AscendancyStart
        ) {
            start_ids.insert(nid);
        }
    }

    // BFS from all start nodes that are in the allocated set
    let mut reachable: HashSet<u32> = HashSet::new();
    let mut queue: VecDeque<u32> = VecDeque::new();

    for &nid in &start_ids {
        if allocated.contains(&nid) {
            reachable.insert(nid);
            queue.push_back(nid);
        }
    }

    while let Some(curr) = queue.pop_front() {
        if let Some(neighbors) = adj.get(&curr) {
            for &next in neighbors {
                if !reachable.contains(&next) {
                    reachable.insert(next);
                    queue.push_back(next);
                }
            }
        }
    }

    // If there are no start nodes in the tree data at all (e.g. test builds with minimal trees),
    // fall back to applying all allocated nodes. This allows unit tests with custom trees to work.
    if start_ids.is_empty() {
        return allocated;
    }

    // If no start nodes are in the allocated set, the build's passive nodes are disconnected
    // (orphaned). Don't apply any passive mods in this case.
    // This matches PoB's BuildAllDependsAndPaths behavior which prunes orphaned nodes.
    reachable
}

/// Normalize a gem skill ID from the build XML to match our gems.json keys.
/// PoB uses legacy/different skill IDs in old builds (e.g. "SparkProjectile", "HatredAura").
/// This function maps those to the current gem IDs used in our data.
fn normalize_gem_skill_id(skill_id: &str) -> &str {
    // Explicit mappings for known legacy IDs from old PoB builds.
    // Maps old skill IDs used in build XML files to current skill IDs in our data.
    match skill_id {
        // Active skill renames
        "SparkProjectile" => "Spark",
        "AccuracyAndCritsAura" => "Precision",
        "NewPhaseRun" => "PhaseRun",
        "NewShieldCharge" => "ShieldCharge",
        "BladestormSandstorm" => "BloodAndSand",
        "AncestralWarchief" => "AncestralProtector",
        "VaalAncestralWarchief" => "AncestralProtector",
        // Aura skill renames (removed "Aura" suffix in newer PoB)
        "HatredAura" => "Hatred",
        "AngerAura" => "Anger",
        "WrathAura" => "Wrath",
        "DamageOverTimeAura" => "Malevolence",
        "PhysicalDamageAura" => "Pride",
        "SpellDamageAura" => "Zealotry",
        "FlammabilityAura" => "Flammability",
        "FlamabilityAura" => "Flammability",
        "PurityOfFireAura" => "PurityOfFire",
        // Support gem renames
        "SupportPenetration" => "SupportLightningPenetration",
        "SupportFasterAttack" => "SupportFasterAttacks",
        "SupportRapidDecay" => "SupportSwiftAffliction",
        "SupportCastOnDamageTaken" => "SupportCastWhenDamageTaken",
        "SupportCastOnCritPlus" => "SupportCastOnCriticalStrike",
        "SupportBrutalityPlus" => "SupportBrutality",
        "SupportControlledDestructionPlus" => "SupportControlledDestruction",
        "SupportGreaterMultipleProjectilesPlus" => "SupportGreaterMultipleProjectiles",
        "SupportMeleePhysicalDamagePlus" => "SupportMeleePhysicalDamage",
        "SupportViciousProjectilesPlus" => "SupportViciousProjectiles",
        "SupportVoidManipulationPlus" => "SupportVoidManipulation",
        "SupportMultipleTotem" => "SupportMultipleTotems",
        "SupportMultiTotem" => "SupportMultipleTotems",
        "SupportRangedAttackTotem" => "SupportMultipleTotems",
        "SupportGemMirageArcher" => "SupportMirageArcher",
        "SupportTrinitySupportGem" => "SupportTrinity",
        "SupportSlowProjectile" => "SupportSlowerProjectiles",
        "SupportWeaponElementalDamage" => "SupportElementalDamageWithAttacks",
        "SupportIncreasedDuration" => "SupportMoreDuration",
        "SupportIgniteChance" => "SupportIgniteProliferation",
        "SupportPowerChargeOnCrit" => "SupportPowerChargeOnCritical",
        "SupportChaosAttacks" => "SupportWitheringTouch",
        "SupportMeleeDamageOnFullLife" => "SupportDamageOnFullLife",
        "SupportRemoteMine" => "SupportBlastchainMine",
        "SupportDamageAgainstChilled" => "SupportHypothermia",
        "SupportStormBarrier" => "SupportInfusedChannelling",
        "SupportAdditionalXP" => "SupportEnlighten",
        "SupportAdditionalLevel" => "SupportEmpower",
        "SupportAdditionalCooldown" => "SupportSecondWind",
        "RainOfSpores" => "ToxicRain",
        "BloodSandStance" => "BloodAndSand",
        "BloodSandArmour" => "FleshAndStone",
        "BloodSpears" => "Perforate",
        "ChargedAttack" => "BladeFlurry",
        "HiddenBlade" => "EtherealKnives",
        "IceStrike" => "FrostBlades",
        "PuresteelBanner" => "DreadBanner",
        "ConduitSigil" => "ArmageddonBrand",
        "FrostBoltNova" => "FrostBolt",
        "QuickDodge" => "Dash",
        // Default: return as-is
        other => other,
    }
}

/// Build the requirements table for attribute requirements computation.
/// Mirrors PoB's requirementsTable construction in CalcSetup.lua.
/// Populates env.requirements_table with one entry per equipped item/gem that has attr requirements.
fn build_requirements_table(build: &Build, env: &mut CalcEnv) {
    use crate::calc::env::RequirementEntry;

    let item_set = match build.item_sets.get(build.active_item_set) {
        Some(set) => set,
        None => return,
    };

    // 1. Item requirements
    for (slot_name, &item_id) in &item_set.slots {
        let slot = match ItemSlot::from_str(slot_name) {
            Some(s) => s,
            None => continue,
        };

        // Skip flask and jewel slots for requirements
        if slot.is_flask() || slot.is_jewel() {
            continue;
        }

        let item = match build.items.get(&item_id) {
            Some(i) => i,
            None => continue,
        };

        // Resolve item requirements from base type
        let mut resolved_item = item.clone();
        crate::build::item_resolver::resolve_item_base(&mut resolved_item, &env.data.bases);

        let base_req = &resolved_item.requirements;
        let mut str_req = base_req.str_req as f64;
        let mut dex_req = base_req.dex_req as f64;
        let mut int_req = base_req.int_req as f64;

        // Apply item-local requirement mods (e.g. "+257 Intelligence Requirement" on Cospri's Malice).
        // These are parsed as BASE mods on StrRequirement, DexRequirement, IntRequirement.
        let req_src = crate::mod_db::types::ModSource::new("Item", &resolved_item.name);
        let mut local_db = crate::mod_db::ModDb::new();
        for mod_lines in resolved_item
            .implicits
            .iter()
            .chain(resolved_item.explicits.iter())
        {
            let mods = crate::build::mod_parser::parse_mod(mod_lines, req_src.clone());
            for m in mods {
                local_db.add(m);
            }
        }
        let empty_output = std::collections::HashMap::new();
        let str_req_mod = local_db.sum_cfg(
            crate::mod_db::types::ModType::Base,
            "StrRequirement",
            None,
            &empty_output,
        );
        let dex_req_mod = local_db.sum_cfg(
            crate::mod_db::types::ModType::Base,
            "DexRequirement",
            None,
            &empty_output,
        );
        let int_req_mod = local_db.sum_cfg(
            crate::mod_db::types::ModType::Base,
            "IntRequirement",
            None,
            &empty_output,
        );
        str_req += str_req_mod;
        dex_req += dex_req_mod;
        int_req += int_req_mod;

        if str_req > 0.0 || dex_req > 0.0 || int_req > 0.0 {
            env.requirements_table.push(RequirementEntry {
                str_req,
                dex_req,
                int_req,
                source_name: format!("{} ({})", resolved_item.name, slot_name),
            });
        }
    }

    // 2. Gem requirements from equipped skill slots
    // Only process gems in equipped item slots (not unequipped)
    // Use the active skill set
    let active_skill_set = build
        .skill_sets
        .get(build.active_skill_set)
        .or_else(|| build.skill_sets.first());
    let skills: &[crate::build::types::Skill] = match active_skill_set {
        Some(ss) => &ss.skills,
        None => &[],
    };

    for skill in skills {
        // Only process gems that are in an equipped slot
        let slot_name = skill.slot.clone();

        if !skill.enabled {
            continue;
        }

        // Check that the slot is equipped
        let is_equipped = item_set.slots.contains_key(&slot_name);
        if !is_equipped {
            continue;
        }

        for gem in &skill.gems {
            if !gem.enabled {
                continue;
            }

            // Look up gem data (with fallback normalization for legacy skill IDs)
            let resolved_skill_id = normalize_gem_skill_id(&gem.skill_id);
            let gem_data = match env.data.gems.get(resolved_skill_id) {
                Some(d) => d,
                None => continue,
            };

            // Compute attribute requirement from gem color and level requirement
            let gem_level = gem.level as usize;
            let level_req = gem_data
                .levels
                .iter()
                .find(|l| l.level as usize == gem_level)
                .map(|l| l.level_requirement)
                .unwrap_or(1);

            if level_req == 0 {
                continue;
            }

            // Determine multipliers from gem requirements data (or fallback to color)
            let (req_str_multi, req_dex_multi, req_int_multi) =
                gem_requirements(resolved_skill_id, gem_data, &env.data.gem_reqs);

            let stat_type = if gem_data.is_support {
                0.5_f64
            } else {
                0.7_f64
            };

            let str_req = if req_str_multi > 0 {
                compute_gem_attr_req(level_req as f64, req_str_multi as f64, stat_type)
            } else {
                0.0
            };
            let dex_req = if req_dex_multi > 0 {
                compute_gem_attr_req(level_req as f64, req_dex_multi as f64, stat_type)
            } else {
                0.0
            };
            let int_req = if req_int_multi > 0 {
                compute_gem_attr_req(level_req as f64, req_int_multi as f64, stat_type)
            } else {
                0.0
            };

            if str_req > 0.0 || dex_req > 0.0 || int_req > 0.0 {
                env.requirements_table.push(RequirementEntry {
                    str_req,
                    dex_req,
                    int_req,
                    source_name: format!("{} ({})", gem_data.display_name, slot_name),
                });
            }
        }
    }
}

/// Get the attribute requirement multipliers for a gem.
/// First tries the gem_reqs lookup table (accurate PoB data),
/// then falls back to color-based heuristic.
/// Returns (str_multi, dex_multi, int_multi) each 0-100.
fn gem_requirements(
    skill_id: &str,
    gem_data: &crate::data::gems::GemData,
    gem_reqs: &std::collections::HashMap<String, crate::data::GemReqMultipliers>,
) -> (u32, u32, u32) {
    // Try exact skill ID match in gem_reqs
    if let Some(req) = gem_reqs.get(skill_id) {
        return (req.req_str, req.req_dex, req.req_int);
    }

    // Fallback: color-based heuristic
    match gem_data.color.as_deref() {
        Some("strength") => (100, 0, 0),
        Some("dexterity") => (0, 100, 0),
        Some("intelligence") => (0, 0, 100),
        _ => (0, 0, 0),
    }
}

/// Compute the actual attribute requirement from level requirement and multiplier.
/// Mirrors PoB's calcLib.getGemStatRequirement().
/// formula: floor(x + 0.5) where x = (20 + (level-3)*3) * (multi/100)^0.9 * statType
/// Uses PoB's round() = floor(x + 0.5) (round half up, not banker's rounding).
/// Returns 0 if result < 14.
fn compute_gem_attr_req(level_req: f64, multi: f64, stat_type: f64) -> f64 {
    if multi == 0.0 {
        return 0.0;
    }
    let x = (20.0 + (level_req - 3.0) * 3.0) * (multi / 100.0_f64).powf(0.9) * stat_type;
    let req = (x + 0.5).floor();
    if req < 14.0 {
        0.0
    } else {
        req
    }
}

#[cfg(test)]
mod tests {
    use super::init_env;
    use crate::{
        build::{parse_xml, types::Build},
        data::GameData,
        mod_db::types::{KeywordFlags, ModFlags, ModType},
    };
    use std::sync::Arc;

    fn make_data_with_node(node_id: u32, stat: &str) -> Arc<GameData> {
        let json = format!(
            r#"{{
            "gems": {{}},
            "misc": {{
                "game_constants": {{
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
                    "soul_eater_maximum_stacks": 40,
                    "maximum_righteous_charges": 10,
                    "maximum_blood_scythe_charges": 8
                }},
                "character_constants": {{"life_per_str": 0.5}},
                "monster_life_table": [],
                "monster_damage_table": [],
                "monster_evasion_table": [],
                "monster_accuracy_table": [],
                "monster_ally_life_table": [],
                "monster_ally_damage_table": [],
                "monster_ailment_threshold_table": [],
                "monster_phys_conversion_multi_table": []
            }},
            "tree": {{
                "nodes": {{
                    "{node_id}": {{ "id": {node_id}, "name": "Test Node", "stats": ["{stat}"], "out": [] }}
                }}
            }}
        }}"#
        );
        Arc::new(GameData::from_json(&json).unwrap())
    }

    fn build_with_node(node_id: u32) -> Build {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="{node_id}" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#
        );
        parse_xml(&xml).unwrap()
    }

    fn build_with_class(class_name: &str) -> Build {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="{class_name}" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#
        );
        parse_xml(&xml).unwrap()
    }

    fn make_default_data() -> Arc<GameData> {
        make_data_with_node(0, "")
    }

    #[test]
    fn allocated_life_node_increases_life_base() {
        let node_id = 99999u32;
        let data = make_data_with_node(node_id, "+40 to maximum Life");
        let build = build_with_node(node_id);
        let env = init_env(&build, data).unwrap();
        let life_base =
            env.player
                .mod_db
                .sum(ModType::Base, "Life", ModFlags::NONE, KeywordFlags::NONE);
        // The base class life (38 + 12*90 = 1118) + 40 from the node
        assert!(
            life_base > 40.0,
            "Life base should include node contribution, got {life_base}"
        );
        // More precisely: should be base (1118) + 40 = 1158
        assert!(
            life_base >= 1118.0 + 40.0 - 1.0,
            "Life base should be at least 1157, got {life_base}"
        );
    }

    #[test]
    fn unallocated_node_has_no_effect() {
        let node_id = 99998u32;
        let data = make_data_with_node(node_id, "+40 to maximum Life");
        // Build without that node allocated
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let env = init_env(&build, data).unwrap();
        // Use tabulate to check no Passive source for Life
        let tabs = env
            .player
            .mod_db
            .tabulate("Life", None, ModFlags::NONE, KeywordFlags::NONE);
        assert!(
            !tabs
                .iter()
                .any(|t| t.source_category == "Passive" && t.source_name == "Test Node"),
            "Unallocated node should not contribute to Life"
        );
    }

    // --- Task 4 tests: base constants ---

    #[test]
    fn base_constants_include_crit_cap_and_leech() {
        let data = make_default_data();
        let build = build_with_class("Marauder");
        let env = init_env(&build, data).unwrap();

        let crit_cap = env.player.mod_db.sum(
            ModType::Base,
            "CritChanceCap",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(crit_cap, 100.0, "CritChanceCap should be 100");

        let totem_limit = env.player.mod_db.sum(
            ModType::Base,
            "ActiveTotemLimit",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert!(
            totem_limit >= 1.0,
            "ActiveTotemLimit should be >= 1, got {totem_limit}"
        );

        let spell_block_max = env.player.mod_db.sum(
            ModType::Base,
            "SpellBlockChanceMax",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(spell_block_max, 75.0, "SpellBlockChanceMax should be 75");

        let max_life_leech = env.player.mod_db.sum(
            ModType::Base,
            "MaxLifeLeechRate",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(max_life_leech, 20.0, "MaxLifeLeechRate should be 20");

        let crit_multi = env.player.mod_db.sum(
            ModType::Base,
            "CritMultiplier",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(crit_multi, 150.0, "CritMultiplier should be 150");
    }

    #[test]
    fn base_constants_include_timing_and_totem_resists() {
        let data = make_default_data();
        let build = build_with_class("Marauder");
        let env = init_env(&build, data).unwrap();

        let trap_time = env.player.mod_db.sum(
            ModType::Base,
            "TrapThrowTime",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert!(
            (trap_time - 0.6).abs() < 0.01,
            "TrapThrowTime should be 0.6"
        );

        let mine_time = env.player.mod_db.sum(
            ModType::Base,
            "MineLayingTime",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert!(
            (mine_time - 0.3).abs() < 0.01,
            "MineLayingTime should be 0.3"
        );

        let totem_fire = env.player.mod_db.sum(
            ModType::Base,
            "TotemFireResist",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(totem_fire, 40.0, "TotemFireResist should be 40");

        let totem_chaos = env.player.mod_db.sum(
            ModType::Base,
            "TotemChaosResist",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(totem_chaos, 20.0, "TotemChaosResist should be 20");
    }

    #[test]
    fn base_constants_include_ailment_and_dot() {
        let data = make_default_data();
        let build = build_with_class("Marauder");
        let env = init_env(&build, data).unwrap();

        let max_shock = env.player.mod_db.sum(
            ModType::Base,
            "MaxShockStacks",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(max_shock, 1.0, "MaxShockStacks should be 1");

        let wither_max = env.player.mod_db.sum(
            ModType::Base,
            "WitherStacksMax",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(wither_max, 15.0, "WitherStacksMax should be 15");

        let bleed_dur = env.player.mod_db.sum(
            ModType::Base,
            "BleedDurationBase",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(bleed_dur, 4.0, "BleedDurationBase should be 4");

        let poison_dur = env.player.mod_db.sum(
            ModType::Base,
            "PoisonDurationBase",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(poison_dur, 2.0, "PoisonDurationBase should be 2");
    }

    // --- Task 5 tests: class stats, resistance penalty, accuracy ---

    #[test]
    fn base_stats_include_resistance_penalty() {
        let data = make_default_data();
        let build = build_with_class("Marauder");
        let env = init_env(&build, data).unwrap();

        let fire_resist = env.player.mod_db.sum(
            ModType::Base,
            "FireResist",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert!(
            fire_resist < 0.0,
            "FireResist base should be < 0 (includes -60 penalty), got {fire_resist}"
        );
        assert_eq!(fire_resist, -60.0, "FireResist should be exactly -60");

        let cold_resist = env.player.mod_db.sum(
            ModType::Base,
            "ColdResist",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(cold_resist, -60.0, "ColdResist should be exactly -60");

        let lightning_resist = env.player.mod_db.sum(
            ModType::Base,
            "LightningResist",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(
            lightning_resist, -60.0,
            "LightningResist should be exactly -60"
        );
    }

    #[test]
    fn base_stats_include_accuracy() {
        let data = make_default_data();
        let build = build_with_class("Marauder"); // level 90
        let env = init_env(&build, data).unwrap();

        let accuracy = env.player.mod_db.sum(
            ModType::Base,
            "Accuracy",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        // For L90: accuracy = 2 * 90 = 180
        assert!(
            accuracy >= 180.0,
            "For L90: accuracy should be >= 180 (2 * 90), got {accuracy}"
        );
        assert_eq!(accuracy, 180.0, "Accuracy should be exactly 2 * 90 = 180");
    }

    #[test]
    fn base_stats_include_evasion() {
        let data = make_default_data();
        let build = build_with_class("Marauder"); // level 90
        let env = init_env(&build, data).unwrap();

        let evasion =
            env.player
                .mod_db
                .sum(ModType::Base, "Evasion", ModFlags::NONE, KeywordFlags::NONE);
        // For L90: evasion = 53 + 3 * 90 = 323
        assert_eq!(evasion, 323.0, "Evasion should be 53 + 3*90 = 323");
    }

    #[test]
    fn class_base_attributes_marauder() {
        let data = make_default_data();
        let build = build_with_class("Marauder");
        let env = init_env(&build, data).unwrap();

        let str_val =
            env.player
                .mod_db
                .sum(ModType::Base, "Str", ModFlags::NONE, KeywordFlags::NONE);
        let dex_val =
            env.player
                .mod_db
                .sum(ModType::Base, "Dex", ModFlags::NONE, KeywordFlags::NONE);
        let int_val =
            env.player
                .mod_db
                .sum(ModType::Base, "Int", ModFlags::NONE, KeywordFlags::NONE);

        assert_eq!(str_val, 32.0, "Marauder Str should be 32, got {str_val}");
        assert_eq!(dex_val, 14.0, "Marauder Dex should be 14, got {dex_val}");
        assert_eq!(int_val, 14.0, "Marauder Int should be 14, got {int_val}");
    }

    #[test]
    fn class_base_attributes_ranger() {
        let data = make_default_data();
        let build = build_with_class("Ranger");
        let env = init_env(&build, data).unwrap();

        let str_val =
            env.player
                .mod_db
                .sum(ModType::Base, "Str", ModFlags::NONE, KeywordFlags::NONE);
        let dex_val =
            env.player
                .mod_db
                .sum(ModType::Base, "Dex", ModFlags::NONE, KeywordFlags::NONE);
        let int_val =
            env.player
                .mod_db
                .sum(ModType::Base, "Int", ModFlags::NONE, KeywordFlags::NONE);

        assert_eq!(str_val, 14.0, "Ranger Str should be 14");
        assert_eq!(dex_val, 32.0, "Ranger Dex should be 32");
        assert_eq!(int_val, 14.0, "Ranger Int should be 14");
    }

    #[test]
    fn class_base_attributes_scion() {
        let data = make_default_data();
        let build = build_with_class("Scion");
        let env = init_env(&build, data).unwrap();

        let str_val =
            env.player
                .mod_db
                .sum(ModType::Base, "Str", ModFlags::NONE, KeywordFlags::NONE);
        let dex_val =
            env.player
                .mod_db
                .sum(ModType::Base, "Dex", ModFlags::NONE, KeywordFlags::NONE);
        let int_val =
            env.player
                .mod_db
                .sum(ModType::Base, "Int", ModFlags::NONE, KeywordFlags::NONE);

        assert_eq!(str_val, 20.0, "Scion Str should be 20");
        assert_eq!(dex_val, 20.0, "Scion Dex should be 20");
        assert_eq!(int_val, 20.0, "Scion Int should be 20");
    }

    // --- Task 6 tests: item mods added to player moddb ---

    #[test]
    fn item_mods_added_to_player_moddb() {
        // XML with a belt that has "+30 to maximum Life" and "+40 to Strength"
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1">
    <Item id="1">
Rarity: RARE
Test Belt
Leather Belt
Implicits: 0
+30 to maximum Life
+40 to Strength
    </Item>
    <ItemSet id="1">
      <Slot name="Belt" itemId="1"/>
    </ItemSet>
  </Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let data = make_default_data();
        let env = init_env(&build, data).unwrap();

        // Check that item mods are sourced from "Item"
        let tabs = env
            .player
            .mod_db
            .tabulate("Life", None, ModFlags::NONE, KeywordFlags::NONE);
        assert!(
            tabs.iter()
                .any(|t| t.source_category == "Item" && t.value.as_f64().abs() >= 30.0),
            "Should find a +30 Life mod from Item source, tabs: {:?}",
            tabs
        );

        let str_tabs = env
            .player
            .mod_db
            .tabulate("Str", None, ModFlags::NONE, KeywordFlags::NONE);
        assert!(
            str_tabs
                .iter()
                .any(|t| t.source_category == "Item" && t.value.as_f64().abs() >= 40.0),
            "Should find a +40 Str mod from Item source, tabs: {:?}",
            str_tabs
        );
    }

    #[test]
    fn flask_and_jewel_slots_skipped() {
        // Ensure flask/jewel items are NOT added to the player moddb
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1">
    <Item id="1">
Rarity: NORMAL
Divine Life Flask
Implicits: 0
+500 to maximum Life
    </Item>
    <Item id="2">
Rarity: RARE
Test Jewel
Cobalt Jewel
Implicits: 0
+10 to Intelligence
    </Item>
    <ItemSet id="1">
      <Slot name="Flask 1" itemId="1"/>
      <Slot name="Jewel 1" itemId="2"/>
    </ItemSet>
  </Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let data = make_default_data();
        let env = init_env(&build, data).unwrap();

        // Neither flask nor jewel mods should be in the player moddb as Item-sourced
        let life_tabs =
            env.player
                .mod_db
                .tabulate("Life", None, ModFlags::NONE, KeywordFlags::NONE);
        assert!(
            !life_tabs
                .iter()
                .any(|t| t.source_category == "Item" && t.value.as_f64() >= 500.0),
            "Flask mods should NOT be in player moddb"
        );

        let int_tabs = env
            .player
            .mod_db
            .tabulate("Int", None, ModFlags::NONE, KeywordFlags::NONE);
        assert!(
            !int_tabs
                .iter()
                .any(|t| t.source_category == "Item" && t.source_name == "Cobalt Jewel"),
            "Jewel mods should NOT be in player moddb"
        );
    }

    // --- Task 8 tests: weapon data extraction ---

    #[test]
    fn weapon_data_extracted_from_equipped_weapon() {
        // Build with a weapon item in Weapon 1 slot
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
            "tree": { "nodes": {} },
            "bases": [
                {
                    "name": "Rusted Sword",
                    "item_type": "One Handed Sword",
                    "sub_type": "Sword",
                    "socket_limit": 3,
                    "tags": ["sword", "weapon"],
                    "weapon": {
                        "physical_min": 10.0,
                        "physical_max": 20.0,
                        "crit_chance_base": 5.0,
                        "attack_rate_base": 1.4,
                        "range": 11
                    }
                }
            ]
        }"#;
        let data = Arc::new(GameData::from_json(json).unwrap());

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1">
    <Item id="1">
Rarity: RARE
Test Sword
Rusted Sword
Quality: 20
Implicits: 0
Adds 10 to 20 Physical Damage
    </Item>
    <ItemSet id="1">
      <Slot name="Weapon 1" itemId="1"/>
    </ItemSet>
  </Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let env = init_env(&build, data).unwrap();

        // weapon_data1 should be populated
        let wd = env
            .player
            .weapon_data1
            .as_ref()
            .expect("weapon_data1 should be set for Weapon 1 slot");
        // Quality 20 => factor 1.2, phys_min = 10 * 1.2 = 12.0
        assert!(
            (wd.phys_min - 12.0).abs() < 1e-9,
            "phys_min should be 12.0, got {}",
            wd.phys_min
        );
        assert!(
            (wd.phys_max - 24.0).abs() < 1e-9,
            "phys_max should be 24.0, got {}",
            wd.phys_max
        );
        assert!(
            (wd.attack_rate - 1.4).abs() < 1e-9,
            "attack_rate should be 1.4"
        );
        assert!(!env.player.has_shield, "should not have shield");
        assert!(!env.player.dual_wield, "should not be dual wielding");
    }

    #[test]
    fn shield_in_weapon2_sets_has_shield() {
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
            "tree": { "nodes": {} },
            "bases": [
                {
                    "name": "Rusted Sword",
                    "item_type": "One Handed Sword",
                    "weapon": {
                        "physical_min": 10.0,
                        "physical_max": 20.0,
                        "crit_chance_base": 5.0,
                        "attack_rate_base": 1.4,
                        "range": 11
                    }
                },
                {
                    "name": "Kite Shield",
                    "item_type": "Shield",
                    "armour": {
                        "armour_min": 50.0,
                        "armour_max": 60.0,
                        "block_chance": 22
                    }
                }
            ]
        }"#;
        let data = Arc::new(GameData::from_json(json).unwrap());

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1">
    <Item id="1">
Rarity: NORMAL
Rusted Sword
Rusted Sword
Implicits: 0
    </Item>
    <Item id="2">
Rarity: NORMAL
Kite Shield
Kite Shield
Implicits: 0
    </Item>
    <ItemSet id="1">
      <Slot name="Weapon 1" itemId="1"/>
      <Slot name="Weapon 2" itemId="2"/>
    </ItemSet>
  </Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let env = init_env(&build, data).unwrap();

        assert!(
            env.player.weapon_data1.is_some(),
            "weapon_data1 should be set"
        );
        assert!(env.player.has_shield, "has_shield should be true");
        // Shield has no weapon data, so weapon_data2 should be None
        assert!(
            env.player.weapon_data2.is_none(),
            "weapon_data2 should be None for shield"
        );
        assert!(
            !env.player.dual_wield,
            "should not be dual wielding with a shield"
        );
    }

    #[test]
    fn dual_wield_detected() {
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
            "tree": { "nodes": {} },
            "bases": [
                {
                    "name": "Rusted Sword",
                    "item_type": "One Handed Sword",
                    "weapon": {
                        "physical_min": 10.0,
                        "physical_max": 20.0,
                        "crit_chance_base": 5.0,
                        "attack_rate_base": 1.4,
                        "range": 11
                    }
                }
            ]
        }"#;
        let data = Arc::new(GameData::from_json(json).unwrap());

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1">
    <Item id="1">
Rarity: NORMAL
Rusted Sword
Rusted Sword
Implicits: 0
    </Item>
    <Item id="2">
Rarity: NORMAL
Rusted Sword
Rusted Sword
Implicits: 0
    </Item>
    <ItemSet id="1">
      <Slot name="Weapon 1" itemId="1"/>
      <Slot name="Weapon 2" itemId="2"/>
    </ItemSet>
  </Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let env = init_env(&build, data).unwrap();

        assert!(
            env.player.weapon_data1.is_some(),
            "weapon_data1 should be set"
        );
        assert!(
            env.player.weapon_data2.is_some(),
            "weapon_data2 should be set"
        );
        assert!(!env.player.has_shield, "should not have shield");
        assert!(env.player.dual_wield, "should be dual wielding");
    }

    // --- Task 11 tests: enemy ModDb initialization ---

    #[test]
    fn enemy_db_initialized_with_level() {
        let data = make_default_data();
        let build = build_with_class("Marauder"); // level 90
        let env = init_env(&build, data).unwrap();

        let level =
            env.enemy
                .mod_db
                .sum(ModType::Base, "Level", ModFlags::NONE, KeywordFlags::NONE);
        assert_eq!(level, 90.0, "Enemy level should be 90");
    }

    #[test]
    fn enemy_db_has_base_resistances() {
        let data = make_default_data();
        let build = build_with_class("Marauder");
        let env = init_env(&build, data).unwrap();

        let fire = env.enemy.mod_db.sum(
            ModType::Base,
            "FireResist",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(fire, 0.0, "Enemy base FireResist should be 0");

        let phys_dr = env.enemy.mod_db.sum(
            ModType::Base,
            "PhysicalDamageReduction",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(phys_dr, 0.0, "Enemy PhysicalDamageReduction should be 0");
    }

    #[test]
    fn enemy_db_config_overrides() {
        // Build with enemyFireResist = 40
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config>
    <Input name="enemyFireResist" number="40"/>
  </Config>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let data = make_default_data();
        let env = init_env(&build, data).unwrap();

        let fire = env.enemy.mod_db.sum(
            ModType::Base,
            "FireResist",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        // Base 0 + config override 40 = 40
        assert_eq!(
            fire, 40.0,
            "Enemy FireResist should include config override, got {fire}"
        );
    }

    #[test]
    fn enemy_db_monster_life_from_table() {
        // Create data with a monster_life_table that has an entry for level 3
        let json = r#"{
            "gems": {},
            "misc": {
                "game_constants": {},
                "character_constants": {},
                "monster_life_table": [100, 120, 145],
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
        let data = Arc::new(GameData::from_json(json).unwrap());

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="3" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let env = init_env(&build, data).unwrap();

        let life = env
            .enemy
            .mod_db
            .sum(ModType::Base, "Life", ModFlags::NONE, KeywordFlags::NONE);
        // Level 3 → index 2 → 145
        assert_eq!(
            life, 145.0,
            "Enemy life should be 145 for level 3, got {life}"
        );
    }

    // --- Task 12 tests: jewel processing ---

    #[test]
    fn jewel_mods_added_to_player_moddb() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1">
    <Item id="1">
Rarity: RARE
Test Jewel
Cobalt Jewel
Implicits: 0
+15 to maximum Life
    </Item>
    <ItemSet id="1">
      <Slot name="Jewel 1" itemId="1"/>
    </ItemSet>
  </Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let data = make_default_data();
        let env = init_env(&build, data).unwrap();

        // Check that the jewel mod is in the player moddb as Item-sourced
        let tabs = env
            .player
            .mod_db
            .tabulate("Life", None, ModFlags::NONE, KeywordFlags::NONE);
        assert!(
            tabs.iter().any(|t| t.source_category == "Item"
                && t.source_name.contains("Cobalt Jewel")
                && t.source_name.contains("Jewel 1")
                && t.value.as_f64().abs() >= 15.0),
            "Should find a +15 Life mod from jewel Item source, tabs: {:?}",
            tabs
        );
    }

    // --- Task 13 tests: flask processing ---

    #[test]
    fn flask_mods_added_when_using_flask() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1">
    <Item id="1">
Rarity: MAGIC
Seething Divine Life Flask of Staunching
Divine Life Flask
Implicits: 0
+50 to maximum Life
    </Item>
    <ItemSet id="1">
      <Slot name="Flask 1" itemId="1"/>
    </ItemSet>
  </Items>
  <Config>
    <Input name="conditionUsingFlask" boolean="true"/>
  </Config>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let data = make_default_data();
        let env = init_env(&build, data).unwrap();

        // Flask mods should be in the player moddb
        let tabs = env
            .player
            .mod_db
            .tabulate("Life", None, ModFlags::NONE, KeywordFlags::NONE);
        assert!(
            tabs.iter().any(|t| t.source_category == "Item"
                && t.source_name.contains("Divine Life Flask")
                && t.value.as_f64().abs() >= 50.0),
            "Should find a +50 Life mod from flask Item source, tabs: {:?}",
            tabs
        );

        // UsingFlask condition should be set
        assert_eq!(
            env.player.mod_db.conditions.get("UsingFlask"),
            Some(&true),
            "UsingFlask condition should be true"
        );
    }

    #[test]
    fn flask_mods_not_added_when_not_using() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1">
    <Item id="1">
Rarity: MAGIC
Seething Divine Life Flask of Staunching
Divine Life Flask
Implicits: 0
+50 to maximum Life
    </Item>
    <ItemSet id="1">
      <Slot name="Flask 1" itemId="1"/>
    </ItemSet>
  </Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let data = make_default_data();
        let env = init_env(&build, data).unwrap();

        // Flask mods should NOT be in the player moddb
        let tabs = env
            .player
            .mod_db
            .tabulate("Life", None, ModFlags::NONE, KeywordFlags::NONE);
        assert!(
            !tabs.iter().any(|t| t.source_category == "Item"
                && t.source_name.contains("Divine Life Flask")),
            "Flask mods should NOT be in player moddb when not using flask"
        );

        // UsingFlask condition should not be set
        assert_ne!(
            env.player.mod_db.conditions.get("UsingFlask"),
            Some(&true),
            "UsingFlask condition should NOT be true"
        );
    }
}

/// Process equipped items: parse their mods into the player ModDb and extract weapon data.
/// Mirrors the item slot processing in CalcSetup.lua.
fn add_item_mods(build: &Build, env: &mut CalcEnv) {
    // Get the active item set
    let item_set = match build.item_sets.get(build.active_item_set) {
        Some(set) => set,
        None => return,
    };

    for (slot_name, &item_id) in &item_set.slots {
        let slot = match ItemSlot::from_str(slot_name) {
            Some(s) => s,
            None => continue,
        };

        // Skip flask and jewel slots (handled separately in later tasks)
        if slot.is_flask() || slot.is_jewel() {
            continue;
        }

        let item = match build.items.get(&item_id) {
            Some(i) => i,
            None => continue,
        };

        // Resolve base stats into a local copy so we don't mutate build
        let mut resolved_item = item.clone();
        crate::build::item_resolver::resolve_item_base(&mut resolved_item, &env.data.bases);

        let source = ModSource::new("Item", &resolved_item.base_type);

        // Parse and add all mod categories
        let mod_lines = resolved_item
            .implicits
            .iter()
            .chain(resolved_item.explicits.iter())
            .chain(resolved_item.crafted_mods.iter())
            .chain(resolved_item.enchant_mods.iter());

        for line in mod_lines {
            let mods = crate::build::mod_parser::parse_mod(line, source.clone());
            for m in mods {
                env.player.mod_db.add(m);
            }
        }

        // Add base armour/evasion/ES/block from resolved armour data
        if let Some(ref ad) = resolved_item.armour_data {
            if ad.armour > 0.0 {
                env.player
                    .mod_db
                    .add(Mod::new_base("Armour", ad.armour, source.clone()));
            }
            if ad.evasion > 0.0 {
                env.player
                    .mod_db
                    .add(Mod::new_base("Evasion", ad.evasion, source.clone()));
            }
            if ad.energy_shield > 0.0 {
                env.player.mod_db.add(Mod::new_base(
                    "EnergyShield",
                    ad.energy_shield,
                    source.clone(),
                ));
            }
            if ad.block > 0.0 {
                env.player
                    .mod_db
                    .add(Mod::new_base("ShieldBlockChance", ad.block, source.clone()));
            }
        }

        // Task 8: Extract weapon data from weapon slots
        match slot {
            ItemSlot::Weapon1 => {
                if let Some(ref wd) = resolved_item.weapon_data {
                    env.player.weapon_data1 = Some(wd.clone());
                }
            }
            ItemSlot::Weapon2 => {
                if let Some(ref wd) = resolved_item.weapon_data {
                    env.player.weapon_data2 = Some(wd.clone());
                }
                // Check if this is a shield (item_type contains "Shield")
                if resolved_item.item_type.contains("Shield") {
                    env.player.has_shield = true;
                }
            }
            _ => {}
        }

        // ── SETUP-11: Item condition & multiplier tracking ────────────────────
        // Mirrors CalcSetup.lua lines 1132-1203.

        // Lines 1132-1135: Class-restriction condition
        // e.g. "Requires Class Scion" → conditions["SolarisLorica"] = true
        if let Some(ref _restriction) = resolved_item.class_restriction {
            let key = resolved_item.name.replace(' ', "");
            env.player.mod_db.set_condition(&key, true);
        }

        // Lines 1136-1203: Item counts (skipped for Jewel/Flask/Tincture/Graft)
        // Jewels and flasks are skipped by the slot check above; also skip Tincture/Graft types.
        if resolved_item.item_type != "Tincture" && resolved_item.item_type != "Graft" {
            // Lines 1138-1151: Rarity multiplier
            let rarity_key = match resolved_item.rarity {
                crate::build::types::ItemRarity::Unique
                | crate::build::types::ItemRarity::Relic => {
                    if resolved_item.foulborn {
                        *env.player
                            .mod_db
                            .multipliers
                            .entry("FoulbornUniqueItem".to_string())
                            .or_insert(0.0) += 1.0;
                    }
                    "UniqueItem"
                }
                crate::build::types::ItemRarity::Rare => "RareItem",
                crate::build::types::ItemRarity::Magic => "MagicItem",
                crate::build::types::ItemRarity::Normal => "NormalItem",
            };
            *env.player
                .mod_db
                .multipliers
                .entry(rarity_key.to_string())
                .or_insert(0.0) += 1.0;

            // Set per-slot rarity condition: "{RarityKey}In{SlotName}"
            // e.g. "UniqueItemInWeapon 1" = true, "RareItemInBody Armour" = true
            env.player
                .mod_db
                .set_condition(&format!("{}In{}", rarity_key, slot_name), true);

            // Lines 1153-1159: Influence multipliers (and their Non* counterparts)
            let influences = [
                ("CorruptedItem", resolved_item.corrupted),
                ("ShaperItem", resolved_item.influence.shaper),
                ("ElderItem", resolved_item.influence.elder),
                ("WarlordItem", resolved_item.influence.warlord),
                ("HunterItem", resolved_item.influence.hunter),
                ("CrusaderItem", resolved_item.influence.crusader),
                ("RedeemerItem", resolved_item.influence.redeemer),
            ];
            for (mult_key, has_it) in &influences {
                if *has_it {
                    *env.player
                        .mod_db
                        .multipliers
                        .entry(mult_key.to_string())
                        .or_insert(0.0) += 1.0;
                } else {
                    let non_key = format!("Non{}", mult_key);
                    *env.player.mod_db.multipliers.entry(non_key).or_insert(0.0) += 1.0;
                }
            }

            // Lines 1160-1162: ShaperOrElderItem
            if resolved_item.influence.shaper || resolved_item.influence.elder {
                *env.player
                    .mod_db
                    .multipliers
                    .entry("ShaperOrElderItem".to_string())
                    .or_insert(0.0) += 1.0;
            }

            // Line 1163: Item type multiplier
            // Lua: item.type:gsub(" ", ""):gsub(".+Handed", "").."Item"
            // Strips spaces then strips any prefix ending in "Handed"
            // e.g. "Two Handed Sword" → "TwoHandedSword" → "Sword" → "SwordItem"
            // e.g. "One Handed Axe" → "OneHandedAxe" → "Axe" → "AxeItem"
            let type_no_spaces = resolved_item.item_type.replace(' ', "");
            let type_key = if let Some(after) = type_no_spaces.strip_prefix("TwoHanded") {
                after.to_string()
            } else if let Some(after) = type_no_spaces.strip_prefix("OneHanded") {
                after.to_string()
            } else {
                type_no_spaces
            };
            let item_type_mult_key = format!("{}Item", type_key);
            *env.player
                .mod_db
                .multipliers
                .entry(item_type_mult_key)
                .or_insert(0.0) += 1.0;

            // Lines 1165-1168: Ring base-name multiplier (for Breachlord ring interactions)
            // e.g. "Cryonic Ring" → "CryonicRingEquipped" += 1
            if resolved_item.item_type == "Ring" {
                let ring_key = format!("{}Equipped", resolved_item.base_type.replace(' ', ""));
                *env.player.mod_db.multipliers.entry(ring_key).or_insert(0.0) += 1.0;
            }

            // Lines 1169-1202: Socket counting
            // Count enabled gems in socket groups assigned to this slot
            let active_skill_set = build.skill_sets.get(build.active_skill_set);
            let socketed_gems = active_skill_set
                .map(|ss| {
                    ss.skills
                        .iter()
                        .filter(|s| s.slot == *slot_name && s.enabled)
                        .flat_map(|s| s.gems.iter())
                        .filter(|g| g.enabled && !g.skill_id.is_empty())
                        .count()
                })
                .unwrap_or(0) as u32;

            // Flatten socket groups to individual sockets (1-based index like Lua's ipairs)
            let flat_sockets: Vec<char> = resolved_item
                .sockets
                .iter()
                .flat_map(|g| g.colors.iter().copied())
                .collect();

            let mut slot_gem_socket_count: u32 = 0;
            let mut slot_empty_r: u32 = 0;
            let mut slot_empty_g: u32 = 0;
            let mut slot_empty_b: u32 = 0;
            let mut slot_empty_w: u32 = 0;

            for (i, color) in flat_sockets.iter().enumerate() {
                let idx = (i + 1) as u32; // 1-based index (mirrors Lua's ipairs)
                match color {
                    'R' | 'G' | 'B' | 'W' => {
                        slot_gem_socket_count += 1;
                        // Sockets beyond the gem count are "empty"
                        if idx > socketed_gems {
                            match color {
                                'R' => slot_empty_r += 1,
                                'G' => slot_empty_g += 1,
                                'B' => slot_empty_b += 1,
                                'W' => slot_empty_w += 1,
                                _ => {}
                            }
                        }
                    }
                    _ => {} // 'A' (abyss) sockets are ignored
                }
            }

            // SocketedGemsIn{SlotName}: capped at actual socket count
            let sg_key = format!("SocketedGemsIn{}", slot_name);
            *env.player.mod_db.multipliers.entry(sg_key).or_insert(0.0) +=
                slot_gem_socket_count.min(socketed_gems) as f64;

            // Accumulate empty socket counts across all items
            *env.player
                .mod_db
                .multipliers
                .entry("EmptyRedSocketsInAnySlot".to_string())
                .or_insert(0.0) += slot_empty_r as f64;
            *env.player
                .mod_db
                .multipliers
                .entry("EmptyGreenSocketsInAnySlot".to_string())
                .or_insert(0.0) += slot_empty_g as f64;
            *env.player
                .mod_db
                .multipliers
                .entry("EmptyBlueSocketsInAnySlot".to_string())
                .or_insert(0.0) += slot_empty_b as f64;
            *env.player
                .mod_db
                .multipliers
                .entry("EmptyWhiteSocketsInAnySlot".to_string())
                .or_insert(0.0) += slot_empty_w as f64;
        }
        // ── End SETUP-11 ──────────────────────────────────────────────────────
    }

    // After processing all slots, determine dual-wield status:
    // Dual wield = weapon in slot 1 AND weapon in slot 2 (not a shield)
    if env.player.weapon_data1.is_some()
        && env.player.weapon_data2.is_some()
        && !env.player.has_shield
    {
        env.player.dual_wield = true;
    }

    // ── SETUP-11 lines 1207-1210: Config override for empty sockets ───────────
    // After all items are processed, allow the config tab to override computed
    // empty socket counts.
    let socket_overrides = [
        ("overrideEmptyRedSockets", "EmptyRedSocketsInAnySlot"),
        ("overrideEmptyGreenSockets", "EmptyGreenSocketsInAnySlot"),
        ("overrideEmptyBlueSockets", "EmptyBlueSocketsInAnySlot"),
        ("overrideEmptyWhiteSockets", "EmptyWhiteSocketsInAnySlot"),
    ];
    for (cfg_key, mult_key) in &socket_overrides {
        if let Some(&v) = build.config.numbers.get(*cfg_key) {
            env.player.mod_db.set_multiplier(mult_key, v);
        }
    }
}

/// Initialize the enemy ModDb with level, base resistances, life, and config overrides.
/// Mirrors the enemy initialization in CalcSetup.lua.
fn init_enemy_db(build: &Build, db: &mut ModDb, data: &GameData) {
    let level = build.level as f64;
    let src = ModSource::new("Base", "enemy defaults");

    // Set enemy level
    db.add(Mod::new_base("Level", level, src.clone()));

    // Base resistances = 0
    db.add(Mod::new_base("FireResist", 0.0, src.clone()));
    db.add(Mod::new_base("ColdResist", 0.0, src.clone()));
    db.add(Mod::new_base("LightningResist", 0.0, src.clone()));
    db.add(Mod::new_base("ChaosResist", 0.0, src.clone()));

    // Physical damage reduction = 0
    db.add(Mod::new_base("PhysicalDamageReduction", 0.0, src.clone()));

    // Enemy life from monster_life_table (0-based index: level 1 → index 0)
    let level_idx = (build.level as usize).saturating_sub(1);
    if let Some(&life) = data.misc.monster_life_table.get(level_idx) {
        db.add(Mod::new_base("Life", life as f64, src.clone()));
    }

    // Config overrides: keys starting with "enemy" set enemy stats
    // e.g. "enemyFireResist" → "FireResist", "enemyLevel" → "Level"
    let config_src = ModSource::new("Config", "enemy override");
    for (key, &val) in &build.config.numbers {
        if let Some(stat_name) = key.strip_prefix("enemy") {
            if !stat_name.is_empty() {
                // First character is already uppercase in camelCase (e.g. "enemyFireResist" → "FireResist")
                db.add(Mod::new_base(stat_name, val, config_src.clone()));
            }
        }
    }
}

/// Process jewel items: parse their mods into the player ModDb.
/// Mirrors the jewel slot processing in CalcSetup.lua.
fn add_jewel_mods(build: &Build, env: &mut CalcEnv) {
    let item_set = match build.item_sets.get(build.active_item_set) {
        Some(set) => set,
        None => return,
    };

    for (slot_name, &item_id) in &item_set.slots {
        let slot = match ItemSlot::from_str(slot_name) {
            Some(s) => s,
            None => continue,
        };

        if !slot.is_jewel() {
            continue;
        }

        let item = match build.items.get(&item_id) {
            Some(i) => i,
            None => continue,
        };

        let source = ModSource::new("Item", format!("{} ({})", item.base_type, slot_name));

        let mod_lines = item
            .implicits
            .iter()
            .chain(item.explicits.iter())
            .chain(item.crafted_mods.iter());

        for line in mod_lines {
            let mods = crate::build::mod_parser::parse_mod(line, source.clone());
            for m in mods {
                env.player.mod_db.add(m);
            }
        }
    }
}

/// Process flask items: parse their mods into the player ModDb when flasks are active.
/// Mirrors the flask slot processing in CalcSetup.lua.
fn add_flask_mods(build: &Build, env: &mut CalcEnv) {
    // Only process flasks if conditionUsingFlask is true in config
    let using_flask = build
        .config
        .booleans
        .get("conditionUsingFlask")
        .copied()
        .unwrap_or(false);

    if !using_flask {
        return;
    }

    // Set the UsingFlask condition on player moddb
    env.player.mod_db.set_condition("UsingFlask", true);

    let item_set = match build.item_sets.get(build.active_item_set) {
        Some(set) => set,
        None => return,
    };

    for (slot_name, &item_id) in &item_set.slots {
        let slot = match ItemSlot::from_str(slot_name) {
            Some(s) => s,
            None => continue,
        };

        if !slot.is_flask() {
            continue;
        }

        let item = match build.items.get(&item_id) {
            Some(i) => i,
            None => continue,
        };

        let source = ModSource::new("Item", format!("{} ({})", item.base_type, slot_name));

        let mod_lines = item
            .implicits
            .iter()
            .chain(item.explicits.iter())
            .chain(item.crafted_mods.iter());

        for line in mod_lines {
            let mods = crate::build::mod_parser::parse_mod(line, source.clone());
            for m in mods {
                env.player.mod_db.add(m);
            }
        }
    }
}

fn add_config_conditions(build: &Build, db: &mut ModDb) {
    // Mirror POB's config tab: boolean inputs set conditions, number inputs set multipliers.
    for (name, &val) in &build.config.booleans {
        if val {
            // Config booleans that start with "condition" set a condition flag
            if let Some(cond_name) = name.strip_prefix("condition") {
                // Convert camelCase to TitleCase: "conditionFullLife" → "FullLife"
                let cond = cond_name[..1].to_uppercase() + &cond_name[1..];
                db.set_condition(&cond, true);
            }
            // "use*" booleans → set condition with TitleCase name
            // e.g. "useEnduranceCharges" → "UseEnduranceCharges"
            else if let Some(rest) = name.strip_prefix("use") {
                if !rest.is_empty() {
                    let first_upper = rest[..1].to_uppercase();
                    let cond = format!("Use{}{}", first_upper, &rest[1..]);
                    db.set_condition(&cond, true);
                }
            }
            // "buff*" booleans → map to the actual game condition
            // POB convention: buffFortification → Fortified, buffOnslaught → Onslaught, etc.
            else if let Some(rest) = name.strip_prefix("buff") {
                if !rest.is_empty() {
                    let cond = match name.as_str() {
                        "buffFortification" => "Fortified",
                        "buffOnslaught" => "Onslaught",
                        "buffTailwind" => "Tailwind",
                        "buffElusive" => "Elusive",
                        "buffUnholyMight" => "UnholyMight",
                        "buffPhasing" => "Phasing",
                        "buffAdrenaline" => "Adrenaline",
                        _ => {
                            // Generic: strip "buff" prefix, keep rest as condition name
                            // e.g. "buffSomething" → "Something"
                            rest
                        }
                    };
                    db.set_condition(cond, true);
                }
            }
        }
    }
    for (name, &val) in &build.config.numbers {
        if let Some(mult_name) = name.strip_prefix("multiplier") {
            db.set_multiplier(mult_name, val);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cluster Jewel Subgraph Generation (SETUP-05)
// ─────────────────────────────────────────────────────────────────────────────

/// Parsed info extracted from a single cluster jewel item's mod lines.
#[derive(Debug, Default)]
struct ClusterJewelInfo {
    /// Number of passive skills (node count), clamped to [min, max] for the size.
    node_count: Option<u32>,
    /// Number of jewel socket slots on this cluster jewel.
    socket_count: u32,
    /// Skill tag ID for the small passives (e.g. "affliction_cold_damage").
    skill_id: Option<String>,
    /// Names of notables on this jewel (e.g. "Blanketed Snow").
    notable_names: Vec<String>,
    /// Additional mod lines added to small passive nodes via enchant/anoint.
    /// These are the "Added Small Passive Skills also grant: ..." lines (excluding skill enchant).
    added_mods: Vec<String>,
    /// Increased effect of small passive skills (e.g. 10 for "10% increased effect").
    inc_effect: Option<f64>,
}

/// Parse a cluster jewel item's mods to extract ClusterJewelInfo.
/// Returns None if the item is not a valid cluster jewel.
fn parse_cluster_jewel_info(
    item: &crate::build::types::Item,
    enchant_to_skill: &std::collections::HashMap<&'static str, &'static str>,
) -> Option<ClusterJewelInfo> {
    use crate::data::cluster_jewels::cluster_size_for_base_type;

    // Only process recognised cluster jewel base types
    let size_def = cluster_size_for_base_type(&item.base_type)?;

    let mut info = ClusterJewelInfo::default();

    // Collect all mod lines (implicits + explicits + crafted + enchant)
    let all_lines = item
        .implicits
        .iter()
        .chain(item.explicits.iter())
        .chain(item.crafted_mods.iter())
        .chain(item.enchant_mods.iter());

    for line in all_lines {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();

        // "Adds N Passive Skills" → node_count
        if let Some(caps) = RE_CLUSTER_NODE_COUNT.captures(trimmed) {
            if let Ok(n) = caps[1].parse::<u32>() {
                let clamped = n.clamp(size_def.min_nodes, size_def.max_nodes);
                info.node_count = Some(clamped);
            }
            continue;
        }

        // "N Added Passive Skills are Jewel Sockets" → socket_count
        if let Some(caps) = RE_CLUSTER_SOCKET_COUNT.captures(trimmed) {
            if let Ok(n) = caps[1].parse::<u32>() {
                info.socket_count = n;
            }
            continue;
        }

        // "Added Passive Skill is a Jewel Socket" → socket_count = 1
        if RE_CLUSTER_SOCKET_ONE.is_match(trimmed) {
            info.socket_count = 1;
            continue;
        }

        // "1 Added Passive Skill is {Notable Name}" → notable_names
        // But NOT if the line matches the skill enchant (e.g. "... is a Jewel Socket" is handled above)
        if let Some(caps) = RE_CLUSTER_NOTABLE.captures(trimmed) {
            let name = caps[1].trim();
            // Skip if this looks like a skill description rather than a notable name
            // (notable names from the sort order table are proper nouns)
            if !name.to_lowercase().contains("jewel socket") {
                info.notable_names.push(name.to_string());
            }
            continue;
        }

        // "Added Small Passive Skills grant: ..." → skill_id (primary enchant line)
        if RE_CLUSTER_SKILL.is_match(trimmed) {
            // Look up the full enchant text (lowercased) in the enchant→skill table
            let full_enchant = lower.as_str();
            if let Some(&skill_id) = enchant_to_skill.get(full_enchant) {
                info.skill_id = Some(skill_id.to_string());
            }
            // Do NOT add to added_mods — this is the skill enchant, not an extra mod
            continue;
        }

        // "Added Small Passive Skills also grant: ..." → added_mods (extra stat lines)
        if let Some(rest) = lower.strip_prefix("added small passive skills also grant: ") {
            // Strip the prefix and keep the actual mod text
            let mod_text = trimmed
                .get("Added Small Passive Skills also grant: ".len()..)
                .unwrap_or(trimmed);
            info.added_mods.push(mod_text.to_string());
            let _ = rest; // suppress unused variable warning
            continue;
        }

        // "Added Small Passive Skills have N% increased Effect" → inc_effect
        if let Some(caps) = RE_CLUSTER_INC_EFFECT.captures(trimmed) {
            if let Ok(n) = caps[1].parse::<f64>() {
                info.inc_effect = Some(n);
            }
            continue;
        }
    }

    Some(info)
}

/// Compute the base synthetic node ID for a cluster jewel at a given socket node.
///
/// ID encoding (mirrors PoB's BuildSubgraph):
/// - bit 16 (0x10000): signal bit
/// - bits 6-8: large socket's `expansionJewel.index`  (for Large cluster jewels)
/// - bits 9-15: medium socket's `expansionJewel.index` (for Medium cluster jewels)
/// - bits 4-5: size index of the JEWEL (0=Small 1=Medium 2=Large)
///
/// The `id` accumulates across recursion levels:
/// - Large socket (size=2): id += index << 6
/// - Medium socket (size=1): id += index << 9 (added on top of parent's id)
///
/// For tree-level Medium sockets nested inside a Large cluster,
/// we need both the parent Large socket's contribution AND this socket's index.
fn compute_subgraph_base_id(
    socket_node_id: u32,
    jewel_size_index: u32,
    tree: &crate::passive_tree::PassiveTree,
    jewel_sockets: &[(u32, u32)], // (node_id, item_id) all sockets in build
) -> u32 {
    let mut id: u32 = 0x10000;

    if let Some(socket_node) = tree.nodes.get(&socket_node_id) {
        if let Some(ej) = &socket_node.expansion_jewel {
            match ej.size {
                2 => {
                    // This is a Large socket: id += index << 6
                    id += ej.index << 6;
                }
                1 => {
                    // This is a Medium socket.
                    // We need to add the parent Large cluster's contribution too.
                    // The parent Large socket's expansionJewel.index contributes << 6.
                    // Then this socket's index contributes << 9.
                    //
                    // Find the parent Large socket by looking at other sockets in the build
                    // that are Large and have this socket in their generated subgraph.
                    // The parent relationship is encoded in the tree data.
                    // For simplicity, we look at all Large sockets and find which one
                    // is the parent of this Medium socket.
                    let parent_large_id =
                        find_parent_large_socket(socket_node_id, tree, jewel_sockets);
                    if let Some(parent_node_id) = parent_large_id {
                        if let Some(parent_node) = tree.nodes.get(&parent_node_id) {
                            if let Some(parent_ej) = &parent_node.expansion_jewel {
                                id += parent_ej.index << 6;
                            }
                        }
                    }
                    id += ej.index << 9;
                }
                0 => {
                    // Small socket: no additional id contribution
                }
                _ => {}
            }
        }
    }

    // Add the jewel's size index in bits 4-5
    id += jewel_size_index << 4;
    id
}

/// Find the parent Large socket node ID for a given Medium socket.
/// Uses the `parent` field from the node's `expansion_jewel` metadata.
fn find_parent_large_socket(
    medium_socket_node_id: u32,
    tree: &crate::passive_tree::PassiveTree,
    _jewel_sockets: &[(u32, u32)],
) -> Option<u32> {
    let node = tree.nodes.get(&medium_socket_node_id)?;
    let ej = node.expansion_jewel.as_ref()?;
    ej.parent
}

/// Generate the synthetic node IDs for a cluster jewel's ring.
///
/// Returns a list of (node_id, node_role) tuples where node_role is one of:
/// "Notable", "Socket", or "Small".
///
/// The IDs match PoB's BuildSubgraph ID encoding exactly, so they can be
/// looked up in `passive_spec.allocated_nodes`.
#[derive(Debug)]
enum ClusterNodeRole {
    /// A notable passive node; `name` is the notable's display name.
    Notable(String),
    /// A jewel socket sub-node (where a Medium/Small cluster jewel can be socketed).
    Socket,
    /// A small fill passive node.
    Small,
}

fn generate_cluster_node_ids(
    base_id: u32,
    size_def: &crate::data::cluster_jewels::ClusterJewelSize,
    info: &ClusterJewelInfo,
    notable_sort_order: &std::collections::HashMap<&'static str, u32>,
) -> Vec<(u32, ClusterNodeRole)> {
    use std::collections::HashSet;

    let node_count = match info.node_count {
        Some(n) => n,
        None => return Vec::new(),
    };

    let socket_count = info.socket_count;
    let notable_count = info.notable_names.len() as u32;
    let small_count = node_count
        .saturating_sub(socket_count)
        .saturating_sub(notable_count);

    let mut used_indicies: HashSet<u32> = HashSet::new();
    let mut result: Vec<(u32, ClusterNodeRole)> = Vec::new();

    // ── Step 1: Place Socket nodes ────────────────────────────────────────
    if size_def.size_name == "Large Cluster Jewel" && socket_count == 1 {
        // Large single socket always at index 6
        let idx = 6u32;
        used_indicies.insert(idx);
        result.push((base_id + idx, ClusterNodeRole::Socket));
    } else {
        let get_jewels = [0u32, 2, 1]; // which physical socket position to use
        for i in 0..socket_count as usize {
            if i >= size_def.socket_indicies.len() {
                break;
            }
            let idx = size_def.socket_indicies[i];
            if !used_indicies.contains(&idx) {
                used_indicies.insert(idx);
                result.push((base_id + idx, ClusterNodeRole::Socket));
            }
            let _ = get_jewels; // used implicitly via i
        }
    }

    // ── Step 2: Place Notable nodes ───────────────────────────────────────
    // Sort notables by notable_sort_order to determine placement order
    let mut sorted_notables: Vec<(u32, &str)> = info
        .notable_names
        .iter()
        .map(|n| {
            let order = notable_sort_order
                .get(n.as_str())
                .copied()
                .unwrap_or(u32::MAX);
            (order, n.as_str())
        })
        .collect();
    sorted_notables.sort_by_key(|&(order, _)| order);

    // Find available notable indices, skipping any already-used indices
    let mut notable_index_list: Vec<u32> = Vec::new();
    let notable_count_needed = sorted_notables.len() as u32;

    for &candidate_idx in size_def.notable_indicies {
        if notable_index_list.len() as u32 >= notable_count_needed {
            break;
        }
        // Apply Medium cluster special rules
        let idx = if size_def.size_name == "Medium Cluster Jewel" {
            apply_medium_notable_index_rules(
                candidate_idx,
                socket_count,
                notable_count_needed,
                node_count,
            )
        } else {
            candidate_idx
        };
        if !used_indicies.contains(&idx) {
            notable_index_list.push(idx);
            used_indicies.insert(idx);
        }
    }
    // Sort notable index list ascending (so ring order is consistent)
    notable_index_list.sort_unstable();

    for (i, &(_, notable_name)) in sorted_notables.iter().enumerate() {
        if let Some(&idx) = notable_index_list.get(i) {
            result.push((
                base_id + idx,
                ClusterNodeRole::Notable(notable_name.to_string()),
            ));
        }
    }

    // ── Step 3: Place Small fill nodes ────────────────────────────────────
    let mut small_index_list: Vec<u32> = Vec::new();

    for &candidate_idx in size_def.small_indicies {
        if small_index_list.len() as u32 >= small_count {
            break;
        }
        // Apply Medium cluster special rules for small indices
        let idx = if size_def.size_name == "Medium Cluster Jewel" {
            apply_medium_small_index_rules(candidate_idx, node_count)
        } else {
            candidate_idx
        };
        if !used_indicies.contains(&idx) {
            small_index_list.push(idx);
            used_indicies.insert(idx);
        }
    }

    for &idx in &small_index_list {
        result.push((base_id + idx, ClusterNodeRole::Small));
    }

    result
}

/// Apply Medium cluster special rules for notable indices.
/// Matches PoB's inline corrections in BuildSubgraph (lines 1432-1444).
fn apply_medium_notable_index_rules(
    idx: u32,
    socket_count: u32,
    notable_count: u32,
    node_count: u32,
) -> u32 {
    if socket_count == 0 && notable_count == 2 {
        if idx == 6 {
            return 4;
        } else if idx == 10 {
            return 8;
        }
    } else if node_count == 4 {
        if idx == 10 {
            return 9;
        } else if idx == 2 {
            return 3;
        }
    }
    idx
}

/// Apply Medium cluster special rules for small indices.
/// Matches PoB's inline corrections in BuildSubgraph (lines 1467-1474).
fn apply_medium_small_index_rules(idx: u32, node_count: u32) -> u32 {
    if node_count == 5 && idx == 4 {
        return 3;
    } else if node_count == 4 {
        if idx == 8 {
            return 9;
        } else if idx == 4 {
            return 3;
        }
    }
    idx
}

/// Determine the socket size from the tree node name.
/// Returns the `sizeIndex` for the socket (0=Small, 1=Medium, 2=Large) or None.
fn socket_size_index_from_node_name(name: &str) -> Option<u32> {
    match name {
        "Large Jewel Socket" => Some(2),
        "Medium Jewel Socket" => Some(1),
        "Small Jewel Socket" => Some(0),
        _ => None, // "Basic Jewel Socket" accepts regular jewels, not cluster
    }
}

/// Process all cluster jewels in `build.passive_spec.jewels` and add their
/// synthesised passive node mods to `env.player.mod_db`.
///
/// This mirrors PoB's `PassiveSpecClass:BuildClusterJewelGraphs()` and
/// `PassiveSpecClass:BuildSubgraph()`, but only for the parity-relevant
/// subset: applying mods for allocated synthetic nodes.
fn add_cluster_jewel_mods(build: &Build, env: &mut CalcEnv) {
    use crate::data::cluster_jewels::{
        build_enchant_to_skill_map, build_notable_sort_order, cluster_size_for_base_type,
    };
    use crate::passive_tree::NodeType;

    let enchant_to_skill = build_enchant_to_skill_map();
    let notable_sort_order = build_notable_sort_order();

    // Get the passive tree to look up socket node types and cluster notable stats.
    // Clone the tree reference to avoid lifetime issues when mutably borrowing env later.
    let tree = env
        .data
        .tree_for_version(&build.passive_spec.tree_version)
        .clone();

    // Collect all cluster jewel sockets from passive_spec.jewels
    // sorted by node ID (deterministic processing order)
    let mut jewel_sockets: Vec<(u32, u32)> = build
        .passive_spec
        .jewels
        .iter()
        .filter_map(|(&node_id, &item_id)| {
            if item_id == 0 {
                return None;
            }
            // Check that the tree node exists and is a cluster jewel socket
            let node = tree.nodes.get(&node_id)?;
            if node.node_type != NodeType::JewelSocket {
                return None;
            }
            // Only process if the socketed item is a cluster jewel
            let item = build.items.get(&item_id)?;
            let _ = cluster_size_for_base_type(&item.base_type)?;
            Some((node_id, item_id))
        })
        .collect();
    jewel_sockets.sort_by_key(|&(node_id, _)| node_id);

    if jewel_sockets.is_empty() {
        return;
    }

    // Note: socket_sizes is no longer needed since we use tree.expansionJewel.index
    // directly in compute_subgraph_base_id

    // Process each cluster jewel socket
    for &(socket_node_id, item_id) in &jewel_sockets {
        let Some(socket_node) = tree.nodes.get(&socket_node_id) else {
            continue;
        };

        // Determine socket size from node name
        let socket_size = match socket_size_index_from_node_name(&socket_node.name) {
            Some(s) => s,
            None => continue, // Not a cluster jewel socket (e.g. "Basic Jewel Socket")
        };

        let Some(item) = build.items.get(&item_id) else {
            continue;
        };

        let Some(size_def) = cluster_size_for_base_type(&item.base_type) else {
            continue;
        };

        // Cluster jewel size must match or be smaller than socket size
        // (e.g. a Medium cluster jewel can go in a Medium or Large socket)
        if size_def.size_index > socket_size {
            continue;
        }

        let Some(info) = parse_cluster_jewel_info(item, &enchant_to_skill) else {
            continue;
        };

        // Validity check: a cluster jewel is buildable if it has a node count
        if info.node_count.is_none() {
            continue;
        }

        // Compute the base subgraph ID for this socket
        let base_id =
            compute_subgraph_base_id(socket_node_id, size_def.size_index, &tree, &jewel_sockets);

        // Generate the synthetic node IDs for this cluster jewel
        let synthetic_nodes =
            generate_cluster_node_ids(base_id, size_def, &info, &notable_sort_order);

        // Apply mods for allocated synthetic nodes
        add_synthetic_node_mods(
            build,
            &mut env.player.mod_db,
            &synthetic_nodes,
            &info,
            &tree,
            size_def,
        );
    }
}

/// Look up the stat strings for a cluster jewel notable by name from the passive tree.
/// Returns the stats from the first matching tree node with that name.
fn get_notable_stats<'a>(
    notable_name: &str,
    tree: &'a crate::passive_tree::PassiveTree,
) -> Option<&'a Vec<String>> {
    tree.nodes
        .values()
        .find(|n| n.name == notable_name)
        .map(|n| &n.stats)
}

/// Add mods to the player ModDb for allocated synthetic cluster jewel nodes.
fn add_synthetic_node_mods(
    build: &Build,
    db: &mut ModDb,
    synthetic_nodes: &[(u32, ClusterNodeRole)],
    info: &ClusterJewelInfo,
    tree: &crate::passive_tree::PassiveTree,
    size_def: &crate::data::cluster_jewels::ClusterJewelSize,
) {
    for (node_id, role) in synthetic_nodes {
        if !build.passive_spec.allocated_nodes.contains(node_id) {
            continue;
        }

        let source_name = match role {
            ClusterNodeRole::Notable(name) => format!("{} (cluster)", name),
            ClusterNodeRole::Socket => continue, // Socket nodes have no mods themselves
            ClusterNodeRole::Small => "Small Passive (cluster)".to_string(),
        };
        let source = ModSource::new("Passive", &source_name);

        match role {
            ClusterNodeRole::Notable(notable_name) => {
                // Look up stats for this notable from the tree's cluster node map
                if let Some(stats) = get_notable_stats(notable_name, tree) {
                    for stat in stats {
                        let mods = crate::build::mod_parser::parse_mod(stat, source.clone());
                        for m in mods {
                            db.add(m);
                        }
                    }
                }
            }
            ClusterNodeRole::Small => {
                // Small passive nodes get their stats from:
                // 1. The skill's stat lines (from the primary enchant, via tree node lookup)
                // 2. Any "Added Small Passive Skills also grant: ..." lines on the item

                // For the skill stats, look up in the tree
                let skill_node_stats: Vec<String> = if let Some(skill_id) = &info.skill_id {
                    find_skill_stats_from_tree(skill_id, tree)
                } else {
                    Vec::new()
                };

                for stat in &skill_node_stats {
                    let mods = crate::build::mod_parser::parse_mod(stat, source.clone());
                    for m in mods {
                        db.add(m);
                    }
                }

                // Add the "also grant" mods
                for added_mod in &info.added_mods {
                    let mods = crate::build::mod_parser::parse_mod(added_mod, source.clone());
                    for m in mods {
                        db.add(m);
                    }
                }

                let _ = size_def;
            }
            ClusterNodeRole::Socket => {} // Already handled above with continue
        }
    }
}

/// Find the stat strings for a skill's small passive from the passive tree.
/// The tree contains "cluster passive" nodes whose names match the skill's human-readable name.
/// We need to find the skill stats for a given skill tag ID.
fn find_skill_stats_from_tree(
    skill_id: &str,
    tree: &crate::passive_tree::PassiveTree,
) -> Vec<String> {
    use crate::data::cluster_jewels::ENCHANT_TO_SKILL_ENTRIES;

    // Find the human-readable skill name by reverse-looking the ENCHANT_TO_SKILL_ENTRIES
    // to get the stat strings. However, the stats come from the tree nodes.
    //
    // The tree has nodes like: { name: "Cold Damage", stats: ["12% increased Cold Damage"] }
    // These are the cluster passive template nodes.
    //
    // We match by skill_id: for each enchant entry that maps to this skill_id,
    // the stat string is the text after "Added Small Passive Skills grant: ".
    // We then look for tree nodes with matching stats.
    //
    // Simpler approach: the enchant text IS the stat text (minus the prefix).
    // "Added Small Passive Skills grant: 12% increased Cold Damage" → "12% increased Cold Damage"
    let mut stats = Vec::new();

    for &(enchant, sid) in ENCHANT_TO_SKILL_ENTRIES {
        if sid == skill_id {
            // Extract the stat text from the enchant line
            let prefix = "added small passive skills grant: ";
            if let Some(stat) = enchant.strip_prefix(prefix) {
                // The stat text is lowercase from our table; we need the proper case.
                // Look for a tree node whose stats (lowercased) match.
                for node in tree.nodes.values() {
                    for node_stat in &node.stats {
                        if node_stat.to_lowercase() == stat {
                            stats.push(node_stat.clone());
                        }
                    }
                }
                // If not found in tree, try to find by node name matching skill
                if stats.is_empty() {
                    // Fallback: use the enchant text as-is (but titlecased)
                    // This is less accurate but better than nothing
                }
            }
        }
    }

    stats
}
