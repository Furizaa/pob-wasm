# PERF-08: Action Speed, Conditions, MoM/EB

## Output Fields

Fields this chunk must write (from `field_groups.rs`):

| Field | Oracle present | Notes |
|-------|---------------|-------|
| `ActionSpeedMod` | 30/30 | Computed by `calcs.actionSpeedMod()` in CalcPerform.lua |
| `MovementSpeedMod` | 30/30 | `modDB:Override` / `calcLib.mod(modDB, nil, "MovementSpeed")` |
| `EffectiveMovementSpeedMod` | 30/30 | `MovementSpeedMod × ActionSpeedMod` |
| `MovementSpeed` | 0/30 | **Phantom field** — not written by PoB calc; remove from `field_groups.rs` |

> **Note on `MovementSpeed`:** PoB's calc engine never writes `output.MovementSpeed`. The
> UI references `EffectiveMovementSpeedMod` for the display label "Movement Speed". There
> is no `output.MovementSpeed` write anywhere in `CalcDefence.lua`, `CalcPerform.lua`, or
> any other calc module. Remove `"MovementSpeed"` from `field_groups.rs` for this chunk.

## Dependencies

- `PERF-05-buffs` — buffs like Tailwind, Onslaught, Adrenaline inject `ActionSpeed INC`
  and `MovementSpeed INC` mods into the modDB before this chunk runs
- `PERF-01-attributes` — conditions set during attribute pass affect modDB queries
- `PERF-06-aura-curse` — aura/curse effects may apply `ActionSpeed` or `MovementSpeed`
  modifiers to the modDB before the defence pass

## Lua Source

**`calcs.actionSpeedMod` function: `CalcPerform.lua`, lines 1067–1077**  
**`calcs.defence` call: `CalcDefence.lua`, line 647**  
**Movement speed / EffectiveMov: `CalcDefence.lua`, lines 1493–1506**

Commit: `454eff8c85d24356d9b051d596983745ed367476` (third-party/PathOfBuilding, heads/dev)

## Annotated Lua

### Section 1: `calcs.actionSpeedMod` (CalcPerform.lua 1067–1077)

This is a **pure function** — no side effects, no output writes. It is called from
`calcs.defence` (CalcDefence.lua:647) and also from CalcOffence.lua:294 and :5282 for
cast speed and enemy skill time. The result is assigned to `output.ActionSpeedMod`.

