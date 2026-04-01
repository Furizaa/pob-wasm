# PERF-05-buffs: Buffs & Debuffs (Fortify, Totems, Mines, Traps, Brands, Golems, Warcry)

## Field Name Discrepancy Warning

`field_groups.rs` lists `FortifyStacks` and `FortifyEffect`, but the actual oracle JSON files
and the Lua source use `FortificationStacks` and `FortificationEffect`. Similarly, `BannerStage`
does not appear in any oracle JSON and is not written as an output field in any Lua module.
`ActiveBrandLimit` appears in oracle JSONs only for brand skills (CalcOffence.lua:1412).

**The actual oracle-verified field names for this chunk are:**

| field_groups.rs name | Actual oracle name | Lua source |
|---------------------|-------------------|-----------|
| `FortifyStacks` | `FortificationStacks` | CalcPerform.lua:636 |
| `FortifyEffect` | `FortificationEffect` | CalcPerform.lua:639,641 |
| `AilmentWarcryEffect` | `AilmentWarcryEffect` | CalcOffence.lua:2503,2712,2717 |
| `ActiveTotemLimit` | `ActiveTotemLimit` | CalcPerform.lua:1259; CalcOffence.lua:1383 |
| `ActiveMineLimit` | `ActiveMineLimit` | CalcOffence.lua:525 |
| `ActiveTrapLimit` | `ActiveTrapLimit` | CalcOffence.lua:524 |
| `ActiveBrandLimit` | `ActiveBrandLimit` | CalcOffence.lua:1412 |
| `ActiveGolemLimit` | `ActiveGolemLimit` | not written to output; it's a modDB stat only |
| `BannerStage` | *(absent from oracle)* | not written as an output field |

`ActiveGolemLimit` is a modDB stat name (used as a `PerStat` tag multiplier), not an
`output.X` write. It does not appear as a top-level key in any oracle JSON — builds that have
golems show `ActiveGolemLimit` as a **number value directly in the output** only if the Lua
writes it. Searching all Lua modules confirms no `output.ActiveGolemLimit = ...` assignment
exists; golem limit only exists as a mod-database stat driving `PerStat` tags.

## Output Fields

| Field | Oracle name | Module | Lines |
|-------|-------------|--------|-------|
| `FortificationStacks` | same | CalcPerform.lua | 636 |
| `FortificationEffect` | same | CalcPerform.lua | 639, 641 |
| `AilmentWarcryEffect` | same | CalcOffence.lua | 2503, 2712, 2717 |
| `ActiveTotemLimit` | same | CalcPerform.lua:1259 + CalcOffence.lua:1383 | two writes |
| `ActiveMineLimit` | same | CalcOffence.lua | 525 |
| `ActiveTrapLimit` | same | CalcOffence.lua | 524 |
| `ActiveBrandLimit` | same | CalcOffence.lua | 1412 |

`ActiveGolemLimit` and `BannerStage` should be **removed from field_groups.rs** for this
chunk as they are not oracle-verified output fields.

## Dependencies

- **PERF-01-attributes**: Not required directly.
- **PERF-02-life-mana-es**: Not required directly.
- **PERF-03-charges**: Not required.
- **SETUP-01 through SETUP-04**: ModDB must be fully populated; Fortify flag comes from
  keystone/ascendancy; skill flags (totem, brand, mine, trap) come from active skill setup.
- CalcOffence.lua fields (`ActiveTrapLimit`, `ActiveMineLimit`, `ActiveBrandLimit`,
  `ActiveTotemLimit`, `AilmentWarcryEffect`) are computed inside the offence pass
  (`calcs.buildActiveSkill`), which runs after the perform pass. These are technically
  CalcOffence-pass fields that happen to be tracked here because they describe "how
  many actives can be on the field" — they depend on skill setup being correct.

## Lua Source

**File 1:** `third-party/PathOfBuilding/src/Modules/CalcPerform.lua`  
**Commit:** `454eff8c85d24356d9b051d596983745ed367476`  
Lines: 604–915 (`doActorMisc`), with Fortify at 625–648 and ActiveTotemLimit at 1257–1262

**File 2:** `third-party/PathOfBuilding/src/Modules/CalcOffence.lua`  
**Commit:** `454eff8c85d24356d9b051d596983745ed367476`  
Lines: 523–525 (`ActiveTrapLimit`, `ActiveMineLimit`), 1383 (`ActiveTotemLimit`),
1409–1412 (`ActiveBrandLimit`), 2498–2503 + 2711–2718 (`AilmentWarcryEffect`)

