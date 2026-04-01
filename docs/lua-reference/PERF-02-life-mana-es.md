# PERF-02-life-mana-es: Life, Mana, Energy Shield, Ward Pools

## Output Fields

| Field | Lua location | Notes |
|-------|-------------|-------|
| `Life` | CalcPerform.lua:82,90 | CI → 1; else `override or round(base*(1+inc/100)*more*(1-conv/100))`, min 1 |
| `Mana` | CalcPerform.lua:109 | `round(calcLib.val(modDB,"Mana") * (1-manaConv/100))` |
| `EnergyShield` | CalcDefence.lua:1044 | Gear slot accumulation + conversions + override |
| `Ward` | CalcDefence.lua:1050 | `m_floor(ward)`, min 0 |
| `EnergyShieldRecoveryCap` | CalcDefence.lua:1058–1061 | Armour/Evasion cap or ES itself |
| `LifeUnreserved` | CalcPerform.lua:535 | `max - reserved` (can be negative) |
| `LifeUnreservedPercent` | CalcPerform.lua:536 | `(max - reserved) / max * 100` |
| `ManaUnreserved` | CalcPerform.lua:535 | Same pattern as Life |
| `ManaUnreservedPercent` | CalcPerform.lua:536 | Same pattern as Life |
| `LifeRecoverable` | CalcDefence.lua:2205–2218 | `LifeUnreserved`, capped when LowLife config or DamageInsteadReservesLife |
| `ManaRecoverable` | *(not a written field)* | Not written in any Lua source; absent from the oracle JSON |

## Dependencies

- **PERF-01-attributes**: Str → Life BASE mod, Int → Mana BASE mod, Int → EnergyShield INC mod must all be injected before pools are summed.
- **PERF-04-reservation**: `LifeUnreserved` / `ManaUnreserved` depend on `reserved_LifeBase`, `reserved_LifePercent` being set by the reservation pass. The Lua calls `doActorLifeManaReservation` after pools are calculated. In Rust, `do_actor_life_mana_reservation` is called after `do_actor_life_mana`.
- **DEF-02-armour-evasion-es-ward**: `EnergyShieldRecoveryCap` depends on `output.Armour`, `output.Evasion`, and `output.EnergyShield` — all written by CalcDefence. In the Lua execution order, `EnergyShieldRecoveryCap` is written inside `CalcDefence.lua` directly after the primary defence block. Rust mirrors this by computing it inside `calc_primary_defences`.

## Lua Source

**File 1:** `third-party/PathOfBuilding/src/Modules/CalcPerform.lua`  
**Commit:** `454eff8c85d24356d9b051d596983745ed367476`  
Relevant lines: 68–130 (`doActorLifeMana`), 519–553 (`doActorLifeManaReservation`)

**File 2:** `third-party/PathOfBuilding/src/Modules/CalcDefence.lua`  
**Commit:** `454eff8c85d24356d9b051d596983745ed367476`  
Relevant lines: 824–1062 (primary defences block, writes `EnergyShield`, `Ward`, `EnergyShieldRecoveryCap`), 2204–2218 (`LifeRecoverable`)

---

## Annotated Lua

### `doActorLifeMana` — Life pool (CalcPerform.lua:68–107)

