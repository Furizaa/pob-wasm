# MIR-01-mirages: Mirage Actor DPS (Mirage Archer, Saviour, Tawhoa's Chosen, General's Cry, Sacred Wisps)

## Output Fields

> **`MirageCount` does not exist in PoB — see note.**

| `field_groups.rs` name | Actual oracle field | Lua source | Notes |
|------------------------|---------------------|-----------|-------|
| `MirageDPS` | `MirageDPS` | CalcOffence.lua:5905 | Correct — written once in CalcOffence after `calcs.mirages` runs |
| `MirageCount` | **does not exist** | n/a | See note below |

> **`MirageCount` does not exist as a PoB output field.** The oracle for the only
> mirage-using build (`realworld_bow_deadeye.expected.json`) has `MirageDPS: 8075544.898`
> but no `MirageCount`. The field `mirage.count` is an internal field on the mirage struct
> (e.g. `activeSkill.mirage.count = mirageCount`) used to scale DPS, but it is never
> written to `output`. The `field_groups.rs` entry should be removed.

> **Where `MirageDPS` is actually written:** The field is NOT written in `CalcMirages.lua`.
> It is written in `CalcOffence.lua:5905` **after** `calcs.mirages` has populated
> `activeSkill.mirage.output` with the mirage's separate calculation result. The Rust
> currently writes `MirageDPS` inside `mirages.rs::calc_mirage_archer`, which means the
> Rust and Lua write this field at structurally different points.

## Dependencies

- `OFF-04-speed-dps` — `TotalDPS`, `AverageDamage`, `Speed` of the **player's** active
  skill must be correct before mirage DPS can be derived.
- `OFF-07-combined-dps` — Mirage DPS is added to `CombinedDPS` during the same pass that
  finalises combined DPS (CalcOffence.lua:5906).
- `TRIG-01-trigger-rates` — For Tawhoa's Chosen, trigger rates are computed within the
  mirage handler itself by calling `calcMultiSpellRotationImpact`.

## Lua Source

Files:
- `third-party/PathOfBuilding/src/Modules/CalcMirages.lua` (primary — mirage actors)
- `third-party/PathOfBuilding/src/Modules/CalcOffence.lua` lines 5902–5938 (MirageDPS write)

Commit: `454eff8c85d24356d9b051d596983745ed367476`

CalcMirages.lua structure:
- **`calculateMirage` helper:** lines 22–54
- **`calcs.mirages` entry point:** lines 56–435
  - Mirage Archer handler: lines 63–115
  - The Saviour (Reflection) handler: lines 116–173
  - Tawhoa's Chosen handler: lines 174–292
  - Sacred Wisps handler: lines 293–354
  - General's Cry handler: lines 355–433

CalcOffence.lua `MirageDPS` write: lines 5902–5938

## Annotated Lua

### 1. Execution order and the return-value protocol

`calcs.mirages` is called from `calcs.perform` (CalcPerform.lua:3448) **between**
triggers and offence:

```lua
calcs.triggers(env, env.player)
if not calcs.mirages(env) then
    calcs.offence(env, env.player, env.player.mainSkill)  -- only if mirages returns false/nil
end
```

> **`calcs.mirages` returns a boolean that controls whether `calcs.offence` runs.**
> When it returns `true` (truthy), `calcs.offence` is skipped. When it returns `false` or
> `nil`, `calcs.offence` runs normally.
>
> From `calculateMirage` (line 53): `return not config.calcMainSkillOffence`.
>
> - `calcMainSkillOffence = true` (Mirage Archer, Sacred Wisps): the mirage's
>   `calcs.perform(newEnv)` populates the mirage output, and `calcs.offence` still runs
>   for the main skill. Returns `not true = false` → offence runs.
> - `calcMainSkillOffence` absent/nil (Saviour, Tawhoa's Chosen, General's Cry): the
>   mirage handler *replaces* `env.player.mainSkill` and `env.player.output`. Returns
>   `not nil = true` → offence is **skipped**, because the mirage itself becomes the main
>   skill and was already calculated inside the handler.

