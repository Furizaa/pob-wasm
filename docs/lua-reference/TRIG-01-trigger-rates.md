# TRIG-01-trigger-rates: Triggered Skill Rates (CoC, CwC, CWDT, Focus, Generic)

## Output Fields

> **Field names in `field_groups.rs` are wrong — see notes below.**

| `field_groups.rs` name | Actual PoB oracle field | Lua source | Notes |
|------------------------|------------------------|-----------|-------|
| `TriggerRate` | **does not exist** | n/a | See note 1 |
| `TriggerTime` | `TriggerTime` | CalcOffence.lua:2192,2196 | `1/SkillTriggerRate` — set in the speed branch for skills with `skillData.triggerTime` or `skillData.triggerRate` |
| `ServerTriggerRate` | **does not exist** | n/a | See note 2 |

The fields that actually appear in the oracle for triggered skills are:

| Actual oracle field | Lua source | Notes |
|--------------------|-----------|-------|
| `SkillTriggerRate` | CalcTriggers.lua:164,263,266,787,800,802,805,810,867 | Final per-second trigger rate of the active skill |
| `TriggerRateCap` | CalcTriggers.lua:163,262,576,578 | Maximum trigger rate given the action cooldown (1/tick-rounded CD) |
| `TriggerTime` | CalcOffence.lua:2192,2196 | `1/SkillTriggerRate`; set for `triggerTime`/`triggerRate` speed branches only |

> **Note 1 — `TriggerRate` does not exist in PoB.** The Lua field written by the Rust
> `triggers.rs` as `"TriggerRate"` has no counterpart in PoB. PoB uses `SkillTriggerRate`
> for the final effective trigger rate. The `field_groups.rs` entry should be changed to
> `SkillTriggerRate`.

> **Note 2 — `ServerTriggerRate` does not exist in current PoB.** It appears in a stale
> 3.13-era spec test file (`spec/TestBuilds/3.13/Dual Wield Cospris CoC.lua`) as a legacy
> field name from an older CalcTriggers implementation. The current CalcTriggers.lua
> (commit `454eff8c`) does not write `ServerTriggerRate` anywhere. The `field_groups.rs`
> entry should be removed.

> **Corrections needed in `field_groups.rs`:**  
> - Remove `"TriggerRate"` → add `"SkillTriggerRate"`  
> - Remove `"ServerTriggerRate"` → add `"TriggerRateCap"`  
> - Keep `"TriggerTime"` (exists in oracle)

## Dependencies

- `OFF-04-speed-dps` — `Speed`, `HitChance`, `CritChance` must be computed for the
  trigger source skill before trigger rates can be derived.
- `SETUP-02-active-skill` — `skillData.triggered`, `skillData.triggerTime`,
  `skillData.triggerRate`, `triggeredBy` must be set up correctly.

## Lua Source

Files:
- `third-party/PathOfBuilding/src/Modules/CalcTriggers.lua` (primary)
- `third-party/PathOfBuilding/src/Modules/CalcOffence.lua` (TriggerTime only)

Commit: `454eff8c85d24356d9b051d596983745ed367476`

Primary line ranges (CalcTriggers.lua):
- **Module-level helpers:** lines 1–133 (`addTriggerIncMoreMods`, `findTriggerSkill`,
  `calcMultiSpellRotationImpact`)
- **Focus handler:** lines 135–217 (`helmetFocusHandler` → writes `TriggerRateCap`,
  `SkillTriggerRate`)
- **CWC handler:** lines 219–387 (`CWCHandler`)
- **`defaultTriggerHandler`:** lines 389–879 (handles all other triggers: CoC, CWDT,
  Arcanist Brand, Manaforged Arrows, etc.)
- **`calcs.triggers` entry point:** lines 1548–1571

CalcOffence.lua `TriggerTime` writes: lines 2185–2198

## Annotated Lua

### 1. Execution order: `calcs.triggers` runs before `calcs.offence`

From `CalcPerform.lua:3447–3449`:

```lua
calcs.triggers(env, env.player)   -- FIRST: establish trigger rate → sets skillData.triggerRate
if not calcs.mirages(env) then
    calcs.offence(env, env.player, env.player.mainSkill)  -- SECOND: uses triggerRate for Speed
end
```

`calcs.triggers` writes `actor.mainSkill.skillData.triggerRate = output.SkillTriggerRate` (line
869). This value is then picked up by `calcs.offence` in the speed calculation:

```lua
-- CalcOffence.lua:2194–2197 (triggerRate speed branch):
elseif skillData.triggerRate and skillData.triggered then
    output.Time = 1 / skillData.triggerRate
    output.TriggerTime = output.Time       -- ← this is the oracle field TriggerTime
    output.Speed = skillData.triggerRate
    skillData.showAverage = false
```

> **`skillData.triggerRate` is a mutable field.** CalcTriggers writes the trigger rate into
> the gem data structure (`skillData`), not into `output`. CalcOffence then reads it from
> `skillData`. This is a cross-module communication pattern via mutation.

---

### 2. Server tick alignment: the core mechanic

All trigger cooldowns are aligned to server ticks (33ms per tick):

```lua
-- Used throughout CalcTriggers.lua:
-- data.misc.ServerTickRate = 1 / 0.033 ≈ 30.30303/s
-- data.misc.ServerTickTime = 0.033s
-- Alignment: ceil(cd * ServerTickRate) / ServerTickRate

local actionCooldownTickRounded = m_ceil(actionCooldownAdjusted * data.misc.ServerTickRate)
                                  / data.misc.ServerTickRate
```

> **Game mechanic:** The server processes actions on 33ms tick boundaries. A cooldown of
> 0.15s requires `ceil(0.15 / 0.033) = 5` ticks = 0.165s. This is why CoC with 0% ICDR
> has a 0.165s cooldown, yielding ~6.06 triggers/s, not 6.67. The Rust
> `align_to_server_tick` function implements this correctly.

---

### 3. `calcs.triggers` dispatch table (lines 1548–1571)

```lua
function calcs.triggers(env, actor)
    if actor and not actor.mainSkill.skillFlags.disable and ... then
        local skillName     = actor.mainSkill.activeEffect.grantedEffect.name
        local triggerName   = actor.mainSkill.triggeredBy and actor.mainSkill.triggeredBy.grantedEffect.name
        local uniqueName    = isTriggered(actor.mainSkill) and getUniqueItemTriggerName(actor.mainSkill)
        -- ...
        -- configTable is a large map: skillName/triggerName → function returning config table
        local config = skillNameLower and configTable[skillNameLower] and configTable[skillNameLower](env)
        config = config or triggerNameLower and configTable[triggerNameLower] and ...
        config = config or uniqueNameLower and configTable[uniqueNameLower] and ...
        if config then
            config.actor = config.actor or actor
            config.triggerName = config.triggerName or triggerName or skillName or uniqueName
            local triggerHandler = config.customHandler or defaultTriggerHandler
            triggerHandler(env, config)
        else
            actor.mainSkill.skillData.triggered = nil  -- not a known trigger, clear the flag
        end
    end
end
```

> **`configTable` look-up chain:** PoB checks (in order) skillName, triggerName, "awakened
> " prefix-stripped triggerName, and uniqueName against a table of known trigger handlers.
> This is how CoC (trigger name = "Cast On Critical Strike Support"), CwC, CWDT, Focus,
> Arcanist Brand, Manaforged Arrows, etc. are dispatched. The Rust uses `triggered_by` from
> the `Build` struct, which is set by the XML parser — this is a reasonable approximation
> but may miss some edge cases (e.g. unique-item triggers).

---

### 4. The `defaultTriggerHandler` flow (lines 389–879)

This handler covers CoC, CWDT, Arcanist Brand, Manaforged Arrows, and many others. The
flow has several distinct phases:

**Phase A: Find the trigger source skill** (lines 399–464)

```lua
-- Find highest-speed skill in the same socket group that can act as the trigger source:
for _, skill in ipairs(env.player.activeSkillList) do
    if config.triggerSkillCond(env, skill) then
        source, trigRate, uuid = findTriggerSkill(env, skill, source, trigRate, config.comparer)
    end
end
-- If no source found → not actually triggered, report as self-cast
if not source then
    actor.mainSkill.skillData.triggered = nil
    actor.mainSkill.infoMessage2 = "DPS reported assuming Self-Cast"
    return
end
```

> **`findTriggerSkill`** uses `GlobalCache.cachedData[env.mode][uuid].HitSpeed or Speed`
> to compare skills and select the highest-attack-rate one. It re-runs
> `calcs.buildActiveSkill` for each candidate if not cached. This is a full separate
> calculation pass per candidate skill.

**Phase B: Source rate adjustments** (lines 437–548)

