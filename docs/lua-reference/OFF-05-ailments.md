# OFF-05-ailments: Ignite, Bleed, and Poison

## Output Fields

All oracle-visible fields are written to `globalOutput` (the top-level pass output), not to
per-pass sub-tables. Per-pass writes to `output.BleedDPS` etc. are combined via
`combineStat` into `globalOutput` after the ailment pass loop.

| Field | Lua source | Notes |
|-------|-----------|-------|
| `IgniteChance` | CalcOffence.lua:4036 (via `calcAilmentDamage`), combined at line 5361 | Combined crit/non-crit chance; written via `output[type.."Chance"] = chance` |
| `IgniteDPS` | CalcOffence.lua:4920, combined at line 5362 | Single ignite DPS (average), capped at `DotDpsCap` |
| `IgniteDamage` | CalcOffence.lua:4943 | `IgniteDPS × IgniteDuration`; only written when `igniteCanStack` |
| `IgniteDuration` | CalcOffence.lua:4727 → `globalOutput.IgniteDuration` | Effective ignite duration in seconds |
| `IgniteEffMult` | CalcOffence.lua:4895 or 4905 → `globalOutput["IgniteEffMult"]` | Enemy resistance × taken mods; only in `mode_effective` |
| `BleedChance` | CalcOffence.lua:4036 (via `calcAilmentDamage`), combined at line 5349 | Combined crit/non-crit chance; only for attacks |
| `BleedDPS` | CalcOffence.lua:4296, combined at line 5350 | Weighted-average bleed DPS across active stacks |
| `BleedDamage` | CalcOffence.lua:4298 → `globalOutput.BleedDamage` | `BaseBleedDPS × BleedDuration` |
| `BleedDuration` | CalcOffence.lua:4138 → `globalOutput.BleedDuration` | Effective bleed duration in seconds |
| `BleedEffMult` | CalcOffence.lua:4282 → `globalOutput["BleedEffMult"]` | Enemy physical reduction × taken mods; only in `mode_effective` |
| `BleedStackPotential` | CalcOffence.lua:4161 → `globalOutput.BleedStackPotential` | `bleedStacks / maxStacks`; ratio of applied to max bleeds |
| `BleedStacks` | CalcOffence.lua:4297 → `globalOutput.BleedStacks` | Average active bleed stacks at constant attack rate |
| `BleedStacksMax` | CalcOffence.lua:4133 → `globalOutput.BleedStacksMax` | Max bleed stacks (1 without Crimson Dance, up to 8 with it) |
| `BleedRollAverage` | CalcOffence.lua:4193 → `globalOutput.BleedRollAverage` | Average % of max damage the active bleed achieves |
| `PoisonChance` | CalcOffence.lua:4036 (via `calcAilmentDamage`), combined at line 5351 | Combined crit/non-crit chance |
| `PoisonDPS` | CalcOffence.lua:4567 | Single-stack poison DPS, capped at `DotDpsCap` |
| `PoisonDamage` | CalcOffence.lua:4568 | `PoisonDPS × PoisonDuration` (single stack damage total) |
| `PoisonDuration` | CalcOffence.lua:4414 → `globalOutput.PoisonDuration` | Effective poison duration in seconds |
| `PoisonStacks` | CalcOffence.lua:4563 → `globalOutput.PoisonStacks` | Average active poison stacks at constant attack rate |
| `PoisonStacksMax` | *(not written — not a PoB output field; see note)* | `field_groups.rs` lists this, but PoB has no such field |

> **`PoisonStacksMax` note:** PoB does not write an output field named `PoisonStacksMax`.
> The Lua has `poisonStackLimit` (a local) and `maxPoisonStacks` (also a local) but neither
> is written to `globalOutput`. The `field_groups.rs` entry is erroneous. Verify against
> oracle JSON before implementing.

> **`globalOutput` vs `output`:** Inside the ailment loop, `globalOutput` is an alias for
> the outer (pass-level) `output` table: `globalOutput, globalBreakdown = output, breakdown`
> (line 3867). Fields written to `globalOutput.IgniteDuration` etc. persist on the main
> `actor.output` table. Fields written to the loop-local `output` (e.g. `output.BleedDPS`)
> are per-weapon-pass and get combined afterward via `combineStat`.

## Dependencies

- `OFF-01-base-damage` — `{Type}HitAverage` and `{Type}CritAverage` must be computed.
- `OFF-02-conversion` — source damage must reflect conversions (e.g. ignite from physical
  via `PhysicalCanIgnite`).
