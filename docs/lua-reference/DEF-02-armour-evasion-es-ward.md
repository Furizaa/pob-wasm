# DEF-02: Armour, Evasion, Energy Shield, Ward

## Output Fields

Fields this chunk must write (from `field_groups.rs`):

| Field | Oracle present | Source function | Notes |
|-------|---------------|----------------|-------|
| `Armour` | 30/30 | `calcs.defence`, line 1045 | Rounded, clamped ≥ 0 |
| `ArmourDefense` | 30/30 | `calcs.defence`, line 1559 | Fraction (0.0–1.0), from `modDB:Max("ArmourDefense")/100` |
| `Evasion` | 30/30 | `calcs.defence`, line 1046 | Rounded, clamped ≥ 0 |
| `EvasionDefense` | 0/30 | **nowhere** | **Phantom field** — PoB never writes this. Remove from `field_groups.rs`. |
| `EnergyShieldOnBody Armour` | 13/30 | `calcs.defenceForConditionals`, line 494 | Raw base ES from item, conditional write |
| `ArmourOnBody Armour` | 9/30 | `calcs.defenceForConditionals`, line 498 | Raw base armour from item, conditional write |
| `ArmourOnHelmet` | 11/30 | `calcs.defenceForConditionals`, line 498 | |
| `ArmourOnGloves` | 13/30 | `calcs.defenceForConditionals`, line 498 | |
| `ArmourOnBoots` | 12/30 | `calcs.defenceForConditionals`, line 498 | |
| `ArmourOnWeapon 1` | 0/30 | **nowhere** | **Phantom field** — `"Weapon 1"` is not in the slot loop. Remove from `field_groups.rs`. |
| `ArmourOnWeapon 2` | 5/30 | `calcs.defenceForConditionals`, line 498 | Armour from shield/offhand |
| `EvasionOnBody Armour` | 8/30 | `calcs.defenceForConditionals`, line 502 | |
| `EvasionOnHelmet` | 8/30 | `calcs.defenceForConditionals`, line 502 | |
| `EvasionOnGloves` | 6/30 | `calcs.defenceForConditionals`, line 502 | |
| `EvasionOnBoots` | 7/30 | `calcs.defenceForConditionals`, line 502 | |
| `EnergyShieldOnHelmet` | 13/30 | `calcs.defenceForConditionals`, line 494 | |
| `EnergyShieldOnGloves` | 17/30 | `calcs.defenceForConditionals`, line 494 | |
| `EnergyShieldOnBoots` | 14/30 | `calcs.defenceForConditionals`, line 494 | |

> **Phantom fields:** `EvasionDefense` and `ArmourOnWeapon 1` appear in
> `field_groups.rs` but are never written by PoB's calc engine (0/30 oracle files).
> Remove them.

> **Also written by this chunk (not in DEF-02 `field_groups.rs`, but produced here):**
> `EnergyShield`, `Ward`, `Gear:Armour`, `Gear:Evasion`, `Gear:EnergyShield`,
> `Gear:Ward`, `LowestOfArmourAndEvasion`, `MeleeEvasion`, `ProjectileEvasion`.
> These are not in DEF-02's field list but must be written correctly as other chunks
> depend on them.

## Dependencies

- `DEF-01-resistances` — `ArmourIncreasedByUncappedFireRes` / `ArmourIncreasedByOvercappedFireRes` inject `Armour INC` using `output.FireResistTotal` / `output.FireResistOverCap`, which must be computed before the primary defence loop runs
- `DEF-03-block-suppression` — `EnergyShieldIncreasedByChanceToBlockSpellDamage` uses `output.SpellBlockChance` to inject `EnergyShield INC`, which must exist when primary defences are computed
- `PERF-02-life-mana-es` — `Mana` base value is consumed by `ManaConvertToArmour` / `ManaGainAsEnergyShield` conversion paths
- `SETUP-03-items` — item `armourData` (per-slot `Armour`, `Evasion`, `EnergyShield`, `Ward` base values) must be loaded before the slot loop runs

## Lua Source

**`calcs.defenceForConditionals`:** `CalcDefence.lua`, lines 480–506  
**`calcs.defence` — primary defences block:** `CalcDefence.lua`, lines 824–1115  
**`calcs.defence` — `ArmourDefense` write:** `CalcDefence.lua`, lines 1559–1560  

Commit: `454eff8c85d24356d9b051d596983745ed367476` (third-party/PathOfBuilding, heads/dev)

## Annotated Lua

### Section 1: `calcs.defenceForConditionals` (lines 480–506)

