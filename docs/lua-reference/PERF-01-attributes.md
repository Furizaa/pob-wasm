# PERF-01-attributes: Attributes (Str / Dex / Int / Omni) and Item Requirements

## Output Fields

| Field | Type | Notes |
|-------|------|-------|
| `Str` | number | Final Strength value (clamped ≥ 0) |
| `Dex` | number | Final Dexterity value (clamped ≥ 0) |
| `Int` | number | Final Intelligence value (clamped ≥ 0) |
| `Omni` | number | Omniscience value (only non-zero when Omniscience keystone is active) |
| `ReqStr` | number | Highest Strength requirement across all items/gems |
| `ReqDex` | number | Highest Dexterity requirement across all items/gems |
| `ReqInt` | number | Highest Intelligence requirement across all items/gems |
| `ReqStrString` | number or string | Display value for Str req (coloured red when unmet if breakdown active) |
| `ReqDexString` | number or string | Display value for Dex req |
| `ReqIntString` | number or string | Display value for Int req |
| `ReqStrItem` | source table | Item/gem that drives the Str requirement |
| `ReqDexItem` | source table | Item/gem that drives the Dex requirement |
| `ReqIntItem` | source table | Item/gem that drives the Int requirement |

## Dependencies

- **SETUP-01** through **SETUP-04**: ModDb must be populated (passives, items, gems) before attributes can be summed.
- No other PERF chunks are required first — this is the first output-writing chunk in Tier 1.

## Lua Source

File: `third-party/PathOfBuilding/src/Modules/CalcPerform.lua`
Commit: `454eff8c85d24356d9b051d596983745ed367476`

Relevant line ranges:
- Lines 380–517: `calculateAttributes` and `calculateOmniscience` inner functions + invocation
- Lines 1924–1987: item/gem attribute requirement loop

## Annotated Lua

### Math / table aliases (file top, lines 8–18)

```lua
local m_min = math.min    -- → x.min(y)
local m_max = math.max    -- → x.max(y)
local m_floor = math.floor -- → x.floor()
local round = ...          -- PoB's global round() = x.round()
```

`calcLib.val(modDB, name)` (CalcTools.lua:32–39):
```lua
function calcLib.val(modStore, name, cfg)
    local baseVal = modStore:Sum("BASE", cfg, name)
    if baseVal ~= 0 then
        return baseVal * calcLib.mod(modStore, cfg, name)
        -- calcLib.mod = (1 + Sum("INC",...)/100) * More(...)
    else
        return 0
    end
end
```
**Rust equivalent**: `calc_val(mod_db, output, name)` in `calc_tools.rs`.
Key gotcha: if `baseVal == 0`, the whole expression short-circuits to `0` — no INC/MORE applied.

---

### `calculateAttributes` (lines 381–408)

```lua
local calculateAttributes = function()
    -- TWO-PASS LOOP: needed because some INC mods on Str/Dex/Int are conditioned on
    -- "StrHigherThanDex" etc. which themselves depend on the Str/Dex/Int values.
    -- Pass 1 computes raw values; conditions are set; pass 2 recomputes with updated conditions.
    for pass = 1, 2 do
        for _, stat in pairs({"Str","Dex","Int"}) do
            -- calcLib.val = BASE * (1 + INC/100) * MORE, floored by round()
            output[stat] = m_max(round(calcLib.val(modDB, stat)), 0)
            --             ^^^^^^ clamp to non-negative; Lua's or-0 pattern absent here
            --                    round() here is standard rounding, not floor
        end

        -- Sort for LowestAttribute (not an output field in PERF-01 but affects conditions)
        local stats = { output.Str, output.Dex, output.Int }
        table.sort(stats)          -- ascending; stats[1] is lowest, stats[3] is highest
        output.LowestAttribute = stats[1]
        condList["TwoHighestAttributesEqual"] = stats[2] == stats[3]

        -- Comparison conditions (used by mods like "if Dex higher than Int")
        condList["DexHigherThanInt"] = output.Dex > output.Int
        condList["StrHigherThanInt"] = output.Str > output.Int
        condList["IntHigherThanDex"] = output.Int > output.Dex
        condList["StrHigherThanDex"] = output.Str > output.Dex
        condList["IntHigherThanStr"] = output.Int > output.Str
        condList["DexHigherThanStr"] = output.Dex > output.Str

        -- "Highest" conditions use >= (tie-breaks: both can be "highest")
        condList["StrHighestAttribute"] = output.Str >= output.Dex and output.Str >= output.Int
        condList["IntHighestAttribute"] = output.Int >= output.Str and output.Int >= output.Dex
        condList["DexHighestAttribute"] = output.Dex >= output.Str and output.Dex >= output.Int
        -- "SingleHighest" conditions use strict > (no ties)
        condList["IntSingleHighestAttribute"] = output.Int > output.Str and output.Int > output.Dex
        condList["DexSingleHighestAttribute"] = output.Dex > output.Str and output.Dex > output.Int
    end
end
```

