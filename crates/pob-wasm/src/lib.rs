use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use wasm_bindgen::prelude::*;

use pob_calc::{
    build::parse_xml,
    calc::env::CalcEnv,
    data::GameData,
    mod_db::types::{KeywordFlags, ModFlags, ModType, ModValue},
};

// ─── Global state ─────────────────────────────────────────────────────────────

/// Shared game data. Initialised once via init().
static GAME_DATA: OnceLock<Arc<GameData>> = OnceLock::new();

/// Active build environments keyed by handle ID.
static BUILD_ENVS: OnceLock<Mutex<HashMap<u32, StoredEnv>>> = OnceLock::new();

/// Next handle ID counter.
static NEXT_HANDLE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

fn build_envs() -> &'static Mutex<HashMap<u32, StoredEnv>> {
    BUILD_ENVS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A stored calculation environment for getMods() queries.
struct StoredEnv {
    /// We store the CalcEnv so getMods() can query the ModDb.
    /// CalcEnv owns its GameData via Arc, so it can outlive the call.
    env: CalcEnv,
}

// ─── WASM exports ─────────────────────────────────────────────────────────────

/// Initialize the engine with combined game data JSON.
/// Must be called once before any calculate() calls.
///
/// game_data_json: JSON string with structure matching data-extractor output.
#[wasm_bindgen]
pub fn init(game_data_json: String) -> Result<(), JsValue> {
    let data = GameData::from_json(&game_data_json)
        .map_err(|e| JsValue::from_str(&format!("init error: {e}")))?;
    GAME_DATA
        .set(Arc::new(data))
        .map_err(|_| JsValue::from_str("init() already called"))?;
    Ok(())
}

/// Run all calculations for a POB XML build string.
/// Returns a JSON string containing { handle, output, breakdown }.
#[wasm_bindgen]
pub fn calculate(pob_xml: String) -> Result<String, JsValue> {
    let data = get_game_data()?;
    let build =
        parse_xml(&pob_xml).map_err(|e| JsValue::from_str(&format!("XML parse error: {e}")))?;

    // Build the CalcEnv and run all calculation passes
    let mut env = pob_calc::calc::setup::init_env(&build, Arc::clone(&data))
        .map_err(|e| JsValue::from_str(&format!("setup error: {e}")))?;

    pob_calc::calc::perform::run(&mut env);
    pob_calc::calc::defence::run(&mut env);
    pob_calc::calc::active_skill::run(&mut env, &build);
    pob_calc::calc::offence::run(&mut env, &build);
    pob_calc::calc::triggers::run(&mut env, &build);
    pob_calc::calc::mirages::run(&mut env, &build);

    // Assign a handle and store the env
    let handle = NEXT_HANDLE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let output = env.player.output.clone();
    let breakdown = env.player.breakdown.clone();
    build_envs()
        .lock()
        .unwrap()
        .insert(handle, StoredEnv { env });

    // Serialize result
    let result = WasmCalcResult {
        handle,
        output,
        breakdown,
    };
    serde_json::to_string(&result)
        .map_err(|e| JsValue::from_str(&format!("serialization error: {e}")))
}

/// Run calculations for a specific skill index (0-based).
#[wasm_bindgen(js_name = calculateSkill)]
pub fn calculate_skill(pob_xml: String, skill_index: u32) -> Result<String, JsValue> {
    let data = get_game_data()?;
    let mut build =
        parse_xml(&pob_xml).map_err(|e| JsValue::from_str(&format!("XML parse error: {e}")))?;
    build.main_socket_group = skill_index as usize;

    let mut env = pob_calc::calc::setup::init_env(&build, Arc::clone(&data))
        .map_err(|e| JsValue::from_str(&format!("setup error: {e}")))?;
    pob_calc::calc::perform::run(&mut env);
    pob_calc::calc::defence::run(&mut env);
    pob_calc::calc::active_skill::run(&mut env, &build);
    pob_calc::calc::offence::run(&mut env, &build);
    pob_calc::calc::triggers::run(&mut env, &build);
    pob_calc::calc::mirages::run(&mut env, &build);

    let handle = NEXT_HANDLE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let output = env.player.output.clone();
    let breakdown = env.player.breakdown.clone();
    build_envs()
        .lock()
        .unwrap()
        .insert(handle, StoredEnv { env });

    let result = WasmCalcResult {
        handle,
        output,
        breakdown,
    };
    serde_json::to_string(&result)
        .map_err(|e| JsValue::from_str(&format!("serialization error: {e}")))
}

