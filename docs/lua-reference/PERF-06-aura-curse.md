# PERF-06-aura-curse: Aura & Curse Effects

## Placeholder Notice

`field_groups.rs` lists this chunk as a placeholder (`&[]`). This document establishes the
actual output fields by cross-referencing all oracle JSON builds. The registry must be updated
before this chunk's tests can run.

## Output Fields

These fields are oracle-verified across the 30 realworld builds:

| Field | Written by | Module | Lines |
|-------|-----------|--------|-------|
| `EnemyCurseLimit` | `calcs.perform` (main flow) | CalcPerform.lua | 3020 |
| `CurseAvoidChance` | `calcs.defence` | CalcDefence.lua | 1578 |
| `SilenceAvoidChance` | `calcs.defence` | CalcDefence.lua | 1579 |
| `CurseEffectOnSelf` | `calcs.defence` | CalcDefence.lua | 1586 |
| `DebuffExpirationRate` | `calcs.defence` | CalcDefence.lua | 1591 |
| `DebuffExpirationModifier` | `calcs.defence` | CalcDefence.lua | 1592 |
| `showDebuffExpirationModifier` | `calcs.defence` | CalcDefence.lua | 1593 |
| `MissingTotemFireResist` | `calcs.resistances` | CalcDefence.lua | 621 |
| `MissingTotemColdResist` | `calcs.resistances` | CalcDefence.lua | 621 |
| `MissingTotemLightningResist` | `calcs.resistances` | CalcDefence.lua | 621 |
| `MissingTotemChaosResist` | `calcs.resistances` | CalcDefence.lua | 621 |
| `TotemFireResist` | `calcs.resistances` | CalcDefence.lua | 618 |
| `TotemColdResist` | `calcs.resistances` | CalcDefence.lua | 618 |
| `TotemLightningResist` | `calcs.resistances` | CalcDefence.lua | 618 |
| `TotemChaosResist` | `calcs.resistances` | CalcDefence.lua | 618 |
| `TotemFireResistTotal` | `calcs.resistances` | CalcDefence.lua | 619 |
| `TotemColdResistTotal` | `calcs.resistances` | CalcDefence.lua | 619 |
| `TotemLightningResistTotal` | `calcs.resistances` | CalcDefence.lua | 619 |
| `TotemChaosResistTotal` | `calcs.resistances` | CalcDefence.lua | 619 |
| `TotemFireResistOverCap` | `calcs.resistances` | CalcDefence.lua | 620 |
| `TotemColdResistOverCap` | `calcs.resistances` | CalcDefence.lua | 620 |
| `TotemLightningResistOverCap` | `calcs.resistances` | CalcDefence.lua | 620 |
| `TotemChaosResistOverCap` | `calcs.resistances` | CalcDefence.lua | 620 |
| `EnemyCritChance` | `calcs.buildDefenceEstimations` | CalcDefence.lua | 1685 |
| `EnemyCritEffect` | `calcs.buildDefenceEstimations` | CalcDefence.lua | 1687 |
| `EnemyStunThresholdMod` | `calcs.buildActiveSkill` | CalcOffence.lua | 5226,5228 |
| `EnemyStunDuration` | `calcs.buildActiveSkill` | CalcOffence.lua | 5238,5243,5247,5252,5259 |
| `EnemyLifeRegen` | `calcs.buildActiveSkill` | CalcOffence.lua | 3516 |
| `EnemyManaRegen` | `calcs.buildActiveSkill` | CalcOffence.lua | 3517 |
| `EnemyEnergyShieldRegen` | `calcs.buildActiveSkill` | CalcOffence.lua | 3518 |
| `ReservationDPS` | `calcs.buildActiveSkill` | CalcOffence.lua | 5951 |
| `ReservationDpsMultiplier` | `calcs.buildActiveSkill` | CalcOffence.lua | 3057 |
| `AuraEffectMod` | `calcs.buildActiveSkill` | CalcOffence.lua | 1138 |
| `CurseEffectMod` | `calcs.buildActiveSkill` | CalcOffence.lua | 1164 |
| `TotalBuildDegen` | `calcs.buildDefenceEstimations` | CalcDefence.lua | 3317–3369 |
| `NetLifeRegen` | `calcs.buildDefenceEstimations` | CalcDefence.lua | 3371,3441 |
| `NetManaRegen` | `calcs.buildDefenceEstimations` | CalcDefence.lua | 3372,3442 |
| `NetEnergyShieldRegen` | `calcs.buildDefenceEstimations` | CalcDefence.lua | 3373,3443 |

