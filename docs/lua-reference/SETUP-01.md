# SETUP-01: Item Mod Parsing & Slot Assignment

## Output Fields

This chunk writes **no `output[]` fields directly**.

- `crates/pob-calc/tests/field_groups.rs` marks SETUP-01 as a Tier-0 setup chunk with no direct output keys.
- `scripts/field_inventory_output.json` therefore has no field-to-line entries attributable to this chunk.
- In `CalcSetup.lua`, there are no `output.X = ...` writes in this section; effects are indirect via `env.itemModDB`, slot state, and item-derived side tables.

## Dependencies

- **None (chunk-order dependency):** this is a Tier-0 foundation chunk.
- Requires parsed build structures to exist (`build.itemsTab`, active item set, passive spec jewels, parsed item mods), but no prior parity chunk must run first.

## Lua Source

**File:** `third-party/PathOfBuilding/src/Modules/CalcSetup.lua`  
**Lines:** 679-1131 (item selection, slot assignment, item mod merge and special-item dispatch)  
**Commit:** `454eff8c85d24356d9b051d596983745ed367476`

## Annotated Lua

### 1) Slot iteration and effective item selection (lines 683-721)

```lua
for _, slot in ipairs(build.itemsTab.orderedSlots) do
    local slotName = slot.slotName

    if slotName == "Graft 1" or slotName == "Graft 2" then
        if not build.spec.treeVersion:find("3_27") then
            goto continue
        end
    end

    local item
    if slotName == override.repSlotName then
        item = override.repItem
    elseif override.repItem and override.repSlotName:match("^Weapon 1") and slotName:match("^Weapon 2") and ... then
        goto continue
    elseif slot.nodeId and override.spec then
        item = build.itemsTab.items[env.spec.jewels[slot.nodeId]]
    else
        item = build.itemsTab.items[slot.selItemId]
    end

    if slot.weaponSet and slot.weaponSet ~= (build.itemsTab.activeItemSet.useSecondWeaponSet and 2 or 1) then
        goto continue
    end
    if slot.weaponSet == 2 and build.itemsTab.activeItemSet.useSecondWeaponSet then
        slotName = slotName:gsub(" Swap","")
    end
    ...
end
```

Lua/Rust notes:

- `ipairs(...)` is ordered, 1-based iteration. Rust equivalent should preserve deterministic slot order (`Vec`) rather than `HashMap` iteration.
- `goto continue` is PoB's idiom for multi-guard `continue`.
- `a and b or c` in Lua is truthiness-based; in Rust use explicit `if/else` to avoid falsy-value edge cases.
- `slotName:gsub(" Swap", "")` mutates the local slot label for second-weapon-set projection.

### 2) Jewel-slot pre-pass and radius registration seed (lines 722-813)

```lua
if slot.nodeId then
    if not env.allocNodes[slot.nodeId] then
        goto continue
    elseif item then
        -- jewel limits, The Adorned multipliers, radius funcList registration,
        -- env.radiusJewelList and env.extraRadiusNodeList population
    end
end
items[slotName] = item
::continue::
```

Lua/Rust notes:

- `env.allocNodes[slot.nodeId]` is a map/set membership check; Rust equivalent is `HashSet::contains(&node_id)`.
- `items[slotName] = item` stores the effective item snapshot used by later passes (disablers + final merge pass).
- Radius-jewel registration here is setup scaffolding; full behavior is split into SETUP-05/08/09, but SETUP-01 still owns the slot-to-item staging loop.

### 3) Item-disabler chain resolution (lines 815-879)

```lua
local itemDisabled = {}
local itemDisablers = {}
... -- collect "CanNotUseItem" from tree and item mod tags
for slot in pairs(itemDisablers) do
    ... -- walk chain and detect cycles
    ... -- disable every other entry in chain
end
for slot in pairs(trueDisabled) do
    items[slot] = nil
end
```

Lua/Rust notes:

- Uses two directed maps (`disabler -> disabled`, `disabled -> disabler`) and alternating traversal to break chains deterministically.
- `pairs(...)` on tables is unordered; algorithm correctness cannot rely on ordering.
- Rust translation is best expressed with `HashMap<String, String>` and a `HashSet<String>` for visited/disabled.

### 4) Final pass: slot assignment + item mod merge + special-item dispatch (lines 884-1131)

