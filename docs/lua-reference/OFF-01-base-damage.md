# OFF-01: Base Damage

## Output Fields

Fields this chunk must write (from `field_groups.rs`):

| Field | Oracle non-trivial | Lua line(s) | Notes |
|-------|-------------------|------------|-------|
| `AverageDamage` | 25/30 | 3528 | `AverageHit * HitChance / 100` |
| `AverageBurstDamage` | 25/30 | 3531 | Burst damage including repeats |
| `AverageBurstHits` | 3/30 | 898, 2087, 2089 | Number of hits in a burst |
| `MainHand.AverageDamage` | 0/30 | n/a | **Phantom field** — never written to oracle output |
| `OffHand.AverageDamage` | 0/30 | n/a | **Phantom field** — never written to oracle output |

> **`MainHand.AverageDamage` and `OffHand.AverageDamage` are phantom fields.**
> In PoB, `output.MainHand` and `output.OffHand` are sub-tables set inside
> `calcs.offence`. They contain per-weapon pass results but are never written as
> dot-notation keys in the oracle expected JSON. The Lua breakdown renderer uses
> `output.MainHand.AverageDamage` for display text only (lines 3728, 3730). No
> oracle build has these fields as top-level output keys. Remove from `field_groups.rs`.

> **`AverageBurstHits` is present in all 30 oracle builds** but is non-trivial
> (> 1) in only 3: `aura_stacker` (2), `spectre_summoner` (2), `spell_caster_inquisitor` (2).
> The default value is 1 and is always present.

## Dependencies

- `OFF-03-crit-hit` (conceptually precedes but runs in same pass):
  `AverageHit = totalHitAvg × (1 - CritChance/100) + totalCritAvg × CritChance/100`,
  so `CritChance` and `CritMultiplier` must be correct first.
- `OFF-03-crit-hit` also computes `HitChance` (from accuracy), which is multiplied
  by `AverageHit` to produce `AverageDamage`.
- The per-damage-type base min/max (from `calcDamage()`) must be correct — these
  feed `totalHitAvg` and `totalCritAvg`.
- `AverageBurstHits` depends on `output.SealMax` (Spellslinger) or
  `skillData.averageBurstHits` or `output.Repeats`.

## Lua Source

**File: `CalcOffence.lua`**, function `calcs.offence`  
**Entry point:** line 319  
**Key computation:** lines 3130–3531

Commit: `454eff8c85d24356d9b051d596983745ed367476` (third-party/PathOfBuilding, heads/dev)

## Annotated Lua

### Architecture overview: `calcs.offence`

`calcs.offence` runs once per active skill. The overall structure is:

1. **Configure damage passes** (lines 1904–1971): for attack skills, create a pass entry
   for each weapon (Main Hand and/or Off Hand) with separate `output` sub-tables.
   For spells, create a single "Skill" pass writing into the global output.
2. **Per-pass loop** (`for _, pass in ipairs(passList)` at line 2082): runs all
   calculations for each weapon pass independently.
3. **After the loop**, `combineStat` merges dual-wield results into the global output.

The result is that `AverageDamage`, `AverageBurstDamage`, and `AverageBurstHits`
are always written to the global (skill-level) output, not per-weapon sub-tables.

### Section 1: Pass configuration (lines 1904–1971)

```lua
local passList = { }
if isAttack then
    output.MainHand = { }   -- creates sub-table; output.MainHand is a Lua table
    output.OffHand = { }    -- not a string key "MainHand.AverageDamage"
    -- Each pass gets its own output table to accumulate results independently
    if skillFlags.weapon1Attack then
        t_insert(passList, {
            label = "Main Hand",
            source = copyTable(actor.weaponData1),  -- weapon base stats
            cfg = activeSkill.weapon1Cfg,           -- per-weapon mod config
            output = output.MainHand,               -- ← writes go here, not top-level
            breakdown = breakdown and breakdown.MainHand,
        })
    end
    if skillFlags.weapon2Attack then
        t_insert(passList, {
            label = "Off Hand",
            source = copyTable(actor.weaponData2),
            cfg = activeSkill.weapon2Cfg,
            output = output.OffHand,                -- ← writes go here
            breakdown = breakdown and breakdown.OffHand,
        })
    end
else
    t_insert(passList, {
        label = "Skill",
        source = skillData,
        cfg = skillCfg,
        output = output,    -- ← spell writes go directly to global output
        breakdown = breakdown,
    })
end
```

