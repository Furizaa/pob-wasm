# PERF-07: Regen / Recharge / Leech

## Output Fields

The following output fields must be written by this chunk's Rust implementation:

**Recovery rate modifiers (CalcDefence.lua ~1191-1197)**
- `LifeRecoveryRateMod` — combined `(1 + INC/100) * More` for life recovery rate
- `ManaRecoveryRateMod` — same for mana
- `EnergyShieldRecoveryRateMod` — same for ES

**Leech caps (CalcDefence.lua ~1200-1232)**
- `MaxLifeLeechRatePercent` — % of life, queried from modDB `MaxLifeLeechRate`
- `MaxLifeLeechRate` — absolute (Life × MaxLifeLeechRatePercent / 100)
- `MaxManaLeechRate` — absolute (Mana × MaxManaLeechRate% / 100)
- `MaxEnergyShieldLeechRate` — absolute (ES × MaxEnergyShieldLeechRate% / 100)

**Regeneration (CalcDefence.lua ~1234-1320)**
- `LifeRegen` — flat life regen per second (0 if NoLifeRegen or ZealotsOath)
- `LifeRegenPercent` — `LifeRegenRecovery / Life * 100` (rounds to 1 dp)
- `ManaRegen` — flat mana regen per second
- `ManaRegenPercent` — `ManaRegenRecovery / Mana * 100`
- `EnergyShieldRegen` — flat ES regen per second
- `EnergyShieldRegenPercent` — `EnergyShieldRegenRecovery / ES * 100`
- `LifeDegen` — net degen rate per second (flat + percent-of-pool)
- `NetLifeRegen` — only written when `TotalBuildDegen > 0` (see §Degen section)
- `NetManaRegen` — same condition
- `NetEnergyShieldRegen` — same condition

**ES recharge (CalcDefence.lua ~1322-1381)**
- `EnergyShieldRecharge` — ES/s recharge rate (0 if both flags below are absent)
- `EnergyShieldRechargeDelay` — seconds before recharge starts
- `WardRechargeDelay` — delay before Ward restores (CalcDefence.lua ~1474)

**Leech rates (CalcOffence.lua ~3802-3843)**
- `LifeLeechRate` — combined instant + capped non-instant × RecoveryRateMod
- `ManaLeechRate` — same for mana
- `EnergyShieldLeechRate` — same for ES
- `LifeLeechGainRate` — LifeLeechRate + LifeOnHitRate (DPS mode)
- `ManaLeechGainRate` — same
- `EnergyShieldLeechGainRate` — same
- `LifeLeechDuration` — duration of a single leech instance in seconds
- `ManaLeechDuration`
- `EnergyShieldLeechDuration`
- `LifeLeechInstances` — number of active leech instances at hit rate
- `ManaLeechInstances`
- `EnergyShieldLeechInstances`
- `LifeLeechInstantRate` — LifeLeechInstant × hitRate
- `ManaLeechInstantRate`
- `EnergyShieldLeechInstantRate`
- `LifeOnHitRate` — LifeOnHit × hitRate
- `ManaOnHitRate`
- `EnergyShieldOnHitRate`

> **Note on `LifeRecoveryRate`, `ManaRecoveryRate`, `EnergyShieldRecoveryRate`:**  
> These names appear in `field_groups.rs` but do **not** appear in any of the 30
> oracle expected JSON files. The actual oracle-present fields are `*RecoveryRateMod`.
> `field_groups.rs` should be corrected to use `LifeRecoveryRateMod` etc. This chunk
> does **not** need to produce `*RecoveryRate` fields as named above.
>
> **Note on `LifeDegenRate`, `LifeRecoveryRateTotal`, `ManaRecoveryRateTotal`,
> `EnergyShieldRechargeRecovery`:**  
> These also appear in `field_groups.rs` but do **not** exist in any oracle expected
> output or in PoB's Lua source. They are stale placeholder entries. This chunk does
> **not** need to produce them.
>
> **Note on `WardRecharge` (the rate, not the delay):**  
> PoB's Lua does not write `output.WardRecharge`. Only `output.WardRechargeDelay` is
> written. `WardRecharge` in `field_groups.rs` is a phantom field.

## Dependencies

- `PERF-02-life-mana-es` — `Life`, `Mana`, `EnergyShield` pools must exist before leech caps and regen can be computed
- `PERF-04-reservation` — `LifeUnreserved`, `ManaUnreserved` (used as pool values for regen in some paths)
- `PERF-05-buffs` — conditions like `VaalPact`, `GhostReaver`, `ZealotsOath` must be set
- `PERF-03-charges` — charge-related conditions that gate some regen behaviour

