# FIX-08: PERF-02 Mana Computation Bug

## Output Fields

Fields this chunk must fix:

| Field | Description |
|-------|-------------|
| `Mana` | Maximum mana pool |
| `ManaUnreserved` | Mana remaining after all reservations (can be negative) |
| `ManaUnreservedPercent` | `ManaUnreserved / Mana * 100` (can be negative) |
| `ManaReserved` | Reserved mana, capped at `Mana` |
| `ManaReservedPercent` | `min(total_reserved / Mana * 100, 100)` |

## Dependencies

- `PERF-01-attributes` — `Int` must be correct; `Int / 2` (floor) is added as BASE Mana in `doActorAttribsConditions`.
- `PERF-02-life-mana-es` — this chunk IS part of PERF-02; the Mana branch of `doActorLifeMana`.
- Item mods from all equipped items including **tree jewels** (see root-cause analysis below).

## Lua Source

**File:** `src/Modules/CalcPerform.lua`
**Commit:** `454eff8c85d24356d9b051d596983745ed367476`

Relevant sections:

| Function | Lines | Purpose |
|----------|-------|---------|
| `doActorAttribsConditions` | 135–517 | Computes attributes, adds `Int / 2` as Mana BASE mod |
| `doActorLifeMana` | 68–130 | Computes `output.Mana` using `calcLib.val` |
| `doActorLifeManaReservation` | 521–553 | Computes `ManaReserved`, `ManaUnreserved`, etc. |

## Annotated Lua

### Step 1 — Int-to-Mana bonus (CalcPerform.lua lines 506–512)

Called from `doActorAttribsConditions`, which runs *before* `doActorLifeMana`.

```lua
-- CalcPerform.lua:506-512
if not modDB:Flag(nil, "NoIntelligenceAttributeBonuses") then
    if not modDB:Flag(nil, "NoIntBonusToMana") then
        -- +1 BASE Mana per 2 Int (floored)
        -- Lua: m_floor = math.floor; output.Int is already computed and in output[]
        modDB:NewMod("Mana", "BASE", m_floor(output.Int / 2), "Intelligence")
        --                           ^^^^^^^^^^^^^^^^^^^^^^^^^
        --   Rust: (int_out / 2.0).floor()  as a BASE mod on the mod_db
    end
    if not modDB:Flag(nil, "NoIntBonusToES") then
        modDB:NewMod("EnergyShield", "INC", m_floor(output.Int / 10), "Intelligence")
    end
end
doActorLifeMana(actor)  -- called immediately after attribute bonuses are added
```

**Rust note:** This is correctly implemented in `perform.rs::do_actor_attribs_conditions`
(lines 266–279). The `mana_from_int` guard only adds if `mana_from_int > 0.0`, matching
Lua (would add 0 either way, but the guard avoids a no-op insert).

### Step 2 — Mana computation (CalcPerform.lua lines 108–128)

```lua
-- CalcPerform.lua:108-128
function doActorLifeMana(actor)
    local modDB = actor.modDB
    local output = actor.output
    local breakdown = actor.breakdown

    -- ManaConvertToArmour: percentage of Mana BASE that converts to Armour.
    -- Normally 0; only non-zero for specific items/keystones.
    local manaConv = modDB:Sum("BASE", nil, "ManaConvertToArmour")

    -- calcLib.val short-circuit (CalcTools.lua:32-38):
    --   if base == 0 then return 0
    --   else return base * (1 + inc/100) * more
    -- This means: if no BASE Mana mod exists, Mana stays 0 regardless of INC/MORE.
    -- Lua: output.Mana = round(...)
    --      round() is PoB's global function → standard rounding (Rust: .round())
    output.Mana = round(calcLib.val(modDB, "Mana") * (1 - manaConv / 100))
    --            ^^^^^  Lua: round() = standard round-half-up
    --                   Rust: .round() on f64

    -- Breakdown only (for UI display; always populate in Rust):
    local base = modDB:Sum("BASE", nil, "Mana")
    local inc  = modDB:Sum("INC",  nil, "Mana")
    local more = modDB:More(nil,        "Mana")
    -- Note: 'base', 'inc', 'more' here are only for the breakdown table.
    -- The actual Mana value is already computed above via calcLib.val.
    -- calcLib.val = base * (1 + inc/100) * more  (when base != 0)

    if breakdown then  -- Rust: always populate (remove this guard)
        if inc ~= 0 or more ~= 1 or manaConv ~= 0 then
            breakdown.Mana = { }
            breakdown.Mana[1] = s_format("%g ^8(base)", base)
            if inc ~= 0 then
                t_insert(breakdown.Mana, s_format("x %.2f ^8(increased/reduced)", 1 + inc/100))
            end
            if more ~= 1 then
                t_insert(breakdown.Mana, s_format("x %.2f ^8(more/less)", more))
            end
            if manaConv ~= 0 then
                t_insert(breakdown.Mana, s_format("x %.2f ^8(converted to Armour)", 1 - manaConv/100))
            end
            t_insert(breakdown.Mana, s_format("= %g", output.Mana))
        end
    end

    -- LowestOfMaximumLifeAndMaximumMana: used by some mods
    output.LowestOfMaximumLifeAndMaximumMana = m_min(output.Life, output.Mana)
    --                                          Rust: life.min(mana)
end
```

