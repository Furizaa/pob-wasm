# OFF-04-speed-dps: Attack/Cast Speed, HitSpeed, TotalDPS, TotalDot, and AoE

## Output Fields

| Field | Lua source | Notes |
|-------|-----------|-------|
| `Speed` | CalcOffence.lua:2178–2338 | Cast/attack rate in uses-per-second; set per-pass, combined post-loop |
| `HitSpeed` | CalcOffence.lua:2376–2451 | Rate of actual hits when `hitTimeMultiplier` > 1 or `hitTimeOverride` set |
| `HitTime` | CalcOffence.lua:2375–2445 | Seconds between individual hits (inverse of `HitSpeed`) |
| `TotalDPS` | CalcOffence.lua:3533 | `AverageDamage × (HitSpeed or Speed) × dpsMultiplier × quantityMultiplier` |
| `TotalDot` | CalcOffence.lua:5568–5608 | Sustained DoT DPS from skill (stackable DoTs × duration × speed) |
| `MainHand.Speed` | via `combineStat("Speed", "HARMONICMEAN")` at line 2411 | Set inside per-pass loop, preserved in sub-table |
| `MainHand.HitSpeed` | via `combineStat("HitSpeed", "OR")` at line 2412 | Set inside per-pass loop when applicable |
| `OffHand.Speed` | via `combineStat("Speed", "HARMONICMEAN")` at line 2411 | |
| `OffHand.HitSpeed` | via `combineStat("HitSpeed", "OR")` at line 2412 | |
| `AreaOfEffectMod` | CalcOffence.lua:343 | `round(round(incArea × moreArea, 10), 2)` — area multiplier |
| `AreaOfEffectRadius` | CalcOffence.lua:357 | Integer radius in game units: `floor(baseRadius × floor(100 × sqrt(areaMod)) / 100)` |
| `AreaOfEffectRadiusMetres` | CalcOffence.lua:358 | `AreaOfEffectRadius / 10` |

## Dependencies

- `OFF-01-base-damage` — `AverageDamage` must be set before `TotalDPS` can be computed.
- `OFF-02-conversion` — hit damage must be final before `AverageDamage` is correct.
- `OFF-03-crit-hit` — `CritEffect` and `HitChance` feed into `AverageDamage`; cooldown and `dpsMultiplier` also rely on crit output ordering.
- `PERF-08-action-speed-conditions` — `ActionSpeedMod` (used to scale `Speed` for self-cast and totem skills) must be set.

## Lua Source

File: `third-party/PathOfBuilding/src/Modules/CalcOffence.lua`  
Commit: `454eff8c85d24356d9b051d596983745ed367476`

Primary line ranges:
- **`calcAreaOfEffect` helper:** lines 341–398 (called before pass loop and again in Fist of War path)
- **Speed calculation** (per-pass, inside `for _, pass in ipairs(passList)`): lines 2175–2373
- **HitSpeed/HitTime** (per-pass): lines 2374–2390
- **`dpsMultiplier` finalization:** lines 2392–2396
- **Post-pass speed combination** (`isAttack` block): lines 2407–2453
- **`TotalDPS` write:** line 3533 (inside the second pass loop, after damage accumulation)
- **`TotalDot` write:** lines 5568–5608 (after ailment loop, in DoT section)

## Annotated Lua

### 1. `calcAreaOfEffect` — AoE mod and radius (lines 341–398)

This is a **nested local function** defined at line 341 inside `calcs.offence()`, called early
(before the pass loop) and also from the Fist of War path.

```lua
local function calcAreaOfEffect(skillModList, skillCfg, skillData, skillFlags, output, breakdown)
    -- calcLib.mods returns (incMod, moreMod) as separate values
    -- incMod = 1 + Sum("INC", cfg, "AreaOfEffect", "AreaOfEffectPrimary") / 100
    -- moreMod = More(cfg, "AreaOfEffect", "AreaOfEffectPrimary")
    local incArea, moreArea = calcLib.mods(skillModList, skillCfg, "AreaOfEffect", "AreaOfEffectPrimary")

    -- Double-round: first to 10 decimal places (floating-point stabilisation),
    -- then to 2 decimal places (display precision).
    -- PoB's round(val, dec) = floor(val * 10^dec + 0.5) / 10^dec
    output.AreaOfEffectMod = round(round(incArea * moreArea, 10), 2)
```

