# PERF-03-charges: Charges (Power / Frenzy / Endurance / Special)

## Output Fields

| Field | Notes |
|-------|-------|
| `PowerCharges` | Current power charges (0 if not using, or max, enforced min, conversion path) |
| `PowerChargesMin` | Minimum sustained power charges (can be non-zero) |
| `PowerChargesMax` | Maximum power charges; respects `Override` before `Sum` |
| `FrenzyCharges` | Current frenzy charges |
| `FrenzyChargesMin` | Minimum sustained frenzy charges |
| `FrenzyChargesMax` | Maximum frenzy charges; `MaximumFrenzyChargesIsMaximumPowerCharges` flag mirrors PC max |
| `EnduranceCharges` | Current endurance charges |
| `EnduranceChargesMin` | Minimum sustained endurance charges |
| `EnduranceChargesMax` | Maximum endurance charges; party-member and frenzy-mirror flags |
| `SiphoningCharges` | 0 unless `UseSiphoningCharges`; then `Override or SiphoningChargesMax` |
| `ChallengerCharges` | 0 unless `UseChallengerCharges`; then `Override or ChallengerChargesMax` |
| `BlitzCharges` | 0 unless `UseBlitzCharges`; then `Override or BlitzChargesMax` |
| `BlitzChargesMax` | Max blitz charges from modDB |
| `BrutalCharges` | Current brutal charges (converted from endurance when `EnduranceChargesConvertToBrutalCharges`) |
| `BrutalChargesMax` | Set when `MaximumEnduranceChargesEqualsMaximumBrutalCharges`; else 0 |
| `BrutalChargesMin` | Set by `MinimumEnduranceChargesEqualsMinimumBrutalCharges` flag chain |
| `AbsorptionCharges` | Current absorption charges (converted from power when `PowerChargesConvertToAbsorptionCharges`) |
| `AbsorptionChargesMax` | Set when `MaximumPowerChargesEqualsMaximumAbsorptionCharges`; else 0 |
| `AbsorptionChargesMin` | Set by `MinimumPowerChargesEqualsMinimumAbsorptionCharges` flag chain |
| `AfflictionCharges` | Current affliction charges (converted from frenzy when `FrenzyChargesConvertToAfflictionCharges`) |
| `AfflictionChargesMax` | Set when `MaximumFrenzyChargesEqualsMaximumAfflictionCharges`; else 0 |
| `AfflictionChargesMin` | Set by `MinimumFrenzyChargesEqualsMinimumAfflictionCharges` flag chain |
| `BloodCharges` | `min(Override or BloodChargesMax, BloodChargesMax)` — always set |
| `BloodChargesMax` | From `modDB:Sum("BASE", nil, "BloodChargesMax")`, min 0 |

**Not in this chunk's output field list** (present in Lua but not tracked here):
`SiphoningChargesMax`, `ChallengerChargesMax`, `InspirationCharges`, `InspirationChargesMax`,
`GhostShrouds`, `CrabBarriers`, `CrabBarriersMax`, `SpiritCharges`, `SpiritChargesMax`,
`RemovablePowerCharges`, `RemovableFrenzyCharges`, `RemovableEnduranceCharges`,
`TotalCharges`, `RemovableTotalCharges`, `*Duration` fields.

## Dependencies

- **PERF-01-attributes**: No direct attribute dependency, but attribute-comparison conditions
  (e.g. `StrHigherThanDex`) set in PERF-01 may gate charge-related mods in the modDB.
- **SETUP-01 through SETUP-04**: The modDB must be fully populated — passives, items, and
  configuration flags like `UsePowerCharges`, `PowerChargesConvertToAbsorptionCharges`,
  `MaximumFrenzyChargesIsMaximumPowerCharges` etc. are set from parsed build data.