/// Query all modifiers contributing to a stat from a cached build environment.
///
/// handle:   returned by calculate() or calculateSkill()
/// mod_name: stat name, e.g. "Life", "FireResist"
/// mod_type: optional — "BASE" | "INC" | "MORE" | "FLAG" | "LIST"
/// cfg:      optional scope — "skill" | "weapon1" | "weapon2"
///
/// Returns a JSON array of ModEntry objects.
#[wasm_bindgen(js_name = getMods)]
pub fn get_mods(
    handle: u32,
    mod_name: String,
    mod_type: Option<String>,
    cfg: Option<String>,
) -> Result<String, JsValue> {
    let envs = build_envs().lock().unwrap();
    let stored = envs
        .get(&handle)
        .ok_or_else(|| JsValue::from_str(&format!("invalid handle: {handle}")))?;

    let filter_type: Option<ModType> = mod_type.as_deref().map(|s| match s {
        "BASE" => ModType::Base,
        "INC" => ModType::Inc,
        "MORE" => ModType::More,
        "FLAG" => ModType::Flag,
        "LIST" => ModType::List,
        _ => ModType::Base,
    });

    // Determine ModFlags from cfg scope
    let (query_flags, query_keyword_flags) = match cfg.as_deref() {
        Some("skill") => {
            if let Some(ref skill) = stored.env.player.main_skill {
                let mut flags = ModFlags::NONE;
                if skill.is_attack {
                    flags = ModFlags(flags.0 | ModFlags::ATTACK.0);
                }
                if skill.is_spell {
                    flags = ModFlags(flags.0 | ModFlags::SPELL.0);
                }
                (flags, KeywordFlags::NONE)
            } else {
                (ModFlags::NONE, KeywordFlags::NONE)
            }
        }
        Some("weapon1") | Some("weapon2") => (ModFlags::ATTACK, KeywordFlags::NONE),
        _ => (ModFlags::NONE, KeywordFlags::NONE),
    };

    let rows =
        stored
            .env
            .player
            .mod_db
            .tabulate(&mod_name, filter_type, query_flags, query_keyword_flags);

    let entries: Vec<WasmModEntry> = rows
        .into_iter()
        .map(|r| WasmModEntry {
            value: match &r.value {
                ModValue::Number(n) => serde_json::Value::Number(
                    serde_json::Number::from_f64(*n).unwrap_or(serde_json::Number::from(0)),
                ),
                ModValue::Bool(b) => serde_json::Value::Bool(*b),
                ModValue::String(s) => serde_json::Value::String(s.clone()),
            },
            mod_type: format!("{:?}", r.mod_type).to_uppercase(),
            source: r.source_category,
            source_name: r.source_name,
            flags: format!("{:?}", r.flags),
            tags: String::new(),
        })
        .collect();

    serde_json::to_string(&entries)
        .map_err(|e| JsValue::from_str(&format!("serialization error: {e}")))
}

/// Release a cached build environment and free its memory.
#[wasm_bindgen(js_name = releaseBuild)]
pub fn release_build(handle: u32) {
    build_envs().lock().unwrap().remove(&handle);
}

/// Returns the engine version string. Used to verify the WASM module loaded correctly.
#[wasm_bindgen]
pub fn version() -> String {
    pob_calc::version().to_string()
}

// ─── Private helpers ──────────────────────────────────────────────────────────

fn get_game_data() -> Result<Arc<GameData>, JsValue> {
    GAME_DATA
        .get()
        .cloned()
        .ok_or_else(|| JsValue::from_str("call init() before calculate()"))
}

// ─── Serialization helpers (not exported to JS) ───────────────────────────────

#[derive(serde::Serialize)]
struct WasmCalcResult {
    handle: u32,
    output: pob_calc::calc::env::OutputTable,
    breakdown: pob_calc::calc::env::BreakdownTable,
}