---

## Annotated Lua

### Math aliases used in this chunk

```lua
local m_min   = math.min
local m_max   = math.max
local m_floor = math.floor
```

---

### Fortification (CalcPerform.lua:620–648) — `FortificationStacks`, `FortificationEffect`

```lua
local function doActorMisc(env, actor)
    -- (only runs when env.mode_combat is true — the "effective DPS" mode)
    if env.mode_combat then
        local alliedFortify =
            modDB:Flag(nil, "YourFortifyEqualToParent")
                and actor.parent.output.FortificationStacks
            or env.partyMembers
                and env.partyMembers.modDB:Flag(nil, "PartyMemberFortifyEqualToYours")
                and env.partyMembers.output.FortificationStacks
            or 0
        -- alliedFortify: inherit fortification stacks from a parent actor (spectres)
        -- or from a party member who mirrors their stacks. Defaults 0 when no allies.
        -- Rust: env.party_members is not modelled → alliedFortify = 0 currently.

        -- Set Fortified condition from minimum Fortification mods or allied fortify
        if modDB:Sum("BASE", nil, "MinimumFortification") > 0 or alliedFortify > 0 then
            condList["Fortified"] = true
        end

        -- Fortify block: runs when Fortified condition is set OR Multiplier:Fortification > 0
        if modDB:Flag(nil, "Fortified") or modDB:Sum("BASE", nil, "Multiplier:Fortification") > 0 then
            local skillModList = actor.mainSkill and actor.mainSkill.skillModList or actor.modDB
            local skillCfg    = actor.mainSkill and actor.mainSkill.skillCfg

            local maxStacks = m_max(
                modDB:Override(nil, "MaximumFortification") or modDB:Sum("BASE", skillCfg, "MaximumFortification"),
                alliedFortify)
            -- maxStacks: override first; else sum BASE mods for MaximumFortification
            -- (skill-scoped: armour with "+N to maximum Fortification" contributes here)
            -- m_max with alliedFortify: if ally has more stacks, take that

            local minStacks = m_min(
                modDB:Flag(nil, "Condition:HaveMaxFortification") and maxStacks
                    or modDB:Sum("BASE", nil, "MinimumFortification"),
                maxStacks)
            -- minStacks: if "HaveMaxFortification" condition → min = max (all stacks)
            -- else: sum BASE MinimumFortification mods (King Maker spectre, etc.)
            -- capped at maxStacks

            local stacks = m_min(
                modDB:Override(nil, "FortificationStacks") or (minStacks > 0 and minStacks) or maxStacks,
                maxStacks)
            -- stacks priority:
            --   1. Override (user config "Set fortification stacks to N")
            --   2. minStacks if > 0 (forced minimum)
            --   3. maxStacks (default: assume full stacks)
            -- Then cap at maxStacks.
            -- Lua `a and b or c` gotcha: (minStacks > 0 and minStacks) — safe since minStacks
            -- is always a positive number when true.

            output.MaximumFortification   = maxStacks
            output.MinimumFortification   = minStacks
            output.RemovableFortification = m_min(
                maxStacks - minStacks,
                (modDB:Override(nil, "FortificationStacks") or maxStacks) - minStacks)
            output.FortificationStacks    = stacks       -- ← PERF-05 tracked field
            output.FortificationStacksOver20 = m_min(m_max(0, stacks - 20), maxStacks - 20)
            output.FortifyDuration = (skillModList:Override(nil, "FortifyDuration") or data.misc.FortifyBaseDuration)
                                   * (1 + increasedDuration / 100)

            output.FortificationEffect = "0"  -- initialised as STRING "0"!
            -- ← PERF-05 tracked field.
            -- Written as the string "0" first to support the Willowgift unique (which
            -- can show "0" mitigation text). Only overwritten if mitigation applies.
            -- Rust: output must store this as a number 0 or string "0"? Oracle JSON shows
            -- FortificationEffect as a number (e.g. 20 or absent). Check oracle to confirm
            -- whether this string initialisation matters.

            if not modDB:Flag(nil, "Condition:NoFortificationMitigation") then
                output.FortificationEffect = stacks
                -- FortificationEffect = the stack count itself (used as % damage reduction).
                -- Only written (as a number) if NoFortificationMitigation is NOT set.
                modDB:NewMod("DamageTakenWhenHit", "MORE", -stacks, "Fortification")
                -- Injects the actual damage reduction into modDB for later defence calc.
            end

            if stacks >= maxStacks then
                modDB:NewMod("Condition:HaveMaximumFortification", "FLAG", true, "")
            end
            modDB.multipliers["BuffOnSelf"] = (modDB.multipliers["BuffOnSelf"] or 0) + 1
        end
    end -- end mode_combat
```

