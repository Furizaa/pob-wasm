# pob-wasm Design Spec

**Date:** 2026-03-29  
**Status:** Approved  

## Summary

A Rust-based port of PathOfBuilding's build calculation engine, compiled to WebAssembly. The WASM module accepts a POB-format XML build string and returns the full calculated output table, per-stat formula breakdowns, and per-mod source attribution as JSON. No UI. Both PoE 1 and PoE 2 supported. All game data extracted from the official `Content.ggpk` file via a native CLI tool.

---

## 1. Repository & Workspace Structure

```
pob-wasm/                          ← repo root
├── third-party/PathOfBuilding/    ← git submodule (reference only, read-only)
├── crates/
│   ├── pob-calc/                  ← core engine library (zero WASM deps)
│   │   └── src/
│   │       ├── mod_db.rs          ← modifier database (ModDB/ModList/ModStore)
│   │       ├── passive_tree.rs    ← passive tree allocation & pathfinding
│   │       ├── skills.rs          ← granted effects, skill data, support links
│   │       ├── items.rs           ← item parsing, stat extraction
│   │       ├── build.rs           ← Build struct + POB XML parser
│   │       ├── calc/
│   │       │   ├── mod.rs
│   │       │   ├── setup.rs       ← CalcSetup.lua port
│   │       │   ├── perform.rs     ← CalcPerform.lua port (main pass)
│   │       │   ├── offence.rs     ← CalcOffence.lua port
│   │       │   ├── defence.rs     ← CalcDefence.lua port
│   │       │   ├── active_skill.rs ← CalcActiveSkill.lua port
│   │       │   ├── triggers.rs    ← CalcTriggers.lua port
│   │       │   └── mirages.rs     ← CalcMirages.lua port
│   │       └── data/              ← game data loading + types
│   ├── pob-wasm/                  ← wasm-bindgen wrapper (~200 lines)
│   │   └── src/lib.rs
│   └── data-extractor/            ← native CLI binary (not compiled to WASM)
│       └── src/main.rs
├── data/                          ← extracted game data (committed to repo)
│   ├── gems.json
│   ├── skills/
│   ├── bases.json
│   ├── mods.json
│   ├── minions.json
│   ├── pantheons.json
│   ├── stat_descriptions/
│   └── tree/
│       ├── poe1_3_28.json
│       ├── poe1_3_28_ruthless.json
│       └── poe2_latest.json       ← and other supported versions
├── scripts/
│   └── extract.sh                 ← orchestrates data-extractor over a GGPK file
└── tests/
    └── oracle/
        ├── melee_str.xml
        ├── melee_str.expected.json
        ├── crit_spellcaster.xml
        ├── crit_spellcaster.expected.json
        └── ...
```

**Key constraint:** `pob-calc` has no dependency on `wasm-bindgen` or any WASM-specific crate. It compiles to native binaries for testing and to WASM via `pob-wasm`.

---

## 2. Data Extraction Pipeline

All game data originates from the `Content.ggpk` file. Nothing is hand-maintained or copied from the POB submodule's data files.

### Tools

The `data-extractor` crate is a native CLI binary that:
1. Reads `Content.ggpk` using an existing Rust GGPK library (preference: search crates.io for `ggpk`, `poe-dat`, or similar). If no suitable crate exists, a minimal GGPK reader is implemented in the `data-extractor` crate itself.
2. Extracts `.dat64` files from the GGPK's `Data/` directory.
3. Parses `.dat64` files using field schemas derived from POB's `src/Export/spec.lua` and `src/Export/Scripts/*.lua`.
4. Outputs normalized JSON files to `data/`.

### Pipeline

```
Content.ggpk (provided by user, not committed)
    ↓  data-extractor CLI
.dat64 raw files  (temp directory, not committed)
    ↓  field transformers (Rust, based on POB export scripts)
data/*.json  (committed to repo, human-readable)
    ↓  compile time / runtime load in pob-calc
embedded game data used by the calc engine
```

### Key .dat tables needed

Derived from `src/Export/Scripts/`:

| Output file | Source .dat tables |
|---|---|
| `gems.json` | `ActiveSkills.dat`, `GrantedEffects.dat`, `GrantedEffectsPerLevel.dat`, `ItemClasses.dat` |
| `skills/` | `SkillGems.dat`, `ActiveSkillType.dat`, `StatInterpolationTypes.dat` |
| `bases.json` | `BaseItemTypes.dat`, `WeaponTypes.dat`, `ArmourTypes.dat`, `Flasks.dat` |
| `mods.json` | `Mods.dat`, `Stats.dat`, `ModType.dat` |
| `minions.json` | `MonsterVarieties.dat` |
| `pantheons.json` | `PantheonPanelLayout.dat` |
| `stat_descriptions/` | `Metadata/StatDescriptions/*.txt` (text files, not .dat) |
| `tree/poe1_<ver>.json` | `PassiveSkills.dat`, `PassiveSkillTrees.dat` |
| `tree/poe2_<ver>.json` | same tables, different GGPK |
| `cluster_jewels.json` | `PassiveSkills.dat` (cluster jewel subset) |
| `enchants.json` | `HelmEnchantments.dat`, etc. |

### Update workflow

When a new patch ships:
1. User runs `scripts/extract.sh /path/to/new/Content.ggpk`
2. `data-extractor` regenerates all JSON files
3. Diff is reviewed and committed

---

## 3. Core Engine Architecture (`pob-calc`)

### Modifier database

The central pattern in POB is the modifier database. Every stat effect in the game is a `Mod`:

```rust
pub struct Mod {
    pub name: ModName,          // stat identifier, e.g. "Life", "FireResist"
    pub type_: ModType,         // BASE | INC | MORE | FLAG | LIST | OVERRIDE
    pub value: ModValue,        // f64, bool, or enum
    pub flags: ModFlags,        // Attack | Spell | Bow | etc. (bitfield)
    pub keyword_flags: KeywordFlags,
    pub conditions: Vec<Condition>,  // gating conditions
    pub source: ModSource,      // "Passive:NodeId", "Item:SlotName", etc.
}

pub struct ModDb {
    mods: HashMap<ModName, Vec<Mod>>,
    parent: Option<Arc<ModDb>>,  // for inheritance (player → minion)
}
```

`ModDb` implements `Sum`, `More`, `Flag`, `List` queries, mirroring POB's `modDB:Sum()`, `modDB:Flag()`, `modDB:List()`.

### Build input model

```rust
pub struct Build {
    pub game_version: GameVersion,   // Poe1 | Poe2
    pub class: Class,
    pub ascendancy: Option<Ascendancy>,
    pub level: u8,
    pub bandit: Bandit,
    pub passive_spec: PassiveSpec,
    pub skill_sets: Vec<SkillSet>,
    pub item_sets: Vec<ItemSet>,
    pub active_item_set: usize,
    pub active_skill_set: usize,
    pub main_skill_index: usize,
    pub config: BuildConfig,         // mirrors ConfigTab options
}

pub struct PassiveSpec {
    pub tree_version: String,
    pub allocated_nodes: HashSet<NodeId>,
    pub mastery_effects: HashMap<NodeId, u32>,
    pub jewel_sockets: HashMap<NodeId, Item>,
}
```

### Calculation environment

Mirrors POB's `env` table constructed in `CalcSetup.lua`. The `Actor` type carries both the output values and the breakdown data, since both are populated during the same calculation pass — the breakdown is never a separate step.