> **`calcLib.mods`** returns `(inc, more)` where `inc = 1 + INC/100` and `more = product(More mods)`.
> Unlike `calcLib.mod` (single combined value), `calcLib.mods` returns them separately.
> Rust equivalent: call `mod_db.sum_cfg(Inc, ...)` and `mod_db.more_cfg(...)` separately,
> then form `inc = 1.0 + inc_sum / 100.0` and `more = more_product`.

> **Double-round pattern:** `round(round(x, 10), 2)` is idiomatic PoB for "round to a stable
> float value, then round to display precision". Rust: `(((x * 1e10).round() / 1e10) * 100.0).round() / 100.0`.
> This is **not** the same as a single `(x * 100.0).round() / 100.0` because the first round
> eliminates floating-point noise that could change the final digit.

```lua
    if skillData.radius then
        skillFlags.area = true
        -- baseRadius includes BASE "AreaOfEffect" mods (flat radius additions)
        local baseRadius = skillData.radius
                         + (skillData.radiusExtra or 0)
                         + skillModList:Sum("BASE", skillCfg, "AreaOfEffect")

        -- calcRadius: floor(baseRadius * floor(100 * sqrt(areaMod)) / 100)
        -- The inner floor(100 * sqrt(areaMod)) converts areaMod to an integer percentage,
        -- which is the in-game unit for AoE scaling before the outer divide-by-100.
        output.AreaOfEffectRadius = calcRadius(baseRadius, output.AreaOfEffectMod)
        output.AreaOfEffectRadiusMetres = output.AreaOfEffectRadius / 10
    end
end
```

> **`calcRadius` formula** (line 158–160):
> ```lua
> local function calcRadius(baseRadius, areaMod)
>     return m_floor(baseRadius * m_floor(100 * m_sqrt(areaMod)) / 100)
> end
> ```
> Rust: `(base * (100.0 * area_mod.sqrt()).floor() / 100.0).floor() as i64`.
> Note the **two separate floors**: one on the `100 × sqrt` intermediate, one on the product.
> A single floor at the end gives a different result.

> **`skillData.radius` nil guard:** `if skillData.radius then` — `AreaOfEffectRadius` and
> `AreaOfEffectRadiusMetres` are **only written when the skill has a defined radius**. Many
> skills (e.g. single-target attacks) have no `skillData.radius` and never write these fields.
> This is why some oracle builds have `AreaOfEffectMod` but not `AreaOfEffectRadius`.

---

### 2. Speed calculation (per-pass, lines 2175–2373)

All of this is **inside** `for _, pass in ipairs(passList) do`. `output` here is the
pass-local sub-table (e.g. `output.MainHand`).

**Branch 1: Instant-cast skills (castTime == 0)**

```lua
if activeSkill.activeEffect.grantedEffect.castTime == 0
   and not skillData.castTimeOverride
   and not skillData.triggered then
    output.Time = 0
    output.Speed = 0   -- skill fires instantly; no meaningful DPS rate
```

**Branch 2: Time/speed overrides**

```lua
elseif skillData.timeOverride then
    output.Time = skillData.timeOverride
    output.Speed = 1 / output.Time

elseif skillData.fixedCastTime then
    -- fixedCastTime ignores attack speed; uses gem's base cast time directly
    output.Time = activeSkill.activeEffect.grantedEffect.castTime
    output.Speed = 1 / output.Time

elseif skillData.triggerTime and skillData.triggered then
    -- Triggered by another skill at a fixed rate; may be shared among linked skills
    local activeSkillsLinked = skillModList:Sum("BASE", cfg, "ActiveSkillsLinkedToTrigger")
    if activeSkillsLinked > 0 then
        output.Time = skillData.triggerTime / (1 + INC_CooldownRecovery/100) * activeSkillsLinked
    else
        output.Time = skillData.triggerTime / (1 + INC_CooldownRecovery/100)
    end
    output.TriggerTime = output.Time
    output.Speed = 1 / output.Time

elseif skillData.triggerRate and skillData.triggered then
    output.Time = 1 / skillData.triggerRate
    output.Speed = skillData.triggerRate
    skillData.showAverage = false    -- triggers show average damage, not DPS
```

**Branch 3: Normal speed calculation (most skills)**

