# ModParser Code Generator — Design Spec

**Date:** 2026-03-30
**Phase:** 3 of pob-wasm parity roadmap
**Parent spec:** `2026-03-30-pob-wasm-parity-design.md` Section 7

## 1. Problem

`item_parser.rs` handles 28 stat text patterns using hand-written string matching.
PoB's `ModParser.lua` has ~5,650 patterns across 6 major tables.
Without these patterns, items, passives, jewels, and enchants contribute almost no stats to the ModDb.

## 2. Solution Overview

A Rust code-generation tool that reads `ModParser.lua` and emits a Rust source file
implementing the same multi-stage parsing pipeline. The generated file is **checked into
the repo** (not generated at build time), with a CI freshness check to prevent drift.

### Decisions

| Decision | Choice | Rationale |
|---|---|---|
| specialModList strategy | Template classification + manual fallback | 100% coverage: ~20 templates handle ~80%, rest hand-written from codegen-emitted stubs |
| Build integration | Checked-in generated file | Simpler build, reviewable in PRs, submodule is pinned so Lua rarely changes |
| Missing ModTag variants | Add as needed | Keeps codegen coverage high, natural extension of Phase 2 |

## 3. Architecture

Two components:

1. **`tools/modparser-codegen/`** — Rust binary that parses ModParser.lua and emits Rust code
2. **`crates/pob-calc/src/build/mod_parser.rs`** — Runtime wrapper exposing `parse_mod()`

### 3.1 File Layout

```
tools/modparser-codegen/
  Cargo.toml                  # Separate binary crate (not in workspace build path)
  src/
    main.rs                   # CLI: reads Lua file, orchestrates pipeline, writes output
    lua_parser.rs             # Extracts 6 tables from ModParser.lua as structured data
    pattern_translator.rs     # Lua pattern syntax → Rust regex syntax
    emitter.rs                # Generates Rust source from parsed data
    templates.rs              # specialModList function classifiers and emitters
    types.rs                  # Internal AST: LuaPattern, FormType, ModNameEntry, etc.

crates/pob-calc/src/build/
  mod_parser.rs               # NEW: thin wrapper, imports generated + manual code
  mod_parser_generated.rs     # NEW: checked-in generated file (~5,000+ lines)
  mod_parser_manual.rs        # NEW: hand-written handlers for entries codegen can't template
  item_parser.rs              # DELETED after mod_parser is verified
  mod.rs                      # UPDATED: replace item_parser with mod_parser
```

### 3.2 Data Flow

```
ModParser.lua (6,762 lines)
  │
  ▼
tools/modparser-codegen (Rust binary)
  ├── lua_parser: extracts 6 tables as AST
  ├── pattern_translator: Lua patterns → Rust regex strings
  ├── templates: classifies specialModList functions into ~20 templates
  └── emitter: generates Rust source
  │
  ├─────────────────────────┐
  ▼                         ▼
mod_parser_generated.rs   mod_parser_manual_manifest.rs
(templated entries)       (stubs for hand-written entries)
  │                         │
  │                         ▼
  │                       mod_parser_manual.rs
  │                       (hand-written handlers)
  │                         │
  ├─────────────────────────┘
  ▼
mod_parser.rs (runtime wrapper, merges both)
  │
  ▼
parse_mod(line, source) -> Vec<Mod>
```

## 4. Lua Parser (`lua_parser.rs`)

Parses ModParser.lua as **structured text extraction**, not a full Lua parser.
Each table has a known format that can be extracted with targeted parsing.

### 4.1 Tables to Extract

