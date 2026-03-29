# Phase 3: Core Types — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the foundational data structures in `pob-calc`: `ModDb`, `GameData` loader, `Build` model, and the POB XML parser. At the end of this phase, you can parse a POB build XML into a `Build` struct and load game data JSON into `GameData` — no calculations yet.

**Architecture:** All types live in `pob-calc`. `GameData` loads from the JSON files produced by Phase 2. `Build` is parsed from POB's XML format. `ModDb` is the central modifier store — a `HashMap<String, Vec<Mod>>` with a parent pointer for inheritance, exposing `sum`, `more`, `flag`, and `tabulate` query methods.

**Tech Stack:** Rust 1.82, `serde/serde_json`, `quick-xml 0.36` for XML parsing, `thiserror`, `base64 0.22`

**Prerequisites:** Phase 1 complete (workspace compiles). Phase 2 data files committed to `data/` (or `GGPK_PATH` available to generate them).

**Reference:**
- `third-party/PathOfBuilding/src/Classes/ModDB.lua` — modifier database
- `third-party/PathOfBuilding/src/Classes/ModStore.lua` — query methods
- `third-party/PathOfBuilding/src/Modules/Build.lua` — build model
- `third-party/PathOfBuilding/src/Classes/PassiveSpec.lua` — passive tree spec
- `third-party/PathOfBuilding/src/Classes/Item.lua` — item model

---

## File Map

```
crates/pob-calc/
  Cargo.toml                   ← add quick-xml, base64
  src/
    lib.rs                     ← re-exports public API
    error.rs                   ← CalcError, ParseError, DataError
    data/
      mod.rs                   ← GameData struct + load_from_json()
      gems.rs                  ← GemData, loaded from gems.json
      bases.rs                 ← BaseItem, loaded from bases.json
      misc.rs                  ← MiscData (game constants), loaded from misc.json
    mod_db/
      mod.rs                   ← ModDb struct, add/query methods
      types.rs                 ← Mod, ModType, ModFlags, ModValue, Condition, ModSource
    build/
      mod.rs                   ← Build struct, re-exports parser
      types.rs                 ← CharacterSpec, PassiveSpec, SkillSet, ItemSet, BuildConfig
      xml_parser.rs            ← parse_xml(xml: &str) -> Result<Build, ParseError>
      item_parser.rs           ← parse_item_text(text: &str) -> Result<Item, ParseError>
    passive_tree/
      mod.rs                   ← PassiveTree, PassiveNode — loaded from tree JSON
```

---

### Task 1: Errors and `pob-calc` dependencies

**Files:**
- Modify: `crates/pob-calc/Cargo.toml`
- Modify: `crates/pob-calc/src/lib.rs`
- Create: `crates/pob-calc/src/error.rs`

- [ ] **Step 1: Update `crates/pob-calc/Cargo.toml`**

```toml
[package]
name = "pob-calc"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
quick-xml = { version = "0.36", features = ["serialize"] }
base64 = "0.22"
```

- [ ] **Step 2: Create `crates/pob-calc/src/error.rs`**

```rust
#[derive(Debug, thiserror::Error)]
pub enum CalcError {
    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),

    #[error("Data error: {0}")]
    Data(#[from] DataError),
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("XML error: {0}")]
    Xml(String),

    #[error("Missing required attribute '{attr}' on element '{element}'")]
    MissingAttr { element: String, attr: String },

    #[error("Invalid value '{value}' for '{field}': {reason}")]
    InvalidValue { field: String, value: String, reason: String },

    #[error("Base64 decode error: {0}")]
    Base64(String),
}

#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unknown gem: {0}")]
    UnknownGem(String),

    #[error("Unknown passive node: {0}")]
    UnknownNode(u32),
}
```

- [ ] **Step 3: Update `crates/pob-calc/src/lib.rs`**

```rust
pub mod error;
pub mod data;
pub mod mod_db;
pub mod build;
pub mod passive_tree;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver() {
        let v = super::version();
        assert!(v.contains('.'));
    }
}
```

- [ ] **Step 4: Create stub modules so it compiles**

Create each of these files with a single empty `// stub` comment:
- `crates/pob-calc/src/data/mod.rs`
- `crates/pob-calc/src/data/gems.rs`
- `crates/pob-calc/src/data/bases.rs`
- `crates/pob-calc/src/data/misc.rs`
- `crates/pob-calc/src/mod_db/mod.rs`
- `crates/pob-calc/src/mod_db/types.rs`
- `crates/pob-calc/src/build/mod.rs`
- `crates/pob-calc/src/build/types.rs`
- `crates/pob-calc/src/build/xml_parser.rs`
- `crates/pob-calc/src/build/item_parser.rs`
- `crates/pob-calc/src/passive_tree/mod.rs`

```bash
mkdir -p crates/pob-calc/src/{data,mod_db,build,passive_tree}
for f in \
  crates/pob-calc/src/data/mod.rs \
  crates/pob-calc/src/data/gems.rs \
  crates/pob-calc/src/data/bases.rs \
  crates/pob-calc/src/data/misc.rs \
  crates/pob-calc/src/mod_db/mod.rs \
  crates/pob-calc/src/mod_db/types.rs \
  crates/pob-calc/src/build/mod.rs \
  crates/pob-calc/src/build/types.rs \
  crates/pob-calc/src/build/xml_parser.rs \
  crates/pob-calc/src/build/item_parser.rs \
  crates/pob-calc/src/passive_tree/mod.rs; do
  echo "// stub" > "$f"
done
```

