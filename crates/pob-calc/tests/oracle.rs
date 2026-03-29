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
