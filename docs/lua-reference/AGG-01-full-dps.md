# AGG-01-full-dps: FullDPS & Multi-Skill Aggregation

## Output Fields

| Field | Type | Written at |
|-------|------|------------|
| `FullDPS` | `f64` | Calcs.lua:429 (buildOutput), also 81/107/131/143 (calculator helpers) |
| `FullDotDPS` | `f64` | Calcs.lua:430 (buildOutput), also 82/108/132/144 |

Both fields are written by `calcs.buildOutput` (the main display path) and by several
calculator-helper functions (`getCalculator`, `getMiscCalculator`). The authoritative
implementation is `calcs.calcFullDPS` (lines 176–388), which is called by all of them.

## Dependencies

- `OFF-04-speed-dps` — `TotalDPS` must be correct for every skill before it is aggregated.
- `OFF-05-ailments` — `BleedDPS`, `IgniteDPS`, `PoisonDPS` (per-skill) feed the ailment
  aggregation logic inside `calcFullDPS`.
- `OFF-06-dot-impale` — `ImpaleDPS`, `DecayDPS`, `TotalDot` feed the same aggregation.
- `OFF-07-combined-dps` — `CombinedDPS` is assembled in CalcOffence and is the hit-DPS
  component that `calcFullDPS` sums across skills. `TotalDotDPS` from the player's main
  skill is also read here (though `FullDotDPS` replaces it with the cross-skill aggregate).
- `MIR-01-mirages` — `MirageDPS` on the main skill's output contributes to the
  `combinedDPS` sum that `calcFullDPS` accumulates.

## Lua Source

File: `third-party/PathOfBuilding/src/Modules/Calcs.lua`, lines 176–388 (core function),
lines 416–431 (primary call site).

Commit: `454eff8c85d24356d9b051d596983745ed367476`

Math aliases used in this file:
```lua
local m_min  = math.min
local m_ceil = math.ceil
local t_insert = table.insert
```

## Annotated Lua

### 1. Architecture: `calcFullDPS` vs single-skill output

PoB separates two concepts:

- **Per-skill output** (`calcs.perform` + `calcs.offence`): computes `TotalDPS`,
  `CombinedDPS`, `BleedDPS`, `IgniteDPS`, etc. for whichever skill is currently
  `env.player.mainSkill`. This is what the single-skill calcs tab shows.

- **Full DPS output** (`calcs.calcFullDPS`): iterates over *all* active skills that have
  "Include in Full DPS" enabled, calculates each one in isolation, and aggregates their
  contributions. The result is `FullDPS` (combined hit + impale + all ailments) and
  `FullDotDPS` (ailment portion only).

`calcs.buildOutput` calls both: first `calcs.perform(env)` for the selected main skill,
then `calcs.calcFullDPS(...)` for the cross-skill aggregate, then writes `FullDPS` and
`FullDotDPS` onto the player output table.

```lua
-- Calcs.lua:416–431
function calcs.buildOutput(build, mode)
    local env, cachedPlayerDB, cachedEnemyDB, cachedMinionDB = calcs.initEnv(build, mode)
    calcs.perform(env)                                         -- single-skill pass

    local output = env.player.output

    -- Cross-skill aggregate:
    local fullDPS = calcs.calcFullDPS(build, "CALCULATOR", {},
        { cachedPlayerDB = cachedPlayerDB, cachedEnemyDB = cachedEnemyDB,
          cachedMinionDB = cachedMinionDB, env = nil })

    env.player.output.SkillDPS   = fullDPS.skills             -- skill list (display only)
    env.player.output.FullDPS    = fullDPS.combinedDPS         -- ← AGG-01 field
    env.player.output.FullDotDPS = fullDPS.TotalDotDPS         -- ← AGG-01 field
    ...
end
```

> **`SkillDPS` is not an oracle field.** It is a list of per-skill DPS entries used by
> the UI. The Rust does not need to produce it for parity.

---

### 2. `calcFullDPS` — the aggregation function (lines 176–388)