**Gotcha — `output.MainHand` is a Lua table, not a string key.** When the Lua writes
`output.MainHand.AverageDamage`, it sets a field inside the sub-table referenced by
`output.MainHand`, which happens to be stored as `output["MainHand"]["AverageDamage"]`.
This never results in a top-level key named `"MainHand.AverageDamage"` in the oracle
JSON. The oracle output has only flat string keys.

**Gotcha — `pass.output` is a local re-binding.** Inside the per-pass loop:
```lua
for _, pass in ipairs(passList) do
    globalOutput, globalBreakdown = output, breakdown  -- save the outer tables
    local source, output, cfg, breakdown = pass.source, pass.output, pass.cfg, pass.breakdown
    -- "output" is now the pass-local table (output.MainHand or output.OffHand or output)
    -- All subsequent writes to "output" in this loop body go to the per-weapon table.
end
```
This is a classic Lua variable shadowing pattern (see LUA-GOTCHAS §3). The `local output`
inside the loop shadows the outer `output` variable. The outer table is accessible as
`globalOutput`.

### Section 2: `AverageBurstHits` (lines 898, 2087–2089)

`AverageBurstHits` is set per-pass, not globally, and represents "how many times does
this skill hit in one use" (for repeating/sealing skills).

```lua
-- Spellslinger seals:
if skillData.SealMax then
    output.AverageBurstHits = output.SealMax   -- line 898
end
-- Later in the pass loop (line 2086):
if skillData.averageBurstHits then
    output.AverageBurstHits = skillData.averageBurstHits
    -- e.g., Storm Brand allocates hits over time → specific average
elseif output.Repeats and output.Repeats > 1 then
    output.AverageBurstHits = output.Repeats
    -- Skills that repeat (e.g., Unload, Barrage) use Repeats count
    -- Note: Repeats > 1, but dual wielding (Repeats ≤ 1) doesn't use this path
end
-- Default: nil (treated as 1 in globalOutput.AverageBurstHits below)
```

In the post-loop section (line 3529):
```lua
globalOutput.AverageBurstHits = output.AverageBurstHits or 1
-- "or 1" nil-coalesces: if output.AverageBurstHits is nil, use 1.
-- Rust: output.AverageBurstHits.unwrap_or(1.0)
```

### Section 3: Core damage averages (lines 3130–3521)

This is the heart of the per-pass loop. The inner dual-pass (lines 3140–3410) iterates
`pass = 1` (critical) and `pass = 2` (non-critical) to compute min/max/avg per type:

```lua
local totalHitMin, totalHitMax, totalHitAvg = 0, 0, 0
local totalCritMin, totalCritMax, totalCritAvg = 0, 0, 0

for pass = 1, 2 do  -- pass 1 = crit, pass 2 = non-crit
    cfg.skillCond["CriticalStrike"] = (pass == 1)
    for _, damageType in ipairs(dmgTypeList) do  -- Physical, Lightning, Cold, Fire, Chaos
        if skillFlags.hit and canDeal[damageType] then
            -- calcDamage returns min, max for this type (after conversion, INC, More)
            damageTypeHitMin, damageTypeHitMax = calcDamage(activeSkill, output, cfg, ...)
            -- convMult = how much of this type remains (not converted away)
            local convMult = conversionTable[damageType].mult
            -- allMult: combined scaling (convMult * ScaledDamageEffect * ruthlessBlowEffect etc.)
            output.allMult = convMult * output.ScaledDamageEffect * output.RuthlessBlowHitEffect
                           * output.FistOfWarDamageEffect * globalOutput.OffensiveWarcryEffect
            local allMult = output.allMult
            if pass == 1 then
                allMult = allMult * output.CritMultiplier  -- apply crit multiplier for crit pass
            end
            damageTypeHitMin = damageTypeHitMin * allMult
            damageTypeHitMax = damageTypeHitMax * allMult
            -- Lucky/unlucky dice rolls modify the average:
            -- ... (handles LuckyHits, UnluckyHits flags)
            damageTypeHitAvg = (damageTypeHitMin + damageTypeHitMax) / 2  -- simplified
            -- (plus lucky/unlucky adjustments)
        end

        if pass == 1 then
            totalCritAvg = totalCritAvg + damageTypeHitAvg  -- crit pass total
        else
            totalHitAvg = totalHitAvg + damageTypeHitAvg   -- non-crit pass total
        end
    end
end
```

