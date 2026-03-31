//! Oracle tests: compare Rust engine output against POB's ground-truth output.
//!
//! Real-world oracle tests are marked #[ignore] and run in CI with DATA_DIR set.
//! Run locally with: DATA_DIR=path/to/data cargo test --test oracle -- --ignored

use pob_calc::{build::parse_xml, calc::calculate, data::GameData};
use std::sync::Arc;

fn load_game_data() -> Option<Arc<GameData>> {
    let data_dir = std::env::var("DATA_DIR").ok()?;
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

    let bases: serde_json::Value = std::fs::read_to_string(format!("{data_dir}/bases.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Array(vec![]));

    let uniques: serde_json::Value = std::fs::read_to_string(format!("{data_dir}/uniques.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Array(vec![]));

    let combined = serde_json::json!({
        "gems": gems,
        "misc": misc,
        "tree": tree,
        "bases": bases,
        "uniques": uniques,
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

#[allow(dead_code)]
fn assert_output_approx(actual: &serde_json::Value, expected: &serde_json::Value, key: &str) {
    let a = actual.get(key).and_then(|v| v.as_f64());
    let e = expected.get(key).and_then(|v| v.as_f64());
    match (a, e) {
        (Some(a), Some(e)) => {
            let tolerance = (e * 0.001).abs().max(0.01);
            assert!(
                (a - e).abs() <= tolerance,
                "output[{key}]: expected {e}, got {a} (tolerance {tolerance})"
            );
        }
        (None, Some(e)) => panic!("output[{key}]: missing from actual output (expected {e})"),
        (Some(a), None) => panic!("output[{key}]: unexpected in actual output (got {a})"),
        (None, None) => {}
    }
}

fn assert_output_strict(actual: &serde_json::Value, expected: &serde_json::Value) {
    let actual_obj = actual
        .as_object()
        .expect("actual output should be a JSON object");
    let expected_obj = expected
        .as_object()
        .expect("expected output should be a JSON object");

    let mut failures: Vec<String> = Vec::new();

    for (key, exp_val) in expected_obj {
        match actual_obj.get(key) {
            None => failures.push(format!("  {key}: missing from actual (expected {exp_val})")),
            Some(act_val) => {
                if let Some(msg) = compare_values_soft(key, act_val, exp_val) {
                    failures.push(format!("  {key}: {msg}"));
                }
            }
        }
    }

    for key in actual_obj.keys() {
        if !expected_obj.contains_key(key) {
            failures.push(format!(
                "  {key}: unexpected in actual (got {})",
                actual_obj[key]
            ));
        }
    }

    if !failures.is_empty() {
        let count = failures.len();
        let detail = failures.join("\n");
        panic!("Oracle parity check failed ({count} field(s)):\n{detail}");
    }
}

fn compare_values_soft(
    _key: &str,
    actual: &serde_json::Value,
    expected: &serde_json::Value,
) -> Option<String> {
    match (expected, actual) {
        (serde_json::Value::Number(e), serde_json::Value::Number(a)) => {
            let e = e.as_f64().unwrap();
            let a = a.as_f64().unwrap();
            let tolerance = (e * 0.001).abs().max(0.01);
            if (a - e).abs() > tolerance {
                Some(format!("expected {e}, got {a} (tolerance {tolerance})"))
            } else {
                None
            }
        }
        (serde_json::Value::Bool(e), serde_json::Value::Bool(a)) => {
            if a != e {
                Some(format!("boolean mismatch — expected {e}, got {a}"))
            } else {
                None
            }
        }
        (serde_json::Value::String(e), serde_json::Value::String(a)) => {
            if a != e {
                Some(format!("string mismatch — expected {e:?}, got {a:?}"))
            } else {
                None
            }
        }
        _ => Some(format!("type mismatch — expected {expected}, got {actual}")),
    }
}

fn run_oracle_parity(name: &str) {
    let data = load_game_data()
        .expect("DATA_DIR must be set and contain valid game data for oracle tests");
    let xml = load_build_xml(name);
    let build = parse_xml(&xml).unwrap_or_else(|e| panic!("Failed to parse {name}.xml: {e}"));
    let result = calculate(&build, Arc::clone(&data))
        .unwrap_or_else(|e| panic!("Failed to calculate {name}: {e}"));
    let actual =
        serde_json::to_value(&result.output).expect("result.output should serialize to JSON");
    let expected_full = load_expected(name);
    let expected_output = expected_full.get("output").unwrap_or(&expected_full);
    assert_output_strict(&actual, expected_output);
}

#[test]
fn oracle_all_builds_parse() {
    let oracle_dir = std::path::Path::new("tests/oracle");
    let mut count = 0;
    for entry in std::fs::read_dir(oracle_dir).expect("tests/oracle/ should exist") {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map(|e| e == "xml").unwrap_or(false) {
            let xml = std::fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("Cannot read {}", path.display()));
            parse_xml(&xml).unwrap_or_else(|e| panic!("Failed to parse {}: {e}", path.display()));
            count += 1;
        }
    }
    assert!(
        count >= 30,
        "Expected at least 30 oracle builds, found {count}"
    );
}

macro_rules! oracle_test {
    ($name:ident, $build:expr) => {
        #[test]
        #[ignore]
        fn $name() {
            run_oracle_parity($build);
        }
    };
}

oracle_test!(oracle_phys_melee_slayer, "realworld_phys_melee_slayer");
oracle_test!(oracle_ele_melee_raider, "realworld_ele_melee_raider");
oracle_test!(
    oracle_spell_caster_inquisitor,
    "realworld_spell_caster_inquisitor"
);
oracle_test!(
    oracle_dot_caster_trickster,
    "realworld_dot_caster_trickster"
);
oracle_test!(oracle_ignite_elementalist, "realworld_ignite_elementalist");
oracle_test!(oracle_bleed_gladiator, "realworld_bleed_gladiator");
oracle_test!(oracle_poison_pathfinder, "realworld_poison_pathfinder");
oracle_test!(oracle_totem_hierophant, "realworld_totem_hierophant");
oracle_test!(oracle_trap_saboteur, "realworld_trap_saboteur");
oracle_test!(oracle_mine_saboteur, "realworld_mine_saboteur");
oracle_test!(oracle_minion_necromancer, "realworld_minion_necromancer");
oracle_test!(oracle_bow_deadeye, "realworld_bow_deadeye");
oracle_test!(oracle_wand_occultist, "realworld_wand_occultist");
oracle_test!(oracle_champion_impale, "realworld_champion_impale");
oracle_test!(oracle_coc_trigger, "realworld_coc_trigger");
oracle_test!(oracle_cwc_trigger, "realworld_cwc_trigger");
oracle_test!(oracle_dual_wield, "realworld_dual_wield");
oracle_test!(oracle_shield_1h, "realworld_shield_1h");
oracle_test!(oracle_two_handed, "realworld_two_handed");
oracle_test!(oracle_ci_lowlife_es, "realworld_ci_lowlife_es");
oracle_test!(oracle_mom_eb, "realworld_mom_eb");
oracle_test!(oracle_aura_stacker, "realworld_aura_stacker");
oracle_test!(oracle_flask_pathfinder, "realworld_flask_pathfinder");
oracle_test!(oracle_cluster_jewel, "realworld_cluster_jewel");
oracle_test!(oracle_timeless_jewel, "realworld_timeless_jewel");
oracle_test!(oracle_max_block_gladiator, "realworld_max_block_gladiator");
oracle_test!(oracle_rf_juggernaut, "realworld_rf_juggernaut");
oracle_test!(
    oracle_phys_to_fire_conversion,
    "realworld_phys_to_fire_conversion"
);
oracle_test!(oracle_triple_conversion, "realworld_triple_conversion");
oracle_test!(oracle_spectre_summoner, "realworld_spectre_summoner");

// ── Phase 5 integration tests: item loading and basic stats ─────────────────

#[test]
#[ignore] // requires DATA_DIR
fn phase5_items_loaded_and_mods_applied() {
    let data = load_game_data()
        .expect("DATA_DIR must be set and contain valid game data for phase5 tests");
    let xml = load_build_xml("realworld_phys_melee_slayer");
    let build = parse_xml(&xml).unwrap();

    // Verify items were parsed
    assert!(
        !build.items.is_empty(),
        "Build should have items, got {}",
        build.items.len()
    );
    println!(
        "realworld_phys_melee_slayer: {} items parsed",
        build.items.len()
    );

    // Run calculation
    let result = calculate(&build, Arc::clone(&data)).unwrap();

    // Life should be substantial for a real build with gear
    let life = result
        .output
        .get("Life")
        .and_then(|v| match v {
            pob_calc::calc::env::OutputValue::Number(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(0.0);
    println!("Life: {life}");
    assert!(
        life > 500.0,
        "Life should be > 500 for a real build, got {life}"
    );

    // Str should be positive for a melee build
    let str_val = result
        .output
        .get("Str")
        .and_then(|v| match v {
            pob_calc::calc::env::OutputValue::Number(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(0.0);
    println!("Str: {str_val}");
    assert!(
        str_val > 0.0,
        "Str should be > 0 for a melee build, got {str_val}"
    );
}

#[test]
#[ignore] // requires DATA_DIR
fn phase5_all_oracle_builds_have_items() {
    let mut builds_with_items = 0;
    let mut builds_total = 0;

    for entry in std::fs::read_dir("tests/oracle").unwrap() {
        let path = entry.unwrap().path();
        let fname = path.file_name().unwrap().to_str().unwrap_or("").to_string();
        if path.extension().map_or(false, |e| e == "xml") && fname.starts_with("realworld_") {
            builds_total += 1;
            let xml = std::fs::read_to_string(&path).unwrap();
            let build = parse_xml(&xml).unwrap();
            let item_count = build.items.len();
            if item_count > 0 {
                builds_with_items += 1;
            }
            println!("{fname}: {item_count} items");
        }
    }

    println!("\n{builds_with_items}/{builds_total} builds have items");
    assert!(
        builds_with_items > 0,
        "At least some builds should have items"
    );
    // All real-world builds should have items
    assert_eq!(
        builds_with_items, builds_total,
        "All real-world builds should have items, but only {builds_with_items}/{builds_total} do"
    );
}

#[test]
#[ignore] // requires DATA_DIR
fn phase5_all_oracle_builds_calculate_with_items() {
    let data = load_game_data()
        .expect("DATA_DIR must be set and contain valid game data for phase5 tests");
    let mut success = 0;
    let mut failed = 0;
    let mut errors: Vec<String> = Vec::new();

    for entry in std::fs::read_dir("tests/oracle").unwrap() {
        let path = entry.unwrap().path();
        let fname = path.file_name().unwrap().to_str().unwrap_or("").to_string();
        if path.extension().map_or(false, |e| e == "xml") && fname.starts_with("realworld_") {
            let xml = std::fs::read_to_string(&path).unwrap();
            let build = match parse_xml(&xml) {
                Ok(b) => b,
                Err(e) => {
                    errors.push(format!("{fname}: parse error: {e}"));
                    failed += 1;
                    continue;
                }
            };

            let item_count = build.items.len();

            match calculate(&build, Arc::clone(&data)) {
                Ok(result) => {
                    let life = result
                        .output
                        .get("Life")
                        .and_then(|v| match v {
                            pob_calc::calc::env::OutputValue::Number(n) => Some(*n),
                            _ => None,
                        })
                        .unwrap_or(0.0);
                    let str_val = result
                        .output
                        .get("Str")
                        .and_then(|v| match v {
                            pob_calc::calc::env::OutputValue::Number(n) => Some(*n),
                            _ => None,
                        })
                        .unwrap_or(0.0);
                    let dex_val = result
                        .output
                        .get("Dex")
                        .and_then(|v| match v {
                            pob_calc::calc::env::OutputValue::Number(n) => Some(*n),
                            _ => None,
                        })
                        .unwrap_or(0.0);
                    let int_val = result
                        .output
                        .get("Int")
                        .and_then(|v| match v {
                            pob_calc::calc::env::OutputValue::Number(n) => Some(*n),
                            _ => None,
                        })
                        .unwrap_or(0.0);

                    println!(
                        "{fname}: {item_count} items, Life={life:.0}, Str={str_val:.0}, Dex={dex_val:.0}, Int={int_val:.0}"
                    );
                    success += 1;
                }
                Err(e) => {
                    errors.push(format!("{fname}: calc error: {e}"));
                    failed += 1;
                }
            }
        }
    }

    println!("\n{success} succeeded, {failed} failed");
    if !errors.is_empty() {
        println!("\nErrors:");
        for e in &errors {
            println!("  {e}");
        }
    }

    assert!(
        success > 0,
        "At least some builds should calculate successfully"
    );
    assert_eq!(failed, 0, "{failed} builds failed to calculate");
}

#[cfg(test)]
mod comparator_tests {
    use super::*;

    #[test]
    fn strict_passes_matching_output() {
        let actual = serde_json::json!({"Life": 1000.5, "Mana": 500});
        let expected = serde_json::json!({"Life": 1000, "Mana": 500});
        assert_output_strict(&actual, &expected);
    }

    #[test]
    #[should_panic(expected = "missing from actual")]
    fn strict_fails_missing_actual_key() {
        let actual = serde_json::json!({"Life": 1000});
        let expected = serde_json::json!({"Life": 1000, "Mana": 500});
        assert_output_strict(&actual, &expected);
    }

    #[test]
    #[should_panic(expected = "unexpected in actual")]
    fn strict_fails_extra_actual_key() {
        let actual = serde_json::json!({"Life": 1000, "Mana": 500});
        let expected = serde_json::json!({"Life": 1000});
        assert_output_strict(&actual, &expected);
    }

    #[test]
    #[should_panic(expected = "expected 1000")]
    fn strict_fails_value_mismatch() {
        let actual = serde_json::json!({"Life": 1200});
        let expected = serde_json::json!({"Life": 1000});
        assert_output_strict(&actual, &expected);
    }

    #[test]
    #[should_panic(expected = "boolean mismatch")]
    fn strict_fails_boolean_mismatch() {
        let actual = serde_json::json!({"CI": false});
        let expected = serde_json::json!({"CI": true});
        assert_output_strict(&actual, &expected);
    }

    #[test]
    fn strict_handles_mixed_types() {
        let actual = serde_json::json!({"Life": 1000, "CI": true, "MainSkill": "Fireball"});
        let expected = serde_json::json!({"Life": 1000, "CI": true, "MainSkill": "Fireball"});
        assert_output_strict(&actual, &expected);
    }

    #[test]
    #[should_panic(expected = "2 field(s)")]
    fn strict_reports_multiple_failures() {
        let actual = serde_json::json!({"Life": 9999, "Mana": 9999});
        let expected = serde_json::json!({"Life": 1000, "Mana": 500});
        assert_output_strict(&actual, &expected);
    }
}
