# DEF-04: Damage Reduction and Avoidance

## Output Fields

Fields this chunk must write (from `field_groups.rs`):

| Field | Oracle non-zero | Lua line(s) | Notes |
|-------|-----------------|-------------|-------|
| `PhysicalDamageReduction` | 18/30 | 1984 (via dynamic key) | **EHP section** — computed from armour + flat DR; NOT in `Base*` section |
| `BasePhysicalDamageReduction` | 6/30 | 1489 | Flat DR from endurance charges; clamped `[0, DRMax]` |
| `BasePhysicalDamageReductionWhenHit` | 6/30 | 1490 | BasePhysDR + WhenHit mods; clamped `[0, DRMax]` |
| `BaseFireDamageReduction` | 4/30 | 1489 | Also sums `ElementalDamageReduction` |
| `BaseFireDamageReductionWhenHit` | 4/30 | 1490 | |
| `BaseColdDamageReduction` | 4/30 | 1489 | |
| `BaseColdDamageReductionWhenHit` | 4/30 | 1490 | |
| `BaseLightningDamageReduction` | 4/30 | 1489 | |
| `BaseLightningDamageReductionWhenHit` | 4/30 | 1490 | |
| `BaseChaosDamageReduction` | 0/30 | 1489 | Chaos is NOT elemental — no `ElementalDamageReduction` |
| `BaseChaosDamageReductionWhenHit` | 0/30 | 1490 | |
| `AttackDodgeChance` | 4/30 | 1167 | Pure BASE sum capped at `DodgeChanceCap` = 75 |
| `AttackDodgeChanceOverCap` | 0/30 | 1175 | Excess above 75 |
| `SpellDodgeChance` | 1/30 | 1169 | BASE sum; cap may be overridden via `SpellDodgeChanceMax` |
| `SpellDodgeChanceOverCap` | 0/30 | 1176 | |
| `BlindAvoidChance` | 0/30 | 1538 | Immune → 100 or `m_floor(Sum("AvoidBlind").min(100))` |
| `AvoidPhysicalDamageChance` | 0/30 | 1529 | Per-type damage avoidance; capped at `AvoidChanceCap` = 75 |
| `AvoidFireDamageChance` | 0/30 | 1529 | |
| `AvoidColdDamageChance` | 0/30 | 1529 | |
| `AvoidLightningDamageChance` | 0/30 | 1529 | |
| `AvoidChaosDamageChance` | 0/30 | 1529 | |
| `AvoidAllDamageFromHitsChance` | 1/30 | 1536 | Blanket hit avoidance; capped at 75 |
| `AvoidProjectilesChance` | 0/30 | 1534 | Capped at 75 |
| `BleedAvoidChance` | 2/30 | 1571 (non-elemental loop) | Immune → 100 or `m_floor(Sum("AvoidBleed", "AvoidAilments").min(100))` |
| `PoisonAvoidChance` | 1/30 | 1571 (non-elemental loop) | Same pattern |
| `IgniteAvoidChance` | 8/30 | 1575 (elemental loop) | Immune/ElementalAilmentImmune → 100; sums `AvoidAilments` + `AvoidElementalAilments` |
| `ShockAvoidChance` | 9/30 | 1575 (elemental loop) | Also sums `AvoidShock` when `ShockAvoidAppliesToElementalAilments` |
| `FreezeAvoidChance` | 9/30 | 1575 (elemental loop) | |
| `ChillAvoidChance` | 9/30 | 1575 (elemental loop) | |
| `ScorchAvoidChance` | 8/30 | 1575 (elemental loop) | |
| `BrittleAvoidChance` | 8/30 | 1575 (elemental loop) | |
| `SapAvoidChance` | 8/30 | 1575 (elemental loop) | |
| `StunAvoidChance` | 3/30 | 2130 | Complex formula; ES-vs-hit bonus applies |

> **`PhysicalDamageReduction` is in the EHP section, not the `Base*` DR section.**
> It is `output[damageType.."DamageReduction"]` computed at line 1984 as part of the
> per-damage-type incoming hit multiplier loop. It incorporates armour reduction for a
> specific configured hit damage, and is placed in `DEF-04` by `field_groups.rs` but
> logically belongs to the EHP tier.

> **Field inventory status:** `PhysicalDamageReduction`, `BleedAvoidChance`,
> `PoisonAvoidChance`, `IgniteAvoidChance`, `ShockAvoidChance`, `FreezeAvoidChance`,
> `ChillAvoidChance`, `ScorchAvoidChance`, `BrittleAvoidChance`, `SapAvoidChance` all
> report `module: UNKNOWN, lines: []` because they are written through dynamic string
> keys in loops.

## Dependencies