**Key facts:**
- `FortificationStacks` (not `FortifyStacks`) is the oracle field name.
- `FortificationEffect` starts as the string `"0"`, not the number `0`. Oracle JSON shows it
  as a number when non-zero. The Rust code must handle the mixed type correctly.
- Default stack count is `maxStacks` (full stacks assumed), not 0.
- Maxstacks default is **not** hard-coded at 20; it is `modDB:Sum("BASE", skillCfg, "MaximumFortification")`.
  Items with "+3 to maximum Fortification" contribute here. The Rust code hard-codes 20.0 as
  a fallback — but in the Lua there is no fallback; if no mods exist, maxStacks = 0 and the
  entire Fortify block produces 0 output fields.

---

### `ActiveTotemLimit` (CalcPerform.lua:1257–1262 + CalcOffence.lua:1383)

CalcPerform writes a preliminary value during the main calculation pass:
```lua
-- Inside: for _, activeSkill in ipairs(env.player.activeSkillList) do
if activeSkill.skillFlags.totem then
    local limit = env.player.mainSkill.skillModList:Sum(
        "BASE", env.player.mainSkill.skillCfg, "ActiveTotemLimit", "ActiveBallistaLimit")
    output.ActiveTotemLimit = m_max(limit, output.ActiveTotemLimit or 0)
    -- ← Takes maximum across all totem skills (handles multiple totem skills)
    -- "or 0": nil-coalesces the first time (output.ActiveTotemLimit may be nil on first pass)
    output.TotemsSummoned = modDB:Override(nil, "TotemsSummoned") or output.ActiveTotemLimit
    enemyDB.multipliers["TotemsSummoned"] = m_max(output.TotemsSummoned or 0,
                                                   enemyDB.multipliers["TotemsSummoned"] or 0)
end
```

CalcOffence then overwrites it for the active skill:
```lua
-- Inside: buildActiveSkill → skill type check
output.ActiveTotemLimit = skillModList:Sum("BASE", skillCfg, "ActiveTotemLimit", "ActiveBallistaLimit")
output.TotemsSummoned   = env.modDB:Override(nil, "TotemsSummoned") or output.ActiveTotemLimit
```