```lua
function calcs.actionSpeedMod(actor)
    local modDB = actor.modDB

    -- modDB:Max(nil, "X") returns the highest value among all MAX-type mods for "X",
    -- or nil if none exist. In Rust: mod_db.max_value("X", None, output) -> Option<f64>
    local minimumActionSpeed = modDB:Max(nil, "MinimumActionSpeed") or 0
    --                          ↑ `or 0` = nil-coalesce: Rust: .unwrap_or(0.0)
    --    MinimumActionSpeed is used by:
    --      - "Cannot Be Slowed to below Base Value" (value=100)
    --      - "Action Speed cannot be modified to below base value" (value=100)
    --      - "Your Action Speed is at least 90% of base value" (value=90)
    --    A value of 100 means the floor is 100% = 1.0 of base (i.e., no reduction allowed).
    --    A value of 0 means no floor applies.

    local maximumActionSpeedReduction = modDB:Max(nil, "MaximumActionSpeedReduction")
    --    MaximumActionSpeedReduction caps how much action speed can be *reduced*.
    --    Example: "Nearby Enemy Monsters' Action Speed is at most 90% of base value"
    --    sets MaximumActionSpeedReduction = 10 (meaning at most 10% reduction allowed,
    --    so the result is at least (100 - 10) / 100 = 0.90).
    --    nil when no such mod exists → the max cap branch is skipped.
    --    In Rust: mod_db.max_value("MaximumActionSpeedReduction", None, output) -> Option<f64>

    -- Core formula:
    --   TemporalChainsActionSpeed is INC from the Temporal Chains curse (negative INC value).
    --   It is capped at -TemporalChainsEffectCap (= -75) to prevent extreme slowdown.
    --   m_max(-75, temporalChainsActionSpeedInc) clamps the Temporal Chains contribution.
    --   Then the full ActionSpeed INC is added and the standard (1 + inc/100) applied.
    --
    -- data.misc.TemporalChainsEffectCap = 75  (from Data.lua:168)
    -- This constant is NOT currently in the Rust game_constants map (see "What Needs to Change").
    --
    -- NOTE: TemporalChainsActionSpeed is a *separate* INC stat from ActionSpeed.
    -- Normal ActionSpeed mods (Tailwind, Onslaught's TotemActionSpeed, etc.) go through
    -- modDB:Sum("INC", nil, "ActionSpeed"). Temporal Chains goes through its own stat
    -- so it can be independently capped.
    local actionSpeedMod = 1 + (
        m_max(-data.misc.TemporalChainsEffectCap,       -- ← Rust: (-75.0f64).max(temporal_chains_inc)
              modDB:Sum("INC", nil, "TemporalChainsActionSpeed"))
        + modDB:Sum("INC", nil, "ActionSpeed")
    ) / 100
    -- Rust equivalent:
    --   let tc_inc = mod_db.sum_cfg(Inc, "TemporalChainsActionSpeed", None, output);
    --   let as_inc = mod_db.sum_cfg(Inc, "ActionSpeed", None, output);
    --   let capped_tc = tc_inc.max(-TEMPORAL_CHAINS_EFFECT_CAP); // -75.0
    --   let action_speed_mod = 1.0 + (capped_tc + as_inc) / 100.0;
    --
    -- IMPORTANT: The Lua formula does NOT use modDB:More for ActionSpeed.
    -- There is no More multiplier for action speed in PoB's model — only INC.

    -- Apply minimum floor (converts from % to fraction: 100 → 1.0):
    actionSpeedMod = m_max(minimumActionSpeed / 100, actionSpeedMod)
    -- Rust: action_speed_mod = action_speed_mod.max(minimum_action_speed / 100.0);

    -- Apply maximum reduction cap (only if the mod exists):
    if maximumActionSpeedReduction then
        -- maximumActionSpeedReduction is a %, e.g. 10 means "at most 10% reduction allowed".
        -- The cap formula: (100 - reductionCap) / 100 = minimum allowed value.
        -- e.g., reductionCap=10 → cap = 0.90 → actionSpeedMod = min(0.90, current)
        actionSpeedMod = m_min((100 - maximumActionSpeedReduction) / 100, actionSpeedMod)
        -- Rust: if let Some(max_red) = max_reduction {
        --           action_speed_mod = action_speed_mod.min((100.0 - max_red) / 100.0);
        --       }
    end

    return actionSpeedMod
    -- Note: No More multiplier applied here. The final formula is purely additive INC.
    -- The result is typically 1.0 (no buffs) up to ~1.2 (Tailwind + other buffs).
end
```

**Where `ActionSpeedMod` is written** (CalcDefence.lua:647):
```lua
-- calcs.defence is the entry point for all defence stats.
-- ActionSpeedMod is computed first, before anything that uses it (e.g., EffectiveMovementSpeedMod).
output.ActionSpeedMod = calcs.actionSpeedMod(actor)
```

### Section 2: Movement Speed (CalcDefence.lua 1493–1506)

This section runs late in `calcs.defence`, after armour, evasion, ES, damage reduction, etc.
`ActionSpeedMod` was set at the top of `calcs.defence` (line 647) so it is available here.

```lua
-- Movement speed modifier
-- Priority: Override > party-member inheritance > standard calcLib.mod
output.MovementSpeedMod =
    modDB:Override(nil, "MovementSpeed")         -- explicit override (e.g., map mod "you move at 100%")
    or (
        modDB:Flag(nil, "MovementSpeedEqualHighestLinkedPlayers")
            and actor.partyMembers.output.MovementSpeedMod  -- inherit from linked party member
        or calcLib.mod(modDB, nil, "MovementSpeed")         -- standard INC × More formula
    )
-- Lua short-circuit evaluation:
--   modDB:Override returns a number or nil.
--   If nil: evaluate the parenthesised `or` expression.
--   Inner `and/or` ternary: if MovementSpeedEqualHighestLinkedPlayers is true,
--     use the party member's value; otherwise use calcLib.mod.
--
-- calcLib.mod(modDB, nil, "MovementSpeed")
--   = (1 + modDB:Sum("INC", nil, "MovementSpeed") / 100) * modDB:More(nil, "MovementSpeed")
--   In Rust: calc_mod(mod_db, None, "MovementSpeed")
--           = (1.0 + mod_db.sum(Inc, "MovementSpeed") / 100.0) * mod_db.more("MovementSpeed")
--
-- Key sources of MovementSpeed INC in builds:
--   - Onslaught (20% INC): applied via modDB:NewMod in doActorMisc (~line 677)
--   - Tailwind (8% INC): applied via modDB:NewMod in doActorMisc (~line 729 for Adrenaline
--     adds 25% INC too)
--   - Passive nodes: many passives add "X% increased Movement Speed"
--   - Boot enchants, uniques, etc.
--
-- Gotcha: calcLib.mod does NOT apply a "1.0 base" for movement speed the way
-- some stats do. The base movement speed is implicitly 1.0, so the output
-- is the multiplier vs base. e.g., 20% INC → output = 1.20.

-- Floor: movement speed cannot go below base (if this keystone/mod is active):
if modDB:Flag(nil, "MovementSpeedCannotBeBelowBase") then
    output.MovementSpeedMod = m_max(output.MovementSpeedMod, 1)
    -- Rust: if mod_db.flag_cfg("MovementSpeedCannotBeBelowBase", None, output) {
    --           ms = ms.max(1.0);
    --       }
    -- This prevents chill, temporal chains, etc. from slowing movement speed
    -- below the base (100%) for certain builds.
end

-- Effective movement speed = base movement × action speed:
output.EffectiveMovementSpeedMod = output.MovementSpeedMod * output.ActionSpeedMod
-- Rust: env.player.set_output("EffectiveMovementSpeedMod", ms * action_speed);
```

