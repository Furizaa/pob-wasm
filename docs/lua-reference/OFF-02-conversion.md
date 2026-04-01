# OFF-02-conversion: Damage Conversion Chain

## Output Fields

Conversion is purely **intermediate** — the conversion table itself is not stored in
`output[]`. Its results manifest in the per-type `{Type}MinBase` / `{Type}MaxBase` fields
written during the base-damage pass (OFF-01), and in the `allMult` / `convMult` scalars
applied during the hit-damage pass. There are no standalone oracle-visible output fields
that belong exclusively to this chunk.

However, the following fields are written **as a direct result of the conversion chain**
during the per-pass hit loop and are only correct when the conversion table is correct:

| Field | Where set | What it is |
|-------|-----------|-----------|
| `output.allMult` | CalcOffence.lua:3184/3186 | `convMult * ScaledDamageEffect * RuthlessBlowHitEffect * FistOfWarDamageEffect * warcryEffect` |
| `{Type}HitAverage` (one per dmgType) | offence.rs:371–373 | Crit-weighted per-type average, used by ailments |
| `{Type}Min`, `{Type}Max` (one pair per dmgType) | CalcOffence.lua / offence.rs:283–342 | Final hit damage after conversion + inc/more |

Because the field_groups registry marks this chunk as placeholder (`&[]`), the chunk
oracle will not assert any fields for OFF-02. The conversion logic correctness is validated
indirectly through the damage fields asserted in OFF-01, OFF-03, OFF-04, OFF-05, and OFF-07.

> **Note for implementer:** when the chunk test infrastructure is wired up for OFF-02, the
> most useful fields to add to `field_groups.rs` are `PhysicalHitAverage`, `FireHitAverage`,
> `ColdHitAverage`, `LightningHitAverage`, `ChaosHitAverage`, and the `{Type}EffMult`
> fields written at line 3341 (enemy resistance applied).

## Dependencies

- `OFF-01-base-damage` — must be correct first; conversion takes `{Type}MinBase` /
  `{Type}MaxBase` as input.
- `SETUP-04-eval-mod-stubs` — `SkillName`, `SkillId`, and `SocketedIn` stubs must be
  complete so that `skillModList:Sum` for per-skill conversion mods returns the right
  values.

## Lua Source

File: `third-party/PathOfBuilding/src/Modules/CalcOffence.lua`  
Commit: `454eff8c85d24356d9b051d596983745ed367476`

Key line ranges:
- **Aliases / globals:** lines 1–33 (math aliases, `dmgTypeList`, `isElemental`)
- **`calcDamage()`:** lines 68–139 (recursive conversion application)
- **`calcAilmentSourceDamage()`:** lines 141–152 (uses `convMult` from table)
- **Random-physical pre-processing:** lines 929–986
- **`buildConversionTable()`:** lines 1848–1894
- **Attach tables to skill/weapon cfgs:** lines 1896–1902
- **Hit-damage pass using `convMult`:** lines 3140–3219

## Annotated Lua

### 1. Module-level aliases and the conversion sequence (lines 30–33)

```lua
local isElemental = { Fire = true, Cold = true, Lightning = true }
-- List of all damage types, ordered according to the conversion sequence
local dmgTypeList = {"Physical", "Lightning", "Cold", "Fire", "Chaos"}
```

> **Rust note:** `DMG_TYPE_NAMES = ["Physical", "Lightning", "Cold", "Fire", "Chaos"]` in
> `offence_utils.rs:16` matches this order exactly. `CONVERSION_ORDER` follows the same
> index sequence. The `isElemental` map → Rust equivalent is just checking whether the
> `dtype` string is one of `"Fire"`, `"Cold"`, `"Lightning"`.

---

### 2. `buildConversionTable(cfg)` (lines 1848–1894)

This function is the core of the chunk. It is called **three times**:
- Once for the skill-level `skillCfg` → `activeSkill.conversionTable`
- Once for `weapon1Cfg` (if attack and weapon 1 is used)
- Once for `weapon2Cfg` (if dual-wielding)