---

### 2. `calculateMirage` — the shared execution core (lines 22–54)

All mirage types (except General's Cry) flow through this function:

```lua
local function calculateMirage(env, config)
    if not config then return end

    local mirageSkill = config.mirageSkill

    -- Optional: search active skill list for the best candidate skill
    if config.compareFunc then
        for _, skill in ipairs(env.player.activeSkillList) do
            if not skill.skillCfg.skillCond["usedByMirage"] then
                mirageSkill = config.compareFunc(skill, env, config, mirageSkill)
            end
        end
    end
```

> **`config.compareFunc`** — present on Saviour and Tawhoa's Chosen (finds the best DPS
> skill among active skills). Absent on Mirage Archer and Sacred Wisps (the skill is
> `config.mirageSkill = env.player.mainSkill` directly).

> **`skill.skillCfg.skillCond["usedByMirage"]`** — guards against infinite recursion.
> When a skill is set up as a mirage, this flag prevents it from being picked as a
> trigger source by another mirage calculation.

```lua
    if mirageSkill then
        local newSkill, newEnv = calcs.copyActiveSkill(env, "CALCULATOR", mirageSkill)
        newSkill.skillCfg.skillCond["usedByMirage"] = true
        newEnv.limitedSkills = newEnv.limitedSkills or {}
        newEnv.limitedSkills[cacheSkillUUID(newSkill, newEnv)] = true
        newSkill.skillData.mirageUses = env.player.mainSkill.skillData.storedUses
        newSkill.skillTypes[SkillType.OtherThingUsesSkill] = true

        config.preCalcFunc(env, newSkill, newEnv)   -- inject mirage-specific mods
        newEnv.player.mainSkill = newSkill
        calcs.perform(newEnv)                        -- full recalculation in isolated env
        config.postCalcFunc(env, newSkill, newEnv)  -- harvest results back to main env
    else
        config.mirageSkillNotFoundFunc(env, config)
    end
    return not config.calcMainSkillOffence
end
```

> **`calcs.copyActiveSkill(env, "CALCULATOR", mirageSkill)`** — creates a deep copy of
> the skill and a fresh environment. The copy inherits all modifiers from the original
> (via `buildActiveSkillModList`), then `preCalcFunc` overlays the mirage-specific mods
> (less damage, less speed, etc.). `calcs.perform(newEnv)` runs the full calculation
> pipeline — defence, triggers, mirages (recursion-guarded), offence — for the isolated
> mirage environment. This is a full nested calculation, not a shortcut.

> **`SkillType.OtherThingUsesSkill`** — marks the copied skill so it is excluded from
> damage synergy calculations that would otherwise double-count it. This flag is checked
> in CalcTriggers.lua's `findTriggerSkill`.

---

### 3. Mirage Archer handler (lines 63–115)

The Mirage Archer trigger is detected by `skillData.triggeredByMirageArcher`, set from
`SkillStatMap.lua` for skills with the `MirageArcherCanUse` skill type. The active skill
is the bow attack; the Mirage Archer *support* gem injects the Less* mods.