## Lua Source

**Primary file: `CalcDefence.lua`**  
Lines 1191–1484 (recovery rate mods, leech caps, regen loop, ES recharge, Ward recharge)

**Secondary file: `CalcOffence.lua`**  
Lines 3457–3843 (leech instance calculation, leech rates, on-hit/on-kill gains)

Commit: `454eff8c85d24356d9b051d596983745ed367476` (third-party/PathOfBuilding, heads/dev)

## Annotated Lua

### Section 1: Recovery Rate Modifiers (CalcDefence.lua 1191–1197)

```lua
-- Recovery modifiers
output.LifeRecoveryRateMod = 1                                -- Rust: default 1.0
if not modDB:Flag(nil, "CannotRecoverLifeOutsideLeech") then
    -- calcLib.mod = (1 + Sum("INC",...)/100) * More(...)
    -- Lua idiom: calcLib.mod returns a multiplier, not a percent.
    output.LifeRecoveryRateMod = calcLib.mod(modDB, nil, "LifeRecoveryRate")
    -- Rust: calc_mod(mod_db, None, "LifeRecoveryRate")
    --       = (1 + mod_db.sum(Inc,"LifeRecoveryRate")/100) * mod_db.more("LifeRecoveryRate")
end
output.ManaRecoveryRateMod = calcLib.mod(modDB, nil, "ManaRecoveryRate")
output.EnergyShieldRecoveryRateMod = calcLib.mod(modDB, nil, "EnergyShieldRecoveryRate")
```

**Gotcha:** `calcLib.mod` queries `INC` and `More` for the *RecoveryRate* stat, not the
*Regen* stat. The result is a multiplicative modifier (e.g., 1.20 means 20% faster).
The default when no mods are present is exactly 1.0. The
`CannotRecoverLifeOutsideLeech` flag must set `LifeRecoveryRateMod = 1` (bypass all
recovery rate mods for life).

### Section 2: Leech Caps (CalcDefence.lua 1199–1232)

```lua
-- Leech caps
output.MaxLifeLeechInstance = output.Life * calcLib.val(modDB, "MaxLifeLeechInstance") / 100
    -- calcLib.val = modDB:Sum("BASE", nil, "MaxLifeLeechInstance")
    -- Default from game constants is 10 (%)
output.MaxLifeLeechRatePercent = calcLib.val(modDB, "MaxLifeLeechRate")
    -- Default from game constants is 20 (%)
if modDB:Flag(nil, "MaximumLifeLeechIsEqualToParent") then
    -- Totem/minion inherits parent's leech rate percent
    output.MaxLifeLeechRatePercent = actor.parent.output.MaxLifeLeechRatePercent
elseif modDB:Flag(nil, "MaximumLifeLeechIsEqualToPartyMember") then
    output.MaxLifeLeechRatePercent = actor.partyMembers.output.MaxLifeLeechRatePercent
end
output.MaxLifeLeechRate = output.Life * output.MaxLifeLeechRatePercent / 100
    -- Absolute cap in life/s

output.MaxEnergyShieldLeechInstance = output.EnergyShield * calcLib.val(modDB, "MaxEnergyShieldLeechInstance") / 100
output.MaxEnergyShieldLeechRate = output.EnergyShield * calcLib.val(modDB, "MaxEnergyShieldLeechRate") / 100
    -- Note: default MaxEnergyShieldLeechRate is NOT the same as life. Check game_constants.

output.MaxManaLeechInstance = output.Mana * calcLib.val(modDB, "MaxManaLeechInstance") / 100
output.MaxManaLeechRate = output.Mana * calcLib.val(modDB, "MaxManaLeechRate") / 100
```

**Gotcha:** `calcLib.val(modDB, "X")` = `modDB:Sum("BASE", nil, "X")`. It does **not**
fall back to a default — callers are expected to seed the default via `NewMod` in `initEnv`
or via game constants. In Rust, the default leech rates (20% life, 10% ES) are seeded by
`add_base_constants` in `defence.rs`. Check that `MaxEnergyShieldLeechRate` game constant
is actually 10 (not 20) — the Lua writes separate `calcLib.val(modDB, "MaxEnergyShieldLeechRate")`.

**Gotcha:** The parent/party-member inheritance branches for
`MaximumLifeLeechIsEqualToParent` and `MaximumLifeLeechIsEqualToPartyMember` are
minion/totem support features. For the player actor these flags are always false. The
current Rust does not implement these branches — acceptable for now, noted as a TAIL gap.

### Section 3: Regeneration Loop (CalcDefence.lua 1234–1320)