- `OFF-03-crit-hit` — `CritChance`, `HitChance` used in chance formulas and stack counts.
- `OFF-04-speed-dps` — `Speed` / `HitSpeed` used in stack count formulas; `Duration`
  needed for bleed/poison/ignite duration calculations.

## Lua Source

File: `third-party/PathOfBuilding/src/Modules/CalcOffence.lua`  
Commit: `454eff8c85d24356d9b051d596983745ed367476`

Primary line ranges:
- **Ailment pass loop header:** 3865–3868
- **Chance-on-hit/crit setup** (all ailments): 3892–3983
- **`calcAilmentDamage` helper** (writes `{Type}Chance`): 4029–4107
- **Bleed:** 4110–4380 (chance, stacks, `BleedStacksMax/Duration/StackPotential/RollAverage/DPS`)
- **Poison:** 4382–4686 (chance, stacks, `PoisonDuration/Stacks/DPS/Damage`)
- **Ignite:** 4688–5044 (chance, stacks, `IgniteStacksMax/Duration/DPS/Damage/EffMult`)
- **`combineStat` for attacks:** 5347–5393

## Annotated Lua

### 1. Ailment chance setup (lines 3892–3983, per-pass)

Each pass first computes per-condition chances. The `cfg.skillCond["CriticalStrike"]`
toggle pattern is used to query crit-condition mods:

```lua
-- Crit context: query mods that only apply on crit
cfg.skillCond["CriticalStrike"] = true
if not skillFlags.attack or skillModList:Flag(cfg, "CannotBleed") then
    output.BleedChanceOnCrit = 0
else
    -- Clamped to [0, 100]. Enemy "self bleed chance" mods (curses) add on.
    output.BleedChanceOnCrit = m_min(100,
        skillModList:Sum("BASE", cfg, "BleedChance")
        + enemyDB:Sum("BASE", nil, "SelfBleedChance"))
end
-- ... same pattern for PoisonChanceOnCrit, IgniteChanceOnCrit

cfg.skillCond["CriticalStrike"] = false  -- reset for non-crit queries
-- ... BleedChanceOnHit, PoisonChanceOnHit, IgniteChanceOnHit
```

> **`cfg.skillCond["CriticalStrike"]` toggle:** This is how PoB gates crit-only mods. When
> `CriticalStrike = true`, `Sum("BASE", cfg, "BleedChance")` includes mods tagged with
> `Condition:CriticalStrike`. In Rust, pass a `SkillCfg` with `skill_cond.crit = true` for
> the crit branch and `false` for the non-crit branch.

For elemental ailments (Ignite, Chill, Freeze, Shock, etc.), PoB uses a loop:

```lua
for _, ailment in ipairs(elementalAilmentTypeList) do
    local chance = skillModList:Sum("BASE", cfg, "Enemy"..ailment.."Chance")
                   + enemyDB:Sum("BASE", nil, "Self"..ailment.."Chance")
    -- Chill is always 100%:
    if ailment == "Chill" then chance = 100 end
    -- Account for enemy immunity/avoidance:
    local avoid = 1 - m_min(enemyDB:Flag(nil, ailment.."Immune", ...) and 100 or 0
                             + enemyDB:Sum("BASE", nil, "Avoid"..ailment, ...), 100) / 100
    chance = chance * avoid
    if skillFlags.hit and not (skillModList:Flag(cfg, "Cannot"..ailment) or avoid == 0) then
        output[ailment.."ChanceOnHit"] = m_min(100, chance)
        -- Crits always apply elemental ailments unless "CritsDontAlways{Ailment}" flag:
        output[ailment.."ChanceOnCrit"] = 100  -- default
    else
        output[ailment.."ChanceOnHit"] = 0
        output[ailment.."ChanceOnCrit"] = 0
    end
end
```

> **`output[ailment.."ChanceOnHit"]`** — dynamic table key construction via string
> concatenation. `ailment` is `"Ignite"`, `"Chill"`, etc. Rust: use a match or iterate
> over an enum. `output["IgniteChanceOnHit"]` is the `Ignite` iteration result.

After the elemental ailment loop, enemy immunity/avoidance is applied in
`env.mode_effective` (line 3975–3983):

```lua
if env.mode_effective then
    for _, ailment in ipairs(ailmentTypeList) do
        local mult = enemyDB:Flag(nil, ailment.."Immune") and 0
                     or 1 - enemyDB:Sum("BASE", nil, "Avoid"..ailment) / 100
        output[ailment.."ChanceOnHit"] = output[ailment.."ChanceOnHit"] * mult
        output[ailment.."ChanceOnCrit"] = output[ailment.."ChanceOnCrit"] * mult
    end
end
```

