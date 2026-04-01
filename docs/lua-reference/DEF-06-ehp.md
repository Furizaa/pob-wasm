# DEF-06: Effective Hit Pool (EHP)

## Output Fields

Fields this chunk must write (from `field_groups.rs`):

| Field | Oracle non-zero | Lua line(s) | Notes |
|-------|-----------------|------------|-------|
| `AverageEvadeChance` | 9/30 | 1644 | Average of melee+projectile evade / 4 |
| `AverageNotHitChance` | 10/30 | 1643 | Average of all 4 not-hit chances / 4 |
| `AverageBlockChance` | 0/30 | **MISSING** | **Never written by PoB**; phantom field |
| `AverageSpellBlockChance` | 0/30 | **MISSING** | **Never written by PoB**; phantom field |
| `MeleeNotHitChance` | 10/30 | 1638 | `1 - (1-evade)(1-dodge)(1-avoidAll)` |
| `ProjectileNotHitChance` | 10/30 | 1639 | Adds projectile avoidance factor |
| `SpellNotHitChance` | 2/30 | 1640 | No evasion, uses spell dodge + avoidAll |
| `AttackTakenHitMult` | 30/30 | 1891 | Global (non-type) attack damage-taken mult |
| `SpellTakenHitMult` | 30/30 | 1891 | Global spell damage-taken mult |
| `TotalEHP` | 30/30 | 2878 | `TotalNumberOfHits * totalEnemyDamageIn` |
| `PhysicalMaximumHitTaken` | 30/30 | 3240 | Max survivable hit, per-type |
| `FireMaximumHitTaken` | 30/30 | 3240 | |
| `ColdMaximumHitTaken` | 30/30 | 3240 | |
| `LightningMaximumHitTaken` | 30/30 | 3240 | |
| `ChaosMaximumHitTaken` | 30/30 | 3240 | |
| `AnyAegis` | 0/30 | 2411, 2415, 2421 | Boolean sentinel |
| `AnyBypass` | 1/30 | 2274, 2278, 2284 | CI sets ChaosEnergyShieldBypass=100 |
| `AnyGuard` | 0/30 | 2390 | Boolean sentinel |
| `AnySpecificMindOverMatter` | 0/30 | 2295, 2340 | Per-type MoM mods exist |
| `AnyTakenReflect` | 0/30 | 1871, 1904 | Always false (commented-out in Lua) |
| `sharedAegis` | 0/30 | 2408 | `modDB:Max(nil, "AegisValue") or 0` |
| `sharedElementalAegis` | 0/30 | 2409 | `modDB:Max(nil, "ElementalAegisValue") or 0` |
| `sharedGuardAbsorbRate` | 2/30 | 2371 | `min(Sum("GuardAbsorbRate"), 100)` |
| `sharedMindOverMatter` | 0/30 | 2296 | `min(Sum("DamageTakenFromManaBeforeLife"), 100)` |
| `sharedMoMHitPool` | 30/30 | 2316, 2334 | Effective life pool used by MoM for hits |
| `sharedManaEffectiveLife` | 30/30 | 2315, 2333 | Effective life for regen/recovery |
| `totalEnemyDamage` | 30/30 | 1659, 1768 | Total enemy damage with crit |
| `totalEnemyDamageIn` | 30/30 | 1660, 1758 | Total enemy damage before crit |
| `totalTakenDamage` | 30/30 | 1848, 1860 | After "damage taken as" conversion |
| `totalTakenHit` | 30/30 | 1933, 2033 | After all mitigations (resist, DR, mults) |
| `enemySkillTime` | 30/30 | 2889, 2891 | Seconds per enemy attack/cast |
| `enemyBlockChance` | 0/30 | CalcOffence 2147 | Per-skill enemy block vs player; 0 for non-attack |
| `noSplitEvade` | 30/30 | 1093 | True when melee/projectile evade equal |
| `ehpSectionAnySpecificTypes` | 0/30 | 2292, 2339, 2389, 2414, 2420 | Per-type variations in EHP model |
| `specificTypeAvoidance` | 0/30 | 1527, 1531 | True if any per-type damage avoidance >0 |
| `preventedLifeLoss` | 0/30 | 2224 | Eternal Youth / Petrified Blood |
| `preventedLifeLossBelowHalf` | 0/30 | 2226 | Petrified Blood below-half effect |
| `preventedLifeLossTotal` | 0/30 | 2231, 2233 | Combined prevented loss total |

> **`AverageBlockChance` and `AverageSpellBlockChance` are phantom fields.** PoB writes
> `EffectiveAverageBlockChance` (line 765) not these. They appear in 0/30 oracle files
> and are never written by any PoB module. Remove from `field_groups.rs`.

> **`AnyTakenReflect` is always `false`.** Line 1904 contains a commented-out `true`
> assignment: `output.AnyTakenReflect = false --true --this needs a rework`. This
> feature was disabled upstream and will always be false.

