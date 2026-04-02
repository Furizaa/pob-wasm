# SETUP-13: Buff Mode Conditions

## Output Fields

This chunk writes **no output fields directly**. Its effect is entirely in the
`CalcEnv` struct and the `ModDb`: it sets three boolean flags on `env`
(`mode_buffs`, `mode_combat`, `mode_effective`) and mirrors them as
`modDB.conditions["Buffed"]`, `modDB.conditions["Combat"]`,
`modDB.conditions["Effective"]` so that downstream condition-gated mods
(parsed as `{ type = "Condition", var = "Buffed/Combat/Effective" }`) evaluate
correctly.

Oracle builds all use the default `"EFFECTIVE"` mode (all three flags `true`),
so correctness for oracle is: the three conditions are unconditionally `true`
in the player's `ModDb`.

## Dependencies

- SETUP-01 through SETUP-12 (or at minimum SETUP-01, so the player `ModDb`
  exists when `initModDB` is called — the buff-mode flags are consumed at line
  108–110 of `initModDB`).

## Lua Source

**File:** `third-party/PathOfBuilding/src/Modules/CalcSetup.lua`  
**Lines:** 444–467 (buff-mode dispatch)  
**Secondary lines:** 108–110 (injection into `modDB.conditions` inside
`calcs.initModDB`, which is called just after the dispatch)  
**Commit:** `454eff8c85d24356d9b051d596983745ed367476`

## Annotated Lua

### CalcSetup.lua lines 444–467 — buff mode dispatch

```lua
-- This code runs inside calcs.initEnv() after env and modDB are created
-- (line 358+). The local variable `mode` is the string argument passed to
-- calcs.initEnv(): "MAIN", "CALCULATOR", "CALCS", or "MINION".
--
-- In Rust, `init_env(build, data)` currently has no `mode` argument and
-- always behaves as mode="MAIN". A BuffMode enum is needed.

-- `local buffMode` — a new local variable, initially nil (Rust: let mut buff_mode: BuffMode)
local buffMode
if mode == "CALCS" then
    -- "CALCS" mode = the Calcs tab in the PoB UI (gear/gem comparison).
    -- The user can choose the buff level via a drop-down. The choice is stored
    -- in `env.calcsInput.misc_buffMode` as one of the four strings below.
    -- In Rust: `build.calcs_input.misc_buff_mode` (not yet modelled).
    -- For oracle builds, this branch is NEVER taken (mode is always "MAIN").
    buffMode = env.calcsInput.misc_buffMode
else
    -- All other modes ("MAIN", "CALCULATOR", "MINION") hard-code "EFFECTIVE".
    -- This is the oracle build path: all 30 builds run in "MAIN" mode → EFFECTIVE.
    buffMode = "EFFECTIVE"
end

-- Four mutually exclusive buff levels, ordered from most permissive to least:
-- "EFFECTIVE" — fully in combat, enemy is present, all conditional mods active.
-- "COMBAT"    — in combat but enemy-dependent effects are excluded.
-- "BUFFED"    — buffs and self-cast auras active, but no combat events.
-- (anything else, incl. nil/"UNBUFFED") — plain character sheet, no buffs.
if buffMode == "EFFECTIVE" then
    -- All three flags true.
    env.mode_buffs = true
    env.mode_combat = true
    env.mode_effective = true
elseif buffMode == "COMBAT" then
    -- mode_effective = false: enemy-dependent effects are excluded.
    -- e.g. "Effective" condition mods (enemy resistances, enemy buffs) don't apply.
    env.mode_buffs = true
    env.mode_combat = true
    env.mode_effective = false
elseif buffMode == "BUFFED" then
    -- mode_combat = false: combat events (flasks, on-hit, warcry exert) excluded.
    -- mode_effective = false: as above.
    env.mode_buffs = true
    env.mode_combat = false
    env.mode_effective = false
else
    -- "UNBUFFED" or nil: plain character sheet. No buffs, no combat, no enemy.
    -- modDB.conditions["Buffed/Combat/Effective"] will all be false below.
    env.mode_buffs = false
    env.mode_combat = false
    env.mode_effective = false
end
```