**Gotcha — pass 1 is CRIT, pass 2 is NON-CRIT.** Despite being called "pass 1" and
"pass 2", the first inner loop pass computes *crit* damage and the second computes
*non-crit*. This is because `cfg.skillCond["CriticalStrike"] = (pass == 1)` enables
the CriticalStrike condition for `pass == 1`.

### Section 4: `AverageHit` (line 3521)

```lua
output.AverageHit = totalHitAvg * (1 - output.CritChance / 100)
                  + totalCritAvg * output.CritChance / 100
-- Weighted average: (non-crit total) × (non-crit chance) + (crit total) × (crit chance)
-- Note: crit chance is in percent [0-100], divide by 100 for fraction.
-- Rust: avg_non_crit * (1.0 - crit_rate) + avg_crit * crit_rate
```

**Important:** `totalCritAvg` already includes `CritMultiplier` applied (in the pass 1
loop above). So this is NOT `totalHitAvg * (1 + critEffect - 1) * critRate` — the
`totalCritAvg` is the full scaled crit damage, not a multiplier applied to hit avg.

This matches the Rust implementation (line 349):
```rust
let average_hit = avg_non_crit * (1.0 - crit_rate) + (avg_non_crit * crit_multi) * crit_rate;
```
The Rust approximates `totalCritAvg ≈ avg_non_crit * crit_multi`, which is correct
when there are no per-type lucky/unlucky adjustments.

### Section 5: `AverageDamage` (line 3528)

```lua
output.AverageDamage = output.AverageHit * output.HitChance / 100
-- Multiplies by hit chance (0–100 percent) to get expected damage per use.
-- Rust (line 361): let average_damage = average_hit_final * hit_chance;
--   where hit_chance = hit_chance_pct / 100.0 (already divided)
```

### Section 6: `AverageBurstDamage` (lines 3529–3531)

```lua
globalOutput.AverageBurstHits = output.AverageBurstHits or 1
-- Copy per-pass AverageBurstHits into the global output; default 1.

local repeatPenalty = skillModList:Flag(nil, "HasSeals")
    and activeSkill.skillTypes[SkillType.CanRapidFire]
    and not skillModList:Flag(nil, "NoRepeatBonuses")
    and calcLib.mod(skillModList, skillCfg, "SealRepeatPenalty")
    or 1
-- repeatPenalty is the % of AverageDamage each repeat deals (default 1.0 = no penalty).
-- "HasSeals" (Spellslinger) combined with "CanRapidFire" skill type activates seal bonus.
-- calcLib.mod(skillModList, skillCfg, "SealRepeatPenalty") = (1 + INC/100) * More
-- If none of the conditions are true: repeatPenalty = 1 (the trailing `or 1`)
--
-- Gotcha: `A and B and C and D and E or 1`:
--   If ALL of A, B, C, D are truthy: returns E (the calcLib.mod result)
--   If ANY of A, B, C, D is false/nil: short-circuits and returns 1
--   This is the multi-condition Lua ternary pattern.

globalOutput.AverageBurstDamage = output.AverageDamage
    + output.AverageDamage * (globalOutput.AverageBurstHits - 1) * repeatPenalty
    or 0
-- AverageBurstDamage = firstHit + (extraHits × penalty × firstHit)
-- = AverageDamage × (1 + (hits - 1) × repeatPenalty)
-- When hits=1: AverageBurstDamage = AverageDamage (no extra hits)
-- When hits=2, penalty=1: AverageBurstDamage = 2 × AverageDamage
-- When hits=2, penalty<1: AverageBurstDamage < 2 × AverageDamage
--
-- The trailing `or 0` is a nil-coalesce: if the entire expression evaluates to nil,
-- use 0. This handles the case where AverageDamage is nil (disabled skills).
-- Rust: average_damage + average_damage * (burst_hits - 1.0) * repeat_penalty
```

