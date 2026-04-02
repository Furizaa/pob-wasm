# SETUP-11: Item Condition & Multiplier Tracking

## Output Fields

This chunk writes **no `output[]` fields directly**. Its effect is entirely via
`modDB.multipliers` and `modDB.conditions` entries that downstream mods reference
through `Multiplier` and `MultiplierThreshold` tags in `eval_mod`.

Key multipliers written (used by generated mod tags in `mod_parser_generated.rs`):

| Key | Purpose |
|-----|---------|
| `UniqueItem` | Count of equipped unique/relic items |
| `RareItem` | Count of equipped rare items |
| `MagicItem` | Count of equipped magic items |
| `NormalItem` | Count of equipped normal items |
| `FoulbornUniqueItem` | Count of unique items with the `foulborn` flag |
| `CorruptedItem` | Count of corrupted items |
| `NonCorruptedItem` | Count of non-corrupted items |
| `ShaperItem` | Count of shaper-influenced items |
| `ElderItem` | Count of elder-influenced items |
| `WarlordItem` | Count of warlord-influenced items |
| `HunterItem` | Count of hunter-influenced items |
| `CrusaderItem` | Count of crusader-influenced items |
| `RedeemerItem` | Count of redeemer-influenced items |
| `NonShaperItem` | Count of non-shaper items |
| `NonElderItem` | etc. |
| `ShaperOrElderItem` | Count of items that are shaper OR elder |
| `{TypeName}Item` | Count per item type, e.g. `BodyArmourItem`, `WeaponItem`, `RingItem` |
| `{BaseName}Equipped` | Count per ring base name (for Breachlord rings) |
| `{RarityKey}In{SlotName}` | Condition (bool): rarity of item in a specific slot |
| `SocketedGemsIn{SlotName}` | Count of socketed+enabled gems in each item slot |
| `EmptyRedSocketsInAnySlot` | Count of empty R sockets across all items |
| `EmptyGreenSocketsInAnySlot` | Count of empty G sockets |
| `EmptyBlueSocketsInAnySlot` | Count of empty B sockets |
| `EmptyWhiteSocketsInAnySlot` | Count of empty W sockets |

Key conditions written:

| Key | Purpose |
|-----|---------|
| `{TitleNoSpaces}` | Class-restriction condition for restricted unique items (e.g. `EldritchBatteryHelmet`) |
| `{RarityKey}In{SlotName}` | e.g. `UniqueItemInRing 1`, `RareItemInBody Armour` |

## Dependencies

- **SETUP-01**: Items must be parsed and stored in `build.items` before this runs.
- **SETUP-03**: Flask/jewel exclusions must be consistent with this chunk's skip logic.

## Lua Source

File: `third-party/PathOfBuilding/src/Modules/CalcSetup.lua`, lines 1132–1210
Commit: `454eff8`

## Annotated Lua

