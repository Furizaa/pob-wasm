# DEF-05: Recovery in Defence

## Output Fields

The `field_groups.rs` entry for this chunk is **intentionally empty**:

```rust
"DEF-05-recovery-in-defence" => &[
    // Recovery rates as computed in CalcDefence's pool-based recovery
    // These overlap with PERF-07 fields but are the final computed values
    // after defence-specific adjustments.
],
```

This is not an oversight — it reflects a design decision documented in the spec comment.
The fields that logically belong to this chunk either:

1. **Overlap with PERF-07** (`LifeRegen`, `ManaRegen`, `EnergyShieldRegen`,
   `*RecoveryRateMod`, `*LeechRate`, `EnergyShieldRecharge`, `EnergyShieldRechargeDelay`,
   `WardRechargeDelay`) — already assigned to PERF-07's field group.
2. **Are zero in all 30 oracle builds** and thus provide no oracle signal (recoup
   fields, per-damage-type recoup, pseudo-recoup, `anyRecoup`).
3. **Are present in oracle but not yet assigned to any chunk field group**
   (`EnergyShieldRecoveryCap`, `EnergyShieldRechargeAppliesToEnergyShield`,
   `*RegenInc`, `*RegenPercent`, `*RegenRecovery`, `NetLifeRegen`, `NetManaRegen`,
   `TotalNetRegen`, `LifeFlaskRecovery`).

This document serves as the **survey and gap analysis** for all recovery-in-defence
fields. Its purpose is to identify which unassigned fields should eventually be added
to PERF-07 or DEF-05's field group, and to document the Lua source for each.

### Recommended field group additions

The following fields are present in oracle outputs but currently assigned to no chunk:

| Field | Oracle non-zero | Lua source | Recommendation |
|-------|-----------------|-----------|----------------|
| `EnergyShieldRechargeAppliesToEnergyShield` | 30/30 | CalcDefence 1324 | Add to PERF-07 or DEF-05 |
| `EnergyShieldRecoveryCap` | 18/30 | CalcDefence 1058–1061 | Add to DEF-05 |
| `LifeRegenPercent` | 14/30 | CalcDefence 1297 | Add to PERF-07 |
| `ManaRegenPercent` | 30/30 | CalcDefence 1297 | Add to PERF-07 |
| `EnergyShieldRegenPercent` | 0/30 | CalcDefence 1297 | Add to PERF-07 |
| `LifeRegenRecovery` | 14/30 | CalcDefence 1293 | Add to PERF-07 |
| `ManaRegenRecovery` | 30/30 | CalcDefence 1293 | Add to PERF-07 |
| `EnergyShieldRegenRecovery` | 0/30 | CalcDefence 1293 | Add to PERF-07 |
| `ManaRegenInc` | 11/30 | CalcDefence 1285 | Add to PERF-07 |
| `NetLifeRegen` | 1/30 | CalcDefence 3371, 3441 | Add to PERF-07 |
| `NetManaRegen` | 1/30 | CalcDefence 3372, 3442 | Add to PERF-07 |
| `TotalNetRegen` | 1/30 | CalcDefence 3444 | Add to PERF-07 |
| `LifeFlaskRecovery` | 2/30 | CalcDefence (flask section) | Add to DEF-05 |
| `WardRechargeDelay` | 30/30 | CalcDefence 1474 | Add to PERF-07 |

## Dependencies

- `PERF-07-regen-recharge-leech` — the majority of regen/recharge logic runs in that
  chunk and is already documented there; DEF-05's contribution is the
  defence-specific parts that require `output.Armour`, `output.Evasion`, and
  `output.EnergyShield` from DEF-02
- `DEF-02-armour-evasion-es-ward` — `EnergyShieldRecoveryCap` depends on
  `output.Armour`, `output.Evasion`, `output.EnergyShield`