```lua
else
    local baseTime
    if isAttack then
        -- Weapon attack rate override for some skills (e.g. slams that override weapon speed)
        if skillData.attackSpeedMultiplier and source.AttackRate then
            source.AttackRate = source.AttackRate * (1 + skillData.attackSpeedMultiplier / 100)
        end

        if skillData.castTimeOverridesAttackTime then
            -- Skill uses gem cast time instead of weapon attack time,
            -- but still scales with weapon's AttackSpeedInc (e.g. baked-in weapon quality)
            baseTime = grantedEffect.castTime / (1 + (source.AttackSpeedInc or 0) / 100)

        elseif calcLib.mod(skillModList, skillCfg, "SkillAttackTime") > 0 then
            -- SkillAttackTime: scales total attack time rather than speed
            baseTime = (1 / (source.AttackRate or 1)
                        + skillModList:Sum("BASE", cfg, "Speed"))
                       * calcLib.mod(skillModList, skillCfg, "SkillAttackTime")
        else
            -- Normal path: base time = 1 / weapon APS + flat speed penalty
            baseTime = 1 / (source.AttackRate or 1) + skillModList:Sum("BASE", cfg, "Speed")
        end
    else
        -- Spell: base time is cast time from gem data (or override)
        baseTime = skillData.castTimeOverride
                   or activeSkill.activeEffect.grantedEffect.castTime
                   or 1
    end

    local more = skillModList:More(cfg, "Speed")  -- multiplicative speed modifiers
    output.Repeats = globalOutput.Repeats or 1

    -- ... Trauma stack calculation (omitted — not relevant to Speed output directly)

    local inc = skillModList:Sum("INC", cfg, "Speed")   -- additive speed modifiers

    if skillFlags.warcry then
        output.Speed = 1 / output.WarcryCastTime   -- warcries use their own time
    else
        -- Core speed formula:
        -- Speed = 1 / (baseTime / round((1+inc/100)*more, 2) + flat_time_additions)
        -- round(..., 2) is applied to the inc/more product before division —
        -- this is an important game rounding step that the Rust currently skips.
        output.Speed = 1 / (baseTime / round((1 + inc/100) * more, 2)
                            + skillModList:Sum("BASE", cfg, "TotalAttackTime")
                            + skillModList:Sum("BASE", cfg, "TotalCastTime"))
    end
    output.CastRate = output.Speed    -- raw cast rate before action speed scaling

    if skillFlags.selfCast then
        output.Speed = output.Speed * globalOutput.ActionSpeedMod
        output.CastRate = output.Speed
    end

    if skillFlags.totem then
        local totemActionSpeed = 1 + (modDB:Sum("INC", nil, "TotemActionSpeed") / 100)
        output.TotemActionSpeed = totemActionSpeed
        output.Speed = output.Speed * totemActionSpeed
        output.CastRate = output.Speed
        if skillData.totemFireOnce then
            output.HitTime = 1 / output.Speed + globalOutput.TotemPlacementTime
            output.HitSpeed = 1 / output.HitTime
        end
    end

    -- Cooldown cap
    if globalOutput.Cooldown then
        output.Cooldown = globalOutput.Cooldown
        -- Can't exceed cooldown-limited speed; Repeats allows multi-repeat skills to fire faster
        output.Speed = m_min(output.Speed, 1 / output.Cooldown * output.Repeats)
    end

    -- Server tick cap (non-channeling skills only)
    -- data.misc.ServerTickRate = 1 / 0.033 ≈ 30.30 actions/second
    if not activeSkill.skillTypes[SkillType.Channel] then
        output.Speed = m_min(output.Speed, data.misc.ServerTickRate * output.Repeats)
    end

    if output.Speed == 0 then
        output.Time = 0
    else
        output.Time = 1 / output.Speed
    end
end
```

> **`round((1 + inc/100) * more, 2)`** — the inc/more product is rounded to 2 decimal places
> **before** dividing into `baseTime`. This means e.g. `1 + 3% = 1.03`, `1.03 × 1.10 = 1.133 → 1.13`.
> The Rust applies `(1.0 + inc / 100.0) * more` without rounding, producing a slightly different
> base. This is a subtle game rounding step.

> **`source.AttackRate`** — for attack passes, `source` is the weapon data table (e.g.
> `actor.weaponData1`). `source.AttackRate` is the weapon's base attacks-per-second.
> `1 / source.AttackRate` converts to seconds per attack. In Rust: `1.0 / skill.attack_speed_base`.