```lua
function calcs.calcFullDPS(build, mode, override, specEnv)
    local fullEnv, cachedPlayerDB, cachedEnemyDB, cachedMinionDB =
        calcs.initEnv(build, mode, override, specEnv)
    local usedEnv = nil

    local fullDPS = {
        combinedDPS      = 0,   -- hit + impale + culling; becomes FullDPS
        TotalDotDPS      = 0,   -- bleed + CB + ignite + burn + poison + caustic + decay + dot; becomes FullDotDPS
        skills           = { }, -- list of per-skill DPS entries for display
        TotalPoisonDPS   = 0,
        causticGroundDPS = 0,
        impaleDPS        = 0,
        igniteDPS        = 0,   -- best ignite across all skills
        burningGroundDPS = 0,   -- best burning ground across all skills
        bleedDPS         = 0,   -- best bleed across all skills
        corruptingBloodDPS = 0, -- best corrupting blood across all skills
        decayDPS         = 0,
        dotDPS           = 0,
        cullingMulti     = 0,
    }
```

> **`igniteDPS` / `bleedDPS` are `max` semantics.** Ignite and bleed do not stack from
> multiple sources — only the highest single-instance DPS matters. The accumulator always
> takes the maximum over skills. Contrast with `TotalPoisonDPS` (summation) and `impaleDPS`
> (summation).

> **`combinedDPS` is NOT the same as the per-skill `CombinedDPS`.** The per-skill
> `output.CombinedDPS` (from CalcOffence OFF-07) is the combined DPS for one skill
> including its own ailments. The `fullDPS.combinedDPS` accumulator here sums only the
> `TotalDPS` values of all included skills and adds impale later. Ailments that do not
> stack (ignite, bleed) are added once at the end, not per-skill.

---

### 3. Per-skill loop (lines 202–337)

```lua
for _, activeSkill in ipairs(fullEnv.player.activeSkillList) do
    if activeSkill.socketGroup and activeSkill.socketGroup.includeInFullDPS then
        local activeSkillCount, enabled = getActiveSkillCount(activeSkill)
        -- activeSkillCount: gem count (e.g. 2 for "spell count = 2" in the socket group)
        -- enabled: whether the skill is enabled (false for inactive Vaal gems)

        if enabled then
            fullEnv.player.mainSkill = activeSkill
            calcs.perform(fullEnv, true)    -- full recalculation with this skill active
            usedEnv = fullEnv
```

> **`calcs.perform(fullEnv, true)` — the `true` flag.** The second argument is
> `skipDefence`; when `true`, CalcDefence is not re-run during the loop. Defence values
> are computed once and reused. This is an optimisation — the Rust equivalent would just
> re-run offence (and triggers/mirages) without defence.

> **`getActiveSkillCount` (lines 150–174):** returns `(count, enabled)`. For normal gems,
> `count = gemData.count or 1`. For Vaal gems, returns `count` for the primary effect and
> checks `enableGlobal1`/`enableGlobal2`. This count is the multiplier for stackable DPS
> (TotalDPS, TotalPoisonDPS, ImpaleDPS) but NOT for non-stackable ailments (ignite, bleed).

#### 3a. Minion branch (lines 210–246)