```lua
-- ─────────────────────────────────────────────────────────────────────────────
-- Lines 1132-1135: Class-restriction condition
-- ─────────────────────────────────────────────────────────────────────────────

-- item.classRestriction is a string like "Scion" (or nil for unrestricted items)
-- Some uniques can only be equipped by specific classes.
-- The condition key is the item's display title with spaces removed.
-- e.g. title = "Solaris Lorica" → "SolarisLorica" = "Scion"
-- In eval_mod, this condition is checked via {type="Condition", var="SolarisLorica"}.
-- Rust: call `mod_db.set_condition(&title_no_spaces, item.class_restriction.is_some())`
-- NOTE: The Lua stores the restriction STRING as the condition value, not a bool.
--       In PoB Lua, conditions can hold non-bool values; eval_mod checks truthiness.
--       In Rust, conditions are HashMap<String, bool>, so store true if restriction exists.
if item.classRestriction then
    env.itemModDB.conditions[item.title:gsub(" ", "")] = item.classRestriction
    -- Lua: `item.title` is the display name. Rust equivalent: item.name
    -- Lua: `:gsub(" ", "")` removes all spaces. Rust: .replace(' ', "")
end

-- ─────────────────────────────────────────────────────────────────────────────
-- Lines 1136-1203: Item counts (excluded for Jewel/Flask/Tincture/Graft)
-- ─────────────────────────────────────────────────────────────────────────────

-- IMPORTANT: This block is SKIPPED for Jewels, Flasks, Tinctures, and Grafts.
-- Rust: check item_type against these type names.
if item.type ~= "Jewel" and item.type ~= "Flask" and item.type ~= "Tincture" and item.type ~= "Graft" then

    -- ── Lines 1138-1151: Rarity multiplier ──────────────────────────────────
    local key
    -- Lua: item.rarity is "UNIQUE", "RELIC", "RARE", "MAGIC", or "NORMAL"
    -- Rust: ItemRarity enum (Normal/Magic/Rare/Unique). No RELIC variant exists in Rust.
    -- RELIC is treated the same as UNIQUE for counting purposes.
    -- FoulbornUniqueItem: `item.foulborn` is a bool flag that doesn't exist in Rust's Item type.
    --   This flag appears on certain unique items in the PoB item database. Needs adding.
    if item.rarity == "UNIQUE" or item.rarity == "RELIC" then
        if item.foulborn then
            -- Lua: (x or 0) + 1  →  Rust: *entry.or_insert(0.0) += 1.0
            env.itemModDB.multipliers["FoulbornUniqueItem"] = (env.itemModDB.multipliers["FoulbornUniqueItem"] or 0) + 1
        end
        key = "UniqueItem"
    elseif item.rarity == "RARE" then
        key = "RareItem"
    elseif item.rarity == "MAGIC" then
        key = "MagicItem"
    else
        key = "NormalItem"
    end
    -- Increment count for this rarity
    -- Lua: (x or 0) + 1  →  Rust: *multipliers.entry(key).or_insert(0.0) += 1.0
    env.itemModDB.multipliers[key] = (env.itemModDB.multipliers[key] or 0) + 1

    -- Set per-slot rarity condition: "{RarityKey}In{SlotName}" = true
    -- e.g. "UniqueItemInWeapon 1" = true, "RareItemInBody Armour" = true
    -- This lets mods like "if you have a Unique in Ring 1" work.
    -- Rust: mod_db.set_condition(&format!("{}In{}", key, slot_name), true)
    env.itemModDB.conditions[key .. "In" .. slotName] = true

    -- ── Lines 1153-1159: Influence multipliers ───────────────────────────────
    -- Maps: Lua property name → Rust ItemInfluence field
    --   "corrupted" → item.corrupted (bool)
    --   "shaper"    → item.influence.shaper
    --   "elder"     → item.influence.elder
    --   "adjudicator" → item.influence.warlord  (NOTE: Lua uses "adjudicator" for warlord)
    --   "basilisk"  → item.influence.hunter     (NOTE: Lua uses "basilisk" for hunter)
    --   "crusader"  → item.influence.crusader
    --   "eyrie"     → item.influence.redeemer   (NOTE: Lua uses "eyrie" for redeemer)
    -- Each influence increments its multiplier AND increments the corresponding "Non" multiplier
    -- for all OTHER items (i.e. items without that influence get "NonShaperItem" += 1, etc.)
    for mult, property in pairs({
        ["CorruptedItem"] = "corrupted",
        ["ShaperItem"] = "shaper",
        ["ElderItem"] = "elder",
        ["WarlordItem"] = "adjudicator",
        ["HunterItem"] = "basilisk",
        ["CrusaderItem"] = "crusader",
        ["RedeemerItem"] = "eyrie"
    }) do
        if item[property] then
            env.itemModDB.multipliers[mult] = (env.itemModDB.multipliers[mult] or 0) + 1
        else
            -- Items WITHOUT this influence increment the "Non" counter.
            -- e.g. a non-shaper item → NonShaperItem += 1
            env.itemModDB.multipliers["Non"..mult] = (env.itemModDB.multipliers["Non"..mult] or 0) + 1
        end
    end

    -- ── Lines 1160-1162: ShaperOrElderItem ──────────────────────────────────
    if item.shaper or item.elder then
        -- Rust: if item.influence.shaper || item.influence.elder
        env.itemModDB.multipliers.ShaperOrElderItem = (env.itemModDB.multipliers.ShaperOrElderItem or 0) + 1
    end

    -- ── Line 1163: Item type multiplier ─────────────────────────────────────
    -- Builds a key from item.type by:
    --   1. Removing all spaces: "Body Armour" → "BodyArmour"
    --   2. Replacing "TwoHanded" / "OneHanded" patterns with "": 
    --      "Two Handed Sword" → (after space removal: "TwoHandedSword") → gsub(".+Handed", "") → "Sword"
    --      This means all two-handed and one-handed weapons of the same base (Axe, Sword, Mace) share
    --      a single multiplier key ("AxeItem", "SwordItem", "MaceItem").
    --   3. Appends "Item" suffix
    -- Examples:
    --   "Body Armour" → "BodyArmourItem"
    --   "Helmet"      → "HelmetItem"
    --   "Gloves"      → "GlovesItem"
    --   "Boots"       → "BootsItem"
    --   "Ring"        → "RingItem"
    --   "Amulet"      → "AmuletItem"
    --   "Belt"        → "BeltItem"
    --   "Two Handed Sword" → "SwordItem"  (strips "TwoHanded")
    --   "One Handed Axe"   → "AxeItem"    (strips "OneHanded")
    --   "Shield"      → "ShieldItem"
    -- Rust: item_type_key = item.item_type.replace(' ', "")
    --         .replacen if it contains "Handed" use regex to strip the prefix
    -- CAUTION: Lua's `.+Handed` pattern is "one or more chars ending in 'Handed'".
    --   Equivalent Rust regex: r".+Handed" or just: strip "OneHanded"/"TwoHanded" prefix.
    env.itemModDB.multipliers[item.type:gsub(" ", ""):gsub(".+Handed", "").."Item"] =
        (env.itemModDB.multipliers[item.type:gsub(" ", ""):gsub(".+Handed", "").."Item"] or 0) + 1

    -- ── Lines 1165-1168: Ring base name multiplier ───────────────────────────
    -- For rings, also counts per base name (for Breachlord ring interactions).
    -- e.g. "Cryonic Ring" → "CryonicRingEquipped" += 1
    -- These are used by Esh of the Storm / Tul of the Blizzard uniques.
    -- Rust: item.base_type.replace(' ', "") + "Equipped"
    if item.type == "Ring" then
        local key = item.baseName:gsub(" ", "").."Equipped"
        -- Lua: item.baseName = "Cryonic Ring" / "Sapphire Ring" etc.
        -- Rust: item.base_type (same concept)
        env.itemModDB.multipliers[key] = (env.itemModDB.multipliers[key] or 0) + 1
    end

    -- ── Lines 1169-1202: Socket counting ─────────────────────────────────────
    -- Counts how many enabled gems are socketed in this slot and how many
    -- sockets of each color are empty.
    --
    -- slotEmptySocketsCount: table with keys R/G/B/W, counts empty sockets per color
    -- slotGemSocketsCount: total non-abyss sockets on this item
    -- socketedGems: number of enabled gems in this slot's socket groups
    local slotEmptySocketsCount = { R = 0, G = 0, B = 0, W = 0 }
    local slotGemSocketsCount = 0
    local socketedGems = 0

    -- Count enabled gems in socket groups assigned to this slot.
    -- In PoB, each socket group (skill) has: .source (nil for player-created), .enabled, .slot, .gemList
    -- Rust equivalent: iterate build.skill_sets[active_skill_set].skills where skill.slot == slot_name
    --   and skill.enabled == true; count enabled gems (gem.enabled == true and gem has gemData).
    -- NOTE: The Lua checks `socketGroup.source == nil` to skip auto-generated groups (e.g. from items).
    --   In Rust, `Skill::source` is not stored — all Skill entries in skill_sets come from the XML.
    --   The equivalent filtering is: only count skills that have `slot == slot_name`.
    for _, socketGroup in pairs(env.build.skillsTab.socketGroupList) do
        if (not socketGroup.source and socketGroup.enabled and socketGroup.slot and
            socketGroup.slot == slotName and socketGroup.gemList) then
            for _, gem in pairs(socketGroup.gemList) do
                if (gem.gemData and gem.enabled) then
                    socketedGems = socketedGems + 1
                end
            end
        end
    end

    -- Iterate sockets on the item. Lua: item.sockets is a list of {color = 'R'/'G'/'B'/'W'/'A'}.
    -- Rust: item.sockets is Vec<SocketGroup> where each group has colors: Vec<char>.
    --   Need to flatten: iterate all chars across all socket groups.
    -- 'A' (abyss) sockets are EXCLUDED from gem-socket counts.
    -- Socket index (1-based in Lua) is used to determine if a socket is "empty":
    --   socket index > socketedGems means the gem count doesn't reach this socket.
    -- CAUTION: Lua uses ipairs(item.sockets) which gives 1-based i.
    --   When flattening Rust's SocketGroup.colors, maintain a 1-based counter.
    for i, socket in ipairs(item.sockets) do
        -- Only count R, B, G, W sockets (not abyss 'A')
        if socket.color == 'R' or socket.color == 'B' or socket.color == 'G' or socket.color == 'W' then
            slotGemSocketsCount = slotGemSocketsCount + 1
            -- Sockets beyond the gem count are "empty"
            if i > socketedGems then
                slotEmptySocketsCount[socket.color] = slotEmptySocketsCount[socket.color] + 1
            end
        end
    end

    -- SocketedGemsIn{SlotName}: capped at actual socket count
    -- math.min(slotGemSocketsCount, socketedGems) = min(available sockets, placed gems)
    -- Rust: slotGemSocketsCount.min(socketedGems)
    -- Key example: "SocketedGemsInBody Armour", "SocketedGemsInHelmet"
    env.itemModDB.multipliers["SocketedGemsIn"..slotName] =
        (env.itemModDB.multipliers["SocketedGemsIn"..slotName] or 0) + math.min(slotGemSocketsCount, socketedGems)

    -- Accumulate empty socket counts across all items
    env.itemModDB.multipliers.EmptyRedSocketsInAnySlot =
        (env.itemModDB.multipliers.EmptyRedSocketsInAnySlot or 0) + slotEmptySocketsCount.R
    env.itemModDB.multipliers.EmptyGreenSocketsInAnySlot =
        (env.itemModDB.multipliers.EmptyGreenSocketsInAnySlot or 0) + slotEmptySocketsCount.G
    env.itemModDB.multipliers.EmptyBlueSocketsInAnySlot =
        (env.itemModDB.multipliers.EmptyBlueSocketsInAnySlot or 0) + slotEmptySocketsCount.B
    env.itemModDB.multipliers.EmptyWhiteSocketsInAnySlot =
        (env.itemModDB.multipliers.EmptyWhiteSocketsInAnySlot or 0) + slotEmptySocketsCount.W

    -- Warning: if more gems are placed than there are sockets, PoB warns.
    -- Rust: can skip the warning (env.itemWarnings is UI-only).
    if socketedGems > slotGemSocketsCount then
        env.itemWarnings.socketLimitWarning = env.itemWarnings.socketLimitWarning or { }
        t_insert(env.itemWarnings.socketLimitWarning, slotName)
    end
end

-- ─────────────────────────────────────────────────────────────────────────────
-- Lines 1207-1210: Config override for empty sockets
-- ─────────────────────────────────────────────────────────────────────────────
-- After all items are processed, allow the config tab to override the computed
-- empty socket counts. In PoB's config tab, "overrideEmptyRedSockets" etc. are
-- number inputs (type="count"). If set, they replace the computed value.
-- Lua: (env.configInput.overrideX or env.itemModDB.multipliers.Y)
--   = if override is non-nil, use override; otherwise keep computed value
-- In Rust: build.config.numbers.get("overrideEmptyRedSockets")
--   These are stored as number inputs in BuildConfig.numbers.
-- Rust equivalent:
--   if let Some(&v) = build.config.numbers.get("overrideEmptyRedSockets") {
--       mod_db.set_multiplier("EmptyRedSocketsInAnySlot", v);
--   }
--   (And same for Green/Blue/White)
env.itemModDB.multipliers.EmptyRedSocketsInAnySlot =
    (env.configInput.overrideEmptyRedSockets or env.itemModDB.multipliers.EmptyRedSocketsInAnySlot)
env.itemModDB.multipliers.EmptyGreenSocketsInAnySlot =
    (env.configInput.overrideEmptyGreenSockets or env.itemModDB.multipliers.EmptyGreenSocketsInAnySlot)
env.itemModDB.multipliers.EmptyBlueSocketsInAnySlot =
    (env.configInput.overrideEmptyBlueSockets or env.itemModDB.multipliers.EmptyBlueSocketsInAnySlot)
env.itemModDB.multipliers.EmptyWhiteSocketsInAnySlot =
    (env.configInput.overrideEmptyWhiteSockets or env.itemModDB.multipliers.EmptyWhiteSocketsInAnySlot)
```

