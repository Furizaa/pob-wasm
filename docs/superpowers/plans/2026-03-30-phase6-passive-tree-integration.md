# Phase 6: Passive Tree Integration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the passive tree into the calculation engine so that allocated nodes actually affect stats. At the end of this phase, the `melee_str` oracle test passes for Life and Mana within 0.1% tolerance when run with `DATA_DIR=data`.

**Architecture:** Three changes work together: (1) the `data-extractor`'s `tree.rs` must output node `stats` arrays so passive nodes carry their stat strings; (2) `GameData` must load and expose a `PassiveTree`; (3) `calc/setup.rs::add_passive_mods` must iterate allocated nodes, parse each stat string via `item_parser::parse_stat_text`, and add the resulting mods to the player `ModDb`.

**Tech Stack:** Rust 1.82, `pob-calc`, `data-extractor`

**Prerequisites:** Phase 5 complete. `data/tree/poe1_current.json` present (may currently be missing node `stats` arrays — that gets fixed in Task 1).

**Branch:** Create a new worktree and branch `feature/phase6-passive-tree`.

**Reference files (read before each task):**
- `third-party/PathOfBuilding/src/Modules/CalcSetup.lua` — `calcs.buildModListForNode()` lines 113–167, `calcs.buildModListForNodeList()` lines 169–211
- `third-party/PathOfBuilding/src/Modules/ModParser.lua` — the full mod-text-to-mod-struct parser
- `crates/data-extractor/src/transform/tree.rs` — current tree extractor (needs stats arrays)
- `crates/pob-calc/src/passive_tree/mod.rs` — PassiveTree loader (already loads `stats`)
- `crates/pob-calc/src/build/item_parser.rs` — stat text parser (needs more patterns)
- `crates/pob-calc/src/calc/setup.rs` — `add_passive_mods` stub (to be filled in)

---

## File Map

```
crates/data-extractor/src/transform/tree.rs     ← add stats[] array to output JSON
crates/pob-calc/src/data/mod.rs                 ← add passive_tree field to GameData
crates/pob-calc/src/build/item_parser.rs        ← expand stat patterns
crates/pob-calc/src/calc/setup.rs               ← implement add_passive_mods
crates/pob-calc/tests/oracle/melee_str.expected.json  ← verify matches POB output
```

---

### Task 1: Fix data-extractor tree output — add node stats arrays

The current `tree.rs` extractor does not output the `stats` arrays on passive nodes. The JSON it produces has nodes without a `stats` key. `PassiveTree::from_json` already handles a `stats` field — it just never has data.

**Files:**
- Read: `crates/data-extractor/src/transform/tree.rs`
- Modify: `crates/data-extractor/src/transform/tree.rs`

- [ ] **Step 1: Read `crates/data-extractor/src/transform/tree.rs` in full**

```bash
cat crates/data-extractor/src/transform/tree.rs
```

Identify where the node JSON objects are constructed (the `serde_json::json!({ ... })` call that builds each node). The `stats` field will need to be added there. Also identify how string arrays are read from `.datc64` rows — look for existing `read_key_array` usage.

- [ ] **Step 2: Identify the PassiveSkills stat string column offset**

The stat strings in `PassiveSkills.datc64` are stored as a key array (an array of string references). In the `.datc64` format, a key array is read with `dat.read_key_array(row, offset)` and each element is decoded as a UTF-16LE string.

Open the POB export reference at `third-party/PathOfBuilding/src/Export/spec.lua` and search for `PassiveSkills` to find the field schema. The stat-description field is typically named `"Stats"` or `"StatDescriptions"`. Note the byte offset of that field.

```bash
grep -n "PassiveSkills\|Stats\|StatDescription" third-party/PathOfBuilding/src/Export/spec.lua | head -40
```

Record the offset of the stats string-array field.

- [ ] **Step 3: Add stats extraction to the node struct in `tree.rs`**

In `crates/data-extractor/src/transform/tree.rs`, find the struct or block that constructs the per-node JSON. Add a `stats` field that reads the stat string array. The pattern follows the same shape as any other key-array column read.

The exact change depends on what's already in the file (read in Step 1). The target is that the output JSON for each node gains a `"stats": ["...", "..."]` key with the human-readable stat description strings exactly as they appear in POB's passive tree tooltips (e.g. `"+10 to maximum Life"`, `"8% increased maximum Life"`).

If `read_key_array` is not already implemented in `dat64.rs`, check — it is already there. Use it.

