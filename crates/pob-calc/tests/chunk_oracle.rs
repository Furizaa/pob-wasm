//! Per-chunk oracle test runner.
//!
//! Runs all 30 realworld oracle builds but only asserts on the output fields
//! belonging to a specific chunk. This gives focused feedback for agents
//! working on one subsystem at a time.
//!
//! Usage:
//!   CHUNK=DEF-01-resistances DATA_DIR=./data cargo test --test chunk_oracle -- --nocapture
//!
//! The CHUNK env var selects which field group to check.
//! If CHUNK is not set, prints available chunk IDs and exits.

mod field_groups;

use pob_calc::{build::parse_xml, calc::calculate, data::GameData};
use std::sync::Arc;

fn load_game_data() -> Option<Arc<GameData>> {
    let data_dir = std::env::var("DATA_DIR").ok()?;
    let json = build_real_game_data_json(&data_dir).ok()?;
    let mut data = GameData::from_json(&json).ok()?;

    // Load version-specific passive trees for better accuracy with old builds.
    let tree_dir = format!("{data_dir}/tree");
    for (version, filename) in &[("3_13", "poe1_3_13.json"), ("3_6", "poe1_3_6.json")] {
        let path = format!("{tree_dir}/{filename}");
        if let Ok(tree_str) = std::fs::read_to_string(&path) {
            if let Ok(tree) = pob_calc::passive_tree::PassiveTree::from_json(&tree_str) {
                data.add_versioned_tree(version.to_string(), tree);
            }
        }
    }

    // Load gem attribute requirement multipliers.
    let gem_reqs_path = format!("{data_dir}/gem_reqs.json");
    if let Ok(gem_reqs_str) = std::fs::read_to_string(&gem_reqs_path) {
        let _ = data.load_gem_reqs_from_json(&gem_reqs_str);
    }

    // Load legion jewel data (for SETUP-06 timeless jewel replacements).
    let legion_path = format!("{data_dir}/legion.json");
    if let Ok(legion_str) = std::fs::read_to_string(&legion_path) {
        let _ = data.load_legion_data_from_json(&legion_str);
    }

    // Load mastery effects sidecar (for SETUP-09-mastery-selections).
    // Optional: gracefully skipped if the file is absent (pre-3.16 builds don't use masteries).
    let mastery_path = format!("{data_dir}/mastery_effects.json");
    if let Ok(mastery_str) = std::fs::read_to_string(&mastery_path) {
        let _ = data.load_mastery_effects_from_json(&mastery_str);
    }

    Some(Arc::new(data))
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

fn compare_value(actual: &serde_json::Value, expected: &serde_json::Value) -> Option<String> {
    match (expected, actual) {
        (serde_json::Value::Number(e), serde_json::Value::Number(a)) => {
            let e = e.as_f64().unwrap();
            let a = a.as_f64().unwrap();
            let tolerance = (e * 0.001).abs().max(0.01);
            if (a - e).abs() > tolerance {
                Some(format!("expected {e}, got {a} (tol {tolerance:.4})"))
            } else {
                None
            }
        }
        (serde_json::Value::Bool(e), serde_json::Value::Bool(a)) => {
            if a != e {
                Some(format!("expected {e}, got {a}"))
            } else {
                None
            }
        }
        (serde_json::Value::String(e), serde_json::Value::String(a)) => {
            if a != e {
                Some(format!("expected {e:?}, got {a:?}"))
            } else {
                None
            }
        }
        _ => Some(format!("type mismatch: expected {expected}, got {actual}")),
    }
}

#[test]
fn chunk_oracle() {
    let chunk_id = match std::env::var("CHUNK") {
        Ok(c) => c,
        Err(_) => {
            println!("CHUNK env var not set. Available chunks:");
            for id in field_groups::all_chunk_ids() {
                let fields = field_groups::fields_for_chunk(id).unwrap_or(&[]);
                println!("  {id:40} ({} fields)", fields.len());
            }
            println!("\nUsage: CHUNK=DEF-01-resistances DATA_DIR=./data cargo test --test chunk_oracle -- --nocapture");
            return;
        }
    };

    let fields = field_groups::fields_for_chunk(&chunk_id)
        .unwrap_or_else(|| panic!("Unknown chunk: {chunk_id}"));

    if fields.is_empty() {
        println!("Chunk {chunk_id} has no fields defined yet (placeholder). Skipping.");
        return;
    }

    let data = load_game_data().expect("DATA_DIR must be set and contain valid game data");

    let build_names = field_groups::realworld_build_names();
    assert!(
        !build_names.is_empty(),
        "No realworld builds found in tests/oracle/"
    );

    let mut builds_pass = 0;
    let mut builds_fail = 0;
    let mut total_fields_correct = 0;
    let mut total_fields_checked = 0;
    let mut failure_details: Vec<String> = Vec::new();

    for build_name in &build_names {
        let xml = load_build_xml(build_name);
        let build = match parse_xml(&xml) {
            Ok(b) => b,
            Err(e) => {
                failure_details.push(format!("{build_name}: parse error: {e}"));
                builds_fail += 1;
                continue;
            }
        };

        let result = match calculate(&build, Arc::clone(&data)) {
            Ok(r) => r,
            Err(e) => {
                failure_details.push(format!("{build_name}: calc error: {e}"));
                builds_fail += 1;
                continue;
            }
        };

        let actual = serde_json::to_value(&result.output).unwrap();
        let actual_obj = actual.as_object().unwrap();

        let expected_full = load_expected(build_name);
        let expected_output = expected_full.get("output").unwrap_or(&expected_full);
        let expected_obj = expected_output.as_object().unwrap();

        let mut build_failures: Vec<String> = Vec::new();
        let mut fields_ok = 0;
        let mut fields_checked = 0;

        for &field in fields {
            let exp = expected_obj.get(field);
            let act = actual_obj.get(field);

            match (act, exp) {
                (None, None) => {
                    // Field not in expected or actual for this build — skip
                }
                (None, Some(exp_val)) => {
                    fields_checked += 1;
                    build_failures.push(format!("  {field}: missing (expected {exp_val})"));
                }
                (Some(_act_val), None) => {
                    // Field in actual but not expected — not a failure for this chunk
                    // (the full oracle test catches unexpected fields)
                    fields_checked += 1;
                    fields_ok += 1;
                }
                (Some(act_val), Some(exp_val)) => {
                    fields_checked += 1;
                    if let Some(msg) = compare_value(act_val, exp_val) {
                        build_failures.push(format!("  {field}: {msg}"));
                    } else {
                        fields_ok += 1;
                    }
                }
            }
        }

        total_fields_correct += fields_ok;
        total_fields_checked += fields_checked;

        if build_failures.is_empty() {
            builds_pass += 1;
        } else {
            builds_fail += 1;
            failure_details.push(format!(
                "{build_name}: {}/{} fields correct\n{}",
                fields_ok,
                fields_checked,
                build_failures.join("\n")
            ));
        }
    }

    println!("\n=== Chunk: {chunk_id} ===");
    println!("Builds: {builds_pass}/{} pass", builds_pass + builds_fail);
    println!("Fields: {total_fields_correct}/{total_fields_checked} correct across all builds");

    if !failure_details.is_empty() {
        println!("\nFailures:");
        for detail in &failure_details {
            println!("{detail}");
        }
    }

    assert_eq!(
        builds_fail, 0,
        "Chunk {chunk_id}: {builds_fail} builds failed"
    );
}