```lua
if env.player.mainSkill.skillData.triggeredByMirageArcher then
    config = {
        calcMainSkillOffence = true,     -- main skill offence still runs
        mirageSkill = env.player.mainSkill,  -- the mirage uses the same skill

        preCalcFunc = function(env, newSkill, newEnv)
            -- These come from the support gem's gem data via SkillStatMap:
            local moreDamage = newSkill.skillModList:Sum("BASE", newSkill.skillCfg, "MirageArcherLessDamage")
            -- Default at gem level 20: -32 (i.e., 32% less damage)
            local moreAttackSpeed = newSkill.skillModList:Sum("BASE", newSkill.skillCfg, "MirageArcherLessAttackSpeed")
            -- Constant stat: always -60 (60% less attack speed)
            local mirageCount = newSkill.skillModList:Sum("BASE", env.player.mainSkill.skillCfg, "MirageArcherMaxCount")
            -- Default: 1 from the support gem constant stat; +N from uniques like the Quiver

            -- Attach mirage metadata to the main skill
            env.player.mainSkill.mirage = { }
            env.player.mainSkill.mirage.name = newSkill.activeEffect.grantedEffect.name
            env.player.mainSkill.mirage.count = mirageCount  -- NOT written to output

            -- Inject damage and speed penalties as MORE mods into the copy
            newSkill.skillModList:NewMod("Damage", "MORE", moreDamage, "Mirage Archer", ...)
            -- moreDamage is negative (e.g. -32), so: MORE with -32 = ×(1 - 32/100) = ×0.68
            newSkill.skillModList:NewMod("Speed", "MORE", moreAttackSpeed, "Mirage Archer", ...)
            -- moreAttackSpeed is negative (e.g. -60), so: MORE with -60 = ×(1 - 60/100) = ×0.4

            -- Does not use player mana/life
            newSkill.skillModList:NewMod("HasNoCost", "FLAG", true, "Used by mirage")
        end,

        postCalcFunc = function(env, newSkill, newEnv)
            -- Store the isolated calculation result on the main skill
            env.player.mainSkill.mirage.output = newEnv.player.output
            env.player.mainSkill.skillFlags.mirageArcher = true
        end,
        ...
    }
```

> **`MORE` mod with negative value:** PoB's MORE mod type means multiplicative.
> A mod `("Damage", "MORE", -32, ...)` applied via `NewMod` causes the skill to have
> `×(1 - 32/100) = ×0.68` damage. The Rust computes `1.0 - less_dmg / 100.0` which is
> correct, but it reads from the query `Sum("BASE", ..., "MirageArcherLessDamage")` which
> returns the negative value. For level 20 Mirage Archer: `moreDamage = -32`, so
> `dmg_mult = 1.0 - (-32) / 100.0 = 1.32` — **this is wrong**. The correct computation
> is `1.0 + (-32) / 100.0 = 0.68` (or equivalently `1.0 + moreDamage / 100.0`).

> **`MirageArcherLessDamage` values:**  
> The PoB mod `"MirageArcherLessDamage"` stores *negative* values:
> level 20 = `-32`, meaning 32% less damage. When used as `MORE`, PoB applies
> `×(1 + value/100)` = `×(1 + (-32)/100)` = `×0.68`. The Rust formula
> `1.0 - less_dmg / 100.0` would give `1.0 - (-32)/100.0 = 1.32` — a 32% *increase* in
> damage. The correct formula is `1.0 + less_dmg / 100.0 = 0.68`.

> **`MirageArcherLessAttackSpeed` constant:** Always `-60` (stored as a constant stat in
> the gem, not a level-scaling stat). The Rust checks `if less_speed > 0.0` — since the
> value is `-60`, this condition is always false, falling back to the hardcoded `0.7`.
> This happens to be approximately correct (the actual multiplier is `1 + (-60)/100 = 0.40`,
> not `0.70`). The hardcoded fallback is therefore also wrong.

The `postCalcFunc` stores the result at `activeSkill.mirage.output`. Then in CalcOffence
(lines 5903–5938), the `MirageDPS` field is assembled:

```lua
-- CalcOffence.lua:5902–5938
local bestCull = 1
if activeSkill.mirage and activeSkill.mirage.output and activeSkill.mirage.output.TotalDPS then
    local mirageCount = activeSkill.mirage.count or 1
    -- Base mirage hit DPS × count:
    output.MirageDPS = activeSkill.mirage.output.TotalDPS * mirageCount
    output.CombinedDPS = output.CombinedDPS + activeSkill.mirage.output.TotalDPS * mirageCount
    output.MirageBurningGroundDPS = activeSkill.mirage.output.BurningGroundDPS
    output.MirageCausticGroundDPS = activeSkill.mirage.output.CausticGroundDPS

    -- Non-stackable ailments: use mirage if it has higher DPS than player
    if activeSkill.mirage.output.IgniteDPS and activeSkill.mirage.output.IgniteDPS > (output.IgniteDPS or 0) then
        output.MirageDPS = output.MirageDPS + activeSkill.mirage.output.IgniteDPS
        output.IgniteDPS = 0   -- zero player ignite to avoid double-counting
    end
    if activeSkill.mirage.output.BleedDPS and activeSkill.mirage.output.BleedDPS > (output.BleedDPS or 0) then
        output.MirageDPS = output.MirageDPS + activeSkill.mirage.output.BleedDPS
        output.BleedDPS = 0
    end

    -- Stackable ailments: scale by count
    if activeSkill.mirage.output.PoisonDPS then
        output.MirageDPS = output.MirageDPS + activeSkill.mirage.output.PoisonDPS * mirageCount
        output.CombinedDPS = output.CombinedDPS + activeSkill.mirage.output.PoisonDPS * mirageCount
    end
    if activeSkill.mirage.output.ImpaleDPS then
        output.MirageDPS = output.MirageDPS + activeSkill.mirage.output.ImpaleDPS * mirageCount
        output.CombinedDPS = output.CombinedDPS + activeSkill.mirage.output.ImpaleDPS * mirageCount
    end
    if activeSkill.mirage.output.DecayDPS then
        output.MirageDPS = output.MirageDPS + activeSkill.mirage.output.DecayDPS
        output.CombinedDPS = output.CombinedDPS + activeSkill.mirage.output.DecayDPS
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

> **`activeSkill.mirage.output.TotalDPS`** — this is the TotalDPS from the mirage's
> *isolated* calculation environment (not the player's). It already incorporates the
> `MirageArcherLessDamage` and `MirageArcherLessAttackSpeed` MORE mods because
> `calcs.perform(newEnv)` ran a full offence calculation on the mirage skill. The Rust
> skips this entirely and computes mirage DPS as `player_dps × dmg_mult × speed_mult × count`,
> which misses all the detailed calculation nuances (conversions, ailments, crits, etc.).

> **`activeSkill.mirage.count or 1`** — Lua nil-coalesce. `mirage.count` was set in
> `preCalcFunc`. The `or 1` fallback means: if no count was set, treat as 1 mirage.

> **Non-stackable ailment logic:** For bleed and ignite, the mirage's ailment replaces
> the player's if it's higher — the two cannot stack independently. Poison, impale, and
> `TotalDot` are stackable (multiple instances from different actors can coexist) so they
> add proportionally.

---

### 4. The Saviour "Reflection" handler (lines 116–173)

```lua
elseif env.player.mainSkill.activeEffect.grantedEffect.name == "Reflection" then
```

> Detection is by **skill name** (`"Reflection"` is the skill granted by The Saviour
> sword). Unlike Mirage Archer which is detected by `skillData.triggeredByMirageArcher`,
> The Saviour is detected by the active skill's granted effect name. The Rust checks for
> a `"SaviourReflection"` flag which does not exist in PoB — there is no such mod.

```lua
    config = {
        -- compareFunc: find the best DPS single-handed sword attack in the skill list
        compareFunc = function(skill, env, config, mirageSkill)
            -- Checks: not main skill, is Attack, uses 1H sword, has crit chance > 0
            if skill ~= env.player.mainSkill
               and skill.skillTypes[SkillType.Attack]
               and not skill.skillTypes[SkillType.Totem]
               and band(skill.skillCfg.flags, bor(ModFlag.Sword, ModFlag.Weapon1H)) == bor(ModFlag.Sword, ModFlag.Weapon1H)
               and not skill.skillCfg.skillCond["usedByMirage"] then
                -- Cache the skill's calculation if not already done
                if not GlobalCache.cachedData[env.mode][uuid] then
                    calcs.buildActiveSkill(env, env.mode, skill, uuid)
                end
                -- Select the highest-TotalDPS skill with crit > 0
                if CritChance > 0 and TotalDPS > usedSkillBestDps then
                    return cached.ActiveSkill  -- returns the best skill so far
                end
            end
            return mirageSkill
        end,

        preCalcFunc = function(env, newSkill, newEnv)
            local moreDamage = env.player.mainSkill.skillModList:Sum("BASE",
                env.player.mainSkill.skillCfg, "SaviourMirageWarriorLessDamage")
            -- Inject damage penalty as MORE mod
            newSkill.skillModList:NewMod("Damage", "MORE", moreDamage, "The Saviour", ...)
            -- Dual-Saviour: if both weapons are identical, warriors = maxWarriors / 2
            if weapon1.name == weapon2.name then
                maxMirageWarriors = maxMirageWarriors / 2
            end
            -- Scale DPS by warrior count via QuantityMultiplier
            newSkill.skillModList:NewMod("QuantityMultiplier", "BASE",
                maxMirageWarriors, "The Saviour Mirage Warriors", ...)
        end,

        postCalcFunc = function(env, newSkill, newEnv)
            -- REPLACE the main skill with the mirage skill
            env.player.mainSkill = newSkill
            -- REPLACE output with the mirage's calculation result
            env.player.output = newEnv.player.output
        end,
    }