**Oracle verification for `aura_stacker`:**
- `AverageDamage = 8397.66`, `AverageBurstHits = 2`, `repeatPenalty = 1.0`
- `AverageBurstDamage = 8397.66 + 8397.66 × (2-1) × 1.0 = 16795.33` ✓

**Oracle verification for `dual_wield`:**
- `AverageDamage = 19.74`, `AverageBurstHits = 1`
- `AverageBurstDamage = 19.74 + 19.74 × 0 × 1.0 = 19.74` — but oracle shows `20.11`
- This discrepancy comes from the `combineStat("AverageDamage", "DPS")` path for
  `bothWeaponAttack` (see section 7 below), where `AverageDamage` is set to the
  combined dual-wield value AFTER the per-pass output.AverageDamage is set, but
  `AverageBurstDamage` uses `globalOutput.AverageBurstHits` which is set from the
  *last pass's* output (the off-hand pass in dual wield). The exact difference of
  `20.11 - 19.74 = 0.37` suggests `AverageBurstHits` for the last pass was slightly
  above 1 in the off-hand, making the burst calculation use the off-hand's value.
  This is a subtle pass-ordering effect.

### Section 7: Dual-wield `combineStat` for `AverageDamage` (lines 3694–3695, 3723–3744)

After the per-pass loop completes, if `skillFlags.bothWeaponAttack`:

```lua
combineStat("AverageDamage", "DPS")
-- "DPS" mode (line 2070–2074):
--   output[stat] = (output.MainHand[stat] or 0) + (output.OffHand[stat] or 0)
--   if not skillData.doubleHitsWhenDualWielding:
--       output[stat] = output[stat] / 2
-- So:
--   doubleHitsWhenDualWielding=true:  AverageDamage = MainHand.AverageDamage + OffHand.AverageDamage
--   doubleHitsWhenDualWielding=false: AverageDamage = (MainHand.AverageDamage + OffHand.AverageDamage) / 2
--
-- The "DPS" mode adds the two, then halves unless "doubleHitsWhenDualWielding" because:
-- - When alternating weapons: you deal MainHand OR OffHand per use → average = (MH + OH) / 2
-- - When hitting with both simultaneously: you deal both per use → total = MH + OH
```

**Gotcha — `output.MainHand.AverageDamage` in the breakdown uses the sub-table field,
not a top-level key.** This is what the breakdown lines reference (lines 3728, 3730),
but is never serialized to the oracle JSON as a flat key.

## Existing Rust Code

**File:** `crates/pob-calc/src/calc/offence.rs`

### What exists

| Feature | Lines | Status |
|---------|-------|--------|
| Hit chance computation | 96–121 | ✅ Present — resolute technique, attack vs spell, fallback evasion |
| Crit chance / multiplier | 122–151 | ✅ Present |
| Per-type base damage (from `skill.base_damage`) | 155–343 | ✅ Present |
| `AverageHit = non-crit × (1-cc) + crit × cc` | 347–357 | ✅ Correct formula |
| `ScaledDamageEffect` (double/triple damage) | 351–356 | ✅ Present |
| `AverageDamage = AverageHit × HitChance/100` | 361–362 | ✅ Correct |
| `TotalDPS = AverageDamage × uses_per_sec` | 364–365 | ✅ Present |

### What is missing or wrong

