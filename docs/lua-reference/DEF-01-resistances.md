# DEF-01: Resistances

## Output Fields

Fields this chunk must write (from `field_groups.rs`):

| Field | Oracle present | Notes |
|-------|---------------|-------|
| `FireResist` | 30/30 | Capped final resistance |
| `FireResistTotal` | 30/30 | Uncapped total before max cap |
| `FireResistOverCap` | 30/30 | Amount exceeding the cap |
| `ColdResist` | 30/30 | |
| `ColdResistTotal` | 30/30 | |
| `ColdResistOverCap` | 30/30 | |
| `LightningResist` | 30/30 | |
| `LightningResistTotal` | 30/30 | |
| `LightningResistOverCap` | 30/30 | |
| `ChaosResist` | 30/30 | |
| `ChaosResistTotal` | 30/30 | |
| `ChaosResistOverCap` | 30/30 | |
| `FireResistOver` | 0/30 | **Phantom** — not written by PoB; remove from `field_groups.rs` |
| `ColdResistOver` | 0/30 | **Phantom** — same |
| `LightningResistOver` | 0/30 | **Phantom** — same |
| `ChaosResistOver` | 0/30 | **Phantom** — same |

> **Note on `*ResistOver` (without "Cap"):** These four fields do not exist in PoB's
> Lua output and appear in 0/30 oracle files. The actual field written by the Lua is
> `*ResistOverCap`. Remove `FireResistOver`, `ColdResistOver`, `LightningResistOver`,
> `ChaosResistOver` from `field_groups.rs`.

> **Note on `field_inventory_output.json`:** All 12 real resistance fields appear as
> `module: "UNKNOWN", lines: []` in the field inventory. This is a script limitation —
> the fields are written via dynamic keys `output[elem.."Resist"]` in a loop, which the
> static inventory script cannot resolve. The actual source is confirmed as
> `CalcDefence.lua`, `calcs.resistances` function, lines 509–635.

## Dependencies

- `SETUP-01` / `SETUP-03` — modDB must be populated with item and passive resistance
  mods (BASE values from gear, passive tree, flasks) before this chunk runs
- `PERF-06-aura-curse` — aura/curse effects that modify resistances must be applied;
  curse exposure can reduce resist (written as negative BASE mods)

## Lua Source

File: `CalcDefence.lua`, lines 509–635  
Commit: `454eff8c85d24356d9b051d596983745ed367476` (third-party/PathOfBuilding, heads/dev)

## Annotated Lua

### Module-level constants (CalcDefence.lua lines 1–29)

```lua
-- Math aliases used throughout the file:
local m_min = math.min   -- Rust: x.min(y)
local m_max = math.max   -- Rust: x.max(y)
local m_modf = math.modf -- Rust: x.trunc() for the integer part (truncate toward zero)

-- isElemental table: used to include "ElementalResist" / "ElementalResistMax" as
-- additional stat names for Fire, Cold, Lightning — NOT for Chaos.
local isElemental = { Fire = true, Cold = true, Lightning = true }
-- Rust pattern:
--   let is_elemental = matches!(elem, "Fire" | "Cold" | "Lightning");

-- The four resistance types, in iteration order (determines loop behaviour):
local resistTypeList = { "Fire", "Cold", "Lightning", "Chaos" }
```

**Key data constants (from `data.misc` in `Data.lua`):**
```lua
data.misc.ResistFloor   = -200  -- minimum resistance (floor applied after all computation)
data.misc.MaxResistCap  = 90    -- absolute upper limit on any resist max (overrides can't exceed this)
-- base_maximum_all_resistances_% = 75 (from character_constants, seeded via CalcSetup.lua)
```

### Section 1: `calcs.resistances` — Resistance conversion (lines 515–560)

This section converts one resistance type's BASE mods into another type's mods.
Triggered by exotic items/keystones (e.g., "Purity of Elements" synergy mods, Sapphire
of Transcendence). **None of the 30 oracle builds use this path**, but it must be
implemented for correctness.

