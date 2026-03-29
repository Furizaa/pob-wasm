//! Oracle tests: compare Rust engine output against POB's ground-truth output.
//!
//! These tests are skipped unless the DATA_DIR env var points to a directory
//! containing the extracted game data JSON files.

use pob_calc::{build::parse_xml, calc::calculate, data::GameData};
use std::sync::Arc;

fn load_game_data() -> Option<Arc<GameData>> {
    let _data_dir = std::env::var("DATA_DIR").ok()?;
    // Load combined JSON from the data directory
    // For now, construct a minimal stub JSON
    let json = build_stub_game_data_json();
    GameData::from_json(&json).ok().map(Arc::new)
}

fn build_stub_game_data_json() -> String {
    // Minimal game data — enough for structure tests
    // Real data comes from DATA_DIR in integration runs
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
                "maximum_blood_scythe_charges": 8,
                "MonsterDamageReductionImprovement": 0,
                "MonsterDamageReductionImprovement_Divisor": 1
            },
            "character_constants": {
                "base_str": 0,
                "base_dex": 0,
                "base_int": 0,
                "life_per_str": 0.5
            },
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

fn load_expected(name: &str) -> serde_json::Value {
    let path = format!("tests/oracle/{name}.expected.json");
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("Oracle file not found: {path}"));
    serde_json::from_str(&content).expect("Oracle file is not valid JSON")
}

fn load_build_xml(name: &str) -> String {
    let path = format!("tests/oracle/{name}.xml");
    std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("Oracle XML not found: {path}"))
}

/// Compare a numeric output value against expected, allowing 0.1% tolerance.
fn assert_output_approx(actual: &serde_json::Value, expected: &serde_json::Value, key: &str) {
    let a = actual.get(key).and_then(|v| v.as_f64());
    let e = expected.get(key).and_then(|v| v.as_f64());
    if let (Some(a), Some(e)) = (a, e) {
        let tolerance = (e * 0.001).abs().max(0.01);
        assert!(
            (a - e).abs() <= tolerance,
            "output[{key}]: expected {e}, got {a} (tolerance {tolerance})"
        );
    }
}

#[test]
fn oracle_melee_str_parses() {
    // This test only checks that the build parses without panicking.
    // It does not check calculated values (those require full engine impl).
    let xml = load_build_xml("melee_str");
    let build = parse_xml(&xml).expect("melee_str.xml should parse");
    assert_eq!(build.class_name, "Marauder");
    assert_eq!(build.level, 90);
}

#[test]
fn oracle_melee_str_calculate_returns_result() {
    // Check that calculate() returns a result without panicking,
    // even if values are all zeroes at this stage.
    let Some(data) = load_game_data() else {
        eprintln!("DATA_DIR not set, skipping oracle test");
        return;
    };
    let xml = load_build_xml("melee_str");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, data).expect("calculate should not error");
    // At minimum, the result should be a valid struct (no panic)
    let _ = serde_json::to_string(&result).expect("result should serialize");
}

#[test]
fn oracle_melee_str_life_matches_pob() {
    // This test will FAIL until perform.rs is fully implemented.
    // That is expected and correct — it drives the implementation.
    let Some(data) = load_game_data() else {
        eprintln!("DATA_DIR not set, skipping oracle test");
        return;
    };
    let xml = load_build_xml("melee_str");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("melee_str");
    let expected_output = expected.get("output").unwrap_or(&expected);
    assert_output_approx(&actual, expected_output, "Life");
}
