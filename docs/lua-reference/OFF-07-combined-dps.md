# OFF-07-combined-dps: Combined DPS, With* Fields, Culling, and FullDPS

## Output Fields

| Field | Lua source | Notes |
|-------|-----------|-------|
| `CombinedDPS` | CalcOffence.lua:5804,5868,5946,5952 | Aggregated DPS from all sources × cull × reservation; distinct from `CombinedAvg` |
| `CombinedAvg` | CalcOffence.lua:5805,5813,5821,5827,5840,5859 | Average-damage aggregate (for `showAverage` skills); does NOT include DPS-mode ailments |
| `WithBleedDPS` | CalcOffence.lua:5839–5845 | `baseDPS + BleedDPS` (or `BleedDamage` in showAverage mode) |
| `WithIgniteDPS` | CalcOffence.lua:5823–5832 | `baseDPS + IgniteDPS` (or `IgniteDamage` / `TotalIgniteDPS` depending on mode/flags) |
| `WithPoisonDPS` | CalcOffence.lua:5814–5816 | `baseDPS + TotalPoisonDPS` (or `TotalPoisonAverageDamage` in showAverage) |
| `CullingMultiplier` | *(not a real field — see note)* | `field_groups.rs` entry is wrong; oracle uses `CullMultiplier` |
| `FullDPS` | Calcs.lua:81,107,131,143,429 | Cross-skill aggregate; set by `calcFullDPS()` after all skills are processed |
| `FullDotDPS` | Calcs.lua:82,108,132,144,430 | Cross-skill DoT aggregate; set by `calcFullDPS()` |
| `WithDotDPS` | CalcOffence.lua:5807 | `baseDPS + TotalDot`; only written when `skillFlags.dot` is true |

> **`CullingMultiplier` note:** The oracle JSON and PoB Lua use `CullMultiplier` (not
> `CullingMultiplier`). The `field_groups.rs` entry is a typo. Verify against oracle JSON
> (all impale/melee oracle files confirm `CullMultiplier: 1`) and rename the entry to
> `CullMultiplier`.

> **`FullDPS` / `FullDotDPS` scope:** These are **not** computed by `calcs.offence()` (the
> per-skill CalcOffence.lua module). They are computed by `calcs.calcFullDPS()` in `Calcs.lua`,
> which iterates over ALL active skills flagged with `includeInFullDPS`. In the oracle test
> harness, only the main skill is calculated, so `FullDPS = 0` and `FullDotDPS = 0` in all
> current oracle files. The Rust `calc_full_dps()` in `calcs.rs` currently returns `CombinedDPS`
> as `FullDPS` instead, which differs from PoB's cross-skill aggregation semantics.

## Dependencies

- `OFF-04-speed-dps` — `TotalDPS`, `AverageDamage`, `TotalDot` must be correct.
- `OFF-05-ailments` — `IgniteDPS`, `IgniteDamage`, `TotalIgniteDPS`, `BleedDPS`,
  `BleedDamage`, `TotalPoisonDPS`, `TotalPoisonAverageDamage` must be set.
- `OFF-06-dot-impale` — `ImpaleDPS` and `TotalDot` (final form) must be set.
- `OFF-03-crit-hit` — `CullPercent` / `CullMultiplier` are computed during the crit section
  (CalcOffence.lua:3047–3054).

## Lua Source

Files:
- `third-party/PathOfBuilding/src/Modules/CalcOffence.lua` (per-skill combinaton)
- `third-party/PathOfBuilding/src/Modules/Calcs.lua` (cross-skill FullDPS)

Commit: `454eff8c85d24356d9b051d596983745ed367476`

Primary line ranges (CalcOffence.lua):
- **Skill disabled guard:** line 337 (`output.CombinedDPS = 0; return`)
- **`CullPercent` and `CullMultiplier`:** lines 3047–3054 (inside the crit/damage pass)
- **`CombinedDPS` init + `With*` fields:** lines 5803–5846
- **Impale contribution to `CombinedDPS`/`CombinedAvg`:** lines 5847–5868
- **Mirage contributions:** lines 5902–5938
- **`TotalDotDPS` aggregation:** lines 5940–5947
- **Final cull/reservation multiplication:** lines 5949–5952