| Table | Lines | Entries | Key type | Value type | Parse strategy |
|---|---|---|---|---|---|
| `formList` | 67–153 | 85 | Lua pattern | Form string (INC/RED/MORE/LESS/BASE/FLAG/...) | Pattern + form name pairs |
| `modNameList` | 156–1056 | ~687 | Plain text | Stat name(s) + optional tags | Key-value with nested table detection |
| `modFlagList` | 870–1056 | ~181 | Plain text | flags/keywordFlags | Key-value with flag field extraction |
| `preFlagList` | 1059–1272 | ~188 | Lua pattern | flags/tags/misc fields | Pattern + structured value extraction |
| `modTagList` | 1275–1935 | ~637 | Lua pattern | Tag type + fields, or function | Pattern + tag extraction, function classification |
| `specialModList` | 2030–5697 | ~2,030 | Lua pattern | Mod table or function | Pattern + template classification |

### 4.2 Form Types

The 25 distinct form types found in `formList`:

```
INC, RED, MORE, LESS, BASE, GAIN, LOSE, GRANTS, REMOVES, CHANCE,
FLAG, TOTALCOST, BASECOST, PEN, REGENFLAT, REGENPERCENT, DEGENFLAT,
DEGENPERCENT, DEGEN, DMG, DMGATTACKS, DMGSPELLS, DMGBOTH, OVERRIDE, DOUBLED
```

### 4.3 Parsing Approach

Each table occupies a known, delimited region of the file. The parser:

1. Reads the entire file into memory
2. Locates each table by finding its opening line (e.g., `local formList = {`)
3. Iterates lines within the table, extracting key-value pairs
4. Handles nested tables `{ ... }` by brace-counting
5. Detects function values by `function(` keyword and extracts the full body via brace-counting
6. Strips Lua comments (`--`)

For `modNameList` values, three shapes are recognized:
- `"Str"` — single stat name (string literal)
- `{ "Str", "Dex" }` — multiple stat names (array of strings)
- `{ "ManaCost", tag = { type = "SkillType", skillType = SkillType.Attack } }` — stat name(s) with inline tag(s)

For `modTagList` and `preFlagList`, function values are extracted as raw Lua source
for template classification.

## 5. Lua Pattern → Rust Regex (`pattern_translator.rs`)

Translates Lua's pattern matching syntax to Rust `regex` crate syntax.

### 5.1 Translation Table

| Lua | Rust regex | Description |
|---|---|---|
| `%d` | `\d` | Digit |
| `%d+` | `\d+` | One or more digits |
| `%a` | `[a-zA-Z]` | Letter |
| `%a+` | `[a-zA-Z]+` | One or more letters |
| `%w` | `\w` | Alphanumeric |
| `%l` | `[a-z]` | Lowercase letter |
| `%u` | `[A-Z]` | Uppercase letter |
| `%s` | `\s` | Whitespace |
| `%p` | `[[:punct:]]` | Punctuation |
| `%%` | `%` | Literal percent |
| `%-` | `\-` | Literal hyphen |
| `%+` | `\+` | Literal plus |
| `%.` | `\.` | Literal dot |
| `(...)` | `(...)` | Capture group (preserved) |
| `(.-)` | `(.*?)` | Non-greedy capture |
| `(.+)` | `(.+)` | Greedy capture |
| `[%d%.]` | `[\d.]` | Character class with escapes |
| `[%+%-]` | `[+\-]` | Character class with literal chars |
| `X?` | `X?` | Zero or one (same semantics) |
| `^` | `^` | Start anchor |

### 5.2 Key Differences

- Lua's `?` means "zero or one of the **previous character**" (not a quantifier on a group).
  In most patterns this aligns with regex `?`, but the translator must handle edge cases.
- Lua has no alternation (`|`). The character-class trick `[hd][ae][va][el]` is preserved as-is
  since it's valid regex too.
- Lua's `-` (non-greedy) in `(.-)` becomes `(.*?)` in regex.

## 6. Template Classification (`templates.rs`)

For `specialModList` entries that are functions, the classifier inspects the Lua function
body and matches it to a known template. Templates are ordered from most specific to
most general.

### 6.1 Template Catalog

