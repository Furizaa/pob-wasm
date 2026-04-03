# PERF-04-reservation: Life & Mana Reservation

## Output Fields

| Field | Notes |
|-------|-------|
| `ManaReserved` | Total mana reserved (flat), capped at `Mana` for display |
| `ManaReservedPercent` | `min(total_reserved / max * 100, 100)` |
| `LifeReserved` | Total life reserved (flat), capped at `Life` for display |
| `LifeReservedPercent` | `min(total_reserved / max * 100, 100)` |

**Fields in `field_groups.rs` but not in Lua or oracle outputs:**
`ManaReservedP` and `LifeReservedP` — these field names appear in `field_groups.rs`
but do not exist in the Lua source or in any oracle expected JSON. They are phantom
entries and should be removed from the field group registry.

**Fields written by Lua but not in the chunk field list** (belong to PERF-02 or are
auxiliary):
`LifeUnreserved`, `LifeUnreservedPercent`, `ManaUnreserved`, `ManaUnreservedPercent`
(already in PERF-02's field group), `LifeUncancellableReservation`,
`LifeCancellableReservation`, `ManaUncancellableReservation`,
`ManaCancellableReservation` (auxiliary display fields, not tracked in any chunk).

Also sets conditions: `LowLife`, `LowMana` (when unreserved/max <= threshold).

## Dependencies

- **PERF-02-life-mana-es**: Requires `output.Life` and `output.Mana` to be computed first
  (the max pool values that reservation subtracts from).
- **SETUP-01 through SETUP-04**: The modDB must be fully populated — items, passives, and
  config flags that affect reservation are set from parsed build data.
- **Active skill list**: The per-skill reservation accumulation (CalcPerform.lua:1810-1919)
  iterates `env.player.activeSkillList` to sum each skill's reservation. This depends on
  SETUP-02 (support gem matching & active skill construction).

## Lua Source

File: `third-party/PathOfBuilding/src/Modules/CalcPerform.lua`
Commit: `454eff8c85d24356d9b051d596983745ed367476`
Lines: 519–553 (`doActorLifeManaReservation` function) + 1810–1922 (per-skill accumulation)
Also: 3196–3201 (Foulborn Choir re-run)

## Annotated Lua

### File-top aliases used in this section

```lua
local m_min   = math.min   -- x.min(y)
local m_max   = math.max   -- x.max(y)
local m_ceil  = math.ceil  -- x.ceil()
local m_floor = math.floor -- x.floor()
local t_insert = table.insert -- vec.push()
```

Also uses PoB global functions:
- `round(val, dec)` — `m_floor(val * 10^dec + 0.5) / 10^dec` (Common.lua:635-641)
- `floor(val, dec)` — `m_floor(val * 10^dec + 0.0001) / 10^dec` (Common.lua:647-654)
  **Note:** `floor(x, 4)` is NOT `x.floor()` — it's floor-to-4-decimals with a small
  epsilon to handle floating-point edge cases. In Rust: `((x * 10000.0) + 0.0001).floor() / 10000.0`

---

### Part A — Per-Skill Reservation Accumulation (lines 1810–1919)

This runs inside CalcPerform's main flow, BEFORE `doActorLifeManaReservation` is called.
It iterates all active skills and sums their reservation into per-pool accumulators on
`env.player`.

#### Step A1 — Initialize accumulators (lines 1811–1816)

```lua
env.player.reserved_LifeBase = 0
env.player.reserved_LifePercent = modDB:Sum("BASE", nil, "ExtraLifeReserved")
-- ⚠ ExtraLifeReserved: flat extra life reservation from modDB (e.g. Petrified Blood).
-- This is BASE reservation that isn't tied to any specific skill.
-- Rust: env.player.reserved_life_percent starts at sum_base("ExtraLifeReserved")

env.player.reserved_ManaBase = 0
env.player.reserved_ManaPercent = 0
-- Mana starts at 0 (no ExtraManaReserved base that gets added to percent like life).

env.player.uncancellable_LifeReservation = modDB:Sum("BASE", nil, "ExtraLifeReserved")
env.player.uncancellable_ManaReservation = modDB:Sum("BASE", nil, "ExtraManaReserved")
-- Uncancellable reservation tracks reservations that can't be removed by deactivating skills.
-- ExtraLifeReserved / ExtraManaReserved mods are always "uncancellable" by nature.
```

> **Rust gap:** The initialization of `reserved_life_percent` from `ExtraLifeReserved`
> and `uncancellable_*_reservation` from `Extra*Reserved` does not happen in the Rust
> perform.rs code. The accumulators are initialized to 0.0 in `Actor::default()` and
> never set from modDB sums before `doActorLifeManaReservation` runs.

#### Step A2 — Iterate active skills (lines 1821–1917)

```lua
for _, activeSkill in ipairs(env.player.activeSkillList) do
    if (activeSkill.skillTypes[SkillType.HasReservation]
        or activeSkill.skillData.triggeredByAutoexertion)
       and not activeSkill.skillTypes[SkillType.ReservationBecomesCost] then
    -- ⚠ Only process skills that HAVE reservation AND haven't converted it to cost.
    -- triggeredByAutoexertion: Warcry skills that reserve via autoexertion.
    -- ReservationBecomesCost: Lifetap, Blood Magic support convert reservation → cost.
    -- Rust: check skill.skill_types.has_reservation && !skill.skill_types.reservation_becomes_cost
```

##### Step A2a — Read base reservation values (lines 1823–1836)

```lua
        local skillModList = activeSkill.skillModList
        local skillCfg = activeSkill.skillCfg
        local mult = floor(skillModList:More(skillCfg, "SupportManaMultiplier"), 4)
        -- ⚠ SupportManaMultiplier: the support gem cost/reservation multiplier.
        -- Uses floor(x, 4) = floor-to-4-decimal-places (NOT m_floor!).
        -- Rust: ((skill_mod_list.more("SupportManaMultiplier") * 10000.0 + 0.0001).floor()) / 10000.0

        local pool = { ["Mana"] = { }, ["Life"] = { } }

        pool.Mana.baseFlat = activeSkill.skillData.manaReservationFlat
            or activeSkill.activeEffect.grantedEffectLevel.manaReservationFlat or 0
        -- ⚠ Nil-coalescing chain: skillData override → grantedEffectLevel → 0
        -- Rust: skill_data.mana_reservation_flat
        --         .or(granted_effect_level.mana_reservation_flat)
        --         .unwrap_or(0.0)

        if skillModList:Flag(skillCfg, "ManaCostGainAsReservation") then
            pool.Mana.baseFlat = skillModList:Sum("BASE", skillCfg, "ManaCostBase")
                + (activeSkill.activeEffect.grantedEffectLevel.cost.Mana or 0)
        end
        -- ⚠ ManaCostGainAsReservation: converts skill's mana cost into flat reservation.
        -- Replaces the normal flat reservation with ManaCostBase + cost.Mana.

        pool.Mana.basePercent = activeSkill.skillData.manaReservationPercent
            or activeSkill.activeEffect.grantedEffectLevel.manaReservationPercent or 0
        -- Same nil-coalescing pattern for percent reservation.

        pool.Life.baseFlat = activeSkill.skillData.lifeReservationFlat
            or activeSkill.activeEffect.grantedEffectLevel.lifeReservationFlat or 0

        if skillModList:Flag(skillCfg, "LifeCostGainAsReservation") then
            pool.Life.baseFlat = skillModList:Sum("BASE", skillCfg, "LifeCostBase")
                + (activeSkill.activeEffect.grantedEffectLevel.cost.Life or 0)
        end
        -- Same pattern as ManaCostGainAsReservation but for life.

        pool.Life.basePercent = activeSkill.skillData.lifeReservationPercent
            or activeSkill.activeEffect.grantedEffectLevel.lifeReservationPercent or 0
```

##### Step A2b — Blood Magic reservation shift (lines 1837–1845)

```lua
        if skillModList:Flag(skillCfg, "BloodMagicReserved") then
            pool.Life.baseFlat = pool.Life.baseFlat + pool.Mana.baseFlat
            pool.Mana.baseFlat = 0
            activeSkill.skillData["LifeReservationFlatForced"] =
                activeSkill.skillData["ManaReservationFlatForced"]
            activeSkill.skillData["ManaReservationFlatForced"] = nil
            pool.Life.basePercent = pool.Life.basePercent + pool.Mana.basePercent
            pool.Mana.basePercent = 0
            activeSkill.skillData["LifeReservationPercentForced"] =
                activeSkill.skillData["ManaReservationPercentForced"]
            activeSkill.skillData["ManaReservationPercentForced"] = nil
        end
        -- ⚠ BloodMagicReserved: shift all mana reservation to life. Moves both flat and
        -- percent from Mana pool to Life pool. Also transfers any "Forced" overrides.
        -- This implements the Blood Magic keystone / Blood Magic Support for reservation.
        -- Rust: if flag("BloodMagicReserved") { life.base_flat += mana.base_flat; mana.base_flat = 0; ... }
```

##### Step A2c — Compute effective reservation per pool (lines 1847–1871)

```lua
        for name, values in pairs(pool) do
            values.more = skillModList:More(skillCfg, name.."Reserved", "Reserved")
            values.inc = skillModList:Sum("INC", skillCfg, name.."Reserved", "Reserved")
            -- ⚠ Multi-stat query: "ManaReserved" + "Reserved" (or "LifeReserved" + "Reserved").
            -- These combine pool-specific and generic reservation modifiers.
            -- Rust: skill_mod_list.more_multi(cfg, &[&format!("{}Reserved", name), "Reserved"])
            -- Rust: skill_mod_list.sum_multi(INC, cfg, &[&format!("{}Reserved", name), "Reserved"])

            values.efficiency = m_max(
                skillModList:Sum("INC", skillCfg,
                    name.."ReservationEfficiency", "ReservationEfficiency"),
                -100)
            -- ⚠ Reservation efficiency: clamped to >= -100 (can't go below -100% efficiency).
            -- Positive = more efficient (reserves less), negative = less efficient (reserves more).
            -- Rust: sum_multi(INC, &["ManaReservationEfficiency", "ReservationEfficiency"]).max(-100.0)

            values.efficiencyMore = skillModList:More(skillCfg,
                name.."ReservationEfficiency", "ReservationEfficiency")
            -- ⚠ Multiplicative efficiency modifier. Separate from the INC stacking.

            env.player[name.."Efficiency"] = values.efficiency
            -- Store for Arcane Cloak calculations in ModStore.GetStat.

            -- Flat reservation
            if activeSkill.skillData[name.."ReservationFlatForced"] then
                values.reservedFlat = activeSkill.skillData[name.."ReservationFlatForced"]
                -- ⚠ Forced override (e.g. from UI override or BloodMagic transfer).
            else
                local baseFlatVal = m_floor(values.baseFlat * mult)
                -- ⚠ m_floor: true floor, NOT floor(x, dec). Truncates fractional part.
                -- mult is the support multiplier (already floor'd to 4 dec).
                values.reservedFlat = 0
                if values.more > 0 and values.inc > -100 and baseFlatVal ~= 0 then
                    values.reservedFlat = m_max(round(
                        baseFlatVal * (100 + values.inc) / 100
                                    * values.more
                                    / (1 + values.efficiency / 100)
                                    / values.efficiencyMore
                    , 0), 0)
                    -- ⚠ Formula: base * (1 + inc/100) * more / (1 + eff/100) / effMore
                    -- round(x, 0) = round to nearest integer.
                    -- Clamped >= 0 by outer m_max.
                    -- Guard: more > 0 && inc > -100 && base != 0 prevents division issues
                    --   and ensures 0 base stays 0.
                    -- Rust: ((base * (1 + inc/100) * more / (1 + eff/100) / eff_more) + 0.5).floor().max(0.0)
                end
            end

            -- Percent reservation
            if activeSkill.skillData[name.."ReservationPercentForced"] then
                values.reservedPercent = activeSkill.skillData[name.."ReservationPercentForced"]
            else
                local basePercentVal = values.basePercent * mult
                -- ⚠ No floor here (unlike flat). basePercent * mult is used directly.
                values.reservedPercent = 0
                if values.more > 0 and values.inc > -100 and basePercentVal ~= 0 then
                    values.reservedPercent = m_max(round(
                        basePercentVal * (100 + values.inc) / 100
                                       * values.more
                                       / (1 + values.efficiency / 100)
                                       / values.efficiencyMore
                    , 2), 0)
                    -- ⚠ round(x, 2) = round to 2 decimal places.
                    -- Same formula as flat but with 2-decimal precision.
                    -- Rust: ((x * 100.0 + 0.5).floor()) / 100.0  — then .max(0.0)
                end
            end
```

##### Step A2d — Mine count and Blood Sacrament multipliers (lines 1872–1879)

```lua
            if activeSkill.activeMineCount then
                values.reservedFlat = values.reservedFlat * activeSkill.activeMineCount
                values.reservedPercent = values.reservedPercent * activeSkill.activeMineCount
            end
            -- ⚠ Mines: each active mine reserves separately. Multiply by mine count.

            if activeSkill.skillCfg.skillName == "Blood Sacrament"
               and activeSkill.activeStageCount then
                values.reservedFlat = values.reservedFlat * (activeSkill.activeStageCount + 1)
                values.reservedPercent = values.reservedPercent * (activeSkill.activeStageCount + 1)
            end
            -- ⚠ Blood Sacrament: reservation increases per stage channelled.
            -- Multiplied by (stageCount + 1), not just stageCount.
```

##### Step A2e — Accumulate into env.player (lines 1881–1916)

```lua
            if values.reservedFlat ~= 0 then
                activeSkill.skillData[name.."ReservedBase"] = values.reservedFlat
                env.player["reserved_"..name.."Base"] =
                    env.player["reserved_"..name.."Base"] + values.reservedFlat
                -- ⚠ Accumulate flat reservation into the actor's running total.
                -- Also store per-skill for tooltip use.
                if breakdown then
                    t_insert(breakdown[name.."Reserved"].reservations, {
                        skillName = activeSkill.activeEffect.grantedEffect.name,
                        base = values.baseFlat,
                        mult = mult ~= 1 and ("x "..mult),
                        more = values.more ~= 1 and ("x "..values.more),
                        inc = values.inc ~= 0 and ("x "..(1 + values.inc / 100)),
                        efficiency = values.efficiency ~= 0
                            and ("x " .. round(100 / (100 + values.efficiency), 4)),
                        efficiencyMore = values.efficiencyMore ~= 1
                            and ("x "..values.efficiencyMore),
                        total = values.reservedFlat,
                    })
                end
            end

            if values.reservedPercent ~= 0 then
                activeSkill.skillData[name.."ReservedPercent"] = values.reservedPercent
                activeSkill.skillData[name.."ReservedBase"] =
                    (values.reservedFlat or 0) + m_ceil(output[name] * values.reservedPercent / 100)
                -- ⚠ For percent reservation, also compute the absolute base value for tooltip.
                -- Uses m_ceil of (maxPool * percent / 100).
                -- This is stored per-skill, NOT used in the final sum (final sum uses percent).
                env.player["reserved_"..name.."Percent"] =
                    env.player["reserved_"..name.."Percent"] + values.reservedPercent
                -- ⚠ Accumulate percent reservation. Note: Lua uses string concat to
                -- build "reserved_ManaPercent" / "reserved_LifePercent" keys.
                if breakdown then
                    t_insert(breakdown[name.."Reserved"].reservations, {
                        skillName = activeSkill.activeEffect.grantedEffect.name,
                        base = values.basePercent .. "%",
                        mult = mult ~= 1 and ("x "..mult),
                        more = values.more ~= 1 and ("x "..values.more),
                        inc = values.inc ~= 0 and ("x "..(1 + values.inc / 100)),
                        efficiency = values.efficiency ~= 0
                            and ("x " .. round(100 / (100 + values.efficiency), 4)),
                        efficiencyMore = values.efficiencyMore ~= 1
                            and ("x "..values.efficiencyMore),
                        total = values.reservedPercent .. "%",
                    })
                end
            end

            if skillModList:Flag(skillCfg, "HasUncancellableReservation") then
                env.player["uncancellable_"..name.."Reservation"] =
                    env.player["uncancellable_"..name.."Reservation"]
                    + values.reservedPercent
            end
            -- ⚠ HasUncancellableReservation: flag on specific skills that can't be deactivated.
            -- Only adds the percent component to uncancellable tracking.
        end  -- for name, values in pairs(pool)
    end  -- if HasReservation
end  -- for activeSkill
```

#### Step A3 — Call doActorLifeManaReservation (line 1922)

```lua
doActorLifeManaReservation(env.player,
    not modDB:Flag(nil, "ManaIncreasedByOvercappedLightningRes"))
-- ⚠ The addAura parameter: normally true (process GrantReservedXAsAura).
-- When ManaIncreasedByOvercappedLightningRes is set (Foulborn Choir of the Storm),
-- hold off on the aura pass — it happens later at line 3201 after resistances
-- are calculated and mana is recalculated from overcapped lightning resistance.
```

---

### Part B — `doActorLifeManaReservation` function (lines 519–553)

This is the function that reads the accumulated totals and writes output fields.

```lua
function doActorLifeManaReservation(actor, addAura)
    local modDB = actor.modDB
    local output = actor.output
    local condList = modDB.conditions

    for _, pool in pairs({"Life", "Mana"}) do
        local max = output[pool]
        -- ⚠ output["Life"] or output["Mana"] — the max pool from PERF-02.
        -- Rust: get_output_f64(output, "Life") / get_output_f64(output, "Mana")

        local reserved
        if max > 0 then
            local lowPerc = modDB:Sum("BASE", nil, "Low" .. pool .. "Percentage")
            -- ⚠ "LowLifePercentage" / "LowManaPercentage" — custom threshold for Low condition.
            -- Default threshold is data.misc.LowPoolThreshold (0.5 = 50%).

            reserved = (actor["reserved_"..pool.."Base"] or 0)
                     + m_ceil(max * (actor["reserved_"..pool.."Percent"] or 0) / 100)
            -- ⚠ Total reserved = flat_base + ceil(max * percent / 100).
            -- Uses m_ceil for the percent portion (rounds UP).
            -- The `or 0` guards handle nil if no skills reserved anything.
            -- Rust: let reserved = actor.reserved_life
            --         + (life * actor.reserved_life_percent / 100.0).ceil();

            uncancellableReservation =
                actor["uncancellable_"..pool.."Reservation"] or 0
            -- ⚠ NOTE: this is a GLOBAL variable (no `local` keyword!).
            -- This is a Lua bug/quirk — the variable leaks to module scope.
            -- Functionally it still works because Life is processed before Mana
            -- in the pairs() iteration order (pairs is unordered in general,
            -- but {"Life", "Mana"} is a sequence so ipairs order applies — however
            -- the code uses pairs(), so technically the order is implementation-defined.
            -- In practice LuaJIT iterates sequences in order).

            output[pool.."Reserved"] = m_min(reserved, max)
            -- ⚠ OUTPUT: LifeReserved / ManaReserved — capped at pool max for display.
            -- Rust: output.insert("LifeReserved", reserved.min(life))

            output[pool.."ReservedPercent"] = m_min(reserved / max * 100, 100)
            -- ⚠ OUTPUT: LifeReservedPercent / ManaReservedPercent — capped at 100%.
            -- Rust: output.insert("LifeReservedPercent", (reserved / life * 100.0).min(100.0))

            output[pool.."Unreserved"] = max - reserved
            -- ⚠ Can be NEGATIVE if over-reserved. Lua does NOT clamp to >= 0.
            -- Rust: output.insert("LifeUnreserved", life - reserved)  -- do NOT clamp

            output[pool.."UnreservedPercent"] = (max - reserved) / max * 100
            -- ⚠ Can also be negative. No clamping.

            output[pool.."UncancellableReservation"] = m_min(uncancellableReservation, 0)
            -- ⚠ Clamped to <= 0. This is a PoB display quirk:
            -- uncancellable reservation is normally positive but this field
            -- stores min(val, 0) which will always be 0 unless there's negative
            -- uncancellable reservation (which shouldn't happen).
            -- Rust: output.insert("LifeUncancellableReservation", uncancellable.min(0.0))

            output[pool.."CancellableReservation"] = 100 - uncancellableReservation
            -- ⚠ What percent of reservation can be cancelled by deactivating skills.
            -- 100 - uncancellable gives the "flexible" portion.

            if (max - reserved) / max
                   <= (lowPerc > 0 and lowPerc or data.misc.LowPoolThreshold) then
                condList["Low"..pool] = true
            end
            -- ⚠ LowLife / LowMana condition.
            -- Threshold: custom LowLifePercentage if > 0, else default 0.5 (50%).
            -- The comparison is (unreserved / max) <= threshold.
            -- Rust: if (unreserved / max) <= threshold { set_condition("LowLife", true); }
            -- NOTE: data.misc.LowPoolThreshold = 0.5 — this is a raw fraction (50%),
            -- NOT already multiplied by 100.
        else
            reserved = 0
        end

        if addAura then
            for _, value in ipairs(modDB:List(nil, "GrantReserved"..pool.."AsAura")) do
                local auraMod = copyTable(value.mod)
                auraMod.value = m_floor(auraMod.value * m_min(reserved, max))
                modDB:NewMod("ExtraAura", "LIST", { mod = auraMod })
            end
        end
        -- ⚠ GrantReservedLifeAsAura / GrantReservedManaAsAura:
        -- For each LIST mod, scale its embedded value by floor(value * min(reserved, max)).
        -- Creates a new ExtraAura LIST mod. Used by Radiant Faith (Guardian ascendancy).
        -- Rust: iterate list mods, clone embedded mod, scale value, add as ExtraAura.
    end
end
```

---

### Part C — Foulborn Choir Re-Run (lines 3196–3201)

```lua
if modDB:Flag(nil, "ManaIncreasedByOvercappedLightningRes") then
    -- Calculate resistances for ManaIncreasedByOvercappedLightningRes
    calcs.resistances(env.player)
    -- Set the life/mana reservations again as we now have increased mana
    -- from overcapped lightning resistance
    doActorLifeMana(env.player)
    doActorLifeManaReservation(env.player, true)
end
-- ⚠ Foulborn Choir of the Storm: mana is increased by overcapped lightning resistance.
-- This creates a circular dependency: reservation affects unreserved mana → affects
-- resistance overcap → affects mana → affects reservation.
-- The re-run breaks the cycle: first call had addAura=false, now re-run with addAura=true
-- after resistances and mana are recalculated.
-- Rust: the current code checks this flag for the addAura parameter in the first call
-- but does NOT perform the second call after resistance calculation.
```

---

## Existing Rust Code

File: `crates/pob-calc/src/calc/perform.rs`, lines 548–734 (`do_actor_life_mana_reservation`)
Also: `crates/pob-calc/src/calc/env.rs`, lines 109–163 (Actor fields for reservation accumulators)

### What Exists

**`doActorLifeManaReservation` (Part B) — the output-writing function:**
- Life/Mana pool read from output: ✓
- `LowLifePercentage` / `LowManaPercentage` custom threshold: ✓
- Reserved = flat + ceil(max * percent / 100): ✓
- `LifeReserved` / `ManaReserved` capped at max: ✓
- `LifeReservedPercent` / `ManaReservedPercent` capped at 100: ✓
- `LifeUnreserved` / `ManaUnreserved` (negative allowed): ✓
- `LifeUnreservedPercent` / `ManaUnreservedPercent`: ✓
- `UncancellableReservation` / `CancellableReservation`: ✓
- `LowLife` / `LowMana` condition with threshold: ✓
- `GrantReservedLifeAsAura` / `GrantReservedManaAsAura` with `addAura` parameter: ✓

**Actor accumulator fields:**
- `reserved_life`, `reserved_life_percent`, `reserved_mana`, `reserved_mana_percent`: ✓ (defined in env.rs, initialized to 0.0)
- `uncancellable_life_reservation`, `uncancellable_mana_reservation`: ✓ (defined, initialized to 0.0)

### What Is Missing

1. **Per-skill reservation accumulation (Part A, lines 1810–1919) — ENTIRELY MISSING:**
   The Rust code has `do_actor_life_mana_reservation` (Part B) but lacks the loop over
   `activeSkillList` that computes per-skill reservation and accumulates the totals into
   `reserved_life`, `reserved_life_percent`, `reserved_mana`, `reserved_mana_percent`.
   Without this, all reservation accumulators stay at 0.0, meaning:
   - `ManaReserved` = 0 for all builds (should be hundreds or thousands for builds with auras)
   - `LifeReserved` = 0 for all builds using Petrified Blood or blood auras
   - All reservation-dependent fields are wrong

2. **Initialization of `reserved_life_percent` from `ExtraLifeReserved`:**
   Lua sets `reserved_LifePercent = modDB:Sum("BASE", nil, "ExtraLifeReserved")` before
   the skill loop (line 1812). Rust initializes to 0.0 and never queries this mod.

3. **Initialization of `uncancellable_*_reservation` from `Extra*Reserved`:**
   Lua sets these from modDB sums (lines 1815-1816). Rust initializes to 0.0.

4. **Foulborn Choir re-run (Part C, lines 3196-3201):**
   The Rust code correctly gates `addAura` based on `ManaIncreasedByOvercappedLightningRes`
   but does NOT perform the second pass (`doActorLifeMana` + `doActorLifeManaReservation`
   with `addAura=true`) after resistance calculation.

5. **`BloodMagicReserved` flag handling:**
   The mana→life reservation shift for Blood Magic is not implemented.

6. **`ManaCostGainAsReservation` / `LifeCostGainAsReservation` flags:**
   Skills that convert their cost into reservation are not handled.

7. **Reservation efficiency (`ReservationEfficiency`, `ManaReservationEfficiency`, etc.):**
   The efficiency modifiers that reduce or increase reservation cost are not applied.

8. **Mine count and Blood Sacrament stage multipliers:**
   `activeMineCount` and Blood Sacrament `activeStageCount` scaling not applied.

9. **`HasUncancellableReservation` flag accumulation:**
   Per-skill uncancellable reservation tracking not implemented.

10. **Breakdown population for reservation:**
    The Lua populates `breakdown.LifeReserved.reservations` and
    `breakdown.ManaReserved.reservations` with per-skill breakdown entries. Not implemented
    in Rust.

### What Is Wrong

1. **`addAura` logic is inverted for the two-pass case:**
   The Rust code computes `add_aura = !flag("ManaIncreasedByOvercappedLightningRes")` and
   runs the aura logic in a single pass. This correctly skips auras in the first pass when
   the flag is set, but never does the second pass. The net effect: builds with Foulborn
   Choir never get GrantReservedXAsAura processed.

2. **`LowLifePercentage` threshold units:**
   Lua's `data.misc.LowPoolThreshold = 0.5` is a fraction (0.5 = 50%). The Lua comparison
   is `(max - reserved) / max <= threshold`, where both sides are fractions. The Rust code
   correctly uses 0.5 as the default threshold and computes `unreserved / life`, so the
   units are consistent. **This is correct.**

3. **Phantom fields in `field_groups.rs`:**
   `ManaReservedP` and `LifeReservedP` are listed in the PERF-04-reservation field group
   but don't correspond to any Lua output field or oracle expected value. These should be
   removed from the field group to avoid false test failures.

---

## What Needs to Change

1. **Implement per-skill reservation accumulation (Part A):**
   Before calling `do_actor_life_mana_reservation`, iterate `env.player.active_skill_list`
   and for each skill with `HasReservation` (and not `ReservationBecomesCost`):
   - Read `mana_reservation_flat`, `mana_reservation_percent`, `life_reservation_flat`,
     `life_reservation_percent` from skill data / granted effect level
   - Handle `BloodMagicReserved` flag (mana→life shift)
   - Compute `more`, `inc`, `efficiency`, `efficiency_more` from skill mod list
   - Apply `SupportManaMultiplier` via `floor(x, 4)` semantics
   - Compute `reserved_flat` and `reserved_percent` using the reservation formula
   - Apply mine count and Blood Sacrament stage multipliers
   - Accumulate into `env.player.reserved_life`, `reserved_life_percent`,
     `reserved_mana`, `reserved_mana_percent`
   - Track `uncancellable_*_reservation` for skills with `HasUncancellableReservation`

2. **Initialize reservation accumulators from modDB:**
   Before the skill loop:
   ```rust
   env.player.reserved_life_percent = mod_db.sum(Base, "ExtraLifeReserved");
   env.player.reserved_life = 0.0;
   env.player.reserved_mana = 0.0;
   env.player.reserved_mana_percent = 0.0;
   env.player.uncancellable_life_reservation = mod_db.sum(Base, "ExtraLifeReserved");
   env.player.uncancellable_mana_reservation = mod_db.sum(Base, "ExtraManaReserved");
   ```

3. **Implement Foulborn Choir re-run:**
   After resistance calculation (or wherever the Rust equivalent runs), if
   `ManaIncreasedByOvercappedLightningRes` flag is set:
   ```rust
   do_actor_life_mana(env);  // recalculate pools with new mana
   do_actor_life_mana_reservation(env);  // with addAura = true
   ```

4. **Implement `ManaCostGainAsReservation` / `LifeCostGainAsReservation`:**
   When flagged, replace base flat reservation with `ManaCostBase + cost.Mana`.

5. **Implement reservation efficiency:**
   Apply the `(1 + efficiency / 100) * efficiency_more` denominator to the
   reservation formula for both flat and percent components.

6. **Remove phantom fields from `field_groups.rs`:**
   Remove `ManaReservedP` and `LifeReservedP` from the `PERF-04-reservation` entry.

7. **Add breakdown population:**
   Populate `breakdown.LifeReserved` and `breakdown.ManaReserved` with per-skill
   reservation entries matching the Lua structure.

8. **Handle `floor(x, 4)` for SupportManaMultiplier:**
   Implement the PoB-specific `floor(val, dec)` function:
   ```rust
   fn pob_floor(val: f64, dec: u32) -> f64 {
       let mult = 10f64.powi(dec as i32);
       (val * mult + 0.0001).floor() / mult
   }
   ```
   This is distinct from `f64::floor()` — it adds a small epsilon before flooring
   to handle floating-point edge cases (e.g. `0.9999999999` → `1.0` after epsilon).