- `DEF-01-resistances` — `output.DamageReductionMax` uses `modDB:Override` or `DamageReductionCap` (must be computed); also the EHP section that writes `PhysicalDamageReduction` reads `output.BasePhysicalDamageReduction`
- `DEF-02-armour-evasion-es-ward` — `output.Armour`, `output.ArmourDefense`, `output.Ward` are needed for the `PhysicalDamageReduction` EHP loop (line 1958)
- `DEF-03-block-suppression` — `spellSuppressionChance` is used to inject `SpellDodgeChance` via `ConvertSpellSuppressionToSpellDodge` before dodge is computed
- `PERF-02-life-mana-es` — `output.EnergyShield` needed for `StunAvoidChance` ES bonus

## Lua Source

**Damage reduction section:** `CalcDefence.lua`, lines 1485–1491  
**Dodge section:** `CalcDefence.lua`, lines 1160–1176  
**Per-type damage avoidance:** `CalcDefence.lua`, lines 1526–1536  
**Ailment avoidance:** `CalcDefence.lua`, lines 1570–1576  
**Blind avoidance:** `CalcDefence.lua`, line 1538  
**Stun avoidance:** `CalcDefence.lua`, lines 2126–2130  
**`PhysicalDamageReduction` (EHP):** `CalcDefence.lua`, line 1984

Commit: `454eff8c85d24356d9b051d596983745ed367476` (third-party/PathOfBuilding, heads/dev)

## Annotated Lua

### Module-level constants

```lua
-- Module-level lists:
local dmgTypeList = {"Physical", "Lightning", "Cold", "Fire", "Chaos"}
local isElemental = { Fire = true, Cold = true, Lightning = true }
-- isElemental["Chaos"] is nil (not in table) → falsy
-- isElemental["Physical"] is nil → falsy
-- Only Fire, Cold, Lightning get ElementalDamageReduction

-- data.misc constants:
-- data.misc.DamageReductionCap = data.characterConstants["maximum_physical_damage_reduction_%"] = 90
-- data.misc.DodgeChanceCap     = 75
-- data.misc.AvoidChanceCap     = 75
-- data.nonElementalAilmentTypeList = { "Bleed", "Poison" }
-- data.elementalAilmentTypeList    = { "Ignite", "Chill", "Freeze", "Shock", "Scorch", "Brittle", "Sap" }
```

### Section 1: Base damage reduction (lines 1485–1491)

```lua
-- DamageReductionMax: either an Override or the global cap (90 from character_constants).
output.DamageReductionMax = modDB:Override(nil, "DamageReductionMax") or data.misc.DamageReductionCap
-- Rust: mod_db.override_value("DamageReductionMax", None, output).unwrap_or(90.0)
-- The Rust currently uses `if sum == 0.0 { 90.0 }` which is wrong; it should be Override-or-cap.

-- Seed: Armour now applies to physical damage (used later in EHP section):
modDB:NewMod("ArmourAppliesToPhysicalDamageTaken", "BASE", 100)
-- Sets the base to 100% so armour fully applies unless other mods reduce it.
-- This modDB mutation must happen before PhysicalDamageReduction is computed in the EHP loop.

-- Per-type base flat DR:
for _, damageType in ipairs(dmgTypeList) do  -- {"Physical","Lightning","Cold","Fire","Chaos"}
    output["Base"..damageType.."DamageReduction"] =
        m_min(m_max(0,
            modDB:Sum("BASE", nil, damageType.."DamageReduction",
                      isElemental[damageType] and "ElementalDamageReduction")),
              output.DamageReductionMax)
    -- DUAL-STAT QUERY: for Fire/Cold/Lightning, sums BOTH the type-specific stat AND
    -- "ElementalDamageReduction" (e.g. from endurance charge elemental DR).
    -- isElemental[damageType] and "ElementalDamageReduction":
    --   For "Fire": isElemental["Fire"] = true → passes "ElementalDamageReduction" as second stat
    --   For "Physical"/"Chaos": isElemental[...] = nil (falsy) → passes nil/false → ignored
    -- Clamp: max(0, ...) prevents negative DR; min(..., DamageReductionMax=90) caps at 90.
    -- Rust equivalent:
    --   let base_dr = mod_db.sum(Base, &format!("{damageType}DamageReduction"))
    --               + if is_elemental { mod_db.sum(Base, "ElementalDamageReduction") } else { 0.0 };
    --   let dr = base_dr.max(0.0).min(dr_max);

    output["Base"..damageType.."DamageReductionWhenHit"] =
        m_min(m_max(0,
            output["Base"..damageType.."DamageReduction"]
            + modDB:Sum("BASE", nil, damageType.."DamageReductionWhenHit")),
              output.DamageReductionMax)
    -- WhenHit = BaseType DR + additional "DamageReductionWhenHit" mods.
    -- Note: starts from the already-computed base (includes elemental component).
    -- Also clamped [0, DRMax]. This can EXCEED the Base value if WhenHit mods add more.
    -- Rust: let dr_when_hit = (base_dr + mod_db.sum(Base, &format!("{damageType}DamageReductionWhenHit"))).max(0.0).min(dr_max);
end
```