Primary line ranges (Calcs.lua):
- **`calcFullDPS` function:** lines 176–388
- **`FullDPS`/`FullDotDPS` writes:** lines 79–83, 107–108, 131–132, 143–144, 429–430

## Annotated Lua

### 1. Cull computation (CalcOffence.lua:3047–3054)

`CullPercent` and `CullMultiplier` are computed **inside the per-pass damage loop**, not at
the final combination stage:

```lua
-- Lines 3047–3054 (inside: for _, pass in ipairs(passList))
local criticalCull = skillModList:Max(cfg, "CriticalCullPercent") or 0
if criticalCull > 0 then
    -- Expected fraction of hits that are crits × criticalCull; min(A, A * crit_uptime)
    criticalCull = m_min(criticalCull, criticalCull * (1 - (1 - output.CritChance / 100) ^ hitRate))
end
local regularCull = skillModList:Max(cfg, "CullPercent") or 0
local maxCullPercent = m_max(criticalCull, regularCull)
globalOutput.CullPercent = maxCullPercent
-- CullMultiplier: if CullPercent = 20, we deal 1/(1-0.2) = 1.25x effective DPS
globalOutput.CullMultiplier = 100 / (100 - globalOutput.CullPercent)
```

> **`skillModList:Max(cfg, "CullPercent")`** — note the use of `Max` not `Sum`. Culling
> takes the maximum source (you can only cull once at the highest threshold), not the sum.
> Rust: `mod_db.max_cfg("CullPercent", cfg, output)`.

> **`(1 - p)^hitRate` formula:** `criticalCull` is scaled by the probability that the
> enemy will be hit at least once with a crit during the cull window. For the oracle
> builds seen so far all have `CullPercent = 0`, so this formula doesn't affect results.

---

### 2. `baseDPS` selection and initial writes (CalcOffence.lua:5803–5808)

```lua
-- Line 5803: showAverage skills use AverageDamage as their DPS base
local baseDPS = output[(skillData.showAverage and "AverageDamage") or "TotalDPS"]
output.CombinedDPS = baseDPS   -- start from hit DPS
output.CombinedAvg = baseDPS   -- tracks "average" contributions (non-DPS ailments)

if skillFlags.dot then
    -- WithDotDPS: hit DPS + non-ailment sustained DoT
    -- Only written when the skill actually has a DoT component (not for pure hit skills)
    output.WithDotDPS = baseDPS + (output.TotalDot or 0)
end
```

> **`output.TotalDot or 0`** — Lua nil-coalesce. `TotalDot` may not exist if no DoT was
> computed. Rust: `get_output_f64(output, "TotalDot")` (returns 0.0 on missing).

> **`WithDotDPS` is conditionally written.** It is only written when `skillFlags.dot` is set.
> The Rust currently writes it unconditionally (`total_dps + total_dot_dps`), which means
> `WithDotDPS` appears in oracle output even for skills with no DoT. The oracle confirms
> `WithDotDPS` is absent for pure-hit builds.

---

### 3. `TotalPoisonDPS` quantity multiplier (lines 5809–5811)

```lua
-- Quantity multiplier applied to poison after CombinedDPS init
-- (This was missed in the per-pass loop — applied here as a post-correction)
if quantityMultiplier > 1 and output.TotalPoisonDPS then
    output.TotalPoisonDPS = m_min(output.TotalPoisonDPS * quantityMultiplier, data.misc.DotDpsCap)
end
```

> **Post-pass quantity correction:** The `quantityMultiplier` is applied here to `TotalPoisonDPS`,
> after the fact. This is a design artifact — the poison calculation in the ailment loop uses
> `quantityMultiplier` for stack count (line 4426) but not for the DPS multiplication. Rust:
> replicate this correction immediately after the `CombinedDPS` init.

---

### 4. `With*` ailment fields (lines 5812–5845)

These are written with `showAverage`-aware branching:

```lua
-- Poison:
if skillData.showAverage then
    output.CombinedAvg = output.CombinedAvg + (output.TotalPoisonAverageDamage or 0)
    output.WithPoisonDPS = baseDPS + (output.TotalPoisonAverageDamage or 0)
else
    output.WithPoisonDPS = baseDPS + (output.TotalPoisonDPS or 0)
end

-- Ignite — three-way branching:
if skillFlags.ignite then
    if skillFlags.igniteCanStack then  -- Emberwake / multi-ignite builds
        if skillData.showAverage then
            output.CombinedAvg = output.CombinedDPS + output.IgniteDamage
            -- NOTE: WithIgniteDPS NOT written in this branch! (intentional gap)
        else
            output.WithIgniteDPS = baseDPS + output.TotalIgniteDPS  -- sum of all ignite stacks DPS
        end
    elseif skillData.showAverage then  -- standard ignite, showAverage mode
        output.WithIgniteDPS = baseDPS + output.IgniteDamage  -- damage per ignite (not DPS)
        output.CombinedAvg = output.CombinedAvg + output.IgniteDamage
    else  -- standard ignite, DPS mode
        output.WithIgniteDPS = baseDPS + output.IgniteDPS  -- single ignite DPS
    end
else
    output.WithIgniteDPS = baseDPS  -- always set, even when no ignite
end

-- Bleed:
if skillFlags.bleed then
    if skillData.showAverage then
        output.WithBleedDPS = baseDPS + output.BleedDamage
        output.CombinedAvg = output.CombinedAvg + output.BleedDamage
    else
        output.WithBleedDPS = baseDPS + output.BleedDPS
    end
else
    output.WithBleedDPS = baseDPS  -- always set, even when no bleed
end
```

> **`WithIgniteDPS` vs `TotalIgniteDPS`:** For single-stack ignite (normal), `WithIgniteDPS =
> baseDPS + IgniteDPS`. For `igniteCanStack` builds (Emberwake), `WithIgniteDPS = baseDPS +
> TotalIgniteDPS` (total across all active stacks). `IgniteDPS` = single stack. The Rust
> always uses `IgniteDPS` which is wrong for Emberwake.

> **`WithIgniteDPS` not written in `igniteCanStack + showAverage` branch.** In that corner
> case (igniteCanStack AND showAverage), only `CombinedAvg` is updated; `WithIgniteDPS` is
> left unset. The Rust writes it unconditionally.

> **`WithBleedDPS` and `WithIgniteDPS` are always written** (even to `baseDPS` when the
> respective ailment is not active). This ensures the oracle always has these fields. The
> Rust currently writes them unconditionally — this is correct behaviour, just the values
> need the mode-aware branching.

---

### 5. `CombinedAvg` — the show-average aggregate

`CombinedAvg` is only meaningfully different from `CombinedDPS` for skills with
`skillData.showAverage = true` (e.g. Glacial Cascade, Tornado Shot, skills that show
"average damage" rather than DPS):

- For DPS-mode skills: `CombinedAvg` = `baseDPS` (no ailment damage added)
- For showAverage skills: `CombinedAvg` = `AverageDamage + BleedDamage + IgniteDamage + TotalPoisonAverageDamage + ImpaleDPS`

This distinction is visible in the bleed gladiator oracle: `CombinedAvg = 192` (just hit
average) while `CombinedDPS = 31805` (hit + bleed DPS).

The Rust does not implement `CombinedAvg` at all.

---

### 6. Mirage contributions (lines 5902–5938)