**Key Lua pattern — `calcLib.val`:**

```lua
-- CalcTools.lua:32-38
function calcLib.val(modStore, name, cfg)
    local baseVal = modStore:Sum("BASE", cfg, name)
    if baseVal ~= 0 then
        return baseVal * calcLib.mod(modStore, cfg, name)
        --               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        --               = (1 + Sum("INC", ...) / 100) * More(...)
    else
        return 0
    end
end
```

The short-circuit `if base == 0 → return 0` is important: if NO BASE Mana mod exists, the
result is 0 regardless of INC/MORE. The Rust `do_actor_life_mana` replicates this:

```rust
// perform.rs:513-517
let mana_pre_conv = if base == 0.0 {
    0.0
} else {
    base * (1.0 + inc / 100.0) * more
};
```

### Step 3 — Reservation (CalcPerform.lua lines 521–553)

```lua
-- CalcPerform.lua:521-553
function doActorLifeManaReservation(actor, addAura)
    local modDB = actor.modDB
    local output = actor.output

    for _, pool in pairs({"Life", "Mana"}) do
        local max = output[pool]   -- output.Mana (the computed pool value)
        local reserved
        if max > 0 then
            -- reserved_ManaBase = flat reserved mana (from flat-cost reservations)
            -- reserved_ManaPercent = % of pool reserved (from % reservations)
            -- m_ceil = math.ceil — percentage-based reservations always round UP
            reserved = (actor["reserved_"..pool.."Base"] or 0)
                       + m_ceil(max * (actor["reserved_"..pool.."Percent"] or 0) / 100)
            --           ^^^^^^ ceil: 1% of 629 = 6.29 → ceils to 7, not floors to 6

            -- ManaReserved is CAPPED at Mana (but reserved is still the raw total)
            output[pool.."Reserved"] = m_min(reserved, max)
            output[pool.."ReservedPercent"] = m_min(reserved / max * 100, 100)
            -- ManaUnreserved CAN BE NEGATIVE (when total reserved > max)
            output[pool.."Unreserved"] = max - reserved
            output[pool.."UnreservedPercent"] = (max - reserved) / max * 100
        else
            reserved = 0
        end
        -- GrantReservedPoolAsAura (FIX-06 territory) ...
    end
end
```

**Rust note:** The Rust `do_actor_life_mana_reservation` correctly uses `.ceil()` for
percentage-based reservations and does not clamp `ManaUnreserved`.

### Step 4 — Mana BASE source: class level formula (CalcSetup.lua line 478)

```lua
-- CalcSetup.lua:478
modDB:NewMod("Mana", "BASE", data.characterConstants["mana_per_level"], "Base",
    { type = "Multiplier", var = "Level", base = 34 })
-- Evaluation: value * floor(Level) + base = 6.0 * Level + 34
-- Level multiplier: modDB.multipliers["Level"] = max(1, min(100, characterLevel))
-- e.g. level 93: 6 * 93 + 34 = 592
```

**Rust note:** Rust's `add_class_base_stats` (setup.rs:692) computes `34.0 + 6.0 * level`
directly as a flat BASE mod — same result, different implementation. This is **correct**
and is not the source of the Mana bug.

## Root Cause Analysis

### Confirmed Bug: Tree Jewel Mods Not Applied to ModDB

**Evidence from chunk oracle test (`26/30 builds pass`, key failures):**

```
realworld_phys_melee_slayer: Mana expected=672, got=640 (gap=32)
realworld_coc_trigger:       Mana expected=874, got=842 (gap=32)
```

**Reverse-engineering the gap:**

For **phys_melee_slayer** (Marauder/Berserker, level 93, Int=96):
- Class base Mana: `34 + 6 × 93 = 592`
- Int bonus: `floor(96/2) = 48`
- Base total: `640`
- Watcher's Eye (tree jewel): `5% increased maximum Mana`
- Expected: `round(640 × 1.05) = round(672.0) = 672` ✓
- Rust gets: `round(640 × 1.00) = 640` (0% INC — WE mod missing)

For **coc_trigger** (Ranger/Raider, level 100, Int=293):
- Class base Mana: `34 + 6 × 100 = 634`
- Int bonus: `floor(293/2) = 146`
- Base total: `780`
- Tree passive `Spec:ManaInc = 8%` (nodes 21033 + 22618: 4%+4%)
- Watcher's Eye (tree jewel): `4% increased maximum Mana`
- Expected: `round(780 × 1.12) = round(873.6) = 874` ✓
- Rust gets: `round(780 × 1.08) = round(842.4) = 842` (only 8% passives, missing 4% WE)