| Feature | Rust status |
|---------|-------------|
| **`AverageBurstDamage`** | ❌ **Missing** — not written at all |
| **`AverageBurstHits`** | ❌ **Missing** — not written at all |
| Dual-wield pass structure (per-weapon sub-tables) | ❌ **Missing** — Rust runs a single pass without `output.MainHand` / `output.OffHand` separation |
| `combineStat("AverageDamage", "DPS")` for `bothWeaponAttack` | ❌ **Missing** |
| `repeatPenalty` for seal skills (Spellslinger) | ❌ **Missing** |
| Lucky/unlucky hit dice rolls | ❌ **Missing** — affects per-type averages; Lua has complex `damageTypeLuckyChance` logic |
| Warcry exert effects (`OffensiveWarcryEffect`, `MaxOffensiveWarcryEffect`) | ❌ **Missing** from `allMult` |
| `RuthlessBlowHitEffect`, `FistOfWarDamageEffect` in `allMult` | ❌ **Missing** |
| `allMult` factor including convMult × ScaledDamageEffect × warcry | ⚠️ Partial — Rust has `ScaledDamageEffect` but not the others |
| The full `calcDamage()` function | ❌ Rust approximates with `base_damage` from `ActiveSkill`; Lua's `calcDamage` applies weapon local mods, flat added damage mods, INC/More per type |
| `skillData.averageBurstHits` path | ❌ **Missing** |
| `output.Repeats > 1` → `AverageBurstHits = Repeats` | ❌ **Missing** |
| `output.SealMax` → `AverageBurstHits = SealMax` | ❌ **Missing** |

### `HitChance` computation difference

The Rust (line 103–121) computes hit chance purely from accuracy vs enemy evasion.
The Lua (line 2147–2152) additionally applies **enemy block chance**:

```lua
output.enemyBlockChance = max(min(enemyDB:Sum("BASE", cfg, "BlockChance"), 100) - skillModList:Sum("BASE", cfg, "reduceEnemyBlock"), 0)
output.HitChance = output.AccuracyHitChance * (1 - output.enemyBlockChance / 100)
```