```lua
-- Regen loop iterates over {"Mana", "Life", "Energy Shield", "Rage"}
-- Note: Lua string "Energy Shield" → output key "EnergyShield" via gsub(" ", "")
local resources = {"Mana", "Life", "Energy Shield", "Rage"}
for i, resourceName in ipairs(resources) do
    local resource = resourceName:gsub(" ", "")  -- "EnergyShield", etc.
    local pool = output[resource] or 0            -- Rust: get_output_f64(output, resource)
    -- ... (see full logic below)
end
```

**Full regen logic per resource:**

```lua
    local inc = modDB:Sum("INC", nil, resource.."Regen")
    local more = modDB:More(nil, resource.."Regen")
    local regen = 0
    local regenRate = 0
    local recoveryRateMod = output[resource.."RecoveryRateMod"] or 1
    --                       ↑ Lua nil-coalesce; in Rust use get_output_f64 which returns 0.0
    --                         but RecoveryRateMod is set to 1 earlier, so should be 1.0.

    if modDB:Flag(nil, "No"..resource.."Regen")
       or modDB:Flag(nil, "CannotGain"..resource) then
        output[resource.."Regen"] = 0
        -- No computation; leaves regenRate = 0

    elseif resource == "Life" and modDB:Flag(nil, "ZealotsOath") then
        output.LifeRegen = 0
        -- Redirect: any LifeRegen BASE mods become EnergyShieldRegen BASE mods
        local lifeBase = modDB:Sum("BASE", nil, "LifeRegen")
        if lifeBase > 0 then
            modDB:NewMod("EnergyShieldRegen", "BASE", lifeBase, "Zealot's Oath")
        end
        local lifePercent = modDB:Sum("BASE", nil, "LifeRegenPercent")
        if lifePercent > 0 then
            modDB:NewMod("EnergyShieldRegenPercent", "BASE", lifePercent, "Zealot's Oath")
        end
        -- IMPORTANT: regenRate stays 0 for life; EnergyShield iteration will pick up the new mods

    else
        -- Chain redirection (rare: keystone redirects INC from one resource to another)
        if inc ~= 0 then
            for j=i+1,#resources do
                if modDB:Flag(nil, resource.."RegenTo"..resources[j]:gsub(" ","").."Regen") then
                    modDB:NewMod(resources[j]:gsub(" ","").."Regen", "INC", inc, ...)
                    inc = 0
                end
            end
        end

        -- Life regen applies to ES (Pious Path variant)
        if resource == "Life"
           and modDB:Sum("BASE", nil, "LifeRegenAppliesToEnergyShield") > 0 then
            local conversion = m_min(modDB:Sum("BASE", nil, "LifeRegenAppliesToEnergyShield"), 100) / 100
            local lifeBase   = modDB:Sum("BASE", nil, "LifeRegen")
            local lifePercent = modDB:Sum("BASE", nil, "LifeRegenPercent")
            -- floor(x, 2) = round to 2 decimal places (PoB helper)
            modDB:NewMod("EnergyShieldRegen", "BASE", floor(lifeBase * conversion, 2), ...)
            modDB:NewMod("EnergyShieldRegenPercent", "BASE", floor(lifePercent * conversion, 2), ...)
        end

        -- Core regen formula:
        baseRegen = modDB:Sum("BASE", nil, resource.."Regen")
                  + pool * modDB:Sum("BASE", nil, resource.."RegenPercent") / 100
        regen = baseRegen * (1 + inc/100) * more
        -- Note: Pious Path detection: if regen != 0, route regen to other resources
        regenRate = round(regen * recoveryRateMod, 1)
        --          ↑ round() rounds to 1 decimal place
        --          ↑ recoveryRateMod applied AFTER inc/more
        output[resource.."Regen"] = regenRate
    end

    -- Always set RegenInc regardless of the branch above
    output[resource.."RegenInc"] = inc

    -- Degen:
    local baseDegen = modDB:Sum("BASE", nil, resource.."Degen")
                    + (pool * modDB:Sum("BASE", nil, resource.."DegenPercent") / 100)
    -- Tincture minimum degen (exotic mechanic; usually 0):
    local tinctureDegenPercent = modDB:Sum("BASE", nil, resource.."DegenPercentTincture")
    baseDegen = baseDegen + m_max(pool * tinctureDegenPercent / 100, tinctureDegenPercent)
    local degenRate = (baseDegen > 0)
                      and baseDegen * calcLib.mod(modDB, nil, resource.."Degen")
                      or 0
    --                ↑ Lua ternary: if baseDegen > 0 apply modifier, else 0
    output[resource.."Degen"] = degenRate

    -- Recovery (from CalcOffence/misc modDB NEW_MODs, e.g. Pious Path recovery events):
    local recoveryRate = modDB:Sum("BASE", nil, resource.."Recovery") * recoveryRateMod
    output[resource.."Recovery"] = recoveryRate

    -- RegenRecovery = net regen (regen - degen + recovery):
    output[resource.."RegenRecovery"] =
        (modDB:Flag(nil, "UnaffectedBy"..resource.."Regen") and 0 or regenRate)
        - degenRate + recoveryRate

    -- Gate condition for downstream chunks:
    if output[resource.."RegenRecovery"] > 0 then
        modDB:NewMod("Condition:CanGain"..resource, "FLAG", true, ...)
    end

    -- RegenPercent = % of pool recovered per second by net regen:
    output[resource.."RegenPercent"] =
        pool > 0
        and round(output[resource.."RegenRecovery"] / pool * 100, 1)
        or 0
    -- ↑ Lua short-circuit: if pool == 0, result is 0 (avoids divide-by-zero)
    -- ↑ round(x, 1) rounds to 1 decimal place
```