```lua
            if activeSkill.minion or usedEnv.minion then
                if usedEnv.minion.output.TotalDPS and usedEnv.minion.output.TotalDPS > 0 then
                    -- Accumulate minion hit DPS × count
                    fullDPS.combinedDPS = fullDPS.combinedDPS + usedEnv.minion.output.TotalDPS * activeSkillCount
                end
                -- Minion bleed / ignite: non-stackable, take the max
                if usedEnv.minion.output.BleedDPS and usedEnv.minion.output.BleedDPS > fullDPS.bleedDPS then
                    fullDPS.bleedDPS = usedEnv.minion.output.BleedDPS
                end
                if usedEnv.minion.output.IgniteDPS and usedEnv.minion.output.IgniteDPS > fullDPS.igniteDPS then
                    fullDPS.igniteDPS = usedEnv.minion.output.IgniteDPS
                end
                -- Minion stackable: sum × count
                if usedEnv.minion.output.PoisonDPS and usedEnv.minion.output.PoisonDPS > 0 then
                    fullDPS.TotalPoisonDPS = fullDPS.TotalPoisonDPS + usedEnv.minion.output.TotalPoisonDPS * activeSkillCount
                end
                if usedEnv.minion.output.ImpaleDPS and usedEnv.minion.output.ImpaleDPS > 0 then
                    fullDPS.impaleDPS = fullDPS.impaleDPS + usedEnv.minion.output.ImpaleDPS * activeSkillCount
                end
                if usedEnv.minion.output.DecayDPS and usedEnv.minion.output.DecayDPS > 0 then
                    fullDPS.decayDPS = fullDPS.decayDPS + usedEnv.minion.output.DecayDPS
                    -- NOTE: no × activeSkillCount here — minion DecayDPS is not scaled by count
                end
                if usedEnv.minion.output.TotalDot and usedEnv.minion.output.TotalDot > 0 then
                    fullDPS.dotDPS = fullDPS.dotDPS + usedEnv.minion.output.TotalDot
                    -- NOTE: no × activeSkillCount here
                end
                -- Culling: take the max
                if usedEnv.minion.output.CullMultiplier and usedEnv.minion.output.CullMultiplier > 1
                   and usedEnv.minion.output.CullMultiplier > fullDPS.cullingMulti then
                    fullDPS.cullingMulti = usedEnv.minion.output.CullMultiplier
                end
                -- Special case: Absolution / Dominating Blow / Holy Strike count override
                if (activeSkill.activeEffect.grantedEffect.name:match("Absolution") and ...)
                   or (activeSkill.activeEffect.grantedEffect.name:match("Dominating Blow") and ...)
                   or (activeSkill.activeEffect.grantedEffect.name:match("Holy Strike") and ...) then
                    activeSkillCount = 1    -- force count = 1 to avoid double-counting
                end
            end
```

> **`DecayDPS` and `TotalDot` for minions are NOT scaled by `activeSkillCount`.** This is
> a difference from poison and impale (which are). The intent: DoT is assumed to not stack
> from multiple copies of the same minion skill. In Rust, do NOT multiply these by count.

> **Absolution / Dominating Blow / Holy Strike count override:** These three skills use a
> "Condition:XxxSkillDamageCountedOnce" flag in `env.modDB`. When that flag is set, the
> game treats the skill as if there's only one source of damage (the buff/exert effect
> ensures the damage is already multiplied by count internally). The override sets
> `activeSkillCount = 1` to avoid double-counting. In Rust, check
> `mod_db.flag(false, "Condition:AbsolutionSkillDamageCountedOnce")` etc.

#### 3b. Mirage branch (lines 248–285)

```lua
            if activeSkill.mirage then
                local mirageCount = (activeSkill.mirage.count or 1) * activeSkillCount
                if activeSkill.mirage.output.TotalDPS and activeSkill.mirage.output.TotalDPS > 0 then
                    fullDPS.combinedDPS = fullDPS.combinedDPS + activeSkill.mirage.output.TotalDPS * mirageCount
                end
                -- mirage bleed / ignite: non-stackable max
                if activeSkill.mirage.output.BleedDPS and activeSkill.mirage.output.BleedDPS > fullDPS.bleedDPS then
                    fullDPS.bleedDPS = activeSkill.mirage.output.BleedDPS
                end
                -- mirage TotalDot: stackable only if DotCanStack flag is set
                if activeSkill.mirage.output.TotalDot and activeSkill.mirage.output.TotalDot > 0
                   and (activeSkill.skillFlags.DotCanStack
                        or (usedEnv.player.output.TotalDot and usedEnv.player.output.TotalDot == 0)) then
                    fullDPS.dotDPS = fullDPS.dotDPS + activeSkill.mirage.output.TotalDot
                        * (activeSkill.skillFlags.DotCanStack and mirageCount or 1)
                end
                -- BurningGroundDPS / CausticGroundDPS: max semantics
                if activeSkill.mirage.output.BurningGroundDPS
                   and activeSkill.mirage.output.BurningGroundDPS > fullDPS.burningGroundDPS then
                    fullDPS.burningGroundDPS = activeSkill.mirage.output.BurningGroundDPS
                end
                if activeSkill.mirage.output.CausticGroundDPS
                   and activeSkill.mirage.output.CausticGroundDPS > fullDPS.causticGroundDPS then
                    fullDPS.causticGroundDPS = activeSkill.mirage.output.CausticGroundDPS
                end
            end
```

