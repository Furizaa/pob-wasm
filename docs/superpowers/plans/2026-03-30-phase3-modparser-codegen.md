# Phase 3: ModParser Code Generator — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the 28-pattern hand-written `item_parser.rs` with a code-generated parser covering 100% of PoB's ModParser.lua (~5,650 patterns across 6 tables), so items, passives, jewels, and enchants can contribute stats to the ModDb.

**Architecture:** A Rust binary (`tools/modparser-codegen`) reads `ModParser.lua`, extracts 6 pattern tables, translates Lua patterns to Rust regex, classifies `specialModList` functions into ~20 templates, and emits `mod_parser_generated.rs`. Entries too complex for templates get stubs emitted into a manifest; humans write the corresponding Rust handlers in `mod_parser_manual.rs`. A runtime wrapper (`mod_parser.rs`) merges both and exposes `parse_mod()`. The generated file is checked into the repo with a CI freshness check.

**Tech Stack:** Rust, `regex` crate, `once_cell`, `clap`, `serde`/`serde_json`

**Spec:** `docs/superpowers/specs/2026-03-30-modparser-codegen-design.md`

---

## File Structure

### New files to create

| File | Responsibility |
|---|---|
| `tools/modparser-codegen/Cargo.toml` | Crate config for the codegen binary |
| `tools/modparser-codegen/src/main.rs` | CLI entry: reads Lua file, runs pipeline, writes output |
| `tools/modparser-codegen/src/types.rs` | Internal AST types: `LuaPattern`, `FormEntry`, `ModNameEntry`, `SpecialModEntry`, etc. |
| `tools/modparser-codegen/src/lua_parser.rs` | Extracts 6 tables from ModParser.lua text |
| `tools/modparser-codegen/src/pattern_translator.rs` | Translates Lua pattern syntax to Rust regex syntax |
| `tools/modparser-codegen/src/templates.rs` | Classifies `specialModList` functions into templates, emits Rust code per template |
| `tools/modparser-codegen/src/emitter.rs` | Generates complete `mod_parser_generated.rs` and `mod_parser_manual_manifest.rs` |
| `crates/pob-calc/src/build/mod_parser.rs` | Runtime wrapper: imports generated + manual, exposes `parse_mod()` |
| `crates/pob-calc/src/build/mod_parser_generated.rs` | Checked-in generated file (output of codegen) |
| `crates/pob-calc/src/build/mod_parser_manual.rs` | Hand-written handlers for entries codegen can't template |
| `tools/modparser-codegen/test_data/generate_expected.lua` | Lua script to generate reference test data from PoB |
| `tools/modparser-codegen/test_data/expected_mods.json` | Checked-in reference data (200+ stat lines → expected mods) |
| `crates/pob-calc/tests/mod_parser_test.rs` | Integration tests comparing parse_mod() against Lua reference |

### Files to modify

| File | Change |
|---|---|
| `Cargo.toml` (workspace root) | Add `tools/modparser-codegen` to members |
| `crates/pob-calc/Cargo.toml` | Add `regex` and `once_cell` dependencies |
| `crates/pob-calc/src/mod_db/types.rs` | Add 5 new `ModTag` variants |
| `crates/pob-calc/src/mod_db/eval_mod.rs` | Add match arms for new `ModTag` variants (stubbed) |
| `crates/pob-calc/src/build/mod.rs` | Replace `pub mod item_parser` with `pub mod mod_parser` |
| `crates/pob-calc/src/calc/setup.rs:90` | Change call from `item_parser::parse_stat_text` to `mod_parser::parse_mod` |
| `.github/workflows/ci.yml` | Add codegen freshness check step |

### Files to delete

| File | When |
|---|---|
| `crates/pob-calc/src/build/item_parser.rs` | After all mod_parser tests pass (Task 14) |

---

## Task 1: Scaffold the codegen crate

**Files:**
- Create: `tools/modparser-codegen/Cargo.toml`
- Create: `tools/modparser-codegen/src/main.rs`
- Create: `tools/modparser-codegen/src/types.rs`
- Create: `tools/modparser-codegen/src/lua_parser.rs`
- Create: `tools/modparser-codegen/src/pattern_translator.rs`
- Create: `tools/modparser-codegen/src/templates.rs`
- Create: `tools/modparser-codegen/src/emitter.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create `tools/modparser-codegen/Cargo.toml`**

```toml
[package]
name = "modparser-codegen"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "modparser-codegen"
path = "src/main.rs"

[dependencies]
regex = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
```

- [ ] **Step 2: Create stub source files**

`tools/modparser-codegen/src/types.rs`:
```rust
//! Internal AST types for the ModParser code generator.

/// A raw Lua pattern string, before translation to Rust regex.
#[derive(Debug, Clone)]
pub struct LuaPattern(pub String);

/// Form types from formList (INC, RED, MORE, LESS, BASE, FLAG, etc.)
#[derive(Debug, Clone, PartialEq)]
pub enum FormType {
    Inc, Red, More, Less, Base, Gain, Lose, Grants, Removes,
    Chance, Flag, TotalCost, BaseCost, Pen, RegenFlat, RegenPercent,
    DegenFlat, DegenPercent, Degen, Dmg, DmgAttacks, DmgSpells,
    DmgBoth, Override, Doubled,
}

/// A parsed entry from formList.
#[derive(Debug, Clone)]
pub struct FormEntry {
    pub pattern: LuaPattern,
    pub form: FormType,
}

/// A parsed entry from modNameList.
#[derive(Debug, Clone)]
pub struct ModNameEntry {
    pub key: String,             // plain text key, e.g. "strength"
    pub names: Vec<String>,      // stat names, e.g. ["Str"]
    pub tags: Vec<LuaTag>,       // inline tags
}

/// A tag extracted from Lua source, not yet translated to Rust.
#[derive(Debug, Clone)]
pub struct LuaTag {
    pub tag_type: String,        // e.g. "SkillType", "Condition", "Multiplier"
    pub fields: Vec<(String, String)>, // key-value pairs as strings
}

/// A parsed entry from modFlagList.
#[derive(Debug, Clone)]
pub struct ModFlagEntry {
    pub key: String,
    pub flags: Vec<String>,           // e.g. ["ATTACK"], mapped to ModFlags constants
    pub keyword_flags: Vec<String>,   // e.g. ["AURA"], mapped to KeywordFlags constants
    pub tags: Vec<LuaTag>,
}

/// A parsed entry from preFlagList.
#[derive(Debug, Clone)]
pub struct PreFlagEntry {
    pub pattern: LuaPattern,
    pub flags: Vec<String>,
    pub keyword_flags: Vec<String>,
    pub tags: Vec<LuaTag>,
    pub add_to_minion: bool,
    pub add_to_skill: bool,
    pub add_to_aura: bool,
    pub new_aura: bool,
    pub apply_to_enemy: bool,
    /// If this entry is a function, the raw Lua body
    pub func_body: Option<String>,
}

/// A parsed entry from modTagList.
#[derive(Debug, Clone)]
pub struct ModTagEntry {
    pub pattern: LuaPattern,
    pub tags: Vec<LuaTag>,
    /// If this entry is a function, the raw Lua body
    pub func_body: Option<String>,
}

/// Classification result for a specialModList entry.
#[derive(Debug, Clone)]
pub enum SpecialModTemplate {
    /// Static table with one or more mod/flag calls — fully resolved at codegen time.
    StaticMods(Vec<LuaModCall>),
    /// Simple function: single return with mod/flag calls using captures.
    SimpleFn(Vec<LuaModCall>),
    /// Known helper call: grantedExtraSkill, triggerExtraSkill, etc.
    HelperCall { helper: String, args: Vec<String> },
    /// Damage conversion: captures damage type, maps via firstToUpper.
    DamageConversion { stat_prefix: String, capture_index: usize },
    /// Damage gain-as: like conversion but different stat prefix.
    DamageGainAs { stat_prefix: String, capture_index: usize },
    /// Numeric scaling: mod("X", "TYPE", num * factor).
    NumericScaling { mod_call: LuaModCall, factor: f64 },
    /// EnemyModifier wrapper: mod("EnemyModifier", "LIST", { mod = inner }).
    EnemyModifier(Vec<LuaModCall>),
    /// MinionModifier wrapper.
    MinionModifier(Vec<LuaModCall>),
    /// Entry too complex for any template — needs manual handler.
    ManualRequired { lua_body: String, line_number: usize },
}

/// A parsed mod() or flag() call from Lua source.
#[derive(Debug, Clone)]
pub struct LuaModCall {
    pub name: String,           // stat name or expression
    pub mod_type: String,       // "BASE", "INC", "MORE", "FLAG", "LIST"
    pub value: String,          // numeric literal, "true", "num", capture ref
    pub flags: Option<String>,
    pub keyword_flags: Option<String>,
    pub tags: Vec<LuaTag>,
    /// If the name uses string concatenation (e.g. firstToUpper(type).."Damage")
    pub dynamic_name: bool,
}

/// A parsed specialModList entry.
#[derive(Debug, Clone)]
pub struct SpecialModEntry {
    pub pattern: LuaPattern,
    pub template: SpecialModTemplate,
    pub line_number: usize,
}

/// All parsed data from ModParser.lua.
#[derive(Debug)]
pub struct ParsedModParser {
    pub forms: Vec<FormEntry>,
    pub mod_names: Vec<ModNameEntry>,
    pub mod_flags: Vec<ModFlagEntry>,
    pub pre_flags: Vec<PreFlagEntry>,
    pub mod_tags: Vec<ModTagEntry>,
    pub special_mods: Vec<SpecialModEntry>,
}
```

`tools/modparser-codegen/src/lua_parser.rs`:
```rust
//! Extracts the 6 major tables from ModParser.lua as structured data.

use crate::types::*;

/// Parse the entire ModParser.lua file and extract all tables.
pub fn parse_mod_parser_lua(source: &str) -> Result<ParsedModParser, String> {
    todo!("Implement in Task 2")
}
```

`tools/modparser-codegen/src/pattern_translator.rs`:
```rust
//! Translates Lua pattern syntax to Rust regex syntax.