```lua
-- Pass 1: Convert MAX resist mods between types
for _, resFrom in ipairs(resistTypeList) do
    local maxRes                           -- lazily initialised (nil until needed)
    for _, resTo in ipairs(resistTypeList) do
        -- conversionRate is the fraction (0–1) to convert
        local conversionRate = modDB:Sum("BASE", nil, resFrom.."MaxResConvertTo"..resTo) / 100
        if conversionRate ~= 0 then
            if not maxRes then
                -- Lazy init: sum only non-"Base" mods (exclude the default 75 seed)
                maxRes = 0
                for _, mod in ipairs(modDB:Tabulate("BASE", nil, resFrom.."ResistMax")) do
                    if mod.mod.source ~= "Base" then
                        -- Tabulate returns {mod, value} pairs; mod.source is the origin string
                        maxRes = maxRes + mod.value
                    end
                end
            end
            if maxRes ~= 0 then
                modDB:NewMod(resTo.."ResistMax", "BASE", maxRes * conversionRate, ...)
                -- Adds a new BASE mod to the destination type's max resist
            end
        end
    end
end

-- Pass 2: Convert actual RESIST mods between types (BASE, INC, and MORE all transferred)
for _, resFrom in ipairs(resistTypeList) do
    local res                              -- lazily initialised
    for _, resTo in ipairs(resistTypeList) do
        local conversionRate = modDB:Sum("BASE", nil, resFrom.."ResConvertTo"..resTo) / 100
        if conversionRate ~= 0 then
            if not res then
                -- Lazy init: sum only non-"Base" source mods
                res = 0
                for _, mod in ipairs(modDB:Tabulate("BASE", nil, resFrom.."Resist")) do
                    if mod.mod.source ~= "Base" then
                        res = res + mod.value
                    end
                end
            end
            if res ~= 0 then
                modDB:NewMod(resTo.."Resist", "BASE", res * conversionRate, ...)
            end
            -- Also copy INC and MORE modifiers proportionally:
            for _, mod in ipairs(modDB:Tabulate("INC", nil, resFrom.."Resist")) do
                modDB:NewMod(resTo.."Resist", "INC", mod.value * conversionRate, mod.mod.source)
            end
            for _, mod in ipairs(modDB:Tabulate("MORE", nil, resFrom.."Resist")) do
                modDB:NewMod(resTo.."Resist", "MORE", mod.value * conversionRate, mod.mod.source)
            end
        end
    end
end
```

**Gotcha — lazy initialisation pattern:** `if not maxRes then ... end` uses Lua's nil
check. In Lua, a local variable starts as `nil`; the first time `conversionRate ~= 0`
is true, `maxRes` is set to `0` (the number) and then accumulated. The `not maxRes`
check is `true` when `maxRes == nil` (not yet computed) but `false` when `maxRes == 0`
(already computed but zero). This avoids re-computing the sum on every inner-loop
iteration. In Rust, use `Option<f64>` initialized to `None`.

**Gotcha — `mod.mod.source`:** `modDB:Tabulate` returns a list of `{mod = modEntry, value = evaluatedValue}`. The `mod.mod.source` is the text label of where the mod came from. `"Base"` is the special source string used for the 75% default resist max seeded in `CalcSetup.lua`. Excluding `"Base"` source mods means the default 75 cap is never transferred during conversion.

### Section 2: Melding of the Flesh (lines 562–578)

```lua
-- Melding of the Flesh unique jewel: all elemental max resists become equal to
-- the highest of the three.
if modDB:Flag(nil, "ElementalResistMaxIsHighestResistMax") then
    local highestResistMax = 0
    local highestResistMaxType = ""
    for _, elem in ipairs(resistTypeList) do
        -- Re-compute each max resist inline (same formula as main loop below):
        local resistMax = modDB:Override(nil, elem.."ResistMax")
                          or m_min(data.misc.MaxResistCap,       -- hard cap at 90
                                   modDB:Sum("BASE", nil, elem.."ResistMax",
                                             isElemental[elem] and "ElementalResistMax"))
        --   isElemental[elem] and "ElementalResistMax"
        --   If elem == "Fire"/"Cold"/"Lightning": passes "ElementalResistMax" as a
        --   second stat name to sum (sums mods from both stats in one call).
        --   If elem == "Chaos": isElemental["Chaos"] == nil (falsy), so this evaluates
        --   to `false`, and modDB:Sum only sums "ChaosResistMax" mods.
        --   In Rust:
        --     let extra_stat = if is_elemental { Some("ElementalResistMax") } else { None };
        --     let sum = mod_db.sum(Base, elem_max) + extra_stat.map_or(0.0, |s| mod_db.sum(Base, s));
        if resistMax > highestResistMax and isElemental[elem] then
            highestResistMax = resistMax
            highestResistMaxType = elem
        end
    end
    -- Override all elemental max resists to the highest value:
    for _, elem in ipairs(resistTypeList) do
        if isElemental[elem] then
            modDB:NewMod(elem.."ResistMax", "OVERRIDE", highestResistMax, highestResistMaxType.." Melding of the Flesh")
        end
    end
    -- Chaos is NOT affected by Melding of the Flesh
end
```