```

> **Saviour replaces the entire output.** Unlike Mirage Archer (which stores into
> `mirage.output`), the Saviour's `postCalcFunc` sets `env.player.mainSkill = newSkill`
> and `env.player.output = newEnv.player.output`. The mirage warrior's calculation
> *becomes* the player's output. `calcs.mirages` returns `true`, so `calcs.offence` is
> skipped — the mirage's isolated offence calculation is the authoritative one.

> **`QuantityMultiplier`** for Saviour: the warrior count is injected as a BASE
> `QuantityMultiplier` mod, which then feeds into `TotalDPS` via the `quantityMultiplier`
> scaling in CalcOffence (lines 2485, 3533). This means the `TotalDPS` the mirage
> calculation returns is already scaled by warrior count — no separate `× count` is
> needed in CalcOffence's mirage section.

> **Detection by skill name** (`"Reflection"`): the Rust currently checks for a flag
> `"SaviourReflection"` which does not exist in PoB. The correct check is
> `activeSkill.activeEffect.grantedEffect.name == "Reflection"`.

---

### 5. Tawhoa's Chosen handler (lines 174–292)

```lua
elseif env.player.mainSkill.activeEffect.grantedEffect.name == "Tawhoa's Chosen" then
    -- Same "replace" pattern as Saviour (postCalcFunc replaces env.player.mainSkill and output)
    -- Finds the best-DPS Melee/Slam Attack in the active skill list
    -- Computes trigger rate using calcMultiSpellRotationImpact
    -- After postCalcFunc: sets output.Speed, output.SkillTriggerRate, output.TriggerRateCap
```

This handler is the Chieftain Ancestor totem-like mechanic. After `postCalcFunc`:
- `env.player.output = newEnv.player.output` — mirage output replaces player output
- `env.player.output.Speed = SkillTriggerRate` — overrides the speed
- `env.player.output.SkillTriggerRate = SkillTriggerRate`
- `env.player.output.TriggerRateCap = TriggerRateCap`

The Rust has no Tawhoa's Chosen handler.

---

### 6. Sacred Wisps handler (lines 293–354)

```lua
elseif env.player.mainSkill.skillData.triggeredBySacredWisps then
    -- Same "keep main skill offence" pattern as Mirage Archer (calcMainSkillOffence = true)
    -- mirageSkill = env.player.mainSkill  (uses the same skill)
    -- Finds the Summon Sacred Wisps skill in the socket group for cast chance and wisp count
    -- Injects: more damage penalty, Speed MORE from wispsCastChance - 100
    -- postCalcFunc: stores mirage.output = newEnv.player.output
