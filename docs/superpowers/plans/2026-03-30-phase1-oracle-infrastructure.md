# Phase 1: Oracle Infrastructure Rebuild — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the oracle test infrastructure so it uses real PoB-generated ground truth, compares ALL output fields, fails on missing keys, and runs in CI.

**Architecture:** Fix `gen_oracle.lua` to reliably produce complete output JSON from the PoB Lua engine. Rewrite `oracle.rs` to use strict comparison across all fields. Create 30+ real-world build XMLs by exporting from PoB, generate their expected JSON via the fixed Lua script, and wire oracle tests into CI.

**Tech Stack:** LuaJIT, Docker, GitHub Actions, Rust test harness, PoB Lua engine (submodule)

**Task order and dependencies:**
1. Fix gen_oracle.lua + add batch generator (Lua/shell)
2. Create 30+ real-world build XMLs (manual PoB export)
3. Generate expected JSON for all builds (runs gen_oracle.lua)
4. Rewrite oracle.rs with strict comparator + macro-based tests (Rust)
5. Create Dockerfile for oracle CI (Docker)
6. Wire oracle tests into GitHub Actions CI (YAML)
7. End-to-end verification (all pieces together)

---

### Task 1: Fix `gen_oracle.lua` and Add Batch Generator

**Files:**
- Modify: `scripts/gen_oracle.lua`
- Create: `scripts/generate_all_oracles.sh`

- [ ] **Step 1: Test that `gen_oracle.lua` runs on the existing melee_str build**

Run from repo root:
```bash
./scripts/run_oracle.sh crates/pob-calc/tests/oracle/melee_str.xml 2>&1
```

If this fails, note the error. Common issues:
- LuaJIT not installed: `brew install luajit` (macOS) or `apt install luajit` (Linux)
- PoB submodule not initialized: `git submodule update --init --recursive`
- Missing C module stubs

If it succeeds, examine the JSON output. Verify it contains many output fields (Life, Mana, Str, Dex, Int, ES, resistances, etc.), not just 2-3.

- [ ] **Step 2: Fix `gen_oracle.lua` stubs if needed**

If Step 1 fails due to missing C modules, add stubs after line 39 in `scripts/gen_oracle.lua`:

```lua
-- lcurl: used for update checking, not needed for calc
package.preload['lcurl'] = function()
    return { easy = function() return {} end }
end
package.preload['lcurl.safe'] = package.preload['lcurl']

-- lzip: used for build import compression
package.preload['lzip'] = function()
    return {
        inflate = function(data) return data end,
        deflate = function(data) return data end,
    }
end
```

If `Deflate`/`Inflate` globals are nil (HeadlessWrapper stubs are TODOs), add before the `dofile(HeadlessWrapper)` call:

```lua
function Inflate(data) return data end
function Deflate(data) return data end
```

Re-run Step 1 to verify the fix works.

- [ ] **Step 3: Create `scripts/generate_all_oracles.sh`**

Write this file:

```bash
#!/bin/bash
# generate_all_oracles.sh: Generate .expected.json for ALL oracle builds.
# Usage: ./scripts/generate_all_oracles.sh
# Requires: luajit, PathOfBuilding submodule initialized

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ORACLE_DIR="$REPO_ROOT/crates/pob-calc/tests/oracle"

if ! command -v luajit &> /dev/null; then
    echo "ERROR: luajit not found. Install with: brew install luajit (macOS) or apt install luajit (Linux)" >&2
    exit 1
fi

if [ ! -f "$REPO_ROOT/third-party/PathOfBuilding/src/HeadlessWrapper.lua" ]; then
    echo "ERROR: PathOfBuilding submodule not initialized. Run: git submodule update --init --recursive" >&2
    exit 1
fi

PASS=0
FAIL=0
TOTAL=0

for xml in "$ORACLE_DIR"/*.xml; do
    name="$(basename "$xml" .xml)"
    expected="$ORACLE_DIR/${name}.expected.json"
    TOTAL=$((TOTAL + 1))

    echo -n "Generating: ${name} ... "

    if output=$("$SCRIPT_DIR/run_oracle.sh" "$xml" 2>/tmp/gen_oracle_stderr.txt); then
        if echo "$output" | python3 -m json.tool > /dev/null 2>&1; then
            echo "$output" > "$expected"
            echo "OK"
            PASS=$((PASS + 1))
        else
            echo "FAIL (invalid JSON)"
            FAIL=$((FAIL + 1))
        fi
    else
        echo "FAIL"
        cat /tmp/gen_oracle_stderr.txt >&2
        FAIL=$((FAIL + 1))
    fi
done

echo ""
echo "Results: ${PASS}/${TOTAL} passed, ${FAIL} failed"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
```