**Gotcha — `isElemental[elem] and "ElementalResistMax"`:** Lua's `and` operator returns
the second operand if the first is truthy, or the first if falsy. So when `elem == "Fire"`,
`isElemental["Fire"]` is `true`, and the expression evaluates to `"ElementalResistMax"`.
When `elem == "Chaos"`, `isElemental["Chaos"]` is `nil` (key not present in table), and
the expression evaluates to `nil` / `false`. The `modDB:Sum` varargs call treats `false`
as no additional stat. In Rust, this is best expressed as an extra conditional sum.

### Section 3: Main resistance computation loop (lines 580–634)

This is the core of the chunk. It iterates over `{"Fire", "Cold", "Lightning", "Chaos"}`.

```lua
for _, elem in ipairs(resistTypeList) do
    local min, max, total, dotTotal, totemTotal, totemMax

    -- FLOOR: always -200 (can go very negative but not below -200)
    min = data.misc.ResistFloor   -- = -200

    -- MAX RESIST: either overridden directly, or sum of BASE mods, capped at 90
    max = modDB:Override(nil, elem.."ResistMax")
          or m_min(data.misc.MaxResistCap,   -- = 90 (absolute upper bound for any max resist)
                   modDB:Sum("BASE", nil, elem.."ResistMax",
                             isElemental[elem] and "ElementalResistMax"))
    -- Rust:
    --   let max_resist = mod_db.override_value(elem_max, None, output)
    --       .unwrap_or_else(|| {
    --           let base = mod_db.sum(Base, elem_max) + elemental_max;
    --           base.min(90.0)  // MaxResistCap = 90
    --       });
    -- NOTE: The Rust currently does NOT apply the 90-cap! See "What Needs to Change".

    -- TOTAL RESIST: either an override, or computed via base * (1 + INC/100) * More
    total = modDB:Override(nil, elem.."Resist")
    if not total then
        local base = modDB:Sum("BASE", nil, elem.."Resist",
                               isElemental[elem] and "ElementalResist")
        -- base sums all BASE mods for both e.g. "FireResist" and "ElementalResist"
        -- Note: "ElementalResist" BASE mods apply to all three elemental resistances.

        local inc = m_max(
            calcLib.mod(modDB, nil, elem.."Resist",
                        isElemental[elem] and "ElementalResist"),
            0)
        -- calcLib.mod = (1 + INC/100) * More
        -- INC and More are summed from BOTH elem+"Resist" and "ElementalResist" (for elemental)
        -- m_max(..., 0) clamps the MULTIPLIER to zero minimum.
        -- This means: if all INC/More mods would reduce the multiplier below 0,
        -- the result is 0 (total resistance = 0), not negative.
        -- IMPORTANT: This does NOT clamp the final resistance value — just the multiplier.
        -- Rust:
        --   let inc = mod_db.sum(Inc, elem_resist) + elemental_inc;
        --   let more = mod_db.more(elem_resist) * elemental_more;
        --   let multiplier = ((1.0 + inc / 100.0) * more).max(0.0);

        total = base * inc
        -- In Rust: let total = base * multiplier;

        -- DOT TOTAL: same multiplier but only BASE mods from Dot-flagged sources
        local dotBase = modDB:Sum("BASE",
                                   { flags = ModFlag.Dot, keywordFlags = 0 },
                                   elem.."Resist",
                                   isElemental[elem] and "ElementalResist")
        dotTotal = dotBase * inc
        -- Note: dotBase uses a cfg table with ModFlag.Dot set — filters to only mods
        -- that apply to damage-over-time. The multiplier (inc) is shared with the hit resist.
        -- dotTotal is used for *ResistOverTime output (NOT in DEF-01's field_groups).
    end

    -- TOTEM TOTAL: totems have their own resist values
    totemMax = modDB:Override(nil, "Totem"..elem.."ResistMax")
               or m_min(data.misc.MaxResistCap,
                        modDB:Sum("BASE", nil, "Totem"..elem.."ResistMax",
                                  isElemental[elem] and "TotemElementalResistMax"))
    totemTotal = modDB:Override(nil, "Totem"..elem.."Resist")
    if not totemTotal then
        local base = modDB:Sum("BASE", nil, "Totem"..elem.."Resist",
                               isElemental[elem] and "TotemElementalResist")
        totemTotal = base * m_max(calcLib.mod(modDB, nil, "Totem"..elem.."Resist",
                                              isElemental[elem] and "TotemElementalResist"), 0)
    end

    -- TRUNCATION: all resistance values are truncated toward zero (not floored!)
    total = m_modf(total)
    -- m_modf returns integer part: m_modf(72.9) = 72, m_modf(-53.7) = -53
    -- NOT math.floor: m_floor(-53.7) = -54, but m_modf(-53.7) = -53
    -- Rust: x.trunc() -- same truncation-toward-zero semantics
    dotTotal = dotTotal and m_modf(dotTotal) or total
    -- Lua ternary: if dotTotal is non-nil, truncate it; otherwise use total
    -- (dotTotal is nil when total was overridden)
    totemTotal = m_modf(totemTotal)
    min = m_modf(min)   -- min = -200 (already integer, no-op)
    max = m_modf(max)   -- max = 75 normally (already integer), but could be fractional

    -- CLAMPING: apply [min, max] bounds
    local final = m_max(m_min(total, max), min)
    -- = total.min(max).max(min) = total.clamp(min, max)
    -- Rust: let final_resist = total.clamp(min, max);
    -- where min = -200 (ResistFloor), max = the computed resist max (usually 75)
    local dotFinal = m_max(m_min(dotTotal, max), min)
    local totemFinal = m_max(m_min(totemTotal, totemMax), min)

    -- OUTPUT WRITES for this chunk:
    output[elem.."Resist"] = final                          -- capped final value
    output[elem.."ResistTotal"] = total                     -- uncapped total
    output[elem.."ResistOverCap"] = m_max(0, total - max)   -- overflow above cap

    -- Additional outputs NOT in DEF-01 field_groups (do not break these):
    output[elem.."ResistOver75"] = m_max(0, final - 75)     -- overcap above base 75
    output["Missing"..elem.."Resist"] = m_max(0, max - final)
    output[elem.."ResistOverTime"] = dotFinal               -- dot resist (usually = final)
    output["Totem"..elem.."Resist"] = totemFinal
    output["Totem"..elem.."ResistTotal"] = totemTotal
    output["Totem"..elem.."ResistOverCap"] = m_max(0, totemTotal - totemMax)
    output["MissingTotem"..elem.."Resist"] = m_max(0, totemMax - totemFinal)

    if breakdown then
        breakdown[elem.."Resist"] = {
            "Min: "..min.."%",
            "Max: "..max.."%",
            "Total: "..total.."%",
        }
        -- Rust: always populate breakdown (see LUA-GOTCHAS §Breakdown Patterns)
    end
end
```

