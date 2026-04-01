//! Parity Dashboard: runs all 30 realworld oracle builds and reports per-chunk
//! field correctness as a summary table.
//!
//! Usage:
//!   DATA_DIR=./data cargo test --test parity_report -- --nocapture
//!
//! Produces a table showing which chunks pass, which are partial, and the
//! overall parity percentage.

mod field_groups;

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

fn compare_value(actual: &serde_json::Value, expected: &serde_json::Value) -> bool {
    match (expected, actual) {
        (serde_json::Value::Number(e), serde_json::Value::Number(a)) => {
            let e = e.as_f64().unwrap();
            let a = a.as_f64().unwrap();
            let tolerance = (e * 0.001).abs().max(0.01);
            (a - e).abs() <= tolerance
        }
        (serde_json::Value::Bool(e), serde_json::Value::Bool(a)) => a == e,
        (serde_json::Value::String(e), serde_json::Value::String(a)) => a == e,
        _ => false,
    }
}

#[test]
fn parity_report() {
    let data = match load_game_data() {
        Some(d) => d,
        None => {
            println!("DATA_DIR not set or invalid — skipping parity report.");
            println!("Usage: DATA_DIR=./data cargo test --test parity_report -- --nocapture");
            return;
        }
    };

    let build_names = field_groups::realworld_build_names();
    assert!(!build_names.is_empty(), "No realworld builds found");

    // Pre-compute all build results
    let mut build_results: Vec<(String, serde_json::Value, serde_json::Value)> = Vec::new();

    for build_name in &build_names {
        let xml = load_build_xml(build_name);
        let build = parse_xml(&xml).unwrap_or_else(|e| panic!("Parse {build_name}: {e}"));
        let result = calculate(&build, Arc::clone(&data))
            .unwrap_or_else(|e| panic!("Calc {build_name}: {e}"));
        let actual = serde_json::to_value(&result.output).unwrap();
        let expected_full = load_expected(build_name);
        let expected = expected_full
            .get("output")
            .cloned()
            .unwrap_or(expected_full);
        build_results.push((build_name.clone(), actual, expected));
    }

    // Per-chunk analysis
    println!("\n{:=<80}", "");
    println!("  PARITY DASHBOARD");
    println!("{:=<80}", "");
    println!(
        "{:<45} {:>10} {:>10} {:>8}",
        "Chunk", "Builds", "Fields", "Status"
    );
    println!("{:-<80}", "");

    let mut grand_total_correct = 0;
    let mut grand_total_checked = 0;
    let mut chunks_pass = 0;
    let mut chunks_total = 0;

    for &chunk_id in field_groups::all_chunk_ids() {
        let fields = match field_groups::fields_for_chunk(chunk_id) {
            Some(f) if !f.is_empty() => f,
            _ => {
                println!("{chunk_id:<45} {:>10} {:>10} {:>8}", "-", "-", "EMPTY");
                continue;
            }
        };

        chunks_total += 1;
        let mut builds_pass = 0;
        let mut total_correct = 0;
        let mut total_checked = 0;

        for (_, actual, expected) in &build_results {
            let actual_obj = actual.as_object().unwrap();
            let expected_obj = expected.as_object().unwrap();

            let mut build_ok = true;
            for &field in fields {
                let exp = expected_obj.get(field);
                let act = actual_obj.get(field);

                match (act, exp) {
                    (None, None) => {} // not relevant for this build
                    (None, Some(_)) => {
                        total_checked += 1;
                        build_ok = false;
                    }
                    (Some(_), None) => {
                        total_checked += 1;
                        total_correct += 1;
                    }
                    (Some(a), Some(e)) => {
                        total_checked += 1;
                        if compare_value(a, e) {
                            total_correct += 1;
                        } else {
                            build_ok = false;
                        }
                    }
                }
            }

            if build_ok {
                builds_pass += 1;
            }
        }

        grand_total_correct += total_correct;
        grand_total_checked += total_checked;

        let status = if builds_pass == build_results.len() {
            chunks_pass += 1;
            "PASS"
        } else if total_correct > 0 {
            "PARTIAL"
        } else {
            "FAIL"
        };

        println!(
            "{chunk_id:<45} {:>4}/{:<4} {:>4}/{:<4} {:>8}",
            builds_pass,
            build_results.len(),
            total_correct,
            total_checked,
            status
        );
    }

    println!("{:-<80}", "");
    let pct = if grand_total_checked > 0 {
        100.0 * grand_total_correct as f64 / grand_total_checked as f64
    } else {
        0.0
    };
    println!(
        "Chunks: {chunks_pass}/{chunks_total} pass | Fields: {grand_total_correct}/{grand_total_checked} correct ({pct:.1}%)"
    );

    // Per-build summary (how many total fields correct per build)
    println!("\n{:=<80}", "");
    println!("  PER-BUILD SUMMARY");
    println!("{:=<80}", "");
    println!("{:<50} {:>10} {:>10}", "Build", "Correct", "Total");
    println!("{:-<80}", "");

    let mut all_fields_by_build: Vec<(String, usize, usize)> = Vec::new();

    for (build_name, actual, expected) in &build_results {
        let actual_obj = actual.as_object().unwrap();
        let expected_obj = expected.as_object().unwrap();

        let mut correct = 0;
        let total = expected_obj.len();

        for (key, exp_val) in expected_obj {
            if let Some(act_val) = actual_obj.get(key) {
                if compare_value(act_val, exp_val) {
                    correct += 1;
                }
            }
        }

        all_fields_by_build.push((build_name.clone(), correct, total));
    }

    // Sort by parity percentage descending
    all_fields_by_build.sort_by(|a, b| {
        let pct_a = a.1 as f64 / a.2.max(1) as f64;
        let pct_b = b.1 as f64 / b.2.max(1) as f64;
        pct_b.partial_cmp(&pct_a).unwrap()
    });

    for (name, correct, total) in &all_fields_by_build {
        let pct = 100.0 * *correct as f64 / (*total).max(1) as f64;
        println!("{name:<50} {correct:>4}/{total:<4} ({pct:.1}%)");
    }

    println!("{:=<80}", "");
}