```lua
function doActorLifeMana(actor)
    local modDB = actor.modDB
    local output = actor.output
    local breakdown = actor.breakdown
    local condList = modDB.conditions

    -- These two output fields are used later in the defence pass and in LifeRecoverable.
    local lowLifePerc = modDB:Sum("BASE", nil, "LowLifePercentage")
    output.LowLifePercentage = 100.0 * (lowLifePerc > 0 and lowLifePerc or data.misc.LowPoolThreshold)
    -- ^^^ data.misc.LowPoolThreshold = 0.5 (Data.lua:167)
    -- So: output.LowLifePercentage = 100.0 * 0.5 = 50.0 by default.
    -- Translates as: 100.0 * (if lowLifePerc > 0 { lowLifePerc } else { 0.5 })
    -- Rust gotcha: `a and b or c` ternary — safe here because `b` (lowLifePerc) is a number
    -- and could be 0 when truthy, so use explicit if-else not the and/or pattern.

    local fullLifePerc = modDB:Sum("BASE", nil, "FullLifePercentage")
    output.FullLifePercentage = 100.0 * (fullLifePerc > 0 and fullLifePerc or 1.0)
    -- Default: 100.0 * 1.0 = 100.0

    output.ChaosInoculation = modDB:Flag(nil, "ChaosInoculation")
    -- Written as a bool to output; used as a read-through by later code.

    if output.ChaosInoculation then
        output.Life = 1          -- Fixed at 1 for CI builds
        condList["FullLife"] = true
    else
        local base = modDB:Sum("BASE", nil, "Life")
        local inc  = modDB:Sum("INC", nil, "Life")
        local more = modDB:More(nil, "Life")
        local override = modDB:Override(nil, "Life")
        -- override: Option<f64> — if present, bypasses the normal formula entirely.
        -- In Rust: mod_db.override_value("Life", None, output)

        local conv = modDB:Sum("BASE", nil, "LifeConvertToEnergyShield")
        -- "LifeConvertToEnergyShield" percentage of life lost to ES conversion (Zealot's Oath etc.)
        -- Reduces effective life: multiply by (1 - conv/100)

        output.Life = override or m_max(round(base * (1 + inc/100) * more * (1 - conv/100)), 1)
        -- Lua `or` coalesces nil: if override is non-nil, use it; else use formula.
        -- m_max(..., 1): life is always at least 1.
        -- round(): standard rounding, NOT floor.

        -- IMPORTANT: conv is a percentage (e.g. 100 means 100% of life converted = Life = 0,
        -- but clamped to min 1). Do NOT clamp conv to [0,100] before applying — PoB doesn't.
        -- The formula can produce negative pre-max values; m_max(...,1) handles that.
    end
```

**Rust translation of Life:**
```rust
let ci = mod_db.flag(None, "ChaosInoculation", output);
if ci {
    output.set("Life", 1.0);
    mod_db.set_condition("FullLife", true);
} else {
    let base  = mod_db.sum(None, ModType::Base, "Life", output);
    let inc   = mod_db.sum(None, ModType::Inc,  "Life", output);
    let more  = mod_db.more(None, "Life", output);
    let conv  = mod_db.sum(None, ModType::Base, "LifeConvertToEnergyShield", output);
    let life  = mod_db.override_value("Life", None, output)
        .unwrap_or_else(|| (base * (1.0 + inc/100.0) * more * (1.0 - conv/100.0))
            .round().max(1.0));
    output.set("Life", life);
}
```

---

### `doActorLifeMana` — Mana pool (CalcPerform.lua:108–128)

```lua
    local manaConv = modDB:Sum("BASE", nil, "ManaConvertToArmour")
    -- "ManaConvertToArmour": percentage of mana converted to Armour (Shavronne's Wrappings etc.)
    -- Reduces effective mana: multiply by (1 - manaConv/100)

    output.Mana = round(calcLib.val(modDB, "Mana") * (1 - manaConv / 100))
    -- calcLib.val(modDB, "Mana") = Base * (1+INC/100) * More, BUT short-circuits to 0 if Base==0.
    -- See PERF-01 annotation: if no Base mods, calcLib.val returns 0 regardless of INC/More.
    -- round(): standard rounding. No min-0 clamp — Mana can be 0 or negative (unusual).

    -- No override check here — unlike Life, Mana has no Override path in this function.
    -- Note: base/inc/more are re-read below only for breakdown, not for the calculation.
    local base = modDB:Sum("BASE", nil, "Mana")
    local inc  = modDB:Sum("INC", nil, "Mana")
    local more = modDB:More(nil, "Mana")
    -- These re-reads are breakdown-only; the actual value uses calcLib.val above.
```