```lua
local function buildConversionTable(cfg)
    local conversionTable = { }
    for damageTypeIndex = 1, 4 do                    -- NOTE: only indices 1–4 (Physical/Lightning/Cold/Fire)
        local damageType = dmgTypeList[damageTypeIndex]
        local globalConv = wipeTable(tempTable1)     -- reused scratch table, cleared each iteration
        local skillConv  = wipeTable(tempTable2)     -- idem
        local add        = wipeTable(tempTable3)     -- idem (for gain-as)
        local globalTotal, skillTotal = 0, 0

        for otherTypeIndex = damageTypeIndex + 1, 5 do   -- only types LATER in the chain
            local otherType = dmgTypeList[otherTypeIndex]

            -- GLOBAL conversion: {Src}DamageConvertTo{Dst}
            -- ALSO checks ElementalDamageConvertTo{Dst} when src is elemental
            -- ALSO checks NonChaosDamageConvertTo{Dst} when src is not Chaos
            globalConv[otherType] = m_max(
                skillModList:Sum("BASE", cfg,
                    damageType.."DamageConvertTo"..otherType,
                    isElemental[damageType] and "ElementalDamageConvertTo"..otherType or nil,
                    damageType ~= "Chaos" and "NonChaosDamageConvertTo"..otherType or nil
                ), 0)
            globalTotal = globalTotal + globalConv[otherType]

            -- SKILL conversion: Skill{Src}DamageConvertTo{Dst}
            -- These come from the active skill itself (e.g. "Skill Fire" gems like Incinerate)
            skillConv[otherType] = m_max(
                skillModList:Sum("BASE", cfg, "Skill"..damageType.."DamageConvertTo"..otherType), 0)
            skillTotal = skillTotal + skillConv[otherType]

            -- GAIN-AS-EXTRA: {Src}DamageGainAs{Dst}
            -- Also checks ElementalDamageGainAs{Dst} and NonChaosDamageGainAs{Dst}
            add[otherType] = m_max(
                skillModList:Sum("BASE", cfg,
                    damageType.."DamageGainAs"..otherType,
                    isElemental[damageType] and "ElementalDamageGainAs"..otherType or nil,
                    damageType ~= "Chaos" and "NonChaosDamageGainAs"..otherType or nil
                ), 0)
        end

        -- ── Capping logic ──────────────────────────────────────────────────
        if skillTotal > 100 then
            -- Skill conversion alone exceeds 100%: scale skill down, zero out global
            local factor = 100 / skillTotal
            for type, val in pairs(skillConv) do
                skillConv[type] = val * factor
            end
            for type, val in pairs(globalConv) do
                globalConv[type] = 0               -- global wiped entirely
            end
        elseif globalTotal + skillTotal > 100 then
            -- Combined exceeds 100%: scale global proportionally, keep skill intact
            local factor = (100 - skillTotal) / globalTotal
            for type, val in pairs(globalConv) do
                globalConv[type] = val * factor
            end
            globalTotal = globalTotal * factor
        end
        -- After capping: globalTotal + skillTotal <= 100

        local dmgTable = { conversion = { }, gain = { } }
        for type in pairs(globalConv) do
            -- Combined conversion (global + skill), divided by 100 → 0..1 fraction
            dmgTable.conversion[type] = (globalConv[type] + skillConv[type]) / 100
            -- Gain-as-extra fraction (not subject to 100% cap)
            dmgTable.gain[type]       = add[type] / 100
            -- Total multiplier for downstream: conv + gain
            dmgTable[type]            = (globalConv[type] + skillConv[type] + add[type]) / 100
        end
        -- mult: fraction of src that stays as itself (1 - total_conversion capped at 1)
        dmgTable.mult = 1 - m_min((globalTotal + skillTotal) / 100, 1)
        conversionTable[damageType] = dmgTable
    end
    -- Chaos is never converted, so only mult = 1 matters
    conversionTable["Chaos"] = { mult = 1 }
    return conversionTable
end
```

**Critical Lua semantics:**

- `for otherTypeIndex = damageTypeIndex + 1, 5 do` — iterates only over types **later**
  in the chain than `src`. Physical can convert to Lightning/Cold/Fire/Chaos; Lightning
  can convert to Cold/Fire/Chaos; Cold can convert to Fire/Chaos; Fire can convert only
  to Chaos. Chaos can never be converted. This is the forward-only constraint.
- `wipeTable(tempTable1)` — clears and returns the same table. The temp tables are
  module-level scratch. In Rust we just use local arrays per iteration (`[0.0; 5]`).
- `m_max(..., 0)` on globalConv / skillConv — clamps negative sums to zero. Negative
  conversion from "reduced conversion" mods must not propagate.