```

The Rust has no Sacred Wisps handler.

---

### 7. General's Cry handler (lines 355–433)

General's Cry is the outlier — it does NOT use `calculateMirage`. Instead:

```lua
elseif env.player.mainSkill.skillData.triggeredByGeneralsCry then
    -- Prevent infinite recursion guard
    if env.limitedSkills and env.limitedSkills[uuid] then return end

    -- Mark as triggered/mirage to prevent double-counting
    env.player.mainSkill.skillTypes[SkillType.Triggered] = true
    env.player.mainSkill.skillCfg.skillCond["usedByMirage"] = true

    -- Build the main skill's cache if not already done
    if not GlobalCache.cachedData[env.mode][uuid] then
        calcs.buildActiveSkill(env, env.mode, env.player.mainSkill, uuid, {uuid})
    end

    -- Find the actual General's Cry gem in the same slot
    for _, skill in ipairs(env.player.activeSkillList) do
        if skill.activeEffect.grantedEffect.name == "General's Cry"
           and skill.socketGroup.slot == env.player.mainSkill.socketGroup.slot then
            cooldown = calcSkillCooldown(skill.skillModList, skill.skillCfg, skill.skillData)
            break
        end
    end

    -- Scale DPS by warrior count (from GeneralsCryDoubleMaxCount mods)
    for _, value in ipairs(generalsCryActiveSkill.skillModList:Tabulate("BASE", gcCfg, "GeneralsCryDoubleMaxCount")) do
        env.player.mainSkill.skillModList:NewMod("QuantityMultiplier", ...)
        maxMirageWarriors = maxMirageWarriors + mod.value
    end

    -- Scale timing: 0.3s for first mirage + 0.2s per extra mirage
    local mirageSpawnTime = 0.3 + 0.2 * maxMirageWarriors
    if env.player.mainSkill.skillTypes[SkillType.Channel] then
        mirageSpawnTime = mirageSpawnTime + 1
    else
        env.player.mainSkill.skillData.timeOverride = 1  -- non-channel: 1s effective cast
    end
    mirageSpawnTime = round(mirageSpawnTime, 2)

    -- Scale DPS with General's Cry cooldown via DPS MORE mod:
    env.player.mainSkill.skillModList:NewMod("DPS", "MORE", (1 / cooldown - 1) * 100, "General's Cry Cooldown")
    -- (1/cooldown - 1) * 100: converts cooldown rate to "% more DPS" above 1/s rate

    -- Propagate exert-related modifiers
    for _, value in ipairs(skillModList:Tabulate("INC", skillCfg, "ExertIncrease")) do ...end
    for _, value in ipairs(skillModList:Tabulate("MORE", skillCfg, "ExertIncrease")) do ...end
    -- etc.