**Rust translation of Mana:**
```rust
let mana_conv = mod_db.sum(None, ModType::Base, "ManaConvertToArmour", output);
let base  = mod_db.sum(None, ModType::Base, "Mana", output);
let inc   = mod_db.sum(None, ModType::Inc,  "Mana", output);
let more  = mod_db.more(None, "Mana", output);
// calcLib.val short-circuit: if base == 0, result is 0 regardless of inc/more
let mana_pre_conv = if base == 0.0 { 0.0 } else { base * (1.0 + inc/100.0) * more };
let mana = (mana_pre_conv * (1.0 - mana_conv/100.0)).round();
output.set("Mana", mana);
```

---

### `doActorLifeManaReservation` — Unreserved pools (CalcPerform.lua:519–553)

```lua
function doActorLifeManaReservation(actor, addAura)
    local modDB  = actor.modDB
    local output = actor.output
    local condList = modDB.conditions

    for _, pool in pairs({"Life", "Mana"}) do
        -- pool = "Life" or "Mana"
        local max = output[pool]   -- e.g. output.Life or output.Mana
        local reserved
        if max > 0 then
            local lowPerc = modDB:Sum("BASE", nil, "Low" .. pool .. "Percentage")
            -- "LowLifePercentage" or "LowManaPercentage" — base fraction (e.g. 0.35)

            reserved = (actor["reserved_"..pool.."Base"] or 0)
                     + m_ceil(max * (actor["reserved_"..pool.."Percent"] or 0) / 100)
            -- actor.reserved_LifeBase: flat life reserved (from Blood Magic etc.)
            -- actor.reserved_LifePercent: % of life reserved (from auras etc.)
            -- Percent portion uses m_ceil, NOT round or floor.
            -- GOTCHA: This means 1% of 1000 Life = ceil(10) = 10, but 0.5% of 1000 = ceil(5) = 5.
            -- The flat part is added directly (already a flat amount, not a percentage).

            uncancellableReservation = actor["uncancellable_"..pool.."Reservation"] or 0
            -- Note: this is a global (no `local`) — Lua bug in PoB but inconsequential here.

            output[pool.."Reserved"]        = m_min(reserved, max)
            output[pool.."ReservedPercent"] = m_min(reserved / max * 100, 100)
            output[pool.."Unreserved"]      = max - reserved
            -- CRITICAL: Unreserved can be NEGATIVE if reserved > max.
            -- PoB does NOT clamp this. The UI warns "Your unreserved Life is below 1".
            -- Rust must NOT add .max(0.0) here.

            output[pool.."UnreservedPercent"] = (max - reserved) / max * 100
            -- Can also be negative.

            output[pool.."UncancellableReservation"] = m_min(uncancellableReservation, 0)
            -- Bug in PoB? m_min(value, 0) always returns ≤0 — but this is what Lua does.
            output[pool.."CancellableReservation"] = 100 - uncancellableReservation
            -- Percentage of pool that can be unreserved by deactivating skills.

            if (max - reserved) / max <= (lowPerc > 0 and lowPerc or data.misc.LowPoolThreshold) then
                condList["Low"..pool] = true
                -- LowLife or LowMana condition set here based on unreserved fraction.
                -- LowPoolThreshold = 0.5 → triggers at ≤50% unreserved.
                -- Custom lowPerc is the raw fraction (0.35 for 35%), not a percentage.
            end
        else
            reserved = 0
            -- If max == 0, nothing is written to output (no Unreserved, no Reserved fields).
        end
        -- addAura branch (GrantReservedPoolAsAura) is not relevant for PERF-02 output fields.
    end
end
```

**Key Rust gotchas for reservation:**
- `m_ceil(max * percent / 100)` — use `.ceil()`, not `.round()` or `.floor()`.
- `LifeUnreserved = max - reserved` — **no clamping**. Can legitimately be ≤ 0.
- LowLife/LowMana condition uses `(max - reserved) / max ≤ threshold` (fraction comparison).