**Two-pass note**: The double loop exists because mods like `"10% increased Strength if Dex is higher than Int"` are `ConditionMod` entries in the modDB. On pass 1 the condition is initially false/stale; after it's set, pass 2 recomputes the stat value incorporating that mod. Without two passes, the circular dependency produces wrong values.

**Rust note**: The current `do_actor_attribs_conditions` in `perform.rs` does **not** implement the two-pass loop — it computes Str/Dex/Int only once. This is the primary correctness bug for this chunk.

---

### `calculateOmniscience` (lines 410–472)

```lua
local calculateOmniscience = function(convert)
    local classStats = env.spec.tree.characterData and env.spec.tree.characterData[env.classId]
                       or env.spec.tree.classes[env.classId]
    -- classStats["base_str"], classStats["base_dex"], classStats["base_int"]
    -- = the character class's base attribute values (e.g. Witch: 14 Str, 14 Dex, 40 Int)

    for pass = 1, 2 do
        if pass ~= 1 then
            -- Pass 2: cap each attribute at its class base value,
            -- then push excess into Omni mods
            for _, stat in pairs({"Str","Dex","Int"}) do
                local base = classStats["base_"..stat:lower()]
                output[stat] = m_min(round(calcLib.val(modDB, stat)), base)
                --                   ^^^^^ capped at class base, not unconstrained

                -- Convert the excess above base into Omni BASE mods
                modDB:NewMod("Omni", "BASE",
                    (modDB:Sum("BASE", nil, stat) - base),
                    stat.." conversion Omniscience")
                -- Also mirror INC and MORE from each attribute to Omni
                modDB:NewMod("Omni", "INC", modDB:Sum("INC", nil, stat), "Omniscience")
                modDB:NewMod("Omni", "MORE", modDB:Sum("MORE", nil, stat), "Omniscience")
            end
        end

        if pass ~= 2 then
            -- Pass 1 only: subtract double/triple dips to avoid over-counting
            -- combined-attribute nodes (StrDex, StrInt, DexInt, All) contribute to
            -- multiple attributes, so we reduce Omni by the overlap.
            local conversion = { }
            local reduction = { }
            for _, type in pairs({"BASE", "INC", "MORE"}) do
                conversion[type] = { }
                for _, stat in pairs({"StrDex", "StrInt", "DexInt", "All"}) do
                    conversion[type][stat] = modDB:Sum(type, nil, stat) or 0
                end
                -- Each dual-attribute node is counted twice; triple counted thrice → subtract excess
                reduction[type] = conversion[type].StrDex + conversion[type].StrInt
                               + conversion[type].DexInt + 2*conversion[type].All
            end
            modDB:NewMod("Omni", "BASE", -reduction["BASE"], "Reduction from Double/Triple Dipped attributes to Omniscience")
            modDB:NewMod("Omni", "INC",  -reduction["INC"],  "Reduction from Double/Triple Dipped attributes to Omniscience")
            modDB:NewMod("Omni", "MORE", -reduction["MORE"], "Reduction from Double/Triple Dipped attributes to Omniscience")
        end

        -- After conversions, each individual stat is just its class base
        for _, stat in pairs({"Str","Dex","Int"}) do
            local base = classStats["base_"..stat:lower()]
            output[stat] = base
        end

        -- Compute final Omni value
        output["Omni"] = m_max(round(calcLib.val(modDB, "Omni")), 0)

        -- Same comparison conditions as calculateAttributes (copy-paste in Lua)
        ...
    end
end
```

**Invocation** (lines 474–478):
```lua
if modDB:Flag(nil, "Omniscience") then
    calculateOmniscience()
else
    calculateAttributes()
end
```