- [ ] **Step 5: Build**

```bash
cargo build -p pob-calc
```

Expected: `Finished` with no errors.

- [ ] **Step 6: Commit**

```bash
git add crates/pob-calc/
git commit -m "feat(calc): add error types and module skeleton"
```

---

### Task 2: `ModDb` — types

**Files:**
- Modify: `crates/pob-calc/src/mod_db/types.rs`

**Reference:** `third-party/PathOfBuilding/src/Classes/ModDB.lua` lines 1-60, and the `mod.source` format used throughout CalcSetup.lua (e.g. `"Passive:1234"`, `"Item:BodyArmour"`, `"Skill:Fireball"`).

- [ ] **Step 1: Write the failing test**

Create `crates/pob-calc/src/mod_db/types.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Mirrors POB's mod type enum: BASE, INC, MORE, FLAG, LIST, OVERRIDE, MAX
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModType {
    Base,
    Inc,
    More,
    Flag,
    List,
    Override,
    Max,
}

/// Skill-type flags (bitfield). Mirrors POB's ModFlag.
/// Values match POB's ModFlag constants in Common.lua.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ModFlags(pub u32);

impl ModFlags {
    pub const NONE: Self = ModFlags(0);
    pub const ATTACK: Self = ModFlags(0x1);
    pub const SPELL: Self = ModFlags(0x2);
    pub const HIT: Self = ModFlags(0x4);
    pub const AILMENT: Self = ModFlags(0x8);
    pub const DOT: Self = ModFlags(0x10);
    pub const BOW: Self = ModFlags(0x80);
    pub const MELEE: Self = ModFlags(0x100);

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// Keyword flags (bitfield). Mirrors POB's KeywordFlag constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct KeywordFlags(pub u32);

impl KeywordFlags {
    pub const NONE: Self = KeywordFlags(0);

    pub fn contains(self, other: Self) -> bool {
        other.0 == 0 || (self.0 & other.0) != 0
    }
}

/// The value a modifier carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModValue {
    Number(f64),
    Bool(bool),
    String(String),
}

impl ModValue {
    pub fn as_f64(&self) -> f64 {
        match self {
            Self::Number(n) => *n,
            Self::Bool(b) => if *b { 1.0 } else { 0.0 },
            _ => 0.0,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Number(n) => *n != 0.0,
            _ => false,
        }
    }
}

/// A condition that gates whether a mod applies.
/// Mirrors POB's tag system: { type = "Condition", var = "FullLife" }, etc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// Mod applies only when this condition flag is true in modDB.conditions
    Flag { var: String, negated: bool },
    /// Mod applies only when a multiplier meets a threshold
    MultiplierThreshold { var: String, threshold: f64, negated: bool },
    /// Mod always applies (no condition)
    None,
}

/// Where a mod came from. Used for source attribution in the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModSource {
    /// Category: "Passive", "Item", "Skill", "Base", "Config", "Buff", etc.
    pub category: String,
    /// Human-readable identifier within category: node name, item slot, skill name
    pub name: String,
}

impl ModSource {
    pub fn new(category: impl Into<String>, name: impl Into<String>) -> Self {
        Self { category: category.into(), name: name.into() }
    }
}

/// A single modifier — the atomic unit of the POB calculation system.
/// Mirrors the mod table created by modLib.createMod() in POB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mod {
    /// Stat name, e.g. "Life", "FireResist", "PhysicalDamage"
    pub name: String,
    pub mod_type: ModType,
    pub value: ModValue,
    pub flags: ModFlags,
    pub keyword_flags: KeywordFlags,
    /// All conditions must be satisfied for this mod to apply
    pub conditions: Vec<Condition>,
    pub source: ModSource,
}

impl Mod {
    pub fn new_base(name: impl Into<String>, value: f64, source: ModSource) -> Self {
        Self {
            name: name.into(),
            mod_type: ModType::Base,
            value: ModValue::Number(value),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: Vec::new(),
            source,
        }
    }

    pub fn new_flag(name: impl Into<String>, source: ModSource) -> Self {
        Self {
            name: name.into(),
            mod_type: ModType::Flag,
            value: ModValue::Bool(true),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: Vec::new(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_flags_contains() {
        let flags = ModFlags(ModFlags::ATTACK.0 | ModFlags::HIT.0);
        assert!(flags.contains(ModFlags::ATTACK));
        assert!(flags.contains(ModFlags::HIT));
        assert!(!flags.contains(ModFlags::SPELL));
    }

    #[test]
    fn keyword_flags_none_always_matches() {
        // KeywordFlags::NONE means "no keyword restriction" — always passes
        assert!(KeywordFlags(0xFF).contains(KeywordFlags::NONE));
        assert!(KeywordFlags::NONE.contains(KeywordFlags::NONE));
    }

    #[test]
    fn mod_value_conversions() {
        assert_eq!(ModValue::Number(3.0).as_f64(), 3.0);
        assert_eq!(ModValue::Bool(true).as_f64(), 1.0);
        assert!(ModValue::Bool(true).as_bool());
        assert!(!ModValue::Number(0.0).as_bool());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p pob-calc mod_db::types
```

Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/pob-calc/src/mod_db/types.rs
git commit -m "feat(calc): add Mod, ModType, ModFlags, ModSource types"
```

---

### Task 3: `ModDb` — storage and queries

**Files:**
- Modify: `crates/pob-calc/src/mod_db/mod.rs`

**Reference:** `third-party/PathOfBuilding/src/Classes/ModDB.lua` and `ModStore.lua`.

The query methods implement the same logic as POB's `Sum`, `More`, `Flag`, and `Tabulate`:
- `sum(mod_type, mod_name, flags, keyword_flags) -> f64` — sums all matching BASE/INC values
- `more(mod_name, flags, keyword_flags) -> f64` — multiplies all matching MORE values
- `flag(mod_name, flags, keyword_flags) -> bool` — returns true if any matching FLAG is set
- `tabulate(mod_name, mod_type, flags, keyword_flags) -> Vec<TabulatedMod>` — all matching mods with display info

Conditions are evaluated against `ModDb::conditions` (a `HashMap<String, bool>`) and `ModDb::multipliers` (a `HashMap<String, f64>`).

- [ ] **Step 1: Write failing tests**

```rust
// In crates/pob-calc/src/mod_db/mod.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mod_db::types::{Mod, ModFlags, KeywordFlags, ModSource, ModValue, ModType};

    fn src() -> ModSource { ModSource::new("Test", "test") }

    #[test]
    fn sum_base_mods() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("Life", 100.0, src()));
        db.add(Mod::new_base("Life", 50.0, src()));
        assert_eq!(db.sum(ModType::Base, "Life", ModFlags::NONE, KeywordFlags::NONE), 150.0);
    }

    #[test]
    fn more_multiplies() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::More,
            value: ModValue::Number(20.0), // +20% more = x1.20
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source: src(),
        });
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::More,
            value: ModValue::Number(10.0), // +10% more = x1.10
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source: src(),
        });
        let result = db.more("Life", ModFlags::NONE, KeywordFlags::NONE);
        assert!((result - 1.32).abs() < 0.001, "expected 1.32, got {result}");
    }

    #[test]
    fn flag_returns_true_when_set() {
        let mut db = ModDb::new();
        db.add(Mod::new_flag("ChaosInoculation", src()));
        assert!(db.flag("ChaosInoculation", ModFlags::NONE, KeywordFlags::NONE));
        assert!(!db.flag("FullLife", ModFlags::NONE, KeywordFlags::NONE));
    }

    #[test]
    fn flags_filter_mods() {
        let mut db = ModDb::new();
        // Spell-only mod
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(50.0),
            flags: ModFlags::SPELL,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source: src(),
        });
        // Querying with ATTACK flags should not match
        assert_eq!(db.sum(ModType::Inc, "Damage", ModFlags::ATTACK, KeywordFlags::NONE), 0.0);
        // Querying with SPELL flags should match
        assert_eq!(db.sum(ModType::Inc, "Damage", ModFlags::SPELL, KeywordFlags::NONE), 50.0);
    }

    #[test]
    fn tabulate_returns_matching_mods() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("Life", 100.0, ModSource::new("Passive", "Thick Skin")));
        db.add(Mod::new_base("Life", 40.0,  ModSource::new("Item", "Kaom's Heart")));
        let rows = db.tabulate("Life", None, ModFlags::NONE, KeywordFlags::NONE);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn condition_gates_mod() {
        use crate::mod_db::types::Condition;
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::Base,
            value: ModValue::Number(500.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![Condition::Flag { var: "FullLife".into(), negated: false }],
            source: src(),
        });
        // Condition not set → mod does not contribute
        assert_eq!(db.sum(ModType::Base, "Life", ModFlags::NONE, KeywordFlags::NONE), 0.0);
        // Set the condition
        db.set_condition("FullLife", true);
        assert_eq!(db.sum(ModType::Base, "Life", ModFlags::NONE, KeywordFlags::NONE), 500.0);
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test -p pob-calc mod_db -- 2>&1 | tail -10
```

Expected: compilation error because `ModDb` is not yet defined.

- [ ] **Step 3: Implement `crates/pob-calc/src/mod_db/mod.rs`**

```rust
pub mod types;

