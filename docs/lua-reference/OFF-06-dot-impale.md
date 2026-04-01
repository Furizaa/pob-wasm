# OFF-06-dot-impale: Non-Ailment DoT and Impale

## Output Fields

| Field | Lua source | Notes |
|-------|-----------|-------|
| `TotalDot` | CalcOffence.lua:5568–5608 | Sustained non-ailment DoT DPS; branches on `DotCanStack`, ground effects, etc. |
| `ImpaleDPS` | CalcOffence.lua:5853–5866 | DPS added by Impale; applies `HitSpeed or Speed` in non-average mode |
| `ImpaleHit` | *(not a real field — see note)* | Does not appear in oracle JSON; `field_groups.rs` entry is erroneous |
| `ImpaleModifier` | CalcOffence.lua:5326 | `1 + impaleDMGModifier`; scalar multiplier to hit damage from all active impales |
| `ImpaleStacks` | CalcOffence.lua:5322 → `globalOutput.ImpaleStacks` | `min(maxStacks, configStacks)` — active impale stack count used in Config tab |
| `ImpaleStacksMax` | CalcOffence.lua:5321 → `globalOutput.ImpaleStacksMax` | `ImpaleStacksMax × (1 + ImpaleAdditionalDurationChance/100)` |
| `impaleStoredHitAvg` | CalcOffence.lua:3139,3249–3251 | Pre-armour-reduction physical hit average; accumulated across crit/non-crit damage pass |

> **`ImpaleHit` note:** This field does not exist in PoB's Lua output and does not appear in
> any oracle JSON file. The `field_groups.rs` entry should be removed. The nearest equivalent
> is `ImpaleStoredDamage` (the per-stack effect percentage × 100, e.g. 15.3 = 15.3%
> = 1.53% × 10 stacks) but that is a different concept.

> **`impaleStoredHitAvg` case:** PoB uses lowercase `impaleStoredHitAvg` (camelCase with
> lowercase first letter). This is intentional — the oracle JSON confirms the lowercase
> spelling. All Rust code must use `"impaleStoredHitAvg"` as the output key.

## Dependencies

- `OFF-01-base-damage` — `{Type}Dot` base values from skill data, `{Type}MinBase`/`{Type}MaxBase`.
- `OFF-02-conversion` — Physical damage with `convMult` for `impaleStoredHitAvg`; DoT source
  damage uses `calcAilmentSourceDamage` which applies conversion.
- `OFF-03-crit-hit` — `CritChance` required for `impaleStoredHitAvg` weighting.
- `OFF-04-speed-dps` — `Speed`/`HitSpeed`, `Duration`, `dpsMultiplier` required for
  `TotalDot` (DotCanStack path) and `ImpaleDPS` rate multiplication.

## Lua Source

File: `third-party/PathOfBuilding/src/Modules/CalcOffence.lua`  
Commit: `454eff8c85d24356d9b051d596983745ed367476`

Primary line ranges:
- **`impaleStoredHitAvg` accumulation:** lines 3139, 3246–3252 (inside the hit damage pass loop)
- **`ImpaleModifier`, `ImpaleStacks`, `ImpaleStacksMax`:** lines 5293–5344 (impale calculation block)
- **Skill DoT construction (`dotCfg`):** lines 5468–5493
- **`TotalDotInstance` accumulation:** lines 5501–5555
- **`TotalDot` finalisation:** lines 5558–5610
- **`ImpaleDPS` calculation:** lines 5847–5868

## Annotated Lua

### 1. `impaleStoredHitAvg` — pre-armour physical average (lines 3139, 3246–3252)

`impaleStoredHitAvg` is initialised to 0 at the start of the hit damage loop (line 3139),
then accumulated inside the per-damage-type × per-pass loop for Physical:

```lua
-- Line 3139 (initialisation at start of damage accumulation):
output.impaleStoredHitAvg = 0

-- Lines 3246–3252 (inside: for damageType in dmgTypeList, pass 1 = crit, pass 2 = non-crit):
if damageType == "Physical" then
    -- store pre-armour physical damage from attacks for impale calculations
    if pass == 1 then
        -- Crit pass: weight by CritChance
        output.impaleStoredHitAvg = output.impaleStoredHitAvg
                                  + damageTypeHitAvg * (output.CritChance / 100)
    else
        -- Non-crit pass: weight by (1 - CritChance)
        output.impaleStoredHitAvg = output.impaleStoredHitAvg
                                  + damageTypeHitAvg * (1 - output.CritChance / 100)
    end
    -- NOTE: this accumulation uses the hit average BEFORE armour reduction is applied
    -- (armour reduction happens in lines 3253–3268 AFTER this block)
end
```

> **"Pre-armour":** `damageTypeHitAvg` at line 3249/3251 is the damage BEFORE enemy armour
> reduction, post-conversion, post-skill-inc/more. Armour reduction for the normal hit is
> applied at lines 3253–3268. Impale stores the full pre-armour value and applies armour
> separately in the impale modifier calculation (line 5311–5312).

> **`pass == 1` = crit, `pass == 2` = non-crit.** This is the opposite of what one might
> expect. Pass 1 sets `cfg.skillCond["CriticalStrike"] = true` (line 3142). The accumulated
> `impaleStoredHitAvg` is thus the crit-weighted average: `critAvg × critRate + nonCritAvg × (1−critRate)`.

---

### 2. Impale modifier calculation (lines 5293–5344)

```lua
if canDeal.Physical and (output.ImpaleChance + output.ImpaleChanceOnCrit) > 0 then
    skillFlags.impale = true
    local critChance = output.CritChance / 100
    -- Combined impale chance: weighted by crit/non-crit hit probability
    -- ImpaleChance = non-crit chance (set at line 3965 as mode_effective result)
    -- ImpaleChanceOnCrit = crit chance (set at line 3907)
    local impaleChance = (m_min(output.ImpaleChance/100, 1) * (1 - critChance)
                        + m_min(output.ImpaleChanceOnCrit/100, 1) * critChance)

    -- ImpaleStacksMax: base (5) × (1 + ImpaleAdditionalDurationChance/100)
    -- ImpaleAdditionalDurationChance: "Impales you inflict last 1 additional hit" nodes
    local maxStacks = skillModList:Sum("BASE", cfg, "ImpaleStacksMax")
                    * (1 + skillModList:Sum("BASE", cfg, "ImpaleAdditionalDurationChance") / 100)

    -- impaleStacks = min(maxStacks, configStacks)
    -- configStacks = enemyDB:Sum("BASE", cfg, "Multiplier:ImpaleStacks")
    --   → set by the Configuration tab "# of impale stacks on enemy" slider
    local configStacks = enemyDB:Sum("BASE", cfg, "Multiplier:ImpaleStacks")
    local impaleStacks = m_min(maxStacks, configStacks)

    -- Stored damage per stack = data.misc.ImpaleStoredDamageBase = 0.1 (10%)
    -- × (1 + ImpaleEffect_inc) × ImpaleEffect_more
    -- round(..., 2) for the more product
    local baseStoredDamage = data.misc.ImpaleStoredDamageBase     -- 0.1
    local storedExpectedDamageIncOnBleed = skillModList:Sum("INC", cfg, "ImpaleEffectOnBleed")
                                         * m_min(skillModList:Sum("BASE", cfg, "BleedChance")/100, 1)
    local storedExpectedDamageInc = (skillModList:Sum("INC", cfg, "ImpaleEffect")
                                    + storedExpectedDamageIncOnBleed) / 100
    local storedExpectedDamageMore = round(skillModList:More(cfg, "ImpaleEffect"), 2)
    local storedExpectedDamageModifier = (1 + storedExpectedDamageInc) * storedExpectedDamageMore
    local impaleStoredDamage = baseStoredDamage * storedExpectedDamageModifier
    -- impaleHitDamageMod = stored_damage_per_stack × stacks
    local impaleHitDamageMod = impaleStoredDamage * impaleStacks

    -- Armour reduction for impale damage (applied separately from hit armour):
    -- impale hits as though dealing impaleHitDamageMod × impaleStoredHitAvg
    local enemyArmour = m_max(calcLib.val(enemyDB, "Armour"), 0)
    local impaleArmourReduction = calcs.armourReductionF(
        enemyArmour,
        impaleHitDamageMod * output.impaleStoredHitAvg)
    -- impaleResist = min(max(0, physDamageReduction + impalePhysDmgReduction + armourReduction),
    --                    EnemyPhysicalDamageReductionCap)
    local impaleResist = m_min(m_max(0,
        enemyDB:Sum("BASE", nil, "PhysicalDamageReduction")
        + skillModList:Sum("BASE", cfg, "EnemyImpalePhysicalDamageReduction")
        + impaleArmourReduction),
        data.misc.EnemyPhysicalDamageReductionCap)
    if skillModList:Flag(cfg, "IgnoreEnemyImpalePhysicalDamageReduction") then
        impaleResist = 0
    end

    -- impaleTaken: enemy physical damage taken multiplier (ReflectedDamageTaken)
    local impaleTakenCfg = { flags = ModFlag.Hit }
    local impaleTaken = (1 + enemyDB:Sum("INC", impaleTakenCfg,
                                          "DamageTaken", "PhysicalDamageTaken", "ReflectedDamageTaken") / 100)
                       * enemyDB:More(impaleTakenCfg,
                                      "DamageTaken", "PhysicalDamageTaken", "ReflectedDamageTaken")
    -- Final impale DPS modifier per hit:
    -- impaleChance × storedDmg × stacks × (1 − resist/100) × takenMult
    local impaleDMGModifier = impaleHitDamageMod * (1 - impaleResist/100) * impaleChance * impaleTaken

    -- Write oracle fields:
    globalOutput.ImpaleStacksMax = maxStacks
    globalOutput.ImpaleStacks = impaleStacks
    output.ImpaleStoredDamage = impaleStoredDamage * 100  -- stored as percentage (15.3%)
    output.ImpaleModifier = 1 + impaleDMGModifier         -- e.g. 1.363 = 36.3% extra hit damage
end
```