```lua
local bestCull = 1
if activeSkill.mirage and activeSkill.mirage.output and activeSkill.mirage.output.TotalDPS then
    local mirageCount = activeSkill.mirage.count or 1
    output.MirageDPS = activeSkill.mirage.output.TotalDPS * mirageCount
    output.CombinedDPS = output.CombinedDPS + activeSkill.mirage.output.TotalDPS * mirageCount

    -- Mirage ailments: non-stackable ailments use max of player vs mirage;
    -- stackable ones (poison, impale, TotalDot) add to CombinedDPS
    if activeSkill.mirage.output.IgniteDPS > (output.IgniteDPS or 0) then
        output.MirageDPS = output.MirageDPS + activeSkill.mirage.output.IgniteDPS
        output.IgniteDPS = 0   -- zero out player ignite so mirage ignite isn't double-counted
    end
    if activeSkill.mirage.output.BleedDPS > (output.BleedDPS or 0) then
        output.MirageDPS = output.MirageDPS + activeSkill.mirage.output.BleedDPS
        output.BleedDPS = 0    -- same pattern
    end
    if activeSkill.mirage.output.PoisonDPS then
        output.MirageDPS = output.MirageDPS + activeSkill.mirage.output.PoisonDPS * mirageCount
        output.CombinedDPS = output.CombinedDPS + activeSkill.mirage.output.PoisonDPS * mirageCount
    end
    if activeSkill.mirage.output.ImpaleDPS then
        output.MirageDPS = output.MirageDPS + activeSkill.mirage.output.ImpaleDPS * mirageCount
        output.CombinedDPS = output.CombinedDPS + activeSkill.mirage.output.ImpaleDPS * mirageCount
    end
    if activeSkill.mirage.output.TotalDot and (skillFlags.DotCanStack or not output.TotalDot or output.TotalDot == 0) then
        output.MirageDPS = output.MirageDPS + activeSkill.mirage.output.TotalDot
                           * (skillFlags.DotCanStack and mirageCount or 1)
        output.CombinedDPS = output.CombinedDPS + activeSkill.mirage.output.TotalDot
                              * (skillFlags.DotCanStack and mirageCount or 1)
    end
    if activeSkill.mirage.output.CullMultiplier > 1 then
        bestCull = activeSkill.mirage.output.CullMultiplier
    end
end
```

> **Non-stackable vs stackable mirage ailments:** Bleed and Ignite from mirages use the
> "best of player/mirage" logic — the higher value replaces the lower, the other is zeroed.
> Poison, Impale, and `TotalDot` are additive (scaled by `mirageCount` or 1). This ensures
> the later `TotalDotDPS` aggregation only counts each source once.

> **`bestCull` tracking:** The mirage's `CullMultiplier` can be used if it's better than the
> player's own culling strike. This is used in the final cull multiply at line 5949.

---

### 7. `TotalDotDPS` aggregation and final multiply (lines 5940–5952)

```lua
-- Line 5940: long nil-coalescing expression
local TotalDotDPS = (output.TotalDot or 0)
    + (output.TotalPoisonDPS or 0)
    + m_max(output.CausticGroundDPS or 0, output.MirageCausticGroundDPS or 0)
    + (output.TotalIgniteDPS or output.IgniteDPS or 0)  -- prefer TotalIgniteDPS (stacking builds)
    + m_max(output.BurningGroundDPS or 0, output.MirageBurningGroundDPS or 0)
    + (output.BleedDPS or 0)
    + (output.CorruptingBloodDPS or 0)
    + (output.DecayDPS or 0)
output.TotalDotDPS = m_min(TotalDotDPS, data.misc.DotDpsCap)

-- Non-showAverage: add all DoTs to CombinedDPS
if not skillData.showAverage then
    output.CombinedDPS = output.CombinedDPS + output.TotalDotDPS
end

-- Culling: bestCull is max(mirage cull, player cull multiplier)
bestCull = m_max(bestCull, output.CullMultiplier)
output.CullingDPS = output.CombinedDPS * (bestCull - 1)
output.ReservationDPS = output.CombinedDPS * (output.ReservationDpsMultiplier - 1)
-- Final: multiply CombinedDPS by cull and reservation multipliers
output.CombinedDPS = output.CombinedDPS * bestCull * output.ReservationDpsMultiplier
```

> **`TotalIgniteDPS or IgniteDPS`** — Lua multi-level nil coalesce. `TotalIgniteDPS` is only
> set on igniteCanStack builds (Emberwake). For normal builds, it falls through to `IgniteDPS`.
> Rust: `get_output_f64(output, "TotalIgniteDPS").max(get_output_f64(output, "IgniteDPS"))` — but
> careful: `TotalIgniteDPS` being 0 means "not set for this type of ignite", not that ignite
> DPS is zero. Use `output.get("TotalIgniteDPS").unwrap_or_else(|| output.get("IgniteDPS"))`.

> **`m_max` for ground effects:** Both caustic ground and burning ground take the max of
> player and mirage DPS (you can only stand in one ground at once).