This function runs **before** buffs/charges (called from `CalcPerform.lua:3271`), while
the primary defence block in `calcs.defence` runs later. The purpose is to write raw
**item base values** per slot so that conditional modifiers that ask "do I have armour
on helmet?" can evaluate correctly.

```lua
function calcs.defenceForConditionals(env, actor)
    local modDB = actor.modDB
    local output = actor.output

    -- Slot loop: exactly {"Helmet","Gloves","Boots","Body Armour","Weapon 2","Weapon 3"}
    -- NOTE: "Weapon 1" is intentionally absent — main-hand weapons don't have armour data.
    for _, slot in pairs({"Helmet","Gloves","Boots","Body Armour","Weapon 2","Weapon 3"}) do
        -- pairs() on a Lua table has unspecified order (not sequential like ipairs)
        -- Rust: iterate over a fixed slice in any order
        local armourData = actor.itemList[slot] and actor.itemList[slot].armourData
        -- actor.itemList[slot] = the item in that slot (nil if empty)
        -- .armourData = the armour-specific stats table (nil if item has no armour stats)
        -- The `and` short-circuit: if no item, armourData = nil (don't crash on .armourData)
        if armourData then
            -- Ward: written only when > 0 AND not blocked by GainNoWardFrom{Slot}
            local wardBase = not modDB:Flag(nil, "GainNoWardFrom" .. slot) and armourData.Ward or 0
            -- Lua `not FLAG and value or 0` pattern:
            --   if GainNoWardFromHelmet flag is set → not true → false → 0
            --   else → not false → true → armourData.Ward
            --   Note: if armourData.Ward is nil, `true and nil` = nil, `nil or 0` = 0
            if wardBase > 0 then
                output["WardOn"..slot] = wardBase          -- e.g. output["WardOnHelmet"] = 42
            end

            local energyShieldBase = not modDB:Flag(nil, "GainNoEnergyShieldFrom" .. slot) and armourData.EnergyShield or 0
            if energyShieldBase > 0 then
                output["EnergyShieldOn"..slot] = energyShieldBase
                -- e.g. output["EnergyShieldOnHelmet"] = 350
                -- Only written when > 0, so absent from output when slot has no ES
            end

            local armourBase = not modDB:Flag(nil, "GainNoArmourFrom" .. slot) and armourData.Armour or 0
            if armourBase > 0 then
                output["ArmourOn"..slot] = armourBase
                -- e.g. output["ArmourOnBody Armour"] = 938
                -- NOTE: slot name includes space: "Body Armour", "Weapon 2"
            end

            local evasionBase = not modDB:Flag(nil, "GainNoEvasionFrom" .. slot) and armourData.Evasion or 0
            if evasionBase > 0 then
                output["EvasionOn"..slot] = evasionBase
            end
        end
    end
end
```

**Gotcha — conditional write (only when > 0):** The `*On{Slot}` fields are only written
when the base value is positive. A slot with zero armour has no `ArmourOn{Slot}` key.
In Rust, `set_output` must be called conditionally: `if base > 0.0 { player.set_output(...) }`.
The field simply won't exist in the output map when the value is zero, which is why
oracle builds with pure-ES body armour have `EnergyShieldOnBody Armour` but not
`ArmourOnBody Armour`.

**Gotcha — `GainNo*From{Slot}` flags:** These flags disable gaining a defence type from
a specific slot. For example, `GainNoEnergyShieldFromBody Armour` forces the body armour
ES to zero regardless of the item. The flag name includes the slot name with spaces, e.g.
`"GainNoArmourFromBody Armour"`. In Rust, query `flag_cfg("GainNoArmourFrom{slot}", None, output)`.

**Gotcha — `pairs()` loop order:** Lua's `pairs()` on a table (array-as-table) has
unspecified iteration order. In practice the slots are small and independent, so order
doesn't matter. In Rust, use any fixed iteration order.

### Section 2: Primary defences computation (lines 824–1115)

The main computation block runs inside `calcs.defence`. It accumulates totals for all
four defence types across all item slots plus global mods, then writes the final outputs.

#### 2a. Pre-computation modDB injections (lines 772–823)

Before the main loop, several conditional blocks inject new INC mods based on
already-computed output values. These are the **order-sensitive** dependencies:

```lua
-- Armour to ES Recharge conversion (rare mastery):
if modDB:Flag(nil, "ArmourAppliesToEnergyShieldRecharge") then
    -- Copies Armour INC mods as EnergyShieldRecharge INC mods
    for _, value in ipairs(modDB:Tabulate("INC", nil, "Armour", "ArmourAndEvasion", "Defences")) do
        local mod = value.mod
        local multiplier = (modDB:Max(nil, "ImprovedArmourAppliesToEnergyShieldRecharge") or 100) / 100
        modDB:NewMod("EnergyShieldRecharge", "INC", m_floor(mod.value * multiplier), ...)
    end
end

-- Armour increased by fire resist (Kirmes Hammer):
if modDB:Flag(nil, "ArmourIncreasedByUncappedFireRes") then
    modDB:NewMod("Armour", "INC", output.FireResistTotal, ...)  -- uses already-computed FireResistTotal
end
if modDB:Flag(nil, "ArmourIncreasedByOvercappedFireRes") then
    modDB:NewMod("Armour", "INC", output.FireResistOverCap, ...)
end

-- Evasion increased by cold resist:
if modDB:Flag(nil, "EvasionRatingIncreasedByUncappedColdRes") then
    modDB:NewMod("Evasion", "INC", output.ColdResistTotal, ...)
end
if modDB:Flag(nil, "EvasionRatingIncreasedByOvercappedColdRes") then
    modDB:NewMod("Evasion", "INC", output.ColdResistOverCap, ...)
end

-- ES increased by spell block chance (The Surrender shield):
if modDB:Flag(nil, "EnergyShieldIncreasedByChanceToBlockSpellDamage") then
    modDB:NewMod("EnergyShield", "INC", output.SpellBlockChance, ...)  -- uses block computation
end

-- ES increased by chaos resistance (Skin of the Loyal-style):
if modDB:Flag(nil, "EnergyShieldIncreasedByChaosResistance") then
    modDB:NewMod("EnergyShield", "INC", output.ChaosResist, ...)
end
```

These `modDB:NewMod` calls mutate the modDB before the primary defence calculation runs,
so the INC values are available when Armour/Evasion/ES are computed. In Rust, these
injections must happen **before** the summing of INC mods for each defence type.

#### 2b. Slot accumulation loop (lines 843–923)

```lua
local ironReflexes = modDB:Flag(nil, "IronReflexes")
-- IronReflexes: all Evasion converts to Armour (gear-slot Evasion → Armour INC×More)
local ward = 0
local energyShield = 0
local armour = 0
local evasion = 0
local gearWard = 0
local gearEnergyShield = 0
local gearArmour = 0
local gearEvasion = 0
local slotCfg = wipeTable(tempTable1)  -- reusable config table, wiped per-slot

for _, slot in pairs({"Helmet","Gloves","Boots","Body Armour","Weapon 2","Weapon 3"}) do
    local armourData = actor.itemList[slot] and actor.itemList[slot].armourData
    if armourData then
        slotCfg.slotName = slot
        -- slotCfg is used in calcLib.mod calls to filter slot-specific mods

        -- GainNo* flags suppress contribution:
        energyShieldBase = not modDB:Flag(nil, "GainNoEnergyShieldFrom" .. slot) and armourData.EnergyShield or 0
        armourBase = not modDB:Flag(nil, "GainNoArmourFrom" .. slot) and armourData.Armour or 0
        evasionBase = not modDB:Flag(nil, "GainNoEvasionFrom" .. slot) and armourData.Evasion or 0
        wardBase = not modDB:Flag(nil, "GainNoWardFrom" .. slot) and armourData.Ward or 0

        -- Body Armour → Ward conversion (rare keystone):
        if slot == "Body Armour" and modDB:Flag(nil, "ConvertBodyArmourArmourEvasionToWard") then
            local conversion = m_min(modDB:Sum("BASE", nil, "BodyArmourArmourEvasionToWardPercent") / 100, 1)
            -- cap conversion at 100%
            local convertedArmour = armourBase * conversion
            local convertedEvasion = evasionBase * conversion
            armourBase = armourBase - convertedArmour
            evasionBase = evasionBase - convertedEvasion
            wardBase = wardBase + (convertedEvasion + convertedArmour)
            -- Shifts item armour/evasion into ward pool
        end

        -- WARD accumulation (slot):
        if wardBase > 0 then
            if modDB:Flag(nil, "EnergyShieldToWard") then
                -- Replica Dreamfeather: ES items contribute to Ward with Ward INC
                local inc = modDB:Sum("INC", slotCfg, "Ward", "Defences", "EnergyShield")
                local more = modDB:More(slotCfg, "Ward", "Defences")
                ward = ward + wardBase * (1 + inc / 100) * more
            else
                ward = ward + wardBase * calcLib.mod(modDB, slotCfg, "Ward", "Defences")
                -- calcLib.mod = (1 + Sum("INC",...)/100) * More(...)
                -- Queries both "Ward" AND "Defences" for INC/More
            end
            gearWard = gearWard + wardBase
        end

        -- ENERGY SHIELD accumulation (slot):
        if energyShieldBase > 0 then
            if modDB:Flag(nil, "EnergyShieldToWard") then
                -- EnergyShieldToWard: ES from items becomes Ward with only More factor
                local more = modDB:More(slotCfg, "EnergyShield", "Defences")
                energyShield = energyShield + energyShieldBase * more
                -- NOTE: only More here, no INC — intentional for this keystone
            elseif not modDB:Flag(nil, "ConvertArmourESToLife") then
                -- ConvertArmourESToLife (Petrified Blood-related): skip ES entirely
                energyShield = energyShield + energyShieldBase * calcLib.mod(modDB, slotCfg, "EnergyShield", "Defences", slot.."ESAndArmour")
                -- "EnergyShield", "Defences", and e.g. "Body ArmourESAndArmour" are all
                -- queried for INC/More simultaneously
            end
            gearEnergyShield = gearEnergyShield + energyShieldBase
        end

        -- ARMOUR accumulation (slot):
        if armourBase > 0 then
            armour = armour + armourBase * calcLib.mod(modDB, slotCfg, "Armour", "ArmourAndEvasion", "Defences", slot.."ESAndArmour")
            -- INC/More queried from: "Armour", "ArmourAndEvasion", "Defences", "{Slot}ESAndArmour"
            -- e.g. for Helmet: "Armour", "ArmourAndEvasion", "Defences", "HelmetESAndArmour"
            gearArmour = gearArmour + armourBase
        end

        -- EVASION accumulation (slot):
        if evasionBase > 0 then
            gearEvasion = gearEvasion + evasionBase
            if ironReflexes then
                -- Iron Reflexes: evasion → armour; uses Armour + Evasion + ArmourAndEvasion + Defences INC
                armour = armour + evasionBase * calcLib.mod(modDB, slotCfg, "Armour", "Evasion", "ArmourAndEvasion", "Defences")
            else
                evasion = evasion + evasionBase * calcLib.mod(modDB, slotCfg, "Evasion", "ArmourAndEvasion", "Defences")
                -- INC/More from: "Evasion", "ArmourAndEvasion", "Defences"
            end
        end
    end
end
```