> **`round(skillModList:More(cfg, "ImpaleEffect"), 2)`** — the `More` product is rounded to
> 2 decimal places before entering the stored damage formula. This is a rounding step the
> Rust currently omits.

> **`ImpaleAdditionalDurationChance`** — fractional extra stack: if an item says "Impales last
> 1 additional hit with 50% chance", the mod is 50 → adds 0.5 to `maxStacks`. The
> `maxStacks` computation is `baseStacks × (1 + ImpaleAdditionalDurationChance/100)`.

> **`impaleTakenCfg = { flags = ModFlag.Hit }`** — Lua table literal. The impale taken
> multiplier uses a special `Hit`-flagged cfg (not the DoT cfg), because impale damage is
> reflected hit damage, not a DoT. Rust: construct a `SkillCfg` with `ModFlags::HIT`.

> **`data.misc.ImpaleStoredDamageBase = 0.1`** (10% per stack, from `Modules/Data.lua:196`).

> **`ImpaleStacksMax` default = 5**, set via `modDB:NewMod("ImpaleStacksMax", "BASE", 5, ...)`
> in `CalcSetup.lua` from `characterConstants["impaled_debuff_number_of_reflected_hits"]`.

---

### 3. `ImpaleDPS` calculation (lines 5847–5868)

`ImpaleDPS` is computed after the combined DPS is assembled, under `if skillFlags.impale`:

```lua
-- Dual-wield double-hits: sum per-hand separately
if skillFlags.attack and skillData.doubleHitsWhenDualWielding and skillFlags.bothWeaponAttack then
    mainHandImpaleDPS = output.MainHand.impaleStoredHitAvg
                      * ((output.MainHand.ImpaleModifier or 1) - 1)
                      * output.MainHand.HitChance / 100
                      * skillData.dpsMultiplier
    offHandImpaleDPS  = output.OffHand.impaleStoredHitAvg
                      * ((output.OffHand.ImpaleModifier or 1) - 1)
                      * output.OffHand.HitChance / 100
                      * skillData.dpsMultiplier
    output.ImpaleDPS = mainHandImpaleDPS + offHandImpaleDPS
else
    -- Single-hand or spell path:
    output.ImpaleDPS = output.impaleStoredHitAvg
                     * ((output.ImpaleModifier or 1) - 1)  -- extra damage fraction
                     * output.HitChance / 100
                     * skillData.dpsMultiplier
end

if skillData.showAverage then
    -- "Show average damage" mode: ImpaleDPS stays as per-hit bonus
    output.WithImpaleDPS = output.AverageDamage + output.ImpaleDPS
    output.CombinedAvg = output.CombinedAvg + output.ImpaleDPS
else
    -- Normal DPS mode: multiply by hit rate to get per-second DPS
    skillFlags.notAverage = true
    output.ImpaleDPS = output.ImpaleDPS * (output.HitSpeed or output.Speed)
    output.WithImpaleDPS = output.TotalDPS + output.ImpaleDPS
end
if quantityMultiplier > 1 then
    output.ImpaleDPS = output.ImpaleDPS * quantityMultiplier
end
output.CombinedDPS = output.CombinedDPS + output.ImpaleDPS
```

