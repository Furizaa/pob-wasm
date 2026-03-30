use crate::lua_env::create_pob_lua_env;
use crate::types::{SkillGemData, SkillLevelData, StatEntry};
use mlua::prelude::*;
use std::collections::HashMap;
use std::path::Path;

/// Skill files to load from Data/Skills/
const SKILL_FILES: &[&str] = &[
    "act_str.lua",
    "act_dex.lua",
    "act_int.lua",
    "sup_str.lua",
    "sup_dex.lua",
    "sup_int.lua",
    "other.lua",
    "minion.lua",
    "glove.lua",
    "spectre.lua",
];

/// Map color integer to string name.
fn color_name(c: i64) -> &'static str {
    match c {
        1 => "strength",
        2 => "dexterity",
        3 => "intelligence",
        _ => "other",
    }
}

/// Build a reverse map from SkillType integer values back to their string names.
fn build_skill_type_reverse_map(lua: &Lua) -> mlua::Result<HashMap<i64, String>> {
    let skill_type: LuaTable = lua.globals().get("SkillType")?;
    let mut map = HashMap::new();
    for pair in skill_type.pairs::<String, LuaValue>() {
        let (k, v) = pair?;
        if k == "__next" {
            continue;
        }
        if let LuaValue::Integer(id) = v {
            map.insert(id, k);
        }
    }
    Ok(map)
}

/// Extract constant_stats or quality_stats from a Lua table.
///
/// `constantStats` is `{ { "stat_id", value }, ... }`
/// `qualityStats` is `{ Default = { { "stat_id", value }, ... } }`
fn extract_stat_entries(table: &LuaTable, key: &str) -> mlua::Result<Vec<StatEntry>> {
    let mut entries = Vec::new();
    let val: LuaValue = table.get(key)?;
    match val {
        LuaValue::Table(tbl) => {
            // Check if this is a qualityStats table with a "Default" key
            let default_val: LuaValue = tbl.get("Default")?;
            let items = match default_val {
                LuaValue::Table(default_tbl) => default_tbl,
                _ => tbl,
            };
            for pair in items.pairs::<LuaValue, LuaValue>() {
                let (_, v) = pair?;
                if let LuaValue::Table(entry) = v {
                    let stat_id: Option<String> = entry.get(1)?;
                    let value: Option<f64> = entry.get(2)?;
                    if let (Some(sid), Some(val)) = (stat_id, value) {
                        entries.push(StatEntry {
                            stat_id: sid,
                            value: val,
                        });
                    }
                }
            }
        }
        _ => {}
    }
    Ok(entries)
}

/// Extract the list of SkillType string names from a skill's `skillTypes` table.
/// The table has keys that are SkillType integer IDs with values of `true`.
fn extract_skill_types(
    skill_table: &LuaTable,
    reverse_map: &HashMap<i64, String>,
) -> mlua::Result<Vec<String>> {
    let val: LuaValue = skill_table.get("skillTypes")?;
    let mut types = Vec::new();
    if let LuaValue::Table(tbl) = val {
        for pair in tbl.pairs::<LuaValue, LuaValue>() {
            let (k, _) = pair?;
            match k {
                LuaValue::Integer(id) => {
                    if let Some(name) = reverse_map.get(&id) {
                        types.push(name.clone());
                    }
                }
                LuaValue::Number(n) => {
                    let id = n as i64;
                    if let Some(name) = reverse_map.get(&id) {
                        types.push(name.clone());
                    }
                }
                _ => {}
            }
        }
    }
    types.sort();
    Ok(types)
}

/// Extract requireSkillTypes / addSkillTypes / excludeSkillTypes arrays.
/// These are sequential arrays of SkillType integer values (not keyed with = true).
fn extract_skill_type_list(
    skill_table: &LuaTable,
    key: &str,
    reverse_map: &HashMap<i64, String>,
) -> mlua::Result<Vec<String>> {
    let val: LuaValue = skill_table.get(key)?;
    let mut types = Vec::new();
    if let LuaValue::Table(tbl) = val {
        for pair in tbl.pairs::<LuaValue, LuaValue>() {
            let (_, v) = pair?;
            match v {
                LuaValue::Integer(id) => {
                    if let Some(name) = reverse_map.get(&id) {
                        types.push(name.clone());
                    }
                }
                LuaValue::Number(n) => {
                    let id = n as i64;
                    if let Some(name) = reverse_map.get(&id) {
                        types.push(name.clone());
                    }
                }
                _ => {}
            }
        }
    }
    Ok(types)
}