> **`AttackTakenHitMult` and `SpellTakenHitMult`** are written at line 1891 as
> `output["Attack"/"Spell".."TakenHitMult"]` — these are the hit-source global mults
> (not per-damage-type). They represent the `(1 + AttackDamageTaken INC/100) * More`
> multiplier applied to all attack or spell hits regardless of damage type.

## Dependencies

- `DEF-01-resistances` — `*Resist` values used in per-type taken-hit multipliers
- `DEF-02-armour-evasion-es-ward` — `Armour`, `EnergyShield`, `Ward`, `ArmourDefense`, evade chance
- `DEF-03-block-suppression` — `EffectiveBlockChance`, `EffectiveSpellBlockChance`, suppression
- `DEF-04-damage-reduction-avoidance` — `Base*DamageReduction`, `AvoidAllDamageFromHitsChance`,
  `AttackDodgeChance`, `SpellDodgeChance`, `specificTypeAvoidance`
- `DEF-05-recovery-in-defence` — `EnergyShieldRecoveryCap` (needed for MoM-EB pool calc)
- `PERF-02-life-mana-es` — `Life`, `Mana`, `ManaUnreserved`, `LifeUnreserved`, `EnergyShield`

## Lua Source

**File: `CalcDefence.lua`**, function `calcs.buildDefenceEstimations`  
**Lines 1625–2901** (main EHP function)  
**Lines 3090–3301** (maximum hit taken calculation, called from above)

Commit: `454eff8c85d24356d9b051d596983745ed367476` (third-party/PathOfBuilding, heads/dev)

## Annotated Lua

### Section 1: Not-hit chances (lines 1635–1656)

Only computed when `damageCategoryConfig != "DamageOverTime"` (almost always true).

```lua
-- These combine evasion, dodge, and avoidance into a composite not-hit probability.
-- Each factor is independent (multiplicative).

output.MeleeNotHitChance = 100 - (1 - output.MeleeEvadeChance / 100)
                               * (1 - output.EffectiveAttackDodgeChance / 100)
                               * (1 - output.AvoidAllDamageFromHitsChance / 100)
                               * 100
-- Rust: let melee_not_hit = (1.0 - (1.0 - evade/100.0) * (1.0 - dodge/100.0) * (1.0 - avoid/100.0)) * 100.0;
-- Note: Lua version does NOT include block in these not-hit calculations.
-- Block is handled separately in the EHP calculation.

output.ProjectileNotHitChance = 100 - (1 - output.ProjectileEvadeChance / 100)
    * (1 - output.EffectiveAttackDodgeChance / 100)
    * (1 - output.AvoidAllDamageFromHitsChance / 100)
    * (1 - (output.specificTypeAvoidance and 0 or output.AvoidProjectilesChance) / 100)
    * 100
-- Gotcha: specificTypeAvoidance suppresses AvoidProjectilesChance in favour of
-- per-type avoidance. If any Avoid*DamageChance > 0, projectile avoidance is excluded
-- from this calculation (it's already accounted for in the specific-type branch).

output.SpellNotHitChance = 100 - (1 - output.EffectiveSpellDodgeChance / 100)
                               * (1 - output.AvoidAllDamageFromHitsChance / 100)
                               * 100
-- Note: no evasion for spells; only spell dodge and blanket avoidance.

output.SpellProjectileNotHitChance = ...  -- spell dodge + avoidAll + proj avoidance (if !specificType)
output.UntypedNotHitChance = ...          -- avoidAll only

output.AverageNotHitChance = (output.MeleeNotHitChance + output.ProjectileNotHitChance
                             + output.SpellNotHitChance + output.SpellProjectileNotHitChance) / 4
-- Average over all four attack categories (not including Untyped).

output.AverageEvadeChance = (output.MeleeEvadeChance + output.ProjectileEvadeChance) / 4
-- Gotcha: divides by 4, not 2! This gives the evade chance averaged over
-- all four hit types (melee, projectile, spell, spell-projectile).
-- Spells and SpellProjectile have 0 evasion, so their 0% contribution halves
-- the value compared to what you might expect.
```

**Rust bugs (calc_not_hit_chances):**
- Lua `MeleeNotHitChance` excludes block; Rust includes block (`EffectiveBlockChance`) — **wrong formula**
- Lua `ProjectileNotHitChance` uses `ProjectileEvadeChance` specifically; Rust uses generic `evade_chance` — **wrong**
- `AverageEvadeChance` not written by Rust at all — **missing**

### Section 2: Enemy damage estimation (lines 1658–1790)

This section reads configured enemy damage per damage type from `env.configInput`
(the "Configuration" tab inputs), applies enemy crit and conversion, and writes the
per-type enemy damage values that all subsequent mitigation calculations use.