> **`mirageCount = (activeSkill.mirage.count or 1) * activeSkillCount`** — mirage count
> multiplied by gem count. `or 1` is Lua nil-coalesce: `mirage.count` is set in the
> CalcMirages handler; if absent for any reason, default to 1.

> **`DotCanStack` flag for mirage `TotalDot`:** Two conditions allow the mirage's TotalDot
> to stack: (a) `skillFlags.DotCanStack` (skills whose DoT explicitly stacks, like
> Caustic Arrow), or (b) the player has no TotalDot of their own (`output.TotalDot == 0`).
> If neither is true, the mirage's DoT is ignored (the player's instance is the only one).
> The scaling factor: if `DotCanStack`, scale by `mirageCount`; otherwise scale by 1.

#### 3c. Player hit DPS (lines 287–325)

```lua
            if usedEnv.player.output.TotalDPS and usedEnv.player.output.TotalDPS > 0 then
                fullDPS.combinedDPS = fullDPS.combinedDPS + usedEnv.player.output.TotalDPS * activeSkillCount
            end
            -- Non-stackable: max
            if usedEnv.player.output.BleedDPS and usedEnv.player.output.BleedDPS > fullDPS.bleedDPS then
                fullDPS.bleedDPS = usedEnv.player.output.BleedDPS
            end
            if usedEnv.player.output.CorruptingBloodDPS and usedEnv.player.output.CorruptingBloodDPS > fullDPS.corruptingBloodDPS then
                fullDPS.corruptingBloodDPS = usedEnv.player.output.CorruptingBloodDPS
            end
            if usedEnv.player.output.IgniteDPS and usedEnv.player.output.IgniteDPS > fullDPS.igniteDPS then
                fullDPS.igniteDPS = usedEnv.player.output.IgniteDPS
            end
            if usedEnv.player.output.BurningGroundDPS and usedEnv.player.output.BurningGroundDPS > fullDPS.burningGroundDPS then
                fullDPS.burningGroundDPS = usedEnv.player.output.BurningGroundDPS
            end
            -- Stackable: sum × count
            if usedEnv.player.output.PoisonDPS and usedEnv.player.output.PoisonDPS > 0 then
                fullDPS.TotalPoisonDPS = fullDPS.TotalPoisonDPS + usedEnv.player.output.TotalPoisonDPS * activeSkillCount
            end
            if usedEnv.player.output.CausticGroundDPS and usedEnv.player.output.CausticGroundDPS > fullDPS.causticGroundDPS then
                fullDPS.causticGroundDPS = usedEnv.player.output.CausticGroundDPS
            end
            if usedEnv.player.output.ImpaleDPS and usedEnv.player.output.ImpaleDPS > 0 then
                fullDPS.impaleDPS = fullDPS.impaleDPS + usedEnv.player.output.ImpaleDPS * activeSkillCount
            end
            if usedEnv.player.output.DecayDPS and usedEnv.player.output.DecayDPS > 0 then
                fullDPS.decayDPS = fullDPS.decayDPS + usedEnv.player.output.DecayDPS
                -- NOTE: no × activeSkillCount — decay does not scale with gem count
            end
            if usedEnv.player.output.TotalDot and usedEnv.player.output.TotalDot > 0 then
                fullDPS.dotDPS = fullDPS.dotDPS + usedEnv.player.output.TotalDot
                    * (activeSkill.skillFlags.DotCanStack and activeSkillCount or 1)
            end
            -- Culling: max
            if usedEnv.player.output.CullMultiplier and usedEnv.player.output.CullMultiplier > 1
               and usedEnv.player.output.CullMultiplier > fullDPS.cullingMulti then
                fullDPS.cullingMulti = usedEnv.player.output.CullMultiplier
            end
```

> **`DecayDPS` for player is NOT scaled by `activeSkillCount`** (same as minion
> `DecayDPS`). Decay is a ground-effect DoT; multiple copies don't meaningfully stack.

> **`TotalDot` (player):** scales by `activeSkillCount` only if `DotCanStack` is set
> (e.g. Caustic Arrow). For non-stacking DoT skills (e.g. ignite sources with `TotalDot`),
> `activeSkillCount` is ignored — only 1 instance is counted.