/// Extract the `baseFlags` table (keys with value `true`).
fn extract_base_flags(skill_table: &LuaTable) -> mlua::Result<Vec<String>> {
    let val: LuaValue = skill_table.get("baseFlags")?;
    let mut flags = Vec::new();
    if let LuaValue::Table(tbl) = val {
        for pair in tbl.pairs::<String, LuaValue>() {
            let (k, v) = pair?;
            if let LuaValue::Boolean(true) = v {
                flags.push(k);
            }
        }
    }
    flags.sort();
    Ok(flags)
}

/// Extract the `stats` array (sequential string values).
fn extract_stats_array(skill_table: &LuaTable) -> mlua::Result<Vec<String>> {
    let val: LuaValue = skill_table.get("stats")?;
    let mut stats = Vec::new();
    if let LuaValue::Table(tbl) = val {
        let len = tbl.raw_len();
        for i in 1..=len {
            let s: Option<String> = tbl.get(i)?;
            if let Some(s) = s {
                stats.push(s);
            }
        }
    }
    Ok(stats)
}

/// Extract a single level entry from the `levels` table.
fn extract_level(
    level_key: u32,
    level_table: &LuaTable,
    num_stats: usize,
) -> mlua::Result<SkillLevelData> {
    // Positional values correspond to stats array indices
    let mut stat_values = Vec::new();
    for i in 1..=num_stats {
        let v: Option<f64> = level_table.get(i)?;
        stat_values.push(v.unwrap_or(0.0));
    }

    let crit_chance: f64 = level_table.get("critChance").unwrap_or(0.0);
    let damage_effectiveness: f64 = level_table.get("damageEffectiveness").unwrap_or(0.0);
    let attack_speed_mult: f64 = level_table.get("attackSpeedMultiplier").unwrap_or(0.0);
    let level_requirement: u32 = level_table.get("levelRequirement").unwrap_or(0);
    let stored_uses: u32 = level_table.get("storedUses").unwrap_or(0);
    let cooldown: f64 = level_table.get("cooldown").unwrap_or(0.0);
    let duration: f64 = level_table.get("duration").unwrap_or(0.0);
    let mana_multiplier: f64 = level_table.get("manaMultiplier").unwrap_or(0.0);

    // Cost table
    let mut mana_cost = 0.0;
    let mut life_cost = 0.0;
    let cost_val: LuaValue = level_table.get("cost")?;
    if let LuaValue::Table(cost_tbl) = cost_val {
        mana_cost = cost_tbl.get("Mana").unwrap_or(0.0);
        life_cost = cost_tbl.get("Life").unwrap_or(0.0);
    }

    Ok(SkillLevelData {
        level: level_key,
        level_requirement,
        stat_values,
        crit_chance,
        damage_effectiveness,
        attack_speed_mult,
        mana_cost,
        life_cost,
        mana_multiplier,
        stored_uses,
        cooldown,
        duration,
    })
}

/// Extract all level data from the `levels` table.
fn extract_levels(skill_table: &LuaTable, num_stats: usize) -> mlua::Result<Vec<SkillLevelData>> {
    let val: LuaValue = skill_table.get("levels")?;
    let mut levels = Vec::new();
    if let LuaValue::Table(tbl) = val {
        for pair in tbl.pairs::<LuaValue, LuaValue>() {
            let (k, v) = pair?;
            let level_key = match k {
                LuaValue::Integer(n) => n as u32,
                LuaValue::Number(n) => n as u32,
                _ => continue,
            };
            if let LuaValue::Table(level_tbl) = v {
                match extract_level(level_key, &level_tbl, num_stats) {
                    Ok(level_data) => levels.push(level_data),
                    Err(e) => {
                        eprintln!("  Warning: failed to extract level {}: {}", level_key, e);
                    }
                }
            }
        }
    }
    levels.sort_by_key(|l| l.level);
    Ok(levels)
}