**Shared with DEF-01-resistances:** `TotemFireResist`, `TotemColdResist`, etc. are written
in the same `calcs.resistances` function as the player resists. They are tracked here because
they relate to the buff/curse domain, but the Lua code writes them as part of resistance
calculation (CalcDefence.lua:594–621).

## Dependencies

- **PERF-01-attributes** → must be complete (Str, Int, Dex correct).
- **PERF-02-life-mana-es** → `LifeRegenRecovery`, `ManaRegenRecovery`, `EnergyShieldRegenRecovery`
  must exist before `NetLifeRegen` etc. can be computed (CalcDefence.lua:3371–3373 reads them).
- **PERF-03-charges** → `PowerChargesMax` must be set before `EnemyCurseLimit` (CalcPerform.lua:3020).
- **DEF-01-resistances** → Totem resists are a by-product of resistance calculation.
- **PERF-07-regen-recharge-leech** → `NetLifeRegen`/`NetManaRegen`/`NetEnergyShieldRegen` depend
  on the `*RegenRecovery` fields from PERF-07. The degen section in CalcDefence:3371–3443
  reads those fields and then subtracts per-type degen values. If PERF-07 fields are wrong,
  the Net* fields will be wrong.

## Lua Source

**File 1:** `third-party/PathOfBuilding/src/Modules/CalcPerform.lua`  
**Commit:** `454eff8c85d24356d9b051d596983745ed367476`  
Lines: 3015–3021 (`EnemyCurseLimit` write)

**File 2:** `third-party/PathOfBuilding/src/Modules/CalcDefence.lua`  
**Commit:** `454eff8c85d24356d9b051d596983745ed367476`  
Lines: 594–634 (totem resists), 1578–1621 (curse avoidance, CurseEffectOnSelf, debuff expiration),
1680–1692 (EnemyCritChance/Effect), 3316–3462 (TotalBuildDegen + NetRegen)

**File 3:** `third-party/PathOfBuilding/src/Modules/CalcOffence.lua`  
**Commit:** `454eff8c85d24356d9b051d596983745ed367476`  
Lines: 1137–1142 (AuraEffectMod), 1163–1167 (CurseEffectMod), 3056–3057 (ReservationDpsMultiplier),
3515–3518 (EnemyLifeRegen/Mana/ES), 5949–5951 (ReservationDPS)

---

## Annotated Lua

### `EnemyCurseLimit` (CalcPerform.lua:3020)

```lua
-- Inside calcs.perform(), after curse list processing:
output.EnemyCurseLimit = modDB:Flag(nil, "CurseLimitIsMaximumPowerCharges")
    and output.PowerChargesMax
    or modDB:Sum("BASE", nil, "EnemyCurseLimit")
-- Lua `a and b or c` ternary:
--   If "CurseLimitIsMaximumPowerCharges" flag is set → use PowerChargesMax (from PERF-03)
--   Else → sum BASE "EnemyCurseLimit" mods
-- Default value: the game constant BASE mod for EnemyCurseLimit = 1 (from game_constants)
-- This write happens AFTER the aura/curse loop processes active skills.
-- Rust: mod_db.flag("CurseLimitIsMaximumPowerCharges", None, output)
--         .then(|| get_output_f64(output, "PowerChargesMax"))
--         .unwrap_or_else(|| mod_db.sum(None, "EnemyCurseLimit"))
```

---

### Totem resists (CalcDefence.lua:594–634)