- [ ] **Step 4: Make it executable and smoke test**

```bash
chmod +x scripts/generate_all_oracles.sh
./scripts/generate_all_oracles.sh
```

Expected: Attempts to generate expected JSON for existing oracle XMLs. Note which succeed and which fail.

- [ ] **Step 5: Commit**

```bash
git add scripts/gen_oracle.lua scripts/generate_all_oracles.sh
git commit -m "feat(oracle): fix gen_oracle.lua stubs and add batch oracle generation script"
```

---

### Task 2: Create Real-World Oracle Build XMLs

**Files:**
- Create: 30 new `.xml` files in `crates/pob-calc/tests/oracle/`

This task requires exporting builds from the Path of Building desktop application. Each build must be a real, functional PoE 1 build.

- [ ] **Step 1: Install Path of Building Community Fork**

Download from https://github.com/PathOfBuildingCommunity/PathOfBuilding/releases if not already installed.

- [ ] **Step 2: Create or import 30 builds**

For each archetype, create a build in PoB or import from community sources (poe.ninja, pobb.in). Each build MUST have:
- A full passive tree (100+ allocated nodes)
- At least 8 equipped items (weapon, body, helmet, gloves, boots, belt, 2 rings, amulet)
- At least 1 active skill with 4+ support gems linked
- An ascendancy class (not "None")
- Enemy level set to 84 in Configuration

Export each build as XML (Edit > Share > Generate > Copy XML) and save with these filenames in `crates/pob-calc/tests/oracle/`:

```
realworld_phys_melee_slayer.xml
realworld_ele_melee_raider.xml
realworld_spell_caster_inquisitor.xml
realworld_dot_caster_trickster.xml
realworld_ignite_elementalist.xml
realworld_bleed_gladiator.xml
realworld_poison_pathfinder.xml
realworld_totem_hierophant.xml
realworld_trap_saboteur.xml
realworld_mine_saboteur.xml
realworld_minion_necromancer.xml
realworld_bow_deadeye.xml
realworld_wand_occultist.xml
realworld_champion_impale.xml
realworld_coc_trigger.xml
realworld_cwc_trigger.xml
realworld_dual_wield.xml
realworld_shield_1h.xml
realworld_two_handed.xml
realworld_ci_lowlife_es.xml
realworld_mom_eb.xml
realworld_aura_stacker.xml
realworld_flask_pathfinder.xml
realworld_cluster_jewel.xml
realworld_timeless_jewel.xml
realworld_max_block_gladiator.xml
realworld_rf_juggernaut.xml
realworld_phys_to_fire_conversion.xml
realworld_triple_conversion.xml
realworld_spectre_summoner.xml
```

- [ ] **Step 3: Verify file count**

```bash
ls crates/pob-calc/tests/oracle/realworld_*.xml | wc -l
```

Expected: 30

- [ ] **Step 4: Commit**

```bash
git add crates/pob-calc/tests/oracle/realworld_*.xml
git commit -m "test(oracle): add 30 real-world oracle build XMLs covering all archetypes"
```

---

### Task 3: Generate Expected JSON for All Oracle Builds

**Files:**
- Create: 30 new `.expected.json` files in `crates/pob-calc/tests/oracle/`
- Remove: old trivial builds and their expected JSON

- [ ] **Step 1: Run the batch oracle generator**

```bash
./scripts/generate_all_oracles.sh
```

This produces `.expected.json` for each `.xml`. If any fail, debug and fix gen_oracle.lua.

- [ ] **Step 2: Verify expected JSON quality**

Spot-check a few files:
```bash
python3 -c "
import json, sys
data = json.load(open(sys.argv[1]))
output = data.get('output', {})
print(f'Output fields: {len(output)}')
print(f'Sample: {list(output.keys())[:20]}')
print(f'Breakdown keys: {len(data.get(\"breakdown\", {}))}')
" crates/pob-calc/tests/oracle/realworld_phys_melee_slayer.expected.json
```