- No dependency on PERF-02 (charges don't need pool values).

## Lua Source

File: `third-party/PathOfBuilding/src/Modules/CalcPerform.lua`  
Commit: `454eff8c85d24356d9b051d596983745ed367476`  
Lines: 917–1065 (`doActorCharges` function)

## Annotated Lua

### File-top aliases used in this function

```lua
local m_max   = math.max   -- x.max(y)
local m_min   = math.min   -- x.min(y)
local m_floor = math.floor -- x.floor()
```

`calcLib.mod(modDB, cfg, ...)` = `(1 + modDB:Sum("INC", cfg, ...)/100) * modDB:More(cfg, ...)`.  
Used only for charge duration fields (not tracked by this chunk).

---

### Step 1 — Compute Max/Min values (lines 922–953)

```lua
-- Power charges
output.PowerChargesMin = m_max(modDB:Sum("BASE", nil, "PowerChargesMin"), 0)
-- → max(sum_base("PowerChargesMin"), 0)
-- Minimum is always ≥ 0.

output.PowerChargesMax = modDB:Override(nil, "PowerChargesMax")
                      or m_max(modDB:Sum("BASE", nil, "PowerChargesMax"), 0)
-- Override takes priority (e.g. "You can have up to X Power Charges" config entry).
-- Lua `or` nil-coalesces: if Override returns nil, fall back to Sum, clamped ≥ 0.
-- Rust: mod_db.override_value("PowerChargesMax", None, output)
--         .unwrap_or_else(|| mod_db.sum(..., "PowerChargesMax", ...).max(0.0))

output.PowerChargesDuration = m_floor(
    modDB:Sum("BASE", nil, "ChargeDuration")
    * calcLib.mod(modDB, nil, "PowerChargesDuration", "ChargeDuration"))
-- Duration field; not in PERF-03 output set. Included here for context.
```

**`MaximumFrenzyChargesIsMaximumPowerCharges` flag** (lines 926–929):
```lua
if modDB:Flag(nil, "MaximumFrenzyChargesIsMaximumPowerCharges") then
    local source = modDB.mods["MaximumFrenzyChargesIsMaximumPowerCharges"][1].source
    modDB:ReplaceMod("FrenzyChargesMax", "OVERRIDE", output.PowerChargesMax, source)
end
-- Replaces any existing FrenzyChargesMax OVERRIDE with the current PowerChargesMax value.
-- This must happen BEFORE FrenzyChargesMax is read below, so the subsequent
-- modDB:Override(nil, "FrenzyChargesMax") call picks up the injected override.
-- Rust: inject an override mod into mod_db for "FrenzyChargesMax" equal to pc_max.
```

```lua
-- Frenzy charges
output.FrenzyChargesMin = m_max(modDB:Sum("BASE", nil, "FrenzyChargesMin"), 0)

output.FrenzyChargesMax = modDB:Override(nil, "FrenzyChargesMax")
    or m_max(
        modDB:Flag(nil, "MaximumFrenzyChargesIsMaximumPowerCharges") and output.PowerChargesMax
        or modDB:Sum("BASE", nil, "FrenzyChargesMax"),
        0)
-- Three-way selection via chained and/or:
--   1. If an OVERRIDE mod exists → use it  (the ReplaceMod above may have injected one)
--   2. Else if the "mirrors PC max" flag → use pc_max (already captured)
--   3. Else → Sum("BASE", "FrenzyChargesMax"), min 0
-- Rust equivalent (after override check):
--   if flag("MaximumFrenzyChargesIsMaximumPowerCharges") { pc_max } else { sum_base("FrenzyChargesMax") }.max(0)
```

**`MaximumEnduranceChargesIsMaximumFrenzyCharges` flag** (lines 933–935):
```lua
if modDB:Flag(nil, "MaximumEnduranceChargesIsMaximumFrenzyCharges") then
    local source = modDB.mods["MaximumEnduranceChargesIsMaximumFrenzyCharges"][1].source
    modDB:ReplaceMod("EnduranceChargesMax", "OVERRIDE", output.FrenzyChargesMax, source)
end
-- Same pattern as the Power→Frenzy mirror above, now Frenzy→Endurance.
-- Must happen before EnduranceChargesMax is read.
```

```lua
-- Endurance charges (line 938) — most complex max computation:
output.EnduranceChargesMax = modDB:Override(nil, "EnduranceChargesMax")
    or m_max(
        env.partyMembers.modDB:Flag(nil, "PartyMemberMaximumEnduranceChargesEqualToYours")
            and env.partyMembers.output.EnduranceChargesMax
        or (modDB:Flag(nil, "MaximumEnduranceChargesIsMaximumFrenzyCharges")
            and output.FrenzyChargesMax
            or modDB:Sum("BASE", nil, "EnduranceChargesMax")),
        0)
-- Priority order:
--   1. Override mod (may have been injected by the ReplaceMod above)
--   2. Party member has "PartyMemberMaximumEnduranceChargesEqualToYours" → use party member's EC max
--   3. "MaximumEnduranceChargesIsMaximumFrenzyCharges" flag → use fc_max
--   4. Else → Sum BASE "EnduranceChargesMax", clamped ≥ 0
-- NOTE: `env.partyMembers` — in Rust this is CalcEnv.party_members (a separate actor).
-- When no party members are configured, env.partyMembers.modDB:Flag(...) returns false.
```

**Alternative charge max/min fields** (lines 940–951):
```lua
output.SiphoningChargesMax  = m_max(modDB:Sum("BASE", nil, "SiphoningChargesMax"),  0)
output.ChallengerChargesMax = m_max(modDB:Sum("BASE", nil, "ChallengerChargesMax"), 0)
output.BlitzChargesMax      = m_max(modDB:Sum("BASE", nil, "BlitzChargesMax"),      0)
-- Simple: sum and clamp. No overrides for these.

output.BrutalChargesMin = m_max(
    modDB:Flag(nil, "MinimumEnduranceChargesEqualsMinimumBrutalCharges")
        and (modDB:Flag(nil, "MinimumEnduranceChargesIsMaximumEnduranceCharges")
             and output.EnduranceChargesMax
             or  output.EnduranceChargesMin)
        or 0,
    0)
-- If "MinimumEnduranceChargesEqualsMinimumBrutalCharges":
--   If also "MinimumEnduranceChargesIsMaximumEnduranceCharges" → use ec_max
--   Else → use ec_min
-- Else → 0
-- Then clamp to ≥ 0 (outer m_max is redundant since inner already can't be negative,
-- but matches the PoB code pattern).

output.BrutalChargesMax = m_max(
    modDB:Flag(nil, "MaximumEnduranceChargesEqualsMaximumBrutalCharges")
        and output.EnduranceChargesMax
        or 0,
    0)
-- If "MaximumEnduranceChargesEqualsMaximumBrutalCharges" → ec_max; else 0.

output.AbsorptionChargesMin = m_max(
    modDB:Flag(nil, "MinimumPowerChargesEqualsMinimumAbsorptionCharges")
        and (modDB:Flag(nil, "MinimumPowerChargesIsMaximumPowerCharges")
             and output.PowerChargesMax
             or  output.PowerChargesMin)
        or 0,
    0)
-- Same pattern as BrutalChargesMin but mirroring Power charges.

output.AbsorptionChargesMax = m_max(
    modDB:Flag(nil, "MaximumPowerChargesEqualsMaximumAbsorptionCharges")
        and output.PowerChargesMax
        or 0,
    0)

output.AfflictionChargesMin = m_max(
    modDB:Flag(nil, "MinimumFrenzyChargesEqualsMinimumAfflictionCharges")
        and (modDB:Flag(nil, "MinimumFrenzyChargesIsMaximumFrenzyCharges")
             and output.FrenzyChargesMax
             or  output.FrenzyChargesMin)
        or 0,
    0)
-- Same pattern but mirroring Frenzy charges.

output.AfflictionChargesMax = m_max(
    modDB:Flag(nil, "MaximumFrenzyChargesEqualsMaximumAfflictionCharges")
        and output.FrenzyChargesMax
        or 0,
    0)

output.BloodChargesMax = m_max(modDB:Sum("BASE", nil, "BloodChargesMax"), 0)
-- Simple sum + clamp, like Siphoning/Challenger/Blitz.
```

> **`and/or` ternary gotcha:** All the `BrutalChargesMin`-style expressions use Lua's
> `condition and value_if_true or value_if_false` idiom.  This is safe here because the
> "true" branch values are always positive numbers (never falsy). In Rust use explicit
> `if flag { ... } else { ... }`.

---

### Step 2 — Zero-initialize all current charges (lines 954–967)

```lua
output.PowerCharges     = 0
output.FrenzyCharges    = 0
output.EnduranceCharges = 0
output.SiphoningCharges = 0
output.ChallengerCharges = 0
output.BlitzCharges     = 0
-- ... (InspirationCharges, GhostShrouds, etc. also zeroed — not in PERF-03 set)
output.BrutalCharges    = 0
output.AbsorptionCharges = 0
output.AfflictionCharges = 0
output.BloodCharges     = 0
```

Every current-charge output field starts at 0 and is only raised by subsequent flag checks.
This matters: if no flag applies, the field is explicitly 0, not absent.

---

### Step 3 — Conditionally override Min values (lines 970–978)

```lua
if modDB:Flag(nil, "MinimumFrenzyChargesIsMaximumFrenzyCharges") then
    output.FrenzyChargesMin = output.FrenzyChargesMax
end
if modDB:Flag(nil, "MinimumEnduranceChargesIsMaximumEnduranceCharges") then
    output.EnduranceChargesMin = output.EnduranceChargesMax
end
if modDB:Flag(nil, "MinimumPowerChargesIsMaximumPowerCharges") then
    output.PowerChargesMin = output.PowerChargesMax
end
-- These OVERWRITE the Min values computed in Step 1.
-- Applied AFTER BloodChargesMax etc. are set but BEFORE current charge values are computed.
-- The Min values already fed into AbsorptionChargesMin / AfflictionChargesMin / BrutalChargesMin
-- in Step 1 — those used the *pre-override* Mins. The overrides here affect only the
-- PowerChargesMin/FrenzyChargesMin/EnduranceChargesMin OUTPUT fields and the
-- subsequent minimum enforcement in Step 4.
```

---

### Step 4 — Determine current charge counts (lines 979–1031)

#### Power Charges / Absorption conversion (lines 979–990)

```lua
if modDB:Flag(nil, "UsePowerCharges") then
    output.PowerCharges = modDB:Override(nil, "PowerCharges") or output.PowerChargesMax
    -- If "Use Power Charges" config is on:
    --   Use Override (e.g. user-specified count) or fall back to max.
end
if modDB:Flag(nil, "PowerChargesConvertToAbsorptionCharges") then
    -- Inquisitor Mastermind of Discord or similar: power charges become absorption charges.
    output.AbsorptionCharges = m_max(output.PowerCharges,
                                     m_min(output.AbsorptionChargesMax, output.AbsorptionChargesMin))
    -- AbsorptionCharges = max(current PC value, min(absorption_max, absorption_min))
    -- The m_min(max, min) enforces the minimum even when PowerCharges is 0.
    output.PowerCharges = 0
    -- Power charges are zeroed when converted.
else
    output.PowerCharges = m_max(output.PowerCharges,
                                m_min(output.PowerChargesMax, output.PowerChargesMin))
    -- Enforce minimum: PowerCharges = max(current, min(max, min))
    -- If not using charges: current=0, min=0 → 0. If using: current=max → max.
    -- If PowerChargesMin > 0: even with 0 charges, result = min(max, min) = min.
end
output.RemovablePowerCharges = m_max(output.PowerCharges - output.PowerChargesMin, 0)
-- How many charges can be removed (stacks above the minimum).
```

> **`m_min(max, min)` pattern:** When `PowerChargesMin = 0` this is `min(N, 0) = 0`.
> When `PowerChargesMin = N` (minimum equals maximum, e.g. "always have max charges"),
> this is `min(N, N) = N`, so `m_max(0, N) = N` ensures those charges are always active.
> This is how "minimum charges" keystones are enforced without a separate flag.

#### Frenzy Charges / Affliction conversion (lines 991–1002)

```lua
if modDB:Flag(nil, "UseFrenzyCharges") then
    output.FrenzyCharges = modDB:Override(nil, "FrenzyCharges") or output.FrenzyChargesMax
end
if modDB:Flag(nil, "FrenzyChargesConvertToAfflictionCharges") then
    output.AfflictionCharges = m_max(output.FrenzyCharges,
                                     m_min(output.AfflictionChargesMax, output.AfflictionChargesMin))
    output.FrenzyCharges = 0
else
    output.FrenzyCharges = m_max(output.FrenzyCharges,
                                 m_min(output.FrenzyChargesMax, output.FrenzyChargesMin))
end
output.RemovableFrenzyCharges = m_max(output.FrenzyCharges - output.FrenzyChargesMin, 0)
-- Exact same pattern as Power/Absorption above, mirrored for Frenzy/Affliction.
```

#### Endurance Charges / Brutal conversion (lines 1003–1014)

```lua
if modDB:Flag(nil, "UseEnduranceCharges") then
    output.EnduranceCharges = modDB:Override(nil, "EnduranceCharges") or output.EnduranceChargesMax
end
if modDB:Flag(nil, "EnduranceChargesConvertToBrutalCharges") then
    output.BrutalCharges = m_max(output.EnduranceCharges,
                                 m_min(output.BrutalChargesMax, output.BrutalChargesMin))
    output.EnduranceCharges = 0
else
    output.EnduranceCharges = m_max(output.EnduranceCharges,
                                    m_min(output.EnduranceChargesMax, output.EnduranceChargesMin))
end
output.RemovableEnduranceCharges = m_max(output.EnduranceCharges - output.EnduranceChargesMin, 0)
-- Same pattern: Endurance/Brutal.
```

#### Siphoning, Challenger, Blitz (lines 1015–1023)

```lua
if modDB:Flag(nil, "UseSiphoningCharges") then
    output.SiphoningCharges = modDB:Override(nil, "SiphoningCharges") or output.SiphoningChargesMax
end
if modDB:Flag(nil, "UseChallengerCharges") then
    output.ChallengerCharges = modDB:Override(nil, "ChallengerCharges") or output.ChallengerChargesMax
end
if modDB:Flag(nil, "UseBlitzCharges") then
    output.BlitzCharges = modDB:Override(nil, "BlitzCharges") or output.BlitzChargesMax
end
-- Simple: flag → override-or-max; no conversion path; no minimum enforcement.
-- These charges have no "minimum" concept; they just use max or 0.
```

#### Blood Charges (line 1030)

```lua
output.BloodCharges = m_min(modDB:Override(nil, "BloodCharges") or output.BloodChargesMax,
                             output.BloodChargesMax)
-- DIFFERENT pattern: BloodCharges is always set (no "UseBloodCharges" guard).
-- Value = min(Override or max, max).
-- If no override: min(max, max) = max.
-- If override exceeds max: clamped to max.
-- BloodCharges are always active (Transcendence keystone).
```

---

### Step 5 — `HaveMaximum*Charges` flag overrides (lines 1034–1042)

```lua
if modDB:Flag(nil, "HaveMaximumPowerCharges") then
    output.PowerCharges = output.PowerChargesMax
end
if modDB:Flag(nil, "HaveMaximumFrenzyCharges") then
    output.FrenzyCharges = output.FrenzyChargesMax
end
if modDB:Flag(nil, "HaveMaximumEnduranceCharges") then
    output.EnduranceCharges = output.EnduranceChargesMax
end
-- These flags can force charges to max AFTER the conversion logic.
-- E.g. "Always have maximum endurance charges" — set by config or keystones.
-- These run AFTER the conversion checks, so if EnduranceChargesConvertToBrutalCharges,
-- EnduranceCharges was zeroed in step 4 but gets overridden back to max here.
-- This is the Lua ordering; Rust must replicate it exactly.
```

---

### Step 6 — TotalCharges and multiplier writes (lines 1043–1064)

```lua
output.TotalCharges = output.PowerCharges + output.FrenzyCharges + output.EnduranceCharges
-- TotalCharges only counts the three core charge types.

modDB.multipliers["PowerCharge"]          = output.PowerCharges
modDB.multipliers["PowerChargeMax"]       = output.PowerChargesMax
modDB.multipliers["RemovablePowerCharge"] = output.RemovablePowerCharges
modDB.multipliers["FrenzyCharge"]         = output.FrenzyCharges
modDB.multipliers["RemovableFrenzyCharge"]= output.RemovableFrenzyCharges
modDB.multipliers["EnduranceCharge"]      = output.EnduranceCharges
modDB.multipliers["RemovableEnduranceCharge"] = output.RemovableEnduranceCharges
modDB.multipliers["TotalCharges"]         = output.TotalCharges
modDB.multipliers["RemovableTotalCharges"]= output.RemovableTotalCharges
modDB.multipliers["SiphoningCharge"]      = output.SiphoningCharges
modDB.multipliers["ChallengerCharge"]     = output.ChallengerCharges
modDB.multipliers["BlitzCharge"]          = output.BlitzCharges
-- (InspirationCharge, GhostShroud, CrabBarrier — not tracked by PERF-03)
modDB.multipliers["BrutalCharge"]         = output.BrutalCharges
modDB.multipliers["AbsorptionCharge"]     = output.AbsorptionCharges
modDB.multipliers["AfflictionCharge"]     = output.AfflictionCharges
modDB.multipliers["BloodCharge"]          = output.BloodCharges
-- Multipliers are used by PerMultiplier tags (e.g. "+5% per Power Charge").
-- In Rust: mod_db.set_multiplier("PowerCharge", pc).
```

---

## Existing Rust Code

File: `crates/pob-calc/src/calc/perform.rs`, lines 494–674 (`do_actor_charges`)

### What Exists

**Core three charges (Power / Frenzy / Endurance):**
- Min/Max summed from modDB Base mods: ✓
- `UsePowerCharges` / `UseFrenzyCharges` / `UseEnduranceCharges` flags: ✓ (also checks conditions map)
- Current value = max when using, min otherwise: ✓
- Outputs written: `PowerChargesMin`, `PowerChargesMax`, `PowerCharges`, and same for Frenzy/Endurance: ✓
- Multipliers set: `PowerCharge`, `FrenzyCharge`, `EnduranceCharge`, `TotalCharges`: ✓
- `HaveMaximum*Charges` conditions: ✓

**Alternative charges (Siphoning, Challenger, Blitz):**
- Present via a generic alt-charges loop (lines 632–673): ✓ for setting the output field
- Their `Max` outputs (`SiphoningChargesMax`, `ChallengerChargesMax`, `BlitzChargesMax`): written implicitly via the loop's `max_key` computation, **but only if the `Use*` flag is set**. When the flag is not set, `val = 0.0` is written for the charge count, but `*Max` is not separately written to output.

**BlitzChargesMax specifically:**  
`BlitzChargesMax` is in PERF-03's field list and is always written by Lua (line 942) regardless of whether `UseBlitzCharges` is set. The Rust loop only queries `BlitzChargesMax` when building `val`, but only calls `set_output("BlitzCharges", val)` — it does **not** separately call `set_output("BlitzChargesMax", max_val)`. So `BlitzChargesMax` is **missing** from Rust output.

### What Is Missing

1. **`PowerChargesMax` Override check** — Rust reads only `sum_cfg(Base, "PowerChargesMax")`;
   does not call `override_value("PowerChargesMax")` first. Same for FrenzyChargesMax and
   EnduranceChargesMax.

2. **`MaximumFrenzyChargesIsMaximumPowerCharges` flag** — Rust does not check this flag;
   `FrenzyChargesMax` is always the raw sum, never mirrored from `PowerChargesMax`.

3. **`MaximumEnduranceChargesIsMaximumFrenzyCharges` flag** — not implemented.

4. **`PartyMemberMaximumEnduranceChargesEqualToYours` party-member path** — not implemented
   (no party member support in current Rust CalcEnv).

5. **`MinimumXIsMaximumX` flag overrides for Min fields** (lines 970–978):
   - `MinimumFrenzyChargesIsMaximumFrenzyCharges`
   - `MinimumEnduranceChargesIsMaximumEnduranceCharges`
   - `MinimumPowerChargesIsMaximumPowerCharges`
   None of these override the Min output value in Rust.

6. **Charge conversion paths** — completely absent:
   - `PowerChargesConvertToAbsorptionCharges` → `AbsorptionCharges`, zeroes `PowerCharges`
   - `FrenzyChargesConvertToAfflictionCharges` → `AfflictionCharges`, zeroes `FrenzyCharges`
   - `EnduranceChargesConvertToBrutalCharges` → `BrutalCharges`, zeroes `EnduranceCharges`

7. **Minimum enforcement via `m_max(current, m_min(max, min))`** — Rust uses
   `if use_pc { pc_max } else { pc_min }` which is correct only when `pc_min = 0`.
   For builds with a non-zero minimum (e.g. "always have N endurance charges"), the Lua
   enforces `m_max(0, m_min(ec_max, ec_min)) = ec_min` even when `UseEnduranceCharges` is
   false. Rust produces `pc_min` in the else branch which is also `ec_min`, so this is
   numerically correct **as long as no conversion is applied**. However it's semantically
   fragile — it conflates "not using" with the minimum floor.

8. **`BrutalCharges`, `BrutalChargesMin`, `BrutalChargesMax`** — not output at all.

9. **`AbsorptionCharges`, `AbsorptionChargesMin`, `AbsorptionChargesMax`** — not output.

10. **`AfflictionCharges`, `AfflictionChargesMin`, `AfflictionChargesMax`** — not output.

11. **`BloodCharges`, `BloodChargesMax`** — not output.

12. **`BlitzChargesMax`** — not written as a separate output field (only used internally in
    the alt-charges loop).

13. **`PowerChargeMax` multiplier** — Lua sets `modDB.multipliers["PowerChargeMax"] = output.PowerChargesMax` (line 1046); Rust does not set this multiplier.

14. **`RemovablePowerCharge`, `RemovableFrenzyCharge`, `RemovableEnduranceCharge`, `RemovableTotalCharges` multipliers** — Lua sets these (lines 1047, 1049, 1051, 1053); Rust does not.

### What Is Wrong

1. **`HaveMaximumPowerCharges` condition set prematurely** — Rust sets this condition at
   line 601–609 before the `HaveMaximumXCharges` flag check that can force charges to max
   (lines 1034–1042 in Lua). This means the condition is computed from the pre-override
   charge count. In Lua the condition is set implicitly after the flag overrides run (the
   condition is checked later in CalcDefence/CalcOffence, not within doActorCharges). The
   Rust code sets the condition early and doesn't re-evaluate it after the flag override.

2. **`SiphoningChargesMax`, `ChallengerChargesMax` not written as separate output fields** —
   the Rust generic loop writes only the current charge count, not the `*Max` variant. Lua
   always writes `*Max` first (lines 940–941) regardless of the `Use*` flag.

---

## What Needs to Change

1. **Add `Override` check for PowerChargesMax / FrenzyChargesMax / EnduranceChargesMax:**
   ```rust
   let pc_max = mod_db.override_value("PowerChargesMax", None, output)
       .unwrap_or_else(|| mod_db.sum(Base, "PowerChargesMax").max(0.0));
   ```

2. **Implement `MaximumFrenzyChargesIsMaximumPowerCharges`:**
   After computing `pc_max`, if the flag is set, inject an override into `mod_db` for
   `FrenzyChargesMax` equal to `pc_max`, then compute `fc_max` after this injection.

3. **Implement `MaximumEnduranceChargesIsMaximumFrenzyCharges`:**
   Same pattern: after `fc_max` is set, inject override for `EnduranceChargesMax` if flagged.

4. **Implement `MinimumXIsMaximumX` overrides for Min output fields:**
   After computing all three Max values, check each flag and overwrite the Min:
   ```rust
   if mod_db.flag("MinimumPowerChargesIsMaximumPowerCharges") { pc_min = pc_max; }
   // etc.
   ```

5. **Implement charge conversion paths (`*ConvertTo*`):**
   After the `Use*` flag sets initial charge counts, apply conversion:
   ```rust
   if mod_db.flag("PowerChargesConvertToAbsorptionCharges") {
       absorption = pc.max(absorption_max.min(absorption_min));
       pc = 0.0;
   } else {
       pc = pc.max(pc_max.min(pc_min)); // enforce minimum
   }
   // same for Frenzy→Affliction and Endurance→Brutal
   ```

6. **Implement minimum enforcement in the non-conversion else branch:**
   Change `let pc = if use_pc { pc_max } else { pc_min };` to:
   ```rust
   let pc = if use_pc {
       mod_db.override_value("PowerCharges").unwrap_or(pc_max)
   } else { 0.0 };
   let pc = pc.max(pc_max.min(pc_min)); // enforce minimum floor
   ```

7. **Apply `HaveMaximumXCharges` flag AFTER conversion logic:**
   Move the flag override to after step 4's conversion, so it properly restores
   converted-away charges if needed:
   ```rust
   if mod_db.flag("HaveMaximumPowerCharges") { pc = pc_max; }
   ```

8. **Output `BlitzChargesMax`, `SiphoningChargesMax`, `ChallengerChargesMax` separately:**
   Write them unconditionally before computing current counts, matching Lua lines 940–942.

9. **Implement Brutal / Absorption / Affliction / Blood charge output fields:**
   Add `BrutalChargesMin`, `BrutalChargesMax`, `BrutalCharges` using the flag-chain formulas
   in lines 945–946 and the conversion in lines 1006–1013. Same for Absorption (power
   mirror) and Affliction (frenzy mirror). Blood charges: always set via
   `min(Override or max, max)` pattern.

10. **Add missing multipliers:**
    - `PowerChargeMax` = `pc_max`
    - `RemovablePowerCharge` = `max(pc - pc_min, 0)`
    - `RemovableFrenzyCharge` = `max(fc - fc_min, 0)`
    - `RemovableEnduranceCharge` = `max(ec - ec_min, 0)`
    - `RemovableTotalCharges` = sum of the above three
    - `BrutalCharge`, `AbsorptionCharge`, `AfflictionCharge`, `BloodCharge`