use std::collections::HashMap;
use std::sync::Arc;
use types::{Condition, KeywordFlags, Mod, ModFlags, ModSource, ModType, ModValue};

/// Per-mod display row returned by tabulate().
/// Mirrors the rows POB's Tabulate() method returns.
#[derive(Debug, Clone)]
pub struct TabulatedMod {
    pub value: ModValue,
    pub mod_type: ModType,
    pub source_category: String,
    pub source_name: String,
    pub flags: ModFlags,
}

/// The central modifier database.
/// Mirrors POB's ModDB class (ModDB.lua + ModStore.lua).
pub struct ModDb {
    mods: HashMap<String, Vec<Mod>>,
    pub conditions: HashMap<String, bool>,
    pub multipliers: HashMap<String, f64>,
    parent: Option<Arc<ModDb>>,
}

impl ModDb {
    pub fn new() -> Self {
        Self {
            mods: HashMap::new(),
            conditions: HashMap::new(),
            multipliers: HashMap::new(),
            parent: None,
        }
    }

    pub fn with_parent(parent: Arc<ModDb>) -> Self {
        Self {
            mods: HashMap::new(),
            conditions: HashMap::new(),
            multipliers: HashMap::new(),
            parent: Some(parent),
        }
    }

    /// Add a modifier to the database.
    pub fn add(&mut self, m: Mod) {
        self.mods.entry(m.name.clone()).or_default().push(m);
    }