```lua
-- Inside calcs.resistances(actor), iterated for each elem in resistTypeList:
-- ("Fire", "Cold", "Lightning", "Chaos")

-- Totem resists have their own BASE mod name ("TotemFireResist" etc.)
-- They share the player's resistance max cap but have their own base values.
if not totemTotal then
    local base = modDB:Sum("BASE", nil, "Totem"..elem.."Resist",
                           isElemental[elem] and "TotemElementalResist")
    -- isElemental = { Fire = true, Cold = true, Lightning = true }
    -- For elemental types, also sum "TotemElementalResist" (a combined stat)
    -- For Chaos, isElemental["Chaos"] = nil (falsy), so second arg is nil → ignored

    totemTotal = base * m_max(calcLib.mod(modDB, nil,
                                          "Totem"..elem.."Resist",
                                          isElemental[elem] and "TotemElementalResist"), 0)
    -- calcLib.mod = (1 + INC/100) * More
    -- m_max(..., 0): prevent negative total if More mods are < 0
end

-- Fractional resistances are truncated (not rounded):
totemTotal = m_modf(totemTotal)   -- m_modf returns integer part only (truncates)
totemMax   = m_modf(totemMax)     -- totemMax = same as player's max resist cap (75 base)

local totemFinal = m_max(m_min(totemTotal, totemMax), min)
-- min = resistance floor (usually 0, can be -infinity for Chaos on some builds)

output["Totem"..elem.."Resist"]       = totemFinal
output["Totem"..elem.."ResistTotal"]  = totemTotal    -- uncapped total
output["Totem"..elem.."ResistOverCap"]= m_max(0, totemTotal - totemMax)
output["MissingTotem"..elem.."Resist"]= m_max(0, totemMax - totemFinal)
-- MissingTotemXxxResist = how many more points of resist totem needs to cap out
-- At 40 base Fire resist and 75 max: MissingTotemFireResist = max(0, 75 - 40) = 35
```

**Key difference from player resists:**
- `m_modf` is used, not `m_floor`. `m_modf(x)` returns the integer part (truncation toward
  zero), equivalent to Rust's `x.trunc()`. For positive values this is the same as `floor`,
  but the Lua code uses `m_modf` explicitly.
- Totem resists use separate `TotemXxxResist` BASE mods, not the player's resist mods.
- Totems share the player's `ResistMax` cap (the same `max` variable computed for player).

**In Rust:** `defence.rs:150–168` — this is already implemented correctly (uses `totem_base_resist.min(totem_max)` and writes all four fields). One potential issue: Rust computes
`totem_base_resist` as a raw sum without the `calcLib.mod` multiplier (INC/MORE mods for
`TotemXxxResist` are not applied). The Lua applies `calcLib.mod` which includes INC and MORE.

---

### `CurseAvoidChance`, `SilenceAvoidChance` (CalcDefence.lua:1578–1579)

```lua
output.CurseAvoidChance = modDB:Flag(nil, "CurseImmune")
    and 100
    or m_min(modDB:Sum("BASE", nil, "AvoidCurse"), 100)
-- If CurseImmune flag → 100 (full immunity)
-- Else → sum BASE "AvoidCurse" mods, capped at 100
-- Rust: flag("CurseImmune") → 100.0, else sum("AvoidCurse").min(100.0)

output.SilenceAvoidChance = modDB:Flag(nil, "SilenceImmune")
    and 100
    or output.CurseAvoidChance
-- SilenceAvoidChance = CurseAvoidChance when not SilenceImmune
-- If SilenceImmune flag → 100
-- This means CurseAvoidChance must be computed BEFORE SilenceAvoidChance
```

**Rust bug:** `defence.rs:790–795` computes `SilenceAvoidChance` from `AvoidSilence` BASE
mod, not from `CurseAvoidChance`. The correct Lua logic is: `SilenceImmune → 100, else
CurseAvoidChance`. There is no `AvoidSilence` stat in PoB's modDB — silence avoidance is
derived entirely from curse avoidance.

---

### `CurseEffectOnSelf` (CalcDefence.lua:1586)

```lua
output.CurseEffectOnSelf = m_max(
    modDB:More(nil, "CurseEffectOnSelf")
    * (100 + modDB:Sum("INC", nil, "CurseEffectOnSelf")),
    0)
-- This is NOT calcLib.mod — it's a manual combination of More × (100 + INC).
-- Result is a percentage (e.g. 100 = no change, 50 = 50% curse effect on self).
-- Base 100 means all INC sums around 0 and More products around 1.
-- m_max(..., 0): prevent negative curse effect (can't be < 0%).
-- Key: the formula is More × (100 + INC), NOT (1 + INC/100) × More × 100.
--      The "100" is the base value already included in the expression.
-- Rust: (mod_db.more(None, "CurseEffectOnSelf") * (100.0 + mod_db.sum(None, ModType::Inc, "CurseEffectOnSelf"))).max(0.0)
```

---

### `DebuffExpirationRate` and `DebuffExpirationModifier` (CalcDefence.lua:1591–1593)