> **`ailmentMode == "CRIT"` / `AilmentsOnlyFromCrit`:** Zero out on-hit chances (line
> 3987–3991). `AilmentsAreNeverFromCrit` (Elemental Overload): copy on-hit chance to
> on-crit, meaning crits use the same chance as normal hits (line 3996–4000).

---

### 2. `calcAilmentDamage` — the shared chance/source-weighting helper (lines 4029–4107)

This function is called for Bleed, Poison, and Ignite. It:
1. Reads `output[type.."ChanceOnHit"]` and `output[type.."ChanceOnCrit"]`
2. Computes the combined effective chance and weighted source damage
3. **Writes `output[type.."Chance"]`** — this is the `IgniteChance`, `BleedChance`,
   `PoisonChance` oracle field

```lua
local function calcAilmentDamage(type, sourceCritChance, sourceHitDmg, sourceCritDmg, hideFromBreakdown)
    local chanceOnHit, chanceOnCrit = output[type.."ChanceOnHit"], output[type.."ChanceOnCrit"]
    local chanceFromHit  = chanceOnHit  * (1 - sourceCritChance / 100)
    local chanceFromCrit = chanceOnCrit * sourceCritChance / 100
    local chance = chanceFromHit + chanceFromCrit
    output[type.."Chance"] = chance     -- ← writes IgniteChance / BleedChance / PoisonChance

    -- Weighted average source damage:
    local baseFromHit  = sourceHitDmg  * chanceFromHit  / (chanceFromHit + chanceFromCrit)
    local baseFromCrit = sourceCritDmg * chanceFromCrit / (chanceFromHit + chanceFromCrit)
    local baseVal = baseFromHit + baseFromCrit
    -- "As though dealing" multiplier (e.g. Unbound Ailments)
    local sourceMult = skillModList:More(cfg, type.."AsThoughDealing")
    -- Returns the weighted base damage
    return baseVal
end
```

> **`sourceCritChance`** is NOT the overall crit chance — for bleed it is the
> "ailment crit chance" accounting for over-stacking (line 4267):
> `ailmentCritChance = 100 × (1 − (1 − critChance/100)^max(stackPotential, 1))`
> This is the probability that at least one of the active bleeds was applied by a crit.
> For ignite (line 4882): `ailmentCritChance = 100 × (1 − (1 − critChance/100)^max(1, igniteStacks))`.
> For poison (lines 4542, 4544): uses `output.CritChance` directly (no stack adjustment).

---

### 3. Bleed (lines 4110–4380)

#### 3a. `BleedStacksMax`, `BleedDuration`, `BleedStackPotential`, `BleedRollAverage`

```lua
-- Max stacks: override or sum (Crimson Dance = 8)
local maxStacks = skillModList:Override(cfg, "BleedStacksMax")
               or skillModList:Sum("BASE", cfg, "BleedStacksMax")
globalOutput.BleedStacksMax = maxStacks

-- Duration formula:
-- durationBase × durationMod / rateMod × debuffDurationMult
-- durationMod = calcLib.mod(... EnemyBleedDuration, EnemyAilmentDuration, ...) × enemy self-duration mods / BleedExpireRate
-- rateMod = calcLib.mod(skillModList, cfg, "BleedFaster") + enemy self-bleed-faster
local durationBase = skillData.bleedDurationIsSkillDuration
                     and skillData.duration  -- some skills use their own duration for bleed
                     or data.misc.BleedDurationBase  -- default 5.0s
globalOutput.BleedDuration = durationBase * durationMod / rateMod * debuffDurationMult
```

> **`data.misc.BleedDurationBase = 5.0`** (hardcoded in PoB data).
> **`debuffDurationMult`** is computed once per pass before the ailment section:
> `1 / max(BuffExpirationSlowCap, calcLib.mod(enemyDB, skillCfg, "BuffExpireFaster"))`.
> Only non-1 in `env.mode_effective`. In Rust this must be computed before the ailment
> functions are called.

```lua
-- Stack count at steady state:
local bleedChance = output.BleedChanceOnHit / 100 * (1 - output.CritChance / 100)
                  + output.BleedChanceOnCrit / 100 * output.CritChance / 100
local bleedStacks = output.HitChance / 100 * bleedChance * skillData.dpsMultiplier
if (globalOutput.HitSpeed or globalOutput.Speed) > 0 then
    bleedStacks = bleedStacks * globalOutput.BleedDuration * (globalOutput.HitSpeed or globalOutput.Speed)
end
-- Totem scaling:
if skillFlags.totem then
    bleedStacks = bleedStacks * activeTotems
end
-- Config override:
if configStacks > 0 then bleedStacks = configStacks end

globalOutput.BleedStackPotential = overrideStackPotential or (bleedStacks / maxStacks)
```

