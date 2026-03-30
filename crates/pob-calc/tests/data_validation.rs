//! Integration tests that verify structural completeness of the data files.
//!
//! These tests require real data files and are marked `#[ignore]`.
//! Run with: DATA_DIR=data cargo test -p pob-calc data_validation -- --ignored --nocapture

use pob_calc::data::GameData;
use pob_calc::passive_tree::NodeType;

fn load_game_data() -> GameData {
    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "../../data".to_string());

    let gems_str =
        std::fs::read_to_string(format!("{data_dir}/gems.json")).expect("gems.json not found");
    let misc_str =
        std::fs::read_to_string(format!("{data_dir}/misc.json")).expect("misc.json not found");
    let tree_str = std::fs::read_to_string(format!("{data_dir}/tree/poe1_current.json"))
        .expect("tree/poe1_current.json not found");

    let gems: serde_json::Value = serde_json::from_str(&gems_str).expect("invalid gems.json");
    let misc: serde_json::Value = serde_json::from_str(&misc_str).expect("invalid misc.json");
    let tree: serde_json::Value = serde_json::from_str(&tree_str).expect("invalid tree json");

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
    let json = serde_json::to_string(&combined).expect("failed to serialize combined JSON");
    GameData::from_json(&json).expect("failed to parse game data")
}

#[test]
#[ignore]
fn gems_have_sufficient_count_and_level_data() {
    let data = load_game_data();
    let total = data.gems.len();
    println!("Total gems: {total}");
    assert!(total >= 500, "Expected >= 500 gems, got {total}");

    let with_levels = data.gems.values().filter(|g| !g.levels.is_empty()).count();
    println!("Gems with levels: {with_levels}");
    assert!(
        with_levels >= 400,
        "Expected >= 400 gems with levels, got {with_levels}"
    );

    let support_count = data.gems.values().filter(|g| g.is_support).count();
    println!("Support gems: {support_count}");
    assert!(
        support_count >= 100,
        "Expected >= 100 support gems, got {support_count}"
    );

    let with_skill_types = data
        .gems
        .values()
        .filter(|g| !g.skill_types.is_empty())
        .count();
    println!("Gems with skill_types: {with_skill_types}");
    assert!(
        with_skill_types >= 400,
        "Expected >= 400 gems with skill_types, got {with_skill_types}"
    );
}

#[test]
#[ignore]
fn bases_have_weapon_and_armour_stats() {
    let data = load_game_data();
    let total = data.bases.len();
    println!("Total bases: {total}");
    assert!(total >= 200, "Expected >= 200 bases, got {total}");

    // Known items should exist
    assert!(
        data.bases.get("Rusted Sword").is_some(),
        "Rusted Sword not found in bases"
    );
    assert!(
        data.bases.get("Short Bow").is_some(),
        "Short Bow not found in bases"
    );
    assert!(
        data.bases.get("Plate Vest").is_some(),
        "Plate Vest not found in bases"
    );

    // Rusted Sword should have weapon stats with physical_min > 0
    let rusted_sword = data.bases.get("Rusted Sword").unwrap();
    let weapon = rusted_sword
        .weapon
        .as_ref()
        .expect("Rusted Sword should have weapon data");
    println!(
        "Rusted Sword physical_min: {}, physical_max: {}",
        weapon.physical_min, weapon.physical_max
    );
    assert!(
        weapon.physical_min > 0.0,
        "Rusted Sword physical_min should be > 0, got {}",
        weapon.physical_min
    );
}

#[test]
#[ignore]
fn tree_has_nodes_with_types() {
    let data = load_game_data();
    let total = data.passive_tree.nodes.len();
    println!("Total tree nodes: {total}");
    assert!(total >= 1000, "Expected >= 1000 nodes, got {total}");

    let keystones = data
        .passive_tree
        .nodes
        .values()
        .filter(|n| n.node_type == NodeType::Keystone)
        .count();
    println!("Keystones: {keystones}");
    assert!(keystones >= 20, "Expected >= 20 keystones, got {keystones}");

    let notables = data
        .passive_tree
        .nodes
        .values()
        .filter(|n| n.node_type == NodeType::Notable)
        .count();
    println!("Notables: {notables}");
    assert!(notables >= 100, "Expected >= 100 notables, got {notables}");

    // Stat description coverage from GGPK extraction is currently limited
    // (StatDescriptions.datc64 only covers a subset of nodes). This threshold
    // will increase when the tree data source is upgraded.
    let with_stats = data
        .passive_tree
        .nodes
        .values()
        .filter(|n| !n.stats.is_empty())
        .count();
    println!("Nodes with stats: {with_stats}");
    assert!(
        with_stats >= 1,
        "Expected >= 1 node with stats, got {with_stats}"
    );
}

#[test]
#[ignore]
fn uniques_have_sufficient_count() {
    let data = load_game_data();
    let total = data.uniques.len();
    println!("Total uniques: {total}");
    assert!(total >= 500, "Expected >= 500 uniques, got {total}");
}