```lua
output.DebuffExpirationRate = modDB:Sum("BASE", nil, "SelfDebuffExpirationRate")
-- Raw BASE sum of faster/slower debuff expiration mods (as an INC-like percentage, e.g. 30 = 30% faster)

output.DebuffExpirationModifier = 10000 / (100 + output.DebuffExpirationRate)
-- Formula: 10000 / (100 + rate)
-- At 0 rate → 10000/100 = 100 (no change)
-- At 100 rate (100% faster) → 10000/200 = 50 (debuffs last half as long)
-- At -50 rate (50% slower) → 10000/50 = 200 (debuffs last twice as long)
-- This is a divisor applied to duration formulas below.
-- Rust: 10000.0 / (100.0 + debuff_rate)

output.showDebuffExpirationModifier = (output.DebuffExpirationModifier ~= 100)
-- Boolean: true when the modifier is not the default value (display gating)
-- Rust: debuff_modifier != 100.0
```

**Rust bug:** `defence.rs:875–885` queries `ModType::Inc, "DebuffExpirationRate"` (INC mods)
but the Lua queries `modDB:Sum("BASE", nil, "SelfDebuffExpirationRate")` (BASE mods, different
stat name `SelfDebuffExpirationRate`). The stat name is wrong AND the mod type is wrong.

Also, the Rust `DebuffExpirationModifier` formula is `100.0 + debuff_rate_inc`
(`defence.rs:883`) but the Lua formula is `10000 / (100 + rate)`. These produce very
different results: for `rate = 0` the Rust gives `100.0` (correct coincidence), but for
`rate = 30` the Rust gives `130.0` while Lua gives `76.9`. The Rust formula is wrong.

---

### `AuraEffectMod` (CalcOffence.lua:1137–1142)

```lua
-- Inside calcs.buildActiveSkill(), when skill has SkillType.Aura:
if activeSkill.skillTypes[SkillType.Aura] then
    output.AuraEffectMod = calcLib.mod(skillModList, skillCfg,
        "AuraEffect",
        not (skillData.auraCannotAffectSelf or activeSkill.skillTypes[SkillType.AuraAffectsEnemies])
            and "SkillAuraEffectOnSelf"
            or nil)
    -- calcLib.mod = (1 + INC/100) * More, where mod names are:
    --   "AuraEffect"                    -- always included
    --   "SkillAuraEffectOnSelf"         -- conditionally included:
    --     EXCLUDED if auraCannotAffectSelf OR skill is AuraAffectsEnemies type
    --     INCLUDED for normal auras that also affect the caster
    -- The nil/string arg to calcLib.mod: nil means "don't also query this stat";
    -- "SkillAuraEffectOnSelf" means query INC/MORE for that stat too.
    -- Result: combined aura effect modifier as a multiplier (e.g. 1.25 = 25% more effective)
end
-- NOTE: only written for aura skills. For non-aura builds, this field is absent.
```

---

### `CurseEffectMod` (CalcOffence.lua:1163–1167)

```lua
-- Inside calcs.buildActiveSkill(), when skill has SkillType.Hex or SkillType.Mark:
if activeSkill.skillTypes[SkillType.Hex] or activeSkill.skillTypes[SkillType.Mark] then
    output.CurseEffectMod = calcLib.mod(skillModList, skillCfg, "CurseEffect")
    -- Simple: (1 + INC("CurseEffect")/100) * More("CurseEffect")
    -- skill-scoped: uses skillModList and skillCfg
end
-- NOTE: only written for hex/mark skills. Absent for non-curse builds.
```

---

### `ReservationDpsMultiplier` and `ReservationDPS` (CalcOffence.lua:3057, 5951)

```lua
-- ReservationDpsMultiplier (line 3057), inside the main damage calc pass:
globalOutput.ReservationDpsMultiplier =
    100 / (100 - enemyDB:Sum("BASE", nil, "LifeReservationPercent"))
-- This models skills like Arakaali's Fang that cause the enemy to "reserve"
-- life, effectively increasing the DPS needed to kill them.
-- At 0% enemy life reservation (normal): 100 / (100 - 0) = 1.0
-- At 20% enemy life reservation: 100 / (100 - 20) = 1.25
-- Note: enemyDB (the enemy's modDB) is queried for LifeReservationPercent.
-- Rust: env.enemy.mod_db.sum(None, "LifeReservationPercent")

-- ReservationDPS (line 5951), at the end of the damage pass:
output.ReservationDPS = output.CombinedDPS * (output.ReservationDpsMultiplier - 1)
-- How much extra DPS is "needed" due to enemy life reservation.
-- CombinedDPS already includes the reservation multiplier in the final product;
-- ReservationDPS is the extra amount above base.
-- At no reservation: (1.0 - 1) * CombinedDPS = 0
-- At 25% multiplier: 0.25 * CombinedDPS
```