> **`bleedStacks`** formula: `hitChance × bleedChance × dpsMultiplier × BleedDuration × speed`.
> This is the average number of active bleeds — `duration × rate`.
> **`overrideStackPotential`**: if a "BleedStackPotentialOverride" is set, use that / maxStacks.

```lua
-- Roll average: what fraction of max damage the active bleed achieves
local bleedRollAverage
if globalOutput.BleedStackPotential > 1 then
    -- Over-stacked: the strongest active bleed is biased toward higher damage rolls
    bleedRollAverage = (bleedStacks - (maxStacks - 1) / 2) / (bleedStacks + 1) * 100
else
    bleedRollAverage = 50  -- assume middle of range
end
globalOutput.BleedRollAverage = bleedRollAverage
```

> **`BleedRollAverage`**: percentage of max damage. Value 50 = average roll (min+max)/2.
> The formula `(stacks - (max-1)/2) / (stacks + 1) * 100` approximates the expected maximum
> order statistic when stacks >> maxStacks. Rust: compute `f64` directly.

#### 3b. `BleedDPS`, `BleedStacks`, `BleedDamage`, `BleedEffMult`

```lua
-- Sub-pass 1: non-crit source damage (dotCfg.skillCond["CriticalStrike"] = false)
-- Sub-pass 2: crit source damage (dotCfg.skillCond["CriticalStrike"] = true)
-- calcAilmentSourceDamage(activeSkill, output, dotCfg, ...) computes
--   physical hit min/max × convMult (the portion not converted away)

-- Effective multiplier from enemy:
local resist = m_min(m_max(0, enemyDB:Sum("BASE", nil, "PhysicalDamageReduction")),
                     data.misc.EnemyPhysicalDamageReductionCap)
local takenInc = enemyDB:Sum("INC", dotCfg, "DamageTaken", "DamageTakenOverTime",
                              "PhysicalDamageTaken", "PhysicalDamageTakenOverTime")
local takenMore = enemyDB:More(dotCfg, "DamageTaken", ...)
effMult = (1 - resist / 100) * (1 + takenInc / 100) * takenMore
globalOutput["BleedEffMult"] = effMult  -- only in mode_effective

-- basePercent: normally data.misc.BleedPercentBase (70% → 0.70)
-- Final DPS:
-- activeBleeds = min(bleedStacks, maxStacks)
-- effectMod = calcLib.mod(skillModList, dotCfg, "AilmentEffect")
output.BaseBleedDPS = baseBleedDps * effectMod * rateMod * activeBleeds * effMult
output.BleedDPS = m_min(output.BaseBleedDPS, data.misc.DotDpsCap)  -- line 4296
globalOutput.BleedStacks = bleedStacks   -- line 4297
globalOutput.BleedDamage = output.BaseBleedDPS * globalOutput.BleedDuration  -- line 4298
```

> **`data.misc.BleedPercentBase = 70`** (bleed deals 70% of physical hit per second).
> Rust: `const BLEED_PERCENT_BASE: f64 = 0.70`.

> **`dotCfg`** is a custom cfg built at line 4111–4122 with flags:
> `ModFlag.Dot | ModFlag.Ailment | weapon_mask | MeleeHit (if melee)`, and keyword flags
> `Bleed | Ailment | PhysicalDot`. This ensures DoT-scoped mods apply. In Rust, construct
> a `DotCfg` with these flags for DoT mod queries.

---

### 4. Poison (lines 4382–4686)