The Rust skips the enemy block chance reduction. For the oracle builds, `enemyBlockChance = 0`
(the test enemy doesn't block), so there's no oracle divergence currently.

### `AverageDamage` formula accuracy

For builds without dual-wield, warcry effects, or lucky rolls, the Rust produces
correct `AverageDamage` values for many oracle builds. Divergences occur when:
1. `OffensiveWarcryEffect > 1` (warcry-exerted attacks)
2. `doubleHitsWhenDualWielding` skills
3. `RuthlessBlowHitEffect != 1`
4. Lucky/unlucky damage rolls
5. Skills where `base_damage` in the Rust `ActiveSkill` doesn't fully capture all
   added flat damage sources (socketed support gems, aura buffs applied via modDB
   rather than base_damage)

## What Needs to Change

1. **Write `AverageBurstHits` to output**:
   ```rust
   // After computing the main pass, check for repeats/seals:
   let burst_hits = skill.average_burst_hits
       .or_else(|| if repeats > 1 { Some(repeats as f64) } else { None })
       .unwrap_or(1.0);
   env.player.set_output("AverageBurstHits", burst_hits);
   ```

2. **Write `AverageBurstDamage` to output**:
   ```rust
   // After AverageDamage is computed:
   let repeat_penalty = compute_seal_repeat_penalty(env, &cfg); // 1.0 for non-seal skills
   let burst_hits = get_output_f64(&env.player.output, "AverageBurstHits");
   let burst = average_damage + average_damage * (burst_hits - 1.0) * repeat_penalty;
   env.player.set_output("AverageBurstDamage", burst);
   ```

3. **Remove `MainHand.AverageDamage` and `OffHand.AverageDamage` from `field_groups.rs`** — they are phantom fields not present in oracle output.

4. **Add warcry exert multipliers to `allMult`** — `OffensiveWarcryEffect` and
   `MaxOffensiveWarcryEffect` (from globalOutput) must multiply into the per-type
   hit averages. These affect all exerted attacks (common in Warcry builds).

5. **Add `RuthlessBlowHitEffect` and `FistOfWarDamageEffect`** to the hit average
   multiplier chain.

6. **Implement dual-wield `combineStat("AverageDamage", "DPS")`**:
   - Run two separate calculation passes when `weapon1Attack && weapon2Attack`
   - Combine: if `doubleHitsWhenDualWielding`: sum; else: average
   - This requires the pass list architecture and per-weapon `output.MainHand` / `output.OffHand` sub-tables

7. **Implement lucky/unlucky damage rolls** — the `damageTypeLuckyChance` logic
   (lines 3195–3246) adjusts per-type averages for lucky (roll twice, take higher)
   and unlucky (roll twice, take lower) effects. Currently absent in Rust.

## Oracle Confirmation (all 30 builds)

| Build | AverageDamage | AverageBurstDamage | AverageBurstHits | Notes |
|-------|---------------|-------------------|-----------------|-------|
| aura_stacker | 8397.66 | 16795.33 | 2 | 2× burst (doubling skill) |
| bleed_gladiator | 189.15 | 189.15 | 1 | |
| bow_deadeye | 2936.22 | 2936.22 | 1 | |
| champion_impale | 2422.74 | 2422.74 | 1 | |
| ci_lowlife_es | 0 | 0 | 1 | ShowAverage skill, 0 hit dmg |
| cluster_jewel | 9356.36 | 9356.36 | 1 | |
| coc_trigger | 190503.88 | 190503.88 | 1 | |
| cwc_trigger | 0 | 0 | 1 | ShowAverage skill, 0 hit dmg |
| dot_caster_trickster | 7052.26 | 7052.26 | 1 | |
| dual_wield | 19.74 | 20.11 | 1 | Dual wield combineStat |
| ele_melee_raider | 2946.94 | 2946.94 | 1 | |
| flask_pathfinder | 377.75 | 377.75 | 1 | |
| ignite_elementalist | 5067.41 | 5067.41 | 1 | |
| max_block_gladiator | 770.80 | 770.80 | 1 | |
| mine_saboteur | 1248.79 | 1248.79 | 1 | |
| minion_necromancer | 0 | 0 | 1 | Summoner — no player hit dmg |
| mom_eb | 5451.42 | 5451.42 | 1 | |
| phys_melee_slayer | 602863.85 | 602863.85 | 1 | |
| phys_to_fire_conversion | 7386.24 | 7386.24 | 1 | |
| poison_pathfinder | 311.86 | 300.78 | 1 | BurstDamage < AvgDamage (repeat penalty) |
| rf_juggernaut | 0 | 0 | 1 | RF — no hit dmg |
| shield_1h | 591.91 | 591.91 | 1 | |
| spectre_summoner | 0 | 0 | 2 | Summoner, 2 burst hits (spectres) |
| spell_caster_inquisitor | 17363.25 | 34726.50 | 2 | 2× burst (repeating spell) |
| timeless_jewel | 338.94 | 338.94 | 1 | |
| totem_hierophant | 3122.09 | 3122.09 | 1 | |
| trap_saboteur | 2021.34 | 2021.34 | 1 | |
| triple_conversion | 867.12 | 855.38 | 1 | BurstDamage < AvgDamage (repeat penalty) |
| two_handed | 56774.53 | 56774.53 | 1 | |
| wand_occultist | 78249.53 | 78249.53 | 1 | |

> `ci_lowlife_es`, `cwc_trigger`, `rf_juggernaut`, `minion_necromancer`, and
> `spectre_summoner` all have `AverageDamage = 0` — either because:
> - The active skill doesn't deal hit damage (RF, minions)
> - The skill configuration uses a trigger (CwC), so the displayed skill shows 0
> - The summoner's own stats show 0 (minion damage is on the minion actor)
>
> `poison_pathfinder` and `triple_conversion` have `AverageBurstDamage < AverageDamage`
> with `AverageBurstHits = 1`. This means `repeatPenalty < 1` for those skills even
> though `AverageBurstHits = 1`, suggesting a single-hit skill with some repeat
> penalty mechanism active. Most likely these are seal skills or skills with a penalty
> that still applies to the base hit.