```rust
pub struct CalcEnv<'data> {
    pub player: Actor,
    pub enemy: Actor,
    pub mode: CalcMode,
    pub aux_skills: Vec<ActiveSkill>,
    pub data: &'data GameData,
}

pub struct Actor {
    pub mod_db: ModDb,
    pub output: OutputTable,
    pub breakdown: BreakdownTable,   // always populated alongside output
    pub main_skill: Option<ActiveSkill>,
    pub minion: Option<Box<Actor>>,
}

pub type OutputTable = HashMap<String, OutputValue>;

// Mirrors actor.breakdown in POB — keyed by stat name, same as OutputTable
pub type BreakdownTable = HashMap<String, BreakdownData>;

pub struct BreakdownData {
    // Formula step strings, e.g. ["4000 (base)", "x 1.85 (increased/reduced)", "= 7400"]
    pub lines: Vec<String>,
    // Per-item-slot contribution rows (used for Armour, Evasion, ES, etc.)
    pub slots: Vec<SlotRow>,
    // Per-damage-type conversion rows (used for hit damage breakdowns)
    pub damage_types: Vec<DamageTypeRow>,
    // Mana/life reservation rows
    pub reservations: Vec<ReservationRow>,
}

pub struct SlotRow {
    pub base: f64,
    pub inc: Option<String>,      // e.g. "x 1.40"
    pub more: Option<String>,     // e.g. "x 1.00"
    pub total: String,
    pub source: String,           // e.g. "Item:BodyArmour"
    pub source_name: String,      // e.g. "Kaom's Heart"
}

pub struct DamageTypeRow {
    pub source: String,           // damage type before conversion, e.g. "Physical"
    pub base: String,             // "100-200"
    pub inc: String,              // "x 1.85"
    pub more: String,             // "x 1.20"
    pub total: String,            // damage total before conversion
    pub conv_dst: String,         // destination type after conversion (empty if none)
    pub gain_dst: String,         // gain-as destination (empty if none)
}

pub struct ReservationRow {
    pub skill_name: String,
    pub base: String,
    pub mult: String,
    pub more: String,
    pub inc: String,
    pub efficiency: String,
    pub efficiency_more: String,
    pub total: String,
}
```

**Implementation note — breakdown always on:** In the POB Lua source, breakdown population is guarded by `if breakdown then ... end` blocks throughout `CalcPerform.lua`, `CalcOffence.lua`, etc. (breakdown is `nil` in headless/non-UI mode). In the Rust port, `BreakdownTable` is always present and always populated — there is no conditional. This avoids a mode-switch and ensures the WASM API always returns complete data.

**Implementation note — handle lifetime:** `CalcEnv` references `GameData` via a Rust lifetime (`'data`), but the WASM handle map needs to own its entries across arbitrary JS call boundaries. In practice, the stored environment will hold a reference-counted pointer (`Arc<GameData>`) rather than a borrow, so `CalcEnv` will own all its data and the handle map can be `HashMap<u32, CalcEnv>` with no lifetime parameter.

### Calculation pipeline

```
Build
  → init_env()              [calc/setup.rs]  populate ModDb from passives + items + skills
  → do_actor_life_mana()    [calc/perform.rs]  → output.Life, breakdown.Life, ...
  → calc_defence()          [calc/defence.rs]  → output.Armour, breakdown.Armour, ...
  → build_active_skill()    [calc/active_skill.rs]
  → calc_offence()          [calc/offence.rs]  → output.TotalDPS, breakdown.Physical, ...
  → calc_triggers()         [calc/triggers.rs]
  → calc_mirages()          [calc/mirages.rs]
  → CalcResult { handle, output, breakdown }
```

Each module maps 1:1 to its Lua counterpart. The POB source is the authoritative reference for every formula, constant, and edge case.

### Game data loading

`GameData` is loaded once and shared immutably across calculations:

```rust
pub struct GameData {
    pub gems: HashMap<GemId, GemData>,
    pub passive_trees: HashMap<TreeVersion, PassiveTree>,
    pub bases: Vec<BaseItem>,
    pub mods: Vec<ModEntry>,
    pub skills: HashMap<SkillId, SkillData>,
    // ...
}
```

In the WASM context, `GameData` is populated from JSON passed to `init()`. In native tests, it loads from the `data/` directory.

---

## 4. WASM API

### Design rationale

POB's Calcs tab exposes three distinct data layers that a feature-equivalent UI requires:

1. **Output values** — the final computed stats (`Life = 8880`, `TotalDPS = 1234567`).
2. **Formula breakdowns** — the step-by-step multiplication chain that produced each value (`4000 base × 1.85 inc × 1.20 more = 8880`), plus structured tables for per-slot armour contributions, per-type damage conversion chains, and reservation breakdowns. This data lives in `actor.breakdown` in POB and is populated during the same pass as the output values.
3. **Mod source attribution** — for any stat, the list of every individual modifier contributing to it, with value, type (BASE/INC/MORE/FLAG), source category (Passive/Item/Skill/etc.), and source name (the node or item name). This comes from `ModDB:Tabulate()` in POB.

The original API design returned only layer 1. All three layers are needed for a POB-equivalent UI.

### Stateful build handles