Expected: at least 50+ output fields (Life, Mana, ES, Str, Dex, Int, TotalDPS, CritChance, HitChance, Speed, Armour, Evasion, resistances, etc.). If only 2-3 fields, gen_oracle.lua is not collecting all output — go back to Task 1 and fix.

- [ ] **Step 3: Remove old trivial oracle builds**

```bash
git rm crates/pob-calc/tests/oracle/melee_str.xml crates/pob-calc/tests/oracle/melee_str.expected.json
git rm crates/pob-calc/tests/oracle/melee_str_passives.xml crates/pob-calc/tests/oracle/melee_str_passives.expected.json
git rm crates/pob-calc/tests/oracle/crit_spellcaster.xml crates/pob-calc/tests/oracle/crit_spellcaster.expected.json
git rm crates/pob-calc/tests/oracle/ignite_dot.xml crates/pob-calc/tests/oracle/ignite_dot.expected.json
git rm crates/pob-calc/tests/oracle/bleed_dot.xml crates/pob-calc/tests/oracle/bleed_dot.expected.json
git rm crates/pob-calc/tests/oracle/poison_dot.xml crates/pob-calc/tests/oracle/poison_dot.expected.json
git rm crates/pob-calc/tests/oracle/trap_saboteur.xml crates/pob-calc/tests/oracle/trap_saboteur.expected.json
git rm crates/pob-calc/tests/oracle/totem_hierophant.xml crates/pob-calc/tests/oracle/totem_hierophant.expected.json
git rm crates/pob-calc/tests/oracle/mine_detonator.xml crates/pob-calc/tests/oracle/mine_detonator.expected.json
git rm crates/pob-calc/tests/oracle/minion_summoner.xml crates/pob-calc/tests/oracle/minion_summoner.expected.json
git rm crates/pob-calc/tests/oracle/poe2_basic.xml crates/pob-calc/tests/oracle/poe2_basic.expected.json
```

- [ ] **Step 4: Commit**

```bash
git add crates/pob-calc/tests/oracle/realworld_*.expected.json
git commit -m "test(oracle): generate PoB ground-truth JSON for 30 builds, remove old trivial builds

Expected JSON generated by gen_oracle.lua against the actual PoB Lua engine.
Old trivial builds (no items, no tree, hand-written expected values) removed."
```

---

### Task 4: Rewrite `oracle.rs` with Strict Comparator and Macro-Based Tests

**Files:**
- Modify: `crates/pob-calc/tests/oracle.rs` (full rewrite)

- [ ] **Step 1: Write the complete new `oracle.rs`**

Replace the entire file with:

```rust
//! Oracle tests: compare Rust engine output against POB's ground-truth output.
//!
//! Real-world oracle tests are marked #[ignore] and run in CI with DATA_DIR set.
//! Run locally with: DATA_DIR=path/to/data cargo test --test oracle -- --ignored

use pob_calc::{build::parse_xml, calc::calculate, data::GameData};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Data loading helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Comparators
// ---------------------------------------------------------------------------

/// Compare a single numeric output value against expected, allowing 0.1% tolerance.
/// Panics if one side has the key and the other doesn't.
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

/// Strict comparison of ALL output fields between actual and expected.
/// Collects all mismatches and reports them in a single panic.
fn assert_output_strict(actual: &serde_json::Value, expected: &serde_json::Value) {
    let actual_obj = actual.as_object().expect("actual output should be a JSON object");
    let expected_obj = expected.as_object().expect("expected output should be a JSON object");

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
            failures.push(format!("  {key}: unexpected in actual (got {})", actual_obj[key]));
        }
    }

    if !failures.is_empty() {
        let count = failures.len();
        let detail = failures.join("\n");
        panic!("Oracle parity check failed ({count} field(s)):\n{detail}");
    }
}

fn compare_values_soft(
    key: &str,
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

// ---------------------------------------------------------------------------
// Oracle parity runner
// ---------------------------------------------------------------------------

/// Run a full oracle parity check for a named build.
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

// ---------------------------------------------------------------------------
// Parse smoke test — runs without DATA_DIR
// ---------------------------------------------------------------------------

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
            parse_xml(&xml)
                .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", path.display()));
            count += 1;
        }
    }
    assert!(count >= 30, "Expected at least 30 oracle builds, found {count}");
}

// ---------------------------------------------------------------------------
// Full parity tests — require DATA_DIR, run with: cargo test -- --ignored
// ---------------------------------------------------------------------------

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
oracle_test!(oracle_spell_caster_inquisitor, "realworld_spell_caster_inquisitor");
oracle_test!(oracle_dot_caster_trickster, "realworld_dot_caster_trickster");
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
oracle_test!(oracle_phys_to_fire_conversion, "realworld_phys_to_fire_conversion");
oracle_test!(oracle_triple_conversion, "realworld_triple_conversion");
oracle_test!(oracle_spectre_summoner, "realworld_spectre_summoner");

// ---------------------------------------------------------------------------
// Comparator unit tests
// ---------------------------------------------------------------------------

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
```