---

### `EnergyShield` and `Ward` — CalcDefence.lua:824–1062

The ES and Ward computation is a two-phase accumulation inside CalcDefence.lua:

**Phase 1: Per-slot accumulation (lines 843–923)**
```lua
for _, slot in pairs({"Helmet","Gloves","Boots","Body Armour","Weapon 2","Weapon 3"}) do
    local armourData = actor.itemList[slot] and actor.itemList[slot].armourData
    if armourData then
        slotCfg.slotName = slot
        energyShieldBase = not modDB:Flag(nil, "GainNoEnergyShieldFrom"..slot) and armourData.EnergyShield or 0
        wardBase         = not modDB:Flag(nil, "GainNoWardFrom"..slot) and armourData.Ward or 0

        -- EnergyShieldToWard keystone: ES items contribute to Ward instead
        if energyShieldBase > 0 then
            if modDB:Flag(nil, "EnergyShieldToWard") then
                energyShield = energyShield + energyShieldBase * modDB:More(slotCfg, "EnergyShield", "Defences")
                -- NO inc applied when EnergyShieldToWard — More only!
            elseif not modDB:Flag(nil, "ConvertArmourESToLife") then
                energyShield = energyShield + energyShieldBase
                    * calcLib.mod(modDB, slotCfg, "EnergyShield", "Defences", slot.."ESAndArmour")
                -- calcLib.mod = (1 + INC/100) * More; slot-scoped cfg
            end
        end
        -- Similar branches for wardBase
    end
end
```

**Phase 2: Global modifiers (lines 925–1043)**
```lua
-- Global Ward
wardBase = modDB:Sum("BASE", nil, "Ward")
if wardBase > 0 then
    if modDB:Flag(nil, "EnergyShieldToWard") then
        local inc  = modDB:Sum("INC", nil, "Ward", "Defences", "EnergyShield")
        local more = modDB:More(nil, "Ward", "Defences")
        ward = ward + wardBase * (1 + inc / 100) * more
    else
        ward = ward + wardBase * calcLib.mod(modDB, nil, "Ward", "Defences")
    end
end

-- Global ES (non-slot mods from passives, flasks, etc.)
energyShieldBase = modDB:Sum("BASE", nil, "EnergyShield")
if energyShieldBase > 0 then
    if modDB:Flag(nil, "EnergyShieldToWard") then
        energyShield = energyShield + energyShieldBase * modDB:More(nil, "EnergyShield", "Defences")
    else
        energyShield = energyShield + energyShieldBase
            * calcLib.mod(modDB, nil, "EnergyShield", "Defences")
    end
end

-- Mana → Armour conversion (line 990): reduces mana pool for armour
local convManaToArmour = modDB:Sum("BASE", nil, "ManaConvertToArmour")
-- Mana → ES conversion (line 999)
local convManaToES = modDB:Sum("BASE", nil, "ManaGainAsEnergyShield")
if convManaToES > 0 then
    energyShieldBase = modDB:Sum("BASE", nil, "Mana") * convManaToES / 100
    energyShield = energyShield + energyShieldBase
        * calcLib.mod(modDB, nil, "Mana", "EnergyShield", "Defences")
end

-- Life → ES conversion (line 1021): LifeConvertToEnergyShield + LifeGainAsEnergyShield
local convLifeToES = modDB:Sum("BASE", nil, "LifeConvertToEnergyShield", "LifeGainAsEnergyShield")
if convLifeToES > 0 then
    energyShieldBase = modDB:Sum("BASE", nil, "Life") * convLifeToES / 100
    local total
    if modDB:Flag(nil, "ChaosInoculation") then
        total = 1   -- CI: life is 1, so conversion also gives just 1
    else
        total = energyShieldBase * calcLib.mod(modDB, nil, "Life", "EnergyShield", "Defences")
        -- Note: mod query uses both "Life" AND "EnergyShield" AND "Defences" names
    end
    energyShield = energyShield + total
end
```