`getMods()` queries the `ModDb` built during `calculate()`. Re-parsing and re-running setup on every `getMods()` call would be wasteful for a UI that tabulates many stats in response to user interaction. Instead, `calculate()` returns an opaque `handle` (a `u32` key into a WASM-side `HashMap<u32, CalcEnv>`) that keeps the environment alive. The consumer calls `releaseBuild(handle)` when done to free the memory.

### Functions

The `pob-wasm` crate exposes five functions via `wasm-bindgen`:

```typescript
/**
 * Initialize the engine with game data JSON.
 * Must be called once before any calculate() calls.
 * gameDataJson: the combined contents of data/*.json
 */
export function init(gameDataJson: string): void;

/**
 * Run all calculations for a build.
 * pobXml: a PathOfBuilding XML build string
 * Returns: JSON string of CalculationResult (see below)
 * Throws: on XML parse errors or fatal engine errors
 */
export function calculate(pobXml: string): string;

/**
 * Run calculations scoped to a specific skill index.
 * skillIndex: 0-based index into the build's active skill set
 * Returns: JSON string of CalculationResult
 */
export function calculateSkill(pobXml: string, skillIndex: number): string;

/**
 * Query the individual modifiers contributing to a stat.
 * Must be called with a handle returned by calculate() or calculateSkill().
 * modName:  stat name, e.g. "Life", "FireResist", "PhysicalDamage"
 * modType:  optional filter — "BASE" | "INC" | "MORE" | "FLAG" | "LIST"
 *           omit to return all types
 * cfg:      optional skill config scope — "skill" | "weapon1" | "weapon2"
 *           omit for the global player mod db
 * Returns:  JSON string of ModEntry[]
 * Throws:   if handle is invalid or already released
 */
export function getMods(
  handle: number,
  modName: string,
  modType?: string,
  cfg?: string
): string;

/**
 * Release a cached build environment and free its memory.
 * Must be called when the consumer is done with a handle.
 */
export function releaseBuild(handle: number): void;
```

### POB XML parsing

A `build::parse_xml(xml: &str) -> Result<Build, ParseError>` function in `pob-calc/src/build.rs` handles the POB XML format. Key sections parsed:

- `<Build level attrib class ascendClassName mainSkillIndex bandit>` — character basics
- `<Skills>` — each `<SkillSet>` containing `<Skill>` with `<Gem>` children
- `<Items>` — item text blocks in POB's extended item format
- `<ItemSet>` — slot assignments
- `<Tree>` — one or more `<Spec treeVersion nodes>` (base64 node bitmap)
- `<Config>` — key/value configuration flags

### `CalculationResult` JSON schema

```typescript
interface CalculationResult {
  // Opaque handle for getMods() / releaseBuild().
  // The consumer is responsible for calling releaseBuild() when done.
  handle: number;

  // Final computed stat values. All keys match POB's env.player.output names
  // exactly, ensuring compatibility with consumers familiar with POB.
  output: Record<string, number | boolean | string>;

  // Per-stat formula breakdowns. Keys are a subset of output keys —
  // only stats with non-trivial breakdowns (more than just a base value)
  // have an entry here.
  breakdown: Record<string, BreakdownData>;
}

interface BreakdownData {
  // Ordered formula step strings.
  // e.g. ["4000 (base)", "x 1.85 (increased/reduced)", "x 1.20 (more/less)", "= 8880"]
  // Mirrors the text lines in POB's CalcBreakdown control.
  lines?: string[];

  // Per-equipment-slot contribution rows.
  // Present for stats like Armour, Evasion, EnergyShield.
  // Mirrors breakdown.slots in POB.
  slots?: SlotRow[];

  // Per-damage-type conversion chain rows.
  // Present for hit damage stats (Physical, Lightning, Cold, Fire, Chaos).
  // Mirrors breakdown.damageTypes in POB.
  damageTypes?: DamageTypeRow[];

  // Mana/life reservation breakdown rows.
  // Present for ManaReserved / LifeReserved stats.
  // Mirrors breakdown.reservations in POB.
  reservations?: ReservationRow[];
}

interface SlotRow {
  base: number;
  inc: string | null;       // formatted multiplier string, e.g. "x 1.40", or null
  more: string | null;
  total: string;
  source: string;           // slot identifier, e.g. "Item:BodyArmour"
  sourceName: string;       // item name, e.g. "Kaom's Heart"
}

interface DamageTypeRow {
  source: string;           // damage type origin, e.g. "Physical"
  base: string;             // range string, e.g. "100-200"
  inc: string;              // e.g. "x 1.85"
  more: string;             // e.g. "x 1.20"
  total: string;            // after inc/more, before conversion
  convDst: string;          // conversion destination type, or ""
  gainDst: string;          // gain-as destination type, or ""
}

interface ReservationRow {
  skillName: string;
  base: string;
  mult: string;
  more: string;
  inc: string;
  efficiency: string;
  efficiencyMore: string;
  total: string;
}
```

