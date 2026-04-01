# DEF-03: Block and Spell Suppression

## Output Fields

Fields this chunk must write (from `field_groups.rs`):

| Field | Oracle present | Lua lines | Notes |
|-------|---------------|-----------|-------|
| `BlockChance` | 24/30 non-zero | 686, 688, 690, 693, 728 | Capped attack block chance |
| `BlockChanceMax` | 30/30 | 663, 665, 667 | Cap from modDB + `BlockChanceCap` |
| `BlockChanceOverCap` | 0/30 non-zero | 669, 694 | Always 0 in oracle; written as 0 init, updated in else-branch |
| `SpellBlockChance` | 14/30 non-zero | 704, 707, 712, 732 | Capped spell block chance |
| `SpellBlockChanceMax` | 30/30 | 699, 701 | Cap for spell block |
| `SpellBlockChanceOverCap` | 0/30 non-zero | 670, 709, 713 | Always 0 in oracle |
| `BlockEffect` | 30/30 (all=100) | 766 | `100 - Sum("BlockEffect")` |
| `BlockDuration` | 30/30 (all=0.363) | 2145, 2156 | Computed in stun section; server-tick aligned |
| `SpellSuppressionChance` | 1/30 non-zero | 1126 | Pure BASE sum, no INC; capped at 100 |
| `SpellSuppressionChanceOverCap` | 0/30 non-zero | 1158 | `max(0, total - 100)` |
| `SpellSuppressionEffect` | 30/30 (all=40) | 1127 | Base 40 + additive modifiers |

> **Notable:** `BlockChanceOverCap` and `SpellBlockChanceOverCap` are both 0 for all 30
> oracle builds (no build exceeds the 75% cap). `SpellSuppressionChanceOverCap` is also
> 0 for all 30 (no build exceeds 100%). `BlockDuration = 0.363` for all 30 (all have
> the base 0.35s stun/block duration rounded up to the nearest server tick at ~33ms).

## Dependencies

- `DEF-01-resistances` — not a hard dependency for block itself, but
  `EnergyShieldIncreasedByChanceToBlockSpellDamage` (line 813) injects
  `EnergyShield INC` using `output.SpellBlockChance`; this requires SpellBlockChance
  to be computed before the DEF-02 primary defence loop
- `DEF-02-armour-evasion-es-ward` — `ArmourDefense` writes at line 1559 use
  `spellSuppressionChance` which is computed after block; call order is:
  block → suppression → armour_defense
- Stun calculation (Rust `calc_stun`) — `BlockDuration` is computed together with
  `StunDuration` in the stun section; the stun section requires `Life`, `Mana`,
  `EnergyShield` from PERF-02 and DEF-02

## Lua Source

**Block section:** `CalcDefence.lua`, lines 662–770  
**Spell suppression section:** `CalcDefence.lua`, lines 1117–1158  
**Block duration (stun section):** `CalcDefence.lua`, lines 2143–2156

Commit: `454eff8c85d24356d9b051d596983745ed367476` (third-party/PathOfBuilding, heads/dev)

## Annotated Lua

### Data constants (from `Data.lua`)

```lua
data.misc.BlockChanceCap     = 90    -- absolute upper bound for any block chance max
data.misc.SuppressionChanceCap = 100 -- cap on spell suppression chance (100%)
data.misc.SuppressionEffect  = 40    -- base suppression effect (%), added to any mods
data.misc.ServerTickRate     = 1 / 0.033  -- ≈ 30.303 ticks/s (for duration alignment)
data.misc.StunBaseDuration   = 0.35  -- base stun/block duration in seconds

-- character_constants seeded via CalcSetup.lua:
--   modDB:NewMod("BlockChanceMax", "BASE", 75, "Base")         -- from "maximum_block_%"
--   modDB:NewMod("SpellBlockChanceMax", "BASE", 75, "Base")    -- from "base_maximum_spell_block_%"
```

### Section 1: Block chance max (lines 662–668)

```lua
-- BlockChanceCap = 90, but the BASE seeded value is 75, so this clamps to 75 normally.
-- The 90 cap only matters if "+X% to max Block Chance" mods push it above 75.
output.BlockChanceMax = m_min(modDB:Sum("BASE", nil, "BlockChanceMax"), data.misc.BlockChanceCap)
-- Rust: sum BASE "BlockChanceMax", clamp to 90.0 (BlockChanceCap)

-- Minion inheritance flags (rarely used, can inherit parent's max block):
if modDB:Flag(nil, "MaximumBlockAttackChanceIsEqualToParent") then
    output.BlockChanceMax = actor.parent.output.BlockChanceMax
elseif modDB:Flag(nil, "MaximumBlockAttackChanceIsEqualToPartyMember") then
    output.BlockChanceMax = actor.partyMembers.output.BlockChanceMax
end
-- Both are minion/totem mechanics; irrelevant for player actor in the 30 oracle builds.
```

### Section 2: Shield block base and block chance (lines 669–695)