    /// Set a condition flag (mirrors modDB.conditions[var] = true/false in POB).
    pub fn set_condition(&mut self, var: &str, value: bool) {
        self.conditions.insert(var.to_string(), value);
    }

    /// Set a multiplier value (mirrors modDB.multipliers[var] = n in POB).
    pub fn set_multiplier(&mut self, var: &str, value: f64) {
        self.multipliers.insert(var.to_string(), value);
    }

    fn eval_conditions(&self, conditions: &[Condition]) -> bool {
        for cond in conditions {
            match cond {
                Condition::None => {}
                Condition::Flag { var, negated } => {
                    let set = self.conditions.get(var).copied().unwrap_or(false);
                    if *negated && set { return false; }
                    if !*negated && !set { return false; }
                }
                Condition::MultiplierThreshold { var, threshold, negated } => {
                    let val = self.multipliers.get(var).copied().unwrap_or(0.0);
                    let meets = val >= *threshold;
                    if *negated && meets { return false; }
                    if !*negated && !meets { return false; }
                }
            }
        }
        true
    }

    fn mod_matches_query(&self, m: &Mod, mod_type: &ModType, flags: ModFlags, keyword_flags: KeywordFlags) -> bool {
        &m.mod_type == mod_type
            && flags.contains(m.flags)
            && keyword_flags.contains(m.keyword_flags)
            && self.eval_conditions(&m.conditions)
    }

    /// Sum all BASE or INC mods for `name` that pass the flag/keyword/condition filters.
    /// Mirrors POB's modDB:Sum(modType, cfg, statName).
    pub fn sum(&self, mod_type: ModType, name: &str, flags: ModFlags, keyword_flags: KeywordFlags) -> f64 {
        let mut total = 0.0;
        if let Some(list) = self.mods.get(name) {
            for m in list {
                if self.mod_matches_query(m, &mod_type, flags, keyword_flags) {
                    total += m.value.as_f64();
                }
            }
        }
        if let Some(parent) = &self.parent {
            total += parent.sum(mod_type, name, flags, keyword_flags);
        }
        total
    }

    /// Multiply all MORE mods for `name`.
    /// Mirrors POB's modDB:More(cfg, statName).
    /// Each MORE mod value N means ×(1 + N/100).
    pub fn more(&self, name: &str, flags: ModFlags, keyword_flags: KeywordFlags) -> f64 {
        let mut result = 1.0_f64;
        if let Some(list) = self.mods.get(name) {
            for m in list {
                if self.mod_matches_query(m, &ModType::More, flags, keyword_flags) {
                    result *= 1.0 + m.value.as_f64() / 100.0;
                }
            }
        }
        // Round to 2 decimal places per POB's precision rules
        result = (result * 100.0).round() / 100.0;
        if let Some(parent) = &self.parent {
            result *= parent.more(name, flags, keyword_flags);
        }
        result
    }

    /// Return true if any FLAG mod with `name` is set and passes filters.
    /// Mirrors POB's modDB:Flag(cfg, statName).
    pub fn flag(&self, name: &str, flags: ModFlags, keyword_flags: KeywordFlags) -> bool {
        if let Some(list) = self.mods.get(name) {
            for m in list {
                if self.mod_matches_query(m, &ModType::Flag, flags, keyword_flags)
                    && m.value.as_bool()
                {
                    return true;
                }
            }
        }
        if let Some(parent) = &self.parent {
            return parent.flag(name, flags, keyword_flags);
        }
        false
    }

    /// Return all mods matching `name` (and optionally `mod_type`) for source-attribution UI.
    /// Mirrors POB's modDB:Tabulate(modType, cfg, statName).
    pub fn tabulate(
        &self,
        name: &str,
        mod_type: Option<ModType>,
        flags: ModFlags,
        keyword_flags: KeywordFlags,
    ) -> Vec<TabulatedMod> {
        let mut rows = Vec::new();
        if let Some(list) = self.mods.get(name) {
            for m in list {
                let type_matches = mod_type.as_ref().map_or(true, |t| t == &m.mod_type);
                if type_matches && flags.contains(m.flags) && keyword_flags.contains(m.keyword_flags) {
                    rows.push(TabulatedMod {
                        value: m.value.clone(),
                        mod_type: m.mod_type.clone(),
                        source_category: m.source.category.clone(),
                        source_name: m.source.name.clone(),
                        flags: m.flags,
                    });
                }
            }
        }
        if let Some(parent) = &self.parent {
            rows.extend(parent.tabulate(name, mod_type, flags, keyword_flags));
        }
        rows
    }
}

impl Default for ModDb {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{Condition, Mod, ModFlags, KeywordFlags, ModSource, ModValue, ModType};

    fn src() -> ModSource { ModSource::new("Test", "test") }