> **`skillModList:Sum("BASE", cfg, "TotalAttackTime")` / `"TotalCastTime"`** — flat time
> additions (in seconds) that go **outside** the speed scaling, making the skill slower
> regardless of attack speed. Example: Resolute Technique-like penalties. The Rust does not
> query these.

> **`globalOutput.ActionSpeedMod`** — applied only for `selfCast` and `totem` skills.
> Attack skills get their action speed applied differently (at the weapon level or externally).
> The Rust currently applies `action_speed` to **all** attacks and spells unconditionally,
> which is wrong for triggered and non-selfCast spell cases.

---

### 3. HitSpeed and HitTime (per-pass, lines 2374–2390)

```lua
if skillData.hitTimeOverride and not skillData.triggeredOnDeath then
    -- Brand skills: fixed hit time regardless of cast speed
    output.HitTime = skillData.hitTimeOverride
    output.HitSpeed = 1 / output.HitTime

elseif skillData.hitTimeMultiplier and output.Time and not skillData.triggeredOnDeath then
    -- Channel skills: each stage fires at Time × multiplier intervals
    output.HitTime = output.Time * skillData.hitTimeMultiplier
    if output.Cooldown and skillData.triggered then
        output.HitSpeed = 1 / m_max(output.HitTime, output.Cooldown)  -- triggered cooldown
    elseif output.Cooldown then
        output.HitSpeed = 1 / (output.HitTime + output.Cooldown)       -- normal cooldown
    else
        output.HitSpeed = 1 / output.HitTime
    end
end
-- NOTE: if neither condition is true, HitSpeed and HitTime are NOT written.
-- After the attack combination block (isAttack), there is a second pass that applies the
-- same hitTimeOverride/hitTimeMultiplier logic to the combined output (lines 2441–2453),
-- this time with a server tick cap:
--   output.HitSpeed = m_min(1 / output.HitTime, data.misc.ServerTickRate)
```

> **`HitSpeed` vs `Speed`:** `Speed` is the skill use rate (how often you cast/attack).
> `HitSpeed` is the per-hit rate when a single use produces multiple hits staggered in time
> (e.g. channels, brands). When `HitSpeed` is set, `TotalDPS` uses it as the rate multiplier
> instead of `Speed` (see line 3533: `globalOutput.HitSpeed or globalOutput.Speed`).

---

### 4. Post-pass speed combination for attack skills (lines 2407–2453)

After the per-pass loop, when `isAttack`:

```lua
combineStat("Speed", "HARMONICMEAN")   -- harmonic mean of MH + OH speeds (dual-wield)
combineStat("HitSpeed", "OR")          -- use whichever hand has HitSpeed set
combineStat("HitTime", "OR")           -- same

if output.Speed == 0 then
    output.Time = 0
else
    output.Time = 1 / output.Speed
end

-- UseOffhandAttackSpeed mastery: force off-hand speed for the combined value
if skillModList:Flag(nil, "UseOffhandAttackSpeed") and not skillFlags.forceMainHand then
    output.Speed = output.OffHand.Speed
    output.Time = output.OffHand.Time
end

-- For dual-wielding, second hitTime pass with server tick cap:
-- output.HitSpeed = m_min(1 / output.HitTime, data.misc.ServerTickRate)
```

> **Harmonic mean of Speed:** For dual-wield skills alternating hands:
> `Speed = 2 / (1/MH_Speed + 1/OH_Speed)`. This correctly models the long-term average
> when both weapon speeds are different. The `combineStat("Speed", "HARMONICMEAN")` mode
> (line 1981–1986) implements this:
> ```lua
> output[stat] = 2 / ((1 / output.MainHand[stat]) + (1 / output.OffHand[stat]))
> ```
> Rust: `2.0 / (1.0 / mh_speed + 1.0 / oh_speed)`.

---

### 5. `dpsMultiplier` finalization (lines 2392–2396)

```lua
-- After speed calculation, before the damage pass loop:
skillData.dpsMultiplier = (skillData.dpsMultiplier or 1)
    * (1 + skillModList:Sum("INC", skillCfg, "DPS") / 100)
    * skillModList:More(skillCfg, "DPS")

-- FINAL/OnlyFinalRepeat mode: divide by repeat count
if env.configInput.repeatMode == "FINAL" or skillModList:Flag(nil, "OnlyFinalRepeat") then
    skillData.dpsMultiplier = skillData.dpsMultiplier / (output.Repeats or 1)
end
```

