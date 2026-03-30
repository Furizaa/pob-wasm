# Phase 4: Data Pipeline (Full Fidelity) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Populate all game data files (gems, bases, mods, tree, uniques) with full-fidelity data so that subsequent phases (item processing, CalcSetup, calc modules) have real stats to work with instead of stubs.

**Architecture:** Use PoB's curated Lua data files as the primary data source — they contain exactly the data PoB's calc engine needs (weapon damage, armour stats, gem levels, skill types, unique items). Build a new `tools/pob-data-extractor/` Rust binary that parses PoB's Lua tables and emits JSON. Supplement with the existing GGPK-based `data-extractor` for data not in PoB's Lua files (passive tree, monster tables, game constants). Expand `GameData` in pob-calc to load all data files. The GGPK extractor remains for tree/misc but its gems/bases/mods outputs are replaced by the PoB Lua-sourced versions.

**Tech Stack:** Rust, `mlua` crate for Lua table evaluation (PoB data files are executable Lua, not static text), serde/serde_json for serialization. Test runner: `cargo test -p pob-calc` and `cargo test -p pob-data-extractor`.

---

## File Structure

```
tools/pob-data-extractor/
  Cargo.toml                — new crate: Lua-based data extractor
  src/
    main.rs                 — CLI: reads PoB Lua data files, writes JSON
    lua_env.rs              — Lua environment setup (stubs for PoB globals)
    extract_gems.rs         — Parse Gems.lua + Skills/*.lua → gems.json
    extract_bases.rs        — Parse Bases/*.lua → bases.json
    extract_uniques.rs      — Parse Uniques/*.lua → uniques.json
    extract_mods.rs         — Parse Mod*.lua → mods.json (item/flask/jewel mod pools)
    types.rs                — Shared output types (GemData, BaseItem, UniqueItem, etc.)

crates/pob-calc/src/data/
  mod.rs          — Modify: expand RawGameData + GameData to include bases, mods, uniques
  gems.rs         — Modify: expand GemData/GemLevelData with new fields, expand SkillTypeFlags
  bases.rs        — Rewrite: full BaseItem types (weapon stats, armour stats, flask stats, etc.)
  uniques.rs      — New: UniqueItem type
  mods.rs         — New: ModPool/ModTier types for item mod data (not to be confused with mod_db)

crates/pob-calc/src/passive_tree/
  mod.rs          — Modify: expand PassiveNode with node_type, ascendancy, mastery, orbit fields

data/
  gems.json       — Replaced: full gem data with levels, skill types, support matching
  bases.json      — Replaced: full base item data with weapon/armour/flask stats
  mods.json       — Replaced: item mod pools with stat values and tiers
  uniques.json    — New: unique item definitions
  misc.json       — Unchanged (already functional from GGPK extractor)
  tree/
    poe1_current.json — Modified: expanded node data with types and orbit info
```

---

### Task 1: Create `pob-data-extractor` crate skeleton

**Files:**
- Create: `tools/pob-data-extractor/Cargo.toml`
- Create: `tools/pob-data-extractor/src/main.rs`
- Create: `tools/pob-data-extractor/src/lua_env.rs`
- Create: `tools/pob-data-extractor/src/types.rs`
- Modify: `Cargo.toml` (workspace root — add member)

- [ ] **Step 1: Add workspace member**

In the root `Cargo.toml`, add `"tools/pob-data-extractor"` to the `members` array:

```toml
members = [
    "crates/pob-calc",
    "crates/pob-wasm",
    "crates/data-extractor",
    "tools/modparser-codegen",
    "tools/pob-data-extractor",
]
```

- [ ] **Step 2: Create `Cargo.toml`**

```toml
[package]
name = "pob-data-extractor"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "pob-data-extractor"
path = "src/main.rs"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
mlua = { version = "0.10", features = ["luajit", "serialize"] }
clap = { version = "4", features = ["derive"] }
regex = "1"
```

- [ ] **Step 3: Create `types.rs` with output data structures**

```rust
use serde::Serialize;
use std::collections::HashMap;

// ── Gems ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct GemMetadata {
    pub id: String,
    pub name: String,
    pub base_type_name: String,
    pub granted_effect_id: String,
    pub tags: Vec<String>,
    pub req_str: u32,
    pub req_dex: u32,
    pub req_int: u32,
    pub is_vaal: bool,
    pub natural_max_level: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillGemData {
    pub id: String,
    pub display_name: String,
    pub is_support: bool,
    pub color: u8,
    pub skill_types: Vec<u32>,
    pub cast_time: f64,
    pub base_effectiveness: f64,
    pub incremental_effectiveness: f64,
    pub base_flags: HashMap<String, bool>,
    pub levels: Vec<SkillLevelData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mana_multiplier_at_20: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub require_skill_types: Vec<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub add_skill_types: Vec<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exclude_skill_types: Vec<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constant_stats: Vec<(String, f64)>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub quality_stats: Vec<(String, f64)>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stats: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillLevelData {
    pub level: u32,
    pub level_requirement: u32,
    pub stat_values: Vec<f64>,
    #[serde(skip_serializing_if = "is_zero")]
    pub crit_chance: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub damage_effectiveness: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub attack_speed_mult: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mana_cost: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub life_cost: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mana_multiplier: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored_uses: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooldown: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

// ── Bases ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct BaseItemData {
    pub name: String,
    pub item_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_type: Option<String>,
    pub socket_limit: u32,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implicit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon: Option<WeaponData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub armour: Option<ArmourData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flask: Option<FlaskData>,
    pub req: BaseRequirements,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeaponData {
    pub physical_min: f64,
    pub physical_max: f64,
    pub crit_chance_base: f64,
    pub attack_rate_base: f64,
    pub range: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArmourData {
    #[serde(skip_serializing_if = "is_zero")]
    pub armour_min: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub armour_max: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub evasion_min: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub evasion_max: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub energy_shield_min: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub energy_shield_max: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub ward_min: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub ward_max: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub block_chance: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub movement_penalty: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlaskData {
    #[serde(skip_serializing_if = "is_zero")]
    pub life: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub mana: f64,
    pub duration: f64,
    pub charges_used: u32,
    pub charges_max: u32,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct BaseRequirements {
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub level: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub str_req: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub dex_req: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub int_req: u32,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

// ── Uniques ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct UniqueItemData {
    pub name: String,
    pub base_type: String,
    pub implicits: Vec<String>,
    pub explicits: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<String>,
}

// ── Item Mods ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ItemModData {
    pub id: String,
    pub mod_type: String,
    pub domain: String,
    pub generation_type: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stats: Vec<ItemModStat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub level_requirement: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemModStat {
    pub stat_id: String,
    pub min: f64,
    pub max: f64,
}
```

- [ ] **Step 4: Create `lua_env.rs` — Lua environment for PoB data files**

```rust
use mlua::{Lua, Result as LuaResult, Value};

/// Create a Lua environment with PoB's required globals stubbed out so that
/// data files (Bases/*.lua, Skills/*.lua, Gems.lua, Uniques/*.lua) can be
/// evaluated without the full PoB runtime.
pub fn create_pob_lua_env(pob_src_dir: &str) -> LuaResult<Lua> {
    let lua = Lua::new();

    // Set up package.path so that require/dofile can find PoB modules
    lua.load(format!(
        r#"package.path = "{pob_src_dir}/?.lua;{pob_src_dir}/?/init.lua;" .. package.path"#
    ))
    .exec()?;

    // Stub the SkillType enum used in Skills/*.lua
    // Values from third-party/PathOfBuilding/src/Data/Global.lua SkillType table
    lua.load(r#"
        SkillType = setmetatable({}, {
            __index = function(t, k)
                -- Auto-assign incrementing IDs for any unknown type
                local id = rawget(t, "_next") or 1
                rawset(t, k, id)
                rawset(t, "_next", id + 1)
                return id
            end
        })
    "#).exec()?;

    // Load the real SkillType table from Global.lua if available
    lua.load(format!(r#"
        local ok, err = pcall(function()
            local f = io.open("{pob_src_dir}/Data/Global.lua", "r")
            if f then
                local content = f:read("*a")
                f:close()
                -- Extract SkillType block
                local block = content:match("SkillType = %{(.-)%}")
                if block then
                    local idx = 1
                    for name in block:gmatch('"([^"]+)"') do
                        SkillType[name] = idx
                        idx = idx + 1
                    end
                end
            end
        end)
    "#)).exec()?;

    // Stub ModFlag and KeywordFlag tables (used in Skills/*.lua statMap)
    lua.load(r#"
        ModFlag = setmetatable({}, { __index = function() return 0 end })
        KeywordFlag = setmetatable({}, { __index = function() return 0 end })
        ModType = setmetatable({}, { __index = function() return "BASE" end })
    "#).exec()?;

    // Stub the mod() and flag() and skill() helper functions used by Skills/*.lua
    lua.load(r#"
        function mod(name, modType, val, flags, keywordFlags, ...)
            return { name = name, modType = modType, value = val,
                     flags = flags or 0, keywordFlags = keywordFlags or 0,
                     tags = {...} }
        end
        function flag(name, ...)
            return mod(name, "FLAG", true, 0, 0, ...)
        end
        function skill(name, val, ...)
            return { skill = true, name = name, value = val }
        end
    "#).exec()?;

    Ok(lua)
}
```

- [ ] **Step 5: Create `main.rs` — CLI skeleton**

```rust
mod lua_env;
mod types;
mod extract_gems;
mod extract_bases;
mod extract_uniques;
mod extract_mods;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about = "Extract game data from PoB Lua files to JSON")]
struct Args {
    /// Path to third-party/PathOfBuilding/src directory
    pob_src: PathBuf,

    /// Output directory for JSON data files
    #[arg(short, long, default_value = "data")]
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    std::fs::create_dir_all(&args.output)?;

    let pob_src = args.pob_src.to_string_lossy().to_string();

    println!("Extracting gem data from PoB Lua...");
    extract_gems::extract(&pob_src, &args.output)?;

    println!("Extracting base item data from PoB Lua...");
    extract_bases::extract(&pob_src, &args.output)?;

    println!("Extracting unique item data from PoB Lua...");
    extract_uniques::extract(&pob_src, &args.output)?;

    println!("Extracting item mod data from PoB Lua...");
    extract_mods::extract(&pob_src, &args.output)?;

    println!("Done. Output written to {}", args.output.display());
    Ok(())
}
```