```lua
-- Duration: base 2.0s × durationMod / rateMod × debuffDurationMult
local durationBase = skillData.poisonDurationIsSkillDuration
                     and skillData.duration or data.misc.PoisonDurationBase  -- 2.0s
globalOutput.PoisonDuration = durationBase * durationMod / rateMod * debuffDurationMult

-- Poison stacks — note: includes additionalPoisonStacks, quantityMultiplier:
local additionalPoisonStacks = 1
if not skillModList:Flag(nil, "CannotMultiplePoison") then
    additionalPoisonStacks = 1
        + m_min(skillModList:Sum("BASE", cfg, "AdditionalPoisonChance") / 100, 1)
        + skillModList:Sum("BASE", cfg, "AdditionalPoisonStacks")
end
local PoisonStacks = output.HitChance / 100 * poisonChance * additionalPoisonStacks
                   * skillData.dpsMultiplier * (skillData.stackMultiplier or 1) * quantityMultiplier
if (globalOutput.HitSpeed or globalOutput.Speed) > 0 then
    PoisonStacks = PoisonStacks * globalOutput.PoisonDuration * (globalOutput.HitSpeed or globalOutput.Speed)
end
-- PoisonStackLimit cap (Mageblood flask, etc.):
if poisonStackLimit and PoisonStacks > poisonStackLimit then ...end
globalOutput.PoisonStacks = PoisonStacks  -- line 4563

-- Source damage: physical + chaos + optional lightning/cold/fire (with flags like LightningCanPoison)
-- Collected across two sub-passes (non-crit and crit)

-- PoisonDotMulti and CritPoisonDotMulti:
output.PoisonDotMulti = 1 + (Override(dotCfg, "DotMultiplier")
                              or Sum("BASE", dotCfg, "DotMultiplier")
                                 + Sum("BASE", dotCfg, "ChaosDotMultiplier")) / 100

-- effMult (Chaos resistance):
local resist = calcResistForType("Chaos", dotCfg)
effMult = (1 - resist / 100) * (1 + takenInc / 100) * takenMore
globalOutput["PoisonEffMult"] = effMult

-- Single-stack DPS:
-- data.misc.PoisonPercentBase = 30 (poison deals 30% per second)
-- baseVal = calcAilmentDamage("Poison", critChance, sourceHit, sourceCrit) × 0.30
-- singlePoisonDPS = baseVal × effectMod × rateMod × effMult
output.PoisonDPS = singlePoisonDPSCapped        -- line 4567
output.PoisonDamage = singlePoisonDPSCapped × globalOutput.PoisonDuration  -- line 4568
-- TotalPoisonDPS = singlePoisonDPS × PoisonStacks (capped)
output.TotalPoisonDPS = PoisonDPSCapped         -- line 4596
```

> **`data.misc.PoisonPercentBase = 30`**. Rust: `const POISON_PERCENT_BASE: f64 = 0.30`.

> **`additionalPoisonStacks`**: "inflict X additional poisons" mods (e.g. Unbound Ailments
> with Viper Strike, certain uniques). The Rust does not implement this.

> **`stackMultiplier`**: gem-data field (not a mod). Used for skills like Viper Strike.
> The Rust does not implement this.

---

### 5. Ignite (lines 4688–5044)

```lua
-- Duration: base 4.0s
-- rateMod = calcLib.mod(skillModList, cfg, "IgniteBurnFaster") + enemySelf
--         / calcLib.mod(skillModList, cfg, "IgniteBurnSlower")
-- durationMod = max(calcLib.mod(...EnemyIgniteDuration, ...ElementalAilmentDuration...), 0)
globalOutput.IgniteDuration = durationBase * durationMod / rateMod * debuffDurationMult

-- Stack count (IgniteStacksMax is 1 by default; IgniteCanStack keystone or Emberwake):
local maxStacks = 1
if skillFlags.igniteCanStack then
    maxStacks = skillModList:Override(cfg, "IgniteStacks")
             or (1 + skillModList:Sum("BASE", cfg, "IgniteStacks"))
end
globalOutput.IgniteStacksMax = maxStacks

-- Source damage: fire (default) + physical/lightning/cold/chaos if respective flags set
-- Collected across sub-passes (non-crit and crit)
-- igniteRollAverage: same formula as bleedRollAverage (50 if stack potential ≤ 1)

-- CritIgniteDotMulti (for crit sub-pass):
output.CritIgniteDotMulti = 1 + (Override(dotCfg, "DotMultiplier")
                                  or Sum("BASE", dotCfg, "DotMultiplier")
                                     + Sum("BASE", dotCfg, "FireDotMultiplier")) / 100
-- IgniteDotMulti (for non-crit sub-pass):
output.IgniteDotMulti = ... (same formula but queried without CriticalStrike condition)

-- effMult: fire resistance (or chaos if IgniteToChaos keystone)
if skillModList:Flag(cfg, "IgniteToChaos") then  -- e.g. Elementalist Shaper of Storms
    resist = calcResistForType("Chaos", dotCfg)
else
    resist = calcResistForType("Fire", dotCfg)
    -- ElementalDamageTaken also applies to ignite
end
globalOutput["IgniteEffMult"] = effMult

-- data.misc.IgnitePercentBase = 90 (ignite deals 90% per second)
-- baseVal = calcAilmentDamage("Ignite", ailmentCritChance, sourceHit, sourceCrit) × 0.90
-- activeIgnites = min(igniteStacks, maxStacks)
-- IgniteDPS = baseVal × effectMod × rateMod × activeIgnites × effMult (capped)
output.IgniteDPS = IgniteDPSCapped                          -- line 4920
globalOutput.IgniteDamage = output.IgniteDPS × globalOutput.IgniteDuration  -- line 4941
if skillFlags.igniteCanStack then
    output.IgniteDamage = output.IgniteDPS × globalOutput.IgniteDuration    -- line 4943
    output.IgniteStacksMax = maxStacks                                        -- line 4944
end
```