    #[test]
    fn sum_base_mods() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("Life", 100.0, src()));
        db.add(Mod::new_base("Life", 50.0, src()));
        assert_eq!(db.sum(ModType::Base, "Life", ModFlags::NONE, KeywordFlags::NONE), 150.0);
    }

    #[test]
    fn more_multiplies() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::More,
            value: ModValue::Number(20.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source: src(),
        });
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::More,
            value: ModValue::Number(10.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source: src(),
        });
        let result = db.more("Life", ModFlags::NONE, KeywordFlags::NONE);
        assert!((result - 1.32).abs() < 0.001, "expected 1.32, got {result}");
    }

    #[test]
    fn flag_returns_true_when_set() {
        let mut db = ModDb::new();
        db.add(Mod::new_flag("ChaosInoculation", src()));
        assert!(db.flag("ChaosInoculation", ModFlags::NONE, KeywordFlags::NONE));
        assert!(!db.flag("FullLife", ModFlags::NONE, KeywordFlags::NONE));
    }

    #[test]
    fn flags_filter_mods() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(50.0),
            flags: ModFlags::SPELL,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source: src(),
        });
        assert_eq!(db.sum(ModType::Inc, "Damage", ModFlags::ATTACK, KeywordFlags::NONE), 0.0);
        assert_eq!(db.sum(ModType::Inc, "Damage", ModFlags::SPELL, KeywordFlags::NONE), 50.0);
    }

    #[test]
    fn tabulate_returns_matching_mods() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("Life", 100.0, ModSource::new("Passive", "Thick Skin")));
        db.add(Mod::new_base("Life", 40.0,  ModSource::new("Item", "Kaom's Heart")));
        let rows = db.tabulate("Life", None, ModFlags::NONE, KeywordFlags::NONE);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn condition_gates_mod() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::Base,
            value: ModValue::Number(500.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![Condition::Flag { var: "FullLife".into(), negated: false }],
            source: src(),
        });
        assert_eq!(db.sum(ModType::Base, "Life", ModFlags::NONE, KeywordFlags::NONE), 0.0);
        db.set_condition("FullLife", true);
        assert_eq!(db.sum(ModType::Base, "Life", ModFlags::NONE, KeywordFlags::NONE), 500.0);
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p pob-calc mod_db
```

Expected: 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pob-calc/src/mod_db/
git commit -m "feat(calc): implement ModDb with sum/more/flag/tabulate and condition gating"
```

---

### Task 4: `GameData` loader

**Files:**
- Modify: `crates/pob-calc/src/data/mod.rs`
- Modify: `crates/pob-calc/src/data/gems.rs`
- Modify: `crates/pob-calc/src/data/misc.rs`

- [ ] **Step 1: Write failing test**

In `crates/pob-calc/src/data/mod.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_from_json_stub_parses() {
        // Minimal JSON that satisfies the schema
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
        assert_eq!(data.gems.len(), 0);
    }
}
```

- [ ] **Step 2: Implement `crates/pob-calc/src/data/gems.rs`**

```rust
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct GemData {
    pub id: String,
    pub display_name: String,
    pub is_support: bool,
    pub skill_types: Vec<u32>,
}

pub type GemsMap = HashMap<String, GemData>;
```

- [ ] **Step 3: Implement `crates/pob-calc/src/data/misc.rs`**

```rust
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct MiscData {
    pub game_constants: HashMap<String, f64>,
    pub character_constants: HashMap<String, f64>,
    pub monster_life_table: Vec<i32>,
    pub monster_damage_table: Vec<i32>,
    pub monster_evasion_table: Vec<i32>,
    pub monster_accuracy_table: Vec<i32>,
    pub monster_ally_life_table: Vec<i32>,
    pub monster_ally_damage_table: Vec<i32>,
    pub monster_ailment_threshold_table: Vec<i32>,
    pub monster_phys_conversion_multi_table: Vec<f32>,
}
```

- [ ] **Step 4: Implement `crates/pob-calc/src/data/mod.rs`**

```rust
pub mod gems;
pub mod misc;
pub mod bases;

use std::sync::Arc;
use serde::Deserialize;
use crate::error::DataError;
use gems::GemsMap;
use misc::MiscData;

#[derive(Deserialize)]
struct RawGameData {
    gems: GemsMap,
    misc: MiscData,
}

/// Immutable game data shared across all calculations.
/// Loaded once at startup from the JSON files produced by data-extractor.
#[derive(Debug, Clone)]
pub struct GameData {
    pub gems: GemsMap,
    pub misc: Arc<MiscData>,
}

impl GameData {
    /// Parse a combined JSON string containing all game data sections.
    /// The JSON structure matches what `data-extractor` produces.
    pub fn from_json(json: &str) -> Result<Self, DataError> {
        let raw: RawGameData = serde_json::from_str(json)?;
        Ok(Self {
            gems: raw.gems,
            misc: Arc::new(raw.misc),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_from_json_stub_parses() {
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
        assert_eq!(data.gems.len(), 0);
    }
}
```

Also create stub `crates/pob-calc/src/data/bases.rs`:
```rust
// stub — populated in a later task
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p pob-calc data
```

Expected: `test data::tests::load_from_json_stub_parses ... ok`

- [ ] **Step 6: Commit**

```bash
git add crates/pob-calc/src/data/
git commit -m "feat(calc): implement GameData loader from JSON"
```

---

### Task 5: `Build` types and POB XML parser

**Files:**
- Modify: `crates/pob-calc/src/build/types.rs`
- Modify: `crates/pob-calc/src/build/xml_parser.rs`
- Modify: `crates/pob-calc/src/build/mod.rs`

**Reference:** `third-party/PathOfBuilding/src/Modules/Build.lua` for the build model. The XML format is documented by POB's own code: `Build.lua:Init()` parses the same XML this parser must handle.