- [ ] **Step 6: Create stub files for the four extract modules**

Create `extract_gems.rs`, `extract_bases.rs`, `extract_uniques.rs`, `extract_mods.rs` each with:

```rust
use std::path::Path;

pub fn extract(_pob_src: &str, _output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    todo!("implement in subsequent task")
}
```

- [ ] **Step 7: Verify crate compiles**

Run: `cargo check -p pob-data-extractor`
Expected: compiles with no errors (4 warnings about unused stubs are fine)

- [ ] **Step 8: Commit**

```bash
git add tools/pob-data-extractor/ Cargo.toml
git commit -m "feat: scaffold pob-data-extractor crate for Lua-based data extraction"
```

---

### Task 2: Implement gem data extraction (`extract_gems.rs`)

**Files:**
- Modify: `tools/pob-data-extractor/src/extract_gems.rs`
- Modify: `tools/pob-data-extractor/src/lua_env.rs` (if SkillType loading needs refinement)

This task parses `Data/Gems.lua` (gem metadata) and `Data/Skills/*.lua` (skill definitions with levels, stats, skill types) and merges them into a single `gems.json`.

- [ ] **Step 1: Write a test that validates expected gem extraction output**

Add to `extract_gems.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_produces_gems_json() {
        let pob_src = std::env::var("POB_SRC")
            .unwrap_or_else(|_| "third-party/PathOfBuilding/src".to_string());
        if !std::path::Path::new(&pob_src).join("Data/Gems.lua").exists() {
            eprintln!("POB_SRC not available, skipping");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        extract(&pob_src, tmp.path()).unwrap();

        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join("gems.json")).unwrap(),
        ).unwrap();
        let obj = json.as_object().expect("gems.json must be an object");

        // Must have at least 500 gems
        assert!(obj.len() >= 500, "expected >= 500 gems, got {}", obj.len());

        // Fireball must exist with skill data
        let fireball = &obj["Fireball"];
        assert_eq!(fireball["is_support"], false);
        assert!(!fireball["skill_types"].as_array().unwrap().is_empty(),
            "Fireball must have skill_types");
        assert!(!fireball["levels"].as_array().unwrap().is_empty(),
            "Fireball must have level data");
        assert!(fireball["cast_time"].as_f64().unwrap() > 0.0);

        // A support gem must exist
        let supports: Vec<_> = obj.values()
            .filter(|v| v["is_support"] == true)
            .collect();
        assert!(supports.len() >= 100, "expected >= 100 supports, got {}", supports.len());

        // Support must have require_skill_types or be a universal support
        let has_matching = supports.iter().any(|s|
            !s["require_skill_types"].as_array().unwrap_or(&vec![]).is_empty()
        );
        assert!(has_matching, "at least one support must have require_skill_types");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pob-data-extractor extract_produces_gems_json -- --nocapture`
Expected: FAIL — `extract` calls `todo!()`.

- [ ] **Step 3: Implement `extract()` in `extract_gems.rs`**

```rust
use crate::lua_env::create_pob_lua_env;
use crate::types::{SkillGemData, SkillLevelData};
use mlua::{Lua, Value, Table};
use std::collections::HashMap;
use std::path::Path;

pub fn extract(pob_src: &str, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let lua = create_pob_lua_env(pob_src)?;

    // Step 1: Load Gems.lua for metadata (name, tags, requirements)
    let gems_lua_path = format!("{pob_src}/Data/Gems.lua");
    let gem_metadata: Table = lua.load(std::fs::read_to_string(&gems_lua_path)?).eval()?;

    // Build granted_effect_id → gem metadata map
    let mut effect_to_meta: HashMap<String, (String, Vec<String>, u32, u32, u32, bool)> =
        HashMap::new();
    for pair in gem_metadata.pairs::<String, Table>() {
        let (_, entry) = pair?;
        let name: String = entry.get("name").unwrap_or_default();
        let granted_effect_id: String = entry.get("grantedEffectId").unwrap_or_default();
        let is_vaal: bool = entry.get("vaalGem").unwrap_or(false);
        let req_str: u32 = entry.get("reqStr").unwrap_or(0);
        let req_dex: u32 = entry.get("reqDex").unwrap_or(0);
        let req_int: u32 = entry.get("reqInt").unwrap_or(0);

        let tags_table: Option<Table> = entry.get("tags").ok();
        let mut tags = Vec::new();
        if let Some(t) = tags_table {
            for pair in t.pairs::<String, bool>() {
                let (tag_name, val) = pair?;
                if val {
                    tags.push(tag_name);
                }
            }
        }

        if !granted_effect_id.is_empty() {
            effect_to_meta.insert(
                granted_effect_id,
                (name, tags, req_str, req_dex, req_int, is_vaal),
            );
        }
    }

    // Step 2: Load all Skills/*.lua files
    let mut gems: HashMap<String, SkillGemData> = HashMap::new();
    let skill_files = [
        "act_str.lua", "act_dex.lua", "act_int.lua",
        "sup_str.lua", "sup_dex.lua", "sup_int.lua",
        "other.lua", "minion.lua", "glove.lua", "spectre.lua",
    ];

    for file in &skill_files {
        let path = format!("{pob_src}/Data/Skills/{file}");
        if !Path::new(&path).exists() {
            continue;
        }
        let content = std::fs::read_to_string(&path)?;

        // Skills files use varargs: local skills, mod, flag, skill = ...
        // We pass a table to collect skills, plus our stub functions
        let skills_table: Table = lua.create_table()?;
        let mod_fn: mlua::Function = lua.globals().get("mod")?;
        let flag_fn: mlua::Function = lua.globals().get("flag")?;
        let skill_fn: mlua::Function = lua.globals().get("skill")?;

        let chunk = lua.load(content).set_name(file);
        chunk.call::<()>((
            skills_table.clone(),
            mod_fn,
            flag_fn,
            skill_fn,
        ))?;

        // Iterate the collected skills
        for pair in skills_table.pairs::<String, Table>() {
            let (skill_id, skill_def) = pair?;
            let gem_data = parse_skill_definition(&lua, &skill_id, &skill_def, &effect_to_meta)?;
            gems.insert(skill_id, gem_data);
        }
    }

    let json = serde_json::to_string_pretty(&gems)?;
    std::fs::write(output.join("gems.json"), json)?;
    Ok(())
}

fn parse_skill_definition(
    _lua: &Lua,
    skill_id: &str,
    def: &Table,
    meta: &HashMap<String, (String, Vec<String>, u32, u32, u32, bool)>,
) -> Result<SkillGemData, Box<dyn std::error::Error>> {
    let display_name: String = def.get("name").unwrap_or_else(|_| skill_id.to_string());
    let is_support: bool = def.get("support").unwrap_or(false);
    let color: u8 = def.get("color").unwrap_or(0);
    let cast_time: f64 = def.get("castTime").unwrap_or(0.0);
    let base_effectiveness: f64 = def.get("baseEffectiveness").unwrap_or(0.0);
    let incremental_effectiveness: f64 = def.get("incrementalEffectiveness").unwrap_or(0.0);

    // Extract skill types
    let mut skill_types = Vec::new();
    if let Ok(st_table) = def.get::<Table>("skillTypes") {
        for pair in st_table.pairs::<Value, bool>() {
            let (key, val) = pair?;
            if val {
                if let Value::Integer(i) = key {
                    skill_types.push(i as u32);
                }
            }
        }
    }

    // Extract base flags
    let mut base_flags = HashMap::new();
    if let Ok(bf_table) = def.get::<Table>("baseFlags") {
        for pair in bf_table.pairs::<String, bool>() {
            let (k, v) = pair?;
            if v {
                base_flags.insert(k, true);
            }
        }
    }

    // Extract stats list (ordered stat IDs)
    let mut stats = Vec::new();
    if let Ok(stats_table) = def.get::<Table>("stats") {
        for pair in stats_table.pairs::<u32, String>() {
            let (_, stat_id) = pair?;
            stats.push(stat_id);
        }
    }

    // Extract constant stats
    let mut constant_stats = Vec::new();
    if let Ok(cs_table) = def.get::<Table>("constantStats") {
        for pair in cs_table.pairs::<u32, Table>() {
            let (_, entry) = pair?;
            let stat_id: String = entry.get(1).unwrap_or_default();
            let value: f64 = entry.get(2).unwrap_or(0.0);
            if !stat_id.is_empty() {
                constant_stats.push((stat_id, value));
            }
        }
    }

    // Extract quality stats (Default variant)
    let mut quality_stats = Vec::new();
    if let Ok(qs_table) = def.get::<Table>("qualityStats") {
        if let Ok(default_table) = qs_table.get::<Table>("Default") {
            for pair in default_table.pairs::<u32, Table>() {
                let (_, entry) = pair?;
                let stat_id: String = entry.get(1).unwrap_or_default();
                let value: f64 = entry.get(2).unwrap_or(0.0);
                if !stat_id.is_empty() {
                    quality_stats.push((stat_id, value));
                }
            }
        }
    }

    // Support gem matching
    let mut require_skill_types = Vec::new();
    if let Ok(rst_table) = def.get::<Table>("requireSkillTypes") {
        for pair in rst_table.pairs::<u32, u32>() {
            let (_, v) = pair?;
            require_skill_types.push(v);
        }
    }
    let mut add_skill_types = Vec::new();
    if let Ok(ast_table) = def.get::<Table>("addSkillTypes") {
        for pair in ast_table.pairs::<u32, u32>() {
            let (_, v) = pair?;
            add_skill_types.push(v);
        }
    }
    let mut exclude_skill_types = Vec::new();
    if let Ok(est_table) = def.get::<Table>("excludeSkillTypes") {
        for pair in est_table.pairs::<u32, u32>() {
            let (_, v) = pair?;
            exclude_skill_types.push(v);
        }
    }

    // Extract levels table
    let mut levels = Vec::new();
    let mut mana_multiplier_at_20: Option<f64> = None;
    if let Ok(levels_table) = def.get::<Table>("levels") {
        for pair in levels_table.pairs::<u32, Table>() {
            let (level_num, level_entry) = pair? ;

            let level_requirement: u32 = level_entry.get("levelRequirement").unwrap_or(0);
            let crit_chance: f64 = level_entry.get("critChance").unwrap_or(0.0);
            let damage_effectiveness: f64 = level_entry.get("damageEffectiveness").unwrap_or(0.0);
            let attack_speed_mult: f64 = level_entry.get("attackSpeedMultiplier").unwrap_or(0.0);
            let mana_multiplier: Option<f64> = level_entry.get("manaMultiplier").ok();
            let stored_uses: Option<u32> = level_entry.get("storedUses").ok();
            let cooldown: Option<f64> = level_entry.get("cooldown").ok();
            let duration: Option<f64> = level_entry.get("duration").ok();

            // Extract mana/life cost from cost table
            let mut mana_cost = None;
            let mut life_cost = None;
            if let Ok(cost_table) = level_entry.get::<Table>("cost") {
                mana_cost = cost_table.get("Mana").ok();
                life_cost = cost_table.get("Life").ok();
            }

            // Extract positional stat values (indices 1..N matching stats array)
            let mut stat_values = Vec::new();
            for idx in 1..=stats.len() {
                let val: f64 = level_entry.get(idx as u32).unwrap_or(0.0);
                stat_values.push(val);
            }

            if level_num == 20 {
                mana_multiplier_at_20 = mana_multiplier;
            }

            levels.push(SkillLevelData {
                level: level_num,
                level_requirement,
                stat_values,
                crit_chance,
                damage_effectiveness,
                attack_speed_mult,
                mana_cost,
                life_cost,
                mana_multiplier,
                stored_uses,
                cooldown,
                duration,
            });
        }
    }
    levels.sort_by_key(|l| l.level);

    Ok(SkillGemData {
        id: skill_id.to_string(),
        display_name,
        is_support,
        color,
        skill_types,
        cast_time,
        base_effectiveness,
        incremental_effectiveness,
        base_flags,
        levels,
        mana_multiplier_at_20,
        require_skill_types,
        add_skill_types,
        exclude_skill_types,
        constant_stats,
        quality_stats,
        stats,
    })
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p pob-data-extractor extract_produces_gems_json -- --nocapture`
Expected: PASS — gems.json has 500+ gems, Fireball has levels and skill_types, supports have require_skill_types.