### CalcSetup.lua lines 105–110 — injection into modDB.conditions

These lines are inside `calcs.initModDB(env, modDB)` which is called at line
476 of `calcs.initEnv()`, AFTER the buff-mode dispatch above sets the flags:

```lua
-- CalcSetup.lua lines 103–110 (inside calcs.initModDB)
modDB:NewMod("AlchemistsGenius", "FLAG", true, "Base", { type = "Condition", var = "AlchemistsGenius" })
modDB:NewMod("LuckyHits", "FLAG", true, "Base", { type = "Condition", var = "LuckyHits" })
modDB:NewMod("Convergence", "FLAG", true, "Base", { type = "Condition", var = "Convergence" })
modDB:NewMod("PhysicalDamageReduction", "BASE", -15, "Base", { type = "Condition", var = "Crushed" })
modDB:NewMod("CritChanceCap", "BASE", 100, "Base")

-- Mirror the three env flags into the modDB conditions table.
-- `modDB.conditions` is a plain Lua table used as a dictionary of booleans.
-- It is checked by eval_mod's Condition tag handler:
--   if modDB.conditions[tag.var] (truthy check — nil is falsy, true is truthy)
-- In Rust: mod_db.set_condition("Buffed", env.mode_buffs) etc.
modDB.conditions["Buffed"]    = env.mode_buffs    -- line 108
modDB.conditions["Combat"]    = env.mode_combat   -- line 109
modDB.conditions["Effective"] = env.mode_effective -- line 110
```

**Important:** `modDB.conditions["Buffed"]` being `false` (not `nil`) is still
falsy in Lua. The Condition tag check in `modDB:CheckFlag()` / eval_mod does a
plain truthiness test: `if conditions[tag.var] then`. So `false` and `nil` are
both "not active". In Rust, `mod_db.conditions.get("Buffed")` returns
`Option<&bool>` — both `None` and `Some(false)` mean the condition fails.
`mod_db.conditions["Buffed"]` (via `Index`) panics on missing key in the Rust
implementation; use `mod_db.conditions.get("Buffed").copied().unwrap_or(false)`.

### CalcActiveSkill.lua lines 234–243 — skill flag mirroring

After `calcs.initEnv()` calls the buff-mode dispatch, `calcs.buildActiveSkill()`
mirrors the env flags onto each active skill's `skillFlags` table. This is a
secondary propagation (the primary consumer is the `if env.mode_combat then`
guards in CalcPerform.lua):

```lua
-- CalcActiveSkill.lua lines 234-243, inside calcs.buildActiveSkill()
-- skillFlags is the per-skill flags table (a plain table used as a set).
-- In Rust: ActiveSkill has no skillFlags yet; when implemented, add:
--   skill.skill_flags.buffs = env.mode_buffs
--   skill.skill_flags.combat = env.mode_combat
--   skill.skill_flags.effective = env.mode_effective
if env.mode_buffs then
    skillFlags.buffs = true        -- gates warcry exert (CalcPerform.lua line 1331)
end
if env.mode_combat then
    skillFlags.combat = true       -- gates flask/tincture application (CalcPerform.lua line 1772)
end
if env.mode_effective then
    skillFlags.effective = true    -- gates enemy-dependent effects (CalcOffence.lua line 309 etc.)
end
```

### CalcPerform.lua — representative `env.mode_*` consumption sites

The three flags gate 30+ conditional branches across Perform, Defence, and
Offence. Representative examples:

| Line | Flag | Effect gated |
|------|------|--------------|
| CalcPerform 232 | `mode_combat` | Combat config conditions (on-kill, recent events) |
| CalcPerform 368 | `mode_effective` | Enemy condition contributions |
| CalcPerform 612 | `mode_combat` | Flask and tincture application |
| CalcPerform 1331 | `mode_buffs` | Warcry exert buff |
| CalcPerform 1991 | `mode_buffs` | Self-cast buff aura application |
| CalcPerform 2131 | `mode_buffs` | Aura buff application to player |
| CalcPerform 2474 | `mode_effective` | Curse application to enemy |
| CalcPerform 2864 | `mode_effective` | AuraDebuff application |
| CalcPerform 2926 | `mode_combat` | On-kill/on-hit conditions |
| CalcDefence 736 | `mode_effective` | Enemy block reduction |
| CalcDefence 1131 | `mode_effective` | Enemy damage reduction contributions |
| CalcOffence 309 | `mode_effective` | Enemy resist/accuracy contributions |
| CalcOffence 2897 | `mode_effective` | Enemy self-crit-chance contribution |
| CalcOffence 3965 | `mode_effective` | Impale chance |