Both builds use **Watcher's Eye** socketed in a **passive tree jewel socket**
(`<Socket nodeId="..." itemId="..."/>` in the XML). The consistent gap of 32 for both
builds is explained by the missing INC from Watcher's Eye in each case:
- Slayer: `640 × 0.05 = 32.0` (32 missing)
- CoC: `780 × 0.04 = 31.2 ≈ 32` (the ceiling effect of rounding gets the rest)

### Why Tree Jewel Mods Are Missing

In the XML, tree jewels are stored differently from gear jewels:

```xml
<!-- Tree jewel socket: lives in <Sockets> under <Spec> -->
<Sockets>
  <Socket nodeId="28475" itemId="12"/>  <!-- Watcher's Eye -->
</Sockets>

<!-- Gear jewel slot: NOT present in these oracle builds (no Jewel1/Jewel2 Slot entries) -->
<!-- <Slot name="Jewel1" itemId="..."/> -->
```

The XML parser stores tree jewels in `passive_spec.jewels` (a `HashMap<node_id, item_id>`),
NOT in `item_set.slots`.

`add_jewel_mods` (setup.rs:4611) iterates `item_set.slots` to find jewel-type slots, but
tree jewels (`passive_spec.jewels`) are never in `item_set.slots`. So `add_jewel_mods`
processes 0 jewels for these builds.

**PoB Lua path (CalcSetup.lua lines 722–811):**

```lua
-- CalcSetup.lua:722-811 (simplified)
for _, slot in ipairs(build.itemsTab.orderedSlots) do
    local item = itemList[slot.slotName]
    -- ...
    if slot.nodeId then
        -- Tree jewel socket: check if allocated
        if not env.allocNodes[slot.nodeId] then goto continue end
        -- If jewel has a radius → radiusJewelList (handled elsewhere)
        -- If NO radius → falls through to:
    end
    items[slotName] = item   -- added to items map regardless
    ::continue::
end

-- Later: item mods are added via env.itemModDB:ScaleAddList(srcList, scale)
-- This applies to ALL items in the `items` map, including tree jewels.
```

**The fix required (not implemented here — documentation only):** `add_jewel_mods` must
also iterate `build.passive_spec.jewels`, check that the socket node is allocated
(`env.alloc_nodes.contains(&socket_node_id)`), and if the jewel has no radius (i.e.,
`extract_radius_index(item)` returns `None`), add its mods to the player modDB just like
any other jewel.

### Other Failing Builds (Not the Mana Base Bug)

**aura_stacker** (`ManaUnreserved: expected -1372, got -1359`, diff=13):

- `Mana=629` is **correct** (passes). 
- The failure is in `ManaUnreserved`, implying total reserved is 13 too low.
- `expected raw_reserved = 629 + 1372 = 2001`, `got raw_reserved = 629 + 1359 = 1988`
- This is likely a percentage-reservation rounding or skill discovery issue in
  `accumulate_skill_reservations`. The aura_stacker uses many aura gems with percentage
  Mana reservations; a small rounding difference in one reservation propagates here.
- **Not directly related to the Mana base computation.** Consider this a PERF-04 residual.

**wand_occultist** (`ManaReserved: expected 1224, got 383`, diff=841):

- This is the known PERF-04 architecture problem documented in the spec (section 5.3).
- Gap of 841 = missing Blasphemy reservation. `SupportBloodMagic` is also missing from
  `gems.json`. This build cannot be fixed without PERF-04 + SETUP-02 work.
- **Out of scope for FIX-08.**

## Existing Rust Code

**File:** `crates/pob-calc/src/calc/perform.rs`, lines 384–552 (`do_actor_life_mana`)
**File:** `crates/pob-calc/src/calc/setup.rs`, lines 4611–4731 (`add_jewel_mods`)

### What Exists (Correct)

1. **`do_actor_life_mana`** (`perform.rs:384`): Correctly implements the Mana pool formula.
   - `calcLib.val` short-circuit (`if base == 0.0 { 0.0 }`) is present.
   - `ManaConvertToArmour` conversion is applied.
   - `round()` matches Lua.
   - Breakdown lines are always populated (correct Rust idiom).
   - `LowestOfMaximumLifeAndMaximumMana` is written.

2. **`do_actor_attribs_conditions`** (`perform.rs:266`): `Int / 2` BASE Mana bonus is
   correctly guarded by `NoIntelligenceAttributeBonuses` and `NoIntBonusToMana` flags.

3. **`do_actor_life_mana_reservation`** (`perform.rs:1028`): Percentage reservations
   use `.ceil()`. `ManaUnreserved` is not clamped (can be negative). Matches Lua.

