use super::env::CalcEnv;
use crate::{
    build::Build,
    data::GameData,
    error::CalcError,
    mod_db::{
        types::{Mod, ModSource},
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

    Ok(CalcEnv::new(player_db, enemy_db, data))
}

fn add_base_constants(db: &mut ModDb, data: &GameData) {
    let gc = &data.misc.game_constants;
    let src = ModSource::new("Base", "game constants");

    let resist_max = gc
        .get("base_maximum_all_resistances_%")
        .copied()
        .unwrap_or(75.0);
    for name in &[
        "FireResistMax",
        "ColdResistMax",
        "LightningResistMax",
        "ChaosResistMax",
    ] {
        db.add(Mod::new_base(*name, resist_max, src.clone()));
    }

    let block_max = gc.get("maximum_block_%").copied().unwrap_or(75.0);
    db.add(Mod::new_base("BlockChanceMax", block_max, src.clone()));

    let power_max = gc.get("max_power_charges").copied().unwrap_or(3.0);
    db.add(Mod::new_base("PowerChargesMax", power_max, src.clone()));

    let frenzy_max = gc.get("max_frenzy_charges").copied().unwrap_or(3.0);
    db.add(Mod::new_base("FrenzyChargesMax", frenzy_max, src.clone()));

    let endurance_max = gc.get("max_endurance_charges").copied().unwrap_or(3.0);
    db.add(Mod::new_base(
        "EnduranceChargesMax",
        endurance_max,
        src.clone(),
    ));
}

fn add_class_base_stats(build: &Build, db: &mut ModDb, data: &GameData) {
    let src = ModSource::new("Base", format!("{} base stats", build.class_name));
    let _cc = &data.misc.character_constants;

    // Base life from class (Marauder=38+hp_per_level*90, etc.)
    // POB stores this in data.characterConstants. For now we add a flat base.
    // The exact class base life table is loaded from game constants in Phase 4 Task 4.
    let base_life = 38.0 + 12.0 * (build.level as f64); // simplified until full table loaded
    db.add(Mod::new_base("Life", base_life, src.clone()));

    let base_mana = 34.0 + 6.0 * (build.level as f64);
    db.add(Mod::new_base("Mana", base_mana, src.clone()));
}

fn add_passive_mods(build: &Build, db: &mut ModDb, data: &GameData) {
    for &node_id in &build.passive_spec.allocated_nodes {
        let Some(node) = data.passive_tree.nodes.get(&node_id) else {
            // Node not found in tree data — skip silently
            continue;
        };
        let source = ModSource::new("Passive", &node.name);
        for stat_text in &node.stats {
            let mods = crate::build::item_parser::parse_stat_text(stat_text, source.clone());
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