- `pairs(globalConv)` — iterates over all string keys that were set. Since `globalConv`
  was cleared at the start of the iteration and only keys `otherType` values were set,
  this is equivalent to iterating over all valid destination types.
- `isElemental[damageType] and "ElementalDamageConvertTo"..otherType or nil` — classic
  Lua `and/or` ternary. If `isElemental[damageType]` is truthy (non-nil, non-false),
  evaluates to the string; otherwise evaluates to `nil`. PoB's `:Sum("BASE", cfg, ...)` 
  accepts `nil` arguments and skips them. In Rust, this becomes a conditional push of
  extra stat names into the query.
- `dmgTable.conversion`, `dmgTable.gain`, `dmgTable[type]` — three parallel sub-tables.
  `conversion` = pure conversion fractions; `gain` = gain-as-extra fractions; the direct
  numeric key (`dmgTable[otherType]`) = **combined** total (conv + gain). The combined
  value is what `calcDamage()` uses as `convMult`. See gotcha §3 below.
- `dmgTable.mult` — the "self-retention" multiplier: how much of the source damage
  remains as the original type after conversion. Used by `calcAilmentSourceDamage()`.

---

### 3. `calcDamage()` using the conversion table (lines 68–139)

```lua
local function calcDamage(activeSkill, output, cfg, breakdown, damageType, typeFlags, convDst)
    -- ...
    local conversionTable = (cfg and cfg.conversionTable) or activeSkill.conversionTable
    -- Walk all types before damageType in the sequence (forward-only)
    for _, otherType in ipairs(dmgTypeList) do
        if otherType == damageType then break end     -- stop at self
        local convMult = conversionTable[otherType][damageType]  -- combined frac
        if convMult > 0 then
            -- Raise Spectre special case: physical-to-non-chaos conversion gets a bonus
            local convPortion = conversionTable[otherType].conversion[damageType]
            if convPortion > 0
               and cfg.summonSkillName == "Raise Spectre"
               and otherType == "Physical"
               and damageType ~= "Chaos"
            then
                local physBonus = 1 + data.monsterPhysConversionMultiTable[activeSkill.actor.level] / 100
                convMult = (convMult - convPortion) + convPortion * physBonus
            end
            -- Recursively compute the source type damage
            local min, max = calcDamage(activeSkill, output, cfg, breakdown, otherType, typeFlags, damageType)
            addMin = addMin + min * convMult
            addMax = addMax + max * convMult
        end
    end
    if addMin ~= 0 and addMax ~= 0 then
        addMin = round(addMin)    -- PoB rounds converted contributions
        addMax = round(addMax)
    end
    -- ...
    return round(((baseMin * inc * more) * genericMoreMinDamage + addMin) * moreMinDamage),
           round(((baseMax * inc * more) * genericMoreMaxDamage + addMax) * moreMaxDamage)
end
```

**Key patterns:**
- `conversionTable[otherType][damageType]` — this is the **combined** `dmgTable[type]`
  value (conv + gain), not `dmgTable.conversion[type]`. Gain-as-extra counts as a full
  multiplier here; the distinction between `conversion` and `gain` sub-tables is only
  used for breakdown display strings.
- **Recursion:** `calcDamage` is recursive. When computing Fire damage, it calls itself
  for Physical, Lightning, and Cold to accumulate their converted contributions. The
  base case terminates when `baseMin == 0 && baseMax == 0` and there are no converted
  contributions.
- **`round()` on converted amounts:** both `addMin` and `addMax` are rounded via PoB's
  `round()` (standard rounding) before being added to the base damage. This intermediate
  rounding step is absent in the current Rust `apply_conversion` function.
- **Raise Spectre physBonus:** only applies to the `conversion` portion (not the `gain`
  portion). The math is `convMult = (convMult - convPortion) + convPortion * physBonus`,
  i.e. the gain-as-extra part keeps its original value while the pure-conversion part is
  scaled by the monster level table.

---

### 4. `calcAilmentSourceDamage()` using `dmgTable.mult` (lines 141–152)

```lua
local function calcAilmentSourceDamage(activeSkill, output, cfg, breakdown, damageType, typeFlags)
    local min, max = calcDamage(activeSkill, output, cfg, breakdown, damageType, typeFlags)
    local conversionTable = (cfg and cfg.conversionTable) or activeSkill.conversionTable
    local convMult = conversionTable[damageType].mult
    -- ...
    return min * convMult, max * convMult
end
```