```lua
output["totalEnemyDamage"] = 0
output["totalEnemyDamageIn"] = 0

-- The enemy damage for each damage type comes from configuration inputs:
-- e.g. env.configInput["enemyPhysicalDamage"] (set by user in Configuration tab)
-- If not set, falls back to env.configPlaceholder (level-based defaults).
-- Default enemy damage at level 84 is approximately 4000 total (varies by type).

for _, damageType in ipairs(dmgTypeList) do  -- {"Physical","Lightning","Cold","Fire","Chaos"}
    local enemyDamage = tonumber(env.configInput["enemy"..damageType.."Damage"])
    -- configInput can be nil; tonumber(nil) = nil; handled below
    if enemyDamage == nil then
        enemyDamage = tonumber(env.configPlaceholder["enemy"..damageType.."Damage"]) or 0
    end
    -- configPlaceholder contains level-based default enemy damage values

    -- Enemy crit effect multiplier (affects both crit chance and damage):
    local enemyCritEffect = output["EnemyCritEffect"]
    -- = 1 + enemyCritChance/100 * (enemyCritDamage/100) * (1 - CritExtraDamageReduction/100)
    -- Already computed above at line 1687

    output[damageType.."EnemyDamage"] = enemyDamage * (1 - conversionTotal/100)
                                       * enemyDamageMult * output["EnemyCritEffect"]
    -- enemyDamageMult = calcLib.mod(enemyDB, nil, "Damage", damageType.."Damage", ...)
    -- This is the enemy's own damage modifiers (not the player's taken modifiers)

    output["totalEnemyDamageIn"] = output["totalEnemyDamageIn"] + enemyDamage
    -- totalEnemyDamageIn = raw input damage before crit/mods

    output["totalEnemyDamage"] = output["totalEnemyDamage"] + output[damageType.."EnemyDamage"]
    -- totalEnemyDamage = after enemy mods + crit
end
```

**Key differences from Rust:**
- The Lua reads per-type damage from `configInput`/`configPlaceholder`; the Rust uses
  a single `EnemyDamage` BASE mod (defaulting to 1500) distributed by percentages.
  The oracle builds have `totalEnemyDamageIn = 9860` for phys_melee_slayer, suggesting
  the configured placeholder uses ~9860 total damage (level 84 enemy).
- The Lua also handles **enemy damage conversion** (physical → other types), which the
  Rust ignores. In practice, most oracle builds have no enemy conversion so this doesn't
  affect results.

### Section 3: "Damage taken as" conversion (lines 1792–1868)

Player-side conversion: some player keystones/items convert a % of one incoming damage
type to another (e.g., "30% of Cold Damage Taken from Hits as Fire Damage").

```lua
-- actor.damageShiftTable[sourceType][destType] = % of sourceType converted to destType
-- Built from modDB: "PhysicalDamageFromHitsTakenAsFire", "ColdDamageTakenAsFire" etc.
-- The remainder stays as the original type: shiftTable[damageType] = max(100 - total, 0)

-- Compute per-type TakenDamage (after conversion):
for _, damageType in ipairs(dmgTypeList) do
    output[damageType.."TakenDamage"] = output[damageType.."EnemyDamage"]
                                       * actor.damageShiftTable[damageType][damageType] / 100
end
-- Then add converted amounts from other types:
for _, damageType in ipairs(dmgTypeList) do
    for _, damageConvertedType in ipairs(dmgTypeList) do
        if damageType ~= damageConvertedType then
            output[damageConvertedType.."TakenDamage"] = output[damageConvertedType.."TakenDamage"]
                + output[damageType.."EnemyDamage"] * actor.damageShiftTable[damageType][damageConvertedType] / 100
        end
    end
end

output["totalTakenDamage"] = sum of all TakenDamage values
```

The Rust does not implement damage-taken-as conversion. All `{Type}TakenDamage` in
the Rust equals the input `{Type}EnemyDamage` unchanged.

### Section 4: Damage taken multipliers (lines 1870–2035)

This is the core taken-hit multiplier loop. It computes per-type, per-source (attack/spell)
multipliers incorporating resistance, armour, flat modifiers, and hit-source scaling.