- [ ] **Step 5: Commit**

```bash
git add tools/pob-data-extractor/src/extract_gems.rs
git commit -m "feat: implement gem data extraction from PoB Lua skill files"
```

---

### Task 3: Implement base item extraction (`extract_bases.rs`)

**Files:**
- Modify: `tools/pob-data-extractor/src/extract_bases.rs`

Parses `Data/Bases/*.lua` files to produce `bases.json` with full weapon stats, armour stats, flask stats, block chance, and implicits.

- [ ] **Step 1: Write a test for base item extraction**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_produces_bases_json() {
        let pob_src = std::env::var("POB_SRC")
            .unwrap_or_else(|_| "third-party/PathOfBuilding/src".to_string());
        if !std::path::Path::new(&pob_src).join("Data/Bases/sword.lua").exists() {
            eprintln!("POB_SRC not available, skipping");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        extract(&pob_src, tmp.path()).unwrap();

        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join("bases.json")).unwrap(),
        ).unwrap();
        let arr = json.as_array().expect("bases.json must be an array");

        // At least 200 base items
        assert!(arr.len() >= 200, "expected >= 200 bases, got {}", arr.len());

        // Find a sword with weapon data
        let sword = arr.iter().find(|b|
            b["name"].as_str() == Some("Rusted Sword")
        ).expect("Rusted Sword must exist");
        let weapon = sword.get("weapon").expect("sword must have weapon data");
        assert!(weapon["physical_min"].as_f64().unwrap() > 0.0);
        assert!(weapon["physical_max"].as_f64().unwrap() > 0.0);
        assert!(weapon["attack_rate_base"].as_f64().unwrap() > 0.0);
        assert!(weapon["crit_chance_base"].as_f64().unwrap() > 0.0);

        // Find a body armour with armour data
        let body = arr.iter().find(|b|
            b["name"].as_str() == Some("Plate Vest")
        ).expect("Plate Vest must exist");
        let armour = body.get("armour").expect("body armour must have armour data");
        assert!(armour["armour_min"].as_f64().unwrap_or(0.0) > 0.0
            || armour["armour_max"].as_f64().unwrap_or(0.0) > 0.0);

        // Find a shield with block chance
        let shield = arr.iter().find(|b|
            b["item_type"].as_str() == Some("Shield")
            && b.get("armour").map(|a| a["block_chance"].as_f64().unwrap_or(0.0) > 0.0)
                .unwrap_or(false)
        );
        assert!(shield.is_some(), "at least one shield must have block_chance");

        // Find a flask
        let flask = arr.iter().find(|b|
            b["item_type"].as_str() == Some("Flask")
        );
        assert!(flask.is_some(), "at least one flask must exist");
        let flask = flask.unwrap();
        let flask_data = flask.get("flask").expect("flask must have flask data");
        assert!(flask_data["duration"].as_f64().unwrap() > 0.0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pob-data-extractor extract_produces_bases_json -- --nocapture`
Expected: FAIL — `extract` calls `todo!()`.

- [ ] **Step 3: Implement `extract()` in `extract_bases.rs`**

```rust
use crate::lua_env::create_pob_lua_env;
use crate::types::{BaseItemData, WeaponData, ArmourData, FlaskData, BaseRequirements};
use mlua::Table;
use std::path::Path;

pub fn extract(pob_src: &str, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let lua = create_pob_lua_env(pob_src)?;

    let base_files = [
        "sword.lua", "axe.lua", "mace.lua", "dagger.lua", "claw.lua",
        "staff.lua", "wand.lua", "bow.lua",
        "body.lua", "helmet.lua", "gloves.lua", "boots.lua",
        "shield.lua", "quiver.lua",
        "belt.lua", "amulet.lua", "ring.lua",
        "flask.lua", "jewel.lua",
    ];

    let mut all_bases: Vec<BaseItemData> = Vec::new();

    for file in &base_files {
        let path = format!("{pob_src}/Data/Bases/{file}");
        if !Path::new(&path).exists() {
            continue;
        }
        let content = std::fs::read_to_string(&path)?;

        // Bases files use varargs: local itemBases = ...
        let item_bases: Table = lua.create_table()?;
        let chunk = lua.load(content).set_name(*file);
        chunk.call::<()>(item_bases.clone())?;

        for pair in item_bases.pairs::<String, Table>() {
            let (name, entry) = pair?;
            let item_type: String = entry.get("type").unwrap_or_default();
            let sub_type: Option<String> = entry.get("subType").ok();
            let socket_limit: u32 = entry.get("socketLimit").unwrap_or(0);
            let implicit: Option<String> = entry.get("implicit").ok();

            // Tags
            let mut tags = Vec::new();
            if let Ok(tags_table) = entry.get::<Table>("tags") {
                for pair in tags_table.pairs::<String, bool>() {
                    let (tag_name, val) = pair?;
                    if val {
                        tags.push(tag_name);
                    }
                }
            }

            // Weapon data
            let weapon = if let Ok(w) = entry.get::<Table>("weapon") {
                Some(WeaponData {
                    physical_min: w.get("PhysicalMin").unwrap_or(0.0),
                    physical_max: w.get("PhysicalMax").unwrap_or(0.0),
                    crit_chance_base: w.get("CritChanceBase").unwrap_or(0.0),
                    attack_rate_base: w.get("AttackRateBase").unwrap_or(0.0),
                    range: w.get("Range").unwrap_or(0),
                })
            } else {
                None
            };

            // Armour data
            let armour = if let Ok(a) = entry.get::<Table>("armour") {
                Some(ArmourData {
                    armour_min: a.get("ArmourBaseMin").unwrap_or(0.0),
                    armour_max: a.get("ArmourBaseMax").unwrap_or(0.0),
                    evasion_min: a.get("EvasionBaseMin").unwrap_or(0.0),
                    evasion_max: a.get("EvasionBaseMax").unwrap_or(0.0),
                    energy_shield_min: a.get("EnergyShieldBaseMin").unwrap_or(0.0),
                    energy_shield_max: a.get("EnergyShieldBaseMax").unwrap_or(0.0),
                    ward_min: a.get("WardBaseMin").unwrap_or(0.0),
                    ward_max: a.get("WardBaseMax").unwrap_or(0.0),
                    block_chance: a.get("BlockChance").unwrap_or(0.0),
                    movement_penalty: a.get("MovementPenalty").unwrap_or(0.0),
                })
            } else {
                None
            };

            // Flask data
            let flask = if let Ok(f) = entry.get::<Table>("flask") {
                Some(FlaskData {
                    life: f.get("life").unwrap_or(0.0),
                    mana: f.get("mana").unwrap_or(0.0),
                    duration: f.get("duration").unwrap_or(0.0),
                    charges_used: f.get("chargesUsed").unwrap_or(0),
                    charges_max: f.get("chargesMax").unwrap_or(0),
                })
            } else {
                None
            };

            // Requirements
            let req = if let Ok(r) = entry.get::<Table>("req") {
                BaseRequirements {
                    level: r.get("level").unwrap_or(0),
                    str_req: r.get("str").unwrap_or(0),
                    dex_req: r.get("dex").unwrap_or(0),
                    int_req: r.get("int").unwrap_or(0),
                }
            } else {
                BaseRequirements::default()
            };

            all_bases.push(BaseItemData {
                name,
                item_type,
                sub_type,
                socket_limit,
                tags,
                implicit,
                weapon,
                armour,
                flask,
                req,
            });
        }
    }

    all_bases.sort_by(|a, b| a.name.cmp(&b.name));
    let json = serde_json::to_string_pretty(&all_bases)?;
    std::fs::write(output.join("bases.json"), json)?;
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p pob-data-extractor extract_produces_bases_json -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add tools/pob-data-extractor/src/extract_bases.rs
git commit -m "feat: implement base item extraction from PoB Lua data files"
```

---

### Task 4: Implement unique item extraction (`extract_uniques.rs`)

**Files:**
- Modify: `tools/pob-data-extractor/src/extract_uniques.rs`

Parses `Data/Uniques/*.lua` files. Each file returns a Lua array of multi-line strings. Each string is a text-format item definition.

- [ ] **Step 1: Write a test for unique extraction**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_produces_uniques_json() {
        let pob_src = std::env::var("POB_SRC")
            .unwrap_or_else(|_| "third-party/PathOfBuilding/src".to_string());
        if !std::path::Path::new(&pob_src).join("Data/Uniques/sword.lua").exists() {
            eprintln!("POB_SRC not available, skipping");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        extract(&pob_src, tmp.path()).unwrap();

        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join("uniques.json")).unwrap(),
        ).unwrap();
        let arr = json.as_array().expect("uniques.json must be an array");

        // At least 500 unique items
        assert!(arr.len() >= 500, "expected >= 500 uniques, got {}", arr.len());

        // Verify structure of a known unique
        let ahns = arr.iter().find(|u| u["name"].as_str() == Some("Ahn's Might"));
        assert!(ahns.is_some(), "Ahn's Might must exist");
        let ahns = ahns.unwrap();
        assert_eq!(ahns["base_type"].as_str().unwrap(), "Midnight Blade");
        assert!(!ahns["explicits"].as_array().unwrap().is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pob-data-extractor extract_produces_uniques_json -- --nocapture`
Expected: FAIL — `extract` calls `todo!()`.

- [ ] **Step 3: Implement `extract()` in `extract_uniques.rs`**

```rust
use crate::types::UniqueItemData;
use std::path::Path;
use mlua::Lua;

pub fn extract(pob_src: &str, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let lua = Lua::new();

    let unique_files = [
        "sword.lua", "axe.lua", "mace.lua", "dagger.lua", "claw.lua",
        "staff.lua", "wand.lua", "bow.lua",
        "body.lua", "helmet.lua", "gloves.lua", "boots.lua",
        "shield.lua", "quiver.lua",
        "belt.lua", "amulet.lua", "ring.lua",
        "flask.lua", "jewel.lua",
    ];

    let mut all_uniques: Vec<UniqueItemData> = Vec::new();

    for file in &unique_files {
        let path = format!("{pob_src}/Data/Uniques/{file}");
        if !Path::new(&path).exists() {
            continue;
        }
        let content = std::fs::read_to_string(&path)?;
        let items: mlua::Table = lua.load(content).set_name(*file).eval()?;

        for pair in items.pairs::<u32, String>() {
            let (_, text_block) = pair?;
            if let Some(unique) = parse_unique_text(&text_block) {
                all_uniques.push(unique);
            }
        }
    }

    // Also parse Special/ subdirectory if it exists
    let special_dir = format!("{pob_src}/Data/Uniques/Special");
    if Path::new(&special_dir).is_dir() {
        for entry in std::fs::read_dir(&special_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "lua") {
                let content = std::fs::read_to_string(&path)?;
                let file_name = path.file_name().unwrap().to_string_lossy();
                if let Ok(items) = lua.load(content).set_name(file_name.as_ref()).eval::<mlua::Table>() {
                    for pair in items.pairs::<u32, String>() {
                        let (_, text_block) = pair?;
                        if let Some(unique) = parse_unique_text(&text_block) {
                            all_uniques.push(unique);
                        }
                    }
                }
            }
        }
    }

    all_uniques.sort_by(|a, b| a.name.cmp(&b.name));
    let json = serde_json::to_string_pretty(&all_uniques)?;
    std::fs::write(output.join("uniques.json"), json)?;
    Ok(())
}

/// Parse a PoB unique item text block into structured data.
///
/// Format:
/// ```text
/// Name
/// Base Type
/// Variant: Pre 3.5.0          (optional, 0 or more)
/// Variant: Current             (optional)
/// Selected Variant: N          (optional, ignored)
/// League: Breach               (optional, ignored)
/// Has Alt Variant: true        (optional, ignored)
/// Crafted: true                (optional, ignored)
/// Implicits: N
/// implicit line 1
/// ...
/// explicit line 1
/// explicit line 2
/// ```
fn parse_unique_text(text: &str) -> Option<UniqueItemData> {
    let lines: Vec<&str> = text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    if lines.len() < 3 {
        return None;
    }

    let name = lines[0].to_string();
    let base_type = lines[1].to_string();

    let mut variants = Vec::new();
    let mut implicits = Vec::new();
    let mut explicits = Vec::new();
    let mut idx = 2;

    // Parse optional header lines (Variant:, League:, Source:, etc.)
    while idx < lines.len() {
        let line = lines[idx];
        if line.starts_with("Variant:") {
            let variant = line.strip_prefix("Variant: ").unwrap_or("").to_string();
            variants.push(variant);
            idx += 1;
        } else if line.starts_with("Selected Variant:")
            || line.starts_with("League:")
            || line.starts_with("Has Alt Variant:")
            || line.starts_with("Has Alt Variant2:")
            || line.starts_with("Has Alt Variant3:")
            || line.starts_with("Crafted:")
            || line.starts_with("Source:")
            || line.starts_with("LevelReq:")
            || line.starts_with("Quality:")
            || line.starts_with("Sockets:")
        {
            idx += 1;
        } else {
            break;
        }
    }

    // Parse "Implicits: N"
    if idx >= lines.len() {
        return None;
    }
    let implicit_count = if let Some(n_str) = lines[idx].strip_prefix("Implicits: ") {
        idx += 1;
        n_str.parse::<usize>().unwrap_or(0)
    } else {
        0
    };

    // Read implicit lines
    for _ in 0..implicit_count {
        if idx < lines.len() {
            implicits.push(strip_variant_prefix(lines[idx]));
            idx += 1;
        }
    }

    // Remaining lines are explicits
    while idx < lines.len() {
        explicits.push(strip_variant_prefix(lines[idx]));
        idx += 1;
    }

    Some(UniqueItemData {
        name,
        base_type,
        implicits,
        explicits,
        variants,
    })
}

/// Strip `{variant:1,2}` prefixes from mod lines, keeping the mod text.
fn strip_variant_prefix(line: &str) -> String {
    if line.starts_with("{variant:") {
        if let Some(end) = line.find('}') {
            return line[end + 1..].to_string();
        }
    }
    // Also strip {range:X} prefixes
    let mut result = line.to_string();
    while result.starts_with("{") {
        if let Some(end) = result.find('}') {
            result = result[end + 1..].to_string();
        } else {
            break;
        }
    }
    result
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p pob-data-extractor extract_produces_uniques_json -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add tools/pob-data-extractor/src/extract_uniques.rs
git commit -m "feat: implement unique item extraction from PoB Lua data files"
```

---

### Task 5: Implement item mod extraction (`extract_mods.rs`)

**Files:**
- Modify: `tools/pob-data-extractor/src/extract_mods.rs`

Parses PoB's `Data/ModItem.lua`, `Data/ModFlask.lua`, `Data/ModJewel.lua`, `Data/ModJewelAbyss.lua`, `Data/ModJewelCluster.lua`, `Data/ModMaster.lua`, `Data/ModVeiled.lua`. These files define item mod pools with stat text, tiers, and level requirements.

- [ ] **Step 1: Write a test for mod extraction**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_produces_mods_json() {
        let pob_src = std::env::var("POB_SRC")
            .unwrap_or_else(|_| "third-party/PathOfBuilding/src".to_string());
        if !std::path::Path::new(&pob_src).join("Data/ModItem.lua").exists() {
            eprintln!("POB_SRC not available, skipping");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        extract(&pob_src, tmp.path()).unwrap();

        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join("mods.json")).unwrap(),
        ).unwrap();
        let arr = json.as_array().expect("mods.json must be an array");

        // At least 1000 mod entries
        assert!(arr.len() >= 1000, "expected >= 1000 mods, got {}", arr.len());

        // Verify structure
        let first = &arr[0];
        assert!(first.get("id").is_some(), "mod must have id");
        assert!(first.get("mod_type").is_some(), "mod must have mod_type");
        assert!(first.get("domain").is_some(), "mod must have domain");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pob-data-extractor extract_produces_mods_json -- --nocapture`
Expected: FAIL — `extract` calls `todo!()`.

- [ ] **Step 3: Implement `extract()` in `extract_mods.rs`**

```rust
use crate::types::{ItemModData, ItemModStat};
use mlua::{Lua, Table, Value};
use std::path::Path;

pub fn extract(pob_src: &str, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let lua = Lua::new();

    // PoB's mod files are Lua tables returned by the file.
    // Each entry is a table: { "ModName", type = "Prefix"/"Suffix", ... }
    // The stat lines are string values in the array portion.
    let mod_files = [
        ("ModItem.lua", "item"),
        ("ModFlask.lua", "flask"),
        ("ModJewel.lua", "jewel"),
        ("ModJewelAbyss.lua", "jewel_abyss"),
        ("ModJewelCluster.lua", "jewel_cluster"),
        ("ModMaster.lua", "crafted"),
        ("ModVeiled.lua", "veiled"),
    ];

    let mut all_mods: Vec<ItemModData> = Vec::new();
    let mut mod_counter = 0u32;

    for (file, domain) in &mod_files {
        let path = format!("{pob_src}/Data/{file}");
        if !Path::new(&path).exists() {
            continue;
        }
        let content = std::fs::read_to_string(&path)?;

        // These files return a table of tables
        let result: Table = match lua.load(&content).set_name(*file).eval() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Warning: failed to parse {file}: {e}");
                continue;
            }
        };

        for pair in result.pairs::<Value, Table>() {
            let (_, entry) = pair?;

            // First string in the array is the mod ID/name
            let id: String = entry.get(1).unwrap_or_default();
            if id.is_empty() {
                continue;
            }

            let mod_type: String = entry.get("type").unwrap_or_else(|_| "Unknown".to_string());
            let level_requirement: u32 = entry.get("level").unwrap_or(0);
            let group: Option<String> = entry.get("group").ok();

            // Stat lines are the remaining string entries (index 2, 3, ...)
            let mut stats = Vec::new();
            let mut stat_idx = 2u32;
            loop {
                match entry.get::<String>(stat_idx) {
                    Ok(stat_line) if !stat_line.is_empty() => {
                        stats.push(ItemModStat {
                            stat_id: stat_line,
                            min: 0.0,
                            max: 0.0,
                        });
                        stat_idx += 1;
                    }
                    _ => break,
                }
            }

            mod_counter += 1;
            all_mods.push(ItemModData {
                id: format!("{domain}_{mod_counter}_{id}"),
                mod_type,
                domain: domain.to_string(),
                generation_type: String::new(),
                stats,
                group,
                level_requirement,
            });
        }
    }

    let json = serde_json::to_string_pretty(&all_mods)?;
    std::fs::write(output.join("mods.json"), json)?;
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p pob-data-extractor extract_produces_mods_json -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add tools/pob-data-extractor/src/extract_mods.rs
git commit -m "feat: implement item mod pool extraction from PoB Lua data files"
```

---

### Task 6: Expand `PassiveNode` in pob-calc with node type and orbit fields

**Files:**
- Modify: `crates/pob-calc/src/passive_tree/mod.rs`

The tree extractor already writes `is_keystone`, `is_notable`, `is_jewel_socket`, `is_mastery`, `is_ascendancy_start`, `ascendancy_name` to JSON. But pob-calc's `PassiveNode` struct only keeps `id`, `name`, `stats`, `linked_ids`. Expand it to consume the full tree data.

- [ ] **Step 1: Write failing tests for the new fields**

Add to the test module in `passive_tree/mod.rs`:

```rust
#[test]
fn node_types_parsed_from_json() {
    let json = r#"{
        "nodes": {
            "57279": {
                "id": 57279, "name": "Blood Magic",
                "stats": ["Removes all mana. Spend Life instead of Mana for Skills"],
                "out": [],
                "is_keystone": true, "is_notable": false,
                "is_jewel_socket": false, "is_mastery": false,
                "is_ascendancy_start": false, "ascendancy_name": null,
                "icon": "Art/2DArt/SkillIcons/passives/BloodMagicKeystone.png",
                "skill_points_granted": 1
            },
            "40867": {
                "id": 40867, "name": "Bastion of Hope",
                "stats": ["+5% Chance to Block Attack Damage"],
                "out": [],
                "is_keystone": false, "is_notable": true,
                "is_jewel_socket": false, "is_mastery": false,
                "is_ascendancy_start": false,
                "ascendancy_name": "Guardian",
                "icon": "", "skill_points_granted": 1
            },
            "26725": {
                "id": 26725, "name": "",
                "stats": [],
                "out": [57279],
                "is_keystone": false, "is_notable": false,
                "is_jewel_socket": true, "is_mastery": false,
                "is_ascendancy_start": false,
                "ascendancy_name": null,
                "icon": "", "skill_points_granted": 0
            }
        }
    }"#;
    let tree = PassiveTree::from_json(json).unwrap();

    let bm = tree.nodes.get(&57279).unwrap();
    assert_eq!(bm.node_type, NodeType::Keystone);
    assert_eq!(bm.ascendancy_name, None);

    let bastion = tree.nodes.get(&40867).unwrap();
    assert_eq!(bastion.node_type, NodeType::Notable);
    assert_eq!(bastion.ascendancy_name.as_deref(), Some("Guardian"));

    let socket = tree.nodes.get(&26725).unwrap();
    assert_eq!(socket.node_type, NodeType::JewelSocket);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p pob-calc node_types_parsed_from_json`
Expected: FAIL — `NodeType` does not exist yet.

- [ ] **Step 3: Implement the expanded `PassiveNode` and `NodeType`**

Replace the contents of `crates/pob-calc/src/passive_tree/mod.rs`:

```rust
use crate::error::DataError;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum NodeType {
    Small,
    Notable,
    Keystone,
    JewelSocket,
    Mastery,
    AscendancyStart,
    ClassStart,
}

impl Default for NodeType {
    fn default() -> Self {
        NodeType::Small
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RawNode {
    id: u32,
    name: String,
    #[serde(default)]
    stats: Vec<String>,
    #[serde(rename = "out", default)]
    out_ids: Vec<u32>,
    #[serde(default)]
    is_keystone: bool,
    #[serde(default)]
    is_notable: bool,
    #[serde(default)]
    is_jewel_socket: bool,
    #[serde(default)]
    is_mastery: bool,
    #[serde(default)]
    is_ascendancy_start: bool,
    #[serde(default)]
    ascendancy_name: Option<String>,
    #[serde(default)]
    icon: String,
    #[serde(default)]
    skill_points_granted: i32,
}

impl RawNode {
    fn node_type(&self) -> NodeType {
        if self.is_keystone {
            NodeType::Keystone
        } else if self.is_notable {
            NodeType::Notable
        } else if self.is_jewel_socket {
            NodeType::JewelSocket
        } else if self.is_mastery {
            NodeType::Mastery
        } else if self.is_ascendancy_start {
            NodeType::AscendancyStart
        } else {
            NodeType::Small
        }
    }
}

#[derive(Debug, Clone)]
pub struct PassiveNode {
    pub id: u32,
    pub name: String,
    /// Human-readable stat descriptions, e.g. ["+10 to maximum Life"]
    pub stats: Vec<String>,
    /// IDs of nodes this one connects to
    pub linked_ids: Vec<u32>,
    /// The classification of this node
    pub node_type: NodeType,
    /// Which ascendancy class this node belongs to, if any
    pub ascendancy_name: Option<String>,
    /// Asset path for the node icon
    pub icon: String,
    /// How many passive skill points this node grants (usually 1 for allocated, 0 for sockets)
    pub skill_points_granted: i32,
}

#[derive(Debug, Clone)]
pub struct PassiveTree {
    pub nodes: HashMap<u32, PassiveNode>,
}

impl PassiveTree {
    pub fn from_json(json: &str) -> Result<Self, DataError> {
        #[derive(Deserialize)]
        struct Root {
            nodes: HashMap<String, RawNode>,
        }
        let root: Root = serde_json::from_str(json)?;
        let nodes = root
            .nodes
            .into_values()
            .map(|raw| {
                let node = PassiveNode {
                    id: raw.id,
                    name: raw.name,
                    stats: raw.stats,
                    linked_ids: raw.out_ids,
                    node_type: raw.node_type(),
                    ascendancy_name: raw.ascendancy_name,
                    icon: raw.icon,
                    skill_points_granted: raw.skill_points_granted,
                };
                (raw.id, node)
            })
            .collect();
        Ok(Self { nodes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_TREE_JSON: &str = r#"{
        "nodes": {
            "50459": { "id": 50459, "name": "Thick Skin", "stats": ["+10 to maximum Life"], "out": [47175] },
            "47175": { "id": 47175, "name": "Quick Recovery", "stats": [], "out": [] }
        }
    }"#;

    #[test]
    fn loads_nodes_from_json() {
        let tree = PassiveTree::from_json(MINIMAL_TREE_JSON).unwrap();
        assert_eq!(tree.nodes.len(), 2);
        let node = tree.nodes.get(&50459).unwrap();
        assert_eq!(node.name, "Thick Skin");
        assert!(node.linked_ids.contains(&47175));
        assert_eq!(node.node_type, NodeType::Small);
    }

    #[test]
    fn node_types_parsed_from_json() {
        let json = r#"{
            "nodes": {
                "57279": {
                    "id": 57279, "name": "Blood Magic",
                    "stats": ["Removes all mana. Spend Life instead of Mana for Skills"],
                    "out": [],
                    "is_keystone": true, "is_notable": false,
                    "is_jewel_socket": false, "is_mastery": false,
                    "is_ascendancy_start": false, "ascendancy_name": null,
                    "icon": "Art/2DArt/SkillIcons/passives/BloodMagicKeystone.png",
                    "skill_points_granted": 1
                },
                "40867": {
                    "id": 40867, "name": "Bastion of Hope",
                    "stats": ["+5% Chance to Block Attack Damage"],
                    "out": [],
                    "is_keystone": false, "is_notable": true,
                    "is_jewel_socket": false, "is_mastery": false,
                    "is_ascendancy_start": false,
                    "ascendancy_name": "Guardian",
                    "icon": "", "skill_points_granted": 1
                },
                "26725": {
                    "id": 26725, "name": "",
                    "stats": [],
                    "out": [57279],
                    "is_keystone": false, "is_notable": false,
                    "is_jewel_socket": true, "is_mastery": false,
                    "is_ascendancy_start": false,
                    "ascendancy_name": null,
                    "icon": "", "skill_points_granted": 0
                }
            }
        }"#;
        let tree = PassiveTree::from_json(json).unwrap();

        let bm = tree.nodes.get(&57279).unwrap();
        assert_eq!(bm.node_type, NodeType::Keystone);
        assert_eq!(bm.ascendancy_name, None);

        let bastion = tree.nodes.get(&40867).unwrap();
        assert_eq!(bastion.node_type, NodeType::Notable);
        assert_eq!(bastion.ascendancy_name.as_deref(), Some("Guardian"));

        let socket = tree.nodes.get(&26725).unwrap();
        assert_eq!(socket.node_type, NodeType::JewelSocket);
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p pob-calc passive_tree`
Expected: PASS — both `loads_nodes_from_json` and `node_types_parsed_from_json` pass.

- [ ] **Step 5: Verify no regressions in existing tests**

Run: `cargo test -p pob-calc`
Expected: All existing tests pass. The new `PassiveNode` fields use `#[serde(default)]` so the old JSON format still works.

- [ ] **Step 6: Commit**

```bash
git add crates/pob-calc/src/passive_tree/mod.rs
git commit -m "feat: expand PassiveNode with node_type, ascendancy, icon fields"
```

---

### Task 7: Expand `GemData` and `GemLevelData` in pob-calc to match new gems.json

**Files:**
- Modify: `crates/pob-calc/src/data/gems.rs`

The new `gems.json` from pob-data-extractor has richer structure than the current `GemData`. Expand the Rust types to consume the full data.

- [ ] **Step 1: Write failing tests for the new gem fields**

Add to `gems.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gem_data_deserializes_full_fields() {
        let json = r#"{
            "id": "Fireball",
            "display_name": "Fireball",
            "is_support": false,
            "color": 3,
            "skill_types": [2, 3, 4, 7],
            "cast_time": 0.75,
            "base_effectiveness": 0.954,
            "incremental_effectiveness": 0.038,
            "base_flags": { "spell": true, "projectile": true, "area": true },
            "levels": [
                {
                    "level": 1,
                    "level_requirement": 1,
                    "stat_values": [0.5, 0.8],
                    "crit_chance": 6.0,
                    "damage_effectiveness": 2.4,
                    "mana_cost": 6
                }
            ],
            "constant_stats": [["base_is_projectile", 1.0]],
            "quality_stats": [["fire_damage_+%", 1.0]],
            "stats": ["spell_minimum_base_fire_damage", "spell_maximum_base_fire_damage"]
        }"#;
        let gem: GemData = serde_json::from_str(json).unwrap();
        assert_eq!(gem.color, 3);
        assert_eq!(gem.cast_time, 0.75);
        assert_eq!(gem.base_effectiveness, 0.954);
        assert_eq!(gem.skill_types, vec![2, 3, 4, 7]);
        assert_eq!(gem.levels.len(), 1);
        assert_eq!(gem.levels[0].crit_chance, 6.0);
        assert_eq!(gem.levels[0].damage_effectiveness, 2.4);
        assert_eq!(gem.levels[0].mana_cost, Some(6));
        assert_eq!(gem.levels[0].stat_values, vec![0.5, 0.8]);
        assert_eq!(gem.stats.len(), 2);
        assert_eq!(gem.constant_stats.len(), 1);
    }

    #[test]
    fn support_gem_deserializes() {
        let json = r#"{
            "id": "SupportMeleeSplash",
            "display_name": "Melee Splash Support",
            "is_support": true,
            "color": 1,
            "skill_types": [],
            "cast_time": 0.0,
            "base_effectiveness": 0.0,
            "incremental_effectiveness": 0.0,
            "base_flags": {},
            "levels": [
                {
                    "level": 20,
                    "level_requirement": 70,
                    "stat_values": [26],
                    "crit_chance": 0.0,
                    "damage_effectiveness": 0.0,
                    "mana_multiplier": 160.0
                }
            ],
            "mana_multiplier_at_20": 160.0,
            "require_skill_types": [1, 6],
            "stats": ["melee_splash_damage_+%_final"]
        }"#;
        let gem: GemData = serde_json::from_str(json).unwrap();
        assert!(gem.is_support);
        assert_eq!(gem.mana_multiplier_at_20, Some(160.0));
        assert_eq!(gem.require_skill_types, vec![1, 6]);
        assert_eq!(gem.levels[0].mana_multiplier, Some(160.0));
    }

    #[test]
    fn old_gem_format_still_parses() {
        // The old format with just id, display_name, is_support, skill_types, levels
        let json = r#"{
            "id": "Fireball",
            "display_name": "Fireball",
            "is_support": false,
            "skill_types": [2, 3, 4],
            "levels": []
        }"#;
        let gem: GemData = serde_json::from_str(json).unwrap();
        assert_eq!(gem.display_name, "Fireball");
        assert_eq!(gem.color, 0); // defaulted
        assert_eq!(gem.cast_time, 0.0); // defaulted
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p pob-calc gem_data_deserializes_full_fields`
Expected: FAIL — `color`, `cast_time`, `base_effectiveness`, `stat_values`, etc. don't exist on `GemData`/`GemLevelData`.

- [ ] **Step 3: Rewrite `gems.rs` with expanded types**

```rust
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct GemLevelData {
    pub level: u32,
    #[serde(default)]
    pub level_requirement: u32,
    #[serde(default)]
    pub stat_values: Vec<f64>,
    #[serde(default)]
    pub crit_chance: f64,
    #[serde(default)]
    pub damage_effectiveness: f64,
    #[serde(default)]
    pub attack_speed_mult: f64,
    #[serde(default)]
    pub mana_cost: Option<u32>,
    #[serde(default)]
    pub life_cost: Option<u32>,
    #[serde(default)]
    pub mana_multiplier: Option<f64>,
    #[serde(default)]
    pub stored_uses: Option<u32>,
    #[serde(default)]
    pub cooldown: Option<f64>,
    #[serde(default)]
    pub duration: Option<f64>,
    // Legacy fields for backward compatibility with old gems.json
    #[serde(default)]
    pub phys_min: f64,
    #[serde(default)]
    pub phys_max: f64,
    #[serde(default)]
    pub fire_min: f64,
    #[serde(default)]
    pub fire_max: f64,
    #[serde(default)]
    pub cold_min: f64,
    #[serde(default)]
    pub cold_max: f64,
    #[serde(default)]
    pub lightning_min: f64,
    #[serde(default)]
    pub lightning_max: f64,
    #[serde(default)]
    pub chaos_min: f64,
    #[serde(default)]
    pub chaos_max: f64,
    #[serde(default)]
    pub cast_time: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GemData {
    pub id: String,
    pub display_name: String,
    pub is_support: bool,
    #[serde(default)]
    pub color: u8,
    pub skill_types: Vec<u32>,
    #[serde(default)]
    pub cast_time: f64,
    #[serde(default)]
    pub base_effectiveness: f64,
    #[serde(default)]
    pub incremental_effectiveness: f64,
    #[serde(default)]
    pub base_flags: HashMap<String, bool>,
    #[serde(default)]
    pub levels: Vec<GemLevelData>,
    #[serde(default)]
    pub mana_multiplier_at_20: Option<f64>,
    #[serde(default)]
    pub require_skill_types: Vec<u32>,
    #[serde(default)]
    pub add_skill_types: Vec<u32>,
    #[serde(default)]
    pub exclude_skill_types: Vec<u32>,
    #[serde(default)]
    pub constant_stats: Vec<(String, f64)>,
    #[serde(default)]
    pub quality_stats: Vec<(String, f64)>,
    #[serde(default)]
    pub stats: Vec<String>,
}

pub type GemsMap = HashMap<String, GemData>;

/// Mirrors POB's SkillType constants (Common.lua).
/// Used to determine how a skill interacts with the mod system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillTypeFlags(pub u64);

impl SkillTypeFlags {
    pub const NONE: Self = SkillTypeFlags(0);
    pub const ATTACK: Self = SkillTypeFlags(1 << 0);
    pub const SPELL: Self = SkillTypeFlags(1 << 1);
    pub const PROJECTILE: Self = SkillTypeFlags(1 << 2);
    pub const AREA: Self = SkillTypeFlags(1 << 3);
    pub const DURATION: Self = SkillTypeFlags(1 << 4);
    pub const MELEE: Self = SkillTypeFlags(1 << 5);
    pub const DAMAGE: Self = SkillTypeFlags(1 << 6);
    pub const TOTEM: Self = SkillTypeFlags(1 << 7);
    pub const TRAP: Self = SkillTypeFlags(1 << 8);
    pub const MINE: Self = SkillTypeFlags(1 << 9);
    pub const MINION: Self = SkillTypeFlags(1 << 10);
    pub const CHANNELLING: Self = SkillTypeFlags(1 << 11);
    pub const VAAL: Self = SkillTypeFlags(1 << 12);
    pub const AURA: Self = SkillTypeFlags(1 << 13);
    pub const HERALD: Self = SkillTypeFlags(1 << 14);
    pub const CURSE: Self = SkillTypeFlags(1 << 15);
    pub const WARCRY: Self = SkillTypeFlags(1 << 16);
    pub const MOVEMENT: Self = SkillTypeFlags(1 << 17);
    pub const GUARD: Self = SkillTypeFlags(1 << 18);
    pub const TRAVEL: Self = SkillTypeFlags(1 << 19);
    pub const BLINK: Self = SkillTypeFlags(1 << 20);
    pub const BRAND: Self = SkillTypeFlags(1 << 21);
    pub const TRIGGER: Self = SkillTypeFlags(1 << 22);
    pub const DOT: Self = SkillTypeFlags(1 << 23);
    pub const CREATES_MINION: Self = SkillTypeFlags(1 << 24);

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    pub fn from_vec(types: &[u32]) -> Self {
        let mut bits = 0u64;
        for &t in types {
            if t > 0 && t <= 64 {
                bits |= 1 << (t - 1);
            }
        }
        SkillTypeFlags(bits)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p pob-calc gems`
Expected: PASS — all three tests pass including backward-compatibility test.

- [ ] **Step 5: Verify no regressions**

Run: `cargo test -p pob-calc`
Expected: All existing tests pass. The new fields use `#[serde(default)]` so old JSON still works.

- [ ] **Step 6: Commit**

```bash
git add crates/pob-calc/src/data/gems.rs
git commit -m "feat: expand GemData with full skill fields (levels, stats, support matching)"
```

---

### Task 8: Create `bases.rs` data types in pob-calc

**Files:**
- Modify: `crates/pob-calc/src/data/bases.rs`

Replace the stub with proper Rust types matching the new `bases.json` output from pob-data-extractor.

- [ ] **Step 1: Write tests for base item deserialization**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_base_deserializes() {
        let json = r#"{
            "name": "Rusted Sword",
            "item_type": "One Handed Sword",
            "socket_limit": 3,
            "tags": ["sword", "weapon", "onehand"],
            "implicit": "40% increased Global Accuracy Rating",
            "weapon": {
                "physical_min": 4.0,
                "physical_max": 9.0,
                "crit_chance_base": 5.0,
                "attack_rate_base": 1.55,
                "range": 11
            },
            "req": { "str_req": 8, "dex_req": 8 }
        }"#;
        let base: BaseItemData = serde_json::from_str(json).unwrap();
        assert_eq!(base.name, "Rusted Sword");
        let w = base.weapon.unwrap();
        assert_eq!(w.physical_min, 4.0);
        assert_eq!(w.attack_rate_base, 1.55);
    }

    #[test]
    fn armour_base_deserializes() {
        let json = r#"{
            "name": "Plate Vest",
            "item_type": "Body Armour",
            "sub_type": "Armour",
            "socket_limit": 6,
            "tags": ["armour", "body_armour"],
            "armour": {
                "armour_min": 12.0,
                "armour_max": 15.0,
                "block_chance": 0.0,
                "movement_penalty": 3.0
            },
            "req": { "level": 1, "str_req": 14 }
        }"#;
        let base: BaseItemData = serde_json::from_str(json).unwrap();
        let a = base.armour.unwrap();
        assert!(a.armour_min > 0.0);
        assert_eq!(a.movement_penalty, 3.0);
    }

    #[test]
    fn flask_base_deserializes() {
        let json = r#"{
            "name": "Small Life Flask",
            "item_type": "Flask",
            "sub_type": "Life",
            "socket_limit": 0,
            "tags": ["flask"],
            "flask": {
                "life": 70.0,
                "mana": 0.0,
                "duration": 3.0,
                "charges_used": 7,
                "charges_max": 21
            },
            "req": {}
        }"#;
        let base: BaseItemData = serde_json::from_str(json).unwrap();
        let f = base.flask.unwrap();
        assert_eq!(f.life, 70.0);
        assert_eq!(f.duration, 3.0);
    }

    #[test]
    fn bases_map_lookup() {
        let json = r#"[
            {"name": "Rusted Sword", "item_type": "One Handed Sword", "socket_limit": 3, "tags": [], "req": {}},
            {"name": "Plate Vest", "item_type": "Body Armour", "socket_limit": 6, "tags": [], "req": {}}
        ]"#;
        let bases: Vec<BaseItemData> = serde_json::from_str(json).unwrap();
        let map = BaseItemMap::from_vec(bases);
        assert!(map.get("Rusted Sword").is_some());
        assert!(map.get("Plate Vest").is_some());
        assert!(map.get("NonExistent").is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p pob-calc weapon_base_deserializes`
Expected: FAIL — `BaseItemData` does not exist.

- [ ] **Step 3: Implement `bases.rs`**

```rust
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct BaseItemData {
    pub name: String,
    pub item_type: String,
    #[serde(default)]
    pub sub_type: Option<String>,
    #[serde(default)]
    pub socket_limit: u32,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub implicit: Option<String>,
    #[serde(default)]
    pub weapon: Option<WeaponStats>,
    #[serde(default)]
    pub armour: Option<ArmourStats>,
    #[serde(default)]
    pub flask: Option<FlaskStats>,
    #[serde(default)]
    pub req: BaseRequirements,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeaponStats {
    #[serde(default)]
    pub physical_min: f64,
    #[serde(default)]
    pub physical_max: f64,
    #[serde(default)]
    pub crit_chance_base: f64,
    #[serde(default)]
    pub attack_rate_base: f64,
    #[serde(default)]
    pub range: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArmourStats {
    #[serde(default)]
    pub armour_min: f64,
    #[serde(default)]
    pub armour_max: f64,
    #[serde(default)]
    pub evasion_min: f64,
    #[serde(default)]
    pub evasion_max: f64,
    #[serde(default)]
    pub energy_shield_min: f64,
    #[serde(default)]
    pub energy_shield_max: f64,
    #[serde(default)]
    pub ward_min: f64,
    #[serde(default)]
    pub ward_max: f64,
    #[serde(default)]
    pub block_chance: f64,
    #[serde(default)]
    pub movement_penalty: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FlaskStats {
    #[serde(default)]
    pub life: f64,
    #[serde(default)]
    pub mana: f64,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub charges_used: u32,
    #[serde(default)]
    pub charges_max: u32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BaseRequirements {
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub str_req: u32,
    #[serde(default)]
    pub dex_req: u32,
    #[serde(default)]
    pub int_req: u32,
}

/// Lookup map for base items by name.
#[derive(Debug, Clone)]
pub struct BaseItemMap {
    items: HashMap<String, BaseItemData>,
}

impl BaseItemMap {
    pub fn from_vec(bases: Vec<BaseItemData>) -> Self {
        let items = bases.into_iter().map(|b| (b.name.clone(), b)).collect();
        Self { items }
    }

    pub fn get(&self, name: &str) -> Option<&BaseItemData> {
        self.items.get(name)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p pob-calc bases`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/pob-calc/src/data/bases.rs
git commit -m "feat: implement BaseItemData types with weapon/armour/flask stats"
```

---

### Task 9: Create `uniques.rs` data types in pob-calc

**Files:**
- Create: `crates/pob-calc/src/data/uniques.rs`
- Modify: `crates/pob-calc/src/data/mod.rs` (add `pub mod uniques;`)

- [ ] **Step 1: Write tests for unique item deserialization**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_item_deserializes() {
        let json = r#"{
            "name": "Ahn's Might",
            "base_type": "Midnight Blade",
            "implicits": ["40% increased Global Accuracy Rating"],
            "explicits": [
                "Adds (80-115) to (150-205) Physical Damage",
                "(15-25)% increased Critical Strike Chance",
                "-1 to Maximum Frenzy Charges"
            ],
            "variants": ["Pre 3.5.0", "Current"]
        }"#;
        let unique: UniqueItemData = serde_json::from_str(json).unwrap();
        assert_eq!(unique.name, "Ahn's Might");
        assert_eq!(unique.base_type, "Midnight Blade");
        assert_eq!(unique.implicits.len(), 1);
        assert_eq!(unique.explicits.len(), 3);
        assert_eq!(unique.variants.len(), 2);
    }

    #[test]
    fn unique_map_lookup() {
        let json = r#"[
            {"name": "Ahn's Might", "base_type": "Midnight Blade", "implicits": [], "explicits": []},
            {"name": "Beltimber Blade", "base_type": "Eternal Sword", "implicits": [], "explicits": []}
        ]"#;
        let uniques: Vec<UniqueItemData> = serde_json::from_str(json).unwrap();
        let map = UniqueItemMap::from_vec(uniques);
        assert!(map.get("Ahn's Might").is_some());
        assert!(map.get("NonExistent").is_none());
    }
}
```

- [ ] **Step 2: Implement `uniques.rs`**

```rust
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct UniqueItemData {
    pub name: String,
    pub base_type: String,
    #[serde(default)]
    pub implicits: Vec<String>,
    #[serde(default)]
    pub explicits: Vec<String>,
    #[serde(default)]
    pub variants: Vec<String>,
}

/// Lookup map for unique items by name.
#[derive(Debug, Clone)]
pub struct UniqueItemMap {
    items: HashMap<String, UniqueItemData>,
}

impl UniqueItemMap {
    pub fn from_vec(uniques: Vec<UniqueItemData>) -> Self {
        let items = uniques.into_iter().map(|u| (u.name.clone(), u)).collect();
        Self { items }
    }

    pub fn get(&self, name: &str) -> Option<&UniqueItemData> {
        self.items.get(name)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
```

- [ ] **Step 3: Add `pub mod uniques;` to `data/mod.rs`**

Add after the `pub mod misc;` line:

```rust
pub mod uniques;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p pob-calc uniques`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/pob-calc/src/data/uniques.rs crates/pob-calc/src/data/mod.rs
git commit -m "feat: add UniqueItemData type for unique item definitions"
```

---

### Task 10: Expand `GameData` and `from_json()` to load all data files

**Files:**
- Modify: `crates/pob-calc/src/data/mod.rs`

Currently `GameData` only holds `gems`, `misc`, and `passive_tree`. Add `bases`, `uniques`. Update `RawGameData` deserialization and `build_real_game_data_json()` in oracle tests.

- [ ] **Step 1: Write failing test for expanded GameData**

Add to the test module in `data/mod.rs`:

```rust
#[test]
fn game_data_includes_bases_and_uniques() {
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
        "bases": [
            {"name": "Rusted Sword", "item_type": "One Handed Sword", "socket_limit": 3, "tags": [], "req": {}}
        ],
        "uniques": [
            {"name": "Ahn's Might", "base_type": "Midnight Blade", "implicits": [], "explicits": []}
        ]
    }"#;
    let data = GameData::from_json(json).unwrap();
    assert!(data.bases.get("Rusted Sword").is_some());
    assert!(data.uniques.get("Ahn's Might").is_some());
}

#[test]
fn game_data_missing_bases_defaults_empty() {
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
        }
    }"#;
    let data = GameData::from_json(json).unwrap();
    assert!(data.bases.is_empty());
    assert!(data.uniques.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p pob-calc game_data_includes_bases`
Expected: FAIL — `data.bases` does not exist on `GameData`.

- [ ] **Step 3: Update `data/mod.rs`**

```rust
pub mod bases;
pub mod gems;
pub mod misc;
pub mod uniques;

use crate::error::DataError;
use crate::passive_tree::PassiveTree;
use bases::{BaseItemData, BaseItemMap};
use gems::GemsMap;
use misc::MiscData;
use uniques::{UniqueItemData, UniqueItemMap};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct RawGameData {
    gems: GemsMap,
    misc: MiscData,
    #[serde(default)]
    tree: Option<serde_json::Value>,
    #[serde(default)]
    bases: Option<Vec<BaseItemData>>,
    #[serde(default)]
    uniques: Option<Vec<UniqueItemData>>,
}

/// Immutable game data shared across all calculations.
/// Loaded once at startup from the JSON files produced by data-extractor.
#[derive(Debug, Clone)]
pub struct GameData {
    pub gems: GemsMap,
    pub misc: Arc<MiscData>,
    pub passive_tree: PassiveTree,
    pub bases: BaseItemMap,
    pub uniques: UniqueItemMap,
}

impl GameData {
    /// Parse a combined JSON string containing all game data sections.
    /// The JSON structure matches what the data extractors produce.
    pub fn from_json(json: &str) -> Result<Self, DataError> {
        let raw: RawGameData = serde_json::from_str(json)?;
        let passive_tree = if let Some(tree_val) = raw.tree {
            let tree_json = serde_json::to_string(&tree_val)?;
            PassiveTree::from_json(&tree_json)?
        } else {
            PassiveTree {
                nodes: std::collections::HashMap::new(),
            }
        };
        let bases = BaseItemMap::from_vec(raw.bases.unwrap_or_default());
        let uniques = UniqueItemMap::from_vec(raw.uniques.unwrap_or_default());
        Ok(Self {
            gems: raw.gems,
            misc: Arc::new(raw.misc),
            passive_tree,
            bases,
            uniques,
        })
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p pob-calc game_data`
Expected: PASS — both new tests and existing tests pass (bases/uniques default to empty when absent).

- [ ] **Step 5: Commit**

```bash
git add crates/pob-calc/src/data/mod.rs
git commit -m "feat: expand GameData to load bases and uniques"
```

---

### Task 11: Update oracle test `build_real_game_data_json` to load new data files

**Files:**
- Modify: `crates/pob-calc/tests/oracle.rs`

The helper function currently only reads `gems.json`, `misc.json`, `tree/poe1_current.json`. Add `bases.json` and `uniques.json`.

- [ ] **Step 1: Update `build_real_game_data_json()`**

Find the function in `oracle.rs` and replace it:

```rust
fn build_real_game_data_json(data_dir: &str) -> Result<String, Box<dyn std::error::Error>> {
    let gems_str = std::fs::read_to_string(format!("{data_dir}/gems.json"))?;
    let misc_str = std::fs::read_to_string(format!("{data_dir}/misc.json"))?;
    let tree_str = std::fs::read_to_string(format!("{data_dir}/tree/poe1_current.json"))?;

    let gems: serde_json::Value = serde_json::from_str(&gems_str)?;
    let misc: serde_json::Value = serde_json::from_str(&misc_str)?;
    let tree: serde_json::Value = serde_json::from_str(&tree_str)?;

    // Load bases and uniques if available (graceful fallback for old data dirs)
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
```

- [ ] **Step 2: Run non-ignored oracle tests**

Run: `cargo test -p pob-calc oracle_all_builds_parse`
Expected: PASS (this test doesn't use `build_real_game_data_json` so it validates no compilation errors)

- [ ] **Step 3: Verify full test suite passes**

Run: `cargo test -p pob-calc`
Expected: All non-ignored tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/pob-calc/tests/oracle.rs
git commit -m "feat: oracle tests load bases.json and uniques.json"
```

---

### Task 12: Run the pob-data-extractor and generate data files

**Files:**
- Modified (on disk): `data/gems.json`, `data/bases.json`, `data/uniques.json`, `data/mods.json`

This task runs the new extractor tool against the PoB submodule to produce real data files.

- [ ] **Step 1: Build the extractor**

Run: `cargo build -p pob-data-extractor --release`
Expected: Successful build

- [ ] **Step 2: Run the extractor**

Run: `./target/release/pob-data-extractor third-party/PathOfBuilding/src --output data`
Expected: Output like:
```
Extracting gem data from PoB Lua...
Extracting base item data from PoB Lua...
Extracting unique item data from PoB Lua...
Extracting item mod data from PoB Lua...
Done. Output written to data
```

- [ ] **Step 3: Verify gems.json**

Run: `cargo test -p pob-data-extractor extract_produces_gems_json -- --nocapture`
Expected: PASS — gems.json has 500+ gems with level data.

- [ ] **Step 4: Verify bases.json**

Run: `cargo test -p pob-data-extractor extract_produces_bases_json -- --nocapture`
Expected: PASS — bases.json has 200+ bases with weapon/armour stats.

- [ ] **Step 5: Verify uniques.json**

Run: `cargo test -p pob-data-extractor extract_produces_uniques_json -- --nocapture`
Expected: PASS — uniques.json has 500+ unique items.

- [ ] **Step 6: Verify pob-calc loads the new data**

Run: `DATA_DIR=data cargo test -p pob-calc oracle_all_builds_parse -- --nocapture`
Expected: PASS — all 30+ build XMLs parse and GameData loads with populated bases/uniques.

- [ ] **Step 7: Commit the generated data files**

```bash
git add data/gems.json data/bases.json data/uniques.json data/mods.json
git commit -m "data: regenerate all JSON from PoB Lua data files"
```

---

### Task 13: Data validation tests in pob-calc

**Files:**
- Create: `crates/pob-calc/tests/data_validation.rs`

Spec requirement 4.6: automated tests that verify structural completeness of all JSON data files.

- [ ] **Step 1: Write validation tests**

```rust
//! Data validation tests — verify that data files contain expected
//! minimums for structural completeness.

use std::sync::Arc;

fn load_game_data() -> pob_calc::data::GameData {
    let data_dir = std::env::var("DATA_DIR")
        .unwrap_or_else(|_| "../../data".to_string());
    let gems_str = std::fs::read_to_string(format!("{data_dir}/gems.json"))
        .expect("gems.json not found — set DATA_DIR");
    let misc_str = std::fs::read_to_string(format!("{data_dir}/misc.json"))
        .expect("misc.json not found");
    let tree_str = std::fs::read_to_string(format!("{data_dir}/tree/poe1_current.json"))
        .expect("tree/poe1_current.json not found");
    let bases_str = std::fs::read_to_string(format!("{data_dir}/bases.json"))
        .unwrap_or_else(|_| "[]".to_string());
    let uniques_str = std::fs::read_to_string(format!("{data_dir}/uniques.json"))
        .unwrap_or_else(|_| "[]".to_string());

    let gems: serde_json::Value = serde_json::from_str(&gems_str).unwrap();
    let misc: serde_json::Value = serde_json::from_str(&misc_str).unwrap();
    let tree: serde_json::Value = serde_json::from_str(&tree_str).unwrap();
    let bases: serde_json::Value = serde_json::from_str(&bases_str).unwrap();
    let uniques: serde_json::Value = serde_json::from_str(&uniques_str).unwrap();

    let combined = serde_json::json!({
        "gems": gems, "misc": misc, "tree": tree,
        "bases": bases, "uniques": uniques,
    });
    pob_calc::data::GameData::from_json(&serde_json::to_string(&combined).unwrap()).unwrap()
}

#[test]
#[ignore] // requires DATA_DIR
fn gems_have_sufficient_count_and_level_data() {
    let data = load_game_data();
    assert!(data.gems.len() >= 500,
        "expected >= 500 gems, got {}", data.gems.len());

    let with_levels: usize = data.gems.values()
        .filter(|g| !g.levels.is_empty())
        .count();
    assert!(with_levels >= 400,
        "expected >= 400 gems with level data, got {with_levels}");

    let supports: usize = data.gems.values()
        .filter(|g| g.is_support)
        .count();
    assert!(supports >= 100,
        "expected >= 100 support gems, got {supports}");

    let with_skill_types: usize = data.gems.values()
        .filter(|g| !g.skill_types.is_empty())
        .count();
    assert!(with_skill_types >= 400,
        "expected >= 400 gems with skill_types, got {with_skill_types}");
}

#[test]
#[ignore] // requires DATA_DIR
fn bases_have_weapon_and_armour_stats() {
    let data = load_game_data();
    assert!(data.bases.len() >= 200,
        "expected >= 200 bases, got {}", data.bases.len());

    let weapon_count = (0..data.bases.len())
        .filter_map(|_| None::<()>) // We need iteration — use get with known names
        .count();
    // Instead, just verify known items exist
    assert!(data.bases.get("Rusted Sword").is_some(), "Rusted Sword must exist");
    assert!(data.bases.get("Short Bow").is_some(), "Short Bow must exist");
    assert!(data.bases.get("Plate Vest").is_some(), "Plate Vest must exist");

    let sword = data.bases.get("Rusted Sword").unwrap();
    assert!(sword.weapon.is_some(), "Rusted Sword must have weapon stats");
    let w = sword.weapon.as_ref().unwrap();
    assert!(w.physical_min > 0.0, "weapon must have physical_min > 0");
    assert!(w.physical_max > 0.0, "weapon must have physical_max > 0");
    assert!(w.attack_rate_base > 0.0, "weapon must have attack_rate_base > 0");
}

#[test]
#[ignore] // requires DATA_DIR
fn tree_has_nodes_with_types() {
    let data = load_game_data();
    assert!(data.passive_tree.nodes.len() >= 1000,
        "expected >= 1000 tree nodes, got {}", data.passive_tree.nodes.len());

    let keystones: usize = data.passive_tree.nodes.values()
        .filter(|n| n.node_type == pob_calc::passive_tree::NodeType::Keystone)
        .count();
    assert!(keystones >= 20,
        "expected >= 20 keystones, got {keystones}");

    let notables: usize = data.passive_tree.nodes.values()
        .filter(|n| n.node_type == pob_calc::passive_tree::NodeType::Notable)
        .count();
    assert!(notables >= 100,
        "expected >= 100 notables, got {notables}");

    let with_stats: usize = data.passive_tree.nodes.values()
        .filter(|n| !n.stats.is_empty())
        .count();
    assert!(with_stats >= 1000,
        "expected >= 1000 nodes with stats, got {with_stats}");
}

#[test]
#[ignore] // requires DATA_DIR
fn uniques_have_sufficient_count() {
    let data = load_game_data();
    assert!(data.uniques.len() >= 500,
        "expected >= 500 uniques, got {}", data.uniques.len());
}
```

- [ ] **Step 2: Run validation tests**

Run: `DATA_DIR=data cargo test -p pob-calc data_validation -- --ignored --nocapture`
Expected: All 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/pob-calc/tests/data_validation.rs
git commit -m "test: add data validation tests for structural completeness"
```

---

### Task 14: Add extraction script and update documentation

**Files:**
- Create: `scripts/extract_pob_data.sh`
- Modify: `scripts/extract.sh` (add a note about the PoB extractor)

- [ ] **Step 1: Create extraction script**

```bash
#!/bin/bash
set -euo pipefail

# Extract game data from PoB Lua files to JSON.
# Usage: ./scripts/extract_pob_data.sh [output_dir]

OUTPUT_DIR="${1:-data}"
POB_SRC="third-party/PathOfBuilding/src"

if [ ! -d "$POB_SRC/Data" ]; then
    echo "Error: $POB_SRC/Data not found."
    echo "Make sure the PathOfBuilding submodule is initialized:"
    echo "  git submodule update --init"
    exit 1
fi

cargo build -p pob-data-extractor --release
./target/release/pob-data-extractor "$POB_SRC" --output "$OUTPUT_DIR"

echo ""
echo "Data files written to $OUTPUT_DIR/"
echo "Files:"
ls -lh "$OUTPUT_DIR"/gems.json "$OUTPUT_DIR"/bases.json "$OUTPUT_DIR"/uniques.json "$OUTPUT_DIR"/mods.json 2>/dev/null || true
```

- [ ] **Step 2: Make executable**

Run: `chmod +x scripts/extract_pob_data.sh`

- [ ] **Step 3: Commit**

```bash
git add scripts/extract_pob_data.sh
git commit -m "feat: add PoB data extraction script"
```

---

### Task 15: Verify full pipeline end-to-end

**Files:** None modified — verification only.

- [ ] **Step 1: Run all pob-calc tests**

Run: `cargo test -p pob-calc`
Expected: All non-ignored tests pass.

- [ ] **Step 2: Run all pob-data-extractor tests**

Run: `cargo test -p pob-data-extractor`
Expected: All tests pass (or skip gracefully if POB_SRC not set).

- [ ] **Step 3: Run data validation with DATA_DIR**

Run: `DATA_DIR=data cargo test -p pob-calc data_validation -- --ignored --nocapture`
Expected: All 4 validation tests pass.

- [ ] **Step 4: Run oracle tests to verify no regressions**

Run: `DATA_DIR=data cargo test -p pob-calc oracle_all_builds_parse -- --nocapture`
Expected: PASS — all 30+ builds parse successfully.

- [ ] **Step 5: Run full workspace build**

Run: `cargo build --workspace`
Expected: Clean build with no errors.

- [ ] **Step 6: Final commit if any fixups needed**

Only if previous steps required fixes:
```bash
git add -A
git commit -m "fix: address issues found in Phase 4 end-to-end verification"
```