> **`CausticGroundDPS`:** takes the max across player skills AND mirage output (checked in
> both the mirage block and the player block). It is NOT scaled by count.

---

### 4. Post-loop: ailment assembly and final `TotalDotDPS` (lines 341–388)

```lua
    -- Re-Add ailment DPS components
    fullDPS.TotalDotDPS = 0

    if fullDPS.bleedDPS > 0 then
        fullDPS.TotalDotDPS = fullDPS.TotalDotDPS + fullDPS.bleedDPS
    end
    if fullDPS.corruptingBloodDPS > 0 then
        fullDPS.TotalDotDPS = fullDPS.TotalDotDPS + fullDPS.corruptingBloodDPS
    end
    if fullDPS.igniteDPS > 0 then
        fullDPS.TotalDotDPS = fullDPS.TotalDotDPS + fullDPS.igniteDPS
    end
    if fullDPS.burningGroundDPS > 0 then
        fullDPS.TotalDotDPS = fullDPS.TotalDotDPS + fullDPS.burningGroundDPS
    end
    if fullDPS.TotalPoisonDPS > 0 then
        fullDPS.TotalPoisonDPS = m_min(fullDPS.TotalPoisonDPS, data.misc.DotDpsCap)  -- cap at DotDpsCap
        fullDPS.TotalDotDPS = fullDPS.TotalDotDPS + fullDPS.TotalPoisonDPS
    end
    if fullDPS.causticGroundDPS > 0 then
        fullDPS.TotalDotDPS = fullDPS.TotalDotDPS + fullDPS.causticGroundDPS
    end
    if fullDPS.impaleDPS > 0 then
        -- Impale goes into combinedDPS (hit), NOT TotalDotDPS
        fullDPS.combinedDPS = fullDPS.combinedDPS + fullDPS.impaleDPS
    end
    if fullDPS.decayDPS > 0 then
        fullDPS.TotalDotDPS = fullDPS.TotalDotDPS + fullDPS.decayDPS
    end
    if fullDPS.dotDPS > 0 then
        fullDPS.TotalDotDPS = fullDPS.TotalDotDPS + fullDPS.dotDPS
    end

    -- Cap total DoT DPS
    fullDPS.TotalDotDPS = m_min(fullDPS.TotalDotDPS, data.misc.DotDpsCap)

    -- Combine hit + all ailment DPS for the grand total
    fullDPS.combinedDPS = fullDPS.combinedDPS + fullDPS.TotalDotDPS

    -- Culling multiplier bonus on top
    if fullDPS.cullingMulti > 0 then
        fullDPS.cullingDPS = fullDPS.combinedDPS * (fullDPS.cullingMulti - 1)
        fullDPS.combinedDPS = fullDPS.combinedDPS + fullDPS.cullingDPS
    end

    return fullDPS
```

> **`data.misc.DotDpsCap`** — PoB's cap for total DoT DPS (currently 35,000,000 in the
> PoB data files). Applied twice: once to `TotalPoisonDPS` alone, then again to the
> entire `TotalDotDPS`. In Rust: read from `game_data.misc.dot_dps_cap`.

> **Impale goes into `combinedDPS`, NOT `TotalDotDPS`.** Impale is a hit-damage
> augmentation, not a true DoT, so it adds to the hit total. `FullDotDPS` = `TotalDotDPS`
> which does not include impale.

> **`fullDPS.cullingMulti - 1`** — the cull contribution is the *extra* DPS from killing
> culled enemies below the threshold (default 10% life). If `cullingMulti = 1.1` (10%
> culling bonus), then `cullingDPS = combinedDPS × 0.1`. The `combinedDPS` already
> includes ailments at this point, so culling scales the entire damage output.

> **Final formula:**
> ```
> FullDotDPS  = bleedDPS + corruptingBloodDPS + igniteDPS + burningGroundDPS
>             + TotalPoisonDPS (capped) + causticGroundDPS + decayDPS + dotDPS
>             (capped at DotDpsCap)
>
> FullDPS     = (TotalDPS × count) summed over all enabled "FullDPS" skills
>             + (minion TotalDPS × count) for skills with minions
>             + (mirage TotalDPS × mirageCount) for skills with mirages
>             + impaleDPS (summed)
>             + FullDotDPS
>             + culling bonus = combinedDPS × (cullingMulti - 1)
> ```