> **`ReservationDpsMultiplier`:** This comes from line 3057:
> `globalOutput.ReservationDpsMultiplier = 100 / (100 - enemyDB:Sum("BASE", nil, "LifeReservationPercent"))`.
> It models enemy life reservation on the player (from certain skill effects). The Rust
> does not compute this. Default is 1.0 (no reservation effect).

> **`CombinedDPS × bestCull × ReservationDpsMultiplier`:** The final `CombinedDPS` includes
> culling and reservation multipliers. The `CullingDPS` field is the extra DPS from culling
> (`CombinedDPS × (cull - 1)`), and `ReservationDPS` is the extra DPS from reservation.
> Neither `CullingDPS` nor `ReservationDPS` are in OFF-07 fields, but they influence
> `CombinedDPS` which is.

---

### 8. `FullDPS` / `FullDotDPS` — cross-skill aggregation (Calcs.lua:176–388)

`FullDPS` and `FullDotDPS` are computed by `calcs.calcFullDPS()`, a separate function that:
1. Iterates ALL active skills flagged with `socketGroup.includeInFullDPS`
2. Re-runs `calcs.perform()` for each skill
3. Sums hit DPS across all skills (additive)
4. Takes the **best** single ignite and best single bleed (non-stackable)
5. Sums all poison, impale, decay, and TotalDot DPS (stackable)
6. Applies the highest culling multiplier found across all skills

```lua
-- Simplified sketch of the aggregation logic:
fullDPS.combinedDPS = 0  -- summed across skills
fullDPS.TotalDotDPS = 0  -- summed across ailments

for each skill with includeInFullDPS:
    fullDPS.combinedDPS += skill.TotalDPS × skillCount
    if skill.BleedDPS > fullDPS.bleedDPS then
        fullDPS.bleedDPS = skill.BleedDPS   -- best bleed
    end
    if skill.IgniteDPS > fullDPS.igniteDPS then
        fullDPS.igniteDPS = skill.IgniteDPS -- best ignite
    end
    fullDPS.TotalPoisonDPS += skill.TotalPoisonDPS × skillCount
    fullDPS.impaleDPS += skill.ImpaleDPS × skillCount

-- Aggregate:
fullDPS.TotalDotDPS += bleedDPS + igniteDPS + TotalPoisonDPS + ...
fullDPS.combinedDPS += impaleDPS           -- impale is hit-additive
fullDPS.combinedDPS += TotalDotDPS
fullDPS.combinedDPS *= cullingMulti

-- Write to output:
env.player.output.FullDPS = fullDPS.combinedDPS
env.player.output.FullDotDPS = fullDPS.TotalDotDPS
```

> **FullDPS is 0 in the oracle because `calcFullDPS` is never called in the Rust test
> harness.** The oracle runs `calcs.perform(env)` for a single skill, not
> `calcs.buildOutput(build, mode)`. This means all `FullDPS = 0` in oracle expected files
> is **correct expected behaviour** for single-skill calculation — `FullDPS` is only
> populated in the full multi-skill build display path.

---

## Existing Rust Code

### `crates/pob-calc/src/calc/offence_dot.rs` — `calc_combined_dps` (lines 154–195)

**What exists:**
- Reads `TotalDPS`, `IgniteDPS`, `BleedDPS`, `TotalPoisonDPS`, `TotalDot`, `DecayDPS`, `ImpaleDPS`.
- Computes `TotalDotDPS = ignite + bleed + poison + totalDot + decay`, writes it.
- Writes `WithIgniteDPS`, `WithBleedDPS`, `WithPoisonDPS`, `WithImpaleDPS`, `WithDotDPS` (all unconditionally).
- Computes `CullPercent` from mod query, derives `CullMultiplier = 100 / (100 - cull_pct)`.
- Computes `CombinedDPS = (TotalDPS + TotalDotDPS + ImpaleDPS) × CullMultiplier`, writes it.

**What's missing / wrong:**

1. **`CombinedAvg` not computed.** The `CombinedAvg` field is entirely absent from the Rust.
   For `showAverage` skills (Glacial Cascade, etc.) the oracle expects `CombinedAvg ≠ TotalDPS`.
   For non-showAverage skills `CombinedAvg = TotalDPS` (the oracle confirms this pattern).

