# OFF-03-crit-hit: Critical Strike, Hit Chance, and Per-Hand Crit Stats

## Output Fields

| Field | Source | Notes |
|-------|--------|-------|
| `CritChance` | CalcOffence.lua:2839–2935 | Effective crit chance including lucky/unlucky/cap; multiplied by AccuracyHitChance in effective mode |
| `CritMultiplier` | CalcOffence.lua:2984–3004 | `1 + extraDamage`; min 1.0; affected by enemy self-crit-multi mods |
| `CritEffect` | CalcOffence.lua:3007 | `1 - critRate + critRate * CritMultiplier`; overall damage multiplier from crits |
| `CritDegenMultiplier` | *(see note below)* | **Does not exist in Lua output** — the Lua field is `BonusCritDotMultiplier` |
| `AccuracyHitChance` | CalcOffence.lua:2133–2144 | Hit-before-block accuracy check; `calcs.hitChance(evasion, accuracy) * HitChanceMod` |
| `MainHand.CritChance` | CalcOffence.lua:3692 | Per-pass CritChance for main hand (set inside the per-pass loop, combined via `combineStat`) |
| `MainHand.CritMultiplier` | CalcOffence.lua:3693 | Per-pass CritMultiplier for main hand |
| `OffHand.CritChance` | CalcOffence.lua:3692 | Per-pass CritChance for off hand |
| `OffHand.CritMultiplier` | CalcOffence.lua:3693 | Per-pass CritMultiplier for off hand |
| `MeleeNotHitChance` | CalcDefence.lua:1638 | Combined evade+dodge+avoid for melee attacks *(in CalcDefence, not CalcOffence)* |
| `ProjectileNotHitChance` | CalcDefence.lua:1639 | Combined evade+dodge+avoid+projectile-avoid *(in CalcDefence, not CalcOffence)* |

> **`CritDegenMultiplier` naming note:** The Lua writes `output.BonusCritDotMultiplier` (line
> 3008), not `CritDegenMultiplier`. The oracle JSON files confirm `BonusCritDotMultiplier` is
> the actual key. The `field_groups.rs` entry `"CritDegenMultiplier"` appears to be a
> mislabelled alias. When implementing, verify against oracle JSON and use
> `BonusCritDotMultiplier` as the actual field name.

## Dependencies

- `OFF-01-base-damage` — speed and skill infrastructure must be set up.
- `OFF-02-conversion` — not a direct prerequisite, but the per-pass loop that executes
  crit calculation runs inside the same damage-pass loop.
- `DEF-02-armour-evasion-es-ward` / `DEF-03-block-suppression` — enemy evasion and
  block chance must be set in `env.enemy.output` before `AccuracyHitChance` is correct.

## Lua Source

File: `third-party/PathOfBuilding/src/Modules/CalcOffence.lua`  
Commit: `454eff8c85d24356d9b051d596983745ed367476`

Primary line ranges:
- **Accuracy calculation:** 2082–2132 (inside `for _, pass in ipairs(passList)`)
- **AccuracyHitChance write:** 2133–2152
- **`calcs.hitChance()` definition:** CalcDefence.lua:32–38
- **CritChance main block:** 2837–2982
- **CritMultiplier + CritEffect:** 2983–3015
- **`combineStat` for crit fields:** 3691–3693
- **MeleeNotHitChance / ProjectileNotHitChance:** CalcDefence.lua:1636–1643

## Annotated Lua

### 1. Accuracy → `AccuracyHitChance` (CalcOffence.lua:2082–2152)

All calculations below happen **inside** `for _, pass in ipairs(passList) do`, which means
they run once per weapon pass (Main Hand, Off Hand). The `output` local inside the loop
refers to the pass-scoped sub-table (e.g. `output.MainHand`), not the global output.

```lua
-- Line 2082: passList loop — cfg, output, source are pass-local from here on.
local source, output, cfg, breakdown = pass.source, pass.output, pass.cfg, pass.breakdown

-- Lines 2092–2098: Accuracy stat query
-- skillModList queries with cfg (pass-specific), yielding per-weapon values
local base = skillModList:Sum("BASE", cfg, "Accuracy")
-- AccuracyVsEnemy: a separate multi-stat query that adds accuracy vs enemy mods
local baseVsEnemy = skillModList:Sum("BASE", cfg, "Accuracy", "AccuracyVsEnemy")
local inc = skillModList:Sum("INC", cfg, "Accuracy")
local incVsEnemy = skillModList:Sum("INC", cfg, "Accuracy", "AccuracyVsEnemy")
-- NOTE: More() signature is ("MORE", cfg, ...) — unusual; the first arg is the mod type string
local more = skillModList:More("MORE", cfg, "Accuracy")
local moreVsEnemy = skillModList:More("MORE", cfg, "Accuracy", "AccuracyVsEnemy")
```