/// Load Gems.lua and return a map from grantedEffectId -> display_name.
fn load_gem_metadata(lua: &Lua, pob_src: &str) -> mlua::Result<HashMap<String, String>> {
    let gems_path = format!("{}/Data/Gems.lua", pob_src);
    let gems_code = std::fs::read_to_string(&gems_path)
        .map_err(|e| mlua::Error::external(format!("Failed to read {}: {}", gems_path, e)))?;

    let gems_table: LuaTable = lua.load(&gems_code).eval()?;

    let mut metadata = HashMap::new();
    for pair in gems_table.pairs::<String, LuaValue>() {
        let (_key, value) = pair?;
        if let LuaValue::Table(gem) = value {
            let granted_effect_id: Option<String> = gem.get("grantedEffectId")?;
            let name: Option<String> = gem.get("name")?;
            if let (Some(gid), Some(n)) = (granted_effect_id, name) {
                metadata.insert(gid, n);
            }
        }
    }

    Ok(metadata)
}

/// Parse one skill definition into a SkillGemData.
fn parse_skill(
    skill_id: &str,
    skill_table: &LuaTable,
    reverse_map: &HashMap<i64, String>,
    gem_names: &HashMap<String, String>,
) -> mlua::Result<SkillGemData> {
    let name: String = skill_table.get("name").unwrap_or_default();
    let display_name = gem_names
        .get(skill_id)
        .cloned()
        .unwrap_or_else(|| name.clone());

    let is_support: bool = skill_table.get("support").unwrap_or(false);
    let color: Option<i64> = skill_table.get("color")?;
    let cast_time: f64 = skill_table.get("castTime").unwrap_or(0.0);
    let base_effectiveness: f64 = skill_table.get("baseEffectiveness").unwrap_or(0.0);
    let incremental_effectiveness: f64 = skill_table.get("incrementalEffectiveness").unwrap_or(0.0);

    let skill_types = extract_skill_types(skill_table, reverse_map)?;
    let base_flags = extract_base_flags(skill_table)?;
    let stats = extract_stats_array(skill_table)?;
    let constant_stats = extract_stat_entries(skill_table, "constantStats")?;
    let quality_stats = extract_stat_entries(skill_table, "qualityStats")?;

    let require_skill_types =
        extract_skill_type_list(skill_table, "requireSkillTypes", reverse_map)?;
    let add_skill_types = extract_skill_type_list(skill_table, "addSkillTypes", reverse_map)?;
    let exclude_skill_types =
        extract_skill_type_list(skill_table, "excludeSkillTypes", reverse_map)?;

    let levels = extract_levels(skill_table, stats.len())?;

    // Get mana_multiplier from level 20 if it exists
    let mana_multiplier_at_20 = levels
        .iter()
        .find(|l| l.level == 20)
        .map(|l| l.mana_multiplier)
        .unwrap_or(0.0);

    Ok(SkillGemData {
        id: skill_id.to_string(),
        display_name,
        is_support,
        color: color.map(|c| color_name(c).to_string()),
        skill_types,
        cast_time,
        base_effectiveness,
        incremental_effectiveness,
        base_flags,
        levels,
        mana_multiplier_at_20,
        require_skill_types,
        add_skill_types,
        exclude_skill_types,
        constant_stats,
        quality_stats,
        stats,
    })
}