> **`(output.ImpaleModifier or 1) - 1`** — extracts the fractional bonus from the
> total multiplier (e.g. `ImpaleModifier = 1.363` → extra = 0.363). This is applied
> to `impaleStoredHitAvg` (pre-armour physical) to get the extra DPS per attack.

> **`HitSpeed or Speed`** — prefer `HitSpeed` for channeling/brand skills. The Rust
> currently uses `uses_per_sec` (which maps to `Speed`), not checking `HitSpeed`.

> **`dpsMultiplier`** — the finalized DPS multiplier (line 2392–2396) is applied here too.
> The Rust does not apply this.

---

### 4. Skill DoT: `dotCfg` construction and `TotalDotInstance` (lines 5468–5555)

Before computing per-type DoT damage, PoB builds a `dotCfg` from the skill's config:

```lua
local dotCfg = {
    skillName = skillCfg.skillName,
    skillPart = skillCfg.skillPart,
    skillTypes = skillCfg.skillTypes,
    summonSkillName = skillCfg.summonSkillName,
    slotName = skillCfg.slotName,
    flags = bor(ModFlag.Dot, skillCfg.flags),      -- Dot | all skill flags
    keywordFlags = band(skillCfg.keywordFlags, bnot(KeywordFlag.Hit)),  -- strip Hit
}
-- Strip flags that don't apply to this skill's DoT:
if not skillData.dotIsArea then
    dotCfg.flags = band(dotCfg.flags, bnot(ModFlag.Area))
end
if not skillData.dotIsProjectile then
    dotCfg.flags = band(dotCfg.flags, bnot(ModFlag.Projectile))
end
if not skillData.dotIsSpell then
    dotCfg.flags = band(dotCfg.flags, bnot(ModFlag.Spell))
end
if not skillData.dotIsAttack then
    dotCfg.flags = band(dotCfg.flags, bnot(ModFlag.Attack))
end
if not skillData.dotIsHit then
    dotCfg.flags = band(dotCfg.flags, bnot(ModFlag.Hit))
end
```

> **`bor`/`band`/`bnot`** = bitwise OR/AND/NOT on integer flag fields. This is standard
> LuaJIT bit library. Rust equivalent: `|`, `&`, `!` on `ModFlags` bitfield types.

> **`dotTakenCfg`** — a copy of `dotCfg` with the `Spell` flag stripped for enemy
> `DamageTaken` queries when `skillData.dotIsSpell`. This prevents "spell damage taken"
> mods from double-applying on the enemy side. The Rust needs to maintain both `dotCfg`
> (for player skill queries) and `dotTakenCfg` (for enemy resistance/taken queries).