> **`dpsMultiplier`** starts at `skillData.dpsMultiplier` (from gem data, e.g. 1.0 for most
> skills, or less for channeling skills that fire multiple times per cast). It is then scaled
> by generic `DPS` INC/More mods. The Rust does not apply this multiplier at all.

---

### 6. `TotalDPS` write (line 3533, inside the per-pass damage loop)

```lua
output.TotalDPS = output.AverageDamage
    * (globalOutput.HitSpeed or globalOutput.Speed)  -- prefer HitSpeed if set
    * skillData.dpsMultiplier
    * quantityMultiplier
```

> **`globalOutput.HitSpeed or globalOutput.Speed`** — Lua nil-coalesce. `HitSpeed` is only
> set for channeling/brand skills; for normal skills `HitSpeed` is nil so `Speed` is used.
> In Rust: `output.get("HitSpeed").unwrap_or_else(|| output.get("Speed"))`.

> **`quantityMultiplier`** — from `skillModList:Sum("BASE", ..., "QuantityMultiplier")`,
> clamped to ≥ 1 (line 2485). Applied to TotalDPS for skills with built-in projectile/hit
> quantity bonuses (e.g. Arrow Nova Support at 4×). The Rust does not apply this.

For `isAttack` builds, `TotalDPS` is then recombined via:
```lua
combineStat("TotalDPS", "DPS")  -- line 3696
```
Which for dual-wield uses `"DPS"` mode (sum, then halve if not `doubleHitsWhenDualWielding`):
```lua
-- "DPS" mode from combineStat (line 2070–2075):
output[stat] = (output.MainHand[stat] or 0) + (output.OffHand[stat] or 0)
if not skillData.doubleHitsWhenDualWielding then
    output[stat] = output[stat] / 2
end
```

---

### 7. `TotalDot` write (lines 5558–5608)

`TotalDot` is the sustained DoT DPS from a skill that applies non-ailment DoT. It branches
on several flags:

```lua
if skillModList:Flag(nil, "DotCanStack") then
    -- Stackable DoT (e.g. Essence Drain, Vortex): each application adds to the total
    -- DPS = instance_damage × uses_per_second × duration × dpsMultiplier × quantityMult
    -- Capped at data.misc.DotDpsCap = (2^31 - 1) / 60 ≈ 35.8M DPS
    local speed = output.Speed   -- or MineLayingSpeed/TrapThrowingSpeed for mine/trap delivery
    output.TotalDot = m_min(
        output.TotalDotInstance * speed * output.Duration * skillData.dpsMultiplier * quantityMultiplier,
        data.misc.DotDpsCap)

elseif skillModList:Flag(nil, "dotIsBurningGround") then
    output.TotalDot = 0   -- burning ground handled separately as BurningGroundDPS

elseif skillModList:Flag(nil, "dotIsCausticGround") then
    output.TotalDot = 0   -- caustic ground handled separately as CausticGroundDPS

elseif skillModList:Flag(nil, "dotIsCorruptingBlood") then
    output.TotalDot = 0   -- corrupting blood handled separately

else
    -- Non-stackable DoT: only the strongest instance applies
    output.TotalDot = output.TotalDotInstance
end
```

> **`TotalDotInstance`** is computed in the DoT damage section (not this chunk's scope —
> covered by OFF-06). `TotalDot` is the DPS contribution from this DoT type as used in
> `TotalDotDPS` aggregation.

> **Mine/trap delivery speed override:** when the DoT is delivered via mine or trap,
> the relevant laying/throw speed is used instead of `output.Speed`:
> ```lua
> if band(dotCfg.keywordFlags, KeywordFlag.Mine) ~= 0 then
>     speed = output.MineLayingSpeed
> elseif band(dotCfg.keywordFlags, KeywordFlag.Trap) ~= 0 then
>     speed = output.TrapThrowingSpeed
> end
> ```
> `band` = `bit.band` (bitwise AND on integer flags). Rust: `dotCfg.keyword_flags & KeywordFlags::MINE != 0`.

---

## Existing Rust Code

File: `crates/pob-calc/src/calc/offence.rs`  
Lines: 153–202 (Speed), 359–365 (TotalDPS)