**Gotcha — `calcLib.mod` with multiple stat names:** `calcLib.mod(modDB, cfg, "Armour", "ArmourAndEvasion", "Defences", slot.."ESAndArmour")` passes **four stat names** as varargs. The function sums `INC` from ALL named stats and takes `More` from ALL named stats combined. This means:
- `% increased Armour` → applies (stat: Armour)
- `% increased Armour and Evasion` → applies (stat: ArmourAndEvasion)
- `% increased Defences` → applies (stat: Defences)
- `% increased Helmet Armour and Energy Shield` → applies (stat: HelmetESAndArmour)

In Rust, `mod_db.sum_cfg(Inc, "Armour", ...)` only queries one stat name at a time.
The Rust implementation would need to sum INC from all relevant stats and similarly
combine More. **This is the primary correctness gap** — the Rust currently queries
only `"Armour"` for INC and `more_cfg("Armour")` for More, missing `ArmourAndEvasion`,
`Defences`, and slot-specific combined stats.

**Gotcha — `slotCfg.slotName`:** The `slotCfg` table passed to `calcLib.mod` has a
`slotName` field set to the current slot. This is used by some mods that specify
`{ type = "Slot", slotName = "Helmet" }` tags, meaning "only apply to this slot".
In Rust, this maps to the `slot_name` field on `SkillCfg`.

#### 2c. Global (non-slot) base accumulation (lines 925–1043)

After the slot loop, global BASE mods and stat-conversion paths are accumulated:

```lua
-- Global Ward BASE mods (e.g. from passive tree or flasks):
wardBase = modDB:Sum("BASE", nil, "Ward")
if wardBase > 0 then
    ward = ward + wardBase * calcLib.mod(modDB, nil, "Ward", "Defences")
end

-- Global ES BASE mods:
energyShieldBase = modDB:Sum("BASE", nil, "EnergyShield")
if energyShieldBase > 0 then
    if modDB:Flag(nil, "EnergyShieldToWard") then
        energyShield = energyShield + energyShieldBase * modDB:More(nil, "EnergyShield", "Defences")
    else
        energyShield = energyShield + energyShieldBase * calcLib.mod(modDB, nil, "EnergyShield", "Defences")
    end
end

-- Global Armour BASE mods (e.g. buffs, passive flats):
armourBase = modDB:Sum("BASE", nil, "Armour", "ArmourAndEvasion")
-- NOTE: "ArmourAndEvasion" is also queried for BASE here
if armourBase > 0 then
    armour = armour + armourBase * calcLib.mod(modDB, nil, "Armour", "ArmourAndEvasion", "Defences")
end

-- Global Evasion BASE mods:
evasionBase = modDB:Sum("BASE", nil, "Evasion", "ArmourAndEvasion")
if evasionBase > 0 then
    if ironReflexes then
        armour = armour + evasionBase * calcLib.mod(modDB, nil, "Armour", "Evasion", "ArmourAndEvasion", "Defences")
    else
        evasion = evasion + evasionBase * calcLib.mod(modDB, nil, "Evasion", "ArmourAndEvasion", "Defences")
    end
end

-- Mana → Armour conversion (Brass Dome / Titanscale):
local convManaToArmour = modDB:Sum("BASE", nil, "ManaConvertToArmour")
if convManaToArmour > 0 then
    armourBase = 2 * modDB:Sum("BASE", nil, "Mana") * convManaToArmour / 100
    -- Factor of 2: "for every 2 mana, gain 1 armour" → base = 2 * Mana * percent / 100
    armour = armour + armourBase * calcLib.mod(modDB, nil, "Mana", "Armour", "ArmourAndEvasion", "Defences")
    -- Mana INC mods also apply to the converted armour!
end

-- Mana → ES conversion:
local convManaToES = modDB:Sum("BASE", nil, "ManaGainAsEnergyShield")
if convManaToES > 0 then
    energyShieldBase = modDB:Sum("BASE", nil, "Mana") * convManaToES / 100
    energyShield = energyShield + energyShieldBase * calcLib.mod(modDB, nil, "Mana", "EnergyShield", "Defences")
end

-- Life → Armour conversion:
local convLifeToArmour = modDB:Sum("BASE", nil, "LifeGainAsArmour")
if convLifeToArmour > 0 then
    armourBase = modDB:Sum("BASE", nil, "Life") * convLifeToArmour / 100
    local total
    if modDB:Flag(nil, "ChaosInoculation") then
        total = 1   -- CI: life is 1, so conversion yields effectively 0 (base=0*pct/100=0)
        -- Actually: CI forces life = 1. armourBase = 1 * convLifeToArmour / 100 ≈ 0
        -- The `total = 1` seems to be a special case to avoid 0 contribution when CI is on.
        -- NOTE: this is potentially a PoB quirk / edge-case worth investigating
    else
        total = armourBase * calcLib.mod(modDB, nil, "Life", "Armour", "ArmourAndEvasion", "Defences")
    end
    armour = armour + total
end

-- Life → ES conversion (Eldrich Battery / keystone):
local convLifeToES = modDB:Sum("BASE", nil, "LifeConvertToEnergyShield", "LifeGainAsEnergyShield")
-- Two stat names queried for BASE simultaneously
if convLifeToES > 0 then
    energyShieldBase = modDB:Sum("BASE", nil, "Life") * convLifeToES / 100
    local total
    if modDB:Flag(nil, "ChaosInoculation") then
        total = 1
    else
        total = energyShieldBase * calcLib.mod(modDB, nil, "Life", "EnergyShield", "Defences")
    end
    energyShield = energyShield + total
end

-- Evasion → Armour conversion (Iron Reflexes via non-slot global evasion):
local convEvasionToArmour = modDB:Sum("BASE", nil, "EvasionGainAsArmour")
if convEvasionToArmour > 0 then
    armourBase = (modDB:Sum("BASE", nil, "Evasion", "ArmourAndEvasion") + gearEvasion) * convEvasionToArmour / 100
    -- NOTE: includes gearEvasion (accumulated slot-level evasion base values)
    local total = armourBase * calcLib.mod(modDB, nil, "Evasion", "Armour", "ArmourAndEvasion", "Defences")
    armour = armour + total
end
```

#### 2d. Output writes (lines 1044–1061)

```lua
-- round() = standard rounding (not floor/trunc) — see LUA-GOTCHAS.md
output.EnergyShield = modDB:Override(nil, "EnergyShield") or m_max(round(energyShield), 0)
-- Override takes precedence for EnergyShield (e.g., CI forces to 1)
output.Armour = m_max(round(armour), 0)
output.Evasion = m_max(round(evasion), 0)
-- NOTE: No Override check for Armour or Evasion

-- Melee and Projectile evasion variations (separate multiplier):
output.MeleeEvasion = m_max(round(evasion * calcLib.mod(modDB, nil, "MeleeEvasion")), 0)
output.ProjectileEvasion = m_max(round(evasion * calcLib.mod(modDB, nil, "ProjectileEvasion")), 0)
-- calcLib.mod("MeleeEvasion") queries INC and More for "MeleeEvasion" stat
-- Default is 1.0 (no mods) → MeleeEvasion == Evasion by default

output.LowestOfArmourAndEvasion = m_min(output.Armour, output.Evasion)

-- Ward: floor, not round!
output.Ward = m_max(m_floor(ward), 0)
-- Rust: ward.floor().max(0.0) — note different rounding than Armour/Evasion/ES

-- Gear totals (raw sum of item base values, before INC/More):
output["Gear:Ward"] = gearWard
output["Gear:EnergyShield"] = gearEnergyShield
output["Gear:Armour"] = gearArmour
output["Gear:Evasion"] = gearEvasion
```