**Final writes (lines 1044–1061):**
```lua
output.EnergyShield = modDB:Override(nil, "EnergyShield") or m_max(round(energyShield), 0)
-- Override takes priority; else round the accumulated float, min 0.

output.Ward = m_max(m_floor(ward), 0)
-- Ward uses floor, NOT round. This differs from EnergyShield (which uses round).

output.CappingES = modDB:Flag(nil, "ArmourESRecoveryCap") and output.Armour < output.EnergyShield
               or modDB:Flag(nil, "EvasionESRecoveryCap") and output.Evasion < output.EnergyShield
               or env.configInput["conditionLowEnergyShield"]
-- CappingES: true when ES recovery is capped by Armour/Evasion (Stasis Prison node)
-- or the "Low Energy Shield" config checkbox is checked.

if output.CappingES then
    -- Priority: both ArmourESRecoveryCap AND EvasionESRecoveryCap → min(Armour, Evasion)
    --           only ArmourESRecoveryCap → Armour
    --           only EvasionESRecoveryCap → Evasion
    --           neither → EnergyShield
    output.EnergyShieldRecoveryCap =
        modDB:Flag(nil, "ArmourESRecoveryCap") and modDB:Flag(nil, "EvasionESRecoveryCap") and m_min(output.Armour, output.Evasion)
        or modDB:Flag(nil, "ArmourESRecoveryCap") and output.Armour
        or modDB:Flag(nil, "EvasionESRecoveryCap") and output.Evasion
        or output.EnergyShield or 0
    -- Additional cap: "Low ES" config → cap at LowPoolThreshold fraction of ES
    output.EnergyShieldRecoveryCap =
        env.configInput["conditionLowEnergyShield"]
        and m_min(output.EnergyShield * data.misc.LowPoolThreshold, output.EnergyShieldRecoveryCap)
        or output.EnergyShieldRecoveryCap
else
    output.EnergyShieldRecoveryCap = output.EnergyShield or 0
end
```

**Lua `and/or` chain gotcha** on the EnergyShieldRecoveryCap line:
```
A and B and C or A and B or A and D or E or 0
```
This is a cascading ternary. In Rust, translate as nested `if`:
```rust
let cap = if armour_es_cap && evasion_es_cap {
    armour.min(evasion)
} else if armour_es_cap {
    armour
} else if evasion_es_cap {
    evasion
} else {
    energy_shield  // output.EnergyShield or 0
};
```
Note `output.EnergyShield or 0` — the `or 0` coalesces nil, but in Rust ES is always f64 so just use the value directly.

---

### `LifeRecoverable` — CalcDefence.lua:2204–2218

```lua
-- LifeRecoverable: the amount of life that can actually be recovered.
-- Normally equals LifeUnreserved. Reduced if "Low Life" config or Dissolution of the Flesh.

output.LifeRecoverable = output.LifeUnreserved
-- Default: equal to unreserved life.

if env.configInput["conditionLowLife"] then
    -- "Low Life" checkbox in PoB config UI — simulates being perpetually at low life.
    output.LifeRecoverable = m_min(
        output.Life * (output.LowLifePercentage or data.misc.LowPoolThreshold) / 100,
        output.LifeUnreserved)
    -- Cap at the low-life threshold fraction of max life.
    -- data.misc.LowPoolThreshold = 0.5; output.LowLifePercentage is set in doActorLifeMana
    --   as 100 * fraction (e.g. 50.0), so divide by 100 here.
    if output.LifeRecoverable < output.LifeUnreserved then
        output.CappingLife = true
        -- Flag used by the display stats to show "Total Recoverable" label
    end
end

-- Dissolution of the Flesh: life recovery is based on cancellable reservation amount.
if modDB:Flag(nil, "DamageInsteadReservesLife") then
    output.LifeRecoverable = (output.LifeCancellableReservation / 100) * output.Life
    -- LifeCancellableReservation is set in the reservation pass (CalcPerform.lua:538).
end

output.LifeRecoverable = m_max(output.LifeRecoverable, 1)
-- Always at least 1 to prevent division-by-zero in EHP calculations.
```