---

### 5. `includeInFullDPS` check

```lua
    if activeSkill.socketGroup and activeSkill.socketGroup.includeInFullDPS then
```

> **`includeInFullDPS`** is a boolean set on the socket group (the gem slot), not on the
> skill itself. In the PoB UI, the user ticks a checkbox on each skill slot to include it
> in the Full DPS calculation. Skills that are not ticked are excluded from aggregation.
> In Rust: this corresponds to `active_skill.socket_group.include_in_full_dps`. This flag
> must be correctly propagated from the build XML for AGG-01 to work.

> **Skills without a `socketGroup`** (internally-constructed skills, e.g. triggered
> effects built in code) will have `activeSkill.socketGroup = nil`. The outer condition
> `if activeSkill.socketGroup and ...` safely skips them. Rust: `if let Some(sg) = &active_skill.socket_group`.

---

### 6. Environment re-initialisation inside the loop (lines 328–336)

```lua
            -- Re-Build env calculator for new run
            local accelerationTbl = {
                nodeAlloc      = true,
                requirementsItems = true,
                requirementsGems  = true,
                skills         = true,
                everything     = true,
            }
            fullEnv, _, _, _ = calcs.initEnv(build, mode, override,
                { cachedPlayerDB = cachedPlayerDB, cachedEnemyDB = cachedEnemyDB,
                  cachedMinionDB = cachedMinionDB, env = fullEnv,
                  accelerate = accelerationTbl })
```

> **Why re-initialise?** `calcs.perform` modifies `fullEnv` in-place (adds conditions,
> sets `mainSkill`, etc.). Before computing the next skill, PoB needs a clean environment.
> The `accelerate` table tells `initEnv` to skip expensive setup steps (node allocation,
> item requirement parsing, etc.) that don't change between skills. In Rust, the equivalent
> is cloning or resetting the environment before each skill's pass.

---

## Existing Rust Code

File: `crates/pob-calc/src/calc/calcs.rs`, lines 1–119

### What exists