```lua
-- Initialise overcap to 0; updated below when totalBlockChance > max.
output.BlockChanceOverCap = 0
output.SpellBlockChanceOverCap = 0

-- Collect shield/offhand block from Weapon 2 and Weapon 3 slots:
local baseBlockChance = 0
if actor.itemList["Weapon 2"] and actor.itemList["Weapon 2"].armourData then
    baseBlockChance = baseBlockChance + actor.itemList["Weapon 2"].armourData.BlockChance
end
if actor.itemList["Weapon 3"] and actor.itemList["Weapon 3"].armourData then
    baseBlockChance = baseBlockChance + actor.itemList["Weapon 3"].armourData.BlockChance
end
-- In Rust, item block chance is loaded via setup.rs:
--   db.add(Mod::new_base("ShieldBlockChance", ad.block, source))
-- So it arrives as a BASE mod named "ShieldBlockChance" (not "BlockChance").

output.ShieldBlockChance = baseBlockChance
-- Written for display only; not a DEF-03 field.

-- Necromantic Aegis keystone replaces the shield with minion's block source:
baseBlockChance = not env.keystonesAdded["Necromantic Aegis"]
                  and modDB:Override(nil, "ReplaceShieldBlock")
                  or baseBlockChance
-- If NOT Necromantic Aegis: use Override("ReplaceShieldBlock") if set, else keep baseBlockChance.
-- Lua `not X and Y or Z` pattern:
--   if NO NecroAegis: (true and Override or baseBlockChance)
--     → if Override is non-nil, use it; otherwise baseBlockChance
--   if HAS NecroAegis: (false and ...) → baseBlockChance
-- This handles items that entirely replace the shield block (e.g. Ivory Tower).
-- In Rust: if !necro_aegis { if let Some(v) = override("ReplaceShieldBlock") { base = v } }

-- Minion inherits player block from Necromantic Aegis:
baseBlockChance = actor == env.minion
                  and env.keystonesAdded["Necromantic Aegis"]
                  and env.player.modDB:Override(nil, "ReplaceShieldBlock")
                  or baseBlockChance
-- Only triggers for minion actors; irrelevant for player.

-- Attack block main branches:
if modDB:Flag(nil, "BlockAttackChanceIsEqualToParent") then
    output.BlockChance = m_min(actor.parent.output.BlockChance, output.BlockChanceMax)
elseif modDB:Flag(nil, "BlockAttackChanceIsEqualToPartyMember") then
    output.BlockChance = m_min(actor.partyMembers.output.BlockChance, output.BlockChanceMax)
elseif modDB:Flag(nil, "MaxBlockIfNotBlockedRecently") then
    -- Gladiator passive: always at max block (simulates best-case)
    output.BlockChance = output.BlockChanceMax
else
    -- Normal case:
    local totalBlockChance = (baseBlockChance + modDB:Sum("BASE", nil, "BlockChance"))
                             * calcLib.mod(modDB, nil, "BlockChance")
    -- baseBlockChance = shield/offhand block (from armourData)
    -- modDB:Sum("BASE", ..., "BlockChance") = flat block chance mods (passives, items)
    -- calcLib.mod = (1 + INC("BlockChance")/100) * More("BlockChance")
    -- NOTE: More mods DO exist for BlockChance: "Chance to Block Attack Damage is doubled"
    --       adds MORE = 100 → factor = (1 + 100/100) = 2.0
    output.BlockChance = m_min(totalBlockChance, output.BlockChanceMax)
    output.BlockChanceOverCap = m_max(0, totalBlockChance - output.BlockChanceMax)
end
```

**Gotcha — `calcLib.mod` for block includes `More`:** `calcLib.mod(modDB, nil, "BlockChance")` = `(1 + INC/100) * More`. There is at least one real `MORE` mod: *"Chance to Block Attack Damage is doubled"* maps to `BlockChance MORE 100` (factor = 2.0). The current Rust implementation applies only INC, not More — this is a bug.

**Gotcha — shield block comes from `ShieldBlockChance` mod, not `BlockChance`:** In PoB, item block chance is injected as `modDB:NewMod("ShieldBlockChance", ...)` from `CalcSetup`. The Lua then reads it as `actor.itemList["Weapon 2"].armourData.BlockChance` and adds it to `baseBlockChance`. In the Rust `setup.rs`, the same value is seeded as `Mod::new_base("ShieldBlockChance", ...)`. The `calc_block` function correctly queries `"ShieldBlockChance"` for the shield component.

**Gotcha — `BlockChanceMax` minimum:** The Lua computes `m_min(Sum("BlockChanceMax"), BlockChanceCap=90)`. The Rust does `if v == 0.0 { 75.0 } else { v }`. This is wrong: it should be `Sum("BlockChanceMax").min(90.0)`. The base of 75 comes from the modDB seed, so Sum will return 75 by default — but the `== 0.0` check is fragile (bypasses the cap clamp).

### Section 3: Projectile and spell block (lines 697–715)