### Summary of the total-resist formula

For non-overridden, non-Chaos:
```
total = (Base("FireResist") + Base("ElementalResist"))
      × max(0, (1 + (INC("FireResist") + INC("ElementalResist")) / 100)
               × More("FireResist") × More("ElementalResist"))

truncated = trunc(total)   -- truncate toward zero, not floor
final = clamp(truncated, -200, max_resist)
```

For Chaos:
```
total = Base("ChaosResist")
      × max(0, (1 + INC("ChaosResist") / 100) × More("ChaosResist"))

truncated = trunc(total)
final = clamp(truncated, -200, max_resist)
```

Note that `max_resist` itself is clamped: `min(90, Base("FireResistMax") + Base("ElementalResistMax"))`.

### Notes on INC/MORE resist mods in the game

Resistances use INC/MORE mods in specific cases:
- Corrupted items: "X% reduced/increased Fire/Cold/Lightning/Chaos Resistance" → INC
- Keystones/uniques: "50% less Cold Resistance" → MORE (`value = -50`, so factor = `1 + (-50/100) = 0.5`)
- "Chaos Resistance is doubled" → MORE with value=100 (factor = 2.0)
- The clamped-to-0 floor on the multiplier ensures these never reverse the sign

In practice, most builds use only BASE resist mods (flat % values from gear/passives),
so INC/MORE resist mods are rare. However, when present they critically affect outcomes.
The Rust `calc_resistances` currently **ignores all INC and MORE** resist mods — this is
the primary correctness bug.