- `DEF-03-block-suppression` — `LifeOnBlock`, `ManaOnBlock`, `EnergyShieldOnBlock`
  require that block is computed first (line 1512: "recovery on block, needs to be
  after primary defences")
- `DEF-04-damage-reduction-avoidance` — the Net regen section (lines 3368–3459)
  reads `output[damageType.."BuildDegen"]`, `output[damageType.."EnergyShieldBypass"]`,
  `output[damageType.."MindOverMatter"]` from the EHP pipeline

## Lua Source

**File: `CalcDefence.lua`**

| Section | Lines | Key fields |
|---------|-------|-----------|
| Recovery rate modifiers | 1191–1197 | `*RecoveryRateMod` — **already covered by PERF-07** |
| Leech caps | 1199–1232 | `Max*LeechRate`, `Max*LeechInstance` — **already covered by PERF-07** |
| Regeneration loop | 1234–1320 | `*Regen`, `*RegenPercent`, `*RegenRecovery`, `*Degen`, `*Recovery`, `*RegenInc` — **already covered by PERF-07** |
| ES recharge | 1322–1381 | `EnergyShieldRecharge`, `EnergyShieldRechargeDelay`, `EnergyShieldRechargeAppliesToEnergyShield`, `LifeRecharge` — **already covered by PERF-07** |
| Recoup | 1383–1471 | `*Recoup`, `anyRecoup`, per-damage-type recoup, pseudo-recoup — **DEF-05 specific** |
| Ward recharge delay | 1473–1483 | `WardRechargeDelay` — **already covered by PERF-07** |
| Recovery on block | 1512–1524 | `LifeOnBlock`, `ManaOnBlock`, `EnergyShieldOnBlock`, `EnergyShieldOnSpellBlock`, `LifeOnSuppress`, `EnergyShieldOnSuppress` — **DEF-05 specific** |
| ES recover cap | 1055–1061 | `EnergyShieldRecoveryCap` — **DEF-05 specific** |
| Net regen / TotalBuildDegen | 3368–3459 | `NetLifeRegen`, `NetManaRegen`, `NetEnergyShieldRegen`, `TotalNetRegen` — **DEF-05 specific** |

Commit: `454eff8c85d24356d9b051d596983745ed367476` (third-party/PathOfBuilding, heads/dev)

## Annotated Lua

### Section 1: `EnergyShieldRecoveryCap` (lines 1055–1061)

This is computed inside the primary defences `do` block, right after `Armour`, `Evasion`,
and `EnergyShield` are written. It caps how much ES can be recovered at once.

```lua
-- CappingES is a boolean sentinel: true when some mechanic limits ES recovery to less than full
output.CappingES = modDB:Flag(nil, "ArmourESRecoveryCap") and output.Armour < output.EnergyShield
                   or modDB:Flag(nil, "EvasionESRecoveryCap") and output.Evasion < output.EnergyShield
                   or env.configInput["conditionLowEnergyShield"]
-- Note: Lua `and/or` chaining: each clause is a complete condition
-- "ArmourESRecoveryCap" (Shavronne's Revelation shield): ES recovery capped by Armour
-- "EvasionESRecoveryCap": ES recovery capped by Evasion
-- "conditionLowEnergyShield": config input "Low Energy Shield" condition

if output.CappingES then
    -- Triple ternary computing the minimum applicable cap:
    output.EnergyShieldRecoveryCap =
        modDB:Flag(nil, "ArmourESRecoveryCap") and modDB:Flag(nil, "EvasionESRecoveryCap")
            and m_min(output.Armour, output.Evasion)
        or modDB:Flag(nil, "ArmourESRecoveryCap") and output.Armour
        or modDB:Flag(nil, "EvasionESRecoveryCap") and output.Evasion
        or output.EnergyShield
        or 0
    -- Translation:
    --   if BOTH ArmourESRecoveryCap AND EvasionESRecoveryCap: min(Armour, Evasion)
    --   elif ArmourESRecoveryCap only: Armour
    --   elif EvasionESRecoveryCap only: Evasion
    --   elif neither (just conditionLowEnergyShield): EnergyShield
    --   elif EnergyShield is nil/0: 0

    output.EnergyShieldRecoveryCap =
        env.configInput["conditionLowEnergyShield"]
            and m_min(output.EnergyShield * data.misc.LowPoolThreshold, output.EnergyShieldRecoveryCap)
        or output.EnergyShieldRecoveryCap
    -- LowPoolThreshold = 0.5 (from data.misc)
    -- If "conditionLowEnergyShield" is set: cap = min(ES * 0.5, current cap)
else
    output.EnergyShieldRecoveryCap = output.EnergyShield or 0
    -- Default: full ES pool is the recovery cap
end
```

**Oracle values:** 18/30 builds have `EnergyShieldRecoveryCap > 0` (those with
non-zero `EnergyShield`). The value equals `EnergyShield` in all oracle builds since
none use `ArmourESRecoveryCap` or `EvasionESRecoveryCap`.

**Gotcha — `output.CappingES` is a boolean output field:** In Lua this is stored
directly in the output table. In Rust: `set_output("CappingES", true/false)`.

### Section 2: Recovery on block (lines 1512–1524)

```lua
-- "needs to be after primary defences" (comment at line 1512)
-- These are written after block and primary defences are computed.
output.LifeOnBlock = 0
output.LifeOnSuppress = 0
if not modDB:Flag(nil, "CannotRecoverLifeOutsideLeech") then
    output.LifeOnBlock = modDB:Sum("BASE", nil, "LifeOnBlock")
    output.LifeOnSuppress = modDB:Sum("BASE", nil, "LifeOnSuppress")
end
-- Explicitly initialised to 0 first, then conditionally overwritten.
-- "CannotRecoverLifeOutsideLeech" prevents all life recovery except leech.
-- In Rust: if !flag("CannotRecoverLifeOutsideLeech") { set life_on_block and life_on_suppress }

output.ManaOnBlock = modDB:Sum("BASE", nil, "ManaOnBlock")
-- No "CannotRecover" check for mana — mana on block always applies.

output.EnergyShieldOnBlock = modDB:Sum("BASE", nil, "EnergyShieldOnBlock")
output.EnergyShieldOnSpellBlock = modDB:Sum("BASE", nil, "EnergyShieldOnSpellBlock")
output.EnergyShieldOnSuppress = modDB:Sum("BASE", nil, "EnergyShieldOnSuppress")
-- All zero in oracle builds (no builds use these mods).
```

**Gotcha — `LifeOnBlock` is always initialised to 0** even when
`CannotRecoverLifeOutsideLeech` is set (the flag prevents writing the actual value).
In Rust, always write the initial 0 then conditionally overwrite.

### Section 3: Recoup (lines 1383–1471)

Recoup recovers a percentage of damage taken back as life/mana/ES over 4 seconds
(or 3 seconds with the `3SecondRecoup` flag). This section computes several classes
of recoup:

**3a. Global recoup (lines 1385–1408):**

```lua
output["anyRecoup"] = 0  -- cumulative sentinel
local recoupTypeList = {"Life", "Mana", "EnergyShield"}

for _, recoupType in ipairs(recoupTypeList) do
    local baseRecoup = modDB:Sum("BASE", nil, recoupType.."Recoup")
    if recoupType == "Life" and modDB:Flag(nil, "EnergyShieldRecoupInsteadOfLife") then
        -- Ghost Reaver keystone: life recoup converts to ES recoup
        output.LifeRecoup = 0
        local lifeRecoup = modDB:Sum("BASE", nil, "LifeRecoup")
        modDB:NewMod("EnergyShieldRecoup", "BASE", lifeRecoup, "Life Recoup Conversion")
        -- Injects a new mod instead of writing the output. EnergyShield iteration will pick it up.
    else
        output[recoupType.."Recoup"] = baseRecoup * output[recoupType.."RecoveryRateMod"]
        -- e.g. output.LifeRecoup = Sum("LifeRecoup") * LifeRecoveryRateMod
        output["anyRecoup"] = output["anyRecoup"] + output[recoupType.."Recoup"]
    end
end
```

**3b. Per-damage-type recoup (lines 1417–1440):**

```lua
for _, recoupType in ipairs(recoupTypeList) do      -- Life, Mana, EnergyShield
    for _, damageType in ipairs(dmgTypeList) do     -- Physical, Lightning, Cold, Fire, Chaos
        -- e.g. output["PhysicalLifeRecoup"] = Sum("PhysicalLifeRecoup") * LifeRecoveryRateMod
        if recoupType == "Life" and modDB:Flag(nil, "EnergyShieldRecoupInsteadOfLife") then
            output[damageType.."LifeRecoup"] = 0
            -- convert to ES recoup via modDB injection
        else
            local recoup = modDB:Sum("BASE", nil, damageType..recoupType.."Recoup")
            output[damageType..recoupType.."Recoup"] = recoup * output[recoupType.."RecoveryRateMod"]
            output["anyRecoup"] = output["anyRecoup"] + output[damageType..recoupType.."Recoup"]
        end
    end
end
-- Generates 15 output fields: Physical/Lightning/Cold/Fire/Chaos × Life/Mana/EnergyShield
-- All are 0 in the 30 oracle builds.
```

**3c. Pseudo-recoup (lines 1443–1470):**

```lua
-- "% physical damage prevented from hits regenerated as X"
-- Only triggered when PhysicalDamageMitigated*PseudoRecoup BASE mods exist.
for _, resource in ipairs(recoupTypeList) do
    if not modDB:Flag(nil, "No"..resource.."Regen")
       and not modDB:Flag(nil, "CannotGain"..resource) then
        local PhysicalDamageMitigatedPseudoRecoup =
            modDB:Sum("BASE", nil, "PhysicalDamageMitigated"..resource.."PseudoRecoup")
        if PhysicalDamageMitigatedPseudoRecoup > 0 then
            output["PhysicalDamageMitigated"..resource.."PseudoRecoupDuration"] =
                modDB:Sum("BASE", nil, "PhysicalDamageMitigated"..resource.."PseudoRecoupDuration")
            if output["PhysicalDamageMitigated"..resource.."PseudoRecoupDuration"] == 0 then
                output["PhysicalDamageMitigated"..resource.."PseudoRecoupDuration"] = 4  -- default 4s
            end
            -- Uses regen INC/More, NOT recoup recovery rate:
            local inc = modDB:Sum("INC", nil, resource.."Regen")
            local more = modDB:More(nil, resource.."Regen")
            output["PhysicalDamageMitigated"..resource.."PseudoRecoup"] =
                PhysicalDamageMitigatedPseudoRecoup * (1 + inc/100) * more
                * output[resource.."RecoveryRateMod"]
            output["anyRecoup"] = output["anyRecoup"] + output["PhysicalDamageMitigated"..resource.."PseudoRecoup"]
        end
    end
end
```

**Gotcha — pseudo-recoup uses `*Regen` INC/More, not `*Recoup` modifiers.** This is
intentional: pseudo-recoup (e.g., from Juggernaut's "Enduring Cry grants X% of Phys
damage mitigated as Life") scales with regen modifiers, not recoup modifiers, because
it functions as regeneration triggered by mitigation.

### Section 4: Net regen (lines 3368–3459)

This section runs **much later** in `calcs.defence`, after the full EHP pipeline
(damage taken per type, resistance/reduction multipliers, etc.). It is gated on
`output.TotalBuildDegen` being non-zero.

```lua
if output.TotalBuildDegen == 0 then
    output.TotalBuildDegen = nil  -- clear the sentinel
else
    -- Net regen = regen recovery - degen allocated to each pool
    output.NetLifeRegen = output.LifeRegenRecovery
    output.NetManaRegen = output.ManaRegenRecovery
    output.NetEnergyShieldRegen = output.EnergyShieldRegenRecovery
    -- LifeRegenRecovery etc. are written during the regen loop (PERF-07/DEF-05 section 1)

    -- Allocate each damage type's BuildDegen across Life/Mana/ES based on:
    -- - EnergyShieldBypass (how much bypasses ES)
    -- - MindOverMatter (how much goes to Mana)
    -- - sharedMindOverMatter
    for _, damageType in ipairs(dmgTypeList) do
        if output[damageType.."BuildDegen"] then
            -- Complex allocation across pools, adjusted for ES recharge, MoM, and bypass
            -- See full logic at lines 3403-3440
            totalLifeDegen = totalLifeDegen + lifeDegen
            totalManaDegen = totalManaDegen + manaDegen
            totalEnergyShieldDegen = totalEnergyShieldDegen + energyShieldDegen
        end
    end

    output.NetLifeRegen = output.NetLifeRegen - totalLifeDegen
    output.NetManaRegen = output.NetManaRegen - totalManaDegen
    output.NetEnergyShieldRegen = output.NetEnergyShieldRegen - totalEnergyShieldDegen
    output.TotalNetRegen = output.NetLifeRegen + output.NetManaRegen + output.NetEnergyShieldRegen
end
```

**Oracle data:** Only 1/30 builds (`rf_juggernaut`) has active `TotalBuildDegen`
(from RF self-ignite). That build shows:
- `NetLifeRegen = -259.8`  (negative — degen exceeds regen)
- `NetManaRegen = 10.6`
- `TotalNetRegen = -249.2`

**Gotcha — `NetLifeRegen` is only written when `TotalBuildDegen != 0`.** If no
self-applied degen exists, these fields are absent from the output table entirely —
not written as zero. In Rust: only call `set_output("NetLifeRegen", ...)` when there
is active build degen.

**Gotcha — the degen allocation requires `output[damageType.."BuildDegen"]`** which
is computed in the EHP section earlier in `calcs.defence`. It is the actual DoT
damage the build takes per type after taking its own DoT into account.

## Existing Rust Code

**File:** `crates/pob-calc/src/calc/defence.rs`

The Rust `defence.rs` maps to DEF-01 through DEF-05 per spec section 13. For recovery
specifically:

| Section | Rust function | Status |
|---------|--------------|--------|
| `*RecoveryRateMod` | `calc_recovery_rates` (~line 530) | ✅ Exists — see PERF-07 doc |
| Leech caps | `calc_leech_caps` (~line 545) | ✅ Exists — see PERF-07 doc |
| Regen loop | `calc_regeneration` (~line 643) | ⚠️ Partial — see PERF-07 doc |
| ES recharge | `calc_es_recharge` (~line 683) | ⚠️ Bugs — see PERF-07 doc |
| **`EnergyShieldRecoveryCap`** | Not found | ❌ **Missing** |
| **Recovery on block** (`LifeOnBlock` etc.) | `calc_movement_and_avoidance` (~line 737) | ✅ Writes `LifeOnBlock`, `ManaOnBlock`, `EnergyShieldOnBlock` — but **missing** `CannotRecoverLifeOutsideLeech` guard for `LifeOnBlock` |
| **Recoup** | Not found | ❌ **Missing entirely** |
| **Net regen** | Not found | ❌ **Missing entirely** |

### Recovery on block detail

```rust
// defence.rs ~line 737-744
for resource in &["Life", "Mana", "EnergyShield"] {
    let stat = format!("{resource}OnBlock");
    let val = env.player.mod_db
        .sum_cfg(ModType::Base, &stat, None, &output);
    env.player.set_output(&stat, val);
}
```

**Bugs vs Lua:**
1. `LifeOnBlock` should be forced to 0 when `CannotRecoverLifeOutsideLeech` is set. The
   Rust does not check this flag.
2. `LifeOnSuppress` and `EnergyShieldOnSuppress` are not written by Rust at all.
3. `EnergyShieldOnSpellBlock` is not written by Rust at all.

## What Needs to Change

1. **Populate `field_groups.rs` for DEF-05** — currently empty. Based on this analysis,
   the following fields should be added to `DEF-05-recovery-in-defence`:
   ```rust
   "DEF-05-recovery-in-defence" => &[
       "EnergyShieldRecoveryCap",
       "LifeOnBlock",
       "ManaOnBlock",
       "EnergyShieldOnBlock",
       "EnergyShieldOnSpellBlock",
       "LifeOnSuppress",
       "EnergyShieldOnSuppress",
       "LifeRecoup",
       "ManaRecoup",
       "EnergyShieldRecoup",
       "anyRecoup",
   ],
   ```
   Fields like `*RegenPercent`, `*RegenRecovery`, `*RegenInc`, `NetLifeRegen`,
   `NetManaRegen`, `TotalNetRegen`, `WardRechargeDelay` should be added to **PERF-07**'s
   field group instead.

2. **Implement `EnergyShieldRecoveryCap`** (`defence.rs`, after primary defences):
   ```rust
   let capping_es = mod_db.flag("ArmourESRecoveryCap") && armour < es
                 || mod_db.flag("EvasionESRecoveryCap") && evasion < es
                 || config.condition_low_energy_shield;
   let mut es_cap = if capping_es {
       let cap = if mod_db.flag("ArmourESRecoveryCap") && mod_db.flag("EvasionESRecoveryCap") {
           armour.min(evasion)
       } else if mod_db.flag("ArmourESRecoveryCap") {
           armour
       } else if mod_db.flag("EvasionESRecoveryCap") {
           evasion
       } else {
           es  // conditionLowEnergyShield only
       };
       if config.condition_low_energy_shield {
           cap.min(es * 0.5) // LowPoolThreshold = 0.5
       } else {
           cap
       }
   } else {
       es  // default: full ES pool
   };
   set_output("EnergyShieldRecoveryCap", es_cap);
   set_output("CappingES", capping_es);
   ```

3. **Fix `LifeOnBlock` to respect `CannotRecoverLifeOutsideLeech`** (`defence.rs`,
   `calc_movement_and_avoidance`):
   ```rust
   let cannot_recover_life = mod_db.flag("CannotRecoverLifeOutsideLeech");
   let life_on_block = if cannot_recover_life { 0.0 } else {
       mod_db.sum_cfg(Base, "LifeOnBlock", None, &output)
   };
   set_output("LifeOnBlock", life_on_block);
   let life_on_suppress = if cannot_recover_life { 0.0 } else {
       mod_db.sum_cfg(Base, "LifeOnSuppress", None, &output)
   };
   set_output("LifeOnSuppress", life_on_suppress);
   ```

4. **Add missing on-block/on-suppress fields** (`defence.rs`):
   ```rust
   // Currently missing:
   set_output("EnergyShieldOnSpellBlock",
       mod_db.sum_cfg(Base, "EnergyShieldOnSpellBlock", None, &output));
   set_output("EnergyShieldOnSuppress",
       mod_db.sum_cfg(Base, "EnergyShieldOnSuppress", None, &output));
   ```

5. **Implement recoup section** (`defence.rs`, after `calc_regeneration`):
   - Global recoup: for each of Life/Mana/EnergyShield, compute
     `Sum("*Recoup") * *RecoveryRateMod` (with Ghost Reaver conversion)
   - Per-damage-type recoup: 15 fields (`{Phys/Light/Cold/Fire/Chaos}{Life/Mana/ES}Recoup`)
   - `anyRecoup` accumulator (sum of all non-zero recoup values)
   - Per-damage-type pseudo-recoup: only when `PhysicalDamageMitigated*PseudoRecoup > 0`
   - Pseudo-recoup uses Regen INC/More (not Recoup modifiers)

6. **Implement Net regen section** (`defence.rs`, after EHP calculation):
   This requires `output[damageType.."BuildDegen"]` to be available, which means it
   must run after `defence_ehp.rs`. The logic:
   - Skip entirely when `TotalBuildDegen == 0`
   - Start from `*RegenRecovery` values
   - For each damage type with `*BuildDegen`, allocate across Life/Mana/ES using
     ES bypass percentage, MindOverMatter percentage
   - Write `NetLifeRegen`, `NetManaRegen`, `NetEnergyShieldRegen`, `TotalNetRegen`

7. **Add unassigned fields to PERF-07 field group** in `field_groups.rs`:
   - `LifeRegenPercent` (oracle: 14/30 non-zero)
   - `ManaRegenPercent` (oracle: 30/30 non-zero)
   - `LifeRegenRecovery` (oracle: 14/30 non-zero)
   - `ManaRegenRecovery` (oracle: 30/30 non-zero)
   - `ManaRegenInc` (oracle: 11/30 non-zero)
   - `WardRechargeDelay` (oracle: 30/30 non-zero)
   - `NetLifeRegen` (oracle: 1/30 non-zero)
   - `NetManaRegen` (oracle: 1/30 non-zero)
   - `TotalNetRegen` (oracle: 1/30 non-zero)
   - `EnergyShieldRechargeAppliesToEnergyShield` (oracle: 30/30 non-zero)

## Oracle Confirmation

Fields that appear in oracle and belong to DEF-05's scope:

| Field | Count non-zero | Sample value | Builds |
|-------|----------------|-------------|--------|
| `EnergyShieldRecoveryCap` | 18/30 | `aura_stacker: 2659` | All ES builds = ES value |
| `EnergyShieldRechargeAppliesToEnergyShield` | 30/30 | `True` | All builds |
| `LifeOnBlock` | 0/30 | `0` | No builds use life on block |
| `ManaOnBlock` | 0/30 | `0` | |
| `EnergyShieldOnBlock` | 0/30 | `0` | |
| `LifeRecoup` | 0/30 | `0` | No builds use recoup |
| `ManaRecoup` | 1/30 | `poison_pathfinder: 0` | One build has mana recoup but it's 0% |
| `anyRecoup` | 1/30 | `poison_pathfinder: 0` | |
| `NetLifeRegen` | 1/30 | `rf_juggernaut: -259.8` | RF self-burn |
| `NetManaRegen` | 1/30 | `rf_juggernaut: 10.6` | |
| `TotalNetRegen` | 1/30 | `rf_juggernaut: -249.2` | |

### rf_juggernaut full regen snapshot (the one build exercising Net regen)

```
LifeRegen: 0             (RF prevents life regen)
ManaRegen: 11.8
EnergyShieldRegen: 0
LifeRegenRecovery: -259.8  (regen = 0 but degen is positive => negative)
ManaRegenRecovery: 10.6
EnergyShieldRechargeDelay: 2
EnergyShieldRecharge: 0
NetLifeRegen: -259.8
NetManaRegen: 10.6
NetEnergyShieldRegen: 0
TotalNetRegen: -249.2
```

> Note: `LifeRegenRecovery` being negative is intentional. The regen loop computes
> `RegenRecovery = regenRate - degenRate + recoveryRate`. For RF juggernaut:
> `regenRate = 0` (no life regen due to RF), `degenRate = 259.8` (RF degen), so
> `LifeRegenRecovery = -259.8`. Then `NetLifeRegen = LifeRegenRecovery - lifeDegen`
> where `lifeDegen` comes from the EHP degen loop. Since RF degen is already in
> `LifeRegenRecovery`, the `BuildDegen` allocation would add additional DoT the
> build takes from outside sources.