A minimal POB XML looks like:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="Juggernaut">
    <PlayerStat stat="Life" value="4521"/>
  </Build>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Cleave" level="20" quality="20" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29"
          nodes="50459,47175"
          classId="1" ascendClassId="1"/>
  </Tree>
  <Items activeItemSet="1">
    <ItemSet id="1">
      <Slot name="Weapon 1" itemId="1"/>
    </ItemSet>
    <Item id="1">
Rarity: UNIQUE
Limbsplit
Woodsplitter
...
    </Item>
  </Items>
  <Config>
    <Input name="enemyLevel" number="84"/>
    <Input name="conditionFullLife" boolean="true"/>
  </Config>
</PathOfBuilding>
```

- [ ] **Step 1: Write failing tests**

Create `crates/pob-calc/src/build/xml_parser.rs` with tests at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="Juggernaut">
  </Build>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Cleave" level="20" quality="20" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="50459,47175" classId="1" ascendClassId="1"/>
  </Tree>
  <Items activeItemSet="1">
    <ItemSet id="1"/>
  </Items>
  <Config>
    <Input name="enemyLevel" number="84"/>
    <Input name="conditionFullLife" boolean="true"/>
  </Config>
</PathOfBuilding>"#;

    #[test]
    fn parses_character_level() {
        let build = parse_xml(MINIMAL_XML).unwrap();
        assert_eq!(build.level, 90);
    }

    #[test]
    fn parses_class_name() {
        let build = parse_xml(MINIMAL_XML).unwrap();
        assert_eq!(build.class_name, "Marauder");
    }

    #[test]
    fn parses_passive_nodes() {
        let build = parse_xml(MINIMAL_XML).unwrap();
        assert!(build.passive_spec.allocated_nodes.contains(&50459));
        assert!(build.passive_spec.allocated_nodes.contains(&47175));
    }

    #[test]
    fn parses_skill_gem() {
        let build = parse_xml(MINIMAL_XML).unwrap();
        assert_eq!(build.skill_sets.len(), 1);
        let skill = &build.skill_sets[0].skills[0];
        assert_eq!(skill.gems[0].skill_id, "Cleave");
        assert_eq!(skill.gems[0].level, 20);
    }

    #[test]
    fn parses_config_flags() {
        let build = parse_xml(MINIMAL_XML).unwrap();
        assert_eq!(build.config.numbers.get("enemyLevel"), Some(&84.0));
        assert_eq!(build.config.booleans.get("conditionFullLife"), Some(&true));
    }

    #[test]
    fn rejects_missing_build_element() {
        assert!(parse_xml("<PathOfBuilding/>").is_err());
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test -p pob-calc build::xml_parser 2>&1 | tail -5
```

Expected: compilation error — `parse_xml` not defined.

- [ ] **Step 3: Implement `crates/pob-calc/src/build/types.rs`**

```rust
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct Build {
    pub class_name: String,
    pub ascend_class_name: String,
    pub level: u8,
    pub bandit: String,
    pub target_version: String,
    pub passive_spec: PassiveSpec,
    pub skill_sets: Vec<SkillSet>,
    pub active_skill_set: usize,   // 0-based index
    pub main_socket_group: usize,  // 0-based index
    pub item_sets: Vec<ItemSet>,
    pub active_item_set: usize,
    pub config: BuildConfig,
}

#[derive(Debug, Clone, Default)]
pub struct PassiveSpec {
    pub tree_version: String,
    pub allocated_nodes: HashSet<u32>,
    pub class_id: u32,
    pub ascend_class_id: u32,
}

#[derive(Debug, Clone)]
pub struct SkillSet {
    pub id: u32,
    pub skills: Vec<Skill>,
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub slot: String,
    pub enabled: bool,
    pub main_active_skill: usize,  // 0-based index into gems
    pub gems: Vec<Gem>,
}

#[derive(Debug, Clone)]
pub struct Gem {
    pub skill_id: String,
    pub level: u8,
    pub quality: u8,
    pub enabled: bool,
    pub is_support: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ItemSet {
    pub id: u32,
    /// Map of slot name → item id
    pub slots: HashMap<String, u32>,
}

#[derive(Debug, Clone, Default)]
pub struct BuildConfig {
    pub numbers: HashMap<String, f64>,
    pub booleans: HashMap<String, bool>,
    pub strings: HashMap<String, String>,
}
```

- [ ] **Step 4: Implement `crates/pob-calc/src/build/xml_parser.rs`**