> **Lua gotcha:** `skillModList:More("MORE", cfg, "Accuracy")` — normally `:More()` takes
> `(cfg, ...)` but here the first arg is `"MORE"`. This is a PoB-specific overload variant
> where the mod type is explicitly named. Verify how the Rust `more_cfg` call is wired.

```lua
-- Line 2100: floor(base * inc * more), clamped ≥ 0
output.Accuracy = m_max(0, m_floor(base * (1 + inc / 100) * more))
-- accuracyVsEnemy is a local (not written to output) used for the hit chance formula
local accuracyVsEnemy = m_max(0, m_floor(baseVsEnemy * (1 + incVsEnemy / 100) * moreVsEnemy))
```

> **`m_floor` + `m_max`:** `m_floor(math.floor)` truncates toward negative infinity; Rust:
> `x.floor()`. The clamp `m_max(0, ...)` → `.max(0.0)`.

```lua
-- Lines 2121–2131: "OffHandAccuracyIsMainHandAccuracy" mastery
-- When this flag is set, off-hand pass reuses main-hand accuracy.
if skillModList:Flag(nil, "Condition:OffHandAccuracyIsMainHandAccuracy") and pass.label == "Main Hand" then
    storedMainHandAccuracy = output.Accuracy
    storedMainHandAccuracyVsEnemy = accuracyVsEnemy
elseif ... and pass.label == "Off Hand" and storedMainHandAccuracy then
    output.Accuracy = storedMainHandAccuracy
    accuracyVsEnemy = storedMainHandAccuracyVsEnemy
end
```

> **Rust note:** `storedMainHandAccuracy` and `storedMainHandAccuracyVsEnemy` are Lua
> upvalues (local variables declared in the enclosing scope at line 2078–2079, shared
> across iterations). In Rust this becomes two `Option<f64>` fields in the pass loop state.

```lua
-- Line 2133–2144: Write AccuracyHitChance
if not isAttack
   or skillModList:Flag(cfg, "CannotBeEvaded")
   or skillData.cannotBeEvaded
   or (env.mode_effective and enemyDB:Flag(nil, "CannotEvade")) then
    output.AccuracyHitChance = 100        -- always hits: spells, CannotBeEvaded attacks
else
    local enemyEvasion = m_max(round(calcLib.val(enemyDB, "Evasion")), 0)
    -- calcs.hitChance formula (defined in CalcDefence.lua:32):
    --   rawChance = accuracy / (accuracy + (evasion/5)^0.9) * 125
    --   return max(min(round(rawChance), 100), 5)
    --
    -- Then multiplied by HitChance mod (inc/more combo via calcLib.mod):
    --   calcLib.mod(skillModList, cfg, "HitChance") = (1 + INC/100) * More
    output.AccuracyHitChance = calcs.hitChance(enemyEvasion, accuracyVsEnemy)
                               * calcLib.mod(skillModList, cfg, "HitChance")
end
```

> **`calcLib.val(enemyDB, "Evasion")`** = `enemyDB:Sum("BASE", nil, "Evasion")`. In Rust:
> `env.enemy.mod_db.sum(None, "Evasion")`.  
> **`round()`** is PoB's standard round — applies before `m_max(..., 0)`. This is a
> potentially meaningful rounding step missing from the Rust impl.  
> **`calcLib.mod(skillModList, cfg, "HitChance")`** = `(1 + INC_HitChance/100) * More_HitChance`.
> The current Rust code does not apply this multiplier — it uses raw `hit_chance(evasion, accuracy)` only.

```lua
-- Lines 2146–2155: Enemy block chance reduces HitChance (not AccuracyHitChance)
output.enemyBlockChance = m_max(m_min((enemyDB:Sum("BASE", cfg, "BlockChance") or 0), 100)
                                - skillModList:Sum("BASE", cfg, "reduceEnemyBlock"), 0)
if enemyDB:Flag(nil, "CannotBlockAttacks") and isAttack then
    output.enemyBlockChance = 0
end
output.HitChance = output.AccuracyHitChance * (1 - output.enemyBlockChance / 100)
```