**Gotcha:** The `recoveryRateMod` is applied to `regen` to get `regenRate`, but the
`LifeRecoveryRateMod` for life specifically comes from the earlier `calcLib.mod` call
(or 1 if `CannotRecoverLifeOutsideLeech`). The current `defence.rs::calc_regeneration`
fetches `*RecoveryRateMod` via `get_output_f64` which defaults to 0.0 if missing — but it
should be 1.0 before the recovery_rate_mods have been computed. The call order matters:
`calc_recovery_rates` must run before `calc_regeneration`.

**Gotcha:** `round(x, 1)` in PoB rounds to 1 decimal place (not integer round). Rust
equivalent: `(x * 10.0).round() / 10.0`. The `LifeRegenPercent` and `ManaRegenPercent`
fields are also rounded with this function.

**Gotcha:** The regen loop processes `resources` in order: `{"Mana", "Life", "Energy Shield", "Rage"}`.
Zealot's Oath and Pious Path mutation of the modDB uses `i+1..#resources` forward
indices — meaning mutations during Mana's iteration can affect Life, and mutations during
Life's iteration can affect Energy Shield. The loop order is **semantically significant**.
In Rust, process in the same order.

**Gotcha:** `modDB:NewMod("EnergyShieldRegen", "BASE", lifeBase, "Zealot's Oath")` mutates
the modDB mid-loop. When the ES iteration runs, it will pick up these new mods. Rust must
either mutate `mod_db` in place (and process ES after Life) or compute redirected values
explicitly.

**Gotcha:** `RegenRecovery` fields (`LifeRegenRecovery` etc.) are *intermediate* values
that are **not** in `field_groups.rs` for this chunk but are read by the `NetLifeRegen`
section (CalcDefence ~3371). They must be written to output.

### Section 4: ES Recharge (CalcDefence.lua 1322–1381)

```lua
-- Two flags control whether ES recharge applies at all:
output.EnergyShieldRechargeAppliesToLife =
    modDB:Flag(nil, "EnergyShieldRechargeAppliesToLife")
    and not modDB:Flag(nil, "CannotRecoverLifeOutsideLeech")

output.EnergyShieldRechargeAppliesToEnergyShield =
    not (modDB:Flag(nil, "NoEnergyShieldRecharge")
         or modDB:Flag(nil, "CannotGainEnergyShield")
         or output.EnergyShieldRechargeAppliesToLife)
    -- If appliesToLife is true, it does NOT apply to ES (mutually exclusive)

if output.EnergyShieldRechargeAppliesToLife
   or output.EnergyShieldRechargeAppliesToEnergyShield then
    local inc  = modDB:Sum("INC", nil, "EnergyShieldRecharge")
    local more = modDB:More(nil, "EnergyShieldRecharge")
    local base = modDB:Override(nil, "EnergyShieldRecharge")
                 or data.misc.EnergyShieldRechargeBase
    --           ↑ Override takes precedence; fallback is game constant (default 0.20 = 20%/s)

    if output.EnergyShieldRechargeAppliesToLife then
        local recharge = output.Life * base * (1 + inc/100) * more
        output.LifeRecharge = round(recharge * output.LifeRecoveryRateMod)
        -- Note: round() here rounds to nearest integer (no decimal arg → integer)
    else
        local recharge = output.EnergyShield * base * (1 + inc/100) * more
        output.EnergyShieldRecharge = round(recharge * output.EnergyShieldRecoveryRateMod)
        -- round() with no second arg = round to integer
    end

    output.EnergyShieldRechargeDelay =
        data.misc.EnergyShieldRechargeDelay
        / (1 + modDB:Sum("INC", nil, "EnergyShieldRechargeFaster") / 100)
else
    output.EnergyShieldRecharge = 0
    -- No delay written when disabled; PoB leaves it nil
end
```