```rust
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use crate::error::ParseError;
use super::types::*;

pub fn parse_xml(xml: &str) -> Result<Build, ParseError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut build: Option<Build> = None;
    let mut skill_sets: Vec<SkillSet> = Vec::new();
    let mut item_sets: Vec<ItemSet> = Vec::new();
    let mut config = BuildConfig::default();
    let mut passive_spec = PassiveSpec::default();
    let mut active_skill_set: usize = 0;
    let mut active_item_set: usize = 0;
    let mut main_socket_group: usize = 0;

    // Parser state
    let mut current_skill_set: Option<SkillSet> = None;
    let mut current_skill: Option<Skill> = None;
    let mut current_item_set: Option<ItemSet> = None;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = std::str::from_utf8(e.name().as_ref())
                    .map_err(|_| ParseError::Xml("invalid UTF-8 in element name".into()))?;
                let attrs: HashMap<String, String> = e.attributes()
                    .filter_map(|a| a.ok())
                    .filter_map(|a| {
                        let k = std::str::from_utf8(a.key.as_ref()).ok()?.to_string();
                        let v = std::str::from_utf8(a.value.as_ref()).ok()?.to_string();
                        Some((k, v))
                    })
                    .collect();

                match name {
                    "Build" => {
                        let level = attrs.get("level")
                            .and_then(|v| v.parse::<u8>().ok())
                            .unwrap_or(1);
                        let class_name = attrs.get("className").cloned().unwrap_or_default();
                        let ascend_class_name = attrs.get("ascendClassName").cloned().unwrap_or_default();
                        let bandit = attrs.get("bandit").cloned().unwrap_or_else(|| "None".into());
                        let target_version = attrs.get("targetVersion").cloned().unwrap_or_default();
                        main_socket_group = attrs.get("mainSocketGroup")
                            .and_then(|v| v.parse::<usize>().ok())
                            .map(|v| v.saturating_sub(1))
                            .unwrap_or(0);
                        build = Some(Build {
                            class_name, ascend_class_name, level, bandit, target_version,
                            passive_spec: PassiveSpec::default(),
                            skill_sets: Vec::new(),
                            active_skill_set: 0,
                            main_socket_group,
                            item_sets: Vec::new(),
                            active_item_set: 0,
                            config: BuildConfig::default(),
                        });
                    }
                    "Skills" => {
                        active_skill_set = attrs.get("activeSkillSet")
                            .and_then(|v| v.parse::<usize>().ok())
                            .map(|v| v.saturating_sub(1))
                            .unwrap_or(0);
                    }
                    "SkillSet" => {
                        let id = attrs.get("id").and_then(|v| v.parse().ok()).unwrap_or(1);
                        current_skill_set = Some(SkillSet { id, skills: Vec::new() });
                    }
                    "Skill" => {
                        let slot = attrs.get("slot").cloned().unwrap_or_default();
                        let enabled = attrs.get("enabled").map(|v| v == "true").unwrap_or(true);
                        let main_active_skill = attrs.get("mainActiveSkill")
                            .and_then(|v| v.parse::<usize>().ok())
                            .map(|v| v.saturating_sub(1))
                            .unwrap_or(0);
                        current_skill = Some(Skill { slot, enabled, main_active_skill, gems: Vec::new() });
                    }
                    "Gem" => {
                        if let Some(ref mut skill) = current_skill {
                            let skill_id = attrs.get("skillId").cloned().unwrap_or_default();
                            let level = attrs.get("level").and_then(|v| v.parse().ok()).unwrap_or(1);
                            let quality = attrs.get("quality").and_then(|v| v.parse().ok()).unwrap_or(0);
                            let enabled = attrs.get("enabled").map(|v| v == "true").unwrap_or(true);
                            skill.gems.push(Gem { skill_id, level, quality, enabled, is_support: false });
                        }
                    }
                    "Spec" => {
                        let tree_version = attrs.get("treeVersion").cloned().unwrap_or_default();
                        let class_id = attrs.get("classId").and_then(|v| v.parse().ok()).unwrap_or(0);
                        let ascend_class_id = attrs.get("ascendClassId").and_then(|v| v.parse().ok()).unwrap_or(0);
                        let mut allocated_nodes = std::collections::HashSet::new();
                        if let Some(nodes_str) = attrs.get("nodes") {
                            for n in nodes_str.split(',') {
                                if let Ok(id) = n.trim().parse::<u32>() {
                                    allocated_nodes.insert(id);
                                }
                            }
                        }
                        passive_spec = PassiveSpec { tree_version, allocated_nodes, class_id, ascend_class_id };
                    }
                    "Items" => {
                        active_item_set = attrs.get("activeItemSet")
                            .and_then(|v| v.parse::<usize>().ok())
                            .map(|v| v.saturating_sub(1))
                            .unwrap_or(0);
                    }
                    "ItemSet" => {
                        let id = attrs.get("id").and_then(|v| v.parse().ok()).unwrap_or(1);
                        current_item_set = Some(ItemSet { id, slots: HashMap::new() });
                    }
                    "Slot" => {
                        if let Some(ref mut iset) = current_item_set {
                            if let (Some(name), Some(item_id)) = (attrs.get("name"), attrs.get("itemId")) {
                                if let Ok(id) = item_id.parse::<u32>() {
                                    iset.slots.insert(name.clone(), id);
                                }
                            }
                        }
                    }
                    "Input" => {
                        if let Some(name) = attrs.get("name") {
                            let name = name.clone();
                            if let Some(v) = attrs.get("number") {
                                if let Ok(n) = v.parse::<f64>() {
                                    config.numbers.insert(name, n);
                                }
                            } else if let Some(v) = attrs.get("boolean") {
                                config.booleans.insert(name, v == "true");
                            } else if let Some(v) = attrs.get("string") {
                                config.strings.insert(name, v.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name = std::str::from_utf8(e.name().as_ref()).unwrap_or("");
                match name {
                    "Skill" => {
                        if let (Some(ref mut ss), Some(skill)) = (&mut current_skill_set, current_skill.take()) {
                            ss.skills.push(skill);
                        }
                    }
                    "SkillSet" => {
                        if let Some(ss) = current_skill_set.take() {
                            skill_sets.push(ss);
                        }
                    }
                    "ItemSet" => {
                        if let Some(iset) = current_item_set.take() {
                            item_sets.push(iset);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(ParseError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    let mut b = build.ok_or_else(|| ParseError::MissingAttr {
        element: "PathOfBuilding".into(),
        attr: "Build".into(),
    })?;
    b.passive_spec = passive_spec;
    b.skill_sets = skill_sets;
    b.active_skill_set = active_skill_set;
    b.item_sets = item_sets;
    b.active_item_set = active_item_set;
    b.config = config;
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="Juggernaut">
  </Build>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Cleave" level="20" quality="20" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="50459,47175" classId="1" ascendClassId="1"/>
  </Tree>
  <Items activeItemSet="1">
    <ItemSet id="1"/>
  </Items>
  <Config>
    <Input name="enemyLevel" number="84"/>
    <Input name="conditionFullLife" boolean="true"/>
  </Config>
</PathOfBuilding>"#;

    #[test]
    fn parses_character_level() {
        let build = parse_xml(MINIMAL_XML).unwrap();
        assert_eq!(build.level, 90);
    }

    #[test]
    fn parses_class_name() {
        let build = parse_xml(MINIMAL_XML).unwrap();
        assert_eq!(build.class_name, "Marauder");
    }

    #[test]
    fn parses_passive_nodes() {
        let build = parse_xml(MINIMAL_XML).unwrap();
        assert!(build.passive_spec.allocated_nodes.contains(&50459));
        assert!(build.passive_spec.allocated_nodes.contains(&47175));
    }

    #[test]
    fn parses_skill_gem() {
        let build = parse_xml(MINIMAL_XML).unwrap();
        assert_eq!(build.skill_sets.len(), 1);
        let skill = &build.skill_sets[0].skills[0];
        assert_eq!(skill.gems[0].skill_id, "Cleave");
        assert_eq!(skill.gems[0].level, 20);
    }

    #[test]
    fn parses_config_flags() {
        let build = parse_xml(MINIMAL_XML).unwrap();
        assert_eq!(build.config.numbers.get("enemyLevel"), Some(&84.0));
        assert_eq!(build.config.booleans.get("conditionFullLife"), Some(&true));
    }

    #[test]
    fn rejects_missing_build_element() {
        assert!(parse_xml("<PathOfBuilding/>").is_err());
    }
}
```