4. **`add_class_base_stats`** (`setup.rs:683`): `base_mana = 34.0 + 6.0 * level` —
   equivalent to Lua's Multiplier-tagged mod. Correct.

### What Is Wrong

**`add_jewel_mods` (setup.rs:4611) does not process tree jewels.**

The function iterates `item_set.slots` for `slot.is_jewel()` entries. However, jewels
socketed in tree nodes use `<Socket nodeId="..." itemId="..."/>` XML and are stored in
`build.passive_spec.jewels`, NOT in `item_set.slots`. As a result, no tree jewel mods
(Watcher's Eye, rare/magic jewels in tree sockets) are ever added to the player modDB.

In PoB Lua, tree jewels flow through the same item processing loop as gear items:
`env.itemModDB:ScaleAddList(srcList, scale)` applies to all items including tree jewels.

### What is Missing

The `add_jewel_mods` function must be extended with a third pass over
`build.passive_spec.jewels`:

```
For each (socket_node_id, item_id) in build.passive_spec.jewels:
  1. Skip if socket_node_id is not in env.alloc_nodes (socket not allocated).
  2. Skip if item_id not in build.items.
  3. Get the item; check if it is a jewel type.
  4. If item.jewel_radius_index is Some(_) → skip (already handled by build_radius_jewel_list).
  5. Otherwise: parse all mod lines (implicits + explicits + crafted) and add to player modDB.
     - Apply The Adorned corrupted-jewel scaling if applicable (as in the existing 2nd pass).
     - Respect the jewel limit check (item.limit guard from CalcSetup.lua:738–748).
     - Respect the Charm subtype exclusion (CalcSetup.lua:1098 `base.subType ~= "Charm"`).
```

This mirrors CalcSetup.lua lines 722–811 (the `if slot.nodeId then` branch) combined
with the general item mod addition at line 1130.

## What Needs to Change

1. **Extend `add_jewel_mods` in `crates/pob-calc/src/calc/setup.rs`** to iterate
   `build.passive_spec.jewels` and add mods for non-radius tree jewels whose socket node
   is allocated. This is the primary fix for the `Mana` gap in phys_melee_slayer and
   coc_trigger.

2. **Add jewel limit tracking for tree jewels** (mirrors CalcSetup.lua:738–748):
   unique jewels with a `limit` field (e.g. Watcher's Eye "Limited to: 1") should not be
   double-counted if the same jewel occupies multiple sockets (rare, but correct behaviour).

3. **Charm subtype exclusion**: jewels with `base.subType == "Charm"` (PoE 2 Charms) must
   not go through the standard add-mods path when socketed in a tree node. This guard is
   at CalcSetup.lua:1098.

4. **The Adorned scale** already exists in the current second pass over `item_set.slots`.
   When adding the third pass for `passive_spec.jewels`, apply the same corrupted-jewel
   scaling logic (lines 4688–4712 in setup.rs) for tree-socketed corrupted jewels.

5. **(Out of scope for FIX-08)** The `aura_stacker` `ManaUnreserved` gap of 13 and the
   `wand_occultist` `ManaReserved` gap of 841 are reservation architecture problems
   (PERF-04). They will not be fixed by change #1 above. After applying change #1, the
   expected chunk oracle result should be:
   - `phys_melee_slayer`: 5/5 fields pass
   - `coc_trigger`: 5/5 fields pass  
   - `aura_stacker`: 3/5 fields pass (ManaUnreserved/ManaUnreservedPercent still wrong)
   - `wand_occultist`: 1/5 fields pass (ManaReserved/ManaUnreserved/etc. still wrong)
   - All other builds: 5/5 fields pass

## Lua Gotchas Specific to This Chunk

| Pattern | Lua | Rust | Notes |
|---------|-----|------|-------|
| `calcLib.val(modDB, "Mana")` | Short-circuits to 0 if BASE sum == 0 | `if base == 0.0 { 0.0 } else { base * (1 + inc/100) * more }` | Already correct in perform.rs |
| `round(x)` | Standard rounding (half-away-from-zero) | `x.round()` | Already correct |
| `m_ceil(max * pct / 100)` | Percentage reservation rounds UP | `.ceil()` | Already correct in reservation |
| `m_min(reserved, max)` | ManaReserved capped at Mana | `.min(mana)` | Already correct |
| `max - reserved` | ManaUnreserved CAN be negative | No clamping | Already correct |
| `(max - reserved) / max * 100` | ManaUnreservedPercent CAN be negative | No clamping | Already correct |
| Tree jewel mods | Applied via `items[slotName] = item` → `itemModDB:ScaleAddList` | **MISSING**: `passive_spec.jewels` not iterated in `add_jewel_mods` | **THIS IS THE BUG** |