```lua
local hitSourceList = {"Attack", "Spell"}  -- defined at line 26

for _, damageType in ipairs(dmgTypeList) do
    local baseTakenInc = modDB:Sum("INC", nil, "DamageTaken", damageType.."DamageTaken")
    local baseTakenMore = modDB:More(nil, "DamageTaken", damageType.."DamageTaken")
    if isElemental[damageType] then
        baseTakenInc = baseTakenInc + modDB:Sum("INC", nil, "ElementalDamageTaken")
        baseTakenMore = baseTakenMore * modDB:More(nil, "ElementalDamageTaken")
    end

    -- Hit multiplier (shared baseline):
    do
        local takenInc = baseTakenInc + modDB:Sum("INC", nil, "DamageTakenWhenHit", damageType.."DamageTakenWhenHit")
        local takenMore = baseTakenMore * modDB:More(nil, "DamageTakenWhenHit", damageType.."DamageTakenWhenHit")
        if isElemental[damageType] then
            takenInc = takenInc + modDB:Sum("INC", nil, "ElementalDamageTakenWhenHit")
            takenMore = takenMore * modDB:More(nil, "ElementalDamageTakenWhenHit")
        end
        output[damageType.."TakenHitMult"] = m_max((1 + takenInc / 100) * takenMore, 0)
        -- Note: does NOT yet include resist or armour — those come later in TakenHit

        -- Per-hit-source (Attack vs Spell) multipliers:
        for _, hitType in ipairs(hitSourceList) do  -- {"Attack", "Spell"}
            local baseTakenIncType = takenInc + modDB:Sum("INC", nil, hitType.."DamageTaken")
            local baseTakenMoreType = takenMore * modDB:More(nil, hitType.."DamageTaken")
            output[hitType.."TakenHitMult"] = m_max((1 + baseTakenIncType / 100) * baseTakenMoreType, 0)
            -- Writes output["AttackTakenHitMult"] and output["SpellTakenHitMult"] (global)
            -- ALSO writes output[damageType..hitType.."TakenHitMult"]:
            output[damageType..hitType.."TakenHitMult"] = output[hitType.."TakenHitMult"]
            -- e.g., output["PhysicalAttackTakenHitMult"] = output["AttackTakenHitMult"]
        end
    end

    -- Full TakenHit (with resist + armour + flat):
    -- (see lines 1947–2033 above for the per-type detailed computation)
    local resist = output[damageType.."Resist"]   -- or ResistWhenHit
    local reduction = output["Base"..damageType.."DamageReductionWhenHit"]  -- or Base DR
    local resMult = 1 - (resist - enemyPen) / 100
    -- armourReduct = calcs.armourReduction(effectiveAppliedArmour, damage * resMult) ...
    local reductMult = 1 - max(min(DRMax, armourReduct + reduction - enemyOverwhelm), 0) / 100
    local takenMult = output[damageType.."AttackTakenHitMult"]  -- (or Spell/Average)
    output[damageType.."TakenHit"] = max(damage * resMult * reductMult + takenFlat, 0) * takenMult * spellSuppressMult
    output[damageType.."TakenHitMult"] = (damage > 0) and (takenHit / damage) or 0
    -- Final TakenHitMult overwrites the earlier one: it's now the *complete* multiplier
    -- including resist, armour, INC, More

    output["totalTakenHit"] = sum of all TakenHit values
end
```

**Key insight:** `AttackTakenHitMult` and `SpellTakenHitMult` (the DEF-06 fields)
are the **INC/More-only** hit-source multipliers. They are `(1 + AttackDamageTaken_INC/100) * More`.
They do NOT include resistance or armour. Those are combined into `*TakenHitMult`
(per-type, per-source) only when `*TakenHit` is computed.

In oracle, `AttackTakenHitMult = SpellTakenHitMult = 0.77` for phys_melee_slayer
because that build has exactly `23% reduced attack/spell damage taken` from a mod.

### Section 5: Life loss prevention / Petrified Blood (lines 2220–2265)

```lua
local preventedLifeLoss = m_min(modDB:Sum("BASE", nil, "LifeLossPrevented"), 100)
output["preventedLifeLoss"] = preventedLifeLoss
-- "LifeLossPrevented" comes from Eternal Youth / some unique items:
-- % of life loss from hits is "taken over 4 seconds as degen instead"
-- Range: 0–100; most builds have 0

local initialLifeLossBelowHalfPrevented = modDB:Sum("BASE", nil, "LifeLossBelowHalfPrevented")
output["preventedLifeLossBelowHalf"] = (1 - preventedLifeLoss / 100) * initialLifeLossBelowHalfPrevented
-- Petrified Blood: "% of life loss below half is prevented"
-- Scaled down by the Eternal Youth effect (if any)

local portionLife = 1
if not env.configInput["conditionLowLife"] then
    portionLife = m_min(output.Life * 0.5 / recoverable, 1)
    output["preventedLifeLossTotal"] = preventedLifeLoss + preventedLifeLossBelowHalf * portionLife
else
    output["preventedLifeLossTotal"] = preventedLifeLoss + preventedLifeLossBelowHalf
end
-- portionLife = fraction of recoverable life below half
-- preventedLifeLossTotal is the % of total damage that bypasses direct life reduction
```

All three `preventedLifeLoss*` fields are 0 for all 30 oracle builds. These are
primarily relevant for Petrified Blood builds.

### Section 6: ES bypass / AnyBypass (lines 2268–2290)