File: `crates/pob-calc/src/calc/offence_dot.rs`  
Lines: 145, 152–166 (TotalDot, TotalDotDPS)

### What exists

**Speed (lines 153–202):**
- `is_attack` branch: queries `INC Speed + AttackSpeed`, `More Speed × AttackSpeed`, computes `attack_speed_base × (1 + inc/100) × more × action_speed`.
- `is_spell` branch: queries `INC Speed + CastSpeed`, `More Speed × CastSpeed`, computes `(1/cast_time) × (1 + inc/100) × more × action_speed`.
- Writes `Speed`.

**TotalDPS (line 364–365):**
- `total_dps = average_damage * uses_per_sec`, writes `TotalDPS`.
- No `dpsMultiplier`, no `quantityMultiplier`, no `HitSpeed or Speed` selection.

**TotalDot (offence_dot.rs:145):**
- Computed by `calc_skill_dot` and written to `TotalDot`.
- The Rust only handles the simple non-stackable path (`TotalDot = TotalDotInstance`).
- Does not apply `DotCanStack` stacking, no mine/trap speed override, no `DotDpsCap`.

**AoE:**
- No AoE calculation exists in the Rust. `AreaOfEffectMod`, `AreaOfEffectRadius`, and
  `AreaOfEffectRadiusMetres` are never written.

### What's missing / wrong

1. **`round((1 + inc/100) * more, 2)` rounding step absent in Speed.** Lua rounds the
   inc/more product to 2 decimal places before dividing into `baseTime`. The Rust applies
   the unrounded product, causing small but cumulative precision errors.

2. **`TotalAttackTime` / `TotalCastTime` flat additions not queried.** These BASE mods
   add flat seconds to attack/cast time and are applied outside the speed multiplier.

3. **Action speed applied unconditionally.** Lua only applies `ActionSpeedMod` to
   `selfCast` and `totem` skills. The Rust always multiplies by `action_speed`, inflating
   speed for non-selfCast spells, triggered skills, and instant-cast skills.

4. **No `castTime == 0` instant-cast guard.** Lua sets `Speed = 0` for instant skills;
   Rust would produce `1/0 = inf` or undefined behaviour.

5. **No `timeOverride`, `fixedCastTime`, `triggerTime`, `triggerRate` branches.** These
   are required for triggered skills (CoC, CwC, CWDT), brands, and warcries.

6. **No cooldown cap.** Lua applies `Speed = min(Speed, Repeats / Cooldown)`. The Rust
   computes a cooldown separately but does not constrain `Speed`.

7. **No server tick cap.** `Speed = min(Speed, ServerTickRate × Repeats)`. `ServerTickRate ≈ 30.30/s`.

8. **No warcry path.** `output.Speed = 1 / output.WarcryCastTime` for warcry skills.

9. **No totem action speed scaling.** `output.Speed = output.Speed × (1 + INC_TotemActionSpeed/100)`.

10. **`HitSpeed` and `HitTime` not computed.** Skills with `hitTimeMultiplier` or
    `hitTimeOverride` (channels, brands, Shockwave Totem) never write these fields.

11. **No per-hand speed sub-tables or `combineStat`.** `MainHand.Speed` / `OffHand.Speed`
    are never set; harmonic mean combination for dual-wield is absent.

12. **`dpsMultiplier` not applied to `TotalDPS`.** Lua: `AverageDamage × (HitSpeed or Speed) × dpsMultiplier × quantityMultiplier`. Rust: `average_damage × uses_per_sec` only.

13. **`HitSpeed or Speed` selection absent in `TotalDPS`.** For channeling/brand skills
    the hit rate is `HitSpeed`, not `Speed`. The Rust always uses `Speed`.

14. **`quantityMultiplier` not applied.** `skillModList:Sum("BASE", skillCfg, "QuantityMultiplier")`, clamped to ≥ 1, multiplies `TotalDPS`.

15. **`TotalDot` for `DotCanStack` skills not implemented.** The current Rust sets
    `TotalDot = total_dot_instance_sum` unconditionally. For stackable DoT skills it should
    be `TotalDotInstance × Speed × Duration × dpsMultiplier × quantityMultiplier`, capped at
    `DotDpsCap ≈ 35.8M`.

16. **Mine/trap speed override for `TotalDot` absent.**