## Existing Rust Code

File: `crates/pob-calc/src/calc/defence.rs`, lines 62–170

```
fn calc_resistances(env: &mut CalcEnv)
```

### Status table

| Feature | Rust status |
|---------|-------------|
| `PhysicalResist = 0` | ✅ Correct |
| Override check for `*ResistMax` | ✅ Present |
| BASE sum for `*ResistMax` | ✅ Correct |
| BASE sum for `ElementalResistMax` (for Fire/Cold/Lightning) | ✅ Correct |
| **MaxResistCap (90) clamp on max resist** | ❌ **Missing** — Lua clamps to `m_min(90, sum)`, Rust does `base + elemental` with no 90-cap |
| ResistFloor (-200) clamp on final | ❌ **Missing** — Rust uses `total.min(max_resist)` but no lower bound |
| Melding of the Flesh (ElementalResistMaxIsHighestResistMax) | ✅ Present (takes highest of three) |
| **Melding override re-uses Override rather than the raw sum** | ⚠️ The Lua re-computes each max with Override check first; Rust uses the already-computed `max_resists` array which may have an Override result. This could diverge if Override and BASE interact — but in practice the Melding path is correct. |
| Override check for `*Resist` | ✅ Present |
| BASE sum for `*Resist` | ✅ Correct |
| BASE sum for `ElementalResist` (for Fire/Cold/Lightning) | ✅ Correct |
| **INC mods for `*Resist` + `ElementalResist`** | ❌ **Missing** — Rust doesn't apply INC at all |
| **MORE mods for `*Resist` + `ElementalResist`** | ❌ **Missing** — Rust doesn't apply More at all |
| **Clamp multiplier to 0 minimum** | ❌ Missing (follows from INC/More being absent) |
| **Truncation (`m_modf`)** before clamping | ❌ **Missing** — Rust does no truncation; values remain f64 |
| `*ResistTotal` output | ✅ Writes uncapped total |
| `*ResistOverCap` output | ✅ Correct formula (`total - max).max(0)` |
| `*ResistOver75` output | ✅ Writes `(total - 75).max(0)` but this is the final capped value minus 75, not the total. Lua writes `m_max(0, final - 75)` which is also the final minus 75, so ✅ same. |
| `Missing*Resist` output | ✅ Correct |
| `*ResistOverTime` output | ⚠️ Rust writes the capped final value (same as `*Resist`), which is correct only when the dotBase equals the regular base. Lua computes a separate dotTotal using Dot-flagged cfg. This diverges only for builds with `*ResistOverTime`-specific mods, none of which appear in the 30 oracle builds. |
| Totem resist computation | ⚠️ Partially wrong — Rust shares the player's max resist for totems (line 157: `let totem_max = max_resist`). The Lua computes a separate `totemMax` via `Totem{elem}ResistMax` and `TotemElementalResistMax` mods. Also, Rust does BASE only for totem resist; Lua applies INC/More the same way. |
| Resistance conversion (`*ResConvertTo*`, `*MaxResConvertTo*`) | ❌ **Missing** entirely — not in the oracle builds but required for correctness |
| **Breakdown population** | ⚠️ Not implemented in Rust (breakdown not used in tests) |

### Critical correctness issue

The INC/MORE omission only affects builds that actually have INC or MORE resist mods.
For the 30 oracle builds, resistance mods appear to be exclusively BASE (flat percentage
values from gear and passives), so the Rust currently produces **correct oracle results
for all 30 builds by coincidence**. However, the MaxResistCap omission may cause issues
on builds where items push the max resist above 75 — the Rust would allow max resists
above 90, while Lua caps at 90.

The truncation omission (`m_modf`) also appears to have no effect on the oracle builds
because all resistance values end up as integers in practice (gear mods are whole numbers
and the INC/More is absent).

## What Needs to Change

1. **Add MaxResistCap (90) clamp to `*ResistMax` computation** (`defence.rs::calc_resistances`):
   ```rust
   const MAX_RESIST_CAP: f64 = 90.0; // data.misc.MaxResistCap
   // In the unwrap_or_else closure:
   (base_max + elemental_max).min(MAX_RESIST_CAP)
   ```
   This is the absolute upper limit on any resistance cap. Even if a player has +20 to
   max fire resist, the max fire resist cap cannot exceed 90%.

2. **Add ResistFloor (-200) clamp to `*Resist` final value** (`defence.rs::calc_resistances`):
   ```rust
   const RESIST_FLOOR: f64 = -200.0; // data.misc.ResistFloor
   let final_resist = total_resist.min(max_resist).max(RESIST_FLOOR);
   // Currently Rust has: total_resist.min(max_resist) — missing the .max(-200.0)
   ```
   Without this, builds with extreme negative resist (e.g., hexblast self-curse builds)
   would produce values below -200.

3. **Apply INC and More multipliers to `*Resist` total** (`defence.rs::calc_resistances`):
   This is the most impactful missing piece for non-oracle correctness:
   ```rust
   let total_resist = mod_db.override_value(resist_stat, None, output)
       .unwrap_or_else(|| {
           let base = mod_db.sum_cfg(Base, resist_stat, None, output)
                    + if is_elemental { mod_db.sum_cfg(Base, "ElementalResist", None, output) }
                      else { 0.0 };
           let inc = mod_db.sum_cfg(Inc, resist_stat, None, output)
                   + if is_elemental { mod_db.sum_cfg(Inc, "ElementalResist", None, output) }
                     else { 0.0 };
           let more = mod_db.more_cfg(resist_stat, None, output)
                    * if is_elemental { mod_db.more_cfg("ElementalResist", None, output) }
                      else { 1.0 };
           let multiplier = ((1.0 + inc / 100.0) * more).max(0.0); // clamp multiplier, not result
           base * multiplier
       });
   ```

4. **Apply truncation (`trunc()`) to all resistance values** (`defence.rs::calc_resistances`):
   ```rust
   let total_trunc = total_resist.trunc(); // m_modf equivalent: truncate toward zero
   let max_trunc = max_resist.trunc();
   let min_trunc = RESIST_FLOOR.trunc(); // -200.0 → -200.0 (no-op)
   let final_resist = total_trunc.clamp(min_trunc, max_trunc);
   ```
   Note: `.trunc()` not `.floor()`. For negative values: `(-53.7).trunc() = -53.0` but
   `(-53.7).floor() = -54.0`. Resistances are negative in chaos resist, so this matters.

5. **Fix totem resist max** (`defence.rs::calc_resistances`):
   Totems have their own max resist stats (`TotemFireResistMax`, etc.) that can be set
   independently of the player's max resist. Rust currently uses `max_resist` (the
   player's max). Change to:
   ```rust
   let totem_max_stat = format!("Totem{elem}ResistMax");
   let totem_max = mod_db.override_value(&totem_max_stat, None, output)
       .unwrap_or_else(|| {
           let base = mod_db.sum_cfg(Base, &totem_max_stat, None, output)
                    + if is_elemental { mod_db.sum_cfg(Base, "TotemElementalResistMax", None, output) }
                      else { 0.0 };
           base.min(MAX_RESIST_CAP)
       });
   // Then apply INC/More for totem resist similarly
   ```

6. **Implement resistance conversion** (low priority — TAIL, not needed for 30 oracle
   builds):
   The `*ResConvertTo*` and `*MaxResConvertTo*` loops (Lua lines 515–560) inject new
   mods into the modDB. This requires the `Tabulate` operation on the Rust modDB
   (iterating all matching mods with their values). Defer until a build using it is added.

7. **Remove phantom fields from `field_groups.rs`**:
   Remove `"FireResistOver"`, `"ColdResistOver"`, `"LightningResistOver"`,
   `"ChaosResistOver"` — these do not exist in PoB output.

## Oracle Confirmation (all 30 builds)

Format: `Resist/Total/OverCap` for each element.

| Build | Fire | Cold | Lightning | Chaos |
|-------|------|------|-----------|-------|
| aura_stacker | 75/296/221 | 75/256/181 | 75/222/147 | -60/-60/0 |
| bleed_gladiator | 75/161/86 | 75/144/69 | 75/100/25 | -60/-60/0 |
| bow_deadeye | 76/77/1 | 76/77/1 | 76/77/1 | -21/-21/0 |
| champion_impale | 75/158/83 | 75/139/64 | 75/130/55 | -60/-60/0 |
| ci_lowlife_es | 75/288/213 | 75/195/120 | 75/219/144 | -38/-38/0 |
| cluster_jewel | 75/253/178 | 75/168/93 | 75/134/59 | -60/-60/0 |
| coc_trigger | 80/125/45 | 75/77/2 | 76/98/22 | -25/-25/0 |
| cwc_trigger | 75/247/172 | 75/126/51 | 75/101/26 | -60/-60/0 |
| dot_caster_trickster | 75/135/60 | 75/145/70 | 75/150/75 | -28/-28/0 |
| dual_wield | -60/-60/0 | -60/-60/0 | -60/-60/0 | -60/-60/0 |
| ele_melee_raider | 75/125/50 | 75/84/9 | 75/79/4 | -60/-60/0 |
| flask_pathfinder | 75/153/78 | 75/126/51 | 75/139/64 | -8/-8/0 |
| ignite_elementalist | 75/231/156 | 75/181/106 | 75/143/68 | -60/-60/0 |
| max_block_gladiator | 75/214/139 | 75/191/116 | 75/144/69 | -60/-60/0 |
| mine_saboteur | 75/251/176 | 75/120/45 | 75/112/37 | -60/-60/0 |
| minion_necromancer | 75/241/166 | 75/158/83 | 75/124/49 | -60/-60/0 |
| mom_eb | 75/212/137 | 75/117/42 | 75/161/86 | -43/-43/0 |
| phys_melee_slayer | 76/105/29 | 72/72/0 | 75/98/23 | -53/-53/0 |
| phys_to_fire_conversion | 76/177/101 | 75/136/61 | 75/150/75 | -60/-60/0 |
| poison_pathfinder | 75/159/84 | 75/130/55 | 75/111/36 | -4/-4/0 |
| rf_juggernaut | 81/332/251 | 75/218/143 | 75/98/23 | -38/-38/0 |
| shield_1h | 75/220/145 | 75/199/124 | 75/150/75 | -60/-60/0 |
| spectre_summoner | 75/244/169 | 75/159/84 | 75/129/54 | -60/-60/0 |
| spell_caster_inquisitor | 75/157/82 | 75/115/40 | 75/175/100 | -60/-60/0 |
| timeless_jewel | 75/170/95 | 75/151/76 | 75/139/64 | -60/-60/0 |
| totem_hierophant | 75/207/132 | 75/141/66 | 75/153/78 | -60/-60/0 |
| trap_saboteur | 75/214/139 | 75/146/71 | 75/139/64 | -60/-60/0 |
| triple_conversion | 75/208/133 | 75/130/55 | 75/119/44 | -60/-60/0 |
| two_handed | 76/221/145 | 75/142/67 | 75/105/30 | -60/-60/0 |
| wand_occultist | 75/84/9 | 75/105/30 | 75/86/11 | 68/68/0 |

Notable builds:
- **bow_deadeye / phys_melee_slayer / two_handed / phys_to_fire**: Fire=76 means
  `FireResistMax` was raised to 76 (e.g., via "+1% to max Fire Resist" on a shield or
  passive node)
- **coc_trigger / rf_juggernaut**: Fire=80/81, raised max resist significantly
- **dual_wield**: All resistances are -60 (fully unresisted)
- **wand_occultist**: Chaos=68 (positive chaos resist, unusual)
- All 30 builds have `Resist == Total` when `OverCap == 0`, confirming the capping logic

> The Rust implementation currently produces correct results for all 30 builds because:
> (a) all resist mods in these builds are BASE type only (no INC/MORE resist mods), and
> (b) no build has a resist max above 90 (so the MaxResistCap omission is harmless).
> The missing INC/MORE support and MaxResistCap clamp are correctness gaps that must be
> fixed before the chunk can be considered complete.