```lua
output.TotalDotInstance = 0  -- line 5501: reset before accumulation

for _, damageType in ipairs(dmgTypeList) do
    local dotTypeCfg = copyTable(dotCfg, true)  -- deep copy
    -- Add type-specific DoT keyword flag: e.g. KeywordFlag.PhysicalDot, FireDot, etc.
    dotTypeCfg.keywordFlags = bor(dotTypeCfg.keywordFlags, KeywordFlag[damageType.."Dot"])

    local baseVal
    if canDeal[damageType] then
        baseVal = skillData[damageType.."Dot"] or 0   -- from gem data, e.g. skillData.FireDot
    else
        baseVal = 0
    end

    if baseVal > 0 or (output[damageType.."Dot"] or 0) > 0 then
        skillFlags.dot = true
        local effMult = 1
        if env.mode_effective then
            -- Resistance and DamageTaken for this damage type
            local resist = damageType == "Physical"
                and m_max(0, m_min(enemyDB:Sum("BASE", nil, "PhysicalDamageReduction"),
                                    data.misc.EnemyPhysicalDamageReductionCap))
                or calcResistForType(damageType, dotTypeCfg)
            local takenInc = enemyDB:Sum("INC", dotTakenCfg, "DamageTaken",
                                          "DamageTakenOverTime",
                                          damageType.."DamageTaken",
                                          damageType.."DamageTakenOverTime")
                           + (isElemental[damageType] and enemyDB:Sum("INC", dotTakenCfg, "ElementalDamageTaken") or 0)
            local takenMore = enemyDB:More(dotTakenCfg, ...) * (isElemental[damageType] and ... or 1)
            effMult = (1 - resist/100) * (1 + takenInc/100) * takenMore
            output[damageType.."DotEffMult"] = effMult
        end

        -- Inc: Damage + {Type}Damage + ElementalDamage (for elemental types)
        local inc = skillModList:Sum("INC", dotTypeCfg, "Damage",
                                      damageType.."Damage",
                                      isElemental[damageType] and "ElementalDamage" or nil)
        -- Aura scaling (if skill is an aura):
        if skillModList:Flag(nil, "dotIsHeraldOfAsh") then
            inc = m_max(inc - skillModList:Sum("INC", skillCfg, "Damage", ...), 0)
        end
        local more = skillModList:More(dotTypeCfg, "Damage", damageType.."Damage",
                                        isElemental[damageType] and "ElementalDamage" or nil)
        -- DotMultiplier: Override takes precedence over Sum
        local mult = skillModList:Override(dotTypeCfg, "DotMultiplier")
                  or skillModList:Sum("BASE", dotTypeCfg, "DotMultiplier")
                    + skillModList:Sum("BASE", dotTypeCfg, damageType.."DotMultiplier")
        -- Aura effect modifier (for aura skills):
        local aura = activeSkill.skillTypes[SkillType.Aura]
                 and not activeSkill.skillTypes[SkillType.RemoteMined]
                 and not activeSkill.skillTypes[SkillType.Banner]
                 and calcLib.mod(skillModList, dotTypeCfg, "AuraEffect")
        local total = baseVal * (1 + inc/100) * more * (1 + mult/100) * (aura or 1) * effMult

        -- Two-path accumulation:
        if output[damageType.."Dot"] == 0 or not output[damageType.."Dot"] then
            -- Normal: TotalDotInstance += total; output.{Type}Dot = total
            output[damageType.."Dot"] = total
            output.TotalDotInstance = m_min(output.TotalDotInstance + total, data.misc.DotDpsCap)
        else
            -- {Type}Dot was already set (by preDotFunc or another source):
            -- add BOTH the existing value AND the new total
            output.TotalDotInstance = m_min(output.TotalDotInstance + total
                                            + (output[damageType.."Dot"] or 0),
                                            data.misc.DotDpsCap)
        end
    end
end
```

> **`output[damageType.."Dot"]`** — dynamic key: `output.PhysicalDot`, `output.FireDot`, etc.
> These may be set by `preDotFunc` (skill-specific pre-processing hook, e.g. Righteous Fire)
> before the loop runs. When they're already set, the accumulation path is different.

> **`KeywordFlag[damageType.."Dot"]`** — dynamic enum lookup: `KeywordFlag.PhysicalDot`,
> `KeywordFlag.FireDot`, etc. Rust: match on `dtype` string to get the right flag variant.

> **`copyTable(dotCfg, true)`** — deep-copy the config table per damage type so that
> per-type keyword flag mutations don't bleed across iterations. Rust: clone the `DotCfg`.

> **`override DotMultiplier`** — if an Override mod exists, use it directly (ignoring Sum).
> Rust: check `mod_db.override_value(dotCfg, "DotMultiplier")`, use Sum only if None.

> **Aura interaction:** If the skill is an aura, `AuraEffect` mod is an additional multiplier.
> `calcLib.mod(skillModList, dotTypeCfg, "AuraEffect")` = `(1 + INC_AuraEffect/100) × More_AuraEffect`.
> This applies to skills like Vitality, Purity, Malevolence when directly casting.