> **`data.misc.IgnitePercentBase = 90`**. Rust: `const IGNITE_PERCENT_BASE: f64 = 0.90`.

> **`ailmentCritChance` for ignite** (line 4882):
> `100 × (1 − (1 − critChance/100)^max(1, igniteStacks))`
> This is the probability that at least one of the `igniteStacks` fires was a crit hit.
> For ignite it uses `igniteStacks` (not `globalOutput.IgniteStackPotential`).

> **`IgniteBurnSlower`** in the rate denominator: unique to ignite vs bleed/poison.
> `rateMod = burnFasterMod / burnSlowerMod`. Higher `rateMod` → shorter duration, higher DPS.

---

### 6. `combineStat` for attack skills (lines 5347–5393)

After the per-pass ailment loop, for `isAttack`:

```lua
combineStat("BleedChance", "AVERAGE")   -- avg of MH and OH chances
combineStat("BleedDPS", "CHANCE_AILMENT", "BleedChance")  -- weighted by HitChance × BleedChance
combineStat("PoisonChance", "AVERAGE")
combineStat("PoisonDPS", "CHANCE", "PoisonChance")
combineStat("PoisonDamage", "CHANCE", "PoisonChance")
combineStat("IgniteChance", "AVERAGE")
combineStat("IgniteDPS", "CHANCE_AILMENT", "IgniteChance")
```

`"CHANCE_AILMENT"` mode (lines 2035–2069): uses hit chance × ailment chance to weight DPS
contributions from each hand, then picks max/min instances based on the `{Name}Stacks` /
`{Name}StacksMax` ratio to correctly blend the DPS contributions.

> **Rust note:** per-hand ailment DPS fields (`output.MainHand.BleedDPS`, etc.) are set
> during the per-pass loop. They are then combined by `combineStat` into the global
> `output.BleedDPS`. Until per-pass sub-tables are implemented (see OFF-03), both fields
> will come from the single-hand (non-dual-wield) path.

---

## Existing Rust Code

File: `crates/pob-calc/src/calc/offence_ailments.rs`, lines 1–836

### What exists

All three ailments (`calc_ignite`, `calc_bleed`, `calc_poison`) have basic implementations:

**`calc_ignite` (lines 22–159):**
- Reads `EnemyIgniteChance` → `IgniteChanceOnHit`.
- Source: `FireHitAverage` (or all types with `ShaperOfFlames`).
- Base DPS: `source × 0.9 / 4.0`.
- Applies `FireDamage + BurningDamage + ElementalDamage + Damage + AilmentDamage + DamageOverTime` INC/More.
- Applies `FireDotMultiplier + DotMultiplier` as dot multiplier.
- Duration: `4.0 × (1 + EnemyIgniteDuration_inc/100) × EnemyIgniteDuration_more`.
- Writes `IgniteDPS`, `IgniteDuration`, `IgniteDamage`.

**`calc_bleed` (lines 167–291):**
- Reads `BleedChance` → `BleedChanceOnHit`.
- Source: `PhysicalHitAverage`.
- Base DPS: `source × 0.7 / 5.0`.
- Applies Physical/Bleed/Damage/Ailment/DoT INC/More + PhysicalDot/Dot multiplier.
- Duration: `5.0 × (1 + EnemyBleedDuration_inc/100)` (missing `More` term).
- Crimson Dance: computes stacks capped at 8.
- Writes `BleedDPS`, `BleedMovingDPS`, `BleedDuration`, (sometimes) `BleedStacks`.

**`calc_poison` (lines 300–422):**
- Reads `PoisonChance` → `PoisonChanceOnHit`.
- Source: `PhysicalHitAverage + ChaosHitAverage`.
- Base DPS: `source × 0.3 / 2.0`.
- Applies Chaos/Poison/Damage/Ailment/DoT INC/More + ChaosDot/Dot multiplier.
- Duration: `2.0 × (1 + EnemyPoisonDuration_inc/100) × EnemyPoisonDuration_more`.
- Stacks: `speed × hitChance × poisonChance × duration`.
- Writes `PoisonDPS`, `TotalPoisonDPS`, `PoisonDuration`, `PoisonStacks`, `PoisonDamage`.