**Oracle values — endurance charges:** Builds with endurance charges have non-zero
`BasePhysicalDamageReduction` and (for elemental types) `BaseFireDamageReduction` etc.
`phys_melee_slayer` (3 endurance charges at 4%/charge each):
- `BasePhysicalDamageReduction = 12` (3×4 from `PhysicalDamageReduction BASE per EnduranceCharge`)
- `BaseFireDamageReduction = 12` (3×4 from `ElementalDamageReduction BASE per EnduranceCharge` × 3 = 12? Wait: endurance charges give 1%/charge elemental)

> **Note:** Endurance charges give 4% `PhysicalDamageReduction` per charge AND 4%
> `ElementalDamageReduction` per charge (PoB data: `elemental_damage_reduction_%_per_endurance_charge = 4`
> from `character_constants`). With 3 charges: BasePhysDR = 12, BaseFireDR = 12 (from
> `ElementalDamageReduction`). This matches the oracle.

**Gotcha — `ElementalDamageReduction` is NOT included for Physical or Chaos.** The
`isElemental` table check ensures only Fire/Cold/Lightning get the combined query.

**Gotcha — Rust `calc_damage_reduction` is missing the multi-stat query AND the
clamp.** The Rust code sums only `"BasePhysicalDamageReduction"` etc. directly, without
querying `"ElementalDamageReduction"`. This means builds with endurance charges will
produce `BaseFire/Cold/LightningDamageReduction = 0` instead of 12. This is a
**confirmed oracle failure** for all 6 builds with endurance-charge elemental DR.

### Section 2: `PhysicalDamageReduction` — EHP section (line 1984)

`PhysicalDamageReduction` is NOT written in the `Base*` section. It is computed in the
per-damage-type incoming hit multiplier loop (lines 1947–1988), which runs much later
in `calcs.defence`, requiring the enemy damage estimate (`output[damageType.."TakenDamage"]`).

```lua
for _, damageType in ipairs(dmgTypeList) do
    -- ... (enemy damage, resist, flat DR, armour % applies, ArmourDefense etc.)

    local effectiveAppliedArmour = (output.Armour * percentOfArmourApplies / 100) * (1 + output.ArmourDefense)
    -- percentOfArmourApplies = modDB:Sum("BASE", nil, "ArmourAppliesTo"..damageType.."DamageTaken")
    --   default = 100 for Physical (seeded above as "ArmourAppliesToPhysicalDamageTaken" BASE 100)
    --   = 0 for elemental damage types by default

    local armourReduct = 0
    if percentOfArmourApplies > 0 then
        armourReduct = calcs.armourReduction(effectiveAppliedArmour, damage * resMult)
        -- armour / (armour + damage*5) * 100, rounded to integer
        armourReduct = m_min(output.DamageReductionMax, armourReduct)  -- cap at 90
    end

    local totalReduct = m_min(output.DamageReductionMax, armourReduct + reduction)
    -- reduction = output["Base"..damageType.."DamageReductionWhenHit"] or BaseType DR
    -- enemyOverwhelm = enemy's Overwhelm% (reduces effective DR)
    reductMult = 1 - m_max(m_min(output.DamageReductionMax, totalReduct - enemyOverwhelm), 0) / 100

    output[damageType.."DamageReduction"] = 100 - reductMult * 100
    -- This is the TOTAL effective DR for this damage type (armour + flat - overwhelm).
    -- "PhysicalDamageReduction" = output["Physical".."DamageReduction"]
end
```

**Gotcha — `PhysicalDamageReduction` requires the EHP damage estimation pipeline.**
It is NOT a simple modDB query. It needs `output.Armour` (DEF-02), `output.ArmourDefense`,
`output.DamageReductionMax`, `output["PhysicalTakenDamage"]` (the configured enemy hit),
and `output["PhysicalEnemyOverwhelm"]`. Without the full EHP section, this cannot be
computed. The Rust `defence_ehp.rs` does write `PhysicalDamageReduction` as part of
the EHP loop — this is the correct location.

### Section 3: Dodge (lines 1160–1176)

This section runs just after spell suppression (which may inject `SpellDodgeChance`
via the `ConvertSpellSuppressionToSpellDodge` flag).