**Rust note**: The current Rust code (`perform.rs:52–69`) implements Omniscience as a simple sum `Str + Dex + Int`, which is completely wrong. The Lua:
1. Caps each stat at its class base value (not zero).
2. Converts the excess into `Omni` BASE mods.
3. Mirrors all INC/MORE from each stat into Omni.
4. Subtracts overlapping combined-attribute mods (StrDex, StrInt, DexInt, All) to prevent double-counting.

---

### TotalAttr and attribute-derived bonuses (lines 480–514)

```lua
output.TotalAttr = output.Str + output.Dex + output.Int

-- Strength bonuses
if not modDB:Flag(nil, "NoAttributeBonuses") then
    if not modDB:Flag(nil, "NoStrengthAttributeBonuses") then
        if not modDB:Flag(nil, "NoStrBonusToLife") then
            modDB:NewMod("Life", "BASE", m_floor(output.Str / 2), "Strength")
            -- +1 Life per 2 Str (floor division)
        end
        local strDmgBonusRatioOverride = modDB:Sum("BASE", nil, "StrDmgBonusRatioOverride")
        if strDmgBonusRatioOverride > 0 then
            actor.strDmgBonus = m_floor((output.Str + modDB:Sum("BASE", nil, "DexIntToMeleeBonus")) * strDmgBonusRatioOverride)
        else
            actor.strDmgBonus = m_floor((output.Str + modDB:Sum("BASE", nil, "DexIntToMeleeBonus")) / 5)
            -- Default: +1% Inc Melee Phys per 5 Str (floor)
        end
        modDB:NewMod("PhysicalDamage", "INC", actor.strDmgBonus, "Strength", ModFlag.Melee)
    end

    -- Dexterity bonuses
    if not modDB:Flag(nil, "NoDexterityAttributeBonuses") then
        modDB:NewMod("Accuracy", "BASE",
            output.Dex * (modDB:Override(nil, "DexAccBonusOverride") or data.misc.AccuracyPerDexBase),
            "Dexterity")
        -- AccuracyPerDexBase = 2 (from Data.lua:166)
        -- DexAccBonusOverride can replace the 2 with a custom per-Dex accuracy rate
        if not modDB:Flag(nil, "NoDexBonusToEvasion") then
            modDB:NewMod("Evasion", "INC", m_floor(output.Dex / 5), "Dexterity")
            -- +1% Inc Evasion per 5 Dex (floor)
        end
    end

    -- Intelligence bonuses
    if not modDB:Flag(nil, "NoIntelligenceAttributeBonuses") then
        if not modDB:Flag(nil, "NoIntBonusToMana") then
            modDB:NewMod("Mana", "BASE", m_floor(output.Int / 2), "Intelligence")
            -- +1 Mana per 2 Int (floor)
        end
        if not modDB:Flag(nil, "NoIntBonusToES") then
            modDB:NewMod("EnergyShield", "INC", m_floor(output.Int / 10), "Intelligence")
            -- +1% Inc ES per 10 Int (floor) — NOTE: the Rust code uses /5 here (WRONG)
        end
    end
end
```

**Critical bug**: The Rust code at `perform.rs:192–204` computes `es_inc_from_int = (int_out / 5.0).floor()`.
The Lua uses `m_floor(output.Int / 10)` — **divisor is 10, not 5**.
This causes ES to be over-scaled by 2× whenever Int > 0.

**`DexIntToMeleeBonus` note**: Some keystones/items add to this mod to let Dex or Int count toward the melee damage bonus. The Rust code ignores this (`perform.rs:117–128`) and only uses `str_out / 5.0`.

**`StrDmgBonusRatioOverride` note**: Some unique items override the 1/5 ratio with a custom value. The Rust code does not handle this override.

**Accuracy per Dex note**: The Rust code (`perform.rs:133–141`) hard-codes `dex_out * 2.0`. While the default is `AccuracyPerDexBase = 2`, it should check `modDB:Override(nil, "DexAccBonusOverride")` first.

---

### Attribute requirements (lines 1924–1987)