### Adjacent Context: Other-Ring Influence (lines 1068-1082)

The loop at lines 1153-1159 also appears earlier (lines 1068-1082) for the **Kalandra's Touch**
"other ring" processing path. When a ring mirrors another ring's mods, the influence multipliers
from the mirrored ring are also added. That code path is part of SETUP-16 (special uniques), not
SETUP-11.

## Existing Rust Code

File: `crates/pob-calc/src/calc/setup.rs`, function `add_item_mods`, lines 1765–1863.

### What Exists

- Iterates over equipped item slots, skips flask and jewel slots.
- Parses and adds all mod lines (implicits, explicits, crafted, enchant) to `player.mod_db`.
- Extracts weapon and armour base stats.
- Sets `has_shield`, `dual_wield` flags.

### What Is Missing (SETUP-11 logic not implemented)

1. **No class-restriction condition** (`item.classRestriction` → `itemModDB.conditions`). The
   Rust `Item` struct has no `class_restriction` field. PoB's item parser sets this from
   `item.base.classRestriction`. Needs addition to both the `Item` struct and the XML parser.

2. **No rarity multiplier tracking** (`UniqueItem`, `RareItem`, `MagicItem`, `NormalItem`).
   The `mod_parser_generated.rs` has entries referencing `UniqueItem` multiplier, but nothing
   ever sets `multipliers["UniqueItem"]`. Result: all "per unique item" mods evaluate to 0.