**Gotcha:** The mod name for the recharge *rate* is `"EnergyShieldRecharge"` but the
current `defence.rs::calc_es_recharge` queries `"EnergyShieldRechargeRate"` — this is
a **bug**: it should query `"EnergyShieldRecharge"` (INC and More).

**Gotcha:** `modDB:Override(nil, "EnergyShieldRecharge")` can override the base recharge
percentage. If present it replaces `data.misc.EnergyShieldRechargeBase` (0.20). The
current Rust hardcodes `es * 0.20` without checking for an override. This is missing.

**Gotcha:** `round(x)` with no second argument rounds to the nearest integer. The oracle
shows `EnergyShieldRecharge: 34` (integer), confirming this.

**Gotcha:** When `EnergyShieldRechargeAppliesToLife` is true the recharge goes into
`output.LifeRecharge` (not `EnergyShieldRecharge`). This is a rare keystone; current Rust
handles it as `LifeRechargeRate`. The field name must match PoB exactly.

**Gotcha:** `EnergyShieldRechargeDelay` default is `data.misc.EnergyShieldRechargeDelay`
(2.0 seconds). Oracle confirms: `EnergyShieldRechargeDelay: 2`. The `%_faster` divisor
uses the same pattern as all speed reductions.

### Section 5: Ward Recharge Delay (CalcDefence.lua 1473–1483)

```lua
output.WardRechargeDelay =
    data.misc.WardRechargeDelay
    / (1 + modDB:Sum("INC", nil, "WardRechargeFaster") / 100)
```

**Note:** PoB does **not** write `output.WardRecharge` (the rate field). Ward recharge
rate is not computed by PoB's calc engine — only the delay is exposed as an output field.
The `WardRecharge` entry in `field_groups.rs` is incorrect. The field in oracle is
`WardRechargeDelay: 2` (float).

**Note:** `data.misc.WardRechargeDelay` default value — check game data; typically 2.0s
(same as ES). Rust must use `game_constants` or a hardcoded constant.

### Section 6: Net Regen (CalcDefence.lua ~3368–3461)

The `NetLifeRegen`, `NetManaRegen`, `NetEnergyShieldRegen` fields are **only written** when
`output.TotalBuildDegen > 0` (i.e., when the build has active DoT degens hitting the
player). The logic computes how much of each degen type hits life vs ES vs mana (accounting
for MindOverMatter and EnergyShieldBypass), then subtracts from the `*RegenRecovery`
totals.

```lua
if output.TotalBuildDegen == 0 then
    output.TotalBuildDegen = nil   -- clears the field if no degen
else
    output.NetLifeRegen = output.LifeRegenRecovery
    output.NetManaRegen = output.ManaRegenRecovery
    output.NetEnergyShieldRegen = output.EnergyShieldRegenRecovery
    -- ... then subtract per-damage-type degen contributions split across pools
    output.NetLifeRegen = output.NetLifeRegen - totalLifeDegen
    output.NetManaRegen = output.NetManaRegen - totalManaDegen
    output.NetEnergyShieldRegen = output.NetEnergyShieldRegen - totalEnergyShieldDegen
end
```

The degen accounting references `output[damageType.."BuildDegen"]`,
`output[damageType.."EnergyShieldBypass"]`, `output[damageType.."MindOverMatter"]`,
`output.sharedMindOverMatter`, and `output.EnergyShieldRegenRecovery`. These are computed
elsewhere in CalcDefence (DEF-04/DEF-05 territory). For PERF-07 implementation purposes,
the Net regen fields should be written as `LifeRegenRecovery` (= `NetLifeRegen` when no
degen is active), and the complex degen routing can be deferred to a later TAIL chunk.

### Section 7: Leech Instances (CalcOffence.lua 3457–3488)

```lua
-- Helper function — computes (duration, instances) for a leech source:
local function getLeechInstances(amount, total)
    if total == 0 then
        return 0, 0
    end
    local duration = amount / total / data.misc.LeechRateBase
    --               ↑ amount = total leech from hit (absolute life/ES/mana)
    --               ↑ total = pool size (Life, ES, Mana)
    --               ↑ data.misc.LeechRateBase = per-second leech rate constant (default 0.02 = 2%/s)
    return duration, duration * hitRate
    --               ↑ instances = duration × hitRate (how many concurrent instances at this hit rate)
end
```