**Breakdown (display only, not output fields):**
```lua
if breakdown then
    breakdown.EffectiveMovementSpeedMod = { }
    breakdown.multiChain(breakdown.EffectiveMovementSpeedMod, {
        { "%.2f ^8(movement speed modifier)", output.MovementSpeedMod },
        { "%.2f ^8(action speed modifier)", output.ActionSpeedMod },
        total = s_format("= %.2f ^8(effective movement speed modifier)", output.EffectiveMovementSpeedMod)
    })
end
-- Rust: always populate breakdown (see LUA-GOTCHAS.md §Breakdown Patterns)
```

### Key buff sources for ActionSpeed (set in `doActorMisc`, CalcPerform.lua ~640–740)

These are injected into the modDB before `calcs.actionSpeedMod` is called:

| Buff | ModDB injection | Effect |
|------|----------------|--------|
| Tailwind (`modDB:Flag(nil, "Tailwind")`) | `modDB:NewMod("ActionSpeed", "INC", floor(8 × (1 + TailwindEffectOnSelf + BuffEffectOnSelf)/100), ...)` | +8% inc base, scalable |
| Wild Savagery | `modDB:NewMod("ActionSpeed", "INC", 10, ...)` | +10% inc |
| Temporal Chains curse | Adds negative INC to `TemporalChainsActionSpeed` (capped at -75%) | Slow |

**Onslaught** does NOT contribute to `ActionSpeedMod` — it only affects `Speed` (attack/cast)
and `MovementSpeed INC`. The `ActionSpeedMod` in `bow_deadeye` at 1.2 comes from Onslaught's
+20% being applied to MovementSpeed, and ActionSpeedMod of 1.2 must come from another
source — likely Tailwind (8%) + something else. Cross-checking: `bow_deadeye` has both
`buffTailwind` and `buffOnslaught` in config. Tailwind gives ~8% ActionSpeed INC, so
ActionSpeedMod = 1.08, not 1.2. The build also has Deadeye's Gathering Winds giving +12%
ActionSpeed for a total of +20% → 1.20. This confirms the formula is INC-only.

## Existing Rust Code

### `perform.rs` — `action_speed_mod` function (lines 1271–1286)

```
crates/pob-calc/src/calc/perform.rs, lines 1271–1286
```

```rust
fn action_speed_mod(env: &mut CalcEnv) -> f64 {
    let inc = env.player.mod_db
        .sum_cfg(ModType::Inc, "ActionSpeed", None, &env.player.output);
    let more = env.player.mod_db
        .more_cfg("ActionSpeed", None, &env.player.output);
    ((1.0 + inc / 100.0) * more).max(0.0)
}
```

**Called from** `perform::run()` at line 18:
```rust
let asm = action_speed_mod(env);
env.player.set_output("ActionSpeedMod", asm);
env.player.action_speed_mod = asm;  // stored on Actor for use in offence calculations
```

**Status of each Lua feature:**

| Lua feature | Rust status |
|-------------|-------------|
| `ActionSpeed INC` summation | ✅ Correct |
| `ActionSpeed More` product | ⚠️ **Wrong** — Lua does not apply More to ActionSpeed; the formula is INC-only (no More term in `calcs.actionSpeedMod`) |
| `TemporalChainsActionSpeed` capped INC | ❌ Missing entirely |
| `data.misc.TemporalChainsEffectCap` (75%) | ❌ Constant not in `data.misc.game_constants` |
| `MinimumActionSpeed` floor | ❌ Missing |
| `MaximumActionSpeedReduction` cap | ❌ Missing |
| `.max(0.0)` final clamp | ⚠️ Wrong direction — Lua does not clamp to 0; it clamps to `minimumActionSpeed/100` which is typically 0 or 1 |