3. **No `FoulbornUniqueItem` support**. The `Item` struct has no `foulborn` boolean. This is
   a rarely-used flag from specific PoB uniques; low impact but needed for completeness.

4. **No RELIC rarity**. `ItemRarity` enum only has `Normal/Magic/Rare/Unique`. Relic items
   (a PoE 1 mechanic) are parsed as-is but not counted with Unique. Add `Relic` variant or
   map it to `Unique` in the counter.

5. **No per-slot rarity condition** (`UniqueItemInWeapon 1`, `RareItemInBody Armour`, etc.).
   These are `itemModDB.conditions` writes, not multipliers. Used by mods like
   `{type="Condition", var="MagicItemInRing 1"}` (seen in `mod_parser_generated.rs` line 38825).

6. **No influence multiplier tracking** (`ShaperItem`, `ElderItem`, `WarlordItem`, etc.).
   The `ItemInfluence` struct has `shaper/elder/crusader/redeemer/hunter/warlord` but the
   `add_item_mods` function never reads them to populate multipliers.
   - CRITICAL naming mismatch: Lua uses `"adjudicator"` for warlord, `"basilisk"` for hunter,
     `"eyrie"` for redeemer. These are PoB's internal field names. Rust uses correct English names.
     The multiplier keys (`WarlordItem`, `HunterItem`, `RedeemerItem`) are correct — just the
     _property lookup_ uses Lua field names. Rust reads `item.influence.warlord`, which is correct.
   - `NonShaperItem`, `NonElderItem`, etc. are never set.