---

### `EnemyLifeRegen`, `EnemyManaRegen`, `EnemyEnergyShieldRegen` (CalcOffence.lua:3515–3518)

```lua
-- Inside calcs.buildActiveSkill(), in the "defence against hits" section:
output.EnemyLifeRegen         = enemyDB:Sum("INC", cfg, "LifeRegen")
output.EnemyManaRegen         = enemyDB:Sum("INC", cfg, "ManaRegen")
output.EnemyEnergyShieldRegen = enemyDB:Sum("INC", cfg, "EnergyShieldRegen")
-- These represent the percentage regen the enemy has (from curse mods like Temporal Chains).
-- The stat type is INC (percentage), not BASE — the enemy's base regen is fixed,
-- these are the INC modifiers applied to it.
-- Note: enemyDB is the enemy actor's modDB, with cfg (skill config) applied.
-- Typically 0 unless a curse or aura grants enemy regen reduction.
```

---

### `EnemyCritChance`, `EnemyCritEffect` (CalcDefence.lua:1680–1692)

```lua
-- Inside calcs.buildDefenceEstimations(), reading from configInput + modifiers:
local enemyCritChance = enemyDB:Flag(nil, "NeverCrit") and 0
    or enemyDB:Flag(nil, "AlwaysCrit") and 100
    or (m_max(m_min(
        (modDB:Override(nil, "enemyCritChance")
         or env.configInput["enemyCritChance"]
         or env.configPlaceholder["enemyCritChance"]
         or 0)
        * (1 + modDB:Sum("INC", nil, "EnemyCritChance") / 100
             + enemyDB:Sum("INC", nil, "CritChance") / 100)
        * (1 - output["ConfiguredEvadeChance"] / 100),
        100), 0))
-- Priority: NeverCrit flag → 0, AlwaysCrit flag → 100
-- Else: config value × INC modifiers × (1 - evade chance), clamped [0,100]
-- Config value comes from env.configInput["enemyCritChance"] (UI config)
-- or env.configPlaceholder (default values)
output["EnemyCritChance"] = enemyCritChance

local enemyCritDamage = enemyDB:Sum("BASE", nil, "CritMultiplier") or 150
output["EnemyCritEffect"] = 1 + enemyCritChance / 100
    * (enemyCritDamage / 100)
    * (1 - output.CritExtraDamageReduction / 100)
-- EnemyCritEffect: average damage multiplier accounting for enemy crit chance.
-- = 1 + (crit chance × crit multiplier × (1 - crit damage reduction))
-- At no crits: 1.0. At 5% crit chance and 150% multi: 1 + 0.05 * 1.5 = 1.075
```

---

### `EnemyStunThresholdMod` and `EnemyStunDuration` (CalcOffence.lua:5220–5285)

```lua
-- Inside calcs.buildActiveSkill(), in the stun section:
local enemyStunThresholdRed = skillModList:Sum("BASE", cfg, "EnemyStunThreshold")
if enemyStunThresholdRed > 75 then
    output.EnemyStunThresholdMod = 1
        - (75 + (enemyStunThresholdRed - 75) * 25 / (enemyStunThresholdRed - 50)) / 100
    -- Diminishing returns formula when threshold reduction > 75%
else
    output.EnemyStunThresholdMod = 1 - enemyStunThresholdRed / 100
    -- Linear reduction below 75%
end
-- EnemyStunThresholdMod: multiplier applied to enemy stun threshold for stun calculations.
-- At 0% reduction → 1.0. At 75% → 0.25. Above 75%, diminishing returns apply.

-- EnemyStunDuration is computed from base stun duration formula (not reproduced in full here)
-- and is scaled by INC stun duration mods and CritChance.
-- At 0.35s base: output.EnemyStunDuration = 0.35 (default in most builds)
```

---

### `TotalBuildDegen`, `NetLifeRegen`, `NetManaRegen`, `NetEnergyShieldRegen` (CalcDefence.lua:3316–3462)