Example `CalculationResult`:

```json
{
  "handle": 1,
  "output": {
    "Life": 8880.0,
    "EnergyShield": 0.0,
    "Mana": 912.0,
    "FireResist": 75.0,
    "Armour": 16800.0,
    "TotalDPS": 1234567.8,
    "CritChance": 45.21
  },
  "breakdown": {
    "Life": {
      "lines": ["4000 (base)", "x 1.85 (increased/reduced)", "x 1.20 (more/less)", "= 8880"]
    },
    "Armour": {
      "lines": ["12000 (base)", "x 1.40 (increased/reduced)", "= 16800"],
      "slots": [
        { "base": 8000, "inc": "x 1.40", "more": null, "total": "11200", "source": "Item:BodyArmour", "sourceName": "Kaom's Heart" },
        { "base": 4000, "inc": "x 1.40", "more": null, "total": "5600",  "source": "Item:Helmet",    "sourceName": "Rare Helmet" }
      ]
    },
    "Physical": {
      "lines": ["Average hit: 45678"],
      "damageTypes": [
        { "source": "Physical", "base": "100-200", "inc": "x 1.85", "more": "x 1.20", "total": "222-444", "convDst": "", "gainDst": "" }
      ]
    },
    "ManaReserved": {
      "reservations": [
        { "skillName": "Hatred", "base": "40%", "mult": "x 1.00", "more": "x 1.00", "inc": "x 1.00", "efficiency": "x 1.00", "efficiencyMore": "x 1.00", "total": "40%" }
      ]
    }
  }
}
```

### `getMods()` return schema

```typescript
interface ModEntry {
  value: number | boolean;
  type: "BASE" | "INC" | "MORE" | "FLAG" | "LIST" | "OVERRIDE";
  source: string;       // source category: "Passive" | "Item" | "Skill" | "Base" | ...
  sourceName: string;   // human-readable name: passive node name, item name, skill name
  flags: string;        // formatted skill-type flags, e.g. "Attack, Melee" or ""
  tags: string;         // formatted condition/tag string, e.g. "when Crit" or ""
}
```

Example `getMods(handle, "Life", "BASE")`:

```json
[
  { "value": 38,  "type": "BASE", "source": "Base",    "sourceName": "Marauder base life",    "flags": "", "tags": "" },
  { "value": 40,  "type": "BASE", "source": "Item",    "sourceName": "Kaom's Heart",          "flags": "", "tags": "" },
  { "value": 15,  "type": "BASE", "source": "Passive", "sourceName": "Bloodless",             "flags": "", "tags": "" },
  { "value": 10,  "type": "BASE", "source": "Passive", "sourceName": "Warrior's Blood",       "flags": "", "tags": "" }
]
```

### Data bundling strategy

Game data is **not** embedded in the `.wasm` binary. Instead, the consumer fetches a `game-data.json` file (or equivalent split files) and passes it to `init()`. This keeps the `.wasm` binary small and allows data updates without recompilation.

---

## 5. Error Handling

- All internal errors use a `CalcError` enum (`ParseError`, `DataError`, `CalcError`)
- `DataError` (unknown gem, missing stat) is non-fatal: falls back to sensible defaults matching POB's behaviour
- Fatal errors surface as JavaScript exceptions at the WASM boundary
- No panics in production paths; debug builds may panic on invariant violations

---

## 6. Testing Strategy

### Layer 1 — Unit tests (native, fast)

In `pob-calc` using `#[test]`. Tests construct `ModDb` directly with known mods and assert on specific output values. Run via `cargo test`.

### Layer 2 — Oracle tests (correctness against POB)