```lua
for _, slot in ipairs(build.itemsTab.orderedSlots) do
    local slotName = slot.slotName
    local item = items[slotName]
    ... -- flask/tincture extraction and slot maps
    ... -- abyss socket gating / scale
    if item then
        env.player.itemList[slotName] = item
        local srcList = item.modList or (item.slotModList and item.slotModList[slot.slotNum]) or {}
        ... -- requirements table insert
        ... -- abyss jewel counters
        if item.type == "Shield" and NecromanticAegis then
            ...
        elseif (slotName == "Weapon 1" or slotName == "Weapon 2") and modDB.conditions["AffectedByEnergyBlade"] then
            ...
        elseif slotName == "Weapon 1" and item.name == "The Iron Mass, Gladius" then
            ...
        elseif slotName == "Weapon 1" and item.grantedSkills[1] and item.grantedSkills[1].skillId == "UniqueAnimateWeapon" then
            ...
        elseif item.name:match("Kalandra's Touch") then
            ...
        elseif item.type == "Quiver" and (...) then
            ...
        elseif ... then
            ...
        else
            env.itemModDB:ScaleAddList(srcList, scale)
        end
    end
end
```

Lua/Rust notes:

- `srcList` fallback chain (`modList` vs `slotModList[slotNum]` vs `{}`) is important for slotted contexts.
- `ScaleAddList` scales only numeric mod payloads; list/string mod values are passed through unchanged.
- Special-item chain is mutually exclusive (`if/elseif/.../else`); ordering matters.
- This block still does not write `output[]`; it mutates ModDb and actor state that downstream chunks consume.

## Existing Rust Code

**Primary file:** `crates/pob-calc/src/calc/setup.rs`

### What exists

- `add_item_mods` exists and handles core per-slot item parsing and mod insertion (`setup.rs:4011`).
- Special unique dispatch exists for the Lua chain (Necromantic Aegis, Energy Blade, The Iron Mass, Dancing Dervish, Kalandra's Touch, Widowhail, gloves/boots scaling) in `setup.rs:4208` onward.
- Local defence mods are separated from global mods and applied into per-slot armour storage (`setup.rs:4126`, `setup.rs:4627`).
- Weapon/shield extraction (`weapon_data1`, `weapon_data2`, `has_shield`, `dual_wield`) is implemented (`setup.rs:4677`).

### What's missing vs Lua

1. **Ordered-slot staging pass is missing:** Lua builds an intermediate `items` table from `orderedSlots` before merging; Rust iterates `item_set.slots` directly (unordered map iteration).
2. **Item-disabler chain algorithm is missing:** Lua lines 815-879 (`CanNotUseItem` / `DisablesItem`) are not mirrored in Rust `add_item_mods`.
3. **`env.player.itemList` equivalent is not populated as a slot map:** Lua preserves effective post-filter items by slot; Rust stores selected derived artifacts but not the full slot map.
4. **Item-granted skill extraction in this pass is missing:** Lua lines 702-711 push `env.grantedSkillsItems`; Rust does not build this list here.
5. **Weapon-set name normalization logic is absent:** Lua handles `weaponSet` gating and `" Swap"` slot renaming.

### What's wrong / drift risk

- **Iteration order drift:** Lua relies on `build.itemsTab.orderedSlots` in multiple places; Rust `HashMap` slot iteration may create non-deterministic behavior for order-sensitive edge logic.
- **SETUP boundary blending:** Rust `add_item_mods` currently includes SETUP-11 logic in the same function (`setup.rs:4696+`). Functional parity can still be correct, but this weakens chunk isolation and increases regression risk during targeted chunk work.
- **Partial first-pass parity:** Lua performs pre-merge slot filtering/disabling before any final merge path; Rust applies many merges without an equivalent pre-pass disabler resolution stage.

## What Needs to Change

1. Add an **ordered slot pre-pass** in Rust that mirrors Lua `items` staging (`CalcSetup.lua:683-813`), including effective item selection and slot-name normalization.
2. Implement Lua's **item disabler chain resolution** (`CalcSetup.lua:815-879`) before final mod merge.
3. Introduce a slot-keyed **effective item map** on env (or equivalent local staging passed through both passes) to match Lua's `env.player.itemList` semantics.
4. Mirror **item-granted skill collection** for this setup stage (`CalcSetup.lua:702-711`) or document/guarantee an equivalent later source of truth.
5. Keep SETUP-01 focused: isolate pure item parsing/slot assignment behavior from SETUP-11-specific counters to preserve chunk testability.