- [ ] **Step 4: Re-run the extractor to regenerate `data/tree/poe1_current.json`**

```bash
./scripts/extract.sh /path/to/Content.ggpk
```

If no GGPK is available in the environment, skip regeneration for now and manually add a `stats` field to one node in `data/tree/poe1_current.json` to unblock the unit tests. The oracle test requires real data — set `DATA_DIR=data` only when a real GGPK-extracted tree is present.

- [ ] **Step 5: Verify at least one node has a non-empty stats array**

```bash
python3 -c "
import json, sys
tree = json.load(open('data/tree/poe1_current.json'))
nodes_with_stats = [(k,v) for k,v in tree['nodes'].items() if v.get('stats')]
print(f'{len(nodes_with_stats)} nodes have stats')
print('Example:', nodes_with_stats[:2] if nodes_with_stats else 'NONE - stats extraction failed')
"
```

Expected: a non-zero count. If zero, the offset in Step 3 is wrong — re-read the spec.lua reference and try the adjacent field offset.

- [ ] **Step 6: Commit**

```bash
git add crates/data-extractor/src/transform/tree.rs data/tree/
git commit -m "feat(extractor): add node stats arrays to tree JSON output"
```

---

### Task 2: Load PassiveTree into GameData

`GameData` currently holds only gems and misc. The passive tree is loaded by `PassiveTree::from_json` but is never plumbed into `GameData`, so `setup.rs` cannot access it.

**Files:**
- Modify: `crates/pob-calc/src/data/mod.rs`

- [ ] **Step 1: Write a failing test**

Add to `crates/pob-calc/src/data/mod.rs` (inside the existing `#[cfg(test)] mod tests` block):

```rust
#[test]
fn game_data_includes_passive_tree() {
    let json = r#"{
        "gems": {},
        "misc": {
            "game_constants": {},
            "character_constants": {},
            "monster_life_table": [],
            "monster_damage_table": [],
            "monster_evasion_table": [],
            "monster_accuracy_table": [],
            "monster_ally_life_table": [],
            "monster_ally_damage_table": [],
            "monster_ailment_threshold_table": [],
            "monster_phys_conversion_multi_table": []
        },
        "tree": {
            "nodes": {
                "50459": { "id": 50459, "name": "Thick Skin", "stats": ["+10 to maximum Life"], "out": [] }
            }
        }
    }"#;
    let data = GameData::from_json(json).unwrap();
    assert!(data.passive_tree.nodes.contains_key(&50459));
}
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cargo test -p pob-calc data::tests::game_data_includes_passive_tree 2>&1 | tail -5
```

Expected: compile error — `GameData` has no `passive_tree` field.

- [ ] **Step 3: Add `passive_tree` to `GameData` in `crates/pob-calc/src/data/mod.rs`**

Find the `RawGameData` struct and `GameData` struct. Add the `tree` deserialization field and the `passive_tree` public field:

```rust
use crate::passive_tree::PassiveTree;

#[derive(Deserialize)]
struct RawGameData {
    gems: GemsMap,
    misc: MiscData,
    #[serde(default)]
    tree: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct GameData {
    pub gems: GemsMap,
    pub misc: Arc<MiscData>,
    pub passive_tree: PassiveTree,
}

impl GameData {
    pub fn from_json(json: &str) -> Result<Self, DataError> {
        let raw: RawGameData = serde_json::from_str(json)?;
        let passive_tree = if let Some(tree_val) = raw.tree {
            let tree_json = serde_json::to_string(&tree_val)?;
            PassiveTree::from_json(&tree_json)?
        } else {
            PassiveTree { nodes: std::collections::HashMap::new() }
        };
        Ok(Self {
            gems: raw.gems,
            misc: Arc::new(raw.misc),
            passive_tree,
        })
    }
}
```

Note: `PassiveTree` needs to derive or implement `Clone`. Check `passive_tree/mod.rs` — add `#[derive(Clone)]` to `PassiveTree` and `PassiveNode` if not already present.

- [ ] **Step 4: Run the test**

```bash
cargo test -p pob-calc data
```

Expected: `game_data_includes_passive_tree` passes. All previously passing tests still pass.

- [ ] **Step 5: Run all pob-calc tests**

```bash
cargo test -p pob-calc
```

Expected: no regressions.

- [ ] **Step 6: Commit**

```bash
git add crates/pob-calc/src/data/mod.rs crates/pob-calc/src/passive_tree/mod.rs
git commit -m "feat(calc): add passive_tree field to GameData"
```