**Gotcha — `round()` vs `m_floor()`:** Armour, Evasion, and EnergyShield use `round(x)`
(rounds to nearest integer: 0.5 rounds up). Ward uses `m_floor(ward)` (rounds down). This
is intentional: Ward is always floored, the others are rounded. Rust: use `.round() as i64` or
`(x + 0.5).floor()` for round, and `.floor()` for ward.

**Gotcha — ES Override check:** `modDB:Override(nil, "EnergyShield")` allows an
override to set a specific ES value (e.g., Chaos Inoculation forces 1). Armour and
Evasion do NOT have override checks. In Rust, add `override_value("EnergyShield", ...)`.

**Gotcha — Iron Reflexes in slot loop:** When `ironReflexes` is true, the slot-loop
adds evasion's contribution to the `armour` accumulator, **not** evasion. The final
`output.Evasion` will be 0 (or very low, from any remaining non-Iron-Reflexes evasion
sources). The Rust `calc_primary_defences` already handles Iron Reflexes — but only for
the global evasion base, not for per-slot contributions.

### Section 3: `ArmourDefense` (lines 1551–1560)

`ArmourDefense` represents a percentage bonus to the effective armour multiplier
(from the Foulborn Ancestral Vision passive). It is written late in `calcs.defence`
after spell suppression is computed.

```lua
-- Lines 1551–1558: inject ArmourDefense MAX mods from SpellSuppressionAppliesToChanceToDefendWithArmour
-- (Foulborn Ancestral Vision only — rare build)
if modDB:Flag(nil, "SpellSuppressionAppliesToChanceToDefendWithArmour") then
    local suppressChance = spellSuppressionChance  -- already computed above
    local suppressArmourPercent = modDB:Max(nil, "SpellSuppressionAppliesToChanceToDefendWithArmourPercent") or 0
    local armourDefensePercent = modDB:Max(nil, "SpellSuppressionAppliesToChanceToDefendWithArmourPercentArmour") or 0
    -- Three scenarios injected as MAX mods (best case, average case, minimum case):
    modDB:NewMod("ArmourDefense", "MAX", armourDefensePercent - 100, "... Max Calc",
        { type = "Condition", var = "ArmourMax" })
    modDB:NewMod("ArmourDefense", "MAX",
        m_min(suppressArmourPercent * suppressChance / 100, 1.0) * (armourDefensePercent - 100),
        "... Average Calc", { type = "Condition", var = "ArmourAvg" })
    modDB:NewMod("ArmourDefense", "MAX",
        m_min(m_floor(suppressArmourPercent * suppressChance / 100), 1.0) * (armourDefensePercent - 100),
        "... Min Calc", ...)
end

-- Line 1559: final output write
output.ArmourDefense = (modDB:Max(nil, "ArmourDefense") or 0) / 100
-- modDB:Max returns highest MAX-type mod value, or nil if none exist
-- / 100 converts from integer percentage to float fraction (0.0–1.0)
-- e.g. ArmourDefense mod = 50 → output.ArmourDefense = 0.50
-- All 30 oracle builds have 0 ArmourDefense (none use Foulborn Ancestral Vision)
```

**Gotcha — `modDB:Max` vs `modDB:Sum`:** `Max` returns the single highest value among
all `MAX`-type mods, not a sum. In Rust: `mod_db.max_value("ArmourDefense", None, output).unwrap_or(0.0) / 100.0`.

## Existing Rust Code

File: `crates/pob-calc/src/calc/defence.rs`, function `calc_primary_defences`, lines 337–426

```
fn calc_primary_defences(env: &mut CalcEnv)
```

### What the Rust currently does

```rust
// Ward: sum Base "Ward", apply INC "Ward", More "Ward"
// ES:   sum Base "EnergyShield", apply INC "EnergyShield", More "EnergyShield"
// Eva:  sum Base "Evasion", apply INC "Evasion", More "Evasion"
// Arm:  sum Base "Armour", apply INC "Armour", More "Armour"
// Iron Reflexes: armour += evasion; evasion = 0
// Outputs: Ward, EnergyShield, Evasion, Armour, LowestOfArmourAndEvasion
```