| Template | Pattern in Lua source | Emitted Rust | Est. entries |
|---|---|---|---|
| `SimpleFlagMod` | `{ flag("X") }` | `vec![Mod::new_flag("X", source)]` | ~200 |
| `SimpleMod` | `{ mod("X", "TYPE", num) }` | `vec![Mod { name: "X", mod_type: TYPE, value: num, ... }]` | ~300 |
| `MultiMod` | `{ mod(...), mod(...), ... }` (static table, no function) | Multiple Mod constructors | ~400 |
| `DamageConversion` | `"PhysicalDamageConvertTo"..firstToUpper(type)` | Captures damage type, maps via lookup | ~30 |
| `DamageGainAs` | `"PhysicalDamageGainAs"..type` | Same pattern as conversion | ~20 |
| `GrantedSkill` | `grantedExtraSkill(name, level)` | `Mod::granted_skill(name, level)` | ~50 |
| `TriggerSkill` | `triggerExtraSkill(name, level, ...)` | `Mod::trigger_skill(name, level)` | ~40 |
| `ExtraSupport` | `extraSupport(name, level, ...)` | `Mod::extra_support(name, level)` | ~30 |
| `ExplodeMod` | `explodeFunc(chance, amount, type)` | `Mod::explode(chance, amount, type)` | ~15 |
| `EnemyModifier` | `mod("EnemyModifier", "LIST", { mod = mod(...) })` | Wraps inner mod in EnemyModifier | ~80 |
| `MinionModifier` | `mod("MinionModifier", "LIST", { mod = mod(...) })` | Wraps inner mod in MinionModifier | ~60 |
| `ConditionalFlag` | `flag("X"), tag/tagList` with Condition tags | Flag mod with condition tags | ~100 |
| `ConditionalMod` | `mod("X", "TYPE", num)` with dynamic tags from captures | Mod with captured tag values | ~150 |
| `AuraEffect` | `ExtraAuraEffect` LIST wrapper | Wraps mods for aura application | ~30 |
| `DamageTypeCapture` | Captures `(.+) damage` and maps type string | Lookup table for damage type → stat name | ~50 |
| `ResourceTypeCapture` | Captures resource type (life/mana/ES) | Lookup table for resource → stat names | ~30 |
| `PercentOfCapture` | "N% of X as extra Y" patterns | Conversion/gain-as mod construction | ~20 |
| `PenTypeCapture` | Captures resistance type for penetration | Lookup table for pen type → stat name | ~15 |
| `NumericScaling` | `function(num) return { mod("X", "BASE", num * N) }` | Scales captured number by constant | ~50 |
### 6.2 Classification Algorithm

```
for each specialModList entry:
  if value is a table (not function):
    → classify as MultiMod or SimpleMod or SimpleFlagMod
  else (value is a function):
    body = extract function body
    if body matches "flag(" pattern only:
      → SimpleFlagMod
    if body matches single "mod(" with no dynamic parts:
      → SimpleMod
    if body contains "firstToUpper" or "ConvertTo":
      → DamageConversion
    if body contains "grantedExtraSkill":
      → GrantedSkill
    if body contains "triggerExtraSkill":
      → TriggerSkill
    ... (ordered most-specific → least-specific)
    else:
      → ManualRequired (emit pattern stub into manifest)
```

### 6.3 Coverage Target: 100%

**Every** specialModList entry must be handled. The two-tier strategy:

1. **Codegen-templated** (~1,600-1,700 entries) — automatically emitted into
   `mod_parser_generated.rs` by template classification.
2. **Hand-written** (~300-400 entries) — the codegen emits a **manifest file**
   (`mod_parser_manual_manifest.rs`) listing every un-templated entry as a Rust stub:
   its pattern (as compiled regex), capture count, Lua source line number, and the
   original Lua function body as a comment. A human then implements each handler in
   `mod_parser_manual.rs`, guided by the stub and the Lua comment.

The codegen also emits a compile-time assertion:
```rust
const _: () = assert!(
    GENERATED_SPECIAL_COUNT + MANUAL_SPECIAL_COUNT == TOTAL_SPECIAL_COUNT,
    "specialModList coverage is not 100% — regenerate or add manual handlers"
);
```