```lua
-- Dual wield: each weapon fires alternately, halving the effective trigger rate
if env.player.weaponData1.type and env.player.weaponData2.type
   and not source.skillData.doubleHitsWhenDualWielding then
    trigRate = trigRate / 2

-- Unleash: multiply by seal count DPS multiplier
if source.skillModList:Flag(nil, "HasSeals") then
    trigRate = trigRate * unleashDpsMult

-- Manaforged Arrows: divide by ceil(manaThreshold / sourceManaCost)
if actor.mainSkill.skillData.triggeredByManaforged then
    local manaRatio = manaSpentThreshold / sourceManaCost
    trigRate = trigRate / m_ceil(manaRatio)
```

**Phase C: Cooldown calculation** (lines 549–580)

```lua
local icdr = calcLib.mod(actor.mainSkill.skillModList, skillCfg, "CooldownRecovery")
-- ICDR = (1 + INC_CooldownRecovery/100) × More_CooldownRecovery

local triggeredCDAdjusted = ((triggeredCD or 0) + (addedCooldown or 0)) / icdr
-- adjusted trigger cooldown (from the triggered skill itself)

local triggerCDAdjusted = ((triggerCD or 0) + (addsCastTime or 0)) / icdr
-- adjusted trigger CD (from the support gem / trigger item)

-- Server-tick-round both:
local triggeredCDTickRounded = m_ceil(triggeredCDAdjusted * data.misc.ServerTickRate)
                                / data.misc.ServerTickRate
local triggerCDTickRounded = m_ceil(triggerCDAdjusted * data.misc.ServerTickRate)
                              / data.misc.ServerTickRate

-- Action cooldown = max of the two (worst case governs)
local actionCooldownTickRounded = m_max(triggerCDTickRounded, triggeredCDTickRounded)

-- TriggerRateCap = 1 / actionCooldownTickRounded (if non-zero, else m_huge)
output.TriggerRateCap = 1 / actionCooldownTickRounded   -- line 578
```

> **`cooldownOverride`:** If `skillModList:Override(skillCfg, "CooldownRecovery")` is set
> (from a "Trigger a Socketed Spell" craft mod or similar), it takes precedence over the
> ICDR calculation. The Rust does not implement cooldown overrides.

> **`addsCastTime`:** The `SpellCastTimeAddedToCooldownIfTriggered` flag causes the
> triggered spell's cast time to be added to the cooldown. This applies to spells triggered
> by Spellslinger and similar mechanics. The Rust does not implement this.

> **`triggeredByBrand` special case:** Arcanist Brand uses activation frequency instead of
> the trigger support's cooldown.

**Phase D: `EffectiveSourceRate` (lines 706–716)**

```lua
if trigRate ~= nil and not globalTrigger and not config.ignoreSourceRate then
    output.EffectiveSourceRate = trigRate
else
    output.EffectiveSourceRate = output.TriggerRateCap
```

`EffectiveSourceRate` is the final source attack/cast rate after all adjustments (dual
wield halving, unleash, manaforged, etc.), capped at `TriggerRateCap`. This feeds into
the simulation.

**Phase E: Trigger chance adjustment** (lines 721–782)

```lua
local triggerChance = 100  -- starts at 100%
-- For attack-based triggers (CoC etc.):
triggerChance = triggerChance * sourceHitChance / 100   -- × hit chance
if triggerOnCrit then
    triggerChance = triggerChance * sourceCritChance / 100  -- × crit chance
-- For explicit trigger chance mods (e.g. CWDT has 100%, some uniques have < 100%):
if config.triggerChance and config.triggerChance ~= 100 then
    triggerChance = triggerChance * config.triggerChance / 100
```

> **Trigger chance in the CoC oracle:** The CoC oracle shows `TriggerRateCap ≈ 7.576`
> and `SkillTriggerRate ≈ 7.487`. The difference is from `calcMultiSpellRotationImpact`
> (the cooldown-alignment simulation) reducing the raw rate below the cap. The hit chance
> and crit chance have already been incorporated into `trigRate` before the simulation.

**Phase F: `SkillTriggerRate` via simulation** (lines 784–868)

```lua
-- For skills that ignoreTickRate with no rotation competition:
output.SkillTriggerRate = m_min(output.TriggerRateCap, output.EffectiveSourceRate)

-- For simple global triggers (no rotation):
output.SkillTriggerRate = output.EffectiveSourceRate

-- For rotation-based triggers (CoC, CwC, generic):
output.SkillTriggerRate, simBreakdown =
    calcMultiSpellRotationImpact(
        env,
        triggeredSkills,     -- all skills in the trigger group
        output.EffectiveSourceRate,
        triggerCD or triggeredCD,   -- cooldown per skill
        triggerChance,
        actor)
```

