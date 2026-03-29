# pob-wasm Design Spec

**Date:** 2026-03-29  
**Status:** Approved  

## Summary

A Rust-based port of PathOfBuilding's build calculation engine, compiled to WebAssembly. The WASM module accepts a POB-format XML build string and returns the full calculated output table as JSON. No UI. Both PoE 1 and PoE 2 supported. All game data extracted from the official `Content.ggpk` file via a native CLI tool.

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

Mirrors POB's `env` table constructed in `CalcSetup.lua`:

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
    pub main_skill: Option<ActiveSkill>,
    pub minion: Option<Box<Actor>>,
}

pub type OutputTable = HashMap<String, OutputValue>;
```

### Calculation pipeline

```
Build
  → init_env()              [calc/setup.rs]  populate ModDb from passives + items + skills
  → do_actor_life_mana()    [calc/perform.rs]
  → calc_defence()          [calc/defence.rs]
  → build_active_skill()    [calc/active_skill.rs]
  → calc_offence()          [calc/offence.rs]
  → calc_triggers()         [calc/triggers.rs]
  → calc_mirages()          [calc/mirages.rs]
  → OutputTable (JSON-serialized)
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

The `pob-wasm` crate exposes three functions via `wasm-bindgen`:

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
 * Returns: JSON string of the full output table
 * Throws: on XML parse errors or fatal engine errors
 */
export function calculate(pobXml: string): string;

/**
 * Run calculations for a specific skill index.
 * skillIndex: 0-based index into the build's skill list
 */
export function calculateSkill(pobXml: string, skillIndex: number): string;
```

### POB XML parsing

A `build::parse_xml(xml: &str) -> Result<Build, ParseError>` function in `pob-calc/src/build.rs` handles the POB XML format. Key sections parsed:

- `<Build level attrib class ascendClassName mainSkillIndex bandit>` — character basics
- `<Skills>` — each `<SkillSet>` containing `<Skill>` with `<Gem>` children
- `<Items>` — item text blocks in POB's extended item format
- `<ItemSet>` — slot assignments
- `<Tree>` — one or more `<Spec treeVersion nodes>` (base64 node bitmap)
- `<Config>` — key/value configuration flags

### Output format

`calculate()` returns a flat JSON object. All keys match POB's `env.player.output` table names exactly, ensuring drop-in compatibility for consumers familiar with POB:

```json
{
  "Life": 4521.0,
  "EnergyShield": 0.0,
  "Mana": 912.0,
  "FireResist": 75.0,
  "ColdResist": 75.0,
  "LightningResist": 75.0,
  "ChaosResist": -60.0,
  "Armour": 12450.0,
  "Evasion": 0.0,
  "TotalDPS": 1234567.8,
  "AverageDamage": 45678.9,
  "CritChance": 45.21,
  "CritMultiplier": 350.0,
  "Speed": 4.5,
  ...
}
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

Stored in `tests/oracle/`. Each oracle test is a pair:
- `{name}.xml` — a POB build XML
- `{name}.expected.json` — the output table captured from POB's HeadlessWrapper

**Oracle generation:** A small Lua wrapper script (`scripts/gen_oracle.lua`) runs POB's HeadlessWrapper, loads a build XML, runs `calcs.perform(env)`, and prints `env.player.output` serialized to JSON via `dkjson` or an equivalent pure-Lua JSON library. Invoked as `luajit scripts/gen_oracle.lua {name}.xml > tests/oracle/{name}.expected.json`. This requires LuaJIT and the POB submodule to be initialized. Oracle files are generated once, committed, and only regenerated when POB's calculation logic changes.

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

Oracle tests run in CI but require LuaJIT; they are skipped if LuaJIT is not installed.

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