17. **`AreaOfEffectMod`, `AreaOfEffectRadius`, `AreaOfEffectRadiusMetres` not written.**
    The entire `calcAreaOfEffect` function is absent from the Rust.

## What Needs to Change

1. **Add `round(inc_more_product, 2)` before dividing into `baseTime` in speed formula.**
   Rust: `let rounded_mod = ((speed_inc_more * 100.0).round()) / 100.0;`

2. **Query `TotalAttackTime` / `TotalCastTime` BASE mods and add to denominator.**
   ```rust
   let flat_time_add = mod_db.sum_cfg(Base, "TotalAttackTime", cfg, output)
                     + mod_db.sum_cfg(Base, "TotalCastTime", cfg, output);
   let speed = 1.0 / (base_time / rounded_mod + flat_time_add);
   ```

3. **Apply `ActionSpeedMod` only for `selfCast` and `totem` skills.** Remove the
   unconditional `* action_speed` multiplication from the current Rust speed calculation.

4. **Add instant-cast guard:** if `grantedEffect.castTime == 0` (and no override): `Speed = 0, Time = 0`.

5. **Implement triggered speed branches** (`timeOverride`, `fixedCastTime`, `triggerTime`,
   `triggerRate`) for triggered skills. These short-circuit the normal inc/more formula.

6. **Implement cooldown speed cap:** `Speed = Speed.min(Repeats / Cooldown)` when Cooldown > 0.

7. **Implement server tick cap:** `Speed = Speed.min(ServerTickRate * Repeats)` for
   non-channeling skills. `ServerTickRate = 1.0 / 0.033 ≈ 30.30303/s`.

8. **Implement totem action speed scaling:** query `INC TotemActionSpeed`, apply as
   `Speed × (1 + INC/100)` when `skill.is_totem`.

9. **Implement `HitSpeed` / `HitTime` for channeling and brand skills.** When
   `skillData.hitTimeMultiplier` is set: `HitTime = Time × multiplier`;
   `HitSpeed = 1 / HitTime` (or `min(1/HitTime, ServerTickRate)` post-combination).

10. **Implement per-hand Speed sub-tables and `combineStat(Speed, HARMONICMEAN)`.** This
    is a structural requirement for dual-wield attack skills.

11. **Apply `dpsMultiplier` to `TotalDPS`:** query `INC DPS` and `More DPS` from
    `skillModList` with `skillCfg`, apply as:
    ```rust
    let dps_mult = (1.0 + inc_dps / 100.0) * more_dps;
    let total_dps = average_damage * (hit_speed.unwrap_or(speed)) * dps_mult * qty_mult;
    ```

12. **Apply `quantityMultiplier` to `TotalDPS`:** `max(1.0, Sum("BASE", "QuantityMultiplier"))`.

13. **Select `HitSpeed or Speed` in `TotalDPS`:** prefer `output.get("HitSpeed")` over `Speed`.

14. **Fix `TotalDot` for `DotCanStack` skills:**
    ```rust
    if dot_can_stack {
        let speed = if is_mine { mine_speed } else if is_trap { trap_speed } else { uses_per_sec };
        let total_dot = (dot_instance * speed * duration * dps_mult * qty_mult)
                        .min(DOT_DPS_CAP);
        env.player.set_output("TotalDot", total_dot);
    }
    // else: existing non-stackable path (TotalDot = TotalDotInstance) is correct
    ```
    `DOT_DPS_CAP = (2u64.pow(31) - 1) as f64 / 60.0 ≈ 35_791_394.0`.

15. **Implement `calcAreaOfEffect`:** query `AreaOfEffect` and `AreaOfEffectPrimary` INC/More
    from `skillModList`, apply double-round, compute `calcRadius`:
    ```rust
    let area_mod = round2(round10(inc_area * more_area));  // double-round
    let radius = ((base_radius as f64) * (100.0 * area_mod.sqrt()).floor() / 100.0).floor() as i64;
    env.player.set_output("AreaOfEffectMod", area_mod);
    if skill.has_radius {
        env.player.set_output("AreaOfEffectRadius", radius as f64);
        env.player.set_output("AreaOfEffectRadiusMetres", radius as f64 / 10.0);
    }
    ```
    Only write `AreaOfEffectRadius`/`AreaOfEffectRadiusMetres` when the skill has a `radius`
    value in its gem data.