```lua
-- Inside calcs.buildDefenceEstimations():
output.TotalBuildDegen = 0
for _, damageType in ipairs(dmgTypeList) do   -- Physical, Lightning, Cold, Fire, Chaos
    local baseVal = modDB:Sum("BASE", nil, damageType.."Degen")
    if baseVal > 0 then
        for damageConvertedType, convertPercent in pairs(actor.damageOverTimeShiftTable[damageType]) do
            if convertPercent > 0 then
                local total = baseVal * (convertPercent / 100)
                           * output[damageConvertedType.."TakenDotMult"]
                -- Degen for this damage type, after conversion and taken-dot multipliers
                output[damageConvertedType.."BuildDegen"] =
                    (output[damageConvertedType.."BuildDegen"] or 0) + total
                -- ^^^ uses "or 0" nil-coalescing for first write; not in PERF-06 field list
                output.TotalBuildDegen = output.TotalBuildDegen + total
            end
        end
    end
end
if output.TotalBuildDegen == 0 then
    output.TotalBuildDegen = nil    -- Explicitly set to nil when no degen
    -- In Rust: do NOT write TotalBuildDegen=0; leave the field absent when 0.
    -- Oracle JSON only contains TotalBuildDegen for builds that actually have degens.
else
    -- Write NetRegen fields based on degens split by pool (Life/Mana/ES):
    output.NetLifeRegen         = output.LifeRegenRecovery    -- start from regen
    output.NetManaRegen         = output.ManaRegenRecovery
    output.NetEnergyShieldRegen = output.EnergyShieldRegenRecovery
    -- LifeRegenRecovery etc. are PERF-07 fields; must already be computed.

    -- For each damage type that has build degen, distribute to pools:
    for _, damageType in ipairs(dmgTypeList) do
        if output[damageType.."BuildDegen"] then
            local takenFromMana = output[damageType.."MindOverMatter"]
                                + output["sharedMindOverMatter"]
            -- (MoM fractions for this damage type)
            if output.EnergyShieldRegenRecovery > 0 then
                -- ES is active: split between ES and Life/Mana
                lifeDegen         = ... -- portion hits life
                energyShieldDegen = ... -- portion absorbed by ES
                manaDegen         = ... -- MoM portion
            else
                lifeDegen = output[damageType.."BuildDegen"] * (1 - takenFromMana / 100)
                manaDegen = output[damageType.."BuildDegen"] * (takenFromMana / 100)
            end
            totalLifeDegen         = totalLifeDegen + lifeDegen
            totalManaDegen         = totalManaDegen + manaDegen
            totalEnergyShieldDegen = totalEnergyShieldDegen + energyShieldDegen
        end
    end
    output.NetLifeRegen         = output.NetLifeRegen - totalLifeDegen
    output.NetManaRegen         = output.NetManaRegen - totalManaDegen
    output.NetEnergyShieldRegen = output.NetEnergyShieldRegen - totalEnergyShieldDegen
    output.TotalNetRegen        = output.NetLifeRegen + output.NetManaRegen + output.NetEnergyShieldRegen
end
```

**`TotalBuildDegen = nil` gotcha:** When no build-side degen exists, the Lua explicitly sets
`output.TotalBuildDegen = nil`, which removes the field from the output table. In the oracle
JSON this field is **absent**, not 0. The Rust code must NOT write a 0 value — simply don't
write the field at all when the computed total is 0.

**Net regen fields only written when degen > 0:** `NetLifeRegen`, `NetManaRegen`, and
`NetEnergyShieldRegen` are only written when `TotalBuildDegen > 0` (inside the `else` branch).
When there are no degens, these fields are absent from the oracle JSON — they should not be
written even if PERF-07 regen fields are present.

---

## Existing Rust Code

### `EnemyCurseLimit` — `perform.rs:931–940`

The Rust code reads `EnemyCurseLimit` from modDB for the curse limit used during `apply_curses`,
but **does not write `output.EnemyCurseLimit`**. The oracle field is never set.

**Missing:** `env.player.set_output("EnemyCurseLimit", curse_limit as f64)`

Also missing: the `CurseLimitIsMaximumPowerCharges` flag check — Rust always uses the BASE
sum, never PowerChargesMax.

### `CurseAvoidChance` — `defence.rs:836–841` ✓ (partially correct)

Reads `AvoidCurse` BASE mod and clamps to 100. Correct formula.
Missing: the `CurseImmune` flag check (should be `100` when `CurseImmune` flag is set).