```lua
-- Projectile block = attack block + extra projectile block, capped at BlockChanceMax:
output.ProjectileBlockChance = m_min(
    output.BlockChance + modDB:Sum("BASE", nil, "ProjectileBlockChance")
                         * calcLib.mod(modDB, nil, "BlockChance"),
    output.BlockChanceMax)
-- Note: "ProjectileBlockChance" BASE is multiplied by BlockChance's INC/More, not its own.
-- This means Projectile block uses the same INC/More multiplier as attack block.
-- Rust: uses (attack_block + extra_proj_block).min(block_max) — omits the INC/More factor
-- on extra_proj_block. Should be: (attack_block + extra_proj_block * (1+inc/100)*more).min(max)

-- Spell block max:
if modDB:Flag(nil, "SpellBlockChanceMaxIsBlockChanceMax") then
    -- Rumi's Concoction / Anvil-style items: spell block max = attack block max
    output.SpellBlockChanceMax = output.BlockChanceMax
else
    output.SpellBlockChanceMax = m_min(modDB:Sum("BASE", nil, "SpellBlockChanceMax"), data.misc.BlockChanceCap)
    -- Same pattern: clamp to 90, but base is 75 from seed
end

-- Spell block main branches:
if modDB:Flag(nil, "MaxSpellBlockIfNotBlockedRecently") then
    output.SpellBlockChance = output.SpellBlockChanceMax
    output.SpellProjectileBlockChance = output.SpellBlockChanceMax
elseif modDB:Flag(nil, "SpellBlockChanceIsBlockChance") then
    -- Rumi's or special items: spell block = attack block
    output.SpellBlockChance = output.BlockChance
    output.SpellProjectileBlockChance = output.ProjectileBlockChance
    output.SpellBlockChanceOverCap = output.BlockChanceOverCap
else
    -- Normal spell block:
    local totalSpellBlockChance = modDB:Sum("BASE", nil, "SpellBlockChance")
                                  * calcLib.mod(modDB, nil, "SpellBlockChance")
    -- NOTE: No baseBlockChance (shield block) here! Spell block uses only BASE mods,
    -- not the shield's inherent block chance.
    output.SpellBlockChance = m_min(totalSpellBlockChance, output.SpellBlockChanceMax)
    output.SpellBlockChanceOverCap = m_max(0, totalSpellBlockChance - output.SpellBlockChanceMax)
    output.SpellProjectileBlockChance = m_max(
        m_min(output.SpellBlockChance
              + modDB:Sum("BASE", nil, "ProjectileSpellBlockChance")
                * calcLib.mod(modDB, nil, "SpellBlockChance"),
              output.SpellBlockChanceMax),
        0)
end
```

**Gotcha — `SpellBlockChanceIsBlockChance` branch in Rust:** The Rust checks this flag and sets `spell_block = attack_block.min(spell_block_max)` — this is correct, but it **re-caps** using spell_block_max. The Lua sets `output.SpellBlockChance = output.BlockChance` (without re-capping). Since BlockChance is already capped at BlockChanceMax which could be different from SpellBlockChanceMax, the Rust re-cap may produce a different result. In the oracle builds, all block values are well below 75 so there's no divergence in practice.

**Gotcha — `SpellBlockChanceMax` default in Rust:** The Rust uses `if v == 0.0 { 75.0 }` as a default. Same issue as for `BlockChanceMax` — it should use `Sum(...).min(90.0)` since the base is seeded from modDB.

### Section 4: `CannotBlock` overrides (lines 727–734)

```lua
-- These flags zero out block chances unconditionally after all the above computation:
if modDB:Flag(nil, "CannotBlockAttacks") then
    output.BlockChance = 0
    output.ProjectileBlockChance = 0
end
if modDB:Flag(nil, "CannotBlockSpells") then
    output.SpellBlockChance = 0
    output.SpellProjectileBlockChance = 0
end
-- The Rust checks CannotBlockAttacks at the top of the branch (before computing total),
-- which is semantically equivalent but different ordering from the Lua.
-- CannotBlockSpells is not separately checked in the Rust (it handles it via the spell
-- block calculation being bypassed when flag is set). This matches for the oracle builds.
```

### Section 5: `BlockEffect` and `ShowBlockEffect` (lines 766–769)

```lua
-- "BlockEffect" BASE mods represent "% of damage taken from blocked hits".
-- The output field is the INVERSE: "% of damage blocked/negated".
-- With no mods: Sum("BlockEffect") = 0 → output.BlockEffect = 100 (block all damage).
-- With Glancing Blows / "You take 65% of Damage from Blocked Hits":
--   Sum("BlockEffect") = 65 → output.BlockEffect = 100 - 65 = 35

output.BlockEffect = 100 - modDB:Sum("BASE", nil, "BlockEffect")
-- Rust: must use: 100.0 - mod_db.sum(Base, "BlockEffect")
-- NOT a hardcoded flag check for GlancingBlows

if output.BlockEffect ~= 0 then
    -- This fires whenever BlockEffect != 0, which is ALWAYS (100 != 0 by default).
    output.ShowBlockEffect = true
    -- In Rust: always set ShowBlockEffect = true (condition is always met with default 100)
    output.DamageTakenOnBlock = 100 - output.BlockEffect
    -- = 100 - 100 = 0 for normal block; = 100 - 35 = 65 for Glancing Blows
end
```