- [ ] **Step 5: Update `crates/pob-calc/src/build/mod.rs`**

```rust
pub mod types;
pub mod xml_parser;
pub mod item_parser;

pub use types::Build;
pub use xml_parser::parse_xml;
```

Create stub `crates/pob-calc/src/build/item_parser.rs`:
```rust
// stub — item text parsing implemented in Phase 4
```

- [ ] **Step 6: Run tests**

```bash
cargo test -p pob-calc build
```

Expected: 6 tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/pob-calc/src/build/
git commit -m "feat(calc): implement Build types and POB XML parser"
```

---

### Task 6: `PassiveTree` loader

**Files:**
- Modify: `crates/pob-calc/src/passive_tree/mod.rs`

The passive tree JSON (from `data/tree/poe1_current.json`) has a `"nodes"` key mapping node ID strings to node objects. We need to load only the fields the calculation engine uses: node ID, name, stat descriptions, and the set of linked node IDs.

- [ ] **Step 1: Write failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_TREE_JSON: &str = r#"{
        "nodes": {
            "50459": {
                "id": 50459,
                "name": "Thick Skin",
                "stats": ["+10 to maximum Life"],
                "out": [47175]
            },
            "47175": {
                "id": 47175,
                "name": "Quick Recovery",
                "stats": [],
                "out": []
            }
        }
    }"#;

    #[test]
    fn loads_nodes_from_json() {
        let tree = PassiveTree::from_json(MINIMAL_TREE_JSON).unwrap();
        assert_eq!(tree.nodes.len(), 2);
        let node = tree.nodes.get(&50459).unwrap();
        assert_eq!(node.name, "Thick Skin");
        assert!(node.linked_ids.contains(&47175));
    }
}
```

- [ ] **Step 2: Implement `crates/pob-calc/src/passive_tree/mod.rs`**

```rust
use std::collections::HashMap;
use serde::Deserialize;
use crate::error::DataError;

#[derive(Debug, Clone, Deserialize)]
struct RawNode {
    id: u32,
    name: String,
    #[serde(default)]
    stats: Vec<String>,
    #[serde(rename = "out", default)]
    out_ids: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct PassiveNode {
    pub id: u32,
    pub name: String,
    /// Human-readable stat descriptions, e.g. ["+10 to maximum Life"]
    pub stats: Vec<String>,
    /// IDs of nodes this one connects to
    pub linked_ids: Vec<u32>,
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
        let nodes = root.nodes.into_values().map(|raw| {
            let node = PassiveNode {
                id: raw.id,
                name: raw.name,
                stats: raw.stats,
                linked_ids: raw.out_ids,
            };
            (raw.id, node)
        }).collect();
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
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p pob-calc passive_tree
```

Expected: `test passive_tree::tests::loads_nodes_from_json ... ok`

- [ ] **Step 4: Commit**

```bash
git add crates/pob-calc/src/passive_tree/
git commit -m "feat(calc): implement PassiveTree JSON loader"
```

---

**Phase 3 complete.** `pob-calc` now has a working `ModDb`, `GameData` loader, `Build` XML parser, and `PassiveTree` loader. All tested with `cargo test -p pob-calc`. Proceed to Phase 4 (calculation engine).