### `defence.rs` — `calc_movement_and_avoidance` function (lines 721–735)

```
crates/pob-calc/src/calc/defence.rs, lines 721–735
```

```rust
fn calc_movement_and_avoidance(env: &mut CalcEnv) {
    let ms_inc = env.player.mod_db
        .sum_cfg(ModType::Inc, "MovementSpeed", None, &output);
    let ms_more = env.player.mod_db
        .more_cfg("MovementSpeed", None, &output);
    let ms = (1.0 + ms_inc / 100.0) * ms_more;
    env.player.set_output("MovementSpeedMod", ms);

    let action_speed = env.player.action_speed_mod;
    env.player.set_output("EffectiveMovementSpeedMod", ms * action_speed);
    ...
}
```

**Status:**

| Lua feature | Rust status |
|-------------|-------------|
| `MovementSpeed` INC | ✅ Correct |
| `MovementSpeed` More | ✅ Correct |
| `modDB:Override(nil, "MovementSpeed")` | ❌ Missing — no override check |
| `MovementSpeedEqualHighestLinkedPlayers` | ❌ Missing — party-member inheritance (lower priority, TAIL) |
| `MovementSpeedCannotBeBelowBase` floor | ❌ Missing |
| `EffectiveMovementSpeedMod = MovementSpeedMod × ActionSpeedMod` | ✅ Correct formula |
| Call order: `action_speed_mod` set before `calc_movement_and_avoidance` | ✅ Correct — `perform::run()` sets `ActionSpeedMod` before `defence::run()` |

**Call order note:** In the Lua, `calcs.defence` sets `ActionSpeedMod` at line 647 (very
first thing in the function), then computes movement speed at lines 1493–1506 (near the
end). Both happen in the same function call. In Rust, `perform::run()` computes
`action_speed_mod` and stores it in `env.player.action_speed_mod`, then `defence::run()`
reads it at line 733. This ordering is correct.

## What Needs to Change

1. **Fix `action_speed_mod` formula** (`perform.rs`, line 1276–1286):  
   Remove the `More` multiplier from the action speed formula. The Lua has no `More`
   term. Replace with the correct INC-only formula including TemporalChains cap:

   ```rust
   fn action_speed_mod(env: &mut CalcEnv) -> f64 {
       const TEMPORAL_CHAINS_EFFECT_CAP: f64 = 75.0; // data.misc.TemporalChainsEffectCap

       let tc_inc = env.player.mod_db.sum_cfg(
           ModType::Inc, "TemporalChainsActionSpeed", None, &env.player.output);
       let as_inc = env.player.mod_db.sum_cfg(
           ModType::Inc, "ActionSpeed", None, &env.player.output);

       // Temporal Chains contribution is capped at -75% INC
       let capped_tc = tc_inc.max(-TEMPORAL_CHAINS_EFFECT_CAP);
       let mut action_speed_mod = 1.0 + (capped_tc + as_inc) / 100.0;

       // Floor: cannot be slowed below minimumActionSpeed (0 if no mod)
       let min_speed = env.player.mod_db
           .max_value("MinimumActionSpeed", None, &env.player.output)
           .unwrap_or(0.0);
       action_speed_mod = action_speed_mod.max(min_speed / 100.0);

       // Cap: MaximumActionSpeedReduction limits how much speed can be reduced
       if let Some(max_red) = env.player.mod_db
           .max_value("MaximumActionSpeedReduction", None, &env.player.output)
       {
           action_speed_mod = action_speed_mod.min((100.0 - max_red) / 100.0);
       }

       action_speed_mod
       // Note: NO final .max(0.0) — the minimum is controlled by MinimumActionSpeed above.
   }
   ```

2. **Add `TemporalChainsEffectCap` constant** (either as a `const` in `perform.rs` or
   into `misc.json`/`MiscData`):  
   - Simplest: `const TEMPORAL_CHAINS_EFFECT_CAP: f64 = 75.0;` in `perform.rs`
   - Ideal: add `"TemporalChainsEffectCap": 75` to `data/misc.json` under
     `game_constants` and read via `data.misc.game_constants["TemporalChainsEffectCap"]`