> **Two distinct hit-chance concepts:**  
> - `AccuracyHitChance`: evasion-based chance to hit (what the oracle field is)  
> - `HitChance`: `AccuracyHitChance × (1 − enemyBlockChance/100)` — used in DPS math  
> The Rust currently writes the combined value to `HitChance` but doesn't separately expose
> `AccuracyHitChance`.

---

### 2. `CritChance` — the main block (CalcOffence.lua:2837–2982)

**Guard: `NeverCrit`**

```lua
-- Lines 2838–2843
if skillModList:Flag(cfg, "NeverCrit") then
    output.PreEffectiveCritChance = 0
    output.CritChance = 0
    output.CritMultiplier = 0
    output.BonusCritDotMultiplier = 0  -- NOTE: PoB field name, not "CritDegenMultiplier"
    output.CritEffect = 1              -- NOTE: CritEffect is set here to short-circuit the else branch
```

> **`output.CritEffect = 1`** set inside the `NeverCrit` branch means the outer
> `if not output.CritEffect then` guard at line 2983 is skipped for `NeverCrit` cases.
> The Rust must replicate this guard: compute CritEffect only when CritChance can be non-zero.

**Guard: `SpellSkillsCannotDealCriticalStrikesExceptOnFinalRepeat`**

```lua
-- Lines 2844–2870: Spell-with-repeats edge case (e.g. Storm Burst)
elseif skillModList:Flag(cfg, "SpellSkillsCannotDealCriticalStrikesExceptOnFinalRepeat") then
    if (output.Repeats or 1) == 1 then
        -- Only one repeat → never crits
        output.CritChance = 0
        ...
    elseif skillModList:Flag(cfg, "SpellSkillsAlwaysDealCriticalStrikesOnFinalRepeat") then
        if env.configInput.repeatMode == "None" then
            output.CritChance = 0          -- viewing non-crit repeat
        elseif env.configInput.repeatMode == "AVERAGE" then
            output.CritChance = 100 / output.Repeats  -- average across repeats
        else
            output.CritChance = 100        -- worst-case / final repeat mode
        end
    end
```

> **`output.Repeats or 1`** — Lua nil-coalesce: if `output.Repeats` is nil, use 1. Rust:
> `get_output_f64(output, "Repeats").max(1.0)` (or use 1.0 when unset).

**Normal crit calculation branch:**

```lua
else -- line 2871
    local critOverride = skillModList:Override(cfg, "CritChance")
    
    -- Destructive Link keystone: inherit crit from parent actor's main hand
    if skillModList:Flag(cfg, "MainHandCritIsEqualToParent") then
        critOverride = actor.parent.output.MainHand and actor.parent.output.MainHand.CritChance
                       or actor.parent.weaponData1.CritChance
    elseif skillModList:Flag(cfg, "MainHandCritIsEqualToPartyMember") then
        critOverride = actor.partyMembers.output.MainHand and actor.partyMembers.output.MainHand.CritChance
                       or (actor.partyMembers.weaponData1 and actor.partyMembers.weaponData1.CritChance or 0)
    end
    
    -- baseCrit: weapon's CritChance (from source table), or override
    local baseCrit = critOverride or source.CritChance or 0
    
    -- BaseCritFromMainHand: some skills (e.g. Spectral Throw clones) use main-hand base crit
    if skillModList:Flag(cfg, "BaseCritFromMainHand") then
        baseCrit = actor.weaponData1.CritChance
    elseif skillModList:Flag(cfg, "AttackCritIsEqualToParentMainHand") then
        baseCrit = actor.parent.weaponData1 and actor.parent.weaponData1.CritChance or baseCrit
    end
```

> **`source.CritChance`** — `source` is the weapon data table for this pass (e.g.
> `actor.weaponData1`), containing base weapon stats. `source.CritChance or 0` is nil-coalesce.