2. **`WithDotDPS` written unconditionally.** Lua only writes it when `skillFlags.dot` is set.
   The Rust writes it as `total_dps + total_dot_dps` even for pure-hit skills where `TotalDot = 0`.
   The oracle shows `WithDotDPS` absent in many builds — the Rust would incorrectly write it as
   a duplicate of `CombinedDPS`.

3. **`showAverage` branching absent in `With*` fields.** For skills with `showAverage`:
   - `WithBleedDPS = baseDPS + BleedDamage` (not `BleedDPS`)
   - `WithIgniteDPS = baseDPS + IgniteDamage` (not `IgniteDPS`)
   - `WithPoisonDPS = baseDPS + TotalPoisonAverageDamage` (not `TotalPoisonDPS`)
   The Rust ignores `showAverage` entirely.

4. **`igniteCanStack` branching absent.** For Emberwake-style builds:
   - `WithIgniteDPS = baseDPS + TotalIgniteDPS` (not `IgniteDPS`)
   The Rust uses `IgniteDPS` unconditionally.

5. **`TotalIgniteDPS or IgniteDPS` selection not implemented.** `TotalDotDPS` aggregation
   should prefer `TotalIgniteDPS` (when set, for stacking ignite builds) over `IgniteDPS`.
   Rust: both are currently added together (`ignite_dps + total_ignite_dps` would double-count).

6. **Ground-effect DoTs not included in `TotalDotDPS`.** Lua aggregates:
   `TotalDotDPS += max(CausticGroundDPS, MirageCausticGroundDPS) + max(BurningGroundDPS, MirageBurningGroundDPS) + CorruptingBloodDPS`.
   The Rust includes only `IgniteDPS + BleedDPS + TotalPoisonDPS + TotalDot + DecayDPS`.

7. **`TotalPoisonDPS` quantity multiplier correction not applied.** Lua applies
   `TotalPoisonDPS × quantityMultiplier` at line 5809–5811 after the `CombinedDPS` init.
   The Rust doesn't have this post-correction.

8. **`ReservationDpsMultiplier` not applied to `CombinedDPS`.** Lua applies
   `output.CombinedDPS × bestCull × ReservationDpsMultiplier`. The Rust ignores the
   reservation multiplier. Default value 1.0 (from line 3057 setting `100 / (100 - 0) = 1`
   when no `LifeReservationPercent` is present), so this doesn't affect most builds.

9. **`CullingDPS` and `ReservationDPS` not written.** These derived fields
   (`CombinedDPS × (bestCull - 1)` and `CombinedDPS × (ReservationDpsMultiplier - 1)`)
   are absent. Neither is in OFF-07 fields, but this is a gap for completeness.

10. **Mirage ailment contributions not handled.** The mirage section (lines 5902–5938) is
    entirely absent from the Rust. Mirage builds would produce wrong `CombinedDPS` values.

11. **`bestCull` across player + mirage not tracked.** Lua uses
    `bestCull = max(mirage.CullMultiplier, output.CullMultiplier)`. The Rust uses only
    the player's `CullMultiplier`, ignoring mirage culling.

12. **`CullPercent` source.** Lua uses `skillModList:Max(cfg, "CullPercent")` (max, not sum)
    combined with the `CriticalCullPercent` calculation. The Rust uses
    `mod_db.sum_cfg(Base, "CullPercent", None, output)` (sum, not max). For builds with
    multiple culling sources, this produces different results.

### `crates/pob-calc/src/calc/calcs.rs` — `calc_full_dps` (lines 1–119)

**What exists:**
- Reads `CombinedDPS` and `MirageDPS`, passes through to `FullDPS`.
- Passes `TotalDotDPS` through to `FullDotDPS`.
- Emits `FullWithPoisonDPS`, `FullWithIgniteDPS`, `FullWithBleedDPS` (non-oracle fields).