7. **No `ShaperOrElderItem` counter**.

8. **No item-type multiplier** (`BodyArmourItem`, `HelmetItem`, `WeaponItem`, etc.).
   The Lua gsub pattern `item.type:gsub(" ", ""):gsub(".+Handed", "")` normalizes two-handed
   and one-handed weapon types to their weapon class. Rust must replicate this.
   Key mapping: `item.item_type` in Rust corresponds to `item.type` in Lua.

9. **No ring base-name multiplier** (`CryonicRingEquipped`, etc.).

10. **No socket counting** (`SocketedGemsIn{SlotName}`, `EmptyRedSocketsInAnySlot`, etc.).
    - The Rust `item.sockets` is `Vec<SocketGroup>` where each group has `colors: Vec<char>`.
      The Lua `item.sockets` is a flat list of sockets. Must flatten the Rust structure.
    - The Lua counts _active gems in this slot's socket groups_ from `skillsTab.socketGroupList`.
      Rust equivalent: iterate `build.skill_sets[build.active_skill_set].skills` filtering
      by `skill.slot == slot_name && skill.enabled == true`, then count `gem.enabled == true`.
    - Socket index comparison (line 1188: `if i > socketedGems`) uses 1-based Lua indexing.
      In Rust, enumerate from 1 when iterating flattened sockets.