**Two-source note:** The CalcPerform write sets the global player output (maximum across all
totem skills); CalcOffence re-writes it on the active-skill's output specifically. The oracle
JSON reflects the CalcOffence value (active skill's limit). For builds without totems, this
field is absent from oracle JSON entirely.

---

### `ActiveTrapLimit`, `ActiveMineLimit` (CalcOffence.lua:523–525)

```lua
-- Inside buildActiveSkill, after skill type setup:
output.ActiveTrapLimit = skillModList:Sum("BASE", skillCfg, "ActiveTrapLimit")
output.ActiveMineLimit = skillModList:Sum("BASE", skillCfg, "ActiveMineLimit")
-- Simple: sum BASE mods for the limit, skill-scoped.
-- Default: no mods → 0 (but game has base values in skill data; these appear as BASE mods).
-- Typical value: 15 for both (3 traps + supports, or 15 mines base).
-- Present in oracle JSON only when the skill has trap/mine flags.
```

---

### `ActiveBrandLimit` (CalcOffence.lua:1409–1412)

```lua
if activeSkill.skillTypes[SkillType.Brand] then
    output.BrandAttachmentRange = data.misc.BrandAttachmentRangeBase
        * calcLib.mod(skillModList, skillCfg, "BrandAttachmentRange")
    output.BrandAttachmentRangeMetre = output.BrandAttachmentRange / 10
    output.ActiveBrandLimit = skillModList:Sum("BASE", skillCfg, "ActiveBrandLimit")
    -- Only written for brand skills. For non-brand builds, absent from oracle.
end
```

---

### `AilmentWarcryEffect` (CalcOffence.lua:2498–2718)

```lua
-- Inside the damage-calculation pass (buildActiveSkill), per skill:
globalOutput.AilmentWarcryEffect = 1   -- line 2503: always initialised to 1

-- ... (warcry processing loop) ...

if activeSkill.skillModList:Flag(nil, "Condition:WarcryMaxHit") then
    -- "WarcryMaxHit": skill uses max hit calculation (e.g. Seismic Cry)
    globalOutput.AilmentWarcryEffect = globalOutput.MaxOffensiveWarcryEffect
else
    globalOutput.AilmentWarcryEffect = globalOutput.OffensiveWarcryEffect
end
-- AilmentWarcryEffect = the warcry damage multiplier that applies to ailments.
-- Used in bleed, poison, and ignite DPS formulas as a scaling factor.
-- When no warcry is active → both OffensiveWarcryEffect and MaxOffensiveWarcryEffect = 1
-- → AilmentWarcryEffect = 1 (no scaling).
-- globalOutput is env.player.output (the outer output table, not the skill-local output).
```

**Execution context:** `AilmentWarcryEffect` is always 1 for non-warcry-using builds.
The initialisation at line 2503 runs unconditionally at the start of each damage pass,
so it always appears in the oracle JSON with value 1 even when no warcry is active.

---

## Existing Rust Code

File: `crates/pob-calc/src/calc/perform.rs`  
Lines: 676–818 (`do_actor_misc`)

### What Exists

- **Fortify check** (`perform.rs:684–708`): checks the `Fortified` flag, reads
  `MaximumFortification` BASE sum, defaults to 20.0 if 0, writes `FortifyStacks` to output,
  sets `FortifyStack` multiplier, sets `Fortified` condition.

- **Onslaught**, **Tailwind**, **Elusive**, **Rage** processing (lines 710–818): these inject
  mods into modDB but do not write PERF-05 tracked fields.

### What Is Wrong

1. **Wrong output field name: `FortifyStacks` instead of `FortificationStacks`**  
   `perform.rs:705` writes to `"FortifyStacks"`, but the oracle JSON and Lua use
   `"FortificationStacks"`. This means the field never matches oracle expected values.

2. **Missing `FortificationEffect` output**  
   The Rust code writes `"FortifyStacks"` (wrong name) and `FortifyStack` multiplier, but
   never writes `"FortificationEffect"`. In the Lua this is `output.FortificationEffect = stacks`
   (or `"0"` string when `NoFortificationMitigation` is set).

3. **Wrong fallback for `MaximumFortification`**  
   `perform.rs:700–703` uses 20.0 as a fallback. The Lua has no such fallback — if no
   `MaximumFortification` BASE mods exist, `maxStacks = 0` and the entire block produces no
   output. Fortify stacks are only non-zero when the build actually has the Fortified condition
   AND some source of maximum Fortification (from keystone/ascendancy/items).

4. **Default stack count wrong**  
   `perform.rs:705` writes `max_fort` (the maximum). Lua also defaults to `maxStacks` via
   `(minStacks > 0 and minStacks) or maxStacks`. However the path through Override is missing:
   the Lua first checks `modDB:Override(nil, "FortificationStacks")`, which lets users set a
   specific stack count in the config UI. The Rust code does not check this override.

5. **`MinimumFortification` logic missing**  
   The Lua computes `minStacks` from `MinimumFortification` BASE mods and the
   `HaveMaxFortification` condition. Rust does not compute this at all, meaning builds with
   spectres like King Maker ("allies have minimum 20 Fortification") won't reflect the
   minimum enforcement.

6. **`alliedFortify` from party members / parents**  
   Not modelled in Rust (`CalcEnv` has no party member actor). This is expected — party
   support is out of scope currently.

7. **`DamageTakenWhenHit` MORE mod not injected when `Fortified`**  
   The Lua injects `modDB:NewMod("DamageTakenWhenHit", "MORE", -stacks, "Fortification")`
   unless `NoFortificationMitigation` is set. Rust does not inject this mod, so the defence
   calculation won't see fortification damage reduction.

8. **`FortifyStack` multiplier name likely wrong**  
   Rust writes `mod_db.set_multiplier("FortifyStack", ...)` at `perform.rs:706`. The Lua
   uses `modDB.multipliers["BuffOnSelf"]` for the buff count, not `FortifyStack`.
   "FortifyStack" is not a named multiplier in PoB's Lua; the relevant multipliers are
   `Multiplier:Fortification` (used for damage scaling per stack on some items) which is
   read via `modDB:Sum("BASE", nil, "Multiplier:Fortification")`. Check whether any mod
   in the oracle builds actually references `"FortifyStack"` as a multiplier name.

### What Is Missing

1. **`FortificationStacks` output** (correct name for oracle)
2. **`FortificationEffect` output** (numeric value = stack count; string "0" when `NoFortificationMitigation`)
3. **`ActiveTrapLimit` output** — not implemented in Rust at all (CalcOffence.lua:524)
4. **`ActiveMineLimit` output** — not implemented in Rust at all (CalcOffence.lua:525)
5. **`ActiveTotemLimit` output** — not implemented (CalcPerform.lua:1259; CalcOffence.lua:1383)
6. **`ActiveBrandLimit` output** — not implemented (CalcOffence.lua:1412)
7. **`AilmentWarcryEffect` output** — not implemented (CalcOffence.lua:2503+); always 1
   when no warcry is used, so this would be easy to stub as `output.set("AilmentWarcryEffect", 1.0)`

---

## What Needs to Change

1. **Rename `FortifyStacks` → `FortificationStacks`** in `do_actor_misc` (`perform.rs:705`):
   ```rust
   env.player.set_output("FortificationStacks", stacks);
   ```

2. **Add `FortificationEffect` output**:
   ```rust
   let no_mitigation = env.player.mod_db
       .flag_cfg("NoFortificationMitigation", None, &env.player.output);
   if no_mitigation {
       // Write "0" as number (oracle stores numeric 0 when effect is disabled)
       env.player.set_output("FortificationEffect", 0.0);
   } else {
       env.player.set_output("FortificationEffect", stacks);
       // Inject the damage reduction mod:
       env.player.mod_db.add(Mod::new_more(
           "DamageTakenWhenHit", -stacks, ModSource::new("Buff", "Fortification")));
   }
   ```

3. **Fix `MaximumFortification` fallback** — remove the hardcoded 20.0 default:
   ```rust
   let max_fort = env.player.mod_db.override_value("MaximumFortification", None, &output)
       .unwrap_or_else(|| env.player.mod_db.sum_cfg(
           ModType::Base, "MaximumFortification", skill_cfg, &output));
   // If max_fort == 0, skip the entire Fortify block (no output written)
   if max_fort == 0.0 { return; } // no Fortification mods on this build
   ```

4. **Add `FortificationStacks` Override check**:
   ```rust
   let stacks = env.player.mod_db.override_value("FortificationStacks", None, &output)
       .unwrap_or_else(|| if min_stacks > 0.0 { min_stacks } else { max_fort })
       .min(max_fort);
   ```

5. **Add `MinimumFortification` computation**:
   ```rust
   let min_fort_raw = env.player.mod_db.sum_cfg(
       ModType::Base, "MinimumFortification", None, &output);
   let have_max = env.player.mod_db.flag_cfg("HaveMaxFortification", None, &output);
   let min_stacks = if have_max { max_fort } else { min_fort_raw }.min(max_fort);
   ```

6. **Add `ActiveTrapLimit` output** in the offence pass or a new pre-offence helper:
   ```rust
   let trap_limit = active_skill_mod_db.sum_cfg(
       ModType::Base, "ActiveTrapLimit", Some(&skill_cfg), &output);
   output.set("ActiveTrapLimit", trap_limit);
   ```
   Same for `ActiveMineLimit`, `ActiveTotemLimit`, `ActiveBrandLimit` (skill-type gated).

7. **Add `AilmentWarcryEffect` output** — for the initial implementation, set to 1.0
   unconditionally (correct for all non-warcry builds, which is the vast majority):
   ```rust
   env.player.set_output("AilmentWarcryEffect", 1.0);
   ```
   Full warcry uptime calculation is CalcOffence territory and belongs in the offence pass.

8. **Correct `BuffOnSelf` multiplier** — the Lua increments `modDB.multipliers["BuffOnSelf"]`
   by 1 for each active buff (including Fortification). Rust should use `"BuffOnSelf"` not
   `"FortifyStack"` as the multiplier name for the buff count.

9. **Update `field_groups.rs`** — replace `"FortifyStacks"` with `"FortificationStacks"`,
   replace `"FortifyEffect"` with `"FortificationEffect"`, and remove `"ActiveGolemLimit"`
   and `"BannerStage"` as they are not oracle-verified output fields.