**What's wrong:**
- `FullDPS` is set to `CombinedDPS` (current skill's combined DPS), not the cross-skill
  sum from `calcFullDPS`. For oracle tests this is OK (FullDPS = 0 expected), but in real
  usage it would be incorrect.
- `FullDotDPS` is passed through from `TotalDotDPS`, same concern.
- The cross-skill iteration logic from `Calcs.lua:calcFullDPS` (iterating `activeSkillList`,
  taking best-ignite/best-bleed, summing impale/poison) is entirely unimplemented.

## What Needs to Change

1. **Implement `CombinedAvg`.** Initialize to `baseDPS = AverageDamage if showAverage else TotalDPS`.
   Add `BleedDamage`, `IgniteDamage`, `TotalPoisonAverageDamage`, and `ImpaleDPS` (in showAverage
   mode) to `CombinedAvg`. Do not include these in `CombinedAvg` in non-showAverage mode.

2. **Write `WithDotDPS` only when the skill has a DoT.** Guard on `skillFlags.dot` (or
   equivalently, check `TotalDot > 0`). Remove unconditional write.

3. **Implement `showAverage` branching for `With*` fields:**
   - `WithBleedDPS`: use `BleedDamage` in showAverage mode, `BleedDPS` otherwise.
   - `WithIgniteDPS`: use `IgniteDamage` in showAverage (standard) / `TotalIgniteDPS`
     (igniteCanStack) mode, `IgniteDPS` in standard DPS mode.
   - `WithPoisonDPS`: use `TotalPoisonAverageDamage` in showAverage mode, `TotalPoisonDPS`
     otherwise.

4. **Implement `TotalIgniteDPS or IgniteDPS` selection in `TotalDotDPS` aggregation.** Use
   `TotalIgniteDPS` when it is set (> 0), fall back to `IgniteDPS`. Do not add both.

5. **Add ground-effect DoTs to `TotalDotDPS`:**
   ```rust
   total_dot_dps += max(caustic_ground_dps, mirage_caustic_ground_dps);
   total_dot_dps += max(burning_ground_dps, mirage_burning_ground_dps);
   total_dot_dps += corrupting_blood_dps;
   ```

6. **Apply `TotalPoisonDPS × quantityMultiplier` correction** after init, before
   `CombinedDPS` assembly, when `quantityMultiplier > 1`.

7. **Apply `ReservationDpsMultiplier` to `CombinedDPS`.**
   `combined_dps = combined_dps × cull_multiplier × reservation_dps_multiplier`.

8. **Fix `CullPercent` query to use `Max` not `Sum`.** Query `CullPercent` using the
   max-across-sources logic (separate `CriticalCullPercent` crit-uptime calculation):
   ```rust
   let crit_cull = mod_db.max_cfg("CriticalCullPercent", cfg, output)
                   .unwrap_or(0.0);
   // scale by crit uptime if > 0
   let regular_cull = mod_db.max_cfg("CullPercent", cfg, output)
                      .unwrap_or(0.0);
   let cull_pct = crit_cull.max(regular_cull);
   ```

9. **Implement mirage ailment contribution merging.** For non-stackable ailments (bleed,
   ignite): if mirage DPS > player DPS, use mirage and zero player. For stackable
   (poison, impale, TotalDot): add mirage × mirageCount to `CombinedDPS`. Track
   `bestCull = max(player CullMultiplier, mirage CullMultiplier)`.

10. **Write `CullingDPS` and `ReservationDPS`.**
    ```rust
    env.player.set_output("CullingDPS", combined_dps_before_final_multiply * (best_cull - 1.0));
    env.player.set_output("ReservationDPS", combined_dps_before_final_multiply * (resv_mult - 1.0));
    ```

11. **Rename `CullingMultiplier` to `CullMultiplier` in `field_groups.rs`.** The oracle JSON
    confirms `CullMultiplier` is the correct field name. `CullingMultiplier` does not exist.

12. **`FullDPS` / `FullDotDPS` semantics clarification.** For the oracle harness
    (single-skill), these correctly remain 0. The cross-skill `calcFullDPS` aggregation
    from `Calcs.lua` is a UI-level feature that is out of scope for the per-skill `calcs.offence()`
    calculation. Document this distinction clearly — `FullDPS` should remain 0 in the oracle,
    and `calc_full_dps()` in `calcs.rs` should not attempt to populate it from single-skill data.