```lua
    if critOverride == 100 then
        -- 100% override → always crit, no modifiers apply
        output.PreEffectiveCritChance = 100
        output.CritChance = 100
    else
        local base = 0
        local inc = 0
        local more = 1
        if not critOverride then
            -- Normal path: sum BASE + INC mods, including enemy self-crit mods
            base = skillModList:Sum("BASE", cfg, "CritChance")
                   + (env.mode_effective and enemyDB:Sum("BASE", nil, "SelfCritChance") or 0)
            inc  = skillModList:Sum("INC", cfg, "CritChance")
                   + (env.mode_effective and enemyDB:Sum("INC", nil, "SelfCritChance") or 0)
            more = skillModList:More(cfg, "CritChance")
        end
        -- Core formula: (baseCrit + BASE_mods) * (1 + INC/100) * More
        output.CritChance = (baseCrit + base) * (1 + inc / 100) * more
        local preCapCritChance = output.CritChance
        
        -- Cap: CritChanceCap (default 100, can be overridden or summed)
        output.CritChance = m_min(output.CritChance,
            skillModList:Override(nil, "CritChanceCap") or skillModList:Sum("BASE", cfg, "CritChanceCap"))
        
        -- Floor: if (baseCrit + base) > 0, never go below 0
        if (baseCrit + base) > 0 then
            output.CritChance = m_max(output.CritChance, 0)
        end
        output.PreEffectiveCritChance = output.CritChance
```

> **`CritChanceCap`:** Default game cap is 100%, but can be higher via the `CritChanceCap`
> mod sum. Note: `Override(nil, "CritChanceCap")` is checked first; if nil, falls back to
> `Sum("BASE", cfg, "CritChanceCap")`. The current Rust hardcodes `.clamp(0.0, 100.0)`,
> which is wrong when `CritChanceCap` > 100.

```lua
        -- Lucky crit: "1 - (1 - p)^(rolls+1)" formula
        local preLuckyCritChance = output.CritChance
        local critRolls = 0
        if env.mode_effective and skillModList:Flag(cfg, "CritChanceLucky") then
            critRolls = critRolls + 1
        end
        if skillModList:Flag(skillCfg, "ExtremeLuck") then
            critRolls = critRolls * 2     -- doubles the roll count
        end
        if critRolls ~= 0 then
            if modDB:Flag(nil, "Unexciting") then
                -- "Unexciting" keystone: 3 rolls, take median → 3p² - 2p³
                output.CritChance = (3 * (output.CritChance / 100) ^ 2
                                     - 2 * (output.CritChance / 100) ^ 3) * 100
            else
                -- Standard lucky: best-of-(rolls+1)
                output.CritChance = (1 - (1 - output.CritChance / 100) ^ (critRolls + 1)) * 100
            end
        end
```

> **`Unexciting` keystone formula:** `3p² − 2p³` is the CDF of `max(X, X, X)` where X is
> uniform — it represents taking the middle of 3 rolls. Note: Lua uses `^` for
> exponentiation (`math.pow` equivalent in Rust is `f64::powi` or `powf`).

```lua
        -- Every-Nth-use guarantees (Cremation, etc.)
        if env.mode_effective then
            if skillModList:Flag(skillCfg, "Every3UseCrit") then
                output.CritChance = (2 * output.CritChance + 100) / 3
            end
            if skillModList:Flag(skillCfg, "Every5UseCrit") then
                output.CritChance = (4 * output.CritChance + 100) / 5
            end
            preHitCheckCritChance = output.CritChance
            -- Effective mode: crit requires both crit roll AND hit roll
            output.CritChance = output.CritChance * output.AccuracyHitChance / 100
        end
    end -- end critOverride != 100
end -- end normal crit branch
```

> **`CritChance × AccuracyHitChance/100`:** In effective mode, the reported `CritChance` is
> the actual probability of dealing a critical strike per use (accounting for misses). The
> Rust does not do this multiplication — it keeps crit chance independent of hit chance.

---

### 3. `CritMultiplier` and `CritEffect` (CalcOffence.lua:2983–3015)