---

### Task 3: Expand the stat text parser

`item_parser::parse_stat_text` currently handles about 10 patterns. Passive tree nodes use dozens of patterns. This task expands coverage to handle all stat strings that appear on the allocated nodes of the `melee_str` oracle build (the Marauder base build with no allocated nodes in the current XML — the XML has `nodes=""`). 

Even though `melee_str.xml` has no allocated nodes, the parser must be complete enough to handle common patterns for when we add richer oracle builds. Add the patterns listed below. Each one mirrors a pattern in `ModParser.lua`.

**Files:**
- Modify: `crates/pob-calc/src/build/item_parser.rs`

- [ ] **Step 1: Add the following patterns to `parse_stat_text` in `item_parser.rs`**

Add these `else if` branches to the existing chain (following the same structure as what's already there). Each pattern maps to a `Mod` constructor call.

```rust
// +N to all Attributes
else if let Some(n) = extract_prefix_num(text, "+", " to all Attributes") {
    mods.push(Mod::new_base("Str", n, source.clone()));
    mods.push(Mod::new_base("Dex", n, source.clone()));
    mods.push(Mod::new_base("Int", n, source.clone()));
}
// N% increased Evasion Rating
else if let Some(n) = extract_inc_pattern(text, "Evasion Rating") {
    mods.push(Mod { name: "Evasion".into(), mod_type: ModType::Inc, value: ModValue::Number(n), flags: ModFlags::NONE, keyword_flags: KeywordFlags::NONE, conditions: vec![], source });
}
// +N to maximum Energy Shield
// (already handled — "Energy Shield" mapped to "EnergyShield")
// N% increased Energy Shield
else if let Some(n) = extract_inc_pattern(text, "Energy Shield") {
    mods.push(Mod { name: "EnergyShield".into(), mod_type: ModType::Inc, value: ModValue::Number(n), flags: ModFlags::NONE, keyword_flags: KeywordFlags::NONE, conditions: vec![], source });
}
// N% increased Mana
else if let Some(n) = extract_inc_pattern(text, "Mana") {
    mods.push(Mod { name: "Mana".into(), mod_type: ModType::Inc, value: ModValue::Number(n), flags: ModFlags::NONE, keyword_flags: KeywordFlags::NONE, conditions: vec![], source });
}
// N% increased Strength / Dexterity / Intelligence
else if let Some(n) = extract_inc_pattern(text, "Strength") {
    mods.push(Mod { name: "Str".into(), mod_type: ModType::Inc, value: ModValue::Number(n), flags: ModFlags::NONE, keyword_flags: KeywordFlags::NONE, conditions: vec![], source });
}
else if let Some(n) = extract_inc_pattern(text, "Dexterity") {
    mods.push(Mod { name: "Dex".into(), mod_type: ModType::Inc, value: ModValue::Number(n), flags: ModFlags::NONE, keyword_flags: KeywordFlags::NONE, conditions: vec![], source });
}
else if let Some(n) = extract_inc_pattern(text, "Intelligence") {
    mods.push(Mod { name: "Int".into(), mod_type: ModType::Inc, value: ModValue::Number(n), flags: ModFlags::NONE, keyword_flags: KeywordFlags::NONE, conditions: vec![], source });
}
// N% increased Attack Speed
else if let Some(n) = extract_inc_pattern(text, "Attack Speed") {
    mods.push(Mod { name: "Speed".into(), mod_type: ModType::Inc, value: ModValue::Number(n), flags: ModFlags(ModFlags::ATTACK.0), keyword_flags: KeywordFlags::NONE, conditions: vec![], source });
}
// N% increased Cast Speed
else if let Some(n) = extract_inc_pattern(text, "Cast Speed") {
    mods.push(Mod { name: "Speed".into(), mod_type: ModType::Inc, value: ModValue::Number(n), flags: ModFlags(ModFlags::SPELL.0), keyword_flags: KeywordFlags::NONE, conditions: vec![], source });
}
// N% increased Physical Damage
else if let Some(n) = extract_inc_pattern(text, "Physical Damage") {
    mods.push(Mod { name: "PhysicalDamage".into(), mod_type: ModType::Inc, value: ModValue::Number(n), flags: ModFlags::NONE, keyword_flags: KeywordFlags::NONE, conditions: vec![], source });
}
// N% increased Area of Effect
else if let Some(n) = extract_inc_pattern(text, "Area of Effect") {
    mods.push(Mod { name: "AreaOfEffect".into(), mod_type: ModType::Inc, value: ModValue::Number(n), flags: ModFlags::NONE, keyword_flags: KeywordFlags::NONE, conditions: vec![], source });
}
// N% increased Projectile Speed
else if let Some(n) = extract_inc_pattern(text, "Projectile Speed") {
    mods.push(Mod { name: "ProjectileSpeed".into(), mod_type: ModType::Inc, value: ModValue::Number(n), flags: ModFlags::NONE, keyword_flags: KeywordFlags::NONE, conditions: vec![], source });
}
// +N to Accuracy Rating
else if let Some(n) = extract_prefix_num(text, "+", " to Accuracy Rating") {
    mods.push(Mod::new_base("Accuracy", n, source));
}
// N% increased Accuracy Rating
else if let Some(n) = extract_inc_pattern(text, "Accuracy Rating") {
    mods.push(Mod { name: "Accuracy".into(), mod_type: ModType::Inc, value: ModValue::Number(n), flags: ModFlags::NONE, keyword_flags: KeywordFlags::NONE, conditions: vec![], source });
}
// N% increased Damage
else if let Some(n) = extract_inc_pattern(text, "Damage") {
    mods.push(Mod { name: "Damage".into(), mod_type: ModType::Inc, value: ModValue::Number(n), flags: ModFlags::NONE, keyword_flags: KeywordFlags::NONE, conditions: vec![], source });
}
// +N% to all Elemental Resistances
else if let Some(n) = extract_prefix_num(text, "+", "% to all Elemental Resistances") {
    mods.push(Mod::new_base("FireResist", n, source.clone()));
    mods.push(Mod::new_base("ColdResist", n, source.clone()));
    mods.push(Mod::new_base("LightningResist", n, source));
}
```

- [ ] **Step 2: Add unit tests for the new patterns**

Add to the `#[cfg(test)] mod tests` block in `item_parser.rs`:

```rust
#[test]
fn parses_all_attributes() {
    let mods = parse_stat_text("+10 to all Attributes", src());
    assert_eq!(mods.len(), 3);
    assert!(mods.iter().any(|m| m.name == "Str"));
    assert!(mods.iter().any(|m| m.name == "Dex"));
    assert!(mods.iter().any(|m| m.name == "Int"));
}

#[test]
fn parses_all_elemental_resists() {
    let mods = parse_stat_text("+15% to all Elemental Resistances", src());
    assert_eq!(mods.len(), 3);
    assert!(mods.iter().all(|m| m.value.as_f64() == 15.0));
}

#[test]
fn parses_inc_evasion() {
    let mods = parse_stat_text("12% increased Evasion Rating", src());
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].name, "Evasion");
    assert!(matches!(mods[0].mod_type, crate::mod_db::types::ModType::Inc));
}

#[test]
fn parses_inc_physical_damage() {
    let mods = parse_stat_text("20% increased Physical Damage", src());
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].name, "PhysicalDamage");
}
```

- [ ] **Step 3: Run parser tests**

```bash
cargo test -p pob-calc build::item_parser
```

Expected: all tests pass (including the 4 existing ones and the 4 new ones).

- [ ] **Step 4: Commit**

```bash
git add crates/pob-calc/src/build/item_parser.rs
git commit -m "feat(calc): expand stat text parser with common passive tree patterns"
```

---

### Task 4: Implement `add_passive_mods` in `setup.rs`

This is the core fix. The current `add_passive_mods` is a no-op stub. It must: look up the passive tree from `GameData`, iterate the build's allocated node IDs, retrieve each node's stat strings, parse them into mods via `parse_stat_text`, and add the mods to the player `ModDb`.

**Files:**
- Modify: `crates/pob-calc/src/calc/setup.rs`

- [ ] **Step 1: Write a failing unit test in `setup.rs`**

Add the following test at the bottom of `crates/pob-calc/src/calc/setup.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        build::{parse_xml, types::Build},
        data::GameData,
        mod_db::types::{ModFlags, KeywordFlags, ModType},
    };
    use std::sync::Arc;

    fn make_data_with_node(node_id: u32, stat: &str) -> Arc<GameData> {
        let json = format!(r#"{{
            "gems": {{}},
            "misc": {{
                "game_constants": {{
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
                }},
                "character_constants": {{"life_per_str": 0.5}},
                "monster_life_table": [],
                "monster_damage_table": [],
                "monster_evasion_table": [],
                "monster_accuracy_table": [],
                "monster_ally_life_table": [],
                "monster_ally_damage_table": [],
                "monster_ailment_threshold_table": [],
                "monster_phys_conversion_multi_table": []
            }},
            "tree": {{
                "nodes": {{
                    "{node_id}": {{ "id": {node_id}, "name": "Test Node", "stats": ["{stat}"], "out": [] }}
                }}
            }}
        }}"#);
        Arc::new(GameData::from_json(&json).unwrap())
    }

    fn build_with_node(node_id: u32) -> Build {
        let xml = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="{node_id}" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#);
        parse_xml(&xml).unwrap()
    }

    #[test]
    fn allocated_life_node_increases_life_base() {
        let node_id = 99999u32;
        let data = make_data_with_node(node_id, "+40 to maximum Life");
        let build = build_with_node(node_id);
        let env = init_env(&build, data).unwrap();
        let life_base = env.player.mod_db.sum(
            ModType::Base, "Life", ModFlags::NONE, KeywordFlags::NONE
        );
        // The base class life + 40 from the node
        assert!(life_base > 40.0, "Life base should include node contribution, got {life_base}");
    }

    #[test]
    fn unallocated_node_has_no_effect() {
        let node_id = 99998u32;
        let data = make_data_with_node(node_id, "+40 to maximum Life");
        // Build without that node allocated
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let env = init_env(&build, data).unwrap();
        let life_base = env.player.mod_db.sum(
            ModType::Base, "Life", ModFlags::NONE, KeywordFlags::NONE
        );
        // Without node: base class life only, no +40
        let life_base_no_node_threshold = 200.0; // 38 + 12*90 = 1118 base — if 40 is missing we'd detect it
        // Just assert the sum does NOT include 40 from the node — any value < 200 can't have the node
        // Actually with level 90 base the number will be large. Use tabulate to check no passive source.
        let tabs = env.player.mod_db.tabulate("Life", None, ModFlags::NONE, KeywordFlags::NONE);
        assert!(!tabs.iter().any(|t| t.source_category == "Passive" && t.source_name == "Test Node"),
            "Unallocated node should not contribute to Life");
    }
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

```bash
cargo test -p pob-calc calc::setup 2>&1 | tail -10
```

Expected: `allocated_life_node_increases_life_base` fails (life base does not include node contribution because `add_passive_mods` is a no-op).

- [ ] **Step 3: Implement `add_passive_mods` in `crates/pob-calc/src/calc/setup.rs`**

Replace the current no-op `add_passive_mods` function with:

```rust
fn add_passive_mods(build: &Build, db: &mut ModDb, data: &GameData) {
    for &node_id in &build.passive_spec.allocated_nodes {
        let Some(node) = data.passive_tree.nodes.get(&node_id) else {
            // Node not found in tree data — skip silently (DataError would be too aggressive here)
            continue;
        };
        let source = ModSource::new("Passive", &node.name);
        for stat_text in &node.stats {
            let mods = crate::build::item_parser::parse_stat_text(stat_text, source.clone());
            for m in mods {
                db.add(m);
            }
        }
    }
}
```

Also update the `add_passive_mods` call signature in `init_env` to pass `data`:

```rust
pub fn init_env(build: &Build, data: Arc<GameData>) -> Result<CalcEnv, CalcError> {
    let mut player_db = ModDb::new();
    let enemy_db = ModDb::new();

    add_base_constants(&mut player_db, &data);
    add_class_base_stats(build, &mut player_db, &data);
    add_passive_mods(build, &mut player_db, &data);   // ← was: add_passive_mods(build, &mut player_db)
    add_config_conditions(build, &mut player_db);

    Ok(CalcEnv::new(player_db, enemy_db, data))
}
```

- [ ] **Step 4: Run the tests**

```bash
cargo test -p pob-calc calc::setup
```

Expected: both new tests pass.

- [ ] **Step 5: Run all pob-calc tests**

```bash
cargo test -p pob-calc
```

Expected: no regressions.

- [ ] **Step 6: Commit**

```bash
git add crates/pob-calc/src/calc/setup.rs
git commit -m "feat(calc): implement add_passive_mods — wire PassiveTree into CalcEnv"
```

---

### Task 5: Oracle parity check — `melee_str` Life and Mana

This task verifies the end-to-end result matches POB's output for the `melee_str` oracle build. The `melee_str.xml` build has no allocated passive nodes (nodes="") so passive tree changes do not affect it, but they must not regress it either. The Life/Mana values must match `melee_str.expected.json` within 0.1% tolerance.

**Files:**
- Read: `crates/pob-calc/tests/oracle/melee_str.expected.json`
- Read: `crates/pob-calc/tests/oracle/melee_str.xml`

- [ ] **Step 1: Read the expected output**

```bash
cat crates/pob-calc/tests/oracle/melee_str.expected.json
```

The file should contain `{"output":{"Life":1118,"Mana":574},"breakdown":{}}`. If the content differs (e.g. if POB was re-run), note the actual values.

- [ ] **Step 2: Run the oracle Life test with DATA_DIR**

```bash
DATA_DIR=data cargo test -p pob-calc oracle_melee_str_life_matches_pob -- --nocapture 2>&1 | tail -20
```

Expected outcome: **PASS** — because `melee_str.xml` allocates no passive nodes and the class-base Life formula `38 + 12 * level` gives `38 + 12 * 90 = 1118` for a Marauder at level 90 with 0 Str.

If it fails, the error message will show actual vs expected. Common causes:
- The `add_class_base_stats` formula is wrong — check against POB's `CalcPerform.lua` for the class base life table
- The Str-to-life bonus is being applied differently — check `do_actor_attribs` in `perform.rs`

Fix any discrepancy before proceeding. The test must pass before this phase is complete.

- [ ] **Step 3: Add a new oracle build with allocated passives**

To validate that passive tree integration actually works, create a second oracle build that allocates a few life nodes near the Marauder start.

First, update `melee_str.xml` to allocate 3–5 passive nodes near the Marauder starting area (e.g. nodes `50459`, `47175`, `36634` — these are the Life and Armour cluster nodes adjacent to the Marauder start). Save as a new file `crates/pob-calc/tests/oracle/melee_str_passives.xml`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None">
  </Build>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Cleave" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="50459,47175,36634" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1">
    <ItemSet id="1"/>
  </Items>
  <Config>
    <Input name="enemyLevel" number="84"/>
  </Config>
</PathOfBuilding>
```

- [ ] **Step 4: Generate the oracle expected output for the passives build**

```bash
./scripts/run_oracle.sh crates/pob-calc/tests/oracle/melee_str_passives.xml \
  > crates/pob-calc/tests/oracle/melee_str_passives.expected.json
cat crates/pob-calc/tests/oracle/melee_str_passives.expected.json
```

The Life value in the output will be higher than 1118 due to the passive node contributions. If oracle generation is unavailable (no LuaJIT/Docker), skip this step — the test added in Step 5 will be guarded by `DATA_DIR`.

- [ ] **Step 5: Add oracle test for the passives build**

Add to `crates/pob-calc/tests/oracle.rs`:

```rust
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
```

- [ ] **Step 6: Run the passives oracle test**

```bash
DATA_DIR=data cargo test -p pob-calc oracle_melee_str_passives_life_matches_pob -- --nocapture 2>&1 | tail -10
```

Expected: PASS (or skipped if DATA_DIR not set). If it fails, the passive node stat parsing is producing wrong values — use `--nocapture` to see which stat is off, then trace back through `item_parser` to find the wrong pattern.

- [ ] **Step 7: Run all tests**

```bash
cargo test -p pob-calc
```

Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/pob-calc/tests/oracle/ crates/pob-calc/tests/oracle.rs
git commit -m "test: add melee_str_passives oracle build and parity test"
```

---

### Task 6: Open PR and merge

- [ ] **Step 1: Push branch and open PR**

```bash
git push -u origin feature/phase6-passive-tree
gh pr create --title "feat: Phase 6 — passive tree integration" \
  --body "Wires PassiveTree into CalcEnv so allocated nodes affect stats. Oracle test melee_str passes within 0.1% tolerance. melee_str_passives oracle test added to validate passive contributions."
```

- [ ] **Step 2: Verify CI passes**

```bash
gh pr checks
```

Expected: all checks green.

- [ ] **Step 3: Merge**

```bash
gh pr merge --squash
```

---

**Phase 6 complete** when:
- `cargo test -p pob-calc` passes all tests
- `DATA_DIR=data cargo test -p pob-calc oracle_melee_str_life_matches_pob` passes
- `DATA_DIR=data cargo test -p pob-calc oracle_melee_str_passives_life_matches_pob` passes (or is skipped with explanation if oracle generation unavailable)
- PR merged to main