The Rust models the computation as a single global pool per type, ignoring both
per-slot multipliers and multi-stat INC queries. It has no knowledge of items or
armour data at all — everything is assumed to be pre-injected into the modDB as
flat BASE values.

### Status table

| Feature | Rust status |
|---------|-------------|
| **Per-slot `*On{Slot}` fields** | ❌ Missing entirely — `defenceForConditionals` not implemented. All 13–17/30 oracle builds miss these. |
| **Multi-stat INC/More (Armour + ArmourAndEvasion + Defences)** | ❌ Missing — Rust queries only `"Armour"` INC/More; misses `ArmourAndEvasion`, `Defences`, `{Slot}ESAndArmour` contributions |
| **Per-slot `slotCfg.slotName` filtering** | ❌ Missing — slot-specific mods cannot be applied |
| **Iron Reflexes (per-slot)** | ❌ Partial — Rust does `armour += evasion; evasion = 0` globally, but the slot-loop evasion accumulation isn't per-slot-filtered |
| **`Gear:Armour`, `Gear:Evasion`, etc.** | ❌ Missing |
| **`Armour` round() rounding** | ⚠️ Partial — Rust uses `.floor()` but Lua uses `round()` (round-half-up). For most integer inputs no difference, but fractional values would diverge. |
| **`Ward` floor rounding** | ✅ Correct — Rust uses `.floor()` matching Lua's `m_floor()` |
| **`EnergyShield` Override check** | ❌ Missing |
| **`EnergyShield` round()** | ⚠️ Same as Armour: `.floor()` vs `round()` |
| **ES INC includes "Defences"** | ❌ Missing — Rust only queries `"EnergyShield"` INC |
| **Mana/Life/Evasion → Armour/ES conversions** | ❌ Missing |
| **`ArmourDefense` write** | ❌ Missing — Rust does not write `ArmourDefense` to output |
| **Pre-computation modDB injections (ArmourIncByFireRes, etc.)** | ❌ Missing — these are not applied before defence computation |
| **`ConvertBodyArmourArmourEvasionToWard`** | ❌ Missing |
| **`EnergyShieldToWard`** | ❌ Missing |
| **`ConvertArmourESToLife`** | ❌ Missing |
| **`GainNo*From{Slot}` suppression flags** | ❌ Missing |
| **EvasionDefense field** | ⚠️ Not in Rust either (correctly absent) — but it's in `field_groups.rs` and must be removed |

### Critical accuracy note