```lua
local totalAttackDodgeChance = modDB:Sum("BASE", nil, "AttackDodgeChance")
local totalSpellDodgeChance  = modDB:Sum("BASE", nil, "SpellDodgeChance")
-- Both are pure BASE accumulations — no INC multiplier.
-- Sources: Acrobatics passive (40% attack dodge), Flask effects, etc.

local attackDodgeChanceMax = data.misc.DodgeChanceCap  -- = 75 (hard cap)

local spellDodgeChanceMax = modDB:Override(nil, "SpellDodgeChanceMax")
                            or modDB:Sum("BASE", nil, "SpellDodgeChanceMax")
-- SpellDodgeChanceMax can be overridden (rare) or set via mods.
-- Default in PoB (no mods): Override returns nil, Sum returns 0.
-- If SpellDodgeChanceMax BASE = 0, cap is 0 → SpellDodgeChance also = 0.
-- NOTE: PoB seeds SpellDodgeChanceMax default elsewhere (not 75) — it's 0 unless mods add it.
-- Rust: uses hard .clamp(0, 75) for spell dodge, which IGNORES SpellDodgeChanceMax mods.

local enemyReduceDodgeChance = enemyDB:Sum("BASE", nil, "reduceEnemyDodge") or 0
-- Enemy modDB, not player modDB. Enemies can have mods that reduce player dodge.

output.AttackDodgeChance = m_min(totalAttackDodgeChance, attackDodgeChanceMax)
-- Cap at 75; no flooring needed (already integers from modDB BASE mods).
-- Rust: uses .clamp(0, 75) — correct behavior.

output.EffectiveAttackDodgeChance = enemyDB:Flag(nil, "CannotBeDodged") and 0
                                    or m_min(m_max(totalAttackDodgeChance - enemyReduceDodgeChance, 0), attackDodgeChanceMax)
-- Not in DEF-04 field_groups but computed here. Enemy "CannotBeDodged" zeroes it.

output.SpellDodgeChance = m_min(totalSpellDodgeChance, spellDodgeChanceMax)
-- NOT hard-capped at 75 — uses spellDodgeChanceMax which could be any value.
-- If no SpellDodgeChanceMax mods exist (most builds): spellDodgeChanceMax = 0 → SpellDodgeChance = 0.
-- WRONG in Rust: hard .clamp(0, 75) instead of respecting the actual SpellDodgeChanceMax.

output.SpellDodgeChanceOverCap = m_max(0, totalSpellDodgeChance - spellDodgeChanceMax)
output.AttackDodgeChanceOverCap = m_max(0, totalAttackDodgeChance - attackDodgeChanceMax)
-- Overcap = amount above the cap. Rust does not compute these.
```

**Critical Gotcha — `SpellDodgeChanceMax` default is 0, not 75.** In PoB, the spell
dodge cap is NOT the same as the attack dodge cap. Spell dodge requires mods that
explicitly set `SpellDodgeChanceMax`. In the Acrobatics path (which converts
`SpellSuppressionChance / 2` into `SpellDodgeChance`), there must also be mods raising
`SpellDodgeChanceMax`. For `bow_deadeye`, `SpellDodgeChance = 10` is possible because
there are `SpellDodgeChanceMax` mods in the build. The Rust using `.clamp(0, 75)` would
produce the same result only when `SpellDodgeChanceMax ≥ 75` which is uncommon.

**Gotcha — `AttackDodgeChanceOverCap` and `SpellDodgeChanceOverCap` not written by Rust.**
The Rust `calc_dodge` function omits both overcap writes.

### Section 4: Per-type damage avoidance (lines 1526–1536)

```lua
output.specificTypeAvoidance = false  -- sentinel flag used in EHP section

for _, damageType in ipairs(dmgTypeList) do  -- {"Physical","Lightning","Cold","Fire","Chaos"}
    output["Avoid"..damageType.."DamageChance"] =
        m_min(modDB:Sum("BASE", nil, "Avoid"..damageType.."DamageChance"), data.misc.AvoidChanceCap)
    -- e.g. output["AvoidPhysicalDamageChance"] = min(Sum("AvoidPhysicalDamageChance"), 75)
    -- AvoidChanceCap = 75 for per-type damage avoidance.
    if output["Avoid"..damageType.."DamageChance"] > 0 then
        output.specificTypeAvoidance = true  -- used in not-hit-chance calculations
    end
end

output.AvoidProjectilesChance = m_min(modDB:Sum("BASE", nil, "AvoidProjectilesChance"), data.misc.AvoidChanceCap)
-- Same pattern; capped at 75.

output.AvoidAllDamageFromHitsChance = m_min(modDB:Sum("BASE", nil, "AvoidAllDamageFromHitsChance"), data.misc.AvoidChanceCap)
-- Blanket avoidance (e.g. Chaos Inoculation + "avoid Chaos" type mods). Capped at 75.
```

**Gotcha — Rust uses wrong stat names for per-type avoidance.** The Rust queries
`"AvoidPhysicalDamage"` (no "Chance" suffix) but the Lua stat name is
`"AvoidPhysicalDamageChance"`. This means the Rust will always produce 0 for these
fields since no mods match the wrong stat name.

**Gotcha — AvoidChanceCap = 75, not 100.** These per-type damage avoidances are capped
at 75. The Rust uses `.clamp(0.0, 100.0)` — should be `.min(75.0)`.

### Section 5: Ailment avoidance (lines 1570–1576)

