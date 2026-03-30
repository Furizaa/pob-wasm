//! Oracle tests: compare Rust engine output against POB's ground-truth output.
//!
//! These tests are skipped unless the DATA_DIR env var points to a directory
//! containing the extracted game data JSON files.

use pob_calc::{build::parse_xml, calc::calculate, data::GameData};
use std::sync::Arc;

fn load_game_data() -> Option<Arc<GameData>> {
    let data_dir = std::env::var("DATA_DIR").ok()?;
    // Compose the combined JSON from individual data files
    let json = build_real_game_data_json(&data_dir).ok()?;
    GameData::from_json(&json).ok().map(Arc::new)
}

fn build_real_game_data_json(data_dir: &str) -> Result<String, Box<dyn std::error::Error>> {
    let gems_str = std::fs::read_to_string(format!("{data_dir}/gems.json"))?;
    let misc_str = std::fs::read_to_string(format!("{data_dir}/misc.json"))?;
    let tree_str = std::fs::read_to_string(format!("{data_dir}/tree/poe1_current.json"))?;

    let gems: serde_json::Value = serde_json::from_str(&gems_str)?;
    let misc: serde_json::Value = serde_json::from_str(&misc_str)?;
    let tree: serde_json::Value = serde_json::from_str(&tree_str)?;

    let combined = serde_json::json!({
        "gems": gems,
        "misc": misc,
        "tree": tree,
    });
    Ok(serde_json::to_string(&combined)?)
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

#[test]
fn oracle_melee_str_passives_life_matches_pob() {
    let Some(data) = load_game_data() else {
        eprintln!("DATA_DIR not set, skipping oracle test");
        return;
    };
    let xml = load_build_xml("melee_str_passives");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("melee_str_passives");
    let expected_output = expected.get("output").unwrap_or(&expected);
    assert_output_approx(&actual, expected_output, "Life");
    assert_output_approx(&actual, expected_output, "Mana");
}

#[test]
fn oracle_crit_spellcaster_dps_matches_pob() {
    let Some(data) = load_game_data() else {
        eprintln!("DATA_DIR not set");
        return;
    };
    let xml = load_build_xml("crit_spellcaster");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("crit_spellcaster");
    let expected_output = expected.get("output").unwrap_or(&expected);
    assert_output_approx(&actual, expected_output, "TotalDPS");
    assert_output_approx(&actual, expected_output, "AverageDamage");
}

#[test]
fn oracle_ignite_dot_dps_matches_pob() {
    let Some(data) = load_game_data() else {
        eprintln!("DATA_DIR not set");
        return;
    };
    let xml = load_build_xml("ignite_dot");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("ignite_dot");
    let expected_output = expected.get("output").unwrap_or(&expected);
    assert_output_approx(&actual, expected_output, "TotalDPS");
    assert_output_approx(&actual, expected_output, "IgniteDPS");
}

#[test]
fn oracle_bleed_dot_dps_matches_pob() {
    let Some(data) = load_game_data() else {
        eprintln!("DATA_DIR not set");
        return;
    };
    let xml = load_build_xml("bleed_dot");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("bleed_dot");
    let expected_output = expected.get("output").unwrap_or(&expected);
    assert_output_approx(&actual, expected_output, "TotalDPS");
    assert_output_approx(&actual, expected_output, "BleedDPS");
}

#[test]
fn oracle_poison_dot_dps_matches_pob() {
    let Some(data) = load_game_data() else {
        eprintln!("DATA_DIR not set");
        return;
    };
    let xml = load_build_xml("poison_dot");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("poison_dot");
    let expected_output = expected.get("output").unwrap_or(&expected);
    assert_output_approx(&actual, expected_output, "TotalDPS");
    assert_output_approx(&actual, expected_output, "PoisonDPS");
}

#[test]
fn oracle_trap_saboteur_dps_matches_pob() {
    let Some(data) = load_game_data() else {
        eprintln!("DATA_DIR not set");
        return;
    };
    let xml = load_build_xml("trap_saboteur");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("trap_saboteur");
    let expected_output = expected.get("output").unwrap_or(&expected);
    assert_output_approx(&actual, expected_output, "TotalDPS");
    assert_output_approx(&actual, expected_output, "TrapCooldown");
}

#[test]
fn oracle_totem_hierophant_dps_matches_pob() {
    let Some(data) = load_game_data() else {
        eprintln!("DATA_DIR not set");
        return;
    };
    let xml = load_build_xml("totem_hierophant");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("totem_hierophant");
    let expected_output = expected.get("output").unwrap_or(&expected);
    assert_output_approx(&actual, expected_output, "TotalDPS");
    assert_output_approx(&actual, expected_output, "TotemLifeTotal");
}

#[test]
fn oracle_mine_detonator_dps_matches_pob() {
    let Some(data) = load_game_data() else {
        eprintln!("DATA_DIR not set");
        return;
    };
    let xml = load_build_xml("mine_detonator");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("mine_detonator");
    let expected_output = expected.get("output").unwrap_or(&expected);
    assert_output_approx(&actual, expected_output, "TotalDPS");
    assert_output_approx(&actual, expected_output, "MineDetonationTime");
}

#[test]
fn oracle_minion_summoner_matches_pob() {
    let Some(data) = load_game_data() else {
        eprintln!("DATA_DIR not set");
        return;
    };
    let xml = load_build_xml("minion_summoner");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("minion_summoner");
    let expected_output = expected.get("output").unwrap_or(&expected);
    assert_output_approx(&actual, expected_output, "TotalDPS");
    assert_output_approx(&actual, expected_output, "MinionCount");
}

#[test]
fn oracle_poe2_basic_matches_pob() {
    let Some(data) = load_game_data() else {
        eprintln!("DATA_DIR not set");
        return;
    };
    let xml = load_build_xml("poe2_basic");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("poe2_basic");
    let expected_output = expected.get("output").unwrap_or(&expected);
    assert_output_approx(&actual, expected_output, "Life");
    assert_output_approx(&actual, expected_output, "TotalDPS");
}