**`dmgTable.mult`:** the portion of damage that is NOT converted away. A fully-converted
type has `mult = 0`. This is used when computing ailment damage: only the portion of
damage that remains as the original type contributes to ailments that care about that type
(e.g. bleed requires physical damage to stay as physical).

---

### 5. Random-physical pre-processing (lines 929–986)

**Before** `buildConversionTable` is called, PoB expands random-physical mods:

```lua
for _, cfg in ipairs({ skillCfg, weapon1Cfg, weapon2Cfg }) do
    if cfg and skillModList:Sum("BASE", cfg, "PhysicalDamageGainAsRandom",
                                "PhysicalDamageConvertToRandom",
                                "PhysicalDamageGainAsColdOrLightning") > 0 then
        skillFlags.randomPhys = true
        -- PhysicalDamageGainAsRandom → splits equally into Fire/Cold/Lightning
        -- (or all into one type, depending on physMode config)
        -- PhysicalDamageConvertToRandom → same but conversion
        -- PhysicalDamageGainAsColdOrLightning → splits into Cold + Lightning
    end
end
```

This happens **before** the conversion table is built, so by the time
`buildConversionTable` runs, the random mods have been expanded into concrete
`PhysicalDamageConvertToFire` / `PhysicalDamageGainAsCold` etc. mods via
`skillModList:NewMod()`. The Rust equivalent must do this expansion step before calling
`build_conversion_table`.

The `physMode` comes from `env.configInput.physMode` — it's a player UI config:
- `"AVERAGE"` (default): split equally (div by 3 for random, div by 2 for cold-or-lightning)
- `"FIRE"` / `"COLD"` / `"LIGHTNING"`: all random phys goes to that element

---

### 6. Per-pass `convMult` application (lines 3140–3219)

```lua
for _, damageType in ipairs(dmgTypeList) do
    -- ...
    damageTypeHitMin, damageTypeHitMax = calcDamage(activeSkill, output, cfg, ...)
    local conversionTable = (cfg and cfg.conversionTable) or activeSkill.conversionTable
    local convMult = conversionTable[damageType].mult
    -- allMult bundles: convMult * ScaledDamageEffect * ruthless * fistofwar * warcry
    output.allMult = convMult * output.ScaledDamageEffect * output.RuthlessBlowHitEffect
                             * output.FistOfWarDamageEffect * globalOutput.OffensiveWarcryEffect
    local allMult = output.allMult
    if pass == 1 then
        allMult = allMult * output.CritMultiplier  -- crit pass multiplies by crit multi
    end
    damageTypeHitMin = damageTypeHitMin * allMult
    damageTypeHitMax = damageTypeHitMax * allMult
end
```

`output.allMult` is the combined scalar applied to each damage type's computed hit range.
It is written per-type per-pass into `output` (scoped to the pass), not to a persistent
output field visible in the oracle JSON.

---

## Existing Rust Code

### `crates/pob-calc/src/calc/offence_utils.rs` (lines 64–124)

**What exists:**
- `build_conversion_table()` builds a `ConversionTable` struct with `base[src][dst]`
  (conversion fractions) and `extra[src][dst]` (gain-as-extra fractions).
- The 100% global cap is implemented correctly for the single-tier case.
- Unit tests cover identity, 50% phys→lightning, 120% overcap (proportional scaling),
  and gain-as-extra.

**What's missing / wrong:**

1. **Two-tier capping (skill vs global) is absent.** Lua has a two-tier priority system:
   - Tier 1: `SkillXDamageConvertToY` mods (from the active skill itself)
   - Tier 2: `XDamageConvertToY` mods (from passive tree, gear, support gems)
   
   When skill-only conversion exceeds 100%, global conversion is zeroed. When combined
   exceeds 100%, only global is scaled down. The Rust code applies a single combined cap
   without distinguishing skill vs global sources. This will produce wrong values for
   skills like Incinerate, Glacial Hammer, etc. that have intrinsic skill-level
   conversion.

2. **`ElementalDamageConvertTo{Dst}` and `NonChaosDamageConvertTo{Dst}` stat names are
   not queried.** Lua (line 1860) queries three stat names per src→dst pair when src is
   elemental: `{Src}DamageConvertTo{Dst}`, `ElementalDamageConvertTo{Dst}`,
   `NonChaosDamageConvertTo{Dst}`. The current Rust only queries `{Src}DamageConvertTo{Dst}`.
   Same omission for gain-as-extra (line 1864).