/// Translate a Lua pattern string to a Rust regex string.
/// Returns the regex string and the number of capture groups.
pub fn lua_pattern_to_regex(lua_pattern: &str) -> Result<(String, usize), String> {
    todo!("Implement in Task 3")
}
```

`tools/modparser-codegen/src/templates.rs`:
```rust
//! Classifies specialModList function entries into templates.

use crate::types::*;

/// Classify a specialModList value (table or function body) into a template.
pub fn classify_special_mod(
    value_source: &str,
    is_function: bool,
    line_number: usize,
) -> SpecialModTemplate {
    todo!("Implement in Task 6")
}
```

`tools/modparser-codegen/src/emitter.rs`:
```rust
//! Generates Rust source code from parsed ModParser data.

use crate::types::*;

/// Generate the complete mod_parser_generated.rs content.
pub fn emit_generated(data: &ParsedModParser) -> Result<String, String> {
    todo!("Implement in Task 7")
}

/// Generate the mod_parser_manual_manifest.rs content (stubs for manual handlers).
pub fn emit_manual_manifest(data: &ParsedModParser) -> Result<String, String> {
    todo!("Implement in Task 7")
}
```

`tools/modparser-codegen/src/main.rs`:
```rust
//! ModParser code generator — reads ModParser.lua, emits Rust source.

mod types;
mod lua_parser;
mod pattern_translator;
mod templates;
mod emitter;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "modparser-codegen")]
#[command(about = "Generate Rust mod parser from PoB's ModParser.lua")]
struct Cli {
    /// Path to ModParser.lua
    #[arg(long, default_value = "third-party/PathOfBuilding/src/Modules/ModParser.lua")]
    input: PathBuf,

    /// Output path for mod_parser_generated.rs
    #[arg(long, default_value = "crates/pob-calc/src/build/mod_parser_generated.rs")]
    output: PathBuf,

    /// Output path for mod_parser_manual_manifest.rs (stubs for manual handlers)
    #[arg(long, default_value = "crates/pob-calc/src/build/mod_parser_manual_manifest.rs")]
    manifest: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    let source = std::fs::read_to_string(&cli.input)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", cli.input.display()));

    let parsed = lua_parser::parse_mod_parser_lua(&source)
        .unwrap_or_else(|e| panic!("Failed to parse: {e}"));

    eprintln!("Parsed: {} forms, {} mod_names, {} mod_flags, {} pre_flags, {} mod_tags, {} special_mods",
        parsed.forms.len(), parsed.mod_names.len(), parsed.mod_flags.len(),
        parsed.pre_flags.len(), parsed.mod_tags.len(), parsed.special_mods.len());

    let generated = emitter::emit_generated(&parsed)
        .unwrap_or_else(|e| panic!("Failed to emit: {e}"));

    std::fs::write(&cli.output, &generated)
        .unwrap_or_else(|e| panic!("Failed to write {}: {e}", cli.output.display()));

    let manifest = emitter::emit_manual_manifest(&parsed)
        .unwrap_or_else(|e| panic!("Failed to emit manifest: {e}"));

    std::fs::write(&cli.manifest, &manifest)
        .unwrap_or_else(|e| panic!("Failed to write {}: {e}", cli.manifest.display()));

    // Report coverage
    let total = parsed.special_mods.len();
    let manual = parsed.special_mods.iter()
        .filter(|e| matches!(e.template, crate::types::SpecialModTemplate::ManualRequired { .. }))
        .count();
    let templated = total - manual;
    eprintln!("specialModList: {templated}/{total} templated ({:.1}%), {manual} manual",
        templated as f64 / total as f64 * 100.0);

