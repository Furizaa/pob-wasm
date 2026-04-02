# SETUP-16: Special Unique Item Handling

## Output Fields

This chunk writes **no direct output fields**. It modifies how item mods are
dispatched into `env.aegisModList`, `env.theIronMass`, `env.weaponModList1`,
and the player `modDB`/`itemModDB`, which downstream calculations then read.
Side-effects include:

- Setting `env.aegisModList` — a separate mod list for minions (Necromantic Aegis)
- Setting `env.theIronMass` — a separate mod list for animated weapons (The Iron Mass)
- Setting `env.weaponModList1` — a separate mod list for dancing dervish
- Replacing `env.player.itemList[slotName]` with a synthetic item (Energy Blade)
- Setting multipliers `WidowHailMultiplier` and `CorruptedMagicJewelEffect` /
  `CorruptedRareJewelEffect` on `env.modDB` (The Adorned)
- Copying mods from the paired ring into `env.itemModDB` (Kalandra's Touch)
- Setting `AffectedByEnergyBlade` condition to trigger a full `initEnv` re-run

Because SETUP-16 changes **what** gets added to the mod databases, the error
appears in every downstream field when these uniques are equipped, not as a
missing dedicated output field.

## Dependencies

- SETUP-01 (item mod parsing & slot assignment) — must populate item mod lists first
- SETUP-02 (support gem construction) — Energy Blade check walks the gem list
- SETUP-11 (item conditions) — same item processing loop; shares slot iteration context

## Lua Source

File: `third-party/PathOfBuilding/src/Modules/CalcSetup.lua`

Commit: `454eff8c85d24356d9b051d596983745ed367476`

The relevant logic is spread across two locations in `calcs.initEnv`:

1. **Inside the item slot loop** (lines 961–1131) — handles Necromantic Aegis,
   Energy Blade weapon replacement, The Iron Mass, Dancing Dervish, Kalandra's
   Touch, and Widowhail when dispatching item mods to `env.itemModDB`.
2. **The Adorned jewel handling** (lines 730–737) — inside the jewel socket
   sub-block of the same slot loop.
3. **Energy Blade re-run trigger** (lines 1731–1741) — after all skills are
   built, checks if any gem is "Energy Blade" and if so re-calls `initEnv` with
   the `AffectedByEnergyBlade` condition set.

The Gloves / Boots multiplier branches (lines 1105–1128) are adjacent to the
special-unique branches but are not unique-specific; they are included below for
completeness because they share the same `if/elseif` chain.

## Annotated Lua

### Local aliases (CalcSetup.lua lines 8–13)

```lua
local t_insert = table.insert   -- Rust: vec.push(...)
local m_min    = math.min       -- Rust: x.min(y)
local m_max    = math.max       -- Rust: x.max(y)
```

---

### The Adorned — lines 730–737 (inside jewel socket block)

```lua
if item and item.type == "Jewel" and item.name:match("The Adorned, Crimson Jewel") then
    -- item.name:match(…) is Lua pattern match — "The Adorned, Crimson Jewel" is a
    -- literal substring match here (no special chars). Rust: item.name.contains("The Adorned, Crimson Jewel")
    if item.jewelData.corruptedMagicJewelIncEffect then
        -- Sets a multiplier on env.modDB so all subsequent magic jewel mods
        -- can be scaled. Value is divided by 100 to convert pct → multiplier.
        -- Rust: env.player.mod_db.set_multiplier("CorruptedMagicJewelEffect", v / 100.0)
        env.modDB.multipliers["CorruptedMagicJewelEffect"] = item.jewelData.corruptedMagicJewelIncEffect / 100
    end
    if item.jewelData.corruptedRareJewelIncEffect then
        -- Same pattern for rare corrupted jewels.
        -- Rust: env.player.mod_db.set_multiplier("CorruptedRareJewelEffect", v / 100.0)
        env.modDB.multipliers["CorruptedRareJewelEffect"] = item.jewelData.corruptedRareJewelIncEffect / 100
    end
end
```

**Notes:**
- `item.jewelData.corruptedMagicJewelIncEffect` is `nil` if the jewel has no
  such property. The `if` guards handle that — Rust: `if let Some(v) = ...`.
- These multipliers are then applied when individual corrupted magic/rare jewels
  scale their mods. No Rust equivalent exists yet (the field is not in `Item` or
  `CalcEnv`).

---

### Item mod dispatch — lines 961–1131 (inner `if item then` block)

The structure is a single `if/elseif/else` chain. For most items, the `else`
branch at line 1129 just does `env.itemModDB:ScaleAddList(srcList, scale)`.
Each special unique branches off before that default.

```lua
-- ── Necromantic Aegis ── (lines 961–978)
if item.type == "Shield" and env.allocNodes[45175] and env.allocNodes[45175].dn == "Necromantic Aegis" then
    -- Necromantic Aegis keystone (passive node 45175) redirects ALL shield mods
    -- to minions instead of the player. Non-socketed-gem mods go to aegisModList;
    -- SocketedIn-tagged mods still go to itemModDB (gems remain player-affecting).
    env.aegisModList = new("ModList")
    -- Rust: env.aegis_mod_list = Some(ModDb::new())
    for _, mod in ipairs(srcList) do
        local add = true
        for _, tag in ipairs(mod) do
            -- tag.type is the ModTag discriminant. "SocketedIn" means the mod only
            -- applies to gems socketed in this item slot.
            if tag.type == "SocketedIn" then
                add = false
                break
            end
        end
        if add then
            env.aegisModList:ScaleAddMod(mod, scale)
            -- Rust: aegis_mod_list.scale_add_mod(mod, scale)
        else
            env.itemModDB:ScaleAddMod(mod, scale)
            -- SocketedIn mods still apply normally.
        end
    end
    -- NOTE: this replaces the entire default dispatch — no call to ScaleAddList after.

-- ── Energy Blade — weapon slot replacement ── (lines 979–1013)
elseif (slotName == "Weapon 1" or slotName == "Weapon 2") and modDB.conditions["AffectedByEnergyBlade"] then
    -- AffectedByEnergyBlade is set on the SECOND pass through initEnv.
    -- On this pass, we replace the actual weapon item with a synthetic Energy Blade item
    -- whose base stats come from the data table. The original item's sockets are copied.
    local previousItem = env.player.itemList[slotName]
    -- previousItem is the original weapon (already assigned earlier in the loop at line 931).
    local type = previousItem and previousItem.weaponData and previousItem.weaponData[1].type
    local info = env.data.weaponTypeInfo[type]
    -- weaponTypeInfo maps "One Handed Sword" → { oneHand = true, … }
    if info and type ~= "Bow" then
        local name = info.oneHand and "Energy Blade One Handed" or "Energy Blade Two Handed"
        -- Synthetic item: name is the item base name, fields are zeroed out.
        local item = new("Item")
        item.name = name
        item.base = data.itemBases[name]
        item.baseName = name
        -- All mod line arrays set to empty — Energy Blade derives its stats
        -- purely from the base item type's implicit line.
        item.classRequirementModLines = { }
        item.buffModLines = { }
        item.enchantModLines = { }
        item.scourgeModLines = { }
        item.implicitModLines = { }
        item.explicitModLines = { }
        item.crucibleModLines = { }
        item.quality = 0
        item.rarity = "NORMAL"
        -- If the base has an implicit mod text, parse it into implicitModLines.
        if item.baseName.implicit then
            local implicitIndex = 1
            for line in item.baseName.implicit:gmatch("[^\n]+") do
                -- gmatch("[^\n]+") splits on newlines — Rust: str.lines()
                local modList, extra = modLib.parseMod(line)
                t_insert(item.implicitModLines, { line = line, extra = extra, modList = modList or { }, modTags = ... })
                implicitIndex = implicitIndex + 1  -- 1-based index
            end
        end
        item:NormaliseQuality()
        item:BuildAndParseRaw()
        item.sockets = previousItem.sockets
        item.abyssalSocketCount = previousItem.abyssalSocketCount
        env.player.itemList[slotName] = item  -- replace item in slot
        -- NOTE: No mods are added to itemModDB here — the new item's mods will be
        -- processed on the NEXT iteration of the outer item loop. But wait —
        -- this replaces mid-loop, so actually the NEXT time initEnv runs (the re-run
        -- triggered at line 1735) the new item is already in itemList.
    else
        -- Bow or unknown weapon type: fall through to normal dispatch.
        env.itemModDB:ScaleAddList(srcList, scale)
    end

-- ── The Iron Mass ── (lines 1014–1031)
elseif slotName == "Weapon 1" and item.name == "The Iron Mass, Gladius" then
    -- The Iron Mass copies its mods to both env.theIronMass (for animated weapons)
    -- AND env.itemModDB (player stats). Unlike Necromantic Aegis, both get the mods.
    -- SocketedIn mods are excluded from theIronMass only.
    env.theIronMass = new("ModList")
    -- Rust: env.the_iron_mass = Some(ModDb::new())
    for _, mod in ipairs(srcList) do
        local add = true
        for _, tag in ipairs(mod) do
            if tag.type == "SocketedIn" then
                add = false
                break
            end
        end
        if add then
            env.theIronMass:ScaleAddMod(mod, scale)
        end
        -- ALL mods (including SocketedIn) also go to the player's itemModDB:
        env.itemModDB:ScaleAddMod(mod, scale)
    end

-- ── Dancing Dervish ── (lines 1032–1049)
elseif slotName == "Weapon 1" and item.grantedSkills[1] and item.grantedSkills[1].skillId == "UniqueAnimateWeapon" then
    -- Dancing Dervish is detected by its granted skill (UniqueAnimateWeapon), not by name.
    -- Its mods are split: non-SocketedIn go to weaponModList1 (for the animated weapon),
    -- SocketedIn go to itemModDB (gems still affect player).
    env.weaponModList1 = new("ModList")
    -- Rust: env.weapon_mod_list1 = Some(ModDb::new())
    for _, mod in ipairs(srcList) do
        local add = true
        for _, tag in ipairs(mod) do
            if tag.type == "SocketedIn" then
                add = false
                break
            end
        end
        if add then
            env.weaponModList1:ScaleAddMod(mod, scale)
        else
            env.itemModDB:ScaleAddMod(mod, scale)
        end
    end
    -- NOTE: Unlike The Iron Mass, non-SocketedIn mods do NOT go to itemModDB.

-- ── Kalandra's Touch ── (lines 1050–1088)
elseif item.name:match("Kalandra's Touch") then
    -- Kalandra's Touch (Ring) mirrors the OTHER ring slot's mods.
    -- If the other ring is not also a Kalandra's Touch, copy its mods.
    local otherRing = items[(slotName == "Ring 1" and "Ring 2") or (slotName == "Ring 2" and "Ring 1")]
    -- Lua ternary: (cond and a or b) — safe here because "Ring 2"/"Ring 1" are non-false.
    -- Rust: let other_slot = if slot_name == "Ring 1" { "Ring 2" } else { "Ring 1" };
    if otherRing and not otherRing.name:match("Kalandra's Touch") then
        for _, mod in ipairs(otherRing.modList or otherRing.slotModList[slot.slotNum] or {}) do
            -- otherRing.modList is the flat mod list; slotModList is used for dual-slot items.
            -- The `or {}` fallback prevents iteration errors on nil.
            -- Rust: .unwrap_or_default() on an empty vec
            for _, tag in ipairs(mod) do
                if tag.type == "SocketedIn" then
                    goto skip_mod  -- Lua `goto` — Rust: `continue 'outer` or labeled block
                end
            end
            local modCopy = copyTable(mod)
            modLib.setSource(modCopy, item.modSource)  -- reassign source to Kalandra's Touch
            env.itemModDB:ScaleAddMod(modCopy, scale)
            ::skip_mod::  -- Lua label for goto — no direct Rust equivalent, use `continue`
        end
        -- Copy influence multipliers from the other ring to itemModDB.multipliers:
        -- CorruptedItem, ShaperItem, ElderItem, WarlordItem, HunterItem, CrusaderItem, RedeemerItem
        for mult, property in pairs({...}) do
            if otherRing[property] then
                env.itemModDB.multipliers[mult] = (env.itemModDB.multipliers[mult] or 0) + 1
                -- "Non" prefix tracks items WITHOUT that influence:
                env.itemModDB.multipliers["Non"..mult] = (env.itemModDB.multipliers["Non"..mult] or 0) - 1
                -- NOTE: The Non-count is decremented, not zeroed. This can go negative
                -- if the other ring also matched earlier in the loop (unusual case).
            end
        end
        if otherRing.elder or otherRing.shaper then
            env.itemModDB.multipliers.ShaperOrElderItem = (env.itemModDB.multipliers.ShaperOrElderItem or 0) + 1
        end
        -- Ring base name key (e.g. "CryonicBandEquipped" for Esh ring base):
        local otherRingKey = otherRing.baseName:gsub(" ", "").."Equipped"
        if otherRingKey then  -- always true since gsub returns a string, never nil
            env.itemModDB.multipliers[otherRingKey] = (env.itemModDB.multipliers[otherRingKey] or 0) + 1
        end
    end
    -- Only ExtraSkill implicit mods on Kalandra's Touch itself work (likely in-game bug):
    for _, mod in ipairs(srcList) do
        if mod.name == "ExtraSkill" then
            env.itemModDB:ScaleAddMod(mod, scale)
        end
    end

-- ── Widowhail ── (lines 1089–1097)
elseif item.type == "Quiver" and (items["Weapon 1"] and items["Weapon 1"].name:match("Widowhail") or env.initialNodeModDB:Sum("INC", nil, "EffectOfBonusesFromQuiver") > 0) then
    -- Widowhail bow multiplies all quiver bonuses.
    -- The multiplier = 1 + (bow's EffectOfBonusesFromQuiver% + passive EffectOfBonusesFromQuiver%) / 100.
    -- Default Widowhail has 100% increased effect on quiver bonuses → 2× scale.
    -- `env.initialNodeModDB` is the passive-tree-only mod database (before items).
    -- Rust: no equivalent yet — `initialNodeModDB` is not a distinct field in CalcEnv.
    local widowHailMod = (1 + (items["Weapon 1"] and items["Weapon 1"].baseModList:Sum("INC", nil, "EffectOfBonusesFromQuiver") + env.initialNodeModDB:Sum("INC", nil, "EffectOfBonusesFromQuiver") or 100) / 100)
    -- NOTE: The `or 100` default applies when items["Weapon 1"] is nil (weapon not present).
    -- In that case widowHailMod = 1 + 100/100 = 2.0.
    scale = scale * widowHailMod
    -- Rust: scale *= widow_hail_mod;
    env.modDB:NewMod("WidowHailMultiplier", "BASE", widowHailMod, "Widowhail")
    -- Sets a named multiplier on modDB so other calculations can reference it.
    -- Rust: env.player.mod_db.add(Mod::new_base("WidowHailMultiplier", widow_hail_mod, source))
    local combinedList = new("ModList")
    for _, mod in ipairs(srcList) do
        combinedList:MergeMod(mod)  -- MergeMod merges duplicate mods (sums BASE, etc.)
    end
    env.itemModDB:ScaleAddList(combinedList, scale)

-- ── Corrupted jewel effect scaling (The Adorned follow-through) ── (lines 1098–1104)
elseif env.modDB.multipliers["Corrupted" .. rarity_normalized .. "JewelEffect"] and item.type == "Jewel" and item.corrupted and slot.nodeId and item.base.subType ~= "Charm" and not env.spec.nodes[slot.nodeId].containJewelSocket then
    -- This branch applies to individual corrupted jewels AFTER The Adorned has set the
    -- CorruptedMagicJewelEffect / CorruptedRareJewelEffect multipliers on modDB.
    -- "rarity_normalized" is computed by the expression:
    --   item.rarity:gsub("(%a)(%u*)", function(a, b) return a..string.lower(b) end)
    -- which Title-cases the rarity: "UNIQUE" → "Unique", "RARE" → "Rare", "MAGIC" → "Magic"
    -- Full check: is there a "CorruptedUniqueJewelEffect" multiplier AND is this jewel
    -- corrupted AND in a regular (non-cluster) jewel socket?
    scale = scale + env.modDB.multipliers["Corrupted" .. rarity_normalized .. "JewelEffect"]
    -- NOTE: `scale` is ADDED to (not multiplied). Default scale=1, so this adds the
    -- fractional modifier. E.g. CorruptedMagicJewelEffect=0.2 → scale = 1.2.
    local combinedList = new("ModList")
    for _, mod in ipairs(srcList) do
        combinedList:MergeMod(mod)
    end
    env.itemModDB:ScaleAddList(combinedList, scale)

-- ── Gloves with EffectOfBonusesFromGloves ── (lines 1105–1116)
elseif item.type == "Gloves" and calcLib.mod(env.initialNodeModDB, nil, "EffectOfBonusesFromGloves") ~= 1 then
    -- Similar to Widowhail but for gloves (no specific unique name — passive-based).
    -- calcLib.mod(modDB, cfg, name) = (1 + Sum("INC", cfg, name)/100) * More(cfg, name)
    -- The scale is set to mod-1 (i.e., 0 → no change; 1 → double), then combined mod list
    -- has the scaled version MERGED back in. This is an additive double-application trick.
    scale = calcLib.mod(env.initialNodeModDB, nil, "EffectOfBonusesFromGloves") - 1
    local combinedList = new("ModList")
    for _, mod in ipairs(srcList) do
        combinedList:MergeMod(mod)
    end
    local scaledList = new("ModList")
    scaledList:ScaleAddList(combinedList, scale)
    for _, mod in ipairs(scaledList) do
        combinedList:MergeMod(mod, true)  -- second arg `true` means "merge additively"
    end
    env.itemModDB:AddList(combinedList)  -- AddList, NOT ScaleAddList (scale already applied)

-- ── Boots with EffectOfBonusesFromBoots ── (lines 1117–1128)
elseif item.type == "Boots" and calcLib.mod(env.initialNodeModDB, nil, "EffectOfBonusesFromBoots") ~= 1 then
    -- Identical pattern to Gloves above but for boots.
    scale = calcLib.mod(env.initialNodeModDB, nil, "EffectOfBonusesFromBoots") - 1
    -- … same combinedList + scaledList merge trick …
    env.itemModDB:AddList(combinedList)

-- ── Default dispatch (line 1129–1130) ──
else
    env.itemModDB:ScaleAddList(srcList, scale)
end
```

---

### Energy Blade re-run trigger — lines 1731–1741

```lua
-- After all socket groups have been processed, scan for Energy Blade gem.
-- If found and AffectedByEnergyBlade is not yet set, re-call initEnv with the condition.
if not modDB.conditions["AffectedByEnergyBlade"] and group.enabled and group.slotEnabled then
    for _, gemInstance in ipairs(group.gemList) do
        local grantedEffect = gemInstance.gemData and gemInstance.gemData.grantedEffect or gemInstance.grantedEffect
        -- NOTE: `and/or` chaining for nil-safe access:
        -- gemInstance.gemData.grantedEffect if gemInstance.gemData is truthy,
        -- else gemInstance.grantedEffect.
        -- Rust: gemInstance.gem_data.as_ref().and_then(|d| d.granted_effect.as_ref())
        --         .unwrap_or(&gemInstance.granted_effect)
        if grantedEffect and not grantedEffect.support and gemInstance.enabled and grantedEffect.name == "Energy Blade" then
            override.conditions = override.conditions or { }
            t_insert(override.conditions, "AffectedByEnergyBlade")
            return calcs.initEnv(build, mode, override, specEnv)
            -- This EARLY RETURNS from initEnv, causing a full recalculation with the
            -- AffectedByEnergyBlade condition set. On the second pass, the weapon
            -- replacement branch (lines 979–1013) fires.
            -- Rust: This recursive re-run pattern would be modeled as a loop or a
            -- two-phase init: first pass detects Energy Blade, second pass replaces weapons.
        end
    end
end
```

## Existing Rust Code

File: `crates/pob-calc/src/calc/setup.rs`

The item mod dispatch lives in `add_item_mods()` (line 1765). It iterates all
non-flask, non-jewel slots and calls `env.player.mod_db.add(m)` for every
parsed mod line. There is **no branching** — every slot goes through the same
path.

**What exists:**
- `add_item_mods()` parses mods from all non-flask/jewel items and adds them to
  `env.player.mod_db` (lines 1802–1807).
- Weapon data extraction into `env.player.weapon_data1` / `weapon_data2` and
  `env.player.has_shield` (lines 1836–1852).
- `CalcEnv` struct (env.rs lines 210–217): has `player: Actor` and
  `enemy: Actor` but **no `aegis_mod_list`, `the_iron_mass`, `weapon_mod_list1`,
  or any special-unique slots**.
- `mod_parser_generated.rs` line 39463: `Necromantic Aegis` keystone mod is
  stubbed as `ModValue::Number(0.0) /* TODO */`.
- `mod_parser_generated.rs` line 26368: The Iron Mass `MinionModifier` mod is
  also stubbed.

**What is missing:**
1. No `aegis_mod_list` field on `CalcEnv` or `Actor`.
2. No `the_iron_mass` field.
3. No `weapon_mod_list1` field.
4. No detection of `AffectedByEnergyBlade` condition during item dispatch.
5. No weapon item replacement logic for Energy Blade (no synthetic item creation).
6. No Energy Blade re-run trigger (no scan of gem list after skill construction).
7. No Kalandra's Touch ring mirroring logic.
8. No Widowhail quiver scaling — `EffectOfBonusesFromQuiver` is not queried.
9. No `WidowHailMultiplier` mod injected.
10. No `initialNodeModDB` concept — passive-only mod DB is not distinguished from
    the full mod DB in Rust.
11. No `EffectOfBonusesFromGloves` / `EffectOfBonusesFromBoots` scaling.
12. No `CorruptedMagicJewelEffect` / `CorruptedRareJewelEffect` multiplier set
    from The Adorned jewel data.
13. No per-corrupted-jewel scale adjustment using those multipliers.

## What Needs to Change

1. **Add special-unique fields to `CalcEnv`** (`env.rs`):
   ```rust
   pub aegis_mod_list: Option<ModDb>,   // Necromantic Aegis
   pub the_iron_mass: Option<ModDb>,    // The Iron Mass animated weapon mods
   pub weapon_mod_list1: Option<ModDb>, // Dancing Dervish animated weapon mods
   ```

2. **Refactor `add_item_mods()` to mirror the Lua `if/elseif/else` dispatch chain**:
   - Before the default `env.player.mod_db.add(m)` call, check for each special case.
   - Necromantic Aegis: check `item.item_type.contains("Shield")` AND the player
     has the Necromantic Aegis keystone allocated. Split mods on `SocketedIn` tag.
   - Energy Blade (weapon pass): check `env.player.mod_db.flag("AffectedByEnergyBlade")`
     and the slot is Weapon 1 or 2. Build synthetic item and skip normal dispatch.
   - The Iron Mass: check `slot == Weapon1` and `item.name == "The Iron Mass, Gladius"`.
     Split on `SocketedIn`: non-SocketedIn to `the_iron_mass`; all mods to player.
   - Dancing Dervish: check `slot == Weapon1` and item grants `UniqueAnimateWeapon`
     skill (check parsed mods for `ExtraSkill` with `skillId == "UniqueAnimateWeapon"`).
     Split on `SocketedIn`: non-SocketedIn to `weapon_mod_list1`; SocketedIn to player.
   - Kalandra's Touch: check `item.name.contains("Kalandra's Touch")`. Copy mods from
     the other ring slot, re-sourced. Copy influence multipliers.
   - Widowhail: check `item.item_type == "Quiver"` and weapon-1 name contains "Widowhail"
     OR passive tree has `EffectOfBonusesFromQuiver > 0`. Scale and inject
     `WidowHailMultiplier`.
   - Corrupted jewel scaling: requires `CorruptedMagicJewelEffect` multiplier already
     set (from The Adorned) and checks `item.corrupted` plus slot type.
   - Gloves/Boots: check `item.item_type` and `EffectOfBonusesFromGloves/Boots` passive.

3. **Add Energy Blade re-run trigger** after skill construction:
   - Walk active skill list for any skill with name `"Energy Blade"`.
   - If found and `AffectedByEnergyBlade` is not set, set the condition and
     re-run the full init. This requires a two-phase or iterative init structure
     in Rust rather than true recursion.

4. **Implement `initialNodeModDB` concept**: The Widowhail and Gloves/Boots
   branches query `env.initialNodeModDB` — a passive-tree-only mod DB built
   before items are added. Rust currently builds one combined mod DB. A separate
   `passive_mod_db` (populated from tree nodes only) must be built first, then
   held alongside the main `mod_db` for these queries.

5. **Fix mod_parser_generated.rs stubs**:
   - Line 39463: `Necromantic Aegis` keystone stub needs a proper `ModValue::Str`
     with the keystone name so SETUP-07/SETUP-10 can grant it.
   - Line 26368: The Iron Mass `MinionModifier` stub needs the full mod value so
     animated weapons get the triple damage chance.

6. **The Adorned jewel data**: `item.jewelData.corruptedMagicJewelIncEffect` comes
   from PoB's item parsing (gem data tables). The Rust `Item` struct currently has
   no `jewel_data` field. Either add a `JewelData` sub-struct or handle this as a
   special parsed mod from the item's explicit lines.

## Notable Gaps and Tricky Patterns

- **SocketedIn tag filtering**: The pattern of iterating a mod's tag list to
  detect `tag.type == "SocketedIn"` appears in four different branches. In Rust,
  `ModTag` has a `SocketedIn` variant — use `mod.tags.iter().any(|t| matches!(t, ModTag::SocketedIn { .. }))`.

- **The `or` default trap in Widowhail**: The expression
  `items["Weapon 1"] and items["Weapon 1"].baseModList:Sum(...) + ... or 100`
  returns `100` when `items["Weapon 1"]` is `nil` (i.e., no weapon equipped).
  In Rust, `None.and_then(|w| ...).unwrap_or(100.0)` is the equivalent.

- **`goto skip_mod` in Kalandra's Touch**: Lua's `goto` is used to skip the rest
  of the inner loop body. In Rust, restructure as an early `continue` or use a
  `'inner: loop { break 'inner; }` pattern.

- **Two-pass Energy Blade**: PoB's `initEnv` calls itself recursively. Rust does
  not support this pattern safely with mutable references. Use a boolean flag
  returned from `init_env` to signal "need another pass" and call again from the
  caller.

- **`item.grantedSkills[1].skillId == "UniqueAnimateWeapon"`**: In Rust the
  `Item` struct has no `granted_skills`. Dancing Dervish could instead be
  detected by item name (`"The Dancing Dervish"`), or by parsing a special
  `ExtraSkill` mod with `skillId == "UniqueAnimateWeapon"` from the item's mod
  lines.

- **`env.initialNodeModDB` vs `env.modDB`**: In the Lua, `initialNodeModDB` holds
  only passive-tree mods (populated before items). `modDB` is the full database
  after merging items. Widowhail and Gloves/Boots reference `initialNodeModDB` to
  check whether passives (not items) grant the bonus-from-slot multiplier.
  This distinction is needed to avoid circular dependency (item scaling items).