3. **`Skill{Src}DamageConvertTo{Dst}` stat names are not queried.** These are the
   skill-tier conversion mods required for the two-tier capping logic.

4. **Random-physical pre-processing is absent.** `PhysicalDamageConvertToRandom`,
   `PhysicalDamageGainAsRandom`, and `PhysicalDamageGainAsColdOrLightning` are not
   expanded before conversion table construction.

5. **`CalcAilmentSourceDamage` / `dmgTable.mult` equivalent is not computed.** The
   `ConversionTable` struct does not expose a per-source `mult` field (the "self-retention"
   fraction). This is needed by ailment calculations (OFF-05 depends on this).

### `crates/pob-calc/src/calc/offence.rs` (lines 267–342)

**What exists:**
- `apply_conversion()` takes `base[src][dst]` + `extra[src][dst]` and sums contributions
  for each destination type. This approximates the Lua's `calcDamage()` recursion but
  differs in that it does a single-pass matrix multiply rather than a recursive traversal.
  For a chain like Phys → Lightning → Cold → Fire, the single-pass approach is correct
  **if** the base arrays have already been resolved. For cross-type chains (e.g., 50%
  phys→lightning + 50% lightning→cold), the Rust single-pass produces the wrong answer:
  it allocates `0.5 * physBase` to lightning and `0.5 * lightningBase` to cold, but does
  not also route `0.5 * physBase * 0.5` to cold via the phys→lightning→cold chain. The
  recursive Lua calcDamage handles this correctly.
- No rounding of intermediate converted amounts (`round(addMin)` step missing).
- No `convMult` (self-retention) tracking.

## What Needs to Change

1. **Two-tier capping in `build_conversion_table`:**
   - Query `Skill{Src}DamageConvertTo{Dst}` separately from global conversion mods.
   - Implement: if `skillTotal > 100` → scale skill down to 100, zero global; else if
     `globalTotal + skillTotal > 100` → scale global by `(100 - skillTotal) / globalTotal`.

2. **Extended stat name queries:**
   - For elemental sources (Fire/Cold/Lightning): also sum `ElementalDamageConvertTo{Dst}`
     and `ElementalDamageGainAs{Dst}`.
   - For non-Chaos sources (all except Chaos): also sum `NonChaosDamageConvertTo{Dst}`
     and `NonChaosDamageGainAs{Dst}`.
   - Apply `m_max(..., 0)` clamp (already done) to the combined sum.

3. **Add `mult` field to `ConversionTable` or compute it separately:**
   - `mult[src] = 1 - min((globalTotal + skillTotal) / 100, 1)` for Physical/Lightning/
     Cold/Fire. Chaos always has `mult = 1`.
   - Expose via `ConversionTable` struct for use in ailment calculations.

4. **Fix `apply_conversion` to match Lua's recursive `calcDamage`:**
   - The current single-pass matrix multiply is wrong for multi-hop chains. Replace with
     a recursive or iterative topological accumulation that walks the dmgTypeList in order
     and accumulates converted contributions from earlier types first.
   - Add the intermediate `round()` step: after summing all converted contributions for a
     type, round them before adding to the base.

5. **Random-physical pre-processing (before `build_conversion_table` call):**
   - Before calling `build_conversion_table`, check for `PhysicalDamageConvertToRandom`,
     `PhysicalDamageGainAsRandom`, `PhysicalDamageGainAsColdOrLightning` mods.
   - Split them into concrete mods based on `env.config_input.phys_mode` (AVERAGE /
     FIRE / COLD / LIGHTNING).
   - This must be done by injecting temporary mods into the skill mod list before table
     construction.

6. **Raise Spectre `physBonus` in `apply_conversion` / recursive damage calc:**
   - When `cfg.summonSkillName == "Raise Spectre"` and converting Physical → non-Chaos:
     apply `physBonus = 1 + data.monsterPhysConversionMultiTable[level] / 100` to the
     pure conversion portion (not the gain-as-extra portion).
   - `monsterPhysConversionMultiTable`: 100-element table indexed 1–100 by actor level,
     values 0 at low levels rising to 300 at high levels (see `Data/Misc.lua:15`).
   - This is an edge case (only affects Spectre builds) but required for full parity.