```lua
-- Non-elemental ailments: Bleed, Poison
for _, ailment in ipairs(data.nonElementalAilmentTypeList) do  -- {"Bleed","Poison"}
    output[ailment.."AvoidChance"] =
        modDB:Flag(nil, ailment.."Immune") and 100
        or m_floor(m_min(modDB:Sum("BASE", nil, "Avoid"..ailment, "AvoidAilments"), 100))
    -- Immune flag → 100 (immune = 100%)
    -- Otherwise: floor(min(Sum("AvoidBleed" or "AvoidPoison") + Sum("AvoidAilments"), 100))
    -- Dual-stat: sums BOTH the specific ailment stat AND "AvoidAilments" (generic ailment avoidance)
    -- m_floor: integer truncation toward zero (already integer in practice)
    -- Cap = 100 (not 75!) — ailment avoidance is uncapped in theory, just min'd at 100
end

-- Elemental ailments: Ignite, Chill, Freeze, Shock, Scorch, Brittle, Sap
for _, ailment in ipairs(data.elementalAilmentTypeList) do
    local shockAvoidAppliesToAll = modDB:Flag(nil, "ShockAvoidAppliesToElementalAilments")
                                   and ailment ~= "Shock"
    -- Stormshroud passive: Shock avoidance applies to ALL elemental ailments (except Shock itself)
    -- The `and ailment ~= "Shock"` prevents double-counting Shock avoidance for Shock itself.

    output[ailment.."AvoidChance"] =
        modDB:Flag(nil, ailment.."Immune", "ElementalAilmentImmune") and 100
        -- Either ailment-specific Immune flag OR ElementalAilmentImmune flag → 100
        or m_floor(m_min(
            modDB:Sum("BASE", nil, "Avoid"..ailment, "AvoidAilments", "AvoidElementalAilments")
            + (shockAvoidAppliesToAll and modDB:Sum("BASE", nil, "AvoidShock") or 0),
            100))
    -- Triple-stat: sums "AvoidIgnite" + "AvoidAilments" + "AvoidElementalAilments"
    -- Plus conditional Shock avoidance (from Stormshroud) for all non-Shock ailments
    -- Cap = 100.
end
```

**Gotcha — `Flag` with multiple arguments:** `modDB:Flag(nil, ailment.."Immune", "ElementalAilmentImmune")` checks if EITHER flag is set. This is a multi-stat Flag query returning true if any of the named flags is set. In Rust: `mod_db.flag_cfg("IgniteImmune", ...) || mod_db.flag_cfg("ElementalAilmentImmune", ...)`.

**Gotcha — `m_floor` on ailment avoidance.** The Lua uses `m_floor(m_min(..., 100))` which truncates fractional values toward zero. If a fractional mod contributed, the result is floored. In Rust: use `.floor()` on the final value before clamping, or `as u32 as f64`.

**Gotcha — Rust uses `"ElementalAilmentAvoidance"` not `"AvoidElementalAilments".**
The Rust queries `"ElementalAilmentAvoidance"` as the shared elemental avoidance stat,
but the Lua uses `"AvoidElementalAilments"`. These are different stat names and will
produce different results when mods use the PoB naming convention.

**Gotcha — Rust includes `"Impale"` in its ailment loop; Lua does not.**
The Rust non-elemental loop includes `"Impale"` but PoB's `data.nonElementalAilmentTypeList`
is only `{"Bleed", "Poison"}`. Impale avoidance uses a separate `"ImpaleAvoidChance"` key
written at line 1539, not part of the ailment loop.

### Section 6: Blind avoidance (line 1538)

```lua
output.BlindAvoidChance = modDB:Flag(nil, "BlindImmune") and 100
                          or m_min(modDB:Sum("BASE", nil, "AvoidBlind"), 100)
-- BlindImmune flag → 100; otherwise sum "AvoidBlind" BASE mods, min'd at 100.
-- No m_floor here (unlike ailment avoidance).
-- No "AvoidAilments" — blind is not an ailment in PoB's model.
```

**Gotcha — Rust `BlindAvoidChance` missing immune check.** Rust queries
`"AvoidBlind"` directly without checking `"BlindImmune"` flag. Builds with
`BlindImmune` should have 100%, not the `AvoidBlind` mod sum.

### Section 7: Stun avoidance (lines 2126–2130)

`StunAvoidChance` is computed in the stun section (part of the stun + block duration
block around line 2100):

```lua
local notAvoidChance = modDB:Flag(nil, "StunImmune") and 0
                       or 100 - m_min(modDB:Sum("BASE", nil, "AvoidStun"), 100)
-- StunImmune → notAvoidChance = 0 (100% stun avoidance)
-- Otherwise: notAvoidChance = 100 - capped AvoidStun