---

### 5. `TotalDot` finalisation (lines 5558–5610)

After the per-type loop, `TotalDot` is set via one of four branches:

```lua
if skillModList:Flag(nil, "DotCanStack") then
    -- Stackable DoT (e.g. Essence Drain, Vortex): each application stacks
    local speed = output.Speed
    -- Mine/trap delivery: use laying/throw speed instead of cast speed
    if band(dotCfg.keywordFlags, KeywordFlag.Mine) ~= 0 then
        speed = output.MineLayingSpeed
    elseif band(dotCfg.keywordFlags, KeywordFlag.Trap) ~= 0 then
        speed = output.TrapThrowingSpeed
    end
    output.TotalDot = m_min(
        output.TotalDotInstance * speed * output.Duration * skillData.dpsMultiplier * quantityMultiplier,
        data.misc.DotDpsCap)

elseif skillModList:Flag(nil, "dotIsBurningGround") then
    output.TotalDot = 0           -- burning ground handled as BurningGroundDPS
    output.BurningGroundDPS = m_max(output.BurningGroundDPS or 0, output.TotalDotInstance)

elseif skillModList:Flag(nil, "dotIsCausticGround") then
    output.TotalDot = 0           -- caustic ground handled as CausticGroundDPS
    output.CausticGroundDPS = m_max(output.CausticGroundDPS or 0, output.TotalDotInstance)

elseif skillModList:Flag(nil, "dotIsCorruptingBlood") then
    output.TotalDot = 0           -- corrupting blood handled as CorruptingBloodDPS
    output.CorruptingBloodDPS = m_max(output.CorruptingBloodDPS or 0, output.TotalDotInstance)

else
    -- DotCanStackAsTotems: totem-specific stackability
    if skillModList:Flag(nil, "DotCanStackAsTotems") and skillFlags.totem then
        skillFlags.DotCanStack = true
    end
    output.TotalDot = output.TotalDotInstance  -- non-stackable: only strongest applies
end
```

> **`DotCanStack`:** `TotalDot = TotalDotInstance × speed × duration × dpsMultiplier × quantityMultiplier`.
> The formula is: how many DoT instances the player applies per second (speed), how long each
> lasts (duration), and how much each instance deals (`TotalDotInstance` per second). The result
> is the total DPS when all stacks are active.

> **Ground effects (`dotIsBurningGround`, `dotIsCausticGround`):** `TotalDot = 0` because the
> DPS is tracked under a separate output field and rolled into `TotalDotDPS` differently. Skills
> like Caustic Arrow and Burning Arrow use these paths.

> **`data.misc.DotDpsCap = (2^31 - 1) / 60 ≈ 35,791,394`** — game engine integer overflow guard.

---

## Existing Rust Code

File: `crates/pob-calc/src/calc/offence_dot.rs`, lines 1–402

### What exists

**`calc_impale` (lines 16–65):**
- Reads `ImpaleChance`, clamps to `[0, 100]`.
- Queries `ImpaleEffect` base / 100 as stored-damage fraction (default 0.1).
- Queries `ImpaleStacksMax` base (default 5).
- `impale_dps = phys_hit × effect × max_stacks × (impale_chance / 100)`.
- Writes `ImpaleChance`, `ImpaleDPS`, `ImpaleStacks`.

**`calc_skill_dot` (lines 73–145):**
- Iterates over `DMG_TYPE_NAMES`, queries `{Type}Dot` base.
- Applies `{Type}Damage + Damage + DamageOverTime` INC/More.
- Applies `{Type}DotMultiplier + DotMultiplier` as dot multiplier.
- Accumulates into `total_dot`, writes `TotalDot`.

**`calc_combined_dps` (lines 154–204):**
- Reads `TotalDPS`, ignite/bleed/poison/dot/decay DPS, `ImpaleDPS`.
- Computes `TotalDotDPS` and `CombinedDPS`.

### What's missing / wrong

**`impaleStoredHitAvg`:**

1. **`impaleStoredHitAvg` is never accumulated.** The Rust does not track the pre-armour
   physical hit average across crit/non-crit passes. The oracle field `impaleStoredHitAvg`
   is never written. The Rust uses `PhysicalHitAverage` (post-resist) as a proxy, which is
   wrong: impale uses the pre-armour value so that its own armour reduction step is applied.