- [ ] **Step 2: Verify comparator tests pass**

Run: `cargo test --package pob-calc --test oracle comparator_tests -- --nocapture`
Expected: All 7 comparator tests pass.

- [ ] **Step 3: Verify parse smoke test**

Run: `cargo test --package pob-calc --test oracle oracle_all_builds_parse -- --nocapture`
Expected: Passes if 30+ XML files exist. May fail if the XML parser can't handle real-world builds (that's OK — those failures are expected and tracked for Phase 5).

- [ ] **Step 4: Verify ignored oracle tests are listed but not run by default**

Run: `cargo test --package pob-calc --test oracle -- --list 2>&1 | grep -c oracle_`
Expected: At least 30 test names listed.

Run: `cargo test --package pob-calc --test oracle 2>&1 | grep "ignored"`
Expected: Shows "30 ignored" (or similar count).

- [ ] **Step 5: Commit**

```bash
git add crates/pob-calc/tests/oracle.rs
git commit -m "refactor(oracle): rewrite test harness with strict comparison and macro-based tests

- assert_output_strict compares ALL fields, collects all mismatches
- oracle_all_builds_parse smoke test for XML parsing
- 30 oracle parity tests via macro, marked #[ignore]
- Comparator unit tests validate the comparison logic itself
- Replaces old per-build test functions that silently passed on missing keys"
```

---

### Task 5: Create Dockerfile for Oracle CI

**Files:**
- Create: `docker/oracle/Dockerfile`
- Create: `docker/oracle/run_oracle_tests.sh`

- [ ] **Step 1: Create the Docker directory**

```bash
mkdir -p docker/oracle
```

- [ ] **Step 2: Write the Dockerfile**

Create `docker/oracle/Dockerfile`:

```dockerfile
# Runs oracle parity tests: compares Rust engine output against PoB ground truth.
FROM rust:1.77-slim-bookworm

RUN apt-get update && apt-get install -y \
    luajit \
    git \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

RUN cargo build --workspace --exclude pob-wasm --release

ENV DATA_DIR=/app/data

CMD ["cargo", "test", "--package", "pob-calc", "--test", "oracle", "--release", "--", "--ignored", "--nocapture"]
```

- [ ] **Step 3: Write the helper script**

Create `docker/oracle/run_oracle_tests.sh`:

```bash
#!/bin/bash
# Build and run oracle tests in Docker.
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "Building oracle test Docker image..."
docker build -f "$SCRIPT_DIR/Dockerfile" -t pob-wasm-oracle "$REPO_ROOT"

echo "Running oracle tests..."
docker run --rm pob-wasm-oracle
```

```bash
chmod +x docker/oracle/run_oracle_tests.sh
```

- [ ] **Step 4: Test the Docker build locally (optional — requires Docker)**

```bash
./docker/oracle/run_oracle_tests.sh
```

Expected: Docker image builds, oracle tests run. They will FAIL — that's expected. The important thing is they run and produce meaningful failure output.

- [ ] **Step 5: Commit**

```bash
git add docker/oracle/Dockerfile docker/oracle/run_oracle_tests.sh
git commit -m "ci(oracle): add Docker container for running oracle parity tests"
```

---

### Task 6: Wire Oracle Tests into GitHub Actions CI

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Update CI workflow**