### `SilenceAvoidChance` — `defence.rs:790–795` ✗ (wrong formula)

Rust reads `AvoidSilence` BASE mod. The Lua derives this from `CurseAvoidChance`:
```lua
output.SilenceAvoidChance = modDB:Flag(nil, "SilenceImmune") and 100 or output.CurseAvoidChance
```
There is no `AvoidSilence` stat. The correct implementation is: if `SilenceImmune` flag → 100,
else copy `CurseAvoidChance`.

### `CurseEffectOnSelf` — not present

Not written anywhere in Rust. The Lua formula `More × (100 + INC)` gives a percentage
(base 100 = no change).

### `DebuffExpirationRate` and `DebuffExpirationModifier` — `defence.rs:874–889` ✗ (wrong)

Two bugs:
1. **Wrong stat name:** Rust queries `ModType::Inc, "DebuffExpirationRate"` but Lua queries
   `modDB:Sum("BASE", nil, "SelfDebuffExpirationRate")` — different stat name and mod type.
2. **Wrong modifier formula:** Rust computes `100.0 + debuff_rate_inc` but Lua computes
   `10000 / (100 + output.DebuffExpirationRate)`. These diverge for any non-zero rate.

### Totem resists — `defence.rs:151–168` ✓ (mostly correct)

All four `TotemXxxResist`, `TotemXxxResistTotal`, `TotemXxxResistOverCap`, and `MissingTotemXxxResist`
fields are written. Two potential gaps:
1. **`calcLib.mod` not applied:** Rust uses only the BASE sum (`totem_base_resist`), while the Lua
   applies `base * calcLib.mod(...)` which multiplies by INC/MORE mods for `TotemXxxResist` and
   `TotemElementalResist`. For builds with "+% increased Totem Resistances" mods, Rust will
   produce lower totem resists.
2. **`m_modf` (truncation) vs `floor`:** Lua uses `m_modf` (truncate toward zero). For positive
   values, truncation equals floor. Since resist values are always positive, this is equivalent.

### `AuraEffectMod` — not present

Not written in Rust. This is a CalcOffence-pass field written only for aura skills.

### `CurseEffectMod` — not present

Not written in Rust. This is a CalcOffence-pass field written only for hex/mark skills.

### `ReservationDPS`, `ReservationDpsMultiplier` — not present

Neither field exists in Rust. The `ReservationDpsMultiplier` is `100 / (100 - enemy life reservation %)`,
computed in CalcOffence at the end of the damage pass. `ReservationDPS` is then
`CombinedDPS × (multiplier - 1)`.

### `EnemyCritChance`, `EnemyCritEffect` — `defence_ehp.rs:78–103`

Partially present. The Rust reads `EnemyCritChance` from the modDB BASE sum, but the Lua:
- Checks `NeverCrit` and `AlwaysCrit` flags first
- Applies INC mods from both modDB and enemyDB
- Applies evade chance reduction

The Rust formula (`defence_ehp.rs:78–85`) does not apply INC modifiers or evade chance
reduction to the base config value.

### `EnemyStunThresholdMod`, `EnemyStunDuration` — not present in perform.rs

These are CalcOffence-pass fields. Not written in Rust.

### `EnemyLifeRegen`, `EnemyManaRegen`, `EnemyEnergyShieldRegen` — not present

Not written in Rust. These are CalcOffence-pass fields from the enemy modDB.

### `TotalBuildDegen`, `NetLifeRegen`, `NetManaRegen`, `NetEnergyShieldRegen` — not present

Not written in Rust. These are CalcDefence-pass fields depending on degen mods.

---

## What Needs to Change

1. **Write `output.EnemyCurseLimit`** in `perform.rs` (in `apply_curses` or a new helper):
   ```rust
   let curse_limit = if mod_db.flag(None, "CurseLimitIsMaximumPowerCharges", output) {
       get_output_f64(output, "PowerChargesMax")
   } else {
       mod_db.sum(None, ModType::Base, "EnemyCurseLimit")
   };
   env.player.set_output("EnemyCurseLimit", curse_limit);
   ```

2. **Fix `SilenceAvoidChance`** in `defence.rs`:
   ```rust
   let silence_avoid = if mod_db.flag(None, "SilenceImmune", output) {
       100.0
   } else {
       get_output_f64(output, "CurseAvoidChance")
   };
   env.player.set_output("SilenceAvoidChance", silence_avoid);
   ```
   (This requires `CurseAvoidChance` to be written before `SilenceAvoidChance`.)