Because the Rust currently sums ALL `"Armour"` BASE mods (from setup.rs which adds
each item's armour as `Mod::new_base("Armour", armour_value, ...)`), the total base
matches. The INC application from only `"Armour"` INC works for most builds that only
have `"Armour"` INC mods (not `"Defences"` INC). However:
- Builds with `% increased Defences` (from Keystone/Passive) will produce incorrect results
- Builds with `% increased Armour and Evasion` will produce incorrect results
- Slot-specific INC (e.g., `% increased Armour and Energy Shield on Body Armour`) will be wrong

For the 30 oracle builds, the Armour/Evasion/ES values are likely correct by coincidence
(most common passive mods are `"Armour"` INC only), but this needs verification.

## What Needs to Change

1. **Implement `defenceForConditionals` equivalent** — write per-slot base values to
   output before the main defence calculation:
   ```rust
   // For each slot in ["Helmet", "Gloves", "Boots", "Body Armour", "Weapon 2", "Weapon 3"]:
   //   if item has armour_data and !flag("GainNo{Type}From{Slot}"):
   //     if base_value > 0: set_output("{Type}On{Slot}", base_value)
   ```
   This requires access to the item list / armour data per slot in `CalcEnv`.

2. **Extend INC/More queries to include multi-stat names**:
   - Armour: add `ArmourAndEvasion`, `Defences`, and `{Slot}ESAndArmour` to INC/More summation
   - Evasion: add `ArmourAndEvasion`, `Defences`
   - ES: add `Defences`
   - Ward: add `Defences`
   This requires `mod_db.sum_cfg` to support querying multiple stat names, or explicit
   multiple calls summed together.

3. **Apply `slotCfg.slotName` during per-slot accumulation** — each slot's contribution
   must be calculated with a `SkillCfg` that has `slot_name = Some(slot.to_string())`,
   so slot-specific mods like `HelmetESAndArmour` are applied only for that slot.

4. **Add rounding fix: `round()` not `.floor()` for Armour/Evasion/ES**:
   ```rust
   let armour = armour.round().max(0.0);  // not .floor()
   let energy_shield = energy_shield.round().max(0.0);
   let evasion = evasion.round().max(0.0);
   // Ward stays: ward.floor().max(0.0)
   ```

5. **Add `EnergyShield` Override check**:
   ```rust
   let energy_shield = mod_db.override_value("EnergyShield", None, &output)
       .unwrap_or_else(|| (es_base * multiplier).round().max(0.0));
   ```

6. **Add `Gear:Armour`, `Gear:Evasion`, `Gear:EnergyShield`, `Gear:Ward` outputs** —
   accumulate raw item base sums separately from the INC×More totals.

7. **Implement `ArmourDefense` output write**:
   ```rust
   let ad = env.player.mod_db.max_value("ArmourDefense", None, &output).unwrap_or(0.0);
   env.player.set_output("ArmourDefense", ad / 100.0);
   ```
   This must run after the `SpellSuppressionAppliesToChanceToDefendWithArmour` block
   injects the relevant `ArmourDefense` MAX mods.

8. **Implement pre-computation modDB injections** — the six conditional blocks at
   lines 772–823 (ArmourIncreasedByFireRes, EvasionIncreasedByColdRes,
   ESIncreasedByBlockChance, etc.) must inject INC mods into the modDB before
   the INC summation for each defence type.

9. **Implement stat-conversion paths** — Mana→Armour, Mana→ES, Life→Armour, Life→ES,
   Evasion→Armour (via `EvasionGainAsArmour`). These are uncommon but affect some
   oracle builds (e.g., builds using Eldritch Battery use Mana→ES).

10. **Remove phantom fields from `field_groups.rs`**:
    - Remove `"EvasionDefense"` — never written by PoB
    - Remove `"ArmourOnWeapon 1"` — Weapon 1 slot is excluded from the slot loop

## Oracle Confirmation (all 30 builds)

Format: `Armour / Evasion / EnergyShield / Ward / ArmourDefense`

| Build | Armour | Evasion | EnergyShield | Ward | ArmourDefense |
|-------|--------|---------|--------------|------|---------------|
| aura_stacker | 0 | 2455 | 2659 | 0 | 0 |
| bleed_gladiator | 4085 | 16 | 0 | 0 | 0 |
| bow_deadeye | 495 | 16426 | 5 | 0 | 0 |
| champion_impale | 3123 | 16 | 0 | 0 | 0 |
| ci_lowlife_es | 0 | 21 | 6024 | 0 | 0 |
| cluster_jewel | 0 | 21 | 3031 | 0 | 0 |
| coc_trigger | 189 | 44 | 5318 | 0 | 0 |
| cwc_trigger | 0 | 16 | 1255 | 0 | 0 |
| dot_caster_trickster | 0 | 18 | 1518 | 0 | 0 |
| dual_wield | 0 | 15 | 0 | 0 | 0 |
| ele_melee_raider | 104 | 3716 | 0 | 0 | 0 |
| flask_pathfinder | 104 | 18432 | 0 | 0 | 0 |
| ignite_elementalist | 0 | 22 | 3043 | 0 | 0 |
| max_block_gladiator | 5045 | 16 | 0 | 0 | 0 |
| mine_saboteur | 0 | 2050 | 507 | 0 | 0 |
| minion_necromancer | 326 | 22 | 3666 | 0 | 0 |
| mom_eb | 0 | 737 | 1117 | 0 | 0 |
| phys_melee_slayer | 14529 | 269 | 103 | 0 | 0 |
| phys_to_fire_conversion | 5341 | 484 | 172 | 0 | 0 |
| poison_pathfinder | 104 | 8677 | 0 | 0 | 0 |
| rf_juggernaut | 8061 | 20 | 0 | 0 | 0 |
| shield_1h | 3371 | 16 | 0 | 0 | 0 |
| spectre_summoner | 326 | 22 | 3752 | 0 | 0 |
| spell_caster_inquisitor | 0 | 15 | 1212 | 0 | 0 |
| timeless_jewel | 3039 | 16 | 0 | 0 | 0 |
| totem_hierophant | 0 | 15 | 1261 | 0 | 0 |
| trap_saboteur | 0 | 2183 | 497 | 0 | 0 |
| triple_conversion | 167 | 2499 | 0 | 0 | 0 |
| two_handed | 7197 | 20 | 0 | 0 | 0 |
| wand_occultist | 2438 | 1745 | 1611 | 0 | 0 |

> `ArmourDefense = 0` for all 30 builds. The Foulborn Ancestral Vision passive (which
> populates `ArmourDefense`) is not used in any oracle build.

> `Ward = 0` for all 30 builds (no Ward-based builds in oracle set). The Ward
> rounding uses `m_floor()` vs the `round()` used for Armour/Evasion/ES.