In the oracle builds (always `"EFFECTIVE"` mode), every `if env.mode_buffs`,
`if env.mode_combat`, and `if env.mode_effective` branch is taken. Missing
these flags in Rust means those branches are silently skipped, causing wrong
(lower) values for buffs, flask effects, curse application, and effective DPS.

## Key Lua Semantics

### `env.mode_buffs` is an env-level boolean, not a modDB mod

Unlike most CalcSetup logic, the three mode flags are **fields on the `env`
table**, not mods in `modDB`. They are then *mirrored* into `modDB.conditions`
(lines 108–110). Downstream code tests both:

1. `if env.mode_effective then` — direct env check (CalcPerform.lua, CalcOffence.lua)
2. `modDB:Flag(nil, "Condition:Effective")` indirectly via Condition tags in
   item mods (e.g. `{ type = "Condition", var = "Effective" }`)

In Rust, two things must happen:
- Add `mode_buffs: bool`, `mode_combat: bool`, `mode_effective: bool` fields to
  `CalcEnv` (in `env.rs`).
- Set `mod_db.set_condition("Buffed", env.mode_buffs)` etc. at the end of
  `add_base_constants()` (or at the end of `init_env()`), so that Condition-tag
  mods with `var = "Buffed"` / `"Combat"` / `"Effective"` evaluate correctly.

### Truthy vs typed booleans

In Lua `env.mode_buffs = false` stores the boolean `false`. The check
`if env.mode_buffs` is a truthiness test — `false` and `nil` both fail.
In Rust `CalcEnv::mode_buffs: bool` is always typed; `if env.mode_buffs` is
always a typed boolean check. There is no semantic difference here, but be
aware that in Lua **all four modes result in valid booleans** (not nil) so
`modDB.conditions["Buffed"]` always gets assigned. In Rust, call
`set_condition("Buffed", mode_buffs)` for all four paths.

### `env.calcsInput.misc_buffMode` (CALCS mode only)

`env.calcsInput` is `build.calcsTab.input` — a table of UI control values from
the Calcs tab. The `misc_buffMode` field is a string matching one of
`"EFFECTIVE"`, `"COMBAT"`, `"BUFFED"`, or `"UNBUFFED"`. This field is not
present in the Rust `Build` struct (the Calcs tab is not modelled). For the
purposes of oracle builds, this branch is unreachable — only the `else` branch
(`buffMode = "EFFECTIVE"`) is taken.

### `a and b or c` is not used here — all assignments are direct

The buff-mode block uses plain `if/elseif/else` assignment, not the `a and b
or c` ternary idiom. No translation gotcha here.

## Existing Rust Code

**Files:** `crates/pob-calc/src/calc/env.rs`, `crates/pob-calc/src/calc/setup.rs`

### What exists

- `CalcEnv` struct (`env.rs:210`) has a `mode: CalcMode` field where
  `CalcMode` is `Normal | Calculator` (`env.rs:186–192`). This covers the
  distinction between `"MAIN"` and `"CALCULATOR"` Lua modes at a coarse level,
  but has no bearing on buff-mode flags.
- `ModDb` (`mod_db/mod.rs:24`) has `pub conditions: HashMap<String, bool>` and
  `pub fn set_condition(&mut self, var: &str, value: bool)`.
- `setup::init_env()` calls `add_base_constants()` which populates many
  `set_condition` calls. However, `"Buffed"`, `"Combat"`, and `"Effective"` are
  **not among them** — they are never set anywhere in the Rust codebase.

### What's missing