11. **No config override for empty sockets** (`overrideEmptyRedSockets`, etc.).
    These config values are stored in `build.config.numbers` with the key names as-is.

## What Needs to Change

### In `crates/pob-calc/src/build/types.rs`

1. Add `foulborn: bool` field to `Item` struct (rare flag, can default to `false`).
2. Add `class_restriction: Option<String>` to `Item` struct.
3. Add `Relic` variant to `ItemRarity` enum, parsed same as `UNIQUE` for counting.

### In `crates/pob-calc/src/build/xml_parser.rs`

4. Parse `foulborn` attribute from item XML if present.
5. Parse `classRestriction` attribute and store in `Item::class_restriction`.
6. Parse `RELIC` rarity string to `ItemRarity::Relic`.

### In `crates/pob-calc/src/calc/setup.rs`, function `add_item_mods`

7. After the mod-parsing loop, add a new block implementing SETUP-11 logic:

   a. **Class-restriction condition** (lines 1132-1135):
      ```rust
      if let Some(ref restriction) = item.class_restriction {
          let key = item.name.replace(' ', "");
          env.player.mod_db.set_condition(&key, true);
      }
      ```

   b. **Skip block guard** (line 1136): skip for jewel/flask slots (already skipped at line
      1779 by `slot.is_flask() || slot.is_jewel()`). But also skip `Tincture` and `Graft`
      item types. Rust: `if item.item_type != "Tincture" && item.item_type != "Graft"`.

   c. **Rarity multiplier** (lines 1138-1151):
      ```rust
      let rarity_key = match item.rarity {
          ItemRarity::Unique | ItemRarity::Relic => {
              if item.foulborn {
                  *env.player.mod_db.multipliers.entry("FoulbornUniqueItem".into()).or_insert(0.0) += 1.0;
              }
              "UniqueItem"
          }
          ItemRarity::Rare   => "RareItem",
          ItemRarity::Magic  => "MagicItem",
          ItemRarity::Normal => "NormalItem",
      };
      *env.player.mod_db.multipliers.entry(rarity_key.into()).or_insert(0.0) += 1.0;
      env.player.mod_db.set_condition(&format!("{}In{}", rarity_key, slot_name), true);
      ```

   d. **Influence multipliers** (lines 1153-1162):
      ```rust
      let influences = [
          ("CorruptedItem",  item.corrupted),
          ("ShaperItem",     item.influence.shaper),
          ("ElderItem",      item.influence.elder),
          ("WarlordItem",    item.influence.warlord),
          ("HunterItem",     item.influence.hunter),
          ("CrusaderItem",   item.influence.crusader),
          ("RedeemerItem",   item.influence.redeemer),
      ];
      for (mult_key, has_it) in &influences {
          if *has_it {
              *env.player.mod_db.multipliers.entry(mult_key.to_string()).or_insert(0.0) += 1.0;
          } else {
              let non_key = format!("Non{}", mult_key);
              *env.player.mod_db.multipliers.entry(non_key).or_insert(0.0) += 1.0;
          }
      }
      if item.influence.shaper || item.influence.elder {
          *env.player.mod_db.multipliers.entry("ShaperOrElderItem".into()).or_insert(0.0) += 1.0;
      }
      ```

   e. **Item type multiplier** (line 1163):
      ```rust
      // item.item_type corresponds to item.type in Lua
      // Normalize: remove spaces, then strip "OneHanded"/"TwoHanded" prefix
      let type_key = item.item_type.replace(' ', "");
      // Lua: gsub(".+Handed", "") removes any prefix ending in "Handed"
      // Regex: r"^.*Handed" matches the part to strip
      let type_key = if let Some(after) = type_key.strip_prefix("TwoHanded") {
          after.to_string()
      } else if let Some(after) = type_key.strip_prefix("OneHanded") {
          after.to_string()
      } else {
          type_key
      };
      let mult_key = format!("{}Item", type_key);
      *env.player.mod_db.multipliers.entry(mult_key).or_insert(0.0) += 1.0;
      ```
      > **Caution:** Lua's regex `.+Handed` matches greedily, so `"TwoHandedSword"` becomes
      > `"Sword"`. The two `strip_prefix` calls above replicate this correctly for known cases.

   f. **Ring base-name multiplier** (lines 1165-1168):
      ```rust
      if item.item_type == "Ring" {
          let key = format!("{}Equipped", item.base_type.replace(' ', ""));
          *env.player.mod_db.multipliers.entry(key).or_insert(0.0) += 1.0;
      }
      ```

   g. **Socket counting** (lines 1169-1202): requires flattening `item.sockets`:
      ```rust
      let mut empty_counts = [0u32; 4]; // R, G, B, W
      let mut gem_socket_count: u32 = 0;
      // Count enabled gems for this slot
      let socketed_gems = build.skill_sets
          .get(build.active_skill_set)
          .map(|ss| ss.skills.iter()
              .filter(|s| s.slot == slot_name && s.enabled)
              .flat_map(|s| s.gems.iter())
              .filter(|g| g.enabled)
              .count())
          .unwrap_or(0) as u32;
      // Flatten socket groups to individual sockets, 1-based index
      let flat_sockets: Vec<char> = item.sockets.iter()
          .flat_map(|g| g.colors.iter().copied())
          .collect();
      for (i, color) in flat_sockets.iter().enumerate() {
          let idx = (i + 1) as u32; // 1-based
          match color {
              'R' | 'G' | 'B' | 'W' => {
                  gem_socket_count += 1;
                  if idx > socketed_gems {
                      match color {
                          'R' => empty_counts[0] += 1,
                          'G' => empty_counts[1] += 1,
                          'B' => empty_counts[2] += 1,
                          'W' => empty_counts[3] += 1,
                          _ => {}
                      }
                  }
              }
              _ => {} // 'A' (abyss) ignored
          }
      }
      let sg_key = format!("SocketedGemsIn{}", slot_name);
      *env.player.mod_db.multipliers.entry(sg_key).or_insert(0.0) +=
          gem_socket_count.min(socketed_gems) as f64;
      *env.player.mod_db.multipliers.entry("EmptyRedSocketsInAnySlot".into()).or_insert(0.0) += empty_counts[0] as f64;
      *env.player.mod_db.multipliers.entry("EmptyGreenSocketsInAnySlot".into()).or_insert(0.0) += empty_counts[1] as f64;
      *env.player.mod_db.multipliers.entry("EmptyBlueSocketsInAnySlot".into()).or_insert(0.0) += empty_counts[2] as f64;
      *env.player.mod_db.multipliers.entry("EmptyWhiteSocketsInAnySlot".into()).or_insert(0.0) += empty_counts[3] as f64;
      ```