**Instant leech:**
```lua
-- InstantLifeLeech is a BASE % value (0–100)
output.LifeLeechInstantProportion =
    m_max(m_min(skillModList:Sum("BASE", cfg, "InstantLifeLeech") or 0, 100), 0) / 100
    -- ↑ clamp to [0, 100] then convert to fraction

if output.LifeLeechInstantProportion > 0 then
    output.LifeLeechInstant = output.LifeLeech * output.LifeLeechInstantProportion
    output.LifeLeech = output.LifeLeech * (1 - output.LifeLeechInstantProportion)
    -- Splits total leech: proportion is instant, remainder is over-time
end

output.LifeLeechDuration, output.LifeLeechInstances =
    getLeechInstances(output.LifeLeech, globalOutput.Life)
output.LifeLeechInstantRate = output.LifeLeechInstant * hitRate
-- ↑ globalOutput.Life = the player's pool, not the skill's localised output
```

### Section 8: Leech Rate Finalisation (CalcOffence.lua 3802–3843)

```lua
-- Per-instance leech rate = pool × LeechRateBase × skillMod("*LeechRate")
-- LifeLeechRate below the INC×More × base per-second:
output.LifeLeechInstanceRate =
    output.Life * data.misc.LeechRateBase
    * calcLib.mod(skillModList, skillCfg, "LifeLeechRate")
    -- calcLib.mod here uses the *skill* modlist and cfg, not player modDB

output.LifeLeechRate = output.LifeLeechInstances * output.LifeLeechInstanceRate

-- Immortal Ambition (keystone): life leech converts to ES leech
if skillModList:Flag(nil, "ImmortalAmbition") then
    output.EnergyShieldLeechRate = output.EnergyShieldLeechRate + output.LifeLeechRate
    output.LifeLeechRate = 0        -- life leech disabled
end

-- UnaffectedByNonInstantLifeLeech:
if skillModList:Flag(nil, "UnaffectedByNonInstantLifeLeech") then
    output.LifeLeechRate = 0
    output.LifeLeechInstances = 0
end

-- Final: add instant rate + min(non-instant, cap) × recoveryRateMod
output.LifeLeechRate =
    output.LifeLeechInstantRate
    + m_min(output.LifeLeechRate, output.MaxLifeLeechRate) * output.LifeRecoveryRateMod

output.EnergyShieldLeechRate =
    output.EnergyShieldLeechInstantRate
    + m_min(output.EnergyShieldLeechRate, output.MaxEnergyShieldLeechRate)
      * output.EnergyShieldRecoveryRateMod

output.ManaLeechRate =
    output.ManaLeechInstantRate
    + m_min(output.ManaLeechRate, output.MaxManaLeechRate) * output.ManaRecoveryRateMod

-- Gain rate = leech + on-hit (only in non-average/DPS mode)
if skillData.showAverage then
    output.LifeLeechGainPerHit = output.LifeLeechPerHit + output.LifeOnHit
    -- ... (per-hit display, not a chunk field)
else
    output.LifeLeechGainRate = output.LifeLeechRate + output.LifeOnHitRate
    output.EnergyShieldLeechGainRate = output.EnergyShieldLeechRate + output.EnergyShieldOnHitRate
    output.ManaLeechGainRate = output.ManaLeechRate + output.ManaOnHitRate
end
```

**Gotcha:** `calcLib.mod(skillModList, skillCfg, "LifeLeechRate")` uses the **skill**
modlist/cfg, not the player modDB. This is distinct from the player-level `*RecoveryRateMod`.
In Rust this means these computations belong in the offence calculation path (per-skill),
not in the global defence/perform path.

**Gotcha:** `m_min(output.LifeLeechRate, output.MaxLifeLeechRate)` — the leech rate is
capped at the player-level `MaxLifeLeechRate` computed in CalcDefence. The cap is applied
*before* multiplying by `LifeRecoveryRateMod`.