`calc_full_dps(env: &mut CalcEnv)` (lines 8–48):
- Reads `CombinedDPS` from the player's current output.
- Reads `MirageDPS` from the player's current output.
- **Falls back** to `TotalDPS + MirageDPS` when `CombinedDPS` is 0.
- Writes `FullDPS` (one of these two values).
- Reads `TotalDotDPS` and writes it as `FullDotDPS`.
- Passes through `WithPoisonDPS`, `WithIgniteDPS`, `WithBleedDPS` into `FullWith*DPS`
  variants (these fields do not exist in PoB's oracle output — see note below).

Unit tests cover the fallback logic and pass-through.

### What's wrong / missing

1. **Structural mismatch.** The Rust `calc_full_dps` is a single-pass function that reads
   already-computed per-skill output fields. The Lua `calcs.calcFullDPS` is a *loop* that
   re-runs `calcs.perform` for each "Include in Full DPS" skill. The Rust does not
   implement multi-skill aggregation at all.

2. **`FullDPS = CombinedDPS` is wrong for multi-skill builds.** For a build with two
   skills both ticked in "Include in Full DPS", `FullDPS` should be the sum of both
   skills' `TotalDPS` plus their ailment contributions. The Rust reads only the main
   skill's `CombinedDPS`. This means for multi-skill builds, `FullDPS` is undercount.
   For single-skill builds, it may be approximately correct if `CombinedDPS` is correct.

3. **`FullDotDPS` = `TotalDotDPS` (single skill) vs aggregate.** In Lua, `FullDotDPS` is
   the sum of the best ignite, best bleed, total poison, etc. across ALL included skills.
   The Rust reads only the main skill's `TotalDotDPS` (a per-skill combined DoT figure).
   This is structurally wrong but may produce correct values for single-skill builds.

4. **Impale not added to `FullDPS`.** The Lua explicitly adds `fullDPS.impaleDPS` into
   `fullDPS.combinedDPS` at line 369. The Rust `CombinedDPS` (OFF-07) already includes
   `ImpaleDPS` within a single skill, so this may be a wash for single-skill builds —
   but for multi-skill builds, impale from secondary skills is not aggregated.

5. **`DotDpsCap` not applied.** The Lua caps `TotalPoisonDPS` and `TotalDotDPS` at
   `data.misc.DotDpsCap` (35,000,000). The Rust does not apply this cap.

6. **Culling multiplier not applied.** The Lua adds `combinedDPS × (cullingMulti - 1)` to
   `combinedDPS` when `cullingMulti > 1`. The Rust does not track or apply this.

7. **`FullWithPoisonDPS` / `FullWithIgniteDPS` / `FullWithBleedDPS` are not PoB fields.**
   These do not appear in the oracle expected output and are not written by PoB's Lua.
   They can be removed from the Rust to reduce noise.

8. **`MirageDPS` handled incorrectly.** The fallback `TotalDPS + MirageDPS` is not the
   Lua logic. In Lua, mirage DPS is included in `combinedDPS` because CalcOffence writes
   `output.CombinedDPS = output.CombinedDPS + mirage.output.TotalDPS × mirageCount`
   (CalcOffence.lua:5907, see MIR-01). The `calcFullDPS` function also separately adds
   `activeSkill.mirage.output.TotalDPS × mirageCount` to `fullDPS.combinedDPS` in the
   mirage branch. There is no separate `FullDPS = TotalDPS + MirageDPS` fallback.

## What Needs to Change

1. **Replace single-pass aggregation with multi-skill loop.** `calc_full_dps` must iterate
   over `env.player.active_skills` where `socket_group.include_in_full_dps == true`,
   re-run `calc_perform` for each skill, and accumulate:
   - Hit DPS: `total_dps × active_skill_count` (player and minion separately)
   - Impale: `impale_dps × active_skill_count` (added to `combined_dps` not `dot_dps`)
   - Non-stackable ailments (`bleed_dps`, `ignite_dps`, `burning_ground_dps`,
     `corrupting_blood_dps`, `caustic_ground_dps`): take the max across all skills.
   - Stackable ailments (`total_poison_dps × count`, `decay_dps`, `total_dot × count`):
     sum across all skills.
   - Culling: take the max `cull_multiplier` across all skills.

2. **Apply `DotDpsCap` to `TotalPoisonDPS` before adding to `TotalDotDPS`, and cap
   `TotalDotDPS` after all components are summed.** Read cap from `game_data.misc.dot_dps_cap`.

3. **Add culling bonus.** After all skills are processed:
   ```rust
   if full_dps.culling_multi > 0.0 {
       let culling_dps = full_dps.combined_dps * (full_dps.culling_multi - 1.0);
       full_dps.combined_dps += culling_dps;
   }
   ```

4. **Remove non-existent output fields.** `FullWithPoisonDPS`, `FullWithIgniteDPS`,
   `FullWithBleedDPS` are not PoB oracle fields. Remove these writes.

5. **Fix mirage handling in the loop.** Inside the loop, check `active_skill.mirage` and
   accumulate mirage `TotalDPS × mirage_count × active_skill_count` into `combined_dps`.
   Non-stackable mirage ailments use max semantics; stackable mirage ailments are summed.
   (See MIR-01 for the full mirage output structure — this depends on MIR-01 being
   correct first.)

6. **Handle the `DotCanStack` flag for `TotalDot` scaling.** When accumulating player or
   mirage `TotalDot`, scale by `active_skill_count` only if `skill_flags.dot_can_stack`
   is set; otherwise use count = 1.

7. **Respect the Absolution / Dominating Blow / Holy Strike count override.** If any of
   the three flags `Condition:AbsolutionSkillDamageCountedOnce`,
   `Condition:DominatingBlowSkillDamageCountedOnce`, or `Condition:HolyStrikeSkillDamageCountedOnce`
   is set in `mod_db`, force `active_skill_count = 1` for that skill's hit DPS accumulation.

8. **Single-skill builds are a degenerate case of multi-skill.** The correct
   implementation subsumes the current implementation — a build with exactly one included
   skill produces the same result as the current code (modulo the missing cap and culling).
   No special-casing needed.