**Critical bug in Rust:** The Rust `calc_block` hardcodes:
```rust
let block_effect = if env.player.mod_db.flag_cfg("GlancingBlows", None, &output) {
    65.0
} else {
    100.0
};
```
This is doubly wrong:
1. `GlancingBlows` is a *keystone flag* that causes a "You take 65% of Damage from Blocked Hits" mod to be seeded as `BlockEffect BASE 65`. The correct approach is to sum the `BlockEffect` BASE mod: `100 - Sum("BlockEffect")`.
2. The Rust never writes `ShowBlockEffect = true`, but all 30 oracle builds have `ShowBlockEffect: true` because `output.BlockEffect = 100 ≠ 0`.

### Section 6: Effective block (lucky/unlucky, lines 735–765)

The Lua applies lucky/unlucky modifiers and `Unexciting` to compute
`output.Effective{BlockType}` for all four block types, but these are **not** in the
DEF-03 `field_groups.rs`. They are computed for EHP and display purposes.

```lua
for _, blockType in ipairs({"BlockChance","ProjectileBlockChance","SpellBlockChance","SpellProjectileBlockChance"}) do
    -- Apply enemy reduceEnemyBlock in effective mode:
    output["Effective"..blockType] = env.mode_effective
        and m_max(output[blockType] - enemyDB:Sum("BASE", nil, "reduceEnemyBlock"), 0)
        or output[blockType]
    -- In non-effective mode: Effective = same as raw

    local blockRolls = 0
    if env.mode_effective then
        if modDB:Flag(nil, blockType.."IsLucky") then blockRolls = blockRolls + 1 end
        if modDB:Flag(nil, blockType.."IsUnlucky") then blockRolls = blockRolls - 1 end
        if modDB:Flag(nil, "ExtremeLuck") then blockRolls = blockRolls * 2 end
    end
    -- EHP unlucky config:
    if env.configInput.EHPUnluckyWorstOf and env.configInput.EHPUnluckyWorstOf ~= 1 then
        blockRolls = -env.configInput.EHPUnluckyWorstOf / 2
    end
    if blockRolls ~= 0 then
        local blockChance = output["Effective"..blockType] / 100
        if modDB:Flag(nil, "Unexciting") then
            -- Unexciting: rolls 3 times, keeps median → 3p² - 2p³
            output["Effective"..blockType] = (3 * blockChance^2 - 2 * blockChance^3) * 100
        elseif blockRolls > 0 then
            -- Lucky: (1 - (1-p)^(rolls+1)) * 100
            output["Effective"..blockType] = (1 - (1 - blockChance)^(blockRolls + 1)) * 100
        else
            -- Unlucky: p^|rolls| * chance
            output["Effective"..blockType] = blockChance^m_abs(blockRolls) * output["Effective"..blockType]
        end
    end
end
output.EffectiveAverageBlockChance = (output.EffectiveBlockChance + output.EffectiveProjectileBlockChance
                                     + output.EffectiveSpellBlockChance + output.EffectiveSpellProjectileBlockChance) / 4
```

**Gotcha — Rust lucky/unlucky implementation:** The Rust uses named flags `"LuckyBlock"` and `"UnluckyBlock"`, but the Lua checks `blockType.."IsLucky"` (e.g. `"BlockChanceIsLucky"`). These are different flag names. Additionally, the Rust `EffectiveAverageBlockChance` is computed as `(eff_block + eff_spell_block) / 2.0` — the Lua averages all four block types (attack, projectile, spell, spell-projectile) divided by 4.

### Section 7: Spell suppression (lines 1117–1158)

```lua
local spellSuppressionChance = modDB:Sum("BASE", nil, "SpellSuppressionChance")
-- RAW BASE sum only — no INC multiplier applied. Suppression chance is purely additive.
-- Sources: boot enchant, Acrobatics ascendancy nodes, item mods.

local totalSpellSuppressionChance = modDB:Override(nil, "SpellSuppressionChance")
                                    or spellSuppressionChance
-- Override takes precedence (e.g. "Spell Suppression Chance is 100%")

-- Acrobatics conversion: SpellSuppressionChance / 2 → SpellDodgeChance
if modDB:Flag(nil, "ConvertSpellSuppressionToSpellDodge") then
    modDB:NewMod("SpellDodgeChance", "BASE", spellSuppressionChance / 2, "Acrobatics")
    -- NOTE: uses the pre-override value (spellSuppressionChance, not total)
end

output.SpellSuppressionChance = m_min(totalSpellSuppressionChance, data.misc.SuppressionChanceCap)
-- SuppressionChanceCap = 100; m_min clamps to 100%

output.SpellSuppressionEffect = m_max(data.misc.SuppressionEffect + modDB:Sum("BASE", nil, "SpellSuppressionEffect"), 0)
-- Base effect = 40 (data.misc.SuppressionEffect)
-- "SpellSuppressionEffect" BASE mods ADD to the base (positive or negative)
-- e.g. "+10% to spell suppression effect" → modDB adds 10 → output = 40+10 = 50
-- Clamped to 0 minimum (cannot have negative effect)
-- Rust uses default 50% which is wrong — should be 40%

output.SpellSuppressionChanceOverCap = m_max(0, totalSpellSuppressionChance - data.misc.SuppressionChanceCap)
-- Overcap = amount above 100%; normal builds: 0
```