2. **`ImpaleModifier` is never written.** Lua writes `output.ImpaleModifier = 1 + impaleDMGModifier`
   (line 5326). The Rust writes only `ImpaleDPS` and `ImpaleStacks`, never `ImpaleModifier`.

3. **`ImpaleStacksMax` not written.** `globalOutput.ImpaleStacksMax` (line 5321) is absent.

4. **`ImpaleAdditionalDurationChance` not applied.** Lua multiplies `ImpaleStacksMax` by
   `(1 + ImpaleAdditionalDurationChance/100)` for the fractional-stack mechanic. The Rust
   uses the raw `ImpaleStacksMax` query.

5. **`ImpaleEffect` query is wrong.** Lua computes `storedExpectedDamageModifier = (1 + INC/100) × More`,
   including `ImpaleEffectOnBleed` bleed-weighted contribution. The Rust reads `ImpaleEffect`
   as a BASE mod (`sum_cfg(Base, "ImpaleEffect")`) without inc/more scaling, and the fallback
   is not how PoB determines the base.

6. **Armour reduction for impale not applied.** Lua applies a separate `impaleArmourReduction`
   step using `calcs.armourReductionF(enemyArmour, impaleHitDamageMod × impaleStoredHitAvg)`.
   The Rust skips this entirely.

7. **`EnemyImpalePhysicalDamageReduction` and `IgnoreEnemyImpalePhysicalDamageReduction` not
   queried.** These modifiers on the impale resist value are absent.

8. **`impaleTaken` (enemy physical damage taken for impale) not applied.** Lua queries enemy
   `DamageTaken + PhysicalDamageTaken + ReflectedDamageTaken` INC/More with `flags = Hit`.

9. **`impaleChance` crit-weighting not implemented.** Lua computes:
   `impaleChance = ImpaleChance/100 × (1−crit) + ImpaleChanceOnCrit/100 × crit`.
   The Rust uses raw `ImpaleChance` without crit weighting.

10. **`ImpaleDPS` formula is wrong.** Lua: `impaleStoredHitAvg × (ImpaleModifier−1) × hitChance/100 × dpsMultiplier × (HitSpeed or Speed) × quantityMultiplier`.
    Rust: `physHit × effect × maxStacks × impaleChance/100` — none of the Lua factors match.

**Skill DoT:**

11. **`dotCfg` flag construction not implemented.** Lua constructs a `dotCfg` with
    `ModFlag.Dot | skillCfg.flags` minus Area/Projectile/Spell/Attack/Hit flags based on
    `skillData.dotIs*` fields. The Rust queries DoT mods using the same `cfg` as hit damage.

12. **`dotTypeCfg` per-type keyword flag not added.** Lua adds `KeywordFlag.{Type}Dot` to
    each `dotTypeCfg` copy. Rust: add e.g. `PhysicalDot`, `FireDot` keyword flags for each
    type-specific DoT query.

13. **`aura` scaling factor not applied.** For aura skills, `AuraEffect` multiplies DoT DPS.

14. **`Override DotMultiplier` not checked.** Lua uses `Override(dotTypeCfg, "DotMultiplier")`
    before falling back to `Sum`. Rust uses only `Sum`.