```lua
-- Line 2983: Guard — only enter if CritEffect not already set (NeverCrit sets it early)
if not output.CritEffect then
    if skillModList:Flag(cfg, "NoCritMultiplier") then
        -- NoCritMultiplier keystone (e.g. Elemental Overload-style builds)
        output.CritMultiplier = 1     -- NOTE: 1, not 1.5 — no bonus from crits
    else
        -- extraDamage = sum of BASE CritMultiplier mods / 100
        -- (these are the "+X% extra crit damage" mods; 50 means +50% = 150% total multi)
        local extraDamage = skillModList:Sum("BASE", cfg, "CritMultiplier") / 100
        
        -- Override: some skills hard-set crit multi (e.g. Vengeance)
        local multiOverride = skillModList:Override(skillCfg, "CritMultiplier")
        if multiOverride then
            extraDamage = (multiOverride - 100) / 100
        end
        
        -- Effective mode: enemy's self-crit-multiplier-taken mods apply
        if env.mode_effective then
            local enemyInc = 1 + enemyDB:Sum("INC", nil, "SelfCritMultiplier") / 100
            extraDamage = extraDamage + enemyDB:Sum("BASE", nil, "SelfCritMultiplier") / 100
            extraDamage = round(extraDamage * enemyInc, 2)  -- round to 2 decimal places
        end
        
        -- Final: 1 + max(0, extraDamage)
        -- Base is 1.0 (100%), plus the extra damage fraction (default 0.5 → 1.5 total)
        -- max(0, ...) ensures never drops below 1.0 even with negative mods
        output.CritMultiplier = 1 + m_max(0, extraDamage)
    end
    
    local critChancePercentage = output.CritChance / 100
    -- CritEffect: weighted average damage multiplier across crit/non-crit hits
    output.CritEffect = 1 - critChancePercentage + critChancePercentage * output.CritMultiplier
    
    -- BonusCritDotMultiplier: for Perfect Agony builds
    -- (CritMultiplier BASE sum - 50) * CritMultiplierAppliesToDegen / 10000
    -- Subtracts 50 because base crit multi (50%) doesn't apply to ailments
    output.BonusCritDotMultiplier = (skillModList:Sum("BASE", cfg, "CritMultiplier") - 50)
                                    * skillModList:Sum("BASE", cfg, "CritMultiplierAppliesToDegen")
                                    / 10000
end
```

> **Base CritMultiplier accounting:** PoB stores "CritMultiplier" mods as the *extra*
> percentage above 100%. A "50% increased crit multiplier" mod stores 50. The base game
> provides 50 (giving 150% total), which is why the formula starts at `1 + extraDamage`
> and the default `extraDamage` already includes the 50 base.  
>
> The current Rust formula `(150.0 + base_crit_multi) / 100.0` achieves the same math but
> is semantically different: it hard-codes the 150 base and adds mods on top, then divides
> by 100. **This is equivalent** but doesn't handle `NoCritMultiplier` (which should set
> the multiplier to 1.0, not 1.5), and doesn't apply `multiOverride` or enemy
> `SelfCritMultiplier` mods.

> **`round(extraDamage * enemyInc, 2)`:** PoB's `round(x, dp)` variant rounds to `dp`
> decimal places. Rust: `(x * 100.0).round() / 100.0`.

---

### 4. `combineStat` — per-hand field propagation (CalcOffence.lua:3691–3693)

After the per-pass crit calculations:

```lua
-- Line 3691–3693: only when isAttack and bothWeaponAttack
combineStat("PreEffectiveCritChance", "AVERAGE")
combineStat("CritChance", "AVERAGE")
combineStat("CritMultiplier", "AVERAGE")
```

```lua
-- combineStat with "AVERAGE" mode (line 1979–1980):
-- When bothWeaponAttack is false: output[stat] = output.MainHand[stat] or output.OffHand[stat]
-- When bothWeaponAttack is true:  output[stat] = (MainHand[stat] + OffHand[stat]) / 2
```

> `output.MainHand.CritChance` / `output.OffHand.CritChance` are the per-pass values
> written during the main crit block. `combineStat` then writes the global `output.CritChance`
> from them. The per-pass sub-objects (`output.MainHand`, `output.OffHand`) persist in the
> output table, so `MainHand.CritChance` and `OffHand.CritChance` become accessible as
> nested output fields in the oracle JSON.

---

### 5. `MeleeNotHitChance` / `ProjectileNotHitChance` (CalcDefence.lua:1636–1643)

These fields live in CalcDefence, not CalcOffence. They are computed in
`calcs.buildDefenceEstimations()` only when `damageCategoryConfig != "DamageOverTime"`:

```lua
-- CalcDefence.lua:1638–1639
output.MeleeNotHitChance = 100 -
    (1 - output.MeleeEvadeChance / 100) *
    (1 - output.EffectiveAttackDodgeChance / 100) *
    (1 - output.AvoidAllDamageFromHitsChance / 100) * 100

output.ProjectileNotHitChance = 100 -
    (1 - output.ProjectileEvadeChance / 100) *
    (1 - output.EffectiveAttackDodgeChance / 100) *
    (1 - output.AvoidAllDamageFromHitsChance / 100) *
    (1 - (output.specificTypeAvoidance and 0 or output.AvoidProjectilesChance) / 100) * 100
```

> **`output.specificTypeAvoidance and 0 or output.AvoidProjectilesChance`:** Lua ternary.
> If `specificTypeAvoidance` is truthy, uses `0` (suppresses the projectile avoid component);
> otherwise uses `AvoidProjectilesChance`. Rust: `if output.specific_type_avoidance { 0.0 } else { avoid_proj_chance }`.
>
> These are defence-layer fields that are more naturally coupled with DEF-04/DEF-06 work,
> but are listed in OFF-03 because the oracle asserts them alongside crit fields. Both
> `MeleeEvadeChance` and `ProjectileEvadeChance` must already be computed (from DEF-02
> evasion calculations) before these fields can be written.

---

## Existing Rust Code

File: `crates/pob-calc/src/calc/offence.rs`, lines 95–151

### What exists

- **Resolute Technique check** (line 98–101): flags as `ResoluteTechnique` via `flag_cfg`.
- **Hit chance branch** (lines 103–120): correctly dispatches spell (100%) vs attack
  (uses `hit_chance(evasion, accuracy)`). Has a fallback formula for missing enemy evasion.
- **Basic CritChance** (lines 125–150): queries `CritChance` BASE + INC + More, clamps to
  `[0, 100]`, writes `CritChance` and `CritMultiplier`.
- **CritMultiplier** (lines 144–149): `(150.0 + base_crit_multi) / 100.0`.

### What's missing / wrong

1. **`AccuracyHitChance` is not written.** Lua writes it separately from `HitChance`; the
   Rust writes only `HitChance`. `AccuracyHitChance` is the pre-block hit chance the oracle
   asserts directly.

2. **`HitChance` mod not applied.** Lua multiplies `calcs.hitChance(...)` by
   `calcLib.mod(skillModList, cfg, "HitChance")` — the inc/more product of `HitChance` mods
   (e.g. +hit-chance from passives). The Rust calls `hit_chance(evasion, accuracy)` directly
   without this multiplier.

3. **Enemy block chance not applied.** Lua computes
   `HitChance = AccuracyHitChance × (1 − enemyBlockChance/100)`. The Rust skips this step.

4. **`CritChance × AccuracyHitChance/100` in effective mode is absent.** Lua multiplies
   the effective crit chance by accuracy hit chance so the reported `CritChance` is the true
   per-use probability. The Rust keeps them independent.

5. **`CritChanceCap` is hardcoded to 100.** Lua checks `Override(nil, "CritChanceCap")` and
   `Sum("BASE", cfg, "CritChanceCap")` for the cap value. Some builds (e.g. via the
   "Overshock" node) can raise this above 100. The Rust's `.clamp(0.0, 100.0)` ignores this.

6. **`NeverCrit` and `NoCritMultiplier` flags are not handled.** The Rust goes directly to the
   formula without checking these flags. `NeverCrit` should set CritChance=0 and
   CritEffect=1. `NoCritMultiplier` should set CritMultiplier=1 (not 1.5).

7. **`CritEffect` is not written.** The combined `1 − p + p × CritMultiplier` scalar is
   absent from Rust output. Oracle asserts `CritEffect`.

8. **`BonusCritDotMultiplier` (oracle key) is not written.** The `field_groups.rs` calls
   it `CritDegenMultiplier` but the oracle JSON and Lua both use `BonusCritDotMultiplier`.
   Neither name is written by the current Rust.

9. **Lucky crit, Every3UseCrit, Every5UseCrit not implemented.** The lucky crit formula
   `1 - (1 - p)^(rolls+1)` and the periodic-crit bonuses are absent.

10. **Enemy `SelfCritMultiplier` mods (effective mode) not applied to `CritMultiplier`.**
    Lua adds `enemyDB:Sum("BASE", nil, "SelfCritMultiplier") / 100` and multiplies by
    `1 + enemyDB:Sum("INC", nil, "SelfCritMultiplier") / 100`.