```

> **`Tabulate`** — PoB mod query that returns a list of all mod entries matching a stat
> name, rather than summing them. Used here to copy individual mod entries onto the
> mirage warrior skill. Rust: iterate over `mod_db.list(cfg, "ExertIncrease")`.

> **`(1 / cooldown - 1) * 100`** — converts the cooldown-based trigger rate to a "% more
> DPS" multiplier. If cooldown = 4.0s: rate = 0.25/s; `(0.25 - 1) * 100 = -75%`. The
> skill already has its base DPS at 1 attack/cycle; the MORE mod scales it to the actual
> 0.25 cycles/second rate.

> **`calcs.mirages` returns `nil`** for General's Cry (does not call `calculateMirage`),
> so `calcs.offence` runs normally. The GC warrior's DPS is computed by the normal
> offence pipeline using the injected mods.

---

## Existing Rust Code

File: `crates/pob-calc/src/calc/mirages.rs`, lines 1–250

### What exists

**`run` dispatcher (lines 5–34):**
- Checks for `"MirageArcher"` flag → `calc_mirage_archer`.
- Checks for `"SaviourReflection"` flag → `calc_saviour_reflection`.
- Checks for `"GeneralsCry"` flag → `calc_generals_cry`.

**`calc_mirage_archer` (lines 38–79):**
- Reads `MirageArcherCount` BASE (defaulting to 1.0).
- Reads `MirageArcherLessDamage` BASE and `MirageArcherLessAttackSpeed` BASE.
- Computes `mirage_dps = player_dps × dmg_mult × speed_mult × mirage_count`.
- Writes `MirageArcherDPS` and `MirageDPS`.

**`calc_saviour_reflection` (lines 83–102):**
- Reads `SaviourLessDamage` BASE.
- Hardcodes `reflection_count = 2.0`.
- Writes `MirageDPS`.

**`calc_generals_cry` (lines 106–139):**
- Hardcodes `base_cd = 4.0`.
- Queries `CooldownRecovery` INC for ICDR.
- Writes `MirageDPS`.

### What's missing / wrong

1. **`MirageCount` field does not exist in PoB.** The `field_groups.rs` entry should be
   removed. `mirage.count` is an internal struct field never written to `output`.

2. **`MirageArcherLessDamage` sign convention is wrong.** The mod stores *negative*
   values (e.g. -32 at level 20). The Rust formula `1.0 - less_dmg / 100.0` computes
   `1.0 - (-32.0)/100.0 = 1.32` — a 32% DPS *increase* instead of a 32% *decrease*.
   Correct formula: `1.0 + less_dmg / 100.0 = 1.0 + (-32)/100.0 = 0.68`.

3. **`MirageArcherLessAttackSpeed` sign convention is wrong.** The constant stat is
   `-60`. The Rust checks `if less_speed > 0.0` — since the value is negative, this is
   always false, using the hardcoded fallback `0.7`. The fallback is also wrong: the
   correct multiplier is `1 + (-60)/100 = 0.40` (60% less attack speed), not `0.70`.

4. **`MirageArcherLessDamage` stat name wrong.** The Rust queries
   `"MirageArcherLessDamage"` using `None` cfg. In the Lua, it's queried with
   `newSkill.skillCfg` context. However, since the support gem injects these as global
   mods (via `NewMod` during skill building), `None` cfg may work in practice. This
   should be verified.

5. **Mirage DPS calculation is structurally wrong.** The Rust approximates mirage DPS as
   `player_dps × dmg_mult × speed_mult × count`. The Lua runs a full isolated
   `calcs.perform(newEnv)` on a copy of the skill with the mirage mods injected. The Lua
   approach correctly handles: conversions, ailments, crit scaling, hit chance, all
   per-pass calculations, and the full DPS pipeline. The Rust approach misses all of these.

6. **Saviour detection is wrong.** The Rust checks for flag `"SaviourReflection"` which
   does not exist in PoB. The Lua detects Saviour by skill name `"Reflection"`.

7. **Saviour reflection count hardcoded to 2.** The Lua queries
   `SaviourMirageWarriorMaxCount` BASE and adjusts for dual-Saviour (halves count if
   both weapons are the same). The Rust always uses 2.

8. **Saviour damage penalty stat name wrong.** The Rust queries `"SaviourLessDamage"`;
   the Lua queries `"SaviourMirageWarriorLessDamage"`.

9. **General's Cry base cooldown hardcoded to 4.0.** The Lua reads the actual General's
   Cry gem's cooldown from `calcSkillCooldown(skill.skillModList, skill.skillCfg,
   skill.skillData)` — finding the actual General's Cry gem in the active skill list.
   Different levels or modifications of the gem have different cooldowns.

10. **General's Cry detection is wrong.** The Rust checks for a `"GeneralsCry"` flag.
    The Lua detects it by `skillData.triggeredByGeneralsCry`, a gem data flag set from
    the skill's stat map.

11. **General's Cry DPS formula is wrong.** The Rust computes
    `player_hit × generals_speed × mirage_count`. The Lua:
    (a) reads `player_hit` from `GlobalCache.cachedData[env.mode][uuid].Env.player.output`
    (already calculated before the handler); (b) injects a `DPS MORE` mod with the
    cooldown rate so the normal offence pipeline handles scaling; (c) applies
    `QuantityMultiplier` for warrior count; (d) lets `calcs.offence` run after
    `calcs.mirages` returns `nil`. The Rust bypasses the offence pipeline entirely.

12. **`MirageDPS` written in wrong location.** The Lua writes `output.MirageDPS` in
    CalcOffence.lua:5905, which also applies ailment contributions and the non-stackable
    ailment replacement logic. The Rust writes `MirageDPS` inside each separate handler
    in `mirages.rs`, skipping all ailment handling.

13. **Mirage ailment contributions absent.** The Lua's CalcOffence mirage section (lines
    5910–5933) adds mirage ignite, bleed (best-of), poison, impale, decay, and TotalDot
    to `MirageDPS`. The Rust only accounts for hit DPS.

14. **`CullMultiplier` from mirage not applied.** The Lua checks
    `activeSkill.mirage.output.CullMultiplier > 1` and uses it as `bestCull`. The Rust
    ignores mirage culling.

15. **Tawhoa's Chosen and Sacred Wisps handlers absent entirely.** These are real
    in-game mechanics (Chieftain ascendancy and Elementalist Sacred Ground mechanics)
    that have no Rust implementation.

## What Needs to Change

1. **Remove `"MirageCount"` from `field_groups.rs`** — this field does not exist in
   PoB. No Rust change needed.

2. **Fix `MirageArcherLessDamage` and `LessAttackSpeed` formulas:**
   ```rust
   // Both mods store negative values
   let dmg_mult   = 1.0 + less_dmg   / 100.0;   // e.g. 1 + (-32)/100 = 0.68
   let speed_mult = 1.0 + less_speed / 100.0;   // e.g. 1 + (-60)/100 = 0.40
   // Remove the `if > 0.0` fallback guards — query the actual mod value.
   ```

3. **Fix Saviour detection.** Replace `flag_cfg("SaviourReflection", ...)` with a check
   on the active skill's granted effect name: `skill_name == "Reflection"`.

4. **Fix Saviour damage stat name:** `"SaviourLessDamage"` → `"SaviourMirageWarriorLessDamage"`.

5. **Fix Saviour warrior count.** Query `SaviourMirageWarriorMaxCount` BASE instead of
   hardcoding 2. Apply the dual-Saviour halving if both weapons have the same name.

6. **Fix General's Cry detection.** Replace `flag_cfg("GeneralsCry", ...)` with a check
   on `skillData.triggeredByGeneralsCry`.

7. **Fix General's Cry base cooldown.** Find the `"General's Cry"` skill in
   `env.player.active_skills` in the same socket group and read its cooldown from
   `calcSkillCooldown` instead of hardcoding 4.0s.

8. **Fix `MirageDPS` write location.** Move the final `MirageDPS` write to after the
   offence calculation, equivalent to CalcOffence.lua:5902–5938. Include:
   - Hit DPS: `mirage.output.TotalDPS × mirageCount`
   - Non-stackable ailments (ignite, bleed): use mirage's if higher than player's; zero player's
   - Stackable ailments (poison, impale, TotalDot, decay): add proportionally
   - `CullMultiplier` comparison for bestCull

9. **Implement the isolated recalculation approach.** The core of the Lua mirage system
   is `calcs.copyActiveSkill` + `calcs.perform(newEnv)` — a full separate calculation in
   an isolated environment with mirage mods injected. Until this is implemented, `MirageDPS`
   values will be approximations that miss ailments, conversions, and other calculations.
   This is the single highest-impact change for mirage correctness.

10. **Implement Tawhoa's Chosen handler.** Detect by skill name `"Tawhoa's Chosen"`.
    Find best-DPS Melee/Slam Attack. Run isolated calculation with MORE damage mod and
    trigger rate computation. Replace `env.player.output` after calculation.

11. **Implement Sacred Wisps handler.** Detect by `skillData.triggeredBySacredWisps`.
    Find the `"Summon Sacred Wisps"` skill for count/chance. Run isolated calculation
    with damage and speed mods. Store result in `activeSkill.mirage.output`.

12. **Track mirage ailment contributions in `TotalDotDPS`.** After mirage DPS is
    assembled, the `TotalDotDPS` aggregation in CalcOffence (line 5940) uses
    `MirageCausticGroundDPS` and `MirageBurningGroundDPS` via `m_max` comparisons.
    These fields need to be set from the mirage's isolated output.