15. **`{Type}Dot` from `preDotFunc` not handled.** When `output[damageType.."Dot"]` is
    already set (e.g. from Righteous Fire's pre-hook), the Lua uses a different accumulation
    path (adds both the existing value and the new total). The Rust ignores this.

16. **`DotCanStack` path not implemented.** For stackable DoTs, `TotalDot = TotalDotInstance
    × speed × duration × dpsMultiplier × quantityMultiplier`. Rust: `TotalDot = total_dot`
    (just the instance sum, no stack scaling).

17. **Ground-effect paths (`dotIsBurningGround`, `dotIsCausticGround`, `dotIsCorruptingBlood`)
    not handled.** The Rust does not zero `TotalDot` for ground-effect skills.

18. **`{Type}DotEffMult` (enemy resistance × taken mods) not applied.** The Lua computes
    `effMult` per type in `mode_effective` and multiplies `total` by it. The Rust ignores
    enemy resistance for non-ailment DoTs.

19. **`ImpaleHit` field in `field_groups.rs` is erroneous** and should be removed.

## What Needs to Change

1. **Accumulate `impaleStoredHitAvg` in the hit damage pass.** In the physical damage type
   branch of the crit/non-crit pass loop (mirrors CalcOffence.lua:3246–3252):
   ```rust
   // Before armour reduction is applied:
   if dtype == "Physical" {
       if pass == 1 { // crit pass
           output.impale_stored_hit_avg += phys_avg * (crit_chance / 100.0);
       } else { // non-crit pass
           output.impale_stored_hit_avg += phys_avg * (1.0 - crit_chance / 100.0);
       }
   }
   ```

2. **Compute `ImpaleModifier` in `calc_impale`.** Implement the full modifier chain:
   - `storedDamage = 0.1 × (1 + ImpaleEffect_inc/100) × round(ImpaleEffect_more, 2)`
   - `impaleDMGModifier = storedDamage × stacks × (1 - impaleResist/100) × impaleChance × impaleTaken`
   - Write `output.ImpaleModifier = 1.0 + impaleDMGModifier`

3. **Compute `ImpaleStacksMax` with `ImpaleAdditionalDurationChance`.**
   ```rust
   let max_stacks = base_stacks * (1.0 + add_dur_chance / 100.0);
   env.player.set_output("ImpaleStacksMax", max_stacks);
   ```

4. **Implement crit-weighted `impaleChance`.**
   ```rust
   let impale_chance = (impale_on_hit / 100.0).min(1.0) * (1.0 - crit_rate)
                     + (impale_on_crit / 100.0).min(1.0) * crit_rate;
   ```

5. **Apply impale armour reduction.** After computing `impaleHitDamageMod`:
   ```rust
   let armour_reduction = calcs_armour_reduction_f(enemy_armour, impale_hit_damage_mod * impale_stored_hit_avg);
   let impale_resist = (enemy_phys_reduction + enemy_impale_phys_reduction + armour_reduction)
                       .clamp(0.0, ENEMY_PHYS_DR_CAP);
   ```

6. **Query `impaleTaken` with `flags = Hit` cfg.**
   ```rust
   let hit_cfg = SkillCfg { flags: ModFlags::HIT, ..Default::default() };
   let impale_taken = (1.0 + enemy_db.sum_cfg(Inc, &hit_cfg, "DamageTaken", "PhysicalDamageTaken", "ReflectedDamageTaken") / 100.0)
                    * enemy_db.more_cfg(&hit_cfg, "DamageTaken", ...);
   ```

7. **Fix `ImpaleDPS` formula.**
   ```rust
   let impale_dps = impale_stored_hit_avg
                  * (impale_modifier - 1.0)
                  * hit_chance / 100.0
                  * dps_multiplier
                  * (hit_speed.unwrap_or(speed))
                  * quantity_multiplier;
   ```

8. **Construct `dotCfg` per skill.** Copy `skillCfg.flags | ModFlag.Dot`, strip Hit keyword,
   conditionally strip Area/Projectile/Spell/Attack/Hit flags based on `skillData.dotIs*`.
   Maintain `dotTakenCfg` (strip Spell when `skillData.dotIsSpell`).

9. **Add `{Type}Dot` keyword flag per iteration.** Per damage type, add the appropriate
   `KeywordFlag.{Type}Dot` to `dotTypeCfg`.

10. **Check `Override DotMultiplier` before `Sum`.**

11. **Apply `aura` scaling factor** for aura-type skills.

12. **Implement `DotCanStack` path for `TotalDot`.**
    `TotalDot = (TotalDotInstance × speed × duration × dpsMultiplier × quantityMultiplier).min(DotDpsCap)`.

13. **Implement ground-effect paths:** zero `TotalDot` when `dotIsBurningGround`,
    `dotIsCausticGround`, or `dotIsCorruptingBlood`.

14. **Apply `{Type}DotEffMult` in effective mode.** Query enemy resistance + taken mods per
    type and multiply into the DoT total.

15. **Remove `ImpaleHit` from `field_groups.rs`** — it is not a real oracle field.