3. **Fix `CurseAvoidChance`** to check `CurseImmune` flag:
   ```rust
   let curse_avoid = if mod_db.flag(None, "CurseImmune", output) {
       100.0
   } else {
       mod_db.sum(None, ModType::Base, "AvoidCurse").min(100.0)
   };
   ```

4. **Add `CurseEffectOnSelf`** in `defence.rs`:
   ```rust
   let more = mod_db.more(None, "CurseEffectOnSelf");
   let inc  = mod_db.sum(None, ModType::Inc, "CurseEffectOnSelf");
   env.player.set_output("CurseEffectOnSelf", (more * (100.0 + inc)).max(0.0));
   ```

5. **Fix `DebuffExpirationRate` and `DebuffExpirationModifier`** in `defence.rs`:
   ```rust
   let debuff_rate = mod_db.sum(None, ModType::Base, "SelfDebuffExpirationRate");
   // Note: BASE mods, stat name "SelfDebuffExpirationRate"
   env.player.set_output("DebuffExpirationRate", debuff_rate);
   let debuff_modifier = 10000.0 / (100.0 + debuff_rate);
   env.player.set_output("DebuffExpirationModifier", debuff_modifier);
   env.player.set_output_bool("showDebuffExpirationModifier", debuff_modifier != 100.0);
   ```

6. **Fix totem resist INC/MORE application** in `defence.rs`:
   Currently `totem_base_resist = sum(BASE, "Totem{elem}Resist")` without INC/MORE.
   Add `calcLib.mod` equivalent:
   ```rust
   let totem_base  = mod_db.sum(None, ModType::Base, &totem_resist_stat);
   let totem_inc   = mod_db.sum(None, ModType::Inc, &totem_resist_stat);
   // also "TotemElementalResist" for Fire/Cold/Lightning
   let totem_more  = mod_db.more(None, &totem_resist_stat);
   let totem_total = totem_base * ((1.0 + totem_inc / 100.0) * totem_more).max(0.0);
   let totem_total = totem_total.trunc(); // m_modf = truncate
   ```

7. **Add `AuraEffectMod` and `CurseEffectMod`** in CalcOffence-equivalent pass:
   These are skill-level output fields (only set for aura/curse skill types). They should be
   computed per active skill in the skill processing pass using `calcLib.mod` equivalent.

8. **Add `ReservationDpsMultiplier` and `ReservationDPS`** in the offence pass:
   ```rust
   let life_reservation_pct = env.enemy.mod_db.sum(None, ModType::Base, "LifeReservationPercent");
   let reservation_mult = 100.0 / (100.0 - life_reservation_pct);
   env.player.set_output("ReservationDpsMultiplier", reservation_mult);
   // At end of combined DPS calculation:
   let reservation_dps = combined_dps * (reservation_mult - 1.0);
   env.player.set_output("ReservationDPS", reservation_dps);
   ```

9. **Fix `EnemyCritChance`** in `defence_ehp.rs`:
   Apply INC modifiers from both modDB and enemyDB, and apply evade chance reduction:
   ```rust
   let config_crit = env.config_input.enemy_crit_chance.unwrap_or(5.0); // default 5%
   let inc_player  = mod_db.sum(None, ModType::Inc, "EnemyCritChance") / 100.0;
   let inc_enemy   = enemy_db.sum(None, ModType::Inc, "CritChance") / 100.0;
   let evade_pct   = get_output_f64(output, "ConfiguredEvadeChance");
   let crit = (config_crit * (1.0 + inc_player + inc_enemy) * (1.0 - evade_pct / 100.0))
       .clamp(0.0, 100.0);
   // Also check NeverCrit / AlwaysCrit flags
   ```

10. **Add `TotalBuildDegen`, `NetLifeRegen`, `NetManaRegen`, `NetEnergyShieldRegen`**:
    Implement the degen accumulation loop from CalcDefence.lua:3316–3462 in `defence.rs` or
    `defence_ehp.rs`. Only write `TotalBuildDegen` when the total > 0; omit it otherwise.
    Only write Net regen fields when degen > 0.

11. **Update `field_groups.rs`** — replace the empty `PERF-06-aura-curse` placeholder with
    the full field list above.