Stored in `crates/pob-calc/tests/oracle/`. Each oracle test is a pair:
- `{name}.xml` — a POB build XML
- `{name}.expected.json` — the output table captured from POB's HeadlessWrapper

**Oracle generation:** `scripts/gen_oracle.lua` runs POB's HeadlessWrapper, loads a build XML, enables breakdown (`actor.breakdown = {}`), calls `calcs.perform(env)`, then serializes `env.player.output` and `env.player.breakdown` to JSON. Use `scripts/run_oracle.sh` (the shell wrapper) rather than invoking `luajit` directly — POB uses relative `dofile()` paths throughout, so the script must run from `third-party/PathOfBuilding/src/`, which `run_oracle.sh` handles automatically:

```bash
./scripts/run_oracle.sh tests/oracle/{name}.xml > crates/pob-calc/tests/oracle/{name}.expected.json
```

Oracle files are generated once, committed, and only regenerated when POB's calculation logic changes.

**Environment requirements for oracle generation:**

POB's `HeadlessWrapper.lua` was designed for a Windows runtime that bundles native Lua C extensions. On macOS/Linux with plain LuaJIT several issues arise:

| Issue | Root cause | Fix applied in `gen_oracle.lua` |
|---|---|---|
| `GetVirtualScreenSize` is nil | HeadlessWrapper defines `GetScreenSize` but not `GetVirtualScreenSize`; `Launch.lua:394` calls it during `OnFrame` when a prompt/error is displayed | Pre-define `GetVirtualScreenSize` returning `1920, 1080` before loading HeadlessWrapper |
| `lua-utf8` module not found | A C extension bundled in POB's Windows runtime; used only for number thousands-separator formatting | Stub as a pure-Lua fallback in `package.preload` before loading HeadlessWrapper |
| `sha1` module path | Stored as `runtime/lua/sha1/init.lua`; requires `?/init.lua` in `package.path` | Add `runtime/lua/?/init.lua` pattern to `package.path` |
| Relative `dofile` calls | POB calls `dofile("Launch.lua")`, `LoadModule("Modules/Calcs.lua")` etc. relative to cwd | Must run from `third-party/PathOfBuilding/src/`; handled by `run_oracle.sh` |

For CI, the POB Docker container (`ghcr.io/pathofbuildingcommunity/pathofbuilding-tests:latest`) bundles all required native extensions and is the recommended environment for generating new oracle files. On a developer machine with macOS, the stubs in `gen_oracle.lua` are sufficient for builds that don't exercise `lua-utf8`'s advanced Unicode paths (which the standard PoE game data doesn't).

**Oracle builds (representative archetypes):**

| Name | Build type |
|---|---|
| `melee_str` | STR melee, basic two-hander |
| `crit_spellcaster` | INT crit spell, no minions |
| `minion_summoner` | Raise Spectre / Raise Zombie |
| `ignite_dot` | Ignite proliferation DoT |
| `bleed_dot` | Bleed-based melee |
| `poison_dot` | Chaos/poison stacker |
| `trap_saboteur` | Trap-throwing build |
| `totem_hierophant` | Spell totem |
| `mine_detonator` | Mine-based damage |
| `poe2_basic` | PoE 2 class, basic build |

Oracle Rust tests (`crates/pob-calc/tests/oracle.rs`) are unconditional — they always run. Tests that compare computed values against expected JSON only assert when `DATA_DIR` env var is set (pointing to a directory containing game data JSON); they skip gracefully otherwise. This means `cargo test` always works without any special setup, and full parity validation is available when game data is present.

### Layer 3 — WASM integration tests

`wasm-pack test --headless --chrome` with a small subset of oracle builds. Verifies the JS API, JSON serialization, and WASM module initialization.

---

## 7. Out of Scope

- Any UI rendering
- Item filtering / trade site integration
- Build storage / persistence
- Network requests from WASM
- The POB update mechanism
- Non-calculation features (notes, build lists, etc.)

---

## 8. Open Questions / Future Work

- PoE 2 support requires understanding differences in the game's data schema; this will be addressed once PoE 1 support is complete and validated.
- Cluster jewel sub-tree generation (dynamic tree expansion) is complex and may be deferred to a later iteration.
- Timeless jewel transformation logic is similarly complex and may be deferred.
- The exact GGPK/dat library choice will be determined during implementation.