Replace `.github/workflows/ci.yml` with:

```yaml
name: CI

on:
  push:
    branches: ["main"]
  pull_request:

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          submodules: recursive

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown

      - name: Install wasm-pack
        run: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

      - name: Install LuaJIT
        run: sudo apt-get install -y luajit

      - name: Run native tests (pob-calc + data-extractor)
        run: cargo test --workspace --exclude pob-wasm

      - name: Build WASM package
        run: wasm-pack build crates/pob-wasm --target web --release

      - name: Run WASM tests in headless Chrome
        run: wasm-pack test --headless --chrome crates/pob-wasm

  oracle:
    runs-on: ubuntu-latest
    continue-on-error: true
    steps:
      - uses: actions/checkout@v4
        with:
          submodules: recursive

      - uses: dtolnay/rust-toolchain@stable

      - name: Install LuaJIT
        run: sudo apt-get install -y luajit

      - name: Run oracle parity tests
        env:
          DATA_DIR: ${{ github.workspace }}/data
        run: cargo test --package pob-calc --test oracle --release -- --ignored --nocapture 2>&1 | tee oracle-results.txt

      - name: Upload oracle results
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: oracle-results
          path: oracle-results.txt
```

- [ ] **Step 2: Validate YAML syntax**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('Valid YAML')"
```

Expected: "Valid YAML"

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add oracle parity test job (continue-on-error until parity reached)

Oracle tests run as a separate job with DATA_DIR set. Uses
continue-on-error so failures are visible but don't block PRs.
Results uploaded as artifact for inspection."
```

---

### Task 7: End-to-End Verification

- [ ] **Step 1: Run the full local oracle pipeline**

```bash
# Generate expected JSON from PoB Lua
./scripts/generate_all_oracles.sh

# Run Rust oracle tests (they will fail — that's expected)
DATA_DIR=$(pwd)/data cargo test --package pob-calc --test oracle -- --ignored --nocapture 2>&1 | tail -50
```

- [ ] **Step 2: Verify failure output is useful**

Oracle test failures should show something like:
```
Oracle parity check failed (47 field(s)):
  Life: expected 5234, got 1118 (tolerance 5.234)
  Mana: expected 1893, got 574 (tolerance 1.893)
  EnergyShield: missing from actual (expected 0)
  Str: missing from actual (expected 214)
  ...
```

This confirms:
1. Strict comparator works (reports ALL mismatches, not just the first)
2. Expected JSON has real PoB values (not hand-written)
3. The Rust engine produces output (even if wrong)
4. Missing fields are reported

- [ ] **Step 3: Record baseline failure count**

```bash
DATA_DIR=$(pwd)/data cargo test --package pob-calc --test oracle -- --ignored 2>&1 | grep "test result"
```

Note the number of failing tests. This is Phase 1's baseline — subsequent phases reduce this toward zero.

- [ ] **Step 4: Run non-ignored tests to verify no regressions**

```bash
cargo test --package pob-calc --test oracle
```

Expected: `oracle_all_builds_parse` passes, comparator tests pass, ignored tests are skipped. No failures.

- [ ] **Step 5: Final commit with any cleanup**

```bash
git add -A
git diff --cached --quiet || git commit -m "chore(oracle): finalize Phase 1 oracle infrastructure"
```

---

## Summary of Files Changed

| Action | File | Purpose |
|--------|------|---------|
| Modify | `scripts/gen_oracle.lua` | Fix C module stubs for headless PoB |
| Create | `scripts/generate_all_oracles.sh` | Batch generate expected JSON |
| Create | 30x `crates/pob-calc/tests/oracle/realworld_*.xml` | Real-world build XMLs |
| Create | 30x `crates/pob-calc/tests/oracle/realworld_*.expected.json` | PoB ground truth |
| Remove | 11x old trivial `.xml` + `.expected.json` pairs | Replaced by real builds |
| Modify | `crates/pob-calc/tests/oracle.rs` | Strict comparator, macro tests, parse smoke test |
| Create | `docker/oracle/Dockerfile` | Docker container for oracle CI |
| Create | `docker/oracle/run_oracle_tests.sh` | Docker helper script |
| Modify | `.github/workflows/ci.yml` | Add oracle CI job |