#[derive(serde::Serialize)]
struct WasmModEntry {
    value: serde_json::Value,
    #[serde(rename = "type")]
    mod_type: String,
    source: String,
    #[serde(rename = "sourceName")]
    source_name: String,
    flags: String,
    tags: String,
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn stub_game_data() -> String {
        r#"{
            "gems": {},
            "misc": {
                "game_constants": {
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
                },
                "character_constants": {"life_per_str": 0.5},
                "monster_life_table": [],
                "monster_damage_table": [],
                "monster_evasion_table": [],
                "monster_accuracy_table": [],
                "monster_ally_life_table": [],
                "monster_ally_damage_table": [],
                "monster_ailment_threshold_table": [],
                "monster_phys_conversion_multi_table": []
            }
        }"#
        .to_string()
    }

    const TEST_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"></Build>
  <Skills activeSkillSet="1"><SkillSet id="1">
    <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
      <Gem skillId="Cleave" level="20" quality="0" enabled="true"/>
    </Skill>
  </SkillSet></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;

    #[test]
    fn init_then_calculate_returns_json() {
        GAME_DATA
            .set(Arc::new(GameData::from_json(&stub_game_data()).unwrap()))
            .ok();

        let result_json = calculate(TEST_XML.to_string()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result_json).unwrap();
        assert!(parsed.get("handle").is_some(), "result must have handle");
        assert!(parsed.get("output").is_some(), "result must have output");
        assert!(
            parsed.get("breakdown").is_some(),
            "result must have breakdown"
        );

        let handle = parsed["handle"].as_u64().unwrap() as u32;
        release_build(handle);
    }

    #[test]
    fn get_mods_returns_array() {
        GAME_DATA
            .set(Arc::new(GameData::from_json(&stub_game_data()).unwrap()))
            .ok();

        let result_json = calculate(TEST_XML.to_string()).unwrap();
        let result: serde_json::Value = serde_json::from_str(&result_json).unwrap();
        let handle = result["handle"].as_u64().unwrap() as u32;

        let mods_json = get_mods(handle, "Life".to_string(), None, None).unwrap();
        let mods: serde_json::Value = serde_json::from_str(&mods_json).unwrap();
        assert!(mods.is_array(), "getMods must return an array");

        release_build(handle);
    }

    #[test]
    fn get_mods_cfg_none_returns_all_mods() {
        GAME_DATA
            .set(Arc::new(GameData::from_json(&stub_game_data()).unwrap()))
            .ok();

        let result_json = calculate(TEST_XML.to_string()).unwrap();
        let result: serde_json::Value = serde_json::from_str(&result_json).unwrap();
        let handle = result["handle"].as_u64().unwrap() as u32;

        // No cfg — returns all Life mods
        let mods_json = get_mods(handle, "Life".to_string(), None, None).unwrap();
        let mods: Vec<serde_json::Value> = serde_json::from_str(&mods_json).unwrap();
        assert!(
            !mods.is_empty(),
            "should return Life mods with no cfg filter"
        );

        // cfg="skill" — Cleave is an attack so ATTACK flag is set; Life mods have no flag
        // restriction (flags==NONE), and ATTACK.contains(NONE) is true, so same count expected
        let skill_mods_json =
            get_mods(handle, "Life".to_string(), None, Some("skill".to_string())).unwrap();
        let skill_mods: Vec<serde_json::Value> = serde_json::from_str(&skill_mods_json).unwrap();
        assert_eq!(
            mods.len(),
            skill_mods.len(),
            "cfg=skill should return same count as no cfg for flag-unrestricted mods"
        );

        release_build(handle);
    }

    #[test]
    fn release_build_removes_handle() {
        GAME_DATA
            .set(Arc::new(GameData::from_json(&stub_game_data()).unwrap()))
            .ok();

        let result_json = calculate(TEST_XML.to_string()).unwrap();
        let result: serde_json::Value = serde_json::from_str(&result_json).unwrap();
        let handle = result["handle"].as_u64().unwrap() as u32;

        // Verify handle exists before release
        assert!(
            build_envs().lock().unwrap().contains_key(&handle),
            "handle should exist before release"
        );

        release_build(handle);

        // After release, the handle should no longer be in the store
        // (JsValue::from_str panics on non-wasm targets so we check the map directly)
        assert!(
            !build_envs().lock().unwrap().contains_key(&handle),
            "handle should be removed after release"
        );
    }
}