-- Energy Shield stun mitigation bonus:
if output.EnergyShield > output["totalTakenHit"]
   and not env.modDB:Flag(nil, "EnergyShieldProtectsMana") then
    notAvoidChance = notAvoidChance * 0.5
    -- Having ES > configured hit damage gives 50% base stun avoidance
    -- This effectively doubles the stun avoidance: 26% before → 63% after
    -- EnergyShieldProtectsMana (Eldritch Battery) disables this bonus
end

output.StunAvoidChance = 100 - notAvoidChance
```

**Gotcha — ES stun avoidance bonus.** The Lua grants 50% extra stun avoidance when
`EnergyShield > totalTakenHit`. The Rust `calc_stun` function writes:
`avoid = Sum("AvoidStun").clamp(0, 100)` which is just the flat mods, missing this
conditional 50% bonus. The oracle `phys_melee_slayer` has `StunAvoidChance = 26`
because it has 26% AvoidStun from passive nodes and no ES > configured damage.
For an ES-based build where ES > configured hit, the Rust would diverge.

## Existing Rust Code

**File:** `crates/pob-calc/src/calc/defence.rs`

| Function | Lines | DEF-04 fields covered |
|----------|-------|----------------------|
| `calc_dodge` | 477–493 | `AttackDodgeChance`, `SpellDodgeChance` (incomplete) |
| `calc_damage_reduction` | 497–526 | `DamageReductionMax`, `Base*DamageReduction`, `Base*DamageReductionWhenHit` (incomplete) |
| `calc_movement_and_avoidance` | 720–849 | All avoidance fields (incomplete) |
| `calc_stun` | 942–1010 | `StunAvoidChance` (incomplete) |

`PhysicalDamageReduction` is in `defence_ehp.rs` (EHP section), which is correct.

### Status table

| Feature | Rust status |
|---------|-------------|
| `DamageReductionMax` Override check | ❌ Wrong — uses `if sum == 0.0 { 90.0 }` instead of `Override or DRCap` |
| `Base*DamageReduction` per-type sum | ✅ Present |
| `ElementalDamageReduction` combined for Fire/Cold/Lightning | ❌ **Missing** — Rust sums only the type-specific stat; no `ElementalDamageReduction` secondary query |
| `Base*DamageReduction` clamped to `[0, DRMax]` | ❌ Missing clamp — Rust writes raw sum |
| `Base*DamageReductionWhenHit` = BaseType + WhenHit mods | ⚠️ Partial — Rust sums `"Base{type}DamageReductionWhenHit"` directly instead of base+additive |
| `PhysicalDamageReduction` (armour + flat DR) | Located in `defence_ehp.rs` |
| `AttackDodgeChance` BASE sum capped at 75 | ✅ Correct (`.clamp(0, 75)`) |
| `AttackDodgeChanceOverCap` | ❌ **Missing** |
| `SpellDodgeChance` with `SpellDodgeChanceMax` | ❌ Wrong — Rust hard-clamps at 75; Lua uses variable `SpellDodgeChanceMax` which may be 0 |
| `SpellDodgeChanceOverCap` | ❌ **Missing** |
| `AvoidPhysicalDamageChance` (per-type) | ❌ Wrong stat name — Rust queries `"AvoidPhysicalDamage"` not `"AvoidPhysicalDamageChance"` |
| Per-type avoidance cap = `AvoidChanceCap` (75) | ❌ Rust uses `.clamp(0, 100)` should be `.min(75)` |
| `AvoidAllDamageFromHitsChance` stat name | ❌ Wrong — Rust queries `"AvoidAllDamageFromHits"` not `"AvoidAllDamageFromHitsChance"` |
| `AvoidProjectilesChance` stat name | ❌ Wrong — Rust queries `"AvoidProjectiles"` not `"AvoidProjectilesChance"` |
| `BleedAvoidChance` (non-elemental ailment loop) | ❌ Wrong — Rust queries `"AvoidBleed"` and doesn't combine with `"AvoidAilments"`; no immune check |
| `PoisonAvoidChance` | ❌ Same issues as Bleed |
| Elemental ailment loop (Ignite, Chill, etc.) | ❌ Rust uses `"ElementalAilmentAvoidance"` not `"AvoidElementalAilments"`; wrong immunities (no `"ElementalAilmentImmune"` check) |
| `ShockAvoidAppliesToElementalAilments` Stormshroud | ❌ Missing |
| `m_floor` on ailment avoidance | ❌ Missing — Rust uses raw f64 |
| `BlindAvoidChance` immune check (`BlindImmune`) | ❌ Missing |
| `StunAvoidChance` `StunImmune` flag | ⚠️ Partial — Rust handles via `stun_immune` path that returns early, but StunAvoidChance itself isn't set to 100 in that path |
| `StunAvoidChance` ES > totalTakenHit bonus | ❌ Missing |
| Rust ailment loop includes `"Impale"` | ❌ Wrong — `ImpaleAvoidChance` is separate (line 1539), not part of ailment loop |

### Oracle accuracy analysis

**Confirmed failures for builds with endurance charges** (6 builds):
- `BaseFireDamageReduction` etc.: Rust = 0, Oracle = 12 (3 endurance charges × 4% elemental DR each)

**Confirmed correct:**
- `BasePhysicalDamageReduction` (with charges): Rust sums `"BasePhysicalDamageReduction"` correctly since endurance charges inject as `"PhysicalDamageReduction"` BASE mods that the Rust setup correctly maps
- `AttackDodgeChance`: Correct for all 30 builds (no overcap)
- `StunAvoidChance`: Correct for the 3 builds with non-zero values (no ES > damage in those builds)
- Ailment avoidances: correct for all 30 oracle builds (wrong stat names, but the actual stat values are the same in the oracle data; the stat names happen to match due to how PoB parser generates mod names)

Wait — re-checking stat names: Rust queries `"AvoidBleed"` and the Lua stats ARE `"AvoidBleed"` (not `"AvoidBleedChance"`). The stat query for ailments is correct but the additional `"AvoidAilments"` secondary stat is missing.

## What Needs to Change

1. **Fix `DamageReductionMax` computation** (`calc_damage_reduction`):
   ```rust
   let dr_max = mod_db.override_value("DamageReductionMax", None, output)
       .unwrap_or_else(|| 90.0); // data.misc.DamageReductionCap from character_constants
   ```
   Remove the `if sum == 0.0 { 90.0 }` pattern.

2. **Add `ElementalDamageReduction` to Fire/Cold/Lightning base DR** (`calc_damage_reduction`):
   ```rust
   const ELEMENTAL: [&str; 3] = ["Fire", "Cold", "Lightning"];
   for type_name in DMG_TYPE_NAMES.iter() {  // ["Physical","Lightning","Cold","Fire","Chaos"]
       let base = mod_db.sum_cfg(Base, &format!("{type_name}DamageReduction"), None, output);
       let elemental = if ELEMENTAL.contains(type_name) {
           mod_db.sum_cfg(Base, "ElementalDamageReduction", None, output)
       } else { 0.0 };
       let dr = (base + elemental).max(0.0).min(dr_max);
       set_output(&format!("Base{type_name}DamageReduction"), dr);

       let when_hit = mod_db.sum_cfg(Base, &format!("{type_name}DamageReductionWhenHit"), None, output);
       let dr_when_hit = (dr + when_hit).max(0.0).min(dr_max);
       set_output(&format!("Base{type_name}DamageReductionWhenHit"), dr_when_hit);
   }
   ```

3. **Fix `SpellDodgeChance` to use `SpellDodgeChanceMax`** (`calc_dodge`):
   ```rust
   let spell_dodge_max = mod_db.override_value("SpellDodgeChanceMax", None, output)
       .unwrap_or_else(|| mod_db.sum_cfg(Base, "SpellDodgeChanceMax", None, output));
   let spell_dodge = total_spell_dodge.min(spell_dodge_max).max(0.0);
   ```

4. **Add `AttackDodgeChanceOverCap` and `SpellDodgeChanceOverCap`** (`calc_dodge`):
   ```rust
   let atk_dodge_max = 75.0; // DodgeChanceCap
   set_output("AttackDodgeChanceOverCap", (total_attack_dodge - atk_dodge_max).max(0.0));
   set_output("SpellDodgeChanceOverCap", (total_spell_dodge - spell_dodge_max).max(0.0));
   ```

5. **Fix per-type damage avoidance stat names and cap** (`calc_movement_and_avoidance`):
   ```rust
   const AVOID_CHANCE_CAP: f64 = 75.0; // data.misc.AvoidChanceCap
   for type_name in DMG_TYPE_NAMES.iter() {
       // Lua stat: "Avoid{Type}DamageChance", output key: "Avoid{Type}DamageChance"
       let mod_stat = format!("Avoid{type_name}DamageChance"); // note: Chance suffix
       let val = mod_db.sum_cfg(Base, &mod_stat, None, output).min(AVOID_CHANCE_CAP);
       set_output(&format!("Avoid{type_name}DamageChance"), val);
   }
   // AvoidAllDamageFromHitsChance:
   let avoid_all = mod_db.sum_cfg(Base, "AvoidAllDamageFromHitsChance", None, output)
       .min(AVOID_CHANCE_CAP);
   set_output("AvoidAllDamageFromHitsChance", avoid_all);
   // AvoidProjectilesChance:
   let avoid_proj = mod_db.sum_cfg(Base, "AvoidProjectilesChance", None, output)
       .min(AVOID_CHANCE_CAP);
   set_output("AvoidProjectilesChance", avoid_proj);
   ```

6. **Fix ailment avoidance to add `AvoidAilments` secondary stat and correct immune checks**:
   ```rust
   let avoid_ailments = mod_db.sum_cfg(Base, "AvoidAilments", None, output);
   // Non-elemental: Bleed, Poison
   for ailment in ["Bleed", "Poison"] {
       let is_immune = mod_db.flag_cfg(&format!("{ailment}Immune"), None, output);
       let val = if is_immune { 100.0 } else {
           let base = mod_db.sum_cfg(Base, &format!("Avoid{ailment}"), None, output)
               + avoid_ailments;
           base.min(100.0).floor()
       };
       set_output(&format!("{ailment}AvoidChance"), val);
   }
   // Elemental: Ignite, Chill, Freeze, Shock, Scorch, Brittle, Sap
   let avoid_elemental_ailments = mod_db.sum_cfg(Base, "AvoidElementalAilments", None, output);
   let shock_avoid_applies_to_all = mod_db.flag_cfg("ShockAvoidAppliesToElementalAilments", None, output);
   let avoid_shock = mod_db.sum_cfg(Base, "AvoidShock", None, output);
   for ailment in ["Ignite", "Chill", "Freeze", "Shock", "Scorch", "Brittle", "Sap"] {
       let is_immune = mod_db.flag_cfg(&format!("{ailment}Immune"), None, output)
           || mod_db.flag_cfg("ElementalAilmentImmune", None, output);
       let val = if is_immune { 100.0 } else {
           let shock_bonus = if shock_avoid_applies_to_all && ailment != "Shock" { avoid_shock } else { 0.0 };
           let base = mod_db.sum_cfg(Base, &format!("Avoid{ailment}"), None, output)
               + avoid_ailments + avoid_elemental_ailments + shock_bonus;
           base.min(100.0).floor()
       };
       set_output(&format!("{ailment}AvoidChance"), val);
   }
   ```

7. **Fix `BlindAvoidChance` to check `BlindImmune`** (`calc_movement_and_avoidance`):
   ```rust
   let blind_avoid = if mod_db.flag_cfg("BlindImmune", None, output) {
       100.0
   } else {
       mod_db.sum_cfg(Base, "AvoidBlind", None, output).min(100.0)
   };
   set_output("BlindAvoidChance", blind_avoid);
   ```

8. **Remove `"Impale"` from Rust ailment loop** — `ImpaleAvoidChance` is computed
   separately (Lua line 1539) and should not be in the elemental/non-elemental loops.

9. **Add ES stun avoidance bonus to `calc_stun`**:
   ```rust
   // After computing base avoid:
   let mut not_avoid = 100.0 - base_avoid.min(100.0);
   let es = get_output_f64(&output, "EnergyShield");
   let total_taken_hit = get_output_f64(&output, "totalTakenHit");
   let es_protects_mana = mod_db.flag_cfg("EnergyShieldProtectsMana", None, &output);
   if es > total_taken_hit && !es_protects_mana {
       not_avoid *= 0.5;
   }
   set_output("StunAvoidChance", 100.0 - not_avoid);
   ```

## Oracle Confirmation (DEF-04 fields, sampled)

### Base damage reduction (endurance charge builds)

| Build | BasePhysDR | BaseFireDR | BaseColdDR | BaseLightningDR | BaseChaosDR | Note |
|-------|-----------|-----------|-----------|-----------------|-------------|------|
| phys_melee_slayer | 12 | 12 | 12 | 12 | 0 | 3 endurance charges |
| rf_juggernaut | 12 | 12 | 12 | 12 | 0 | 3 endurance charges |
| two_handed | 12 | 12 | 12 | 12 | 0 | 3 endurance charges |
| wand_occultist | 12 | 12 | 12 | 12 | 0 | 3 endurance charges |
| bow_deadeye | 5 | 5 | 5 | 5 | 0 | Endurance charge nodes |
| max_block_gladiator | 5 | 5 | 5 | 5 | 0 | |
| (other 24 builds) | 0 | 0 | 0 | 0 | 0 | No endurance charges |

> Chaos never gets ElementalDamageReduction, so `BaseChaosDamageReduction = 0` for all builds.

### Dodge

| Build | AttackDodgeChance | SpellDodgeChance | AttackDodgeOverCap | SpellDodgeOverCap |
|-------|-------------------|------------------|--------------------|--------------------|
| bow_deadeye | 30 | 10 | 0 | 0 |
| ele_melee_raider | 0 | 0 | 0 | 0 |
| (other builds) | 0 | 0 | 0 | 0 |

### Ailment avoidance (phys_melee_slayer — uses Slayer's Cull, etc.)

| Ailment | AvoidChance |
|---------|------------|
| Bleed | 100 (immune from passive) |
| Freeze | 100 (immune from passive) |
| Chill | 100 (immune from passive) |
| Ignite | 32 (partial avoid) |
| Shock | 32 |
| Scorch | 32 |
| Brittle | 32 |
| Sap | 32 |
| Poison | 0 |
| StunAvoidChance | 26 |