```lua
output.AnyBypass = false
output.MinimumBypass = 100  -- used internally for MoM+EB calculation
for _, damageType in ipairs(dmgTypeList) do
    if modDB:Flag(nil, "UnblockedDamageDoesBypassES") then
        -- Some builds (Chaos Inoculation partially) bypass ES for all types
        output[damageType.."EnergyShieldBypass"] = 100
        output.AnyBypass = true
    else
        output[damageType.."EnergyShieldBypass"] = modDB:Override(nil, damageType.."EnergyShieldBypass")
                                                   or modDB:Sum("BASE", nil, damageType.."EnergyShieldBypass")
                                                   or 0
        if output[damageType.."EnergyShieldBypass"] ~= 0 then
            output.AnyBypass = true
        end
        if damageType == "Chaos" then
            if not modDB:Flag(nil, "ChaosNotBypassEnergyShield") then
                -- Chaos damage bypasses ES by default! (+100% bypass)
                output[damageType.."EnergyShieldBypass"] = output[damageType.."EnergyShieldBypass"] + 100
            else
                -- "ChaosNotBypassEnergyShield" (Chaos Inoculation): chaos DOESN'T bypass
                output.AnyBypass = true  -- still set true as an edge case
            end
        end
    end
    output[damageType.."EnergyShieldBypass"] = m_max(m_min(bypass, 100), 0)
    output.MinimumBypass = m_min(output.MinimumBypass, output[damageType.."EnergyShieldBypass"])
end
```

**Gotcha — Chaos damage always bypasses ES by default.** The `ChaosEnergyShieldBypass`
starts at 0 and then gets `+ 100` added (line 2282), resulting in 100% bypass (clamped).
This is the default game mechanic. `AnyBypass = true` for almost all builds because chaos
always bypasses. The oracle shows `AnyBypass: 1/30` non-false, but CI builds have
`ChaosNotBypassEnergyShield` set so they get `AnyBypass = true` from the else branch.

### Section 7: Mind over Matter (lines 2292–2367)

```lua
output["sharedMindOverMatter"] = m_min(modDB:Sum("BASE", nil, "DamageTakenFromManaBeforeLife"), 100)
-- "X% of damage taken from mana before life" — shared MoM applies to all damage types

-- calcMoMEBPool: computes effective life pool considering MoM mana + ES (if ESProtectsMana)
-- Returns: (totalPool, maxManaUsable, manaUsed, ESUsed)
local function calcMoMEBPool(lifePool, MoMEffect, esBypass)
    local mana = m_max(output.ManaUnreserved or 0, 0)
    local maxMoMPool = MoMEffect < 1 and lifePool / (1 - MoMEffect) - lifePool or m_huge
    local maxManaUsable = m_floor(m_min(mana, maxMoMPool))
    local maxESUsable = modDB:Flag(nil, "EnergyShieldProtectsMana") and esBypass < 1 and
        m_floor(m_min(output.EnergyShieldRecoveryCap, maxMoMPool * (1 - esBypass),
                      (lifePool + maxManaUsable) / (1 - (1 - esBypass) * MoMEffect) - (lifePool + maxManaUsable)))
        or 0
    local manaUsed = m_floor(m_min(maxMoMPool - maxESUsable, maxManaUsable))
    return lifePool + manaUsed + maxESUsable, maxManaUsable, manaUsed, maxESUsable
end

if output["sharedMindOverMatter"] > 0 then
    local MoMEffect = output["sharedMindOverMatter"] / 100
    local esBypass = output.MinimumBypass / 100
    local sharedMoMPool = calcMoMEBPool(output.LifeRecoverable, MoMEffect, esBypass)
    output["sharedManaEffectiveLife"] = sharedMoMPool
    -- For hits (uses LifeHitPool instead of LifeRecoverable):
    output["sharedMoMHitPool"] = calcMoMEBPool(output.LifeHitPool, MoMEffect, esBypass)
else
    output["sharedManaEffectiveLife"] = output.LifeRecoverable
    output["sharedMoMHitPool"] = output.LifeHitPool
end
```

**Gotcha — `sharedMoMHitPool` is a number, not a table.** `calcMoMEBPool` returns 4
values: `return lifePool + manaUsed + maxESUsable, maxManaUsable, manaUsed, maxESUsable`.
`output["sharedMoMHitPool"] = calcMoMEBPool(...)` only captures the first return value
(Lua multi-return: `a = f()` only captures first return). So `sharedMoMHitPool` is
`LifeHitPool + manaUsed + ESUsed` — a single number.

For builds without MoM (`sharedMindOverMatter = 0`):
- `sharedManaEffectiveLife = LifeRecoverable`
- `sharedMoMHitPool = LifeHitPool`

Oracle confirms: phys_melee_slayer has `sharedMoMHitPool = sharedManaEffectiveLife = 4961`
which equals `LifeUnreserved` (no MoM, no EB).

### Section 8: Guard (lines 2369–2403)