```lua
do
    local reqMult = calcLib.mod(modDB, nil, "GlobalAttributeRequirements")
    -- reqMult = (1 + INC("GlobalAttributeRequirements")/100) * More("GlobalAttributeRequirements")
    -- Default is 1.0 (no mods). "40% reduced attribute requirements" → reqMult = 0.6

    local omniRequirements = modDB:Flag(nil, "OmniscienceRequirements")
                             and calcLib.mod(modDB, nil, "OmniAttributeRequirements")
    -- If OmniscienceRequirements flag is set, all three attr reqs are consolidated
    -- into Omni with: omniReqMult = 1 / (omniRequirements - 1)

    local ignoreAttrReq = modDB:Flag(nil, "IgnoreAttributeRequirements")
    -- If set, all requirements become 0 (e.g. Scion's Ascendancy node "Solaris Lorica")

    local attrTable = omniRequirements and {"Omni","Str","Dex","Int"} or {"Str","Dex","Int"}
    for _, attr in ipairs(attrTable) do
        local breakdownAttr = omniRequirements and "Omni" or attr
        -- When Omniscience is active, all reqs map to breakdownAttr = "Omni"

        local out = {val = 0, source = nil}
        for _, reqSource in ipairs(env.requirementsTable) do
            -- requirementsTable is built during setup: one entry per equipped item / active gem
            -- Each entry has reqSource.Str, reqSource.Dex, reqSource.Int (base requirement values)
            -- and reqSource.source ("Item" or "Gem"), reqSource.sourceItem, reqSource.sourceSlot, reqSource.sourceGem

            if reqSource[attr] and reqSource[attr] > 0 then
                local req = m_floor(reqSource[attr] * reqMult)
                if omniRequirements then
                    local omniReqMult = 1 / (omniRequirements - 1)
                    local attributereq = m_floor(reqSource[attr] * reqMult)
                    req = m_floor(attributereq * omniReqMult)
                end
                if req > out.val then
                    out.val = req
                    out.source = reqSource
                    -- Track the single highest requirement source
                end
            end
        end

        if ignoreAttrReq then
            out.val = 0
        end

        -- Write the initial "String" as 0; overwrite if this attr has a positive req
        output["Req"..attr.."String"] = 0
        if out.val > (output["Req"..breakdownAttr] or 0) then
            output["Req"..breakdownAttr.."String"] = out.val
            output["Req"..breakdownAttr] = out.val
            output["Req"..breakdownAttr.."Item"] = out.source
            -- In breakdown mode, the string gets colorCode.NEGATIVE prefix when unmet
            if breakdown then
                output["Req"..breakdownAttr.."String"] = out.val > (output[breakdownAttr] or 0)
                    and colorCodes.NEGATIVE..(out.val) or out.val
            end
        end
    end
end
```

**Key semantics**:
- `output["ReqStr"] = 0` is written only when `out.val > (output["ReqStr"] or 0)` — the field is
  only written when there IS a requirement. For builds with no requirements, these fields remain absent.
- `output["ReqStrString"] = 0` is always written as a pre-initialization (line 1978), then overwritten.
  The "String" variants exist so the UI can display coloured requirement text; in numeric oracle builds,
  these fields are the same numeric value as `ReqStr`.
- `output["ReqStrItem"]` is the `reqSource` table itself, pointing to the item/gem driving the req.
  **This is a reference to a Lua table** — it has no numeric equivalent in Rust oracle output.
  The oracle JSON likely serialises it as a string (item name) or omits it.

---

## Existing Rust Code

File: `crates/pob-calc/src/calc/perform.rs`, lines 33–288 (`do_actor_attribs_conditions`)

### What Exists

- Single-pass computation of Str/Dex/Int via `mod_db.sum_cfg` + `mod_db.more_cfg` separately.
- Omniscience path (lines 52–69): sets Omni = Str + Dex + Int with Str/Dex/Int zeroed out.
- Attribute comparison conditions (StrHigherThanDex, etc.) — all 6 pairs present.
- `StrHighestAttribute`, `IntHighestAttribute`, `DexHighestAttribute` conditions present.
- `TotalAttr` and `LowestAttribute` outputs written.
- Attribute bonus mods injected (Life from Str, Accuracy from Dex, etc.).

### What Is Missing

1. **Two-pass attribute calculation** — the entire `for pass = 1, 2 do` loop is absent. Circular
   conditions (`StrHigherThanDex` → affects INC Str → changes Str value) never converge.
2. **`IntSingleHighestAttribute` and `DexSingleHighestAttribute` conditions** — not set anywhere.
3. **`TwoHighestAttributesEqual` condition** — not set.
4. **`StrDmgBonusRatioOverride` handling** — the custom melee damage ratio override for unique items
   is not checked; always uses the default `/5` ratio.