11. **`PreEffectiveCritChance` not written.** Lua writes this field at line 2907 (pre-lucky
    cap). Some oracle builds assert it.

12. **Per-hand fields `MainHand.CritChance`, `OffHand.CritChance`, etc. not written.**
    The Rust has no per-pass sub-output concept yet. These fields come from running the
    crit block per-pass and storing in `output.MainHand` / `output.OffHand` sub-tables.

13. **`MeleeNotHitChance` / `ProjectileNotHitChance` not written.** These are in
    `defence.rs` territory but currently absent from the Rust defence calculations.

14. **`OffHandAccuracyIsMainHandAccuracy` mastery not handled.** The stored-main-hand
    accuracy branch is missing.

## What Needs to Change

1. **Write `AccuracyHitChance` separately from `HitChance`.** Split:
   - `AccuracyHitChance = calcs_hit_chance(enemy_evasion, accuracy_vs_enemy) * hit_chance_mod`
   - `HitChance = AccuracyHitChance * (1 − enemy_block_chance / 100)`

2. **Implement `HitChance` mod multiplier.** After computing the accuracy hit chance, multiply
   by `calc_mod(skill_mod_db, cfg, "HitChance")` = `(1 + INC/100) * More`.

3. **Implement enemy block chance.** Query `enemyDB:Sum("BASE", cfg, "BlockChance")`, clamp
   to `[0, 100]`, subtract `skillModList:Sum("BASE", cfg, "reduceEnemyBlock")`, apply to
   produce the final `HitChance`.

4. **Add `NeverCrit` guard.** If `skillModList.flag_cfg("NeverCrit", cfg)`: set
   `CritChance=0`, `CritMultiplier=0` (or 1?), `CritEffect=1`, `BonusCritDotMultiplier=0`.

5. **Add `NoCritMultiplier` guard.** If flag: set `CritMultiplier = 1.0` instead of `1.5`.

6. **Implement configurable `CritChanceCap`.** Query `Override(nil, "CritChanceCap")` first;
   if nil, use `Sum("BASE", cfg, "CritChanceCap")` (default should be 100).

7. **Implement effective-mode `CritChance × AccuracyHitChance / 100`.** Only apply in
   `env.mode_effective`.

8. **Implement lucky crit formula.** When `CritChanceLucky` flag (or `ExtremeLuck`):
   `CritChance = (1 − (1 − p)^(rolls + 1)) × 100`. Add `Unexciting` variant:
   `3p² − 2p³`.

9. **Implement `Every3UseCrit` / `Every5UseCrit` bonuses** (effective mode only).

10. **Apply enemy `SelfCritMultiplier` mods to `CritMultiplier`** in effective mode.

11. **Write `CritEffect`.** After computing `CritMultiplier`:
    `CritEffect = 1 − p_crit + p_crit × CritMultiplier`
    where `p_crit = CritChance / 100`.

12. **Write `BonusCritDotMultiplier`** (field_groups.rs erroneously calls it
    `CritDegenMultiplier`): `(Sum("BASE", cfg, "CritMultiplier") − 50) × Sum("BASE", cfg, "CritMultiplierAppliesToDegen") / 10000`.
    Update `field_groups.rs` to use `BonusCritDotMultiplier` as the field name.

13. **Write `PreEffectiveCritChance`.** Capture the crit chance before the
    `AccuracyHitChance` multiplication step and write it to output.

14. **Implement per-hand output sub-tables and `combineStat`.** This is a structural change:
    when `isAttack` and both weapons are active (`bothWeaponAttack`), crit and speed stats
    are computed per-pass into `output.MainHand` / `output.OffHand` sub-tables, then
    combined via the `combineStat` logic (AVERAGE, HARMONICMEAN, OR, DPS modes).

15. **Implement `MeleeNotHitChance` / `ProjectileNotHitChance` in `defence.rs`:**
    ```
    MeleeNotHitChance = 100 − (1−MeleeEvade%) × (1−AttackDodge%) × (1−AvoidAll%) × 100
    ProjectileNotHitChance = 100 − (1−ProjEvade%) × (1−AttackDodge%) × (1−AvoidAll%) × (1−AvoidProj%) × 100
    ```
    Only computed when `damageCategoryConfig != "DamageOverTime"`.