**Gotcha:** `output.MaxLifeLeechRate` in this context is `globalOutput.MaxLifeLeechRate`
(the player's cap), not a local skill output.

## Existing Rust Code

**File: `crates/pob-calc/src/calc/perform.rs`, lines 1108–1269**

The function `do_regen_recharge_leech` exists but is substantially incomplete and
structurally divergent from the Lua:

| Aspect | Status |
|--------|--------|
| `LifeRecoveryRateMod` | **Missing** — not written by perform.rs; written by defence.rs |
| `MaxLifeLeechRatePercent` | Partially present but deferred to defence.rs |
| `LifeRegen` | Partial — computes `(pct + flat) × recoveryInc × recoveryMore` but uses wrong modifier names |
| `ManaRegen` | Partial — applies both regen INC/More AND recovery INC/More separately, **double-applying** |
| `EnergyShieldRegen` | Missing from perform.rs (only computed in defence.rs) |
| `LifeDegen` | Partial — flat + percent only, missing tincture minimum |
| Zealot's Oath | Partial — handled in defence.rs but not perform.rs |
| ES recharge | Partial — in defence.rs but uses wrong stat name `"EnergyShieldRechargeRate"` instead of `"EnergyShieldRecharge"` |
| Override for ES recharge base | **Missing** |
| `WardRechargeDelay` | **Missing** entirely |
| `NetLifeRegen` | **Missing** |
| All leech rates | **Missing** from perform.rs; leech rate computation does not exist in any Rust file |

**File: `crates/pob-calc/src/calc/defence.rs`, lines 528–717**

The defence.rs has a parallel (and better) implementation for some of these fields:

| Aspect | Status |
|--------|--------|
| `calc_recovery_rates` (~530–543) | Correct for INC×More pattern; but queries `"{resource}RecoveryRateMod"` as stat name rather than `"*RecoveryRate"`. Check stat name. |
| `calc_leech_caps` (~545–641) | Mostly correct; missing `MaxLifeLeechRatePercent` for Mana and ES (only life has percent). The ES leech rate default is hardcoded to 10 — verify game constant. |
| `calc_regeneration` (~643–681) | Applies recovery_rate but fetches it with `get_output_f64` which defaults to 0.0 not 1.0 — **bug if recovery rate not set first**. Missing Pious Path and tincture degen. Missing `*RegenPercent` output. Missing `*RegenRecovery` intermediate output. |
| `calc_es_recharge` (~683–717) | Bug: queries `"EnergyShieldRechargeRate"` instead of `"EnergyShieldRecharge"`. Missing Override check. |
| WardRechargeDelay | **Missing** |
| Leech rates | **Missing** — must be in offence calculation path |

## What Needs to Change

1. **Fix ES recharge stat name** (`defence.rs::calc_es_recharge`):  
   Change `"EnergyShieldRechargeRate"` → `"EnergyShieldRecharge"` for INC and More queries.

2. **Add ES recharge Override** (`defence.rs::calc_es_recharge`):  
   Before computing `recharge_rate`, check `mod_db.override_value(None, output, "EnergyShieldRecharge")`.
   If present, use it as the base rate instead of `0.20`.

3. **Add `WardRechargeDelay`** (`defence.rs`):  
   ```rust
   let ward_delay_base = data.misc.ward_recharge_delay; // typically 2.0
   let ward_faster = mod_db.sum_cfg(Inc, "WardRechargeFaster", None, output);
   let ward_delay = ward_delay_base / (1.0 + ward_faster / 100.0);
   output.insert("WardRechargeDelay", ward_delay);
   ```

4. **Fix `calc_regeneration` default for `*RecoveryRateMod`** (`defence.rs`):  
   Replace `get_output_f64(&output, &recovery_stat).max(1.0)` with a proper fallback:
   the `.max(1.0)` currently hides the 0.0-when-missing bug but causes incorrect results
   when the actual mod is below 1.0 (e.g., `CannotRecoverLifeOutsideLeech` sets life to
   exactly 1.0 — correct; but mana at 0.5 would be clamped to 1.0 incorrectly). Fix
   by ensuring `calc_recovery_rates` always runs before `calc_regeneration` and
   initialises `*RecoveryRateMod` to 1.0.

5. **Add `*RegenPercent` output** (`defence.rs::calc_regeneration`):  
   After computing `regen`, also compute and write:
   ```rust
   let regen_recovery = regen; // (degen subtracted later for Net fields)
   let regen_pct = if pool > 0.0 { (regen_recovery / pool * 100.0 * 10.0).round() / 10.0 } else { 0.0 };
   output.insert(&format!("{resource}RegenPercent"), regen_pct);
   output.insert(&format!("{resource}RegenRecovery"), regen_recovery);
   ```

6. **Add `*Degen` tincture minimum** (`defence.rs::calc_regeneration` or a new helper):  
   ```rust
   let tincture_pct = mod_db.sum_cfg(Base, &format!("{resource}DegenPercentTincture"), ...);
   base_degen += (pool * tincture_pct / 100.0).max(tincture_pct);
   ```
   This is exotic but PoB includes it unconditionally.

7. **Fix Mana regen double-application** (`perform.rs::do_regen_recharge_leech`):  
   The function currently applies both `ManaRegen INC/More` and `ManaRecoveryRate INC/More`
   to mana regen. The Lua uses `calcLib.mod(modDB, nil, "ManaRegen")` which is only
   `(1 + Inc("ManaRegen")/100) * More("ManaRegen")`. The recovery rate multiplier is
   **not** applied in perform.rs — it is applied in defence.rs `calc_regeneration`.
   Remove the recovery rate application from perform.rs to avoid double-counting.

8. **Add Zealot's Oath to regen loop** (`defence.rs::calc_regeneration`):  
   Currently Zealot's Oath transfers `LifeRegen` to `EnergyShieldRegen` post-loop. The
   Lua also transfers `LifeRegenPercent` BASE mods and `LifeRegen` BASE mods into the
   modDB for ES before the ES iteration. Implementing this fully requires mutating modDB
   mid-loop (or computing conversions explicitly before the loop). The current approach of
   adding `life_regen` to `es_regen` post-loop is approximately correct but misses
   `LifeRegenPercent` conversion to `EnergyShieldRegenPercent` mods.

9. **Implement leech rates** (new function in offence.rs or a separate helper):  
   Leech rates (`LifeLeechRate`, `ManaLeechRate`, `EnergyShieldLeechRate`,
   `LifeLeechGainRate` etc.) are computed in CalcOffence's per-skill pass. They require:
   - `globalOutput.MaxLifeLeechRate` / `MaxManaLeechRate` / `MaxEnergyShieldLeechRate`
     (from defence.rs leech caps — already present)
   - `globalOutput.LifeRecoveryRateMod` etc. (from defence.rs recovery rates)
   - Per-skill `LifeLeechInstances`, `LifeLeechInstant`, `LifeLeechInstantRate` (from
     CalcOffence leech instance calculation — currently missing in Rust offence path)
   - `data.misc.LeechRateBase` (game constant, default 0.02)
   - `ImmortalAmbition` and `UnaffectedByNonInstantLifeLeech` flag handling

10. **Correct `field_groups.rs` entries for PERF-07**:  
    - Remove or rename: `"LifeRecoveryRate"`, `"ManaRecoveryRate"`, `"EnergyShieldRecoveryRate"`
      → they should be `"LifeRecoveryRateMod"`, `"ManaRecoveryRateMod"`, `"EnergyShieldRecoveryRateMod"`
    - Remove phantom fields: `"LifeDegenRate"`, `"LifeRecoveryRateTotal"`, `"ManaRecoveryRateTotal"`,
      `"EnergyShieldRechargeRecovery"`, `"WardRecharge"`
    - These do not appear in any oracle expected output

### Oracle Confirmation (realworld_phys_melee_slayer)

| Field | Expected Value | Current Rust Status |
|-------|---------------|---------------------|
| `LifeRegen` | 388.7 | Partial (wrong formula) |
| `LifeRegenPercent` | 7.8 | Missing |
| `ManaRegen` | 11.8 | Partial (double-applies recovery rate) |
| `ManaRegenPercent` | 1.8 | Missing |
| `EnergyShieldRegen` | 0 | Partial |
| `EnergyShieldRegenPercent` | 0 | Missing |
| `LifeDegen` | 0 | Partial |
| `MaxLifeLeechRate` | 992.2 | Present in defence.rs |
| `MaxLifeLeechRatePercent` | 20 | Present in defence.rs |
| `MaxManaLeechRate` | 134.4 | Present in defence.rs |
| `MaxEnergyShieldLeechRate` | 10.3 | Present in defence.rs |
| `LifeLeechRate` | 992.2 | Missing (offence path) |
| `ManaLeechRate` | 134.4 | Missing |
| `EnergyShieldLeechRate` | 0 | Missing |
| `LifeLeechGainRate` | 1000.978 | Missing |
| `ManaLeechGainRate` | 134.4 | Missing |
| `EnergyShieldLeechGainRate` | 0 | Missing |
| `LifeLeechDuration` | 5 | Missing |
| `ManaLeechDuration` | 5 | Missing |
| `EnergyShieldLeechDuration` | 0 | Missing |
| `LifeLeechInstances` | 21.945 | Missing |
| `ManaLeechInstances` | 21.945 | Missing |
| `EnergyShieldLeechInstances` | 0 | Missing |
| `LifeLeechInstantRate` | 0 | Missing |
| `EnergyShieldRecharge` | 34 | Wrong (wrong stat name) |
| `EnergyShieldRechargeDelay` | 2 | Present in defence.rs |
| `WardRechargeDelay` | 2 | Missing |
| `LifeRecoveryRateMod` | 1 | Present |
| `ManaRecoveryRateMod` | 1 | Present |
| `EnergyShieldRecoveryRateMod` | 1 | Present |
| `LifeOnHitRate` | 8.778 | Missing |
| `ManaOnHitRate` | 0 | Missing |
| `EnergyShieldOnHitRate` | 0 | Missing |