5. **`DexIntToMeleeBonus` in Str damage calculation** — the bonus that lets other stats contribute to
   melee damage is not summed; Rust only uses raw `str_out`.
6. **`DexAccBonusOverride` check** — accuracy per Dex is hard-coded as `2.0`; does not check
   `modDB:Override(nil, "DexAccBonusOverride")`.
7. **Attribute requirements** (`ReqStr`, `ReqDex`, `ReqInt`, `ReqStrString`, etc.) — **completely
   absent** from perform.rs. This entire section (Lua lines 1924–1987) has no Rust counterpart.

### What Is Wrong

1. **ES bonus from Int uses wrong divisor** (`perform.rs:192`): `(int_out / 5.0).floor()` should be
   `(int_out / 10.0).floor()`. The Lua is `m_floor(output.Int / 10)`.
2. **Omniscience implementation is incorrect** (`perform.rs:52–69`): The Rust code sets Omni = Str +
   Dex + Int with each individual stat zeroed. The Lua actually:
   - Caps each stat at the character's class base value (not 0).
   - Transfers excess above the class base into Omni BASE mods (not a simple sum).
   - Mirrors INC and MORE mods from each stat into Omni.
   - Subtracts combined-attribute node contributions to prevent double-counting.
3. **`modDB.more_cfg` used directly for Str/Dex/Int** (`perform.rs:39–50`): The Rust code manually
   computes `base * (1 + inc/100) * more`, whereas the Lua uses `calcLib.val(modDB, stat)` which
   short-circuits to `0` if `BASE == 0`. If no BASE mods exist for a stat, the Lua returns `0`
   without applying INC/MORE; the Rust code would return `0 * ... = 0` anyway, so this is likely
   equivalent in practice, but the intent differs.

---

## What Needs to Change

1. **Fix ES bonus from Int** (`perform.rs:192`):  
   Change `(int_out / 5.0).floor()` to `(int_out / 10.0).floor()`.

2. **Implement two-pass attribute loop**:  
   Wrap the Str/Dex/Int calculation + condition-setting in a `for pass in 0..2` loop. On each
   pass, recompute Str/Dex/Int from the modDB and update all 8 comparison conditions, so that
   circular `ConditionMod` entries converge correctly.

3. **Add missing conditions**:  
   After the two-pass loop, set `IntSingleHighestAttribute`, `DexSingleHighestAttribute`, and
   `TwoHighestAttributesEqual` (requires sorting the three values).

4. **Fix Omniscience path**:  
   Implement the real Omniscience algorithm:
   - Read `classStats` for the character's base Str/Dex/Int.
   - Pass 1: subtract double/triple-dip overlap from combined-attribute nodes.
   - Pass 2: cap each stat at its class base; push excess into Omni BASE mods; mirror INC/MORE.
   - Set each individual stat output to its class base value (not 0).
   - Compute final Omni via `calc_val`.

5. **Add `DexIntToMeleeBonus` to Str damage calculation**:  
   Change melee damage bonus computation to:
   ```rust
   let dex_int_bonus = env.player.mod_db.sum_cfg(ModType::Base, "DexIntToMeleeBonus", None, o);
   let str_dmg_bonus_override = env.player.mod_db.sum_cfg(ModType::Base, "StrDmgBonusRatioOverride", None, o);
   let str_dmg_bonus = if str_dmg_bonus_override > 0.0 {
       ((str_out + dex_int_bonus) * str_dmg_bonus_override).floor()
   } else {
       ((str_out + dex_int_bonus) / 5.0).floor()
   };
   ```

6. **Add `DexAccBonusOverride` to accuracy calculation**:  
   Before hard-coding `2.0`, check `mod_db.override_value(None, output, "DexAccBonusOverride")`.

7. **Implement attribute requirements** (new function `do_attr_requirements`):  
   Iterate `env.requirements_table` (needs to exist in `CalcEnv`), compute the max requirement
   per attribute after applying `reqMult`, handle `IgnoreAttributeRequirements` and
   `OmniscienceRequirements` flags, and write `ReqStr`, `ReqDex`, `ReqInt`,
   `ReqStrString`, `ReqDexString`, `ReqIntString`, `ReqStrItem`, `ReqDexItem`, `ReqIntItem`.
   Note: `ReqXxxItem` is a struct reference in Rust, not a Lua table — use the item/gem name
   string for oracle comparison if the oracle stores it as a string.