**Gotcha — no INC for suppression chance:** `SpellSuppressionChance` is a **pure BASE additive** stat. There are no INC mods for it in PoB's current game data. The Rust incorrectly applies `base * (1 + inc/100)` which would give wrong results if any INC mod for `SpellSuppressionChance` were seeded.

**Gotcha — suppression effect base is 40, not 50:** `data.misc.SuppressionEffect = 40`. The Rust defaults to `50.0` when `effect_base == 0.0`. This produces `SpellSuppressionEffect = 50` for all 30 oracle builds, but the correct answer is `40`. This is a confirmed discrepancy that can be verified with the oracle (all 30 builds expected to have `SpellSuppressionEffect = 40`, but Rust produces `50`).

**Gotcha — `SpellSuppressionChanceOverCap` not written by Rust:** The Rust `calc_spell_suppression` function does not write this field at all.

### Section 8: Block duration (lines 2143–2156, in stun section)

`BlockDuration` is computed together with `StunDuration` in the stun section of
`calcs.defence`, deep inside a conditional that requires the enemy damage estimate.
The formula is the same as `StunDuration` but with a **combined** recovery rate that
includes both `StunRecovery` and `BlockRecovery` INC:

```lua
if output.StunAvoidChance >= 100 then
    output.BlockDuration = 0
    -- Player is stun-immune → can't be stunned → block recovery time is 0
else
    local stunDuration = (1 + modDB:Sum("INC", nil, "StunDuration") / 100)
    local baseStunDuration = data.misc.StunBaseDuration  -- = 0.35 seconds
    local stunRecovery = (1 + modDB:Sum("INC", nil, "StunRecovery") / 100)
    local stunAndBlockRecovery = (1 + modDB:Sum("INC", nil, "StunRecovery", "BlockRecovery") / 100)
    -- NOTE: StunDuration uses only "StunRecovery" for divisor
    -- BlockDuration uses "StunRecovery" + "BlockRecovery" combined (vararg sum)

    -- StunDuration = ceil(0.35 * duration / stunRecovery * tickRate) / tickRate
    output.StunDuration = m_ceil(baseStunDuration * stunDuration / stunRecovery * data.misc.ServerTickRate) / data.misc.ServerTickRate

    output.BlockDuration = m_ceil(baseStunDuration * stunDuration / stunAndBlockRecovery * data.misc.ServerTickRate) / data.misc.ServerTickRate
    -- Server-tick alignment: ceil to next tick (33ms granularity)
    -- 0.35 * 1.0 / 1.0 * 30.303 = 10.606... → ceil = 11 ticks → 11 / 30.303 ≈ 0.363s
end
```

**Formula for oracle value 0.363:**
- `baseStunDuration = 0.35`
- `stunDuration = 1.0` (no INC StunDuration mods in oracle builds)
- `stunAndBlockRecovery = 1.0` (no INC StunRecovery or BlockRecovery)
- `ServerTickRate = 1/0.033 ≈ 30.303`
- `ceil(0.35 * 1.0 / 1.0 * 30.303) = ceil(10.606) = 11`
- `11 / 30.303 ≈ 0.363` ✓

**Gotcha — `BlockDuration` uses combined `StunRecovery + BlockRecovery`:** The Lua uses `modDB:Sum("INC", nil, "StunRecovery", "BlockRecovery")` which sums INC from both stat names. `StunDuration` uses only `"StunRecovery"`. Building up `BlockRecovery` (faster block recovery from Gladiator ascendancy) reduces `BlockDuration` but not `StunDuration`.

**Gotcha — server-tick alignment uses `m_ceil`:** Duration is ceiled to the next tick, not rounded. `m_ceil(x * ServerTickRate) / ServerTickRate`. The Rust `calc_stun` computes `StunDuration` without server-tick alignment at all: `0.35 / (1 + recovery/100) * (1 + duration/100)` — this will produce `0.35` instead of `0.363` and does not compute `BlockDuration`.

## Existing Rust Code

**File:** `crates/pob-calc/src/calc/defence.rs`  
- **`calc_block`**: lines 174–333  
- **`calc_spell_suppression`**: lines 430–475  
- **`calc_stun`** (contains BlockDuration): lines 942–1010

### Status table