---

## Existing Rust Code

**`perform.rs`** (Life and Mana): lines 294–434 (`do_actor_life_mana`), 440–488 (`do_actor_life_mana_reservation`)

**`defence.rs`** (ES, Ward, EnergyShieldRecoveryCap): lines 337–426 (`calc_primary_defences`)

`LifeRecoverable` and `EnergyShieldRecoveryCap` are **not present** in Rust at all.

### What Exists

**Life (`do_actor_life_mana`, perform.rs:294–388)**
- CI path: ✓ Life = 1, FullLife condition set, ChaosInoculation condition set.
- Normal path: ✓ Base/Inc/More/conv formula with `round()` and `max(1.0)`.
- `LowLifePercentage` / `FullLifePercentage` outputs: ✗ not written.
- `ChaosInoculation` bool output: ✗ not written (condition is set, but `output.ChaosInoculation` is not).
- Life Override (`modDB:Override(nil, "Life")`): ✗ not checked.

**Mana (`do_actor_life_mana`, perform.rs:390–415)**
- Normal path: ✓ Base/Inc/More formula with `round()`.
- `ManaConvertToArmour` conversion factor: ✗ missing entirely.
- `calcLib.val` short-circuit (0 if base == 0): ✗ Rust applies inc/more even with zero base (harmless numerically but semantically different).

**Reservation (`do_actor_life_mana_reservation`, perform.rs:440–488)**
- `LifeUnreserved` / `ManaUnreserved`: ✓ written.
- `LifeUnreservedPercent` / `ManaUnreservedPercent`: ✓ written.
- Percent-portion rounding: **WRONG** — Rust uses `.floor()` (`perform.rs:446,463`) but Lua uses `m_ceil()`. See annotation above.
- Unreserved clamped to `max(0.0)`: **WRONG** — Rust adds `.max(0.0)` (`perform.rs:448,465`), but Lua allows negative values. The display layer warns the user, but the oracle JSON can contain negative values.
- LowMana condition: ✓ set, but uses hardcoded 50% (`perform.rs:486`) instead of reading `LowManaPercentage` from modDB.
- LowLife condition: ✓ set in `do_actor_life_mana`, not in the reservation function — that is correct (the Lua sets it here from the unreserved fraction, but only when `max > 0`). However, the Rust condition check is in `do_actor_life_mana` using different logic (unreserved_pct calculation) rather than in `do_actor_life_mana_reservation`. The Lua sets it in `doActorLifeManaReservation`.
- `LifeReservedPercent` / `ManaReservedPercent`: ✓ written; these are PERF-04 fields but computed alongside.
- `UncancellableReservation` / `CancellableReservation`: ✗ not written.

**EnergyShield (`calc_primary_defences`, defence.rs:355–368)**
- Simple Base/Inc/More formula: present but fundamentally wrong. The Lua accumulates ES slot-by-slot with slot-scoped `calcLib.mod` calls, applies `EnergyShieldToWard` keystone, and adds conversion paths from Mana and Life. The Rust sums a single global Base/Inc/More, which is only correct for builds with no gear, no conversions, and no EnergyShieldToWard.
- `Override` for EnergyShield: ✗ not checked.
- Rounding: Rust uses `.floor()` (`defence.rs:366`) but Lua uses `round()`.

**Ward (`calc_primary_defences`, defence.rs:340–353)**
- Simple Base/Inc/More formula: present; rounding matches (`.floor()`).
- `EnergyShieldToWard` path (ES items contribute to Ward): ✗ missing.

**EnergyShieldRecoveryCap**: ✗ completely absent.

**LifeRecoverable**: ✗ completely absent.

---

## What Needs to Change