    eprintln!("Generated: {}", cli.output.display());
    eprintln!("Manifest:  {}", cli.manifest.display());
}
```

- [ ] **Step 3: Add to workspace**

In the root `Cargo.toml`, add `"tools/modparser-codegen"` to the `members` array:

```toml
[workspace]
resolver = "2"
members = [
    "crates/pob-calc",
    "crates/pob-wasm",
    "crates/data-extractor",
    "tools/modparser-codegen",
]
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p modparser-codegen`

Expected: Compiles with warnings about unused imports and `todo!()` macros, but no errors.

- [ ] **Step 5: Commit**

```bash
git add tools/modparser-codegen/ Cargo.toml
git commit -m "feat(phase3): scaffold modparser-codegen crate with types and stubs"
```

---

## Task 2: Lua Parser — Extract All 6 Tables

**Files:**
- Modify: `tools/modparser-codegen/src/lua_parser.rs`
- Modify: `tools/modparser-codegen/src/types.rs` (if needed)

**Reference:** `third-party/PathOfBuilding/src/Modules/ModParser.lua`
- `formList`: lines 67–153 (85 entries, pattern→form string)
- `modNameList`: lines 156–868 (~687 entries, plain text→stat name(s) + optional tags)
- `modFlagList`: lines 870–1056 (~181 entries, plain text→flags)
- `preFlagList`: lines 1059–1272 (~188 entries, pattern→flags/tags)
- `modTagList`: lines 1275–1935 (~637 entries, pattern→tag(s) or function)
- `specialModList`: lines 2030–5685 (~2,030 entries, pattern→table or function)
- Dynamic additions: lines 5686–5696 (keystones + re-keying), lines 5846–5899 (gem-driven entries)

The Lua parser does **structured text extraction** — not a full Lua parser. Each table has a predictable format.

- [ ] **Step 1: Write tests for formList extraction**

Add to `tools/modparser-codegen/src/lua_parser.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn load_mod_parser_lua() -> String {
        std::fs::read_to_string(
            "../../third-party/PathOfBuilding/src/Modules/ModParser.lua"
        ).expect("ModParser.lua not found — ensure git submodule is checked out")
    }

    #[test]
    fn parses_form_list() {
        let source = load_mod_parser_lua();
        let parsed = parse_mod_parser_lua(&source).unwrap();
        // formList has 85 entries
        assert!(parsed.forms.len() >= 80, "Expected ~85 forms, got {}", parsed.forms.len());
        // First entry: ^(%d+)%% increased → INC
        let first = &parsed.forms[0];
        assert_eq!(first.form, FormType::Inc);
        assert!(first.pattern.0.contains("%d+"), "First pattern should contain %d+");
        assert!(first.pattern.0.contains("increased"), "First pattern should contain 'increased'");
        // Check a FLAG entry exists
        assert!(parsed.forms.iter().any(|f| f.form == FormType::Flag));
        // Check a DOUBLED entry exists
        assert!(parsed.forms.iter().any(|f| f.form == FormType::Doubled));
    }

    #[test]
    fn parses_mod_name_list() {
        let source = load_mod_parser_lua();
        let parsed = parse_mod_parser_lua(&source).unwrap();
        assert!(parsed.mod_names.len() >= 650, "Expected ~687 mod_names, got {}", parsed.mod_names.len());
        // Check "strength" → ["Str"]
        let strength = parsed.mod_names.iter().find(|e| e.key == "strength");
        assert!(strength.is_some(), "Should have 'strength' entry");
        assert_eq!(strength.unwrap().names, vec!["Str"]);
        // Check multi-stat: "attributes" → ["Str", "Dex", "Int", "All"]
        let attrs = parsed.mod_names.iter().find(|e| e.key == "attributes");
        assert!(attrs.is_some(), "Should have 'attributes' entry");
        assert_eq!(attrs.unwrap().names.len(), 4);
    }

    #[test]
    fn parses_mod_flag_list() {
        let source = load_mod_parser_lua();
        let parsed = parse_mod_parser_lua(&source).unwrap();
        assert!(parsed.mod_flags.len() >= 170, "Expected ~181 mod_flags, got {}", parsed.mod_flags.len());
    }

    #[test]
    fn parses_pre_flag_list() {
        let source = load_mod_parser_lua();
        let parsed = parse_mod_parser_lua(&source).unwrap();
        assert!(parsed.pre_flags.len() >= 180, "Expected ~188 pre_flags, got {}", parsed.pre_flags.len());
    }

    #[test]
    fn parses_mod_tag_list() {
        let source = load_mod_parser_lua();
        let parsed = parse_mod_parser_lua(&source).unwrap();
        assert!(parsed.mod_tags.len() >= 600, "Expected ~637 mod_tags, got {}", parsed.mod_tags.len());
    }

    #[test]
    fn parses_special_mod_list() {
        let source = load_mod_parser_lua();
        let parsed = parse_mod_parser_lua(&source).unwrap();
        // ~2,030 static entries + 56 keystones + 8 cluster keystones = ~2,094+
        assert!(parsed.special_mods.len() >= 2000,
            "Expected ~2,094+ special_mods, got {}", parsed.special_mods.len());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p modparser-codegen`

Expected: All 6 tests fail with `todo!()` panic.

- [ ] **Step 3: Implement table boundary detection**

In `lua_parser.rs`, add helper functions:

```rust
/// Find the line index where a table starts (e.g., `local formList = {`).
fn find_table_start(lines: &[&str], table_name: &str) -> Option<usize> {
    lines.iter().position(|line| {
        let trimmed = line.trim();
        trimmed.starts_with(&format!("local {table_name}")) && trimmed.contains('=') && trimmed.contains('{')
    })
}

/// From a starting line, find the matching closing brace, accounting for nesting.
fn find_table_end(lines: &[&str], start: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (i, line) in lines[start..].iter().enumerate() {
        let stripped = strip_lua_comments(line);
        for ch in stripped.chars() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(start + i);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// Strip Lua line comments (-- ...) but not inside strings.
fn strip_lua_comments(line: &str) -> &str {
    // Simplified: find -- that's not inside a string literal
    // For ModParser.lua, comments are always at end of line or on their own line
    if let Some(pos) = line.find("--") {
        // Check if -- is inside a string by counting unescaped quotes before it
        let before = &line[..pos];
        let single_quotes = before.matches('\'').count() - before.matches("\\'").count();
        let double_quotes = before.matches('"').count() - before.matches("\\\"").count();
        if single_quotes % 2 == 0 && double_quotes % 2 == 0 {
            return &line[..pos];
        }
    }
    line
}

/// Extract the key from a Lua table entry line.
/// Handles: ["pattern text"] = value
/// Returns (key_content, is_pattern, rest_of_line_after_equals)
fn extract_table_key(line: &str) -> Option<(String, &str)> {
    let trimmed = line.trim();
    if !trimmed.starts_with('[') {
        return None;
    }
    // Find matching ]
    if let Some(bracket_start) = trimmed.find("[\"") {
        if let Some(bracket_end) = trimmed[bracket_start + 2..].find("\"]") {
            let key = &trimmed[bracket_start + 2..bracket_start + 2 + bracket_end];
            let rest = trimmed[bracket_start + 2 + bracket_end + 2..].trim();
            let rest = rest.strip_prefix('=').unwrap_or(rest).trim();
            return Some((key.to_string(), rest));
        }
    }
    None
}
```

- [ ] **Step 4: Implement formList parser**

```rust
fn parse_form_list(lines: &[&str], start: usize, end: usize) -> Vec<FormEntry> {
    let mut forms = Vec::new();
    for line in &lines[start..=end] {
        let trimmed = strip_lua_comments(line).trim();
        if let Some((key, rest)) = extract_table_key(trimmed) {
            let form_str = rest.trim_matches(|c| c == '"' || c == ',' || c == ' ');
            if let Some(form) = parse_form_type(form_str) {
                forms.push(FormEntry {
                    pattern: LuaPattern(key),
                    form,
                });
            }
        }
    }
    forms
}

fn parse_form_type(s: &str) -> Option<FormType> {
    match s {
        "INC" => Some(FormType::Inc),
        "RED" => Some(FormType::Red),
        "MORE" => Some(FormType::More),
        "LESS" => Some(FormType::Less),
        "BASE" => Some(FormType::Base),
        "GAIN" => Some(FormType::Gain),
        "LOSE" => Some(FormType::Lose),
        "GRANTS" => Some(FormType::Grants),
        "REMOVES" => Some(FormType::Removes),
        "CHANCE" => Some(FormType::Chance),
        "FLAG" => Some(FormType::Flag),
        "TOTALCOST" => Some(FormType::TotalCost),
        "BASECOST" => Some(FormType::BaseCost),
        "PEN" => Some(FormType::Pen),
        "REGENFLAT" => Some(FormType::RegenFlat),
        "REGENPERCENT" => Some(FormType::RegenPercent),
        "DEGENFLAT" => Some(FormType::DegenFlat),
        "DEGENPERCENT" => Some(FormType::DegenPercent),
        "DEGEN" => Some(FormType::Degen),
        "DMG" => Some(FormType::Dmg),
        "DMGATTACKS" => Some(FormType::DmgAttacks),
        "DMGSPELLS" => Some(FormType::DmgSpells),
        "DMGBOTH" => Some(FormType::DmgBoth),
        "OVERRIDE" => Some(FormType::Override),
        "DOUBLED" => Some(FormType::Doubled),
        _ => None,
    }
}
```

- [ ] **Step 5: Implement modNameList parser**

Parse the `modNameList` table. Each entry is `["key text"] = "StatName"` or `["key text"] = { "Stat1", "Stat2" }` or `["key text"] = { "Stat1", tag = { type = "...", ... } }`.

```rust
fn parse_mod_name_list(lines: &[&str], start: usize, end: usize) -> Vec<ModNameEntry> {
    let mut entries = Vec::new();
    let mut i = start;
    while i <= end {
        let trimmed = strip_lua_comments(lines[i]).trim();
        if let Some((key, rest)) = extract_table_key(trimmed) {
            if rest.starts_with('"') {
                // Simple string value: "StatName",
                let name = rest.trim_matches(|c| c == '"' || c == ',' || c == ' ');
                entries.push(ModNameEntry {
                    key,
                    names: vec![name.to_string()],
                    tags: vec![],
                });
            } else if rest.starts_with('{') {
                // Table value — collect full table (may span multiple lines)
                let table_src = collect_braced_block(lines, i, rest);
                let (names, tags) = parse_mod_name_table(&table_src);
                entries.push(ModNameEntry { key, names, tags });
            }
        }
        i += 1;
    }
    entries
}
```

The `collect_braced_block` helper gathers multi-line table contents by brace-counting. The `parse_mod_name_table` helper extracts string array elements and `tag`/`tagList` fields.

- [ ] **Step 6: Implement remaining table parsers**

Following the same pattern, implement parsers for:
- `parse_mod_flag_list()` — plain text keys → flags/keywordFlags extraction
- `parse_pre_flag_list()` — pattern keys → flags/tags/misc, including function detection
- `parse_mod_tag_list()` — pattern keys → tag tables or functions
- `parse_special_mod_list()` — pattern keys → table or function body extraction

For `specialModList`, also handle the dynamic additions:
- Lines 5686–5688: keystones loop (hardcode the 56 keystone names from `Data.lua`)
- Lines 5689–5691: cluster keystones loop (hardcode the 8 names)
- Lines 5692–5696: `^...$` re-keying (wrap all patterns with anchors)

The gem-driven entries (lines 5846–5899) require gem data. Since the codegen tool doesn't have access to the full gem database at parse time, these entries will be emitted as a **separate runtime function** that accepts gem data and generates the per-gem patterns dynamically. This is documented in a comment in the generated code.

- [ ] **Step 7: Wire up the main parse function**

```rust
pub fn parse_mod_parser_lua(source: &str) -> Result<ParsedModParser, String> {
    let lines: Vec<&str> = source.lines().collect();

    let form_start = find_table_start(&lines, "formList")
        .ok_or("formList not found")?;
    let form_end = find_table_end(&lines, form_start)
        .ok_or("formList end not found")?;
    let forms = parse_form_list(&lines, form_start, form_end);

    let mn_start = find_table_start(&lines, "modNameList")
        .ok_or("modNameList not found")?;
    let mn_end = find_table_end(&lines, mn_start)
        .ok_or("modNameList end not found")?;
    let mod_names = parse_mod_name_list(&lines, mn_start, mn_end);

    // ... same for modFlagList, preFlagList, modTagList, specialModList

    Ok(ParsedModParser { forms, mod_names, mod_flags, pre_flags, mod_tags, special_mods })
}
```

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo test -p modparser-codegen`

Expected: All 6 table-count tests pass.

- [ ] **Step 9: Commit**

```bash
git add tools/modparser-codegen/src/
git commit -m "feat(phase3): implement Lua parser for all 6 ModParser.lua tables"
```

---

## Task 3: Pattern Translator — Lua Patterns to Rust Regex

**Files:**
- Modify: `tools/modparser-codegen/src/pattern_translator.rs`

**Reference:** Spec Section 5 — Lua Pattern → Rust Regex translation table.

- [ ] **Step 1: Write tests for pattern translation**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_digit_class() {
        let (regex, caps) = lua_pattern_to_regex("^(%d+)%% increased").unwrap();
        assert_eq!(regex, r"^(\d+)% increased");
        assert_eq!(caps, 1);
    }

    #[test]
    fn translates_signed_number() {
        let (regex, caps) = lua_pattern_to_regex("^([%+%-][%d%.]+)%%?").unwrap();
        assert_eq!(regex, r"^([+\-][\d.]+)%?");
        assert_eq!(caps, 1);
    }

    #[test]
    fn translates_non_greedy_capture() {
        let (regex, caps) = lua_pattern_to_regex("regenerate (.-)%% of").unwrap();
        assert_eq!(regex, r"regenerate (.*?)% of");
        assert_eq!(caps, 1);
    }

    #[test]
    fn translates_letter_class() {
        let (regex, caps) = lua_pattern_to_regex("(%a+) damage").unwrap();
        assert_eq!(regex, r"([a-zA-Z]+) damage");
        assert_eq!(caps, 1);
    }

    #[test]
    fn translates_literal_percent() {
        let (regex, _) = lua_pattern_to_regex("%%").unwrap();
        assert_eq!(regex, "%");
    }

    #[test]
    fn translates_literal_hyphen() {
        let (regex, _) = lua_pattern_to_regex("non%-curse").unwrap();
        assert_eq!(regex, r"non\-curse");
    }

    #[test]
    fn translates_literal_dot() {
        let (regex, _) = lua_pattern_to_regex("%.").unwrap();
        assert_eq!(regex, r"\.");
    }

    #[test]
    fn translates_char_class_with_lua_escapes() {
        let (regex, _) = lua_pattern_to_regex("[%d%.]+").unwrap();
        assert_eq!(regex, r"[\d.]+");
    }

    #[test]
    fn translates_optional_char() {
        // Lua ? = 0 or 1 of previous char
        let (regex, _) = lua_pattern_to_regex("hits?").unwrap();
        assert_eq!(regex, "hits?");
    }

    #[test]
    fn translates_word_class() {
        let (regex, _) = lua_pattern_to_regex("%w+").unwrap();
        assert_eq!(regex, r"\w+");
    }

    #[test]
    fn translates_lowercase_class() {
        let (regex, _) = lua_pattern_to_regex("%l").unwrap();
        assert_eq!(regex, "[a-z]");
    }

    #[test]
    fn translates_complex_pattern() {
        // Real pattern from specialModList
        let (regex, caps) = lua_pattern_to_regex(
            "^(%d+)%% of physical damage converted to (%a+) damage"
        ).unwrap();
        assert_eq!(regex, r"^(\d+)% of physical damage converted to ([a-zA-Z]+) damage");
        assert_eq!(caps, 2);
    }

    #[test]
    fn preserves_char_class_brackets() {
        let (regex, _) = lua_pattern_to_regex("[hd][ae][va][el]").unwrap();
        assert_eq!(regex, "[hd][ae][va][el]");
    }

    #[test]
    fn translates_escaped_plus() {
        let (regex, _) = lua_pattern_to_regex("%+").unwrap();
        assert_eq!(regex, r"\+");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p modparser-codegen -- pattern_translator`

Expected: All fail with `todo!()`.

- [ ] **Step 3: Implement the translator**

```rust
pub fn lua_pattern_to_regex(lua_pattern: &str) -> Result<(String, usize), String> {
    let mut result = String::new();
    let mut captures = 0usize;
    let chars: Vec<char> = lua_pattern.chars().collect();
    let mut i = 0;
    let mut in_char_class = false;

    while i < chars.len() {
        match chars[i] {
            '%' if i + 1 < chars.len() => {
                let next = chars[i + 1];
                let replacement = if in_char_class {
                    // Inside [...], translate Lua escapes to regex equivalents
                    match next {
                        'd' => r"\d",
                        'a' => "a-zA-Z",
                        'w' => r"\w",
                        'l' => "a-z",
                        'u' => "A-Z",
                        's' => r"\s",
                        'p' => r"[:punct:]",
                        '%' => "%",
                        '+' => "+",
                        '-' => r"\-",
                        '.' => ".",
                        _ => {
                            // Literal escape inside char class
                            result.push('\\');
                            result.push(next);
                            i += 2;
                            continue;
                        }
                    }
                } else {
                    // Outside char class
                    match next {
                        'd' => r"\d",
                        'a' => "[a-zA-Z]",
                        'w' => r"\w",
                        'l' => "[a-z]",
                        'u' => "[A-Z]",
                        's' => r"\s",
                        'p' => "[[:punct:]]",
                        '%' => "%",
                        '+' => r"\+",
                        '-' => r"\-",
                        '.' => r"\.",
                        '(' => r"\(",
                        ')' => r"\)",
                        '[' => r"\[",
                        ']' => r"\]",
                        _ => {
                            result.push('\\');
                            result.push(next);
                            i += 2;
                            continue;
                        }
                    }
                };
                result.push_str(replacement);
                i += 2;
            }
            '(' => {
                captures += 1;
                result.push('(');
                // Check for (.-) → (.*?)
                if i + 3 < chars.len() && chars[i + 1] == '.' && chars[i + 2] == '-' && chars[i + 3] == ')' {
                    result.push_str(".*?)");
                    i += 4;
                    continue;
                }
                i += 1;
            }
            '[' => {
                in_char_class = true;
                result.push('[');
                i += 1;
            }
            ']' => {
                in_char_class = false;
                result.push(']');
                i += 1;
            }
            // Lua's standalone `-` as non-greedy quantifier (rare outside capture groups)
            // Only matters in (.-) which is handled above
            ch => {
                result.push(ch);
                i += 1;
            }
        }
    }

    Ok((result, captures))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p modparser-codegen -- pattern_translator`

Expected: All pass.

- [ ] **Step 5: Add a bulk validation test against real patterns**

```rust
#[test]
fn all_form_list_patterns_translate() {
    let source = std::fs::read_to_string(
        "../../third-party/PathOfBuilding/src/Modules/ModParser.lua"
    ).unwrap();
    let parsed = crate::lua_parser::parse_mod_parser_lua(&source).unwrap();
    for form in &parsed.forms {
        let result = lua_pattern_to_regex(&form.pattern.0);
        assert!(result.is_ok(), "Failed to translate formList pattern '{}': {:?}",
            form.pattern.0, result.err());
        // Verify it compiles as a regex
        let (regex_str, _) = result.unwrap();
        assert!(regex::Regex::new(&regex_str).is_ok(),
            "Invalid regex '{}' from Lua pattern '{}'", regex_str, form.pattern.0);
    }
}
```

- [ ] **Step 6: Run bulk test**

Run: `cargo test -p modparser-codegen -- all_form_list_patterns_translate`

Expected: PASS. If any fail, fix the translator and re-run.

- [ ] **Step 7: Commit**

```bash
git add tools/modparser-codegen/src/pattern_translator.rs
git commit -m "feat(phase3): implement Lua pattern to Rust regex translator"
```

---

## Task 4: Add New ModTag Variants

**Files:**
- Modify: `crates/pob-calc/src/mod_db/types.rs`
- Modify: `crates/pob-calc/src/mod_db/eval_mod.rs`

- [ ] **Step 1: Write tests for new tag variants**

Add to `crates/pob-calc/src/mod_db/eval_mod.rs` test module:

```rust
#[test]
fn eval_skill_name_tag_stubbed_passes() {
    let m = Mod {
        name: "Damage".into(),
        mod_type: ModType::Inc,
        value: ModValue::Number(10.0),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags: vec![ModTag::SkillName { name: "Fireball".into() }],
        source: ModSource::new("Test", "test"),
    };
    let db = ModDb::new();
    let output = OutputTable::new();
    // Stubbed: SkillName always passes
    assert_eq!(eval_mod(&m, None, &db, &output), Some(10.0));
}

#[test]
fn eval_skill_id_tag_stubbed_passes() {
    let m = Mod {
        name: "Damage".into(),
        mod_type: ModType::Inc,
        value: ModValue::Number(5.0),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags: vec![ModTag::SkillId { id: "Fireball".into() }],
        source: ModSource::new("Test", "test"),
    };
    let db = ModDb::new();
    let output = OutputTable::new();
    assert_eq!(eval_mod(&m, None, &db, &output), Some(5.0));
}

#[test]
fn eval_socketed_in_tag_stubbed_passes() {
    let m = Mod {
        name: "Level".into(),
        mod_type: ModType::Base,
        value: ModValue::Number(1.0),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags: vec![ModTag::SocketedIn { slot_name: "Body Armour".into() }],
        source: ModSource::new("Test", "test"),
    };
    let db = ModDb::new();
    let output = OutputTable::new();
    assert_eq!(eval_mod(&m, None, &db, &output), Some(1.0));
}

#[test]
fn eval_item_condition_tag_stubbed_passes() {
    let m = Mod {
        name: "Armour".into(),
        mod_type: ModType::Inc,
        value: ModValue::Number(20.0),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags: vec![ModTag::ItemCondition { var: "UsingShield".into(), neg: false }],
        source: ModSource::new("Test", "test"),
    };
    let db = ModDb::new();
    let output = OutputTable::new();
    assert_eq!(eval_mod(&m, None, &db, &output), Some(20.0));
}

#[test]
fn eval_skill_part_tag_stubbed_passes() {
    let m = Mod {
        name: "Damage".into(),
        mod_type: ModType::More,
        value: ModValue::Number(30.0),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags: vec![ModTag::SkillPart { part: 1 }],
        source: ModSource::new("Test", "test"),
    };
    let db = ModDb::new();
    let output = OutputTable::new();
    assert_eq!(eval_mod(&m, None, &db, &output), Some(30.0));
}
```

- [ ] **Step 2: Run tests to verify they fail (ModTag variants don't exist yet)**

Run: `cargo test -p pob-calc -- eval_skill_name`

Expected: Compile error — `SkillName` is not a variant of `ModTag`.

- [ ] **Step 3: Add new variants to ModTag enum**

In `crates/pob-calc/src/mod_db/types.rs`, add to the `ModTag` enum:

```rust
    // --- New variants for Phase 3 ---
    SkillName { name: String },
    SkillId { id: String },
    SkillPart { part: u32 },
    SocketedIn { slot_name: String },
    ItemCondition { var: String, neg: bool },
```

- [ ] **Step 4: Add stub match arms in eval_mod**

In `crates/pob-calc/src/mod_db/eval_mod.rs`, in the tag match block, add:

```rust
                // Phase 3 stubs — these tags are stored correctly but evaluation
                // is deferred to Phase 4+ when calc modules can provide context.
                ModTag::SkillName { .. } => {
                    // TODO(phase4): check against cfg.skill_name
                }
                ModTag::SkillId { .. } => {
                    // TODO(phase4): check against cfg.skill_id
                }
                ModTag::SkillPart { .. } => {
                    // TODO(phase4): check against cfg.skill_part
                }
                ModTag::SocketedIn { .. } => {
                    // TODO(phase4): check against item socket context
                }
                ModTag::ItemCondition { .. } => {
                    // TODO(phase4): check against item condition context
                }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p pob-calc`

Expected: All tests pass, including existing tests and new stub tests.

- [ ] **Step 6: Commit**

```bash
git add crates/pob-calc/src/mod_db/types.rs crates/pob-calc/src/mod_db/eval_mod.rs
git commit -m "feat(phase3): add SkillName, SkillId, SkillPart, SocketedIn, ItemCondition ModTag variants (stubbed)"
```

---

## Task 5: Add Runtime Dependencies to pob-calc

**Files:**
- Modify: `crates/pob-calc/Cargo.toml`

- [ ] **Step 1: Add regex and once_cell**

Add to `crates/pob-calc/Cargo.toml` under `[dependencies]`:

```toml
regex = "1"
once_cell = "1"
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p pob-calc`

Expected: Compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add crates/pob-calc/Cargo.toml
git commit -m "feat(phase3): add regex and once_cell dependencies to pob-calc"
```

---

## Task 6: Template Classification for specialModList

**Files:**
- Modify: `tools/modparser-codegen/src/templates.rs`

This is the core classification logic that determines whether a specialModList entry can be auto-generated or needs manual implementation.

**Key insight from analysis:** Of ~2,030 entries:
- ~994 are static tables (no `function` keyword) → always templateable
- ~916 are simple functions (single return with mod/flag) → mostly templateable
- ~71 are complex functions (if/else, loops, data lookups, closures) → ManualRequired

- [ ] **Step 1: Write classification tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_static_flag_table() {
        let template = classify_special_mod(
            r#"{ flag("CannotBeEvaded") }"#,
            false, 0,
        );
        assert!(matches!(template, SpecialModTemplate::StaticMods(_)));
    }

    #[test]
    fn classifies_static_multi_mod_table() {
        let template = classify_special_mod(
            r#"{ mod("AttackDodgeChance", "BASE", 30), mod("Armour", "MORE", -50) }"#,
            false, 0,
        );
        assert!(matches!(template, SpecialModTemplate::StaticMods(_)));
    }

    #[test]
    fn classifies_simple_function_with_num() {
        let template = classify_special_mod(
            r#"function(num) return { mod("PhysicalDamageConvertToFire", "BASE", num) } end"#,
            true, 0,
        );
        assert!(matches!(template, SpecialModTemplate::SimpleFn(_)));
    }

    #[test]
    fn classifies_damage_conversion() {
        let template = classify_special_mod(
            r#"function(num, _, type) return { mod("PhysicalDamageConvertTo"..firstToUpper(type), "BASE", num) } end"#,
            true, 0,
        );
        assert!(matches!(template, SpecialModTemplate::DamageConversion { .. }));
    }

    #[test]
    fn classifies_granted_extra_skill() {
        let template = classify_special_mod(
            r#"function(num, _, skill) return grantedExtraSkill(skill, num) end"#,
            true, 0,
        );
        assert!(matches!(template, SpecialModTemplate::HelperCall { .. }));
    }

    #[test]
    fn classifies_trigger_extra_skill() {
        let template = classify_special_mod(
            r#"function(num, _, skill) return triggerExtraSkill(skill, num) end"#,
            true, 0,
        );
        assert!(matches!(template, SpecialModTemplate::HelperCall { .. }));
    }

    #[test]
    fn classifies_enemy_modifier() {
        let template = classify_special_mod(
            r#"{ mod("EnemyModifier", "LIST", { mod = mod("FireResist", "BASE", -10) }) }"#,
            false, 0,
        );
        assert!(matches!(template, SpecialModTemplate::EnemyModifier(_)));
    }

    #[test]
    fn classifies_complex_function_as_manual() {
        let template = classify_special_mod(
            r#"function(num) local mods = {} for i, ailment in ipairs(data.nonDamagingAilmentTypeList) do mods[i] = mod("Self"..ailment.."Effect", "INC", -num) end return mods end"#,
            true, 0,
        );
        assert!(matches!(template, SpecialModTemplate::ManualRequired { .. }));
    }

    #[test]
    fn classifies_jewel_func_as_manual() {
        let template = classify_special_mod(
            r#"function(_, radius, dmgType) return { mod("ExtraJewelFunc", "LIST", {radius = firstToUpper(radius), func = function(node, out, data) getSimpleConv(...) end}) } end"#,
            true, 0,
        );
        assert!(matches!(template, SpecialModTemplate::ManualRequired { .. }));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p modparser-codegen -- templates`

Expected: All fail with `todo!()`.

- [ ] **Step 3: Implement classification logic**

```rust
use crate::types::*;

/// Classify a specialModList value into a template.
///
/// `value_source` is the raw Lua source for the value (after the `=`).
/// `is_function` is true if the value starts with `function(`.
pub fn classify_special_mod(
    value_source: &str,
    is_function: bool,
    line_number: usize,
) -> SpecialModTemplate {
    let trimmed = value_source.trim();

    // Detect complex patterns that always need manual handling
    if is_complex_function(trimmed) {
        return SpecialModTemplate::ManualRequired {
            lua_body: trimmed.to_string(),
            line_number,
        };
    }

    if !is_function {
        // Static table
        return classify_static_table(trimmed, line_number);
    }

    // Function — try specific templates first, then fall back
    classify_function(trimmed, line_number)
}

fn is_complex_function(source: &str) -> bool {
    // Functions with control flow, loops, data lookups, or closures
    let complex_markers = [
        "if ", "for ", "while ", "ipairs(", "pairs(",
        "data.", "gemIdLookup[", "table.insert", "t_insert",
        "function(node", "getSimpleConv", "getPerStat", "getThreshold",
        "ExtraJewelFunc", "gmatch",
    ];
    // Only flag as complex if it's a function (not static table)
    if !source.contains("function(") && !source.starts_with("function(") {
        return false;
    }
    complex_markers.iter().any(|marker| source.contains(marker))
}

fn classify_static_table(source: &str, line_number: usize) -> SpecialModTemplate {
    // Parse the table contents for mod() and flag() calls
    if source == "{ }" || source == "{}" {
        return SpecialModTemplate::StaticMods(vec![]);
    }

    // Check for EnemyModifier wrapper
    if source.contains("\"EnemyModifier\"") {
        if let Some(mods) = parse_mod_calls(source) {
            return SpecialModTemplate::EnemyModifier(mods);
        }
    }

    // Check for MinionModifier wrapper
    if source.contains("\"MinionModifier\"") {
        if let Some(mods) = parse_mod_calls(source) {
            return SpecialModTemplate::MinionModifier(mods);
        }
    }

    // General static table with mod/flag calls
    if let Some(mods) = parse_mod_calls(source) {
        return SpecialModTemplate::StaticMods(mods);
    }

    SpecialModTemplate::ManualRequired {
        lua_body: source.to_string(),
        line_number,
    }
}

fn classify_function(source: &str, line_number: usize) -> SpecialModTemplate {
    // Try helper call patterns first (most specific)
    if source.contains("grantedExtraSkill(") {
        return SpecialModTemplate::HelperCall {
            helper: "grantedExtraSkill".into(),
            args: extract_helper_args(source, "grantedExtraSkill"),
        };
    }
    if source.contains("triggerExtraSkill(") {
        return SpecialModTemplate::HelperCall {
            helper: "triggerExtraSkill".into(),
            args: extract_helper_args(source, "triggerExtraSkill"),
        };
    }
    if source.contains("extraSupport(") {
        return SpecialModTemplate::HelperCall {
            helper: "extraSupport".into(),
            args: extract_helper_args(source, "extraSupport"),
        };
    }
    if source.contains("explodeFunc(") || source.contains("explodeFunc ") {
        return SpecialModTemplate::HelperCall {
            helper: "explodeFunc".into(),
            args: extract_helper_args(source, "explodeFunc"),
        };
    }

    // Damage conversion: contains firstToUpper and ConvertTo
    if source.contains("firstToUpper") && source.contains("ConvertTo") {
        return SpecialModTemplate::DamageConversion {
            stat_prefix: extract_stat_prefix(source, "ConvertTo"),
            capture_index: 0,
        };
    }

    // Damage gain-as: contains firstToUpper and GainAs
    if source.contains("firstToUpper") && source.contains("GainAs") {
        return SpecialModTemplate::DamageGainAs {
            stat_prefix: extract_stat_prefix(source, "GainAs"),
            capture_index: 0,
        };
    }

    // Simple function with mod/flag calls and no complex logic
    if let Some(mods) = parse_function_body_mods(source) {
        return SpecialModTemplate::SimpleFn(mods);
    }

    SpecialModTemplate::ManualRequired {
        lua_body: source.to_string(),
        line_number,
    }
}
```

Implement the helper functions `parse_mod_calls`, `parse_function_body_mods`, `extract_helper_args`, `extract_stat_prefix` as needed. Each extracts structured data from the Lua source text.

- [ ] **Step 4: Run tests**

Run: `cargo test -p modparser-codegen -- templates`

Expected: All pass.

- [ ] **Step 5: Add integration test against real specialModList**

```rust
#[test]
fn coverage_at_least_80_percent() {
    let source = std::fs::read_to_string(
        "../../third-party/PathOfBuilding/src/Modules/ModParser.lua"
    ).unwrap();
    let parsed = crate::lua_parser::parse_mod_parser_lua(&source).unwrap();

    let total = parsed.special_mods.len();
    let manual = parsed.special_mods.iter()
        .filter(|e| matches!(e.template, SpecialModTemplate::ManualRequired { .. }))
        .count();
    let templated = total - manual;
    let pct = templated as f64 / total as f64 * 100.0;

    eprintln!("specialModList coverage: {templated}/{total} ({pct:.1}%), {manual} manual");

    assert!(pct >= 80.0,
        "Expected ≥80% template coverage, got {pct:.1}% ({manual} manual entries)");
}
```

- [ ] **Step 6: Run coverage test, iterate on templates until ≥80%**

Run: `cargo test -p modparser-codegen -- coverage_at_least_80 -- --nocapture`

Iterate: if below 80%, examine the stderr output listing unclassified entries, identify common patterns among them, and add new template cases. Repeat until the test passes.

- [ ] **Step 7: Commit**

```bash
git add tools/modparser-codegen/src/templates.rs
git commit -m "feat(phase3): implement specialModList template classification (≥80% coverage)"
```

---

## Task 7: Code Emitter — Generate mod_parser_generated.rs

**Files:**
- Modify: `tools/modparser-codegen/src/emitter.rs`

This is the largest task. The emitter produces the complete generated Rust source file.

- [ ] **Step 1: Write tests for emitter output**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emitted_code_compiles_in_isolation() {
        let source = std::fs::read_to_string(
            "../../third-party/PathOfBuilding/src/Modules/ModParser.lua"
        ).unwrap();
        let parsed = crate::lua_parser::parse_mod_parser_lua(&source).unwrap();
        let generated = emit_generated(&parsed).unwrap();
        // Basic structure checks
        assert!(generated.contains("AUTO-GENERATED from ModParser.lua"));
        assert!(generated.contains("enum FormType"));
        assert!(generated.contains("fn parse_mod_generated"));
        assert!(generated.contains("FORM_PATTERNS"));
        assert!(generated.contains("MOD_NAME_MAP"));
        assert!(generated.contains("SPECIAL_MOD_PATTERNS"));
    }

    #[test]
    fn emitted_manifest_has_all_manual_entries() {
        let source = std::fs::read_to_string(
            "../../third-party/PathOfBuilding/src/Modules/ModParser.lua"
        ).unwrap();
        let parsed = crate::lua_parser::parse_mod_parser_lua(&source).unwrap();
        let manifest = emit_manual_manifest(&parsed).unwrap();

        let manual_count = parsed.special_mods.iter()
            .filter(|e| matches!(e.template, SpecialModTemplate::ManualRequired { .. }))
            .count();

        // Every ManualRequired entry must appear in the manifest
        assert!(manifest.contains(&format!("MANUAL_SPECIAL_COUNT: usize = {manual_count}")));
    }
}
```

- [ ] **Step 2: Implement the emitter**

The emitter generates Rust source code section by section. Key sections:

1. **File header** with auto-generated warning and coverage stats
2. **Imports** (`use once_cell::sync::Lazy; use regex::Regex; use std::collections::HashMap;`)
3. **FormType enum** and FORM_PATTERNS array
4. **ModNameEntry struct** and MOD_NAME_MAP HashMap
5. **PreFlagEntry struct** and PRE_FLAG_PATTERNS array
6. **ModFlagEntry struct** and MOD_FLAG_MAP HashMap
7. **ModTagEntry struct** and MOD_TAG_PATTERNS array
8. **SpecialModPattern struct** and SPECIAL_MOD_PATTERNS array (for templated entries)
9. **MANUAL_SPECIAL_PATTERNS** array (patterns that delegate to manual handler callback)
10. **Lookup tables** (DAMAGE_TYPE_MAP, SUFFIX_TYPE_MAP, etc.)
11. **scan_patterns()** and **scan_plain()** functions
12. **extract_pre_flags()**, **extract_form()**, **extract_mod_tags()**, **extract_mod_name()**, **extract_mod_flags()** functions
13. **build_mods()** — combines all extracted data into `Vec<Mod>`
14. **parse_mod_generated()** — the main pipeline function
15. **Count constants** for the compile-time assertion

For each templated specialModList entry, the emitter generates a handler function. For example, a `SimpleFn` with `mod("CritChance", "INC", num)` becomes:

```rust
|caps: &regex::Captures, source: &ModSource| -> Vec<Mod> {
    let num: f64 = caps.get(1).map(|m| m.as_str().parse().unwrap_or(0.0)).unwrap_or(0.0);
    vec![Mod {
        name: "CritChance".into(),
        mod_type: ModType::Inc,
        value: ModValue::Number(num),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags: vec![],
        source: source.clone(),
    }]
}
```

For the `scan_patterns` function, implement the PoB tie-breaking semantics: earliest match position, then longest match, then longest pattern string.

For the `scan_plain` function, implement plain-text substring search with the same tie-breaking.

The generated `parse_mod_generated()` function takes a `manual_handler` callback parameter for manual entries:

```rust
pub fn parse_mod_generated(
    line: &str,
    source: &ModSource,
    manual_handler: &dyn Fn(&str, &regex::Captures, &ModSource) -> Vec<Mod>,
) -> Vec<Mod> {
    // ... pipeline as described in spec Section 7
}
```

- [ ] **Step 3: Run emitter tests**

Run: `cargo test -p modparser-codegen -- emitter`

Expected: Pass.

- [ ] **Step 4: Run the codegen tool and inspect output**

Run: `cargo run -p modparser-codegen`

Expected: Creates two files. Inspect stderr for coverage report. Inspect the generated file to verify it looks reasonable.

- [ ] **Step 5: Commit**

```bash
git add tools/modparser-codegen/src/emitter.rs
git commit -m "feat(phase3): implement code emitter for mod_parser_generated.rs"
```

---

## Task 8: Generate and Check In the Generated File

**Files:**
- Create: `crates/pob-calc/src/build/mod_parser_generated.rs`
- Create: `crates/pob-calc/src/build/mod_parser_manual_manifest.rs`

- [ ] **Step 1: Run the codegen tool**

Run: `cargo run -p modparser-codegen`

Expected: Two files created at the default output paths.

- [ ] **Step 2: Verify the generated file compiles**

The generated file cannot compile alone — it needs the wrapper module. But we can check for syntax errors by attempting to include it. Create a temporary test:

Run: `cargo check -p pob-calc` (will fail because mod_parser.rs doesn't exist yet — that's expected. We just want the generated file to exist for the next task.)

- [ ] **Step 3: Inspect coverage report**

Check stderr output from step 1. Verify:
- formList: 85 entries
- modNameList: ≥650 entries
- modFlagList: ≥170 entries
- preFlagList: ≥180 entries
- modTagList: ≥600 entries
- specialModList: total count and templated vs manual split

- [ ] **Step 4: Commit the generated files**

```bash
git add crates/pob-calc/src/build/mod_parser_generated.rs
git add crates/pob-calc/src/build/mod_parser_manual_manifest.rs
git commit -m "feat(phase3): check in generated mod_parser_generated.rs and manual manifest"
```

---

## Task 9: Runtime Wrapper (mod_parser.rs)

**Files:**
- Create: `crates/pob-calc/src/build/mod_parser.rs`
- Create: `crates/pob-calc/src/build/mod_parser_manual.rs` (initial stub)

- [ ] **Step 1: Create mod_parser_manual.rs with stub handler**

```rust
//! Hand-written handlers for specialModList entries that codegen can't template.
//!
//! Each handler is identified by a string ID (emitted by the codegen manifest).
//! The Lua source for each entry is included as a comment above the handler
//! in mod_parser_manual_manifest.rs for reference.

use crate::mod_db::types::*;

/// Handle a manually-implemented special mod pattern.
///
/// `id` is the handler ID assigned by the codegen manifest.
/// `caps` contains the regex captures from the pattern match.
/// `source` is the ModSource for the resulting mods.
///
/// Returns the mods for this pattern, or empty vec if the handler is not yet implemented.
pub fn handle_manual_special(id: &str, caps: &regex::Captures, source: &ModSource) -> Vec<Mod> {
    match id {
        // TODO: implement handlers from manifest — see mod_parser_manual_manifest.rs
        _ => {
            // Unimplemented manual handler — return empty
            // This should not happen once all handlers are written
            vec![]
        }
    }
}
```

- [ ] **Step 2: Create mod_parser.rs wrapper**

```rust
//! Mod parser — generated from Path of Building's ModParser.lua.
//!
//! This module wraps the auto-generated parser (`mod_parser_generated.rs`)
//! and the hand-written handlers for complex patterns (`mod_parser_manual.rs`).
//! Together they provide 100% coverage of ModParser.lua's pattern tables.

#[allow(clippy::all, unused_imports, dead_code, unused_variables, unreachable_patterns)]
#[path = "mod_parser_generated.rs"]
mod generated;

pub mod mod_parser_manual;

use crate::mod_db::types::{Mod, ModSource};

/// Parse a stat text line into zero or more Mod values.
///
/// Mirrors Path of Building's `ModParser.parseMod()` function.
/// Handles all form types (increased, reduced, more, less, base, flag, etc.),
/// weapon-specific mods, conditional mods, gem-specific mods, and all
/// special mod patterns (auto-generated + hand-written).
pub fn parse_mod(line: &str, source: ModSource) -> Vec<Mod> {
    generated::parse_mod_generated(line, &source, &mod_parser_manual::handle_manual_special)
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p pob-calc`

Expected: Compiles (possibly with warnings about dead code in the generated file — the `#[allow(...)]` attribute handles this).

If compile errors occur in the generated file, fix the emitter (Task 7), re-run codegen (Task 8), and retry.

- [ ] **Step 4: Commit**

```bash
git add crates/pob-calc/src/build/mod_parser.rs crates/pob-calc/src/build/mod_parser_manual.rs
git commit -m "feat(phase3): add mod_parser.rs wrapper and mod_parser_manual.rs stub"
```

---

## Task 10: Wire Up mod_parser and Migrate Call Sites

**Files:**
- Modify: `crates/pob-calc/src/build/mod.rs`
- Modify: `crates/pob-calc/src/calc/setup.rs`

- [ ] **Step 1: Add mod_parser module to build/mod.rs**

In `crates/pob-calc/src/build/mod.rs`, add the new module (keep item_parser for now):

```rust
pub mod item_parser;
pub mod mod_parser;
pub mod types;
pub mod xml_parser;

pub use types::Build;
pub use xml_parser::parse_xml;
```

- [ ] **Step 2: Verify it compiles with both modules**

Run: `cargo check -p pob-calc`

Expected: Compiles.

- [ ] **Step 3: Migrate the call site in setup.rs**

In `crates/pob-calc/src/calc/setup.rs`, line 90, change:

```rust
// Before:
let mods = crate::build::item_parser::parse_stat_text(stat_text, source.clone());

// After:
let mods = crate::build::mod_parser::parse_mod(stat_text, source.clone());
```

- [ ] **Step 4: Run existing tests**

Run: `cargo test -p pob-calc`

Expected: All existing tests pass. The mod_parser should handle all 28 patterns that item_parser handled (and many more).

- [ ] **Step 5: Commit**

```bash
git add crates/pob-calc/src/build/mod.rs crates/pob-calc/src/calc/setup.rs
git commit -m "feat(phase3): wire up mod_parser, migrate setup.rs call site"
```

---

## Task 11: Lua Reference Data Generation

**Files:**
- Create: `tools/modparser-codegen/test_data/generate_expected.lua`
- Create: `tools/modparser-codegen/test_data/stat_lines.txt`
- Create: `tools/modparser-codegen/test_data/expected_mods.json`

This task creates the Lua script that runs PoB's ModParser on 200+ stat lines and generates reference JSON for Rust integration tests.

- [ ] **Step 1: Create stat_lines.txt with 200+ representative lines**

```text
10% increased maximum Life
5% reduced Mana Cost of Skills
20% more Attack Damage
10% less Attack Speed
+50 to maximum Life
+30 to Strength
+10 to all Attributes
+40% to Fire Resistance
+15% to all Elemental Resistances
Adds 10 to 20 Fire Damage to Attacks
50% increased Critical Strike Chance
+1.5% to Critical Strike Multiplier
15% increased Physical Damage with Axes
20% increased Attack Speed with Swords
10% increased Attack Speed while Dual Wielding
4% increased Attack Damage per Frenzy Charge
Damage Penetrates 10% Fire Resistance
50% of Physical Damage Converted to Fire Damage
1% of Physical Attack Damage Leeched as Life
Your hits can't be evaded
Chaos Damage does not bypass Energy Shield
Supported Skills deal 20% more Damage
Minions deal 30% increased Damage
+100 to Evasion Rating
12% increased Armour
+20 to Dexterity
+20 to Intelligence
8% increased Evasion Rating
10% increased Cast Speed
20% increased Physical Damage
15% increased Damage
+30 to maximum Mana
+50 to maximum Energy Shield
6% increased maximum Energy Shield
10% increased Mana Regeneration Rate
3% increased Movement Speed
+10% to Chaos Resistance
+12% to Cold Resistance
+12% to Lightning Resistance
30% increased Elemental Damage
20% increased Spell Damage
25% increased Fire Damage
25% increased Cold Damage
25% increased Lightning Damage
10% increased Area of Effect
15% increased Projectile Speed
+200 to Accuracy Rating
Adds 5 to 10 Physical Damage to Attacks
Adds 1 to 40 Lightning Damage to Attacks
Adds 3 to 5 Cold Damage to Spells
10% chance to Freeze
10% chance to Shock
10% chance to Ignite
20% increased Effect of Non-Curse Auras from your Skills
30% increased Damage over Time
15% increased Poison Damage
20% increased Bleeding Damage
10% increased Burning Damage
2% of Life Regenerated per second
Regenerate 1% of Energy Shield per second
+1 to Level of all Fire Spell Skill Gems
+2 to Level of all Spell Skill Gems
20% increased Totem Damage
15% increased Trap Damage
15% increased Mine Damage
20% increased Brand Damage
30% increased Minion Life
20% increased Minion Movement Speed
+1 to maximum number of Summoned Totems
10% increased Flask Effect Duration
20% increased Flask Charges gained
25% increased effect of Non-Curse Auras you Cast
You have Onslaught while on Full Frenzy Charges
Attacks have 10% chance to cause Bleeding
Gain 5% of Physical Damage as Extra Fire Damage
10% of Physical Damage taken as Fire Damage
Enemies have -5% to Total Physical Damage Reduction against your Hits
+1 to Level of all Minion Skill Gems
20% increased Cooldown Recovery Rate
10% reduced Mana Cost of Skills
+5% to maximum Fire Resistance
Reflects 50 Physical Damage to Melee Attackers
5% additional Physical Damage Reduction
25% increased Stun Duration on Enemies
15% increased Stun Recovery
Cannot be Stunned
Gain 20% of Maximum Life as Extra Maximum Energy Shield
10% increased Attack and Cast Speed
20% increased Global Critical Strike Chance
+10% to Global Critical Strike Multiplier
Adds 1 to 2 Physical Damage to Attacks per 10 Strength
2% increased Attack Speed per Frenzy Charge
5% increased Spell Damage per Power Charge
10% increased Damage per Endurance Charge
+3% to Chaos Resistance per Endurance Charge
Point Blank
Acrobatics
Iron Reflexes
Chaos Inoculation
Vaal Pact
Resolute Technique
Elemental Overload
Avatar of Fire
Blood Magic
Mind Over Matter
Zealot's Oath
Ghost Reaver
Pain Attunement
Iron Grip
Iron Will
10% more Maximum Energy Shield
15% more Spell Damage while on Low Life
30% increased Damage while Leeching
40% increased Damage with Hits against Rare or Unique Enemies
Adds 1 to 3 Lightning Damage to Attacks per 10 Intelligence
```

Include at least 200 lines covering all categories from spec Section 10.2. The above is a starting point — expand to 200+ covering weapon-specific, conditional, per-stat, penetration, conversion, leech, flag, gem-specific, minion, and edge-case patterns.

- [ ] **Step 2: Create generate_expected.lua**

```lua
-- Generate expected mod output from PoB's ModParser for Rust integration tests.
-- Run from the PoB root: luajit tools/modparser-codegen/test_data/generate_expected.lua
--
-- Outputs JSON to stdout.

-- Bootstrap PoB's module loading
package.path = package.path .. ";third-party/PathOfBuilding/src/?.lua"
package.path = package.path .. ";third-party/PathOfBuilding/src/Modules/?.lua"
package.path = package.path .. ";third-party/PathOfBuilding/src/Data/?.lua"

-- Minimal PoB environment setup
-- (This needs to load enough of PoB's infrastructure to run ModParser.
--  The exact bootstrap sequence depends on PoB's internal module loading.
--  This may need adjustment based on PoB's actual require chain.)

local function loadModParser()
    -- Load PoB environment
    -- ...
    -- Return the parseMod function
end

local parseMod = loadModParser()

-- Read stat lines
local lines = {}
for line in io.lines("tools/modparser-codegen/test_data/stat_lines.txt") do
    if line ~= "" and not line:match("^#") then
        table.insert(lines, line)
    end
end

-- Parse each line and collect results
local results = {}
for _, line in ipairs(lines) do
    local modList, extra = parseMod(line)
    local entry = {
        line = line,
        mods = {},
        extra = extra or "",
    }
    if modList then
        for _, m in ipairs(modList) do
            table.insert(entry.mods, {
                name = m.name,
                type = m.type,
                value = m.value,
                flags = m.flags or 0,
                keywordFlags = m.keywordFlags or 0,
                tagCount = m[1] and #m or 0,
            })
        end
    end
    table.insert(results, entry)
end

-- Output as JSON
-- (PoB doesn't include a JSON library by default — use a simple serializer)
local function toJson(v)
    -- Simple JSON serializer for our known structure
    -- ...
end

print(toJson(results))
```

Note: Getting PoB's ModParser to run standalone requires bootstrapping its environment. This script may need significant adjustment. An alternative is to extract the expected output by running a modified PoB build. The exact approach will depend on what's feasible with the submodule.

- [ ] **Step 3: Run the Lua script and generate expected_mods.json**

Run: `cd /Users/andreashoffmann1/projects/pob-wasm && luajit tools/modparser-codegen/test_data/generate_expected.lua > tools/modparser-codegen/test_data/expected_mods.json`

If the Lua environment setup is too complex, an alternative approach is to hand-write the expected output for the 200+ lines based on knowledge of PoB's parsing rules. This is more tedious but avoids the Lua bootstrapping problem.

- [ ] **Step 4: Commit**

```bash
git add tools/modparser-codegen/test_data/
git commit -m "feat(phase3): add Lua reference data generator and 200+ test stat lines"
```

---

## Task 12: Rust Integration Tests

**Files:**
- Create: `crates/pob-calc/tests/mod_parser_test.rs`

- [ ] **Step 1: Write integration tests**

```rust
//! Integration tests for mod_parser — compares parse_mod() output against
//! PoB's Lua ModParser output for 200+ representative stat lines.

use pob_calc::build::mod_parser;
use pob_calc::mod_db::types::*;

fn src() -> ModSource {
    ModSource::new("Test", "test")
}

// --- Basic increased/reduced ---

#[test]
fn parse_inc_max_life() {
    let mods = mod_parser::parse_mod("10% increased maximum Life", src());
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].name, "Life");
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - 10.0).abs() < 0.001);
    assert_eq!(mods[0].flags, ModFlags::NONE);
    assert_eq!(mods[0].keyword_flags, KeywordFlags::NONE);
}

#[test]
fn parse_red_mana_cost() {
    let mods = mod_parser::parse_mod("5% reduced Mana Cost of Skills", src());
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].name, "ManaCost");
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - (-5.0)).abs() < 0.001);
}

// --- More/less ---

#[test]
fn parse_more_attack_damage() {
    let mods = mod_parser::parse_mod("20% more Attack Damage", src());
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].mod_type, ModType::More);
    assert!((mods[0].value.as_f64() - 20.0).abs() < 0.001);
}

#[test]
fn parse_less_attack_speed() {
    let mods = mod_parser::parse_mod("10% less Attack Speed", src());
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].mod_type, ModType::More);
    assert!((mods[0].value.as_f64() - (-10.0)).abs() < 0.001);
}

// --- Flat base stats ---

#[test]
fn parse_flat_life() {
    let mods = mod_parser::parse_mod("+50 to maximum Life", src());
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].name, "Life");
    assert_eq!(mods[0].mod_type, ModType::Base);
    assert!((mods[0].value.as_f64() - 50.0).abs() < 0.001);
}

#[test]
fn parse_flat_strength() {
    let mods = mod_parser::parse_mod("+30 to Strength", src());
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].name, "Str");
    assert_eq!(mods[0].mod_type, ModType::Base);
    assert!((mods[0].value.as_f64() - 30.0).abs() < 0.001);
}

#[test]
fn parse_all_attributes() {
    let mods = mod_parser::parse_mod("+10 to all Attributes", src());
    assert!(mods.len() >= 3, "Expected ≥3 mods for all attributes, got {}", mods.len());
}

// --- Resistances ---

#[test]
fn parse_fire_resist() {
    let mods = mod_parser::parse_mod("+40% to Fire Resistance", src());
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].name, "FireResist");
    assert_eq!(mods[0].mod_type, ModType::Base);
    assert!((mods[0].value.as_f64() - 40.0).abs() < 0.001);
}

#[test]
fn parse_all_elemental_resistances() {
    let mods = mod_parser::parse_mod("+15% to all Elemental Resistances", src());
    assert!(mods.len() >= 3);
}

// --- Conversion ---

#[test]
fn parse_phys_to_fire_conversion() {
    let mods = mod_parser::parse_mod("50% of Physical Damage Converted to Fire Damage", src());
    assert!(!mods.is_empty(), "Should parse conversion mod");
}

// --- Keystones ---

#[test]
fn parse_keystone_point_blank() {
    let mods = mod_parser::parse_mod("Point Blank", src());
    assert!(!mods.is_empty(), "Should parse keystone");
}

// --- Flag mods ---

#[test]
fn parse_cannot_be_evaded() {
    let mods = mod_parser::parse_mod("Your hits can't be evaded", src());
    assert!(!mods.is_empty(), "Should parse flag mod");
}

// Add more tests to reach 200+ covering all categories from spec Section 10.2.
// Each test follows the same pattern: call parse_mod, assert on name/type/value/flags.
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p pob-calc --test mod_parser_test`

Expected: Most tests pass. Some may fail if the generated code has issues — debug and fix the emitter/codegen as needed.

- [ ] **Step 3: Fix any failures by iterating on codegen**

For each failing test:
1. Check what `parse_mod()` actually returns
2. Trace through the generated code to find where the parsing diverges
3. Fix the emitter, re-run codegen, re-check the test

- [ ] **Step 4: Commit**

```bash
git add crates/pob-calc/tests/mod_parser_test.rs
git commit -m "feat(phase3): add 200+ mod_parser integration tests"
```

---

## Task 13: Implement Manual Handlers

**Files:**
- Modify: `crates/pob-calc/src/build/mod_parser_manual.rs`

**Reference:** `crates/pob-calc/src/build/mod_parser_manual_manifest.rs` — contains stubs with Lua source comments for every entry that needs a manual handler.

- [ ] **Step 1: Read the manifest to understand the scope**

Open `mod_parser_manual_manifest.rs` and count the handlers needed. Group them by similarity:

- **Jewel/radius functions** (ExtraJewelFunc, getSimpleConv, getPerStat, getThreshold) — these are the most complex. They involve closures that operate on passive tree nodes. For Phase 3, emit a placeholder `Mod` with `ModType::List` and the relevant data, matching PoB's output structure.
- **Data-driven loops** (data.nonDamagingAilmentTypeList, etc.) — hardcode the ailment list in Rust (Chill, Freeze, Shock, Scorch, Brittle, Sap).
- **gemIdLookup-dependent** — these need gem name→ID mapping. Build a static HashMap from the Lua data or the existing Rust gems.json.
- **String manipulation** (gsub, firstToUpper) — implement Rust equivalents.
- **Conditional logic** (if/else on captures) — straightforward match arms.

- [ ] **Step 2: Implement Rust helper functions**

Add to `mod_parser_manual.rs`:

```rust
/// Capitalize the first letter of a string (equivalent to Lua's firstToUpper).
fn first_to_upper(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

/// Title-case a string: capitalize first letter of each word.
/// Equivalent to Lua's `string.gsub(" "..s, "%W%l", string.upper):sub(2)`.
fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|word| first_to_upper(word))
        .collect::<Vec<_>>()
        .join("")
}

/// The non-damaging ailment types from PoB's data.
const NON_DAMAGING_AILMENTS: &[&str] = &["Chill", "Freeze", "Shock", "Scorch", "Brittle", "Sap"];

/// Default mod constructor with source.
fn default_mod(name: &str, mod_type: ModType, value: f64, source: &ModSource) -> Mod {
    Mod {
        name: name.into(),
        mod_type,
        value: ModValue::Number(value),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags: vec![],
        source: source.clone(),
    }
}
```

- [ ] **Step 3: Implement handlers group by group**

Work through the manifest stubs. For each stub:
1. Read the Lua comment to understand what the handler should do
2. Write the Rust match arm
3. Test against the expected output

Example handler for the ailment loop pattern:

```rust
"non_damaging_ailments_reduced_effect_arcane_surge" => {
    // Line 2772: non%-damaging ailments have (%d+)%% reduced effect on you while you have arcane surge
    let num: f64 = caps[1].parse().unwrap_or(0.0);
    NON_DAMAGING_AILMENTS.iter().map(|ailment| {
        let mut m = default_mod(
            &format!("Self{ailment}Effect"), ModType::Inc, -num, source
        );
        m.tags.push(ModTag::Condition { var: "AffectedByArcaneSurge".into(), neg: false });
        m
    }).collect()
}
```

- [ ] **Step 4: Add the compile-time count assertion**

In `mod_parser_manual.rs`, after the match block, add:

```rust
/// Total number of manual handlers implemented.
/// Must match MANUAL_SPECIAL_COUNT in the generated file.
pub const IMPLEMENTED_MANUAL_COUNT: usize = /* count of match arms */;
```

In `mod_parser.rs`, add:

```rust
// Compile-time check: generated + manual = total
const _: () = assert!(
    generated::GENERATED_SPECIAL_COUNT + mod_parser_manual::IMPLEMENTED_MANUAL_COUNT
        == generated::TOTAL_SPECIAL_COUNT,
    "specialModList coverage is not 100% — regenerate or add manual handlers"
);
```

- [ ] **Step 5: Verify compilation with assertion**

Run: `cargo check -p pob-calc`

Expected: If the counts don't match, the assertion fails at compile time. Implement remaining handlers until it passes.

- [ ] **Step 6: Commit (may be multiple commits as handlers are implemented)**

```bash
git add crates/pob-calc/src/build/mod_parser_manual.rs crates/pob-calc/src/build/mod_parser.rs
git commit -m "feat(phase3): implement all manual special mod handlers (100% coverage)"
```

---

## Task 14: Delete item_parser.rs and Final Verification

**Files:**
- Modify: `crates/pob-calc/src/build/mod.rs`
- Delete: `crates/pob-calc/src/build/item_parser.rs`

- [ ] **Step 1: Verify all old item_parser tests pass with mod_parser**

The 10 inline tests in `item_parser.rs` test specific stat lines. Ensure equivalent tests exist in `mod_parser_test.rs`. Compare:

| item_parser test | Stat line | Expected mod_parser equivalent |
|---|---|---|
| `parses_base_life` | `"+40 to maximum Life"` | `parse_flat_life` or equivalent |
| `parses_inc_life` | `"8% increased maximum Life"` | `parse_inc_max_life` or equivalent |
| `parses_fire_resist` | `"+30% to Fire Resistance"` | `parse_fire_resist` |
| `unknown_stat_returns_empty` | `"Socketed Gems..."` | verify mod_parser also returns empty |
| `parses_all_attributes` | `"+10 to all Attributes"` | `parse_all_attributes` |
| `parses_all_elemental_resists` | `"+15% to all Elemental Resistances"` | `parse_all_elemental_resistances` |
| `parses_inc_evasion` | `"12% increased Evasion Rating"` | add test if missing |
| `parses_inc_physical_damage` | `"20% increased Physical Damage"` | add test if missing |

- [ ] **Step 2: Remove item_parser module declaration**

In `crates/pob-calc/src/build/mod.rs`, change:

```rust
// Before:
pub mod item_parser;
pub mod mod_parser;

// After:
pub mod mod_parser;
```

- [ ] **Step 3: Delete item_parser.rs**

```bash
rm crates/pob-calc/src/build/item_parser.rs
```

- [ ] **Step 4: Run full test suite**

Run: `cargo test --workspace --exclude pob-wasm --exclude modparser-codegen`

Expected: All tests pass. No references to `item_parser` remain in compiled code.

- [ ] **Step 5: Run the codegen tool's own tests**

Run: `cargo test -p modparser-codegen`

Expected: All codegen tests pass.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(phase3): remove item_parser.rs, mod_parser is now the sole stat parser"
```

---

## Task 15: CI Freshness Check

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add freshness check step to CI**

In `.github/workflows/ci.yml`, add a step to the `test` job after the existing test step:

```yaml
      - name: Check generated code is fresh
        run: |
          cargo run -p modparser-codegen -- \
            --output /tmp/mod_parser_generated.rs \
            --manifest /tmp/mod_parser_manual_manifest.rs
          diff crates/pob-calc/src/build/mod_parser_generated.rs /tmp/mod_parser_generated.rs || \
            (echo "ERROR: mod_parser_generated.rs is stale. Run 'cargo run -p modparser-codegen' and commit." && exit 1)
          diff crates/pob-calc/src/build/mod_parser_manual_manifest.rs /tmp/mod_parser_manual_manifest.rs || \
            (echo "ERROR: mod_parser_manual_manifest.rs is stale. Run 'cargo run -p modparser-codegen' and commit." && exit 1)
```

- [ ] **Step 2: Update CI test command to exclude modparser-codegen from workspace tests**

Change the test step:

```yaml
      - name: Run native tests (pob-calc + data-extractor)
        run: cargo test --workspace --exclude pob-wasm --exclude modparser-codegen
```

And add a separate step for codegen tests:

```yaml
      - name: Run codegen tests
        run: cargo test -p modparser-codegen
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci(phase3): add mod_parser_generated.rs freshness check and codegen tests"
```

---

## Task 16: Final End-to-End Verification

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace --exclude pob-wasm --exclude modparser-codegen`

Expected: All pass.

- [ ] **Step 2: Run codegen tests**

Run: `cargo test -p modparser-codegen`

Expected: All pass.

- [ ] **Step 3: Run mod_parser integration tests**

Run: `cargo test -p pob-calc --test mod_parser_test -- --nocapture`

Expected: All 200+ tests pass.

- [ ] **Step 4: Verify freshness**

Run: `cargo run -p modparser-codegen -- --output /tmp/gen.rs --manifest /tmp/man.rs && diff crates/pob-calc/src/build/mod_parser_generated.rs /tmp/gen.rs`

Expected: No diff.

- [ ] **Step 5: Verify no references to item_parser remain**

Run: `grep -r "item_parser\|parse_stat_text" crates/ --include="*.rs"`

Expected: No matches in source files (only in test data or comments if any).

- [ ] **Step 6: Check compile-time assertion**

Run: `cargo check -p pob-calc`

Expected: Compiles cleanly. The `const _: () = assert!(...)` passes.

- [ ] **Step 7: Final commit if any loose changes**

```bash
git status
# If clean, no commit needed.
# If there are changes:
git add -A
git commit -m "fix(phase3): final cleanup and verification"
```
