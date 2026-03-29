use super::env::CalcEnv;
use crate::{
    build::{item_parser::parse_stat_text, Build},
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
    add_passive_mods(build, &mut player_db);

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

fn add_passive_mods(build: &Build, db: &mut ModDb) {
    // For each allocated passive node, parse its stat strings and add mods.
    // Node stat strings come from the PassiveTree loader.
    // At this stage we only have node IDs — the tree data is not yet linked in CalcEnv.
    // This is filled in once PassiveTree integration is complete (Task 4 below).
    // For now, this is a no-op stub.
    let _ = build;
    let _ = db;
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