The `calcMultiSpellRotationImpact` simulation:
1. Simulates 1000 source-skill uses
2. Each use attempts to trigger each skill in round-robin order
3. A skill fires only if its cooldown has expired (tick-aligned)
4. Returns the main skill's trigger count / simulated duration

> **`calcMultiSpellRotationImpact` is a time-domain simulation**, not a closed-form
> formula. It accounts for cooldown misalignment when multiple spells share a trigger.
> A CoC build with one triggered spell gives rate ≈ `min(1/cd, source_rate)`, but with
> multiple triggered spells each skill gets a lower share. The Rust does not implement
> this simulation at all.

**Phase G: finalize** (lines 869–879)

```lua
actor.mainSkill.skillData.triggerRate = output.SkillTriggerRate  -- feed into CalcOffence
output.Speed = actor.mainSkill.skillData.triggerRate              -- set speed directly too
addTriggerIncMoreMods(actor.mainSkill, source or actor.mainSkill) -- propagate TriggeredDamage mods
```

> `output.Speed` is **overridden here**, after CalcOffence already computed it from
> `skillData.triggerRate`. This means CalcTriggers sets `Speed` a second time with the
> final `SkillTriggerRate`. CalcOffence uses `skillData.triggerRate` (set earlier in the
> `triggerRate` speed branch) but CalcTriggers then updates `output.Speed` directly. For
> skills with `triggerRate` set in gem data, the two agree; for skills where the trigger
> rate is computed by CalcTriggers, CalcOffence's `Speed = triggerRate` is set but
> CalcTriggers overrides it.

---

### 5. `TriggerTime` in CalcOffence (lines 2185–2198)

```lua
-- Branch for skills with skillData.triggerTime set (e.g. Cast While Channeling):
elseif skillData.triggerTime and skillData.triggered then
    -- triggerTime = fixed interval between casts while channeling (seconds)
    local activeSkillsLinked = skillModList:Sum("BASE", cfg, "ActiveSkillsLinkedToTrigger")
    if activeSkillsLinked > 0 then
        -- Multiple linked skills share the trigger, each fires less often
        output.Time = skillData.triggerTime
                      / (1 + skillModList:Sum("INC", cfg, "CooldownRecovery") / 100)
                      * activeSkillsLinked
    else
        output.Time = skillData.triggerTime
                      / (1 + skillModList:Sum("INC", cfg, "CooldownRecovery") / 100)
    end
    output.TriggerTime = output.Time   -- oracle field
    output.Speed = 1 / output.Time

-- Branch for skills with skillData.triggerRate set (already computed by CalcTriggers):
elseif skillData.triggerRate and skillData.triggered then
    output.Time = 1 / skillData.triggerRate
    output.TriggerTime = output.Time   -- oracle field: 1/SkillTriggerRate
    output.Speed = skillData.triggerRate
    skillData.showAverage = false
```

> **`TriggerTime` is always `1 / Speed`** for triggered skills. In the oracle for the CoC
> build: `TriggerTime = 0.1336 ≈ 1 / 7.487 = 1 / SkillTriggerRate`.  
> `TriggerTime` is NOT written for skills that aren't `skillData.triggered` — it's absent
> in non-trigger oracle builds.

> **`ActiveSkillsLinkedToTrigger`** — for CwC with multiple spells linked, the trigger
> time is multiplied by the count (each spell fires 1/N as often as the trigger interval).
> The Rust does not implement this.

---

### 6. `helmetFocusHandler` (lines 135–217)

Focus is a special case — it's a helmet enchantment trigger, not a support gem:

```lua
local focusDuration = (skillFocus.constantStats[1][2] / 1000)  -- duration in seconds
local focusCD = skillFocus.levels[1].cooldown / icdrFocus       -- cooldown after ICDR
local focusTotalCD = focusDuration + focusCD                    -- full cycle time
output.SkillTriggerRate = 1 / focusTotalCD                      -- triggers per second
```

---

## Existing Rust Code

File: `crates/pob-calc/src/calc/triggers.rs`, lines 96–198

### What exists

**`dispatch_trigger` (lines 118–198):**
- Handles `"CastOnCrit"` / `"CoC"`: `baseCD = 0.15`, aligns to tick, computes
  `source_rate = Speed × HitChance/100 × CritChance/100`, then
  `trigger_rate = min(source_rate, 1/cd)`. Writes `TriggerRate`, `TriggerCooldown`, `Speed`.