The runtime `try_special_mod()` function checks both the generated and manual tables.
Order is: generated patterns first (they're more numerous and faster to match),
then manual patterns.

### 6.4 Manual Handler Workflow

When the codegen runs, it produces two outputs:
1. `mod_parser_generated.rs` — all templated entries (as before)
2. `mod_parser_manual_manifest.rs` — stubs for entries needing hand-written handlers

Each stub in the manifest looks like:
```rust
// Line 3847 in ModParser.lua:
// ["(%d+)%% chance to gain a power charge on critical strike"] =
//   function(num) return {
//     mod("PowerChargeOnCritChance", "BASE", num),
//     flag("PowerChargeOnCrit"),
//   } end,
(
    Lazy::new(|| Regex::new(r"(\d+)% chance to gain a power charge on critical strike").unwrap()),
    2,  // capture count
    "power_charge_on_crit",  // handler ID
)
```

The developer writes the matching handler in `mod_parser_manual.rs`:
```rust
pub fn handle_manual_special(id: &str, caps: &regex::Captures, source: &ModSource) -> Vec<Mod> {
    match id {
        "power_charge_on_crit" => {
            let num: f64 = caps[1].parse().unwrap_or(0.0);
            vec![
                Mod { name: "PowerChargeOnCritChance".into(), mod_type: ModType::Base,
                      value: ModValue::Number(num), ..default(source) },
                Mod::new_flag("PowerChargeOnCrit", source.clone()),
            ]
        }
        // ... all other manual handlers ...
        other => {
            // Compile-time count assertion prevents this from being reachable
            // if all handlers are implemented, but guard defensively at runtime.
            eprintln!("unhandled manual special mod: {other}");
            vec![]
        }
    }
}
```

This ensures:
- Nothing is silently skipped
- The Lua source is right there as a comment for reference
- The compile-time count assertion catches missing handlers
- New entries from Lua updates are caught by the CI freshness check

## 7. Generated Code Structure (`mod_parser_generated.rs`)

The emitter produces a single Rust source file with the following structure:

```rust
// ==========================================================
// AUTO-GENERATED from ModParser.lua — do not edit manually
// Generated by tools/modparser-codegen
// Source: third-party/PathOfBuilding/src/Modules/ModParser.lua
// Coverage: N/M specialModList entries (X%), all other tables 100%
// ==========================================================

use std::collections::HashMap;
use once_cell::sync::Lazy;
use regex::Regex;
use crate::mod_db::types::*;

// --- Form patterns (85 entries) ---
#[derive(Debug, Clone, Copy, PartialEq)]
enum FormType {
    Inc, Red, More, Less, Base, Gain, Lose, Grants, Removes,
    Chance, Flag, TotalCost, BaseCost, Pen, RegenFlat, RegenPercent,
    DegenFlat, DegenPercent, Degen, Dmg, DmgAttacks, DmgSpells,
    DmgBoth, Override, Doubled,
}

struct FormPattern {
    regex: Lazy<Regex>,
    form: FormType,
}

static FORM_PATTERNS: &[FormPattern] = &[
    // 85 entries...
];

// --- Mod name lookup (687 entries, plain text) ---
struct ModNameEntry {
    names: &'static [&'static str],   // stat name(s)
    tags: &'static [ModTag],          // inline tags
}

static MOD_NAME_MAP: Lazy<HashMap<&'static str, ModNameEntry>> = Lazy::new(|| {
    let mut m = HashMap::with_capacity(700);
    // 687 entries...
    m
});

// --- Pre-flag patterns (188 entries) ---
struct PreFlagEntry {
    flags: ModFlags,
    keyword_flags: KeywordFlags,
    tags: Vec<ModTag>,
    add_to_minion: bool,
    add_to_skill: bool,
    add_to_aura: bool,
    apply_to_enemy: bool,
}

// ... (similar structures for modFlagList, modTagList)

// --- Special mod patterns (~2,030 entries) ---
type SpecialModFn = fn(caps: &regex::Captures, source: &ModSource) -> Vec<Mod>;

struct SpecialModPattern {
    regex: Lazy<Regex>,
    handler: SpecialModFn,
}

static SPECIAL_MOD_PATTERNS: &[SpecialModPattern] = &[
    // ~1,700 classified entries...
];

// --- Lookup tables ---
static DAMAGE_TYPE_MAP: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
    [("physical", "Physical"), ("fire", "Fire"), ("cold", "Cold"),
     ("lightning", "Lightning"), ("chaos", "Chaos")].into()
});

static SUFFIX_TYPE_MAP: Lazy<HashMap<&str, &str>> = Lazy::new(|| { /* ... */ });

// --- Scan function (mirrors ModParser.lua's scan()) ---
fn scan_patterns<T>(line: &str, patterns: &[(Regex, T)]) -> Option<(usize, &T, regex::Captures)> {
    // Find earliest match, break ties by longest match, then longest pattern
    // Return matched entry + captures, remainder computed by caller
}

fn scan_plain<T>(line: &str, map: &HashMap<&str, T>) -> Option<(usize, usize, &T)> {
    // Find earliest plain-text substring match
    // Return (start, end, entry)
}

// --- Main pipeline ---
pub fn parse_mod_generated(line: &str, source: &ModSource) -> Vec<Mod> {
    let line = line.to_lowercase();
    let line = line.trim();

    // 1. Special mod (full-line match, short-circuits)
    for sp in SPECIAL_MOD_PATTERNS.iter() {
        if let Some(caps) = sp.regex.find(&line) {
            if caps.as_str().len() == line.len() {  // full match
                return (sp.handler)(&caps, source);
            }
        }
    }

    // 2. Pre-flags (prefix extraction)
    let (line, pre) = extract_pre_flags(&line);

    // 3. Form match (required)
    let (line, form, num) = match extract_form(&line) {
        Some(r) => r,
        None => return vec![],
    };

    // 4. Mod tags (suffix conditions, up to 2 passes)
    let (line, tags1) = extract_mod_tags(&line);
    let (line, tags2) = extract_mod_tags(&line);

    // 5. Mod name (plain text lookup)
    let (line, name_entry) = match extract_mod_name(&line, &form) {
        Some(r) => r,
        None => return vec![],
    };

    // 6. Mod flags (weapon/skill type, plain text)
    let (_line, flag_entry) = extract_mod_flags(&line);

    // 7. Construct mods
    build_mods(form, num, name_entry, pre, flag_entry, tags1, tags2, source)
}
```

### 7.1 scan() Semantics

The `scan()` function in ModParser.lua finds the **earliest** match among all patterns,
breaking ties by **longest match length**, then by **longest pattern string**. This is
critical for correctness — the Rust implementation must replicate this tie-breaking.

For regex-based tables (`formList`, `preFlagList`, `modTagList`, `specialModList`):
iterate all patterns, find all matches, select by (earliest start, longest match, longest pattern).

For plain-text tables (`modNameList`, `modFlagList`, `suffixTypes`):
use substring search, same tie-breaking rules.

After a match, the matched portion is **excised** from the line (text before + text after
the match are concatenated). This progressive excision is how multiple components are
extracted from a single line.

### 7.2 Regex Performance

With ~3,600 regex patterns, naive iteration would be slow. Mitigations:

1. **`once_cell::Lazy`** — each regex compiled once on first use
2. **Early termination** — specialModList checked first; if it fully matches, skip everything else
3. **Pattern ordering** — emit patterns in frequency order (most common first) where possible
4. **RegexSet potential** — for future optimization, `regex::RegexSet` can test all patterns
   in a single pass. Not needed initially but noted as an optimization path.

## 8. Runtime Wrapper (`mod_parser.rs`)

Thin module that imports both the generated code and the manual handlers:

```rust
// crates/pob-calc/src/build/mod_parser.rs

#[path = "mod_parser_generated.rs"]
mod generated;

mod mod_parser_manual;

use crate::mod_db::types::{Mod, ModSource};

/// Parse a stat text line into zero or more Mod values.
///
/// Mirrors Path of Building's ModParser.parseMod() function.
/// 100% coverage of ModParser.lua's pattern tables — templated entries
/// handled by generated code, remaining entries by hand-written handlers.
pub fn parse_mod(line: &str, source: ModSource) -> Vec<Mod> {
    generated::parse_mod_generated(line, &source, &mod_parser_manual::handle_manual_special)
}
```

The generated `parse_mod_generated()` accepts a callback for manual handlers.
When `try_special_mod()` matches a pattern that was classified as `ManualRequired`,
it delegates to the callback instead of inlining the handler.

### 8.1 Normalization

Before pattern matching, the input line is:
1. Stripped of PoB colour codes (`^8`, `^xABCDEF`)
2. Converted to lowercase
3. Trimmed of leading/trailing whitespace
4. Trailing space appended (matches ModParser.lua's `line = line .. " "`)

## 9. New ModTag Variants

Added to `crates/pob-calc/src/mod_db/types.rs`:

```rust
// Existing 12 variants remain unchanged. New variants:
pub enum ModTag {
    // ... existing ...
    SkillName { name: String },
    SkillId { id: String },
    SkillPart { part: u32 },
    SocketedIn { slot_name: String },
    ItemCondition { var: String, neg: bool },
}
```

The `eval_mod()` function in `mod_db/eval.rs` is updated to handle these new variants.
For Phase 3, the new tag evaluation can be stubbed (always returns true) since the
calc modules that consume these tags aren't ported yet. The important thing is that
the tags are **stored correctly** so they're ready for Phase 4+.

## 10. Verification Strategy

### 10.1 Lua Reference Data

A Lua script at `tools/modparser-codegen/test_data/generate_expected.lua`:
- Loads PoB's ModParser module
- Feeds 200+ representative stat lines through `parseMod()`
- Serializes each result as JSON: `{ "line": "...", "mods": [...] }`
- Output stored as `test_data/expected_mods.json` (checked into repo)

### 10.2 Stat Line Categories (200+ lines)

| Category | Count | Examples |
|---|---|---|
| Basic increased/reduced | 20 | "10% increased maximum Life", "5% reduced Mana Cost" |
| More/less | 10 | "20% more Attack Damage", "10% less Attack Speed" |
| Flat base stats | 20 | "+50 to maximum Life", "+30 to Strength" |
| Resistances | 10 | "+40% to Fire Resistance", "+15% to all Elemental Resistances" |
| Damage ranges | 10 | "Adds 10 to 20 Fire Damage to Attacks" |
| Critical strikes | 10 | "50% increased Critical Strike Chance", "+1.5% to Critical Strike Multiplier" |
| Weapon-specific | 15 | "15% increased Physical Damage with Axes", "20% increased Attack Speed with Swords" |
| Conditional mods | 15 | "10% increased Attack Speed while Dual Wielding" |
| Per-charge/stat mods | 10 | "4% increased Attack Damage per Frenzy Charge" |
| Penetration | 5 | "Damage Penetrates 10% Fire Resistance" |
| Conversion | 5 | "50% of Physical Damage Converted to Fire Damage" |
| Leech | 5 | "1% of Physical Attack Damage Leeched as Life" |
| Flag mods | 10 | "Your hits can't be evaded", "Chaos Damage does not bypass Energy Shield" |
| Gem-specific | 10 | "Supported Skills deal 20% more Damage" |
| Minion mods | 10 | "Minions deal 30% increased Damage" |
| Special/unique | 20 | Various specialModList patterns with captures |
| Edge cases | 15 | Multi-stat mods, all-attributes, composite patterns |

### 10.3 Rust Integration Tests

`crates/pob-calc/tests/mod_parser_test.rs`:
- Loads `expected_mods.json`
- For each line, calls `parse_mod(line, test_source)`
- Compares output against expected: stat names, mod types, values, flags, keyword flags, tag types
- Tags compared structurally (type + key fields), not by exact float equality (epsilon = 0.001)

### 10.4 CI Freshness Check

A CI step that:
1. Runs `cargo run -p modparser-codegen -- third-party/PathOfBuilding/src/Modules/ModParser.lua --output /tmp/mod_parser_generated.rs`
2. Diffs `/tmp/mod_parser_generated.rs` against `crates/pob-calc/src/build/mod_parser_generated.rs`
3. Fails if they differ, with message: "Generated code is stale — run `cargo run -p modparser-codegen` and commit the result"

## 11. Integration Changes

### 11.1 `crates/pob-calc/src/build/mod.rs`

```rust
// Before:
pub mod item_parser;

// After:
pub mod mod_parser;
```

### 11.2 Call Site Migration

All call sites of `item_parser::parse_stat_text(text, source)` change to
`mod_parser::parse_mod(text, source)`. The signature is identical (`&str, ModSource -> Vec<Mod>`).

Search for `parse_stat_text` and `item_parser` across the crate to find all call sites.

### 11.3 `item_parser.rs` Removal

After `mod_parser` is verified (all existing item_parser tests pass against the new
implementation, plus 200+ new lines), `item_parser.rs` is deleted.

## 12. Cargo Workspace Changes

### 12.1 New Crate

```toml
# tools/modparser-codegen/Cargo.toml
[package]
name = "modparser-codegen"
version = "0.1.0"
edition = "2021"

[dependencies]
regex = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
```

### 12.2 Workspace Membership

Add to workspace `Cargo.toml` members:
```toml
members = ["crates/pob-calc", "crates/pob-wasm", "crates/data-extractor", "tools/modparser-codegen"]
```

The CI test command already uses `--exclude pob-wasm` and will also exclude
`modparser-codegen`: `cargo test --workspace --exclude pob-wasm --exclude modparser-codegen`.
The codegen tool has its own unit tests run separately via `cargo test -p modparser-codegen`.

### 12.3 Runtime Dependencies

`crates/pob-calc/Cargo.toml` gains:
```toml
regex = "1"
once_cell = "1"
```

## 13. Exit Criteria

1. `parse_mod("10% increased maximum Life", src)` returns `Mod { name: "Life", mod_type: Inc, value: 10.0, flags: NONE, keyword_flags: NONE, tags: [] }`
2. 200+ stat lines match PoB's Lua output (structural comparison)
3. All 28 existing `item_parser` test cases pass against `mod_parser`
4. **100% coverage** of all 6 tables: `formList`, `modNameList`, `modFlagList`, `preFlagList`, `modTagList`, `specialModList`
5. Every specialModList entry handled — either by codegen template or by hand-written handler in `mod_parser_manual.rs`
6. Compile-time assertion verifies generated + manual count == total count
7. Generated code compiles with no warnings
8. `item_parser.rs` deleted
9. CI green: `cargo test --workspace --exclude pob-wasm`
10. CI freshness check passes

## 14. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Lua parser fails on edge-case formatting | Medium | Blocks codegen | Comprehensive test suite of extracted entries; fall back to line-by-line debugging |
| Manual handler volume (~300-400) is large | High | Significant effort | Manifest stubs with Lua comments make each handler quick to write; most follow 3-4 recurring shapes |
| Regex performance with ~3,600 patterns | Low | Slow parse_mod() | Lazy compilation; future RegexSet optimization; specialModList short-circuits |
| Generated code too large for compiler | Low | Build failure | Split into multiple modules if needed; current estimate ~5,000-8,000 lines is fine |
| scan() tie-breaking semantics wrong | Medium | Wrong mod selection | Verify against Lua on ambiguous inputs; integration tests catch mismatches |
| Missing ModTag variants break eval_mod() | Low | Test failures | Stub new tag evaluation (always true) until Phase 4 |
| Manual handlers have bugs vs Lua | Medium | Wrong mod output | Each manual handler tested against Lua reference data in the 200+ line test suite |