```lua
output.AnyGuard = false
output["sharedGuardAbsorbRate"] = m_min(modDB:Sum("BASE", nil, "GuardAbsorbRate"), 100)
-- "GuardAbsorbRate" = % of damage taken by a guard skill before reaching the player
-- Siphoning Trap, Warlord's Mark, Guardian's Blessing etc. give this
if output["sharedGuardAbsorbRate"] > 0 then
    output.AnyGuard = true  -- set below if per-type guard exists
    output["sharedGuardAbsorb"] = calcLib.val(modDB, "GuardAbsorbLimit")
    -- GuardAbsorbLimit = maximum life in the guard pool
end
```

Oracle: 2/30 builds have `sharedGuardAbsorbRate = 75` (these use Guardian's Blessing
or similar). `AnyGuard` is still false for them because the type-specific loop at
lines 2386–2403 doesn't set it for shared-only guard.

### Section 9: Aegis (lines 2406–2428)

```lua
output.AnyAegis = false
output["sharedAegis"] = modDB:Max(nil, "AegisValue") or 0
-- modDB:Max returns the highest MAX-type mod, or nil → or 0
-- "AegisValue" comes from Aegis Aurora (the unique shield)
output["sharedElementalAegis"] = modDB:Max(nil, "ElementalAegisValue") or 0
-- ElementalAegisValue for elemental-only aegis (uncommon)
if output["sharedAegis"] > 0 then
    output.AnyAegis = true
end
if output["sharedElementalAegis"] > 0 then
    output.ehpSectionAnySpecificTypes = true
    output.AnyAegis = true
end
```

### Section 10: `TotalEHP` computation (lines 2877–2900)

```lua
-- numberOfHitsToDie: runs the full EHP simulation to count how many hits kill you
-- This is the core of the EHP calculation — it simulates the actual hit sequence
-- accounting for block, ES recharge, aegis absorb, MoM, guard, etc.

-- TotalEHP is the final output:
output["TotalEHP"] = output["TotalNumberOfHits"] * output["totalEnemyDamageIn"]
-- TotalEHP = (number of configured hits to die) × (raw enemy damage per hit)
-- This gives "total damage absorbed" as the EHP metric.
-- Note: uses totalEnemyDamageIn (pre-crit) not totalEnemyDamage (post-crit)

-- Survival time:
output.enemySkillTime = (env.configInput.enemySpeed or env.configPlaceholder.enemySpeed or 700)
                       / (1 + enemyDB:Sum("INC", nil, "Speed") / 100)
output.enemySkillTime = output.enemySkillTime / 1000 / calcs.actionSpeedMod(actor.enemy)
-- enemySpeed default = 700ms between attacks → 0.7s
-- Divided by 1000 to convert ms → s
-- Divided by enemy action speed modifier
```

**Oracle**: `phys_melee_slayer.enemySkillTime = 0.7` confirms the default value.
**Rust**: `calc_enemy_damage` uses a fixed `0.7` default — this matches but ignores
`env.configInput.enemySpeed` and `enemyDB.Speed` INC modifiers.

### Section 11: `*MaximumHitTaken` (CalcDefence.lua lines 3090–3301)

The maximum hit taken per damage type is computed via binary-search iteration:
PoB finds the largest enemy hit where the player survives (all pools go to 0 but not negative).

```lua
-- For each damage type:
local enemyDamageMult = output[damageType.."EnemyDamageMult"]
-- Use binary search or simplified formula to find max hit
-- The simplified path (when no aegis/guard/MoM complications):
-- passIncomingDamage iterates until pools are exactly drained
local finalMaxHit = round(passIncomingDamage / enemyDamageMult)
output[damageType.."MaximumHitTaken"] = finalMaxHit
```

The binary search variant iterates:
1. Compute how much damage pools would take
2. Adjust `passIncomingDamage` to converge on the exact pool-draining amount
3. Divide by enemy damage mult to get the "raw enemy hit" that would do this

**Oracle**: `phys_melee_slayer` Physical = 9537, Fire = 35621. Physical is much lower
because armour only partially mitigates physical (formula: armour/(armour+5×damage)),
while elemental resistances fully apply.

### Section 12: `enemySkillTime` and `enemyBlockChance`

- `enemySkillTime` is written in `calcs.buildDefenceEstimations` (line 2889–2891)
  from `configInput.enemySpeed` default 700ms.
