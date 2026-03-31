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
use std::sync::Arc;

/// Build the CalcEnv for a build.
/// Mirrors calcs.initEnv() in CalcSetup.lua.
pub fn init_env(build: &Build, data: Arc<GameData>) -> Result<CalcEnv, CalcError> {
    let mut player_db = ModDb::new();
    let enemy_db = ModDb::new();

    // Add base constants (mirrors calcs.initModDB())
    add_base_constants(&mut player_db, &data);

    // Add class base stats
    add_class_base_stats(build, &mut player_db, &data);

    // Add passive tree node mods
    add_passive_mods(build, &mut player_db, &data);

    // Add config conditions
    add_config_conditions(build, &mut player_db);

    // Create the env first so add_item_mods can populate weapon data on the Actor
    let mut env = CalcEnv::new(player_db, enemy_db, data);

    // Add item mods from equipped items and extract weapon data
    add_item_mods(build, &mut env);

    Ok(env)
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

fn add_passive_mods(build: &Build, db: &mut ModDb, data: &GameData) {
    for &node_id in &build.passive_spec.allocated_nodes {
        let Some(node) = data.passive_tree.nodes.get(&node_id) else {
            // Node not found in tree data — skip silently
            continue;
        };
        let source = ModSource::new("Passive", &node.name);
        for stat_text in &node.stats {
            let mods = crate::build::mod_parser::parse_mod(stat_text, source.clone());
            for m in mods {
                db.add(m);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    }

    // After processing all slots, determine dual-wield status:
    // Dual wield = weapon in slot 1 AND weapon in slot 2 (not a shield)
    if env.player.weapon_data1.is_some()
        && env.player.weapon_data2.is_some()
        && !env.player.has_shield
    {
        env.player.dual_wield = true;
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
        }
    }
    for (name, &val) in &build.config.numbers {
        if let Some(mult_name) = name.strip_prefix("multiplier") {
            db.set_multiplier(mult_name, val);
        }
    }
}