1. **Write `output.LowLifePercentage` and `output.FullLifePercentage`** in `do_actor_life_mana`:  
   ```rust
   let low_life_perc_raw = mod_db.sum(None, ModType::Base, "LowLifePercentage", output);
   let low_life_threshold = if low_life_perc_raw > 0.0 { low_life_perc_raw } else { 0.5 };
   output.set("LowLifePercentage", 100.0 * low_life_threshold);
   let full_life_perc_raw = mod_db.sum(None, ModType::Base, "FullLifePercentage", output);
   output.set("FullLifePercentage", 100.0 * (if full_life_perc_raw > 0.0 { full_life_perc_raw } else { 1.0 }));
   ```

2. **Write `output.ChaosInoculation`** bool to the output table (in addition to the condition):  
   `output.set("ChaosInoculation", true)`

3. **Add Life Override check** before the formula:  
   ```rust
   if let Some(ov) = mod_db.override_value("Life", None, output) {
       output.set("Life", ov);
   } else { /* normal formula */ }
   ```

4. **Fix Mana: add `ManaConvertToArmour` multiplier**:  
   ```rust
   let mana_conv = mod_db.sum(None, ModType::Base, "ManaConvertToArmour", output);
   let mana = (mana_pre_conv * (1.0 - mana_conv / 100.0)).round();
   ```

5. **Fix reservation rounding: floor → ceil for percent-derived portion**:  
   In `do_actor_life_mana_reservation`, change `.floor()` to `.ceil()` for the percent portion:
   - `perform.rs:446`: `(reserved_life_percent / 100.0 * life).floor()` → `.ceil()`
   - `perform.rs:463`: `(reserved_mana_percent / 100.0 * mana).floor()` → `.ceil()`

6. **Remove `.max(0.0)` clamp on `LifeUnreserved` and `ManaUnreserved`**:  
   - `perform.rs:448`: remove `.max(0.0)` from `life_unreserved`
   - `perform.rs:465`: remove `.max(0.0)` from `mana_unreserved`

7. **Fix LowMana to use `LowManaPercentage`** from modDB instead of hardcoded 50%:  
   ```rust
   let low_mana_perc = mod_db.sum(None, ModType::Base, "LowManaPercentage", output);
   let threshold = if low_mana_perc > 0.0 { low_mana_perc } else { 0.5 };
   mod_db.set_condition("LowMana", mana_unreserved / mana <= threshold);
   ```

8. **Fix LowLife: move condition set to reservation function**:  
   Remove the LowLife condition logic from `do_actor_life_mana`. Set it in `do_actor_life_mana_reservation` from the `(max - reserved) / max <= threshold` check, matching Lua exactly.

9. **Fix EnergyShield: use slot-by-slot accumulation in defence.rs**:  
   The single-query formula is wrong for ES. Need to:
   - Iterate gear slots with per-slot cfg (slot-scoped `calcLib.mod`).
   - Handle `GainNoEnergyShieldFrom{slot}` flags.
   - Handle `EnergyShieldToWard` keystone (ES items → Ward, skipping INC).
   - Handle `ConvertArmourESToLife` flag.
   - Add Mana→ES, Life→ES, and Evasion→Armour conversion paths.
   - Use `mod_db.override_value("EnergyShield")` before the formula.
   - Use `.round()` (not `.floor()`) for the final `output.EnergyShield`.

10. **Implement `EnergyShieldRecoveryCap`** in `calc_primary_defences` after ES and Armour/Evasion are finalised:  
    Implement the `CappingES` flag check + the priority chain (`ArmourESRecoveryCap`, `EvasionESRecoveryCap`, fallback to ES) + `conditionLowEnergyShield` secondary cap.

11. **Implement `LifeRecoverable`** in `calc_primary_defences` or a new function called from `defence.rs::run`, after `LifeUnreserved` is available:  
    Base = `LifeUnreserved`; if `conditionLowLife` → cap at `LowLifePercentage/100 * Life`; if `DamageInsteadReservesLife` → `(LifeCancellableReservation / 100) * Life`; then `max(result, 1.0)`.