- Handles `"CastWhileChannelling"` / `"CWC"`: `baseInterval = 0.35`, aligns to tick,
  rate = `1/cd`. Writes `TriggerRate`, `TriggerCooldown`, `Speed`.
- Handles `"CastWhenDamageTaken"` / `"CWDT"`: `baseCD = 0.25`, aligns to tick,
  rate = `1/cd`. Writes `TriggerRate`, `TriggerCooldown`, `Speed`.
- Generic trigger: reads `TriggerCooldown` BASE mod, same formula.

**Helper functions:** `align_to_server_tick`, `apply_icdr`, `calc_trigger_rate` — these
are correct pure functions.

### What's missing / wrong

1. **`TriggerRate` is not a real PoB output field.** The Rust writes `"TriggerRate"` but
   PoB uses `"SkillTriggerRate"`. The oracle expects `SkillTriggerRate`. The Rust should be
   updated to write `"SkillTriggerRate"` instead.

2. **`ServerTriggerRate` is a stale (3.13-era) field name.** The `field_groups.rs` entry
   should be removed. The Rust does not write it either, so nothing needs to change in
   `triggers.rs` for this point.

3. **`TriggerRateCap` not written.** PoB writes `output.TriggerRateCap` (the maximum
   achievable trigger rate before rotation impact). The oracle asserts this field. The Rust
   never writes it.

4. **`TriggerTime` not written.** PoB writes `output.TriggerTime = 1/SkillTriggerRate` in
   CalcOffence for triggered skills. The Rust never writes `TriggerTime`.

5. **CoC formula is wrong.** PoB computes CoC source rate as the **attack rate**, not
   `Speed × HitChance × CritChance`. The Rust computes `Speed × HitChance × CritChance`
   (line 136), which is the rate of crits per second. But the cooldown limits trigger
   rate, not crit rate — the distinction matters because the simulation then accounts
   for the crit fraction internally. In `calcMultiSpellRotationImpact`, the crit chance
   is passed as `triggerChance` (0–100) and reduces the effective rate via the geometric
   distribution formula, not as a direct multiplier on source rate. In the Rust, passing
   `source_rate = speed * hit_chance * crit_chance` (already factoring in crit chance)
   and then calling `calc_trigger_rate(source_rate, 1.0, cd)` conflates two concepts.

6. **`calcMultiSpellRotationImpact` simulation not implemented.** PoB simulates 1000
   source uses to find the actual trigger rate per spell when multiple spells share a
   trigger group. The Rust uses a simple `min(source_rate, 1/cd)` formula, which is
   only correct for a single triggered spell with no cooldown alignment issues.

7. **ICDR only queries `CooldownRecovery` INC without More.** Lua uses:
   `icdr = calcLib.mod(skillModList, skillCfg, "CooldownRecovery")`
   which is `(1 + INC/100) × More`. The Rust uses:
   `icdr = mod_db.sum(Inc, "CooldownRecovery", ...)` (INC only, no More term).

8. **`cooldownOverride` not checked.** The `skillModList:Override(skillCfg,
   "CooldownRecovery")` check (from "Trigger a Socketed Spell" craft mods) is absent.
   When an override is set, it replaces the ICDR-divided cooldown entirely.

9. **`addsCastTime` (`SpellCastTimeAddedToCooldownIfTriggered`) not handled.** Some skills
   add their cast time to the trigger cooldown when triggered (line 566). The Rust ignores
   this flag.

10. **`ActiveSkillsLinkedToTrigger` not applied to `TriggerTime`.** When multiple spells
    are linked to the same trigger (CwC), the trigger time per spell is multiplied by the
    count. The Rust's CWC handler uses a fixed `baseInterval = 0.35` regardless of linked
    skill count.

11. **Trigger source skill not found via active skill list.** PoB searches `env.player.activeSkillList`
    for the highest-speed non-triggered skill in the same socket group to use as the trigger
    source. The Rust hardcodes `Speed`, `HitChance`, and `CritChance` from the triggered
    skill's own output, which would be zero for a CoC spell that has no attack speed of its own.
    In a real CoC build the spell being triggered has no attacks — the source is the attack
    skill in the same socket group.

12. **CWC base interval should come from `source.skillData.triggerTime`**, not a hardcoded
    `0.35`. CwC triggers every N seconds while the channeled skill is active; `triggerTime`
    is the interval between triggers as stored in the channeled skill's gem data.