### What's missing / wrong

**All three ailments:**

1. **`{Type}Chance` (combined) not written.** Lua writes `output[type.."Chance"]` inside
   `calcAilmentDamage` (line 4036). The Rust writes only `{Type}ChanceOnHit` (the pre-crit
   value), never the combined crit-weighted `IgniteChance` / `BleedChance` / `PoisonChance`
   that the oracle asserts. These are the oracle fields.

2. **`{Type}EffMult` not computed.** Enemy resistance × taken mods multiplier. Written to
   `globalOutput["IgniteEffMult"]`, `globalOutput["BleedEffMult"]`, `globalOutput["PoisonEffMult"]`.
   The Rust has no `effMult` calculation for any ailment.

3. **Chance modelled incorrectly.** Lua uses per-condition chance (`ChanceOnHit` ×
   non-crit probability + `ChanceOnCrit` × crit probability), incorporating `SelfBleedChance`
   enemy mods and `AilmentsOnlyFromCrit` / `AilmentsAreNeverFromCrit` flags. The Rust uses a
   single flat `Sum("BASE", cfg, "BleedChance")` without the per-condition or enemy modifiers.

4. **Source damage calculation is incorrect.** Lua uses `calcAilmentSourceDamage` which calls
   `calcDamage` with the dotCfg to get pre-resist physical/chaos/fire damage (the `convMult`
   portion that stays as the original type). The Rust uses post-conversion, post-resist
   `{Type}HitAverage` fields, which already have resistance applied and don't use the DoT
   cfg. This produces wrong values for builds with penetration, enemy resistance, or
   non-standard conversions.

5. **`AilmentEffect` modifier not applied.** `calcLib.mod(skillModList, dotCfg, "AilmentEffect")`
   is a multiplicative modifier that scales all ailment DPS. The Rust ignores it.

6. **`rateMod` (BleedFaster/IgniteBurnFaster/PoisonFaster) not applied.** This multiplicative
   burn/bleed/poison rate modifier affects both duration (divides) and DPS (multiplies).
   The Rust ignores it for all ailments. For bleed: `durationBase × durationMod / rateMod`.

7. **`debuffDurationMult` not applied to duration.** The `BuffExpireFaster` enemy debuff
   modifier shortens ailment durations. The Rust does not compute or apply this.

8. **`dotCfg` (DoT-scoped skill config) not used.** PoB builds a separate `dotCfg` with
   `ModFlag.Dot | ModFlag.Ailment | weapon_mask` for querying ailment-specific mods. The
   Rust queries mod stats using the normal `cfg`, which will over- or under-count mods that
   are scoped to DoT or to hits only.

**Ignite-specific:**

9. **`IgniteStacksMax` and ignite stack count not computed.** Lua computes
   `globalOutput.IgniteStacksMax` and the per-stack logic for over-stacking builds
   (e.g. Emberwake). The Rust always assumes 1 ignite stack.

10. **`ailmentCritChance` for ignite not computed.** The weighted crit probability
    `100 × (1 − (1 − p)^stacks)` is missing; the Rust passes raw `crit_chance` to the
    source damage calculation.

11. **`IgniteBurnSlower` divisor missing from `rateMod`.** Lua divides `burnFasterMod`
    by `burnSlowerMod`. The Rust has no `IgniteBurnSlower` query.

12. **`IgniteToChaos` keystone not handled.** Ignite effMult should use chaos resistance
    instead of fire resistance when this flag is set.

13. **`PhysicalCanIgnite` / `LightningCanIgnite` / `ColdCanIgnite` / `ChaosCanIgnite`
    flags not checked.** Ignite source damage can include non-fire damage types.

**Bleed-specific:**

14. **`BleedStacksMax`, `BleedStackPotential`, `BleedRollAverage` not computed.**
    These three fields are entirely absent from the Rust.

15. **`BleedDuration` missing `More` term.** Rust: `5.0 × (1 + inc/100)`. Lua also
    multiplies by `durationMod` (which includes a More product) and divides by `rateMod`.

16. **`BleedDamage` not written.** `BaseBleedDPS × BleedDuration`.

17. **`BleedStacksMax` from `CrimsonDance` hardcoded to 8.** In Lua it is queried via
    `skillModList:Override(cfg, "BleedStacksMax") or skillModList:Sum("BASE", cfg, "BleedStacksMax")`.

18. **Weighted roll average not used.** Lua weights the bleed damage by `bleedRollAverage`
    which shifts toward higher damage when over-stacked. The Rust assumes always-average.