| Feature | Rust status |
|---------|-------------|
| `BlockChanceMax` seeding (from modDB, capped at 90) | ⚠️ Wrong — Rust uses `if v==0.0 { 75.0 }` instead of `Sum.min(90.0)`. Correct for oracle (seeded base is 75), but wrong when no mods exist. |
| `BlockChanceMax` parent/partyMember inheritance | ❌ Missing |
| Shield block from armourData | ✅ Correct (seeded as `ShieldBlockChance` in setup.rs) |
| Necromantic Aegis override | ❌ Missing |
| `MaxBlockIfNotBlockedRecently` → max block | ❌ Missing |
| `BlockChance` = parent/partyMember | ❌ Missing |
| Attack block total = (shield + flat) × INC × More | ⚠️ Missing `More` — applies INC but not `More("BlockChance")` |
| `BlockChanceOverCap` | ✅ Computed (implicitly: `max_block` is capped, so `attack_block` > `block_max` can't happen unless `More` is involved) |
| `ProjectileBlockChance` extra proj block applies block's INC/More | ❌ Missing — Rust adds `extra_proj_block` without scaling |
| `SpellBlockChanceMax` max (from modDB, capped at 90) | ⚠️ Same `==0.0` issue as `BlockChanceMax` |
| `SpellBlockChanceMaxIsBlockChanceMax` flag | ✅ Present |
| `MaxSpellBlockIfNotBlockedRecently` | ❌ Missing |
| `SpellBlockChanceIsBlockChance` flag | ✅ Present (but re-caps with spell_block_max, Lua doesn't) |
| Spell block = BASE × INC × More | ⚠️ Missing `More` |
| `SpellProjectileBlockChance` with block's INC/More | ❌ Missing multiplier |
| `CannotBlockAttacks` | ✅ Present (checked at top of branch) |
| `CannotBlockSpells` | ✅ Present (checked at top of spell block branch) |
| **`BlockEffect = 100 - Sum("BlockEffect")`** | ❌ **Wrong** — hardcodes `GlancingBlows` check; should read `BlockEffect` BASE mod |
| **`ShowBlockEffect`** | ❌ **Missing** — Rust never writes `ShowBlockEffect`; all 30 oracle builds expect `true` |
| `DamageTakenOnBlock = 100 - BlockEffect` | ✅ Formula correct (but inputs wrong) |
| Lucky/unlucky block (per-type flag names) | ❌ Wrong flag names (`LuckyBlock`/`UnluckyBlock` vs `BlockChanceIsLucky` etc.) |
| `EffectiveAverageBlockChance` (avg of 4 types) | ❌ Wrong — Rust averages 2 (attack + spell), Lua averages all 4 |
| Enemy `reduceEnemyBlock` applied to Effective | ❌ Missing |
| Unexciting keystone on block | ❌ Missing |
| **Spell suppression: no INC** | ❌ **Wrong** — Rust applies `base × (1 + INC/100)`; Lua uses only BASE |
| Spell suppression Override check | ❌ Missing |
| **`SpellSuppressionEffect` default = 40** | ❌ **Wrong** — Rust defaults to `50.0`; correct default is `40` |
| `SpellSuppressionEffect` = base 40 + additive mods | ❌ Wrong (wrong base, wrong formula: should be `max(40 + Sum("SpellSuppressionEffect"), 0)`) |
| **`SpellSuppressionChanceOverCap`** | ❌ **Missing** — not written at all |
| Suppression lucky/unlucky (per-type flag) | ❌ Wrong flag names (`LuckySuppression`/`UnluckySuppression` vs `SpellSuppressionChanceIsLucky`) |
| `ConvertSpellSuppressionToSpellDodge` | ❌ Missing |
| `CannotBeSuppressed` (enemy flag) on EffectiveSuppression | ❌ Missing |
| **`BlockDuration`** | ❌ **Missing** — `calc_stun` writes `StunDuration` but not `BlockDuration` |
| `BlockDuration` server-tick alignment (m_ceil) | ❌ Missing — `StunDuration` also lacks this |
| `BlockDuration` uses `StunRecovery + BlockRecovery` | ❌ Missing |

### Oracle accuracy analysis

**Confirmed producing wrong values for all 30 builds:**
- `SpellSuppressionEffect`: Rust = 50, Oracle = 40
- `ShowBlockEffect`: Rust = not written (absent), Oracle = `true`
- `BlockDuration`: Rust = not written (absent), Oracle = 0.363

**Currently correct for all 30 builds (by coincidence):**
- `BlockChance`, `BlockChanceMax`, `BlockChanceOverCap`: correct because no `More` mods on block in any oracle build
- `SpellBlockChance`, `SpellBlockChanceMax`: correct for same reason
- `BlockEffect`: Rust outputs 100 via hardcoded branch, matches oracle. But formula is wrong.

## What Needs to Change

1. **Fix `BlockChanceMax` computation** (`calc_block`):
   ```rust
   const BLOCK_CHANCE_CAP: f64 = 90.0; // data.misc.BlockChanceCap
   let block_max = mod_db.sum_cfg(Base, "BlockChanceMax", None, output).min(BLOCK_CHANCE_CAP);
   ```
   Remove the `if v == 0.0 { 75.0 }` fallback; the default comes from the seeded modDB BASE value.

2. **Add `More` to attack block formula** (`calc_block`):
   ```rust
   let more_block = mod_db.more_cfg("BlockChance", None, output);
   let total = (shield_block + base_block) * (1.0 + inc_block / 100.0) * more_block;
   ```

3. **Apply block's INC/More to extra projectile block** (`calc_block`):
   ```rust
   let proj_multiplier = (1.0 + inc_block / 100.0) * more_block;
   let proj_block = (attack_block + extra_proj_block * proj_multiplier).min(block_max).max(0.0);
   ```

4. **Add `MaxBlockIfNotBlockedRecently` branch** (`calc_block`):
   ```rust
   } else if mod_db.flag_cfg("MaxBlockIfNotBlockedRecently", None, output) {
       block_max
   ```

5. **Fix `BlockEffect` formula** (`calc_block`):
   ```rust
   let block_effect = 100.0 - mod_db.sum_cfg(Base, "BlockEffect", None, output);
   env.player.set_output("BlockEffect", block_effect);
   env.player.set_output("ShowBlockEffect", true);  // always true when BlockEffect != 0
   // BlockEffect default = 100 (no mods), so ShowBlockEffect = true for all builds
   let damage_taken = 100.0 - block_effect;
   env.player.set_output("DamageTakenOnBlock", damage_taken);
   ```

6. **Fix lucky/unlucky block flag names** (`calc_block`):
   The Lua checks `blockType.."IsLucky"` (e.g. `"BlockChanceIsLucky"`), not a single `"LuckyBlock"` flag. The Rust needs per-type lucky checks:
   ```rust
   for block_type in ["BlockChance", "ProjectileBlockChance", "SpellBlockChance", "SpellProjectileBlockChance"] {
       let lucky = mod_db.flag_cfg(&format!("{block_type}IsLucky"), None, output);
       let unlucky = mod_db.flag_cfg(&format!("{block_type}IsUnlucky"), None, output);
       // ...
   }
   ```

7. **Fix `EffectiveAverageBlockChance`** (`calc_block`):
   Average all four effective block types, not just attack + spell:
   ```rust
   let avg = (eff_attack + eff_proj + eff_spell + eff_spell_proj) / 4.0;
   ```

8. **Fix spell suppression formula** (`calc_spell_suppression`):
   ```rust
   const SUPPRESSION_CHANCE_CAP: f64 = 100.0;
   let base = mod_db.sum_cfg(Base, "SpellSuppressionChance", None, output);
   let total = mod_db.override_value("SpellSuppressionChance", None, output).unwrap_or(base);
   let chance = total.min(SUPPRESSION_CHANCE_CAP);
   env.player.set_output("SpellSuppressionChance", chance);
   env.player.set_output("SpellSuppressionChanceOverCap", (total - SUPPRESSION_CHANCE_CAP).max(0.0));
   ```
   Remove the INC application from suppression chance.

9. **Fix `SpellSuppressionEffect` default and formula** (`calc_spell_suppression`):
   ```rust
   const SUPPRESSION_EFFECT_BASE: f64 = 40.0; // data.misc.SuppressionEffect
   let effect_mods = mod_db.sum_cfg(Base, "SpellSuppressionEffect", None, output);
   let effect = (SUPPRESSION_EFFECT_BASE + effect_mods).max(0.0);
   env.player.set_output("SpellSuppressionEffect", effect);
   ```

10. **Fix suppression lucky/unlucky flag names** (`calc_spell_suppression`):
    ```rust
    let lucky = mod_db.flag_cfg("SpellSuppressionChanceIsLucky", None, output);
    let unlucky = mod_db.flag_cfg("SpellSuppressionChanceIsUnlucky", None, output);
    ```

11. **Add `BlockDuration` computation** (`calc_stun` in `defence.rs`):
    ```rust
    const SERVER_TICK_RATE: f64 = 1.0 / 0.033; // ≈ 30.303
    const STUN_BASE_DURATION: f64 = 0.35;
    
    let block_recovery_inc = mod_db.sum_cfg(Inc, "StunRecovery", None, output)
        + mod_db.sum_cfg(Inc, "BlockRecovery", None, output);
    // Note: query separately and add, or query with multi-stat if supported
    let stun_and_block_recovery = 1.0 + block_recovery_inc / 100.0;
    let block_duration_raw = STUN_BASE_DURATION * stun_duration_mult / stun_and_block_recovery;
    let block_duration = (block_duration_raw * SERVER_TICK_RATE).ceil() / SERVER_TICK_RATE;
    env.player.set_output("BlockDuration", block_duration);
    ```
    Also fix `StunDuration` to use server-tick alignment:
    ```rust
    let stun_duration_raw = STUN_BASE_DURATION * stun_duration_mult / stun_recovery_mult;
    let stun_duration = (stun_duration_raw * SERVER_TICK_RATE).ceil() / SERVER_TICK_RATE;
    ```

## Oracle Confirmation (all 30 builds)

All oracle builds have the default block/suppression values unless noted below.
Builds not listed use defaults: BlockEffect=100, BlockDuration=0.363, SpellSuppressionEffect=40.

| Build | BlockChance | BlockChanceMax | BlockChanceOverCap | SpellBlockChance | SpellBlockChanceMax | BlockEffect | BlockDuration | SpellSuppressionChance | SpellSuppressionEffect |
|-------|------------|----------------|-------------------|-----------------|---------------------|-------------|---------------|----------------------|----------------------|
| aura_stacker | 25 | 75 | 0 | 5 | 75 | 100 | 0.363 | 0 | 40 |
| bleed_gladiator | 25 | 75 | 0 | 0 | 75 | 100 | 0.363 | 0 | 40 |
| bow_deadeye | 0 | 75 | 0 | 0 | 75 | 100 | 0.363 | 14 | 40 |
| champion_impale | 25 | 75 | 0 | 0 | 75 | 100 | 0.363 | 0 | 40 |
| ci_lowlife_es | 25 | 75 | 0 | 5 | 75 | 100 | 0.363 | 0 | 40 |
| cluster_jewel | 25 | 75 | 0 | 5 | 75 | 100 | 0.363 | 0 | 40 |
| coc_trigger | 20 | 75 | 0 | 0 | 75 | 100 | 0.363 | 0 | 40 |
| cwc_trigger | 25 | 75 | 0 | 5 | 75 | 100 | 0.363 | 0 | 40 |
| dot_caster_trickster | 25 | 75 | 0 | 5 | 75 | 100 | 0.363 | 0 | 40 |
| dual_wield | 20 | 75 | 0 | 0 | 75 | 100 | 0.363 | 0 | 40 |
| ele_melee_raider | 0 | 75 | 0 | 0 | 75 | 100 | 0.363 | 0 | 40 |
| flask_pathfinder | 0 | 75 | 0 | 0 | 75 | 100 | 0.363 | 0 | 40 |
| ignite_elementalist | 25 | 75 | 0 | 5 | 75 | 100 | 0.363 | 0 | 40 |
| max_block_gladiator | 30 | 75 | 0 | 0 | 75 | 100 | 0.363 | 0 | 40 |
| mine_saboteur | 25 | 75 | 0 | 5 | 75 | 100 | 0.363 | 0 | 40 |
| minion_necromancer | 25 | 75 | 0 | 5 | 75 | 100 | 0.363 | 0 | 40 |
| mom_eb | 25 | 75 | 0 | 5 | 75 | 100 | 0.363 | 0 | 40 |
| phys_melee_slayer | 30 | 75 | 0 | 0 | 75 | 100 | 0.363 | 0 | 40 |
| phys_to_fire_conversion | 25 | 75 | 0 | 5 | 75 | 100 | 0.363 | 0 | 40 |
| poison_pathfinder | 14 | 75 | 0 | 0 | 75 | 100 | 0.363 | 0 | 40 |
| rf_juggernaut | 25 | 75 | 0 | 0 | 75 | 100 | 0.363 | 0 | 40 |
| shield_1h | 25 | 75 | 0 | 0 | 75 | 100 | 0.363 | 0 | 40 |
| spectre_summoner | 25 | 75 | 0 | 5 | 75 | 100 | 0.363 | 0 | 40 |
| spell_caster_inquisitor | 25 | 75 | 0 | 5 | 75 | 100 | 0.363 | 0 | 40 |
| timeless_jewel | 25 | 75 | 0 | 0 | 75 | 100 | 0.363 | 0 | 40 |
| totem_hierophant | 25 | 75 | 0 | 5 | 75 | 100 | 0.363 | 0 | 40 |
| trap_saboteur | 25 | 75 | 0 | 5 | 75 | 100 | 0.363 | 0 | 40 |
| triple_conversion | 20 | 75 | 0 | 0 | 75 | 100 | 0.363 | 0 | 40 |
| two_handed | 0 | 75 | 0 | 0 | 75 | 100 | 0.363 | 0 | 40 |
| wand_occultist | 24 | 75 | 0 | 0 | 75 | 100 | 0.363 | 0 | 40 |

> The 14 builds with `SpellBlockChance = 5` are builds where `SpellBlockChanceIsBlockChance`
> is set and `BlockChance = 0` (no shield), but they have a shield giving `SpellBlockChance`
> BASE = 5 directly.

> `BlockDuration = 0.363` for all 30: confirms server-tick alignment formula is needed.
> Rust currently produces no value (field missing entirely) for `BlockDuration`.

> `SpellSuppressionEffect = 40` for all 30: confirms Rust's default of 50 is wrong for
> all oracle builds. This is a clear oracle failure.