pub fn extract(pob_src: &str, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let lua = create_pob_lua_env(pob_src)?;

    // Load gem metadata (display names from Gems.lua)
    println!("  Loading Gems.lua metadata...");
    let gem_names = load_gem_metadata(&lua, pob_src)?;
    println!("  Found {} gem metadata entries", gem_names.len());

    // Build the reverse SkillType map after all skill files have touched SkillType
    // We need to load the skill files first to populate SkillType, then build the map.
    // Strategy: load all files into a combined skills table, then build the reverse map.

    // Create a shared `skills` table in Lua
    lua.load("__all_skills = {}").exec()?;

    let skills_file_dir = format!("{}/Data/Skills", pob_src);

    for filename in SKILL_FILES {
        let filepath = format!("{}/{}", skills_file_dir, filename);
        let code = match std::fs::read_to_string(&filepath) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  Warning: could not read {}: {}", filepath, e);
                continue;
            }
        };

        // Each file receives (skills, mod, flag, skill) as varargs
        let chunk = lua.load(&code);
        let result: mlua::Result<()> = (|| {
            let skills_table: LuaTable = lua.globals().get("__all_skills")?;
            let mod_fn: LuaFunction = lua.globals().get("mod")?;
            let flag_fn: LuaFunction = lua.globals().get("flag")?;
            let skill_fn: LuaFunction = lua.globals().get("skill")?;
            chunk
                .call::<()>((skills_table, mod_fn, flag_fn, skill_fn))
                .map_err(|e| e.into())
        })();

        match result {
            Ok(()) => {
                println!("  Loaded {}", filename);
            }
            Err(e) => {
                eprintln!("  Warning: failed to evaluate {}: {}", filename, e);
            }
        }
    }

    // Now build the reverse map
    let reverse_map = build_skill_type_reverse_map(&lua)?;
    println!("  SkillType reverse map has {} entries", reverse_map.len());

    // Extract all skills
    let all_skills: LuaTable = lua.globals().get("__all_skills")?;
    let mut gems: HashMap<String, SkillGemData> = HashMap::new();

    for pair in all_skills.pairs::<String, LuaValue>() {
        let (skill_id, value) = pair?;
        if let LuaValue::Table(skill_table) = value {
            match parse_skill(&skill_id, &skill_table, &reverse_map, &gem_names) {
                Ok(gem_data) => {
                    gems.insert(skill_id, gem_data);
                }
                Err(e) => {
                    eprintln!("  Warning: failed to parse skill '{}': {}", skill_id, e);
                }
            }
        }
    }

    println!("  Extracted {} gems total", gems.len());

    let supports = gems.values().filter(|g| g.is_support).count();
    println!("  {} are support gems", supports);

    // Write output
    let out_path = output.join("gems.json");
    let json = serde_json::to_string_pretty(&gems)?;
    std::fs::write(&out_path, json)?;
    println!("  Written to {}", out_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn pob_src_dir() -> String {
        std::env::var("POB_SRC").unwrap_or_else(|_| {
            let workspace = env!("CARGO_MANIFEST_DIR");
            format!("{}/../../third-party/PathOfBuilding/src", workspace)
        })
    }

    #[test]
    fn extract_produces_gems() {
        let pob_src = pob_src_dir();
        let tmp = tempfile::tempdir().unwrap();
        extract(&pob_src, tmp.path()).unwrap();

        let json_path = tmp.path().join("gems.json");
        assert!(json_path.exists(), "gems.json should be created");

        let data: HashMap<String, SkillGemData> =
            serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();

        // At least 500 gems total
        assert!(
            data.len() >= 500,
            "Expected at least 500 gems, got {}",
            data.len()
        );

        // Fireball exists with skill_types and levels
        let fireball = data.get("Fireball").expect("Fireball should exist");
        assert!(
            !fireball.skill_types.is_empty(),
            "Fireball should have skill_types"
        );
        assert!(!fireball.levels.is_empty(), "Fireball should have levels");
        assert!(
            fireball.levels.len() >= 20,
            "Fireball should have at least 20 levels, got {}",
            fireball.levels.len()
        );
        assert!(
            fireball.cast_time > 0.0,
            "Fireball should have cast_time > 0"
        );

        // At least 100 support gems
        let supports: Vec<_> = data.values().filter(|g| g.is_support).collect();
        assert!(
            supports.len() >= 100,
            "Expected at least 100 supports, got {}",
            supports.len()
        );

        // At least one support has require_skill_types
        let has_req = supports.iter().any(|g| !g.require_skill_types.is_empty());
        assert!(
            has_req,
            "At least one support should have require_skill_types"
        );
    }
}