- `enemyBlockChance` is written in **CalcOffence.lua** (line 2147), not CalcDefence.
  It is the enemy's block chance against the player's attacks. All 30 oracle builds
  have `enemyBlockChance = 0` (enemies don't block in the oracle configs).

## Existing Rust Code

**File:** `crates/pob-calc/src/calc/defence_ehp.rs`

| Function | Lines | Status |
|----------|-------|--------|
| `calc_not_hit_chances` | 17–55 | ⚠️ Wrong formulas |
| `calc_enemy_damage` | 59–200 | ⚠️ Partial — simplified |
| `calc_damage_taken_mult` | 204–278 | ⚠️ Partial — missing per-source mults |
| `calc_max_hit_taken` | 282–329 | ⚠️ Partial — simplified pool model |
| `calc_total_ehp` | 333–357 | ❌ Wrong — uses `PhysicalMaximumHitTaken` not `TotalNumberOfHits * totalEnemyDamageIn` |

### Detailed status

| Feature | Rust status |
|---------|-------------|
| `MeleeNotHitChance` formula | ❌ **Wrong** — Rust includes block in not-hit calc; Lua doesn't (block is separate EHP factor) |
| `ProjectileNotHitChance` | ❌ **Wrong** — uses generic `evade_chance` not `ProjectileEvadeChance`; includes proj_block |
| `SpellNotHitChance` | ❌ **Wrong** — includes `spell_block` (Lua doesn't) |
| `AverageNotHitChance` | ✅ Correct formula (averages 4 types) |
| **`AverageEvadeChance`** | ❌ **Missing** — not written at all |
| `totalEnemyDamageIn` | ✅ Written (as `totalEnemyDamageIn`) |
| `totalEnemyDamage` | ✅ Written (includes avg crit mult) |
| `enemySkillTime` | ⚠️ Hardcoded 0.7s — misses `configInput.enemySpeed` and enemy Speed INC |
| `EnemyCritChance` | ✅ Present (defaults to 5%) |
| Per-type enemy damage | ⚠️ Simplified — uses percentage distribution mods instead of configInput |
| Damage-taken-as conversion | ❌ Missing — `*TakenDamage` ignores `damageShiftTable` |
| `totalTakenDamage` | ❌ Wrong — won't match oracle when any "damage taken as" mods exist |
| **`AttackTakenHitMult` / `SpellTakenHitMult`** | ❌ **Missing** — Rust doesn't write these global hit-source mults |
| `*TakenHitMult` per-damage-type | ⚠️ Partial — includes resist+armour but misses per-source (attack/spell), elemental DamageTaken, WhenHit mods |
| `totalTakenHit` | ⚠️ Present but imprecise for same reasons |
| `PhysicalDamageReduction` (EHP section) | ✅ Written (line 1984 → Rust computes correctly) |
| **`sharedMoMHitPool`** | ❌ **Wrong** — Rust uses `sharedMoMHitPool = total_pool` (life+es+ward+mom_mana); Lua sets it to `LifeHitPool` (or `LifeHitPool + moM_pool`); different semantics for the "hit pool" |
| `sharedManaEffectiveLife` | ⚠️ Present but uses different pool definition |
| `sharedMindOverMatter` | ❌ Missing — Rust reads `DamageTakenFromManaBeforeLife` as `mom_pct` but doesn't write `sharedMindOverMatter` to output |
| `AnyAegis` / `sharedAegis` / `sharedElementalAegis` | ❌ Missing entirely |
| `AnyBypass` | ❌ Missing |
| `AnyGuard` / `sharedGuardAbsorbRate` | ❌ Missing — guard system not implemented |
| `ehpSectionAnySpecificTypes` | ❌ Missing |
| `preventedLifeLoss` / `preventedLifeLossTotal` | ❌ Missing |
| **`TotalEHP`** | ❌ **Wrong formula** — Rust uses `PhysicalMaximumHitTaken`; Lua uses `TotalNumberOfHits * totalEnemyDamageIn` |
| **`*MaximumHitTaken`** | ⚠️ Simplistic — Rust uses `pool / takenMult`; Lua uses binary-search simulation |
| `AverageBlockChance` / `AverageSpellBlockChance` | ❌ Phantom fields — should be removed from `field_groups.rs` |

### TotalEHP formula discrepancy — critical

The Rust `calc_total_ehp` sets `TotalEHP = PhysicalMaximumHitTaken`, which is the
maximum survivable physical hit. The Lua computes a completely different quantity:

```
Lua: TotalEHP = TotalNumberOfHits × totalEnemyDamageIn
```

Where `TotalNumberOfHits` is the number of configured hits (including block/suppress/
avoidance, MoM drain, ES recharge, guard absorb, aegis, etc.) before dying, multiplied
by the raw enemy damage per hit. This is a survival metric, not a damage reduction metric.

**Oracle**: `phys_melee_slayer.TotalEHP = 23846.45` while
`PhysicalMaximumHitTaken = 9537`. These are fundamentally different quantities. The
Rust value will never match the oracle.

### `MeleeNotHitChance` formula discrepancy

```
Lua:  1 - (1-evade/100) × (1-attackDodge/100) × (1-avoidAll/100)
Rust: 1 - (1-evade/100) × (1-attackDodge/100) × (1-block/100)
```

Block is NOT in the Lua's not-hit formula. Block is handled as a separate factor
in the hit-count simulation (`numberOfHitsToDie`). The Rust incorrectly substitutes
block for avoid-all.

## What Needs to Change

1. **Fix `MeleeNotHitChance` formula** — remove block, add `AvoidAllDamageFromHitsChance`:
   ```rust
   let avoid_all = get_output_f64(&output, "AvoidAllDamageFromHitsChance");
   let melee_not_hit = (1.0 - (1.0 - melee_evade/100.0)
       * (1.0 - attack_dodge/100.0) * (1.0 - avoid_all/100.0)) * 100.0;
   ```

2. **Use `MeleeEvadeChance` and `ProjectileEvadeChance` separately** (not generic `EvadeChance`):
   ```rust
   let melee_evade = get_output_f64(&output, "MeleeEvadeChance");
   let proj_evade = get_output_f64(&output, "ProjectileEvadeChance");
   ```

3. **Fix `ProjectileNotHitChance`** — uses projectile evade, attack dodge, avoid-all,
   and conditionally `AvoidProjectilesChance` (suppressed when `specificTypeAvoidance = true`).

4. **Write `AverageEvadeChance`**:
   ```rust
   // (MeleeEvadeChance + ProjectileEvadeChance + 0 + 0) / 4 — spells have no evasion
   set_output("AverageEvadeChance", (melee_evade + proj_evade) / 4.0);
   ```

5. **Write `AttackTakenHitMult` and `SpellTakenHitMult`** (global, non-type):
   ```rust
   // Sum INC and More from "AttackDamageTaken" (and "DamageTakenWhenHit" + "DamageTaken" base)
   let attack_taken_mult = (1.0 + attack_taken_inc / 100.0) * attack_taken_more;
   set_output("AttackTakenHitMult", attack_taken_mult.max(0.0));
   // Similar for SpellTakenHitMult
   ```

6. **Fix `*TakenHitMult` to include per-source (Attack/Spell) modifiers** — the Lua
   writes `*AttackTakenHitMult` and `*SpellTakenHitMult` separately; the Rust combines
   everything into one `*TakenHitMult`. Need to separate attack vs. spell paths.

7. **Write `sharedMindOverMatter` output field**:
   ```rust
   set_output("sharedMindOverMatter", mom_pct);  // already computed, just not written
   ```

8. **Implement the sentinel boolean fields** (`AnyBypass`, `AnyAegis`, `AnyGuard`,
   `AnySpecificMindOverMatter`, `ehpSectionAnySpecificTypes`):
   - `AnyBypass = true` when any `*EnergyShieldBypass > 0` (chaos always bypasses)
   - `AnyAegis`, `AnyGuard` currently always false for oracle builds (TAIL priority)
   - `ehpSectionAnySpecificTypes` gates per-type MoM/guard/aegis paths

9. **Fix `TotalEHP` formula** — must implement `numberOfHitsToDie` simulation:
   ```
   TotalEHP = TotalNumberOfHits × totalEnemyDamageIn
   ```
   This requires the full hit simulation (PoB lines 2706–2876), which accounts for
   block/suppress/avoidance, MoM mana drain, ES recharge windows, guard absorption,
   and aegis pools. This is the most complex piece of the EHP chunk.

10. **Improve `*MaximumHitTaken` to use iterative pool simulation** — the Rust
    simple `pool / takenMult` will underestimate for high-MoM builds and ES builds.
    The Lua converges via binary search using `calcs.takenHitFromDamage` (iterative).

11. **Remove phantom fields from `field_groups.rs`**:
    - `AverageBlockChance` — never written by PoB
    - `AverageSpellBlockChance` — never written by PoB

## Oracle Confirmation (selected builds)

### phys_melee_slayer

| Field | Oracle value | Notes |
|-------|-------------|-------|
| `TotalEHP` | 23846.5 | `TotalNumberOfHits × 9860` |
| `totalEnemyDamageIn` | 9860 | Configured enemy damage |
| `totalEnemyDamage` | 10007.9 | With crit mult (5% × 1.3) |
| `totalTakenHit` | 3166.3 | After resist+armour+DR+mults |
| `PhysicalMaximumHitTaken` | 9537 | High armour build |
| `FireMaximumHitTaken` | 35621 | 76% fire resist |
| `ChaosMaximumHitTaken` | 5444 | -53% chaos resist |
| `AttackTakenHitMult` | 0.77 | 23% reduced damage taken |
| `SpellTakenHitMult` | 0.77 | Same mod applies to both |
| `sharedMoMHitPool` | 4961 | = LifeHitPool (no MoM) |
| `enemySkillTime` | 0.7 | Default 700ms |
| `noSplitEvade` | True | Melee = Projectile evade |
| `AnyBypass` | False | CI: chaos doesn't bypass |

### mom_eb (Mind over Matter + Eldritch Battery)

| Field | Oracle value | Notes |
|-------|-------------|-------|
| `sharedMindOverMatter` | 0 | Standard MoM is 0 here |
| `sharedMoMHitPool` | 3413 | life + mana contribution |
| `sharedManaEffectiveLife` | 3413 | Same (no shared MoM active) |

> Note: The oracle `sharedMoMHitPool` for mom_eb is 3413 (> `Life` alone), confirming
> that mana contribution to the hit pool is correctly included.