**Poison-specific:**

19. **`additionalPoisonStacks` not computed.** "Inflict X additional poisons" mods
    (Unbound Ailments, Viper Strike, etc.) scale both the stack count and the per-hit
    probability.

20. **`stackMultiplier` (gem data) not applied to stacks.**

21. **`PoisonStackLimit` cap not applied.**

## What Needs to Change

1. **Build per-condition crit/non-crit chance framework.** Toggle `cfg.skill_cond.crit`
   true/false to query `BleedChance`, `PoisonChance`, `EnemyIgniteChance` in both crit and
   non-crit contexts. Include `SelfBleedChance` / `SelfPoisonChance` enemy mods.

2. **Implement `calcAilmentDamage` equivalent.** Compute combined `{Type}Chance` as:
   `chanceOnHit × (1 − crit_pct) + chanceOnCrit × crit_pct`
   and write it to output. This is what the oracle asserts.

3. **Build `dotCfg` per ailment.** Construct a `SkillCfg` with flags
   `Dot | Ailment | weapon_mask | (MeleeHit if melee)` and keyword flags
   `{Ailment} | Ailment | {Type}Dot`. Use this for all ailment DoT mod queries.

4. **Compute source damage via DoT config.** Use `calcAilmentSourceDamage` approach:
   call the per-type damage calculation (Physical/Fire/etc. with `convMult` applied) via
   the `dotCfg`, not post-resist hit averages.

5. **Apply `AilmentEffect` mod.** `calcLib.mod(skillModList, dotCfg, "AilmentEffect")` as
   a multiplier on base ailment DPS.

6. **Apply `rateMod` to all ailments:**
   - Bleed: `calcLib.mod(skillModList, cfg, "BleedFaster") + enemy SelfBleedFaster / 100`
   - Poison: `calcLib.mod(skillModList, cfg, "PoisonFaster") + enemy SelfPoisonFaster / 100`
   - Ignite: `calcLib.mod(..., "IgniteBurnFaster") / calcLib.mod(..., "IgniteBurnSlower")`
   Duration formula: `durationBase × durationMod / rateMod × debuffDurationMult`.

7. **Compute and apply `debuffDurationMult`.**
   `1 / max(BuffExpirationSlowCap, calcLib.mod(enemyDB, skillCfg, "BuffExpireFaster"))`.

8. **Implement `effMult` for all ailments:**
   - Bleed: physical damage reduction + PhysicalDamageTaken/DamageTaken inc/more from enemy
   - Poison: chaos resistance + ChaosDamageTaken/DamageTaken inc/more from enemy
   - Ignite: fire resistance + FireDamageTaken/ElementalDamageTaken inc/more (or chaos if IgniteToChaos)
   Write to `globalOutput["BleedEffMult"]`, `globalOutput["PoisonEffMult"]`, `globalOutput["IgniteEffMult"]`.

9. **Implement bleed weighted stack mechanics:**
   - `BleedStacksMax`: query `Override(cfg, "BleedStacksMax") or Sum("BASE", cfg, "BleedStacksMax")`
   - `bleedStacks`: `hitChance × bleedChance × dpsMultiplier × BleedDuration × speed`
   - `BleedStackPotential`: `bleedStacks / maxStacks`
   - `BleedRollAverage`: 50 if ≤ 1 stack potential; else `(stacks − (max−1)/2) / (stacks+1) × 100`
   - `activeBleeds = min(bleedStacks, maxStacks)` — multiply base DPS by this
   - Write `BleedStacks = bleedStacks`, `BleedDamage = BaseBleedDPS × BleedDuration`.

10. **Implement `ailmentCritChance` for bleed and ignite.**
    Bleed: `100 × (1 − (1 − crit_pct)^max(stackPotential, 1))`
    Ignite: `100 × (1 − (1 − crit_pct)^max(1, igniteStacks))`
    Use this as `sourceCritChance` in the weighted damage calculation.

11. **Implement poison `additionalPoisonStacks` and `stackMultiplier`.**

12. **Implement `PoisonStackLimit` cap in stack count.**

13. **Implement `IgniteCanStack` / Emberwake for `IgniteStacksMax` and multi-ignite DPS.**

14. **Handle multi-type ignite sources** (`PhysicalCanIgnite`, `LightningCanIgnite`,
    `ColdCanIgnite`, `ChaosCanIgnite`) by accumulating source damage across types.

15. **Remove the `PoisonStacksMax` entry from `field_groups.rs`** — it does not exist in
    the oracle JSON. Confirm by checking all oracle expected files before removing.