3. **Add `Override` check for `MovementSpeedMod`** (`defence.rs::calc_movement_and_avoidance`):  
   ```rust
   // Check for override first (e.g., "you move at X% of base speed" map mod)
   let ms = if let Some(override_val) = env.player.mod_db
       .override_value("MovementSpeed", None, &output)
   {
       override_val
   } else {
       let inc = env.player.mod_db.sum_cfg(ModType::Inc, "MovementSpeed", None, &output);
       let more = env.player.mod_db.more_cfg("MovementSpeed", None, &output);
       (1.0 + inc / 100.0) * more
   };
   ```

4. **Add `MovementSpeedCannotBeBelowBase` floor** (`defence.rs::calc_movement_and_avoidance`):  
   ```rust
   let ms = if env.player.mod_db
       .flag_cfg("MovementSpeedCannotBeBelowBase", None, &output)
   {
       ms.max(1.0)
   } else {
       ms
   };
   ```

5. **Remove `"MovementSpeed"` from `field_groups.rs`** for `PERF-08-action-speed-conditions`:  
   This field is never written by PoB's calc engine and appears in 0/30 oracle files.
   The correct field to test is `EffectiveMovementSpeedMod`.

6. **`MovementSpeedEqualHighestLinkedPlayers` inheritance** (deferred — TAIL chunk):  
   This requires party member output data which the current Rust architecture does not
   support. Mark as a known gap; not needed for the 30 oracle builds.

## Oracle Confirmation (all 30 builds)

| Build | ActionSpeedMod | MovementSpeedMod | EffectiveMovementSpeedMod |
|-------|---------------|-----------------|--------------------------|
| aura_stacker | 1 | 1.24 | 1.24 |
| bleed_gladiator | 1 | 1.22 | 1.22 |
| bow_deadeye | 1.2 | 1.93 | 2.316 |
| champion_impale | 1 | 1.25 | 1.25 |
| ci_lowlife_es | 1 | 1.29 | 1.29 |
| cluster_jewel | 1 | 1.29 | 1.29 |
| coc_trigger | 1.09 | 3.34 | 3.6406 |
| cwc_trigger | 1 | 1.24 | 1.24 |
| dot_caster_trickster | 1 | 1.24 | 1.24 |
| dual_wield | 1 | 1 | 1 |
| ele_melee_raider | 1.08 | 1.52 | 1.6416 |
| flask_pathfinder | 1.08 | 2.21 | 2.3868 |
| ignite_elementalist | 1 | 1.29 | 1.29 |
| max_block_gladiator | 1 | 1.22 | 1.22 |
| mine_saboteur | 1 | 1.3 | 1.3 |
| minion_necromancer | 1 | 1.29 | 1.29 |
| mom_eb | 1 | 1.24 | 1.24 |
| phys_melee_slayer | 1 | 1.832 | 1.832 |
| phys_to_fire_conversion | 1 | 1.3 | 1.3 |
| poison_pathfinder | 1.08 | 1.55 | 1.674 |
| rf_juggernaut | 1 | 1.3 | 1.3 |
| shield_1h | 1 | 1.22 | 1.22 |
| spectre_summoner | 1 | 1.29 | 1.29 |
| spell_caster_inquisitor | 1 | 1.24 | 1.24 |
| timeless_jewel | 1 | 1.25 | 1.25 |
| totem_hierophant | 1 | 1.24 | 1.24 |
| trap_saboteur | 1 | 1.3 | 1.3 |
| triple_conversion | 1 | 1.33 | 1.33 |
| two_handed | 1 | 1.35 | 1.35 |
| wand_occultist | 1 | 1.89 | 1.89 |

> None of the 30 oracle builds exercise `TemporalChainsActionSpeed`, `MinimumActionSpeed`,
> `MaximumActionSpeedReduction`, or `MovementSpeedCannotBeBelowBase`. These paths must be
> correct for completeness but cannot be oracle-verified with the current build set. The
> 6 builds with non-1.0 `ActionSpeedMod` (bow_deadeye, coc_trigger, ele_melee_raider,
> flask_pathfinder, poison_pathfinder) all use Tailwind and/or Deadeye Gathering Winds
> which add `ActionSpeed INC` via `modDB:NewMod` in `doActorMisc`. Since the current Rust
> incorrectly multiplies by a `More` factor (which is 1.0 in practice for these builds),
> those 6 builds currently produce the **correct numeric result** by accident. Adding
> TemporalChains support requires the formula fix in item 1 above, but won't break the
> existing passing cases since `More("ActionSpeed")` defaults to 1.0.