1. **Three boolean fields on `CalcEnv`** (`env.rs`):
   ```rust
   pub mode_buffs: bool,
   pub mode_combat: bool,
   pub mode_effective: bool,
   ```
   Initialized in `CalcEnv::new()` to `true` (the oracle default = EFFECTIVE).

2. **`set_condition` calls for the three mode conditions** (`setup.rs` or
   `env.rs`):
   ```rust
   env.player.mod_db.set_condition("Buffed",    env.mode_buffs);
   env.player.mod_db.set_condition("Combat",    env.mode_combat);
   env.player.mod_db.set_condition("Effective", env.mode_effective);
   ```
   These must be called at the end of `init_env()` (after
   `add_base_constants()` returns), mirroring CalcSetup.lua lines 108–110
   which run at the end of `calcs.initModDB()`.

3. **All `if env.mode_combat` / `if env.mode_buffs` / `if env.mode_effective`
   guards** in `perform.rs`, `defence.rs`, and `offence.rs` are currently
   **absent** — those conditional branches are always taken (or always skipped)
   with no mode gating. Since every oracle build uses EFFECTIVE mode and the
   Rust code implicitly behaves as if everything is active, the numeric impact
   on oracle tests is **zero** today. However, the structural correctness gap is
   significant: any non-EFFECTIVE build would produce wrong results.

4. **`calcsInput.misc_buffMode` / the CALCS-mode path** — not needed for oracle
   builds. Can be deferred indefinitely unless CALCS-mode support is added.

### What's wrong / notable gaps

- The `Condition` tag with `var = "Effective"` appears 40+ times in
  `mod_parser_generated.rs` (lines ~21275–23980) and is currently generated as
  `ModFlags::NONE /* { type = "Condition", var = "Effective" } */`. These stubs
  mean the condition is not evaluated: the mod always applies regardless of
  effective mode. This is a SETUP-04 (eval_mod stubs) issue, not specific to
  this chunk. However, once SETUP-04 is fixed and the `Effective` Condition tag
  evaluates correctly, it will query `mod_db.conditions["Effective"]` — which
  will be `None` (missing) unless this chunk populates it. So SETUP-13 is a
  **prerequisite for SETUP-04 to work correctly for `var = "Effective"` tags**.
- Similarly, `"Buffed"` and `"Combat"` condition tags exist in PoB mod text
  (e.g. `"Damage is Lucky while Buffed"`) and will need the conditions set.

## What Needs to Change

1. **Add three boolean fields to `CalcEnv`** (`crates/pob-calc/src/calc/env.rs`):
   ```rust
   pub mode_buffs: bool,     // true = buffs/auras are active
   pub mode_combat: bool,    // true = in combat (flasks, warcry, on-hit active)
   pub mode_effective: bool, // true = enemy is present, enemy-dependent effects active
   ```
   Initialize all three to `true` in `CalcEnv::new()` (oracle default = EFFECTIVE).

2. **Mirror the mode flags into `modDB.conditions`** at the end of
   `setup::init_env()` (`crates/pob-calc/src/calc/setup.rs`), after the
   existing `add_base_constants()` call:
   ```rust
   env.player.mod_db.set_condition("Buffed",    env.mode_buffs);
   env.player.mod_db.set_condition("Combat",    env.mode_combat);
   env.player.mod_db.set_condition("Effective", env.mode_effective);
   ```
   This mirrors CalcSetup.lua lines 108–110.

3. **Add `if env.mode_combat`, `if env.mode_buffs`, `if env.mode_effective`
   guards** in `perform.rs`, `defence.rs`, and `offence.rs` at every site
   where the Lua gates logic on these flags (see the table above for a
   representative list; the full list spans ~30 branches). This structural
   work is spread across Tier 1–5 chunk implementations — each chunk that
   contains a mode-gated branch must add the corresponding `env.mode_*` check.
   This chunk's deliverable is only items 1 and 2 above.

4. **(Optional / deferred)** Support non-EFFECTIVE buff modes: add a
   `BuffMode` enum (`Effective`, `Combat`, `Buffed`, `Unbuffed`) and a way to
   set it from the caller (e.g. a second argument to `init_env`). This is not
   needed for oracle builds and can be deferred.