8. **After the per-slot loop**, apply config overrides for empty socket counts (lines 1207-1210):
   ```rust
   let overrides = [
       ("overrideEmptyRedSockets",   "EmptyRedSocketsInAnySlot"),
       ("overrideEmptyGreenSockets", "EmptyGreenSocketsInAnySlot"),
       ("overrideEmptyBlueSockets",  "EmptyBlueSocketsInAnySlot"),
       ("overrideEmptyWhiteSockets", "EmptyWhiteSocketsInAnySlot"),
   ];
   for (cfg_key, mult_key) in &overrides {
       if let Some(&v) = build.config.numbers.get(*cfg_key) {
           env.player.mod_db.set_multiplier(mult_key, v);
       }
   }
   ```

### Notes on `itemModDB` vs `player.mod_db`

In Lua, all item-derived multipliers and conditions are written to `env.itemModDB`, which is then
merged into `env.modDB` via `mergeDB(env.modDB, env.itemModDB)` at line 1228. In Rust, there is
no separate `itemModDB` — everything goes directly into `env.player.mod_db`. The net effect is
identical since the merge is unconditional.

### Priority

The most impactful missing items are (in order):
1. **Rarity multipliers** (UniqueItem, RareItem) — referenced by many mods in generated code
2. **Influence multipliers** (ShaperItem, ElderItem) — affect elder/shaper-only builds
3. **Socket counts** (SocketedGemsIn*, EmptyRedSockets*) — affects several support gem mechanics
4. **Non* influence counters** — needed for "non-shaper" mod conditions
5. **Class restriction conditions**, ring base names, and config overrides — lower frequency