13. **CoC base cooldown should come from the support gem's level data**, not hardcoded `0.15`.
    Different levels/quality of CoC may have different cooldowns. The PoB code reads
    `triggeredBy.grantedEffect.levels[level].cooldown`.

14. **`TriggeredDamage` INC/MORE mods not added to skill modifier list.** After computing
    the trigger rate, PoB calls `addTriggerIncMoreMods(actor.mainSkill, source)` (line 873)
    which propagates `TriggeredDamage` mods to `Damage` mods on the triggered skill. This
    multiplier (from "increased Damage with Triggered Skills" passive nodes) is completely
    absent from the Rust.

15. **Focus handler absent.** Helmet enchantment focus triggers are not handled.

16. **Manaforged Arrows, Arcanist Brand, Kitava's Thirst, Battlemage's Cry** handlers
    absent — all use the `defaultTriggerHandler` path with special config in PoB.

## What Needs to Change

1. **Change `"TriggerRate"` → `"SkillTriggerRate"` in `triggers.rs`.** Every
   `set_output("TriggerRate", ...)` call should become `set_output("SkillTriggerRate", ...)`.

2. **Remove `"ServerTriggerRate"` from `field_groups.rs`.** Add `"TriggerRateCap"` and
   `"SkillTriggerRate"` in its place.

3. **Write `TriggerRateCap`.** After computing the tick-aligned cooldown:
   ```rust
   env.player.set_output("TriggerRateCap", 1.0 / tick_aligned_cd);
   ```

4. **Write `TriggerTime`.** After setting `Speed = SkillTriggerRate`:
   ```rust
   if trigger_rate > 0.0 {
       env.player.set_output("TriggerTime", 1.0 / trigger_rate);
   }
   ```

5. **Fix ICDR to include More term.** Change:
   ```rust
   let icdr = mod_db.sum(Inc, "CooldownRecovery", ...);
   // to:
   let icdr_inc = mod_db.sum(Inc, "CooldownRecovery", ...);
   let icdr_more = mod_db.more("CooldownRecovery", ...);
   let icdr = (1.0 + icdr_inc / 100.0) * icdr_more;
   ```

6. **Check `cooldownOverride` before dividing by ICDR.** Query
   `mod_db.override_value("CooldownRecovery", Some(cfg), output)`. If set, use it
   directly as the effective cooldown.

7. **Fix CoC source rate.** The source rate should be the trigger skill's attack rate
   (not multiplied by hit/crit chance). Trigger chance adjustments happen in the simulation:
   ```rust
   // Source rate = attack speed of the trigger skill (not the triggered spell)
   let source_speed = find_trigger_source_speed(env);  // TODO: requires active skill lookup
   let trigger_rate_uncapped = source_speed * hit_chance * crit_chance;  // per-crit rate
   // But this should really be implemented as EffectiveSourceRate → simulation
   ```

8. **Implement the multi-spell rotation simulation for `SkillTriggerRate`.** The simple
   `min(source_rate, 1/cd)` formula is wrong when multiple spells share a trigger.
   The simulation in `calcMultiSpellRotationImpact` is the correct approach. A simplified
   version for single-spell CoC (the common case): `min(1/cd, source_attack_rate * crit_chance)`.

9. **Find trigger source skill from active skill list.** For CoC, CwC, CWDT, and similar:
   search `env.player.active_skills` for the highest-speed non-triggered skill in the
   same socket group to use as `source`. Use `source.output.Speed` (or `HitSpeed`) as
   `trigRate`, not the triggered spell's own speed.

10. **Implement `addTriggerIncMoreMods`.** After computing `SkillTriggerRate`:
    - Tabulate `TriggeredDamage` INC and MORE mods from the trigger source skill's mod list.
    - Add them as `Damage` INC and MORE mods to the triggered skill's mod list.
    - This affects all damage calculations for the triggered skill.

11. **Fix CWC base interval**: read from `source.skillData.triggerTime` (the channeled
    skill's gem data), not hardcoded `0.35`.

12. **Fix CoC/CWDT base cooldown**: read from `triggeredBy.grantedEffect.levels[level].cooldown`
    (the support gem's level table), not hardcoded `0.15` / `0.25`.

13. **Handle `ActiveSkillsLinkedToTrigger` for CwC TriggerTime multiplication.**

14. **Handle `addsCastTime` (`SpellCastTimeAddedToCooldownIfTriggered`)** — when this flag
    is set on the triggered skill, add its cast time to the trigger cooldown before tick-rounding.
