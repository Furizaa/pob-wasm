# SETUP-14: Tattoo / Hash Overrides

## Output Fields

This chunk writes **no output fields directly**. Its effect is entirely in the
`ModDb`: it populates tattoo-type count multipliers such as
`Multiplier:KeystoneTattoo`, `Multiplier:StrTattoo`, etc., into
`env.modDB.multipliers[type]` so that mods with `{ type = "Multiplier", var =
"KeystoneTattoo" }` (e.g., "Limited to 1 Keystone Tattoo") evaluate correctly.

The related allocated-node-type counts (`Multiplier:AllocatedNotable`,
`Multiplier:AllocatedKeystone`, `Multiplier:AllocatedMastery`,
`Multiplier:AllocatedMasteryType`, `Multiplier:AllocatedLifeMastery`) are
written in the same CalcSetup.lua block and are included in this chunk's scope.

No oracle builds use tattoos, so this chunk has **no test coverage** via oracle.

## Dependencies

- SETUP-05 (cluster jewels) — builds `env.spec.allocNodes` which is the source
  of the node type counts
- SETUP-09 (mastery selections) — populates `env.spec.allocatedMasteryCount`
  and `env.spec.allocatedMasteryTypes`

## Lua Source

**Primary file:** `third-party/PathOfBuilding/src/Classes/PassiveSpec.lua`  
**Lines:** 83, 117–173, 258–265, 1312–1362 (hashOverride population, node
replacement, and `allocatedTattooTypes` accumulation)

**Secondary file:** `third-party/PathOfBuilding/src/Modules/CalcSetup.lua`  
**Lines:** 582–677 (reads `env.spec.allocatedTattooTypes`, adjusts for
`override.addNodes`/`removeNodes`, writes multipliers into `modDB`)

**Tertiary file:** `third-party/PathOfBuilding/src/Classes/PassiveTree.lua`  
**Lines:** 689–717 (builds tattoo node `modList` from `tree.tattoo.nodes`)

**Tattoo data:** `third-party/PathOfBuilding/src/Data/TattooPassives.lua`  
(auto-generated; contains every tattoo's `dn`, `isTattoo`, `overrideType`,
`targetType`, `sd`, `stats`, etc.)

**Commit:** `454eff8c85d24356d9b051d596983745ed367476`

## Annotated Lua

### PassiveSpec.lua lines 83, 117–173 — XML parsing and `hashOverrides` population

```lua
-- PassiveSpec:Init() — called when a PassiveSpec object is constructed.
-- Initialises self.hashOverrides as an empty table.
-- In Rust: PassiveSpec struct needs a `hash_overrides: HashMap<u32, TattooNode>`
-- field, initialized to HashMap::new().
self.hashOverrides = { }   -- line 83
```

```lua
-- PassiveSpec:Load() — called when the build XML is parsed.
-- This loop iterates over child elements of the <Spec> XML element.
-- It looks for <Overrides> blocks (tattoo slot data saved by the UI).

for _, node in pairs(xml) do
    if type(node) == "table" then
        if node.elem == "Overrides" then
            -- node is the <Overrides> container; iterate its children.
            for _, child in ipairs(node) do
                -- Each child is an <Override> element with attributes:
                --   nodeId       = tree node ID (u32) being overridden
                --   dn           = tattoo display name ("Acrobatics", "Ancestral Bond", …)
                --   icon         = path to the tattoo's icon asset
                --   activeEffectImage = path to the tattoo's active-effect background

                if not child.attrib.nodeId then
                    -- Missing nodeId = malformed XML; bail out.
                    launch:ShowErrMsg(...)
                    return true
                end

                -- Attempt to look up this tattoo by its display name (dn) in
                -- the pre-loaded tattoo node table.
                -- tree.tattoo.nodes is keyed by dn (e.g. "Acrobatics").
                -- In Rust: data.tattoo_nodes: HashMap<String, TattooNode>
                if not self.tree.tattoo.nodes[child.attrib.dn] then
                    -- The tattoo's dn wasn't found — it may have been renamed.
                    -- Fall back to matching by (activeEffectImage, icon) pair.
                    -- Lua: `pairs(t)` iterates ALL keys, unordered — Rust: `.iter()`
                    for name, data in pairs(self.tree.tattoo.nodes) do
                        if data["activeEffectImage"] == child.attrib["activeEffectImage"]
                            and data["icon"] == child.attrib["icon"]
                        then
                            -- Found a match by image assets; register it under the old dn.
                            -- This mutates tree.tattoo.nodes in place (global state!).
                            -- Rust: data.tattoo_nodes.insert(child_dn.clone(), data.clone())
                            self.tree.tattoo.nodes[child.attrib.dn] = data
                            ConPrintf(...)  -- logging only; no Rust equivalent needed
                        end
                    end
                end

                -- If lookup succeeded (either directly or via fallback), store.
                -- If it failed, silently skip this override to avoid crashing.
                if self.tree.tattoo.nodes[child.attrib.dn] then
                    local nodeId = tonumber(child.attrib.nodeId)
                    -- copyTable(..., true) = shallow copy; in Rust: .clone()
                    self.hashOverrides[nodeId] =
                        copyTable(self.tree.tattoo.nodes[child.attrib.dn], true)
                    -- Store the tree node ID on the tattoo object so ReplaceNode
                    -- can set old.id; in Rust: set hash_override.id = node_id
                    self.hashOverrides[nodeId].id = nodeId
                else
                    ConPrintf(...)  -- log failure; no Rust equivalent
                end
            end
        end
    end
end
-- After the Overrides loop, call ImportFromNodeList passing hashOverrides:
self:ImportFromNodeList(..., self.hashOverrides, masteryEffects)
```

### PassiveSpec.lua lines 258–265 — applying hashOverrides during ImportFromNodeList

```lua
-- ImportFromNodeList iterates hashOverrides and calls ReplaceNode on each.
-- This runs BEFORE the hashList allocation loop (lines 266–277), so tattooed
-- nodes have their stats replaced before they are marked as allocated.

for id, override in pairs(hashOverrides) do
    -- Rust: for (id, override) in &hash_overrides { ... }
    local node = self.nodes[id]
    if node then
        -- Populate sprite references from the tree's spriteMap (UI only; no Rust equiv).
        override.effectSprites = self.tree.spriteMap[override.activeEffectImage]
        override.sprites       = self.tree.spriteMap[override.icon]
        -- Replace the tree node's stats/mods with the tattoo's stats/mods.
        -- ReplaceNode copies: dn, sd, name, mods, modKey, modList, sprites, effectSprites,
        -- isTattoo, overrideType, keystoneMod, icon, spriteId, activeEffectImage,
        -- reminderText.
        self:ReplaceNode(node, override)
    end
end
```

### PassiveSpec.lua lines 1312–1362 — accumulating `allocatedTattooTypes`

```lua
-- This block is inside BuildAllDependsAndPaths(), which is called at the end
-- of ImportFromNodeList (line 287).
-- It rebuilds all node-type counters from scratch on every recalculation.
-- In Rust: rebuild these counts in setup.rs each time init_env() is called.

-- Reset all counters to zero.
self.allocatedMasteryCount    = 0
self.allocatedNotableCount    = 0
self.allocatedKeystoneCount   = 0
self.allocatedMasteryTypes    = { }   -- string->int map
self.allocatedMasteryTypeCount = 0
self.allocatedTattooTypes     = { }   -- string->int map (overrideType -> count)

for id, node in pairs(self.nodes) do
    -- self.ignoredNodes: nodes that existed in old tree version but are
    -- incompatible with the current tree; skip them.
    if self.ignoredNodes[id] and self.allocNodes[id] then
        -- De-allocate incompatible nodes (tree version conversion).
        self.nodes[id].alloc = false
        self.allocNodes[id]  = nil
        self.ignoredNodes[id] = nil
    else
        -- Count mastery types (for Multiplier:AllocatedMastery,
        -- Multiplier:AllocatedMasteryType, Multiplier:AllocatedLifeMastery).
        -- These are tracked here but live in CalcSetup.lua in the multiplier writes.
        if node.type == "Mastery" and self.masterySelections[id] then
            -- ... mastery counting omitted (covered by SETUP-09) ...
        elseif node.type == "Notable" and node.alloc then
            self.allocatedNotableCount = self.allocatedNotableCount + 1
        elseif node.type == "Keystone" and node.alloc then
            self.allocatedKeystoneCount = self.allocatedKeystoneCount + 1
        end

        -- Count tattoo types for each ALLOCATED tattooed node.
        -- A node is tattooed if ReplaceNode was called on it (isTattoo = true).
        -- overrideType is a string like "KeystoneTattoo", "StrTattoo", etc.
        -- Rust: if node.is_tattoo && node.alloc && node.override_type.is_some() { ... }
        if node.isTattoo and node.alloc and node.overrideType then
            if not self.allocatedTattooTypes[node.overrideType] then
                -- First occurrence of this tattoo type.
                -- `x or 0` nil-coalescing not needed here — explicit branch.
                self.allocatedTattooTypes[node.overrideType] = 1
            else
                self.allocatedTattooTypes[node.overrideType] =
                    self.allocatedTattooTypes[node.overrideType] + 1
            end
        end
    end
end
```

### CalcSetup.lua lines 582–677 — reading counters and writing multipliers

```lua
-- This block is inside calcs.initEnv(), AFTER accelerate checks.
-- env.spec is the PassiveSpec object constructed above.

-- Read cached counters from env.spec (populated by BuildAllDependsAndPaths).
local allocatedNotableCount     = env.spec.allocatedNotableCount
local allocatedKeystoneCount    = env.spec.allocatedKeystoneCount
local allocatedMasteryCount     = env.spec.allocatedMasteryCount
local allocatedMasteryTypeCount = env.spec.allocatedMasteryTypeCount
-- copyTable() = shallow copy; Rust: .clone()
local allocatedMasteryTypes     = copyTable(env.spec.allocatedMasteryTypes)
local allocatedTattooTypes      = copyTable(env.spec.allocatedTattooTypes)

-- If in "override mode" (e.g. the Calcs tab is simulating a gear swap),
-- addNodes / removeNodes adjust the counts incrementally.
-- override is a table with optional fields addNodes and removeNodes.
-- In Rust: CalcOverride struct with optional node sets. Not needed for oracle builds.
if not accelerate.nodeAlloc then
    local nodes
    if override.addNodes or override.removeNodes then
        nodes = { }
        if override.addNodes then
            for node in pairs(override.addNodes) do   -- pairs() = unordered iteration
                nodes[node.id] = node
                -- Increment type counters for added nodes.
                if node.type == "Notable" then
                    allocatedNotableCount = allocatedNotableCount + 1
                elseif node.type == "Keystone" then
                    allocatedKeystoneCount = allocatedKeystoneCount + 1
                end
                -- Tattoo type increment for added nodes:
                if node.isTattoo and node.overrideType then
                    if not allocatedTattooTypes[node.overrideType] then
                        allocatedTattooTypes[node.overrideType] = 1
                    else
                        local prevCount = allocatedTattooTypes[node.overrideType]
                        allocatedTattooTypes[node.overrideType] = prevCount + 1
                    end
                end
            end
        end
        for _, node in pairs(env.spec.allocNodes) do
            if not override.removeNodes or not override.removeNodes[node] then
                nodes[node.id] = node
            elseif override.removeNodes[node] then
                -- Decrement type counters for removed nodes.
                if node.type == "Notable" then
                    allocatedNotableCount = allocatedNotableCount - 1
                elseif node.type == "Keystone" then
                    allocatedKeystoneCount = allocatedKeystoneCount - 1
                end
                -- Tattoo type decrement for removed nodes:
                if node.isTattoo and node.overrideType then
                    if allocatedTattooTypes[node.overrideType] then
                        allocatedTattooTypes[node.overrideType] =
                            allocatedTattooTypes[node.overrideType] - 1
                    end
                end
            end
        end
    else
        nodes = copyTable(env.spec.allocNodes, true)
    end
    env.allocNodes          = nodes
    env.initialNodeModDB    = calcs.buildModListForNodeList(env, env.allocNodes, true)
    modLib.mergeKeystones(env, env.initialNodeModDB)
end

-- ── Write node-type multipliers to modDB ─────────────────────────────────────

-- All four of these use modDB:NewMod("Multiplier:X", "BASE", count).
-- In Rust: db.add(Mod::new_base("Multiplier:AllocatedNotable", count as f64, src))
-- `x and x > 0` guards against nil (if count was never set) OR zero.
-- Rust: if count > 0 { db.add(...) }

if allocatedNotableCount and allocatedNotableCount > 0 then
    modDB:NewMod("Multiplier:AllocatedNotable", "BASE", allocatedNotableCount)
end
if allocatedKeystoneCount and allocatedKeystoneCount > 0 then
    modDB:NewMod("Multiplier:AllocatedKeystone", "BASE", allocatedKeystoneCount)
end
if allocatedMasteryCount and allocatedMasteryCount > 0 then
    modDB:NewMod("Multiplier:AllocatedMastery", "BASE", allocatedMasteryCount)
end
if allocatedMasteryTypeCount and allocatedMasteryTypeCount > 0 then
    modDB:NewMod("Multiplier:AllocatedMasteryType", "BASE", allocatedMasteryTypeCount)
end
-- Life Mastery specifically has its own multiplier key.
-- allocatedMasteryTypes is a string→int map (mastery name → count).
-- `allocatedMasteryTypes["Life Mastery"] and ... > 0` avoids both nil and zero.
-- Rust: if let Some(&n) = alloc_mastery_types.get("Life Mastery") { if n > 0 { ... } }
if allocatedMasteryTypes["Life Mastery"] and allocatedMasteryTypes["Life Mastery"] > 0 then
    modDB:NewMod("Multiplier:AllocatedLifeMastery", "BASE", allocatedMasteryTypes["Life Mastery"])
end

-- ── Write tattoo type multipliers (the core of SETUP-14) ─────────────────────
-- allocatedTattooTypes: string → int (e.g. "KeystoneTattoo" → 1, "StrTattoo" → 3)
-- DIFFERENT from the above: these go directly into env.modDB.multipliers (a raw
-- key→value table), NOT via modDB:NewMod("Multiplier:X", "BASE", ...).
-- env.modDB.multipliers is the backing store that Multiplier-type mod tags read.
-- In Lua: `env.modDB.multipliers[type] = count` sets the raw multiplier directly.
-- In Rust: `env.player.mod_db.set_multiplier(type_name, count as f64)`
--          where `mod_db.multipliers: HashMap<String, f64>`.
--
-- `if allocatedTattooTypes then` — truthy check, always true since copyTable
-- returns an empty table (not nil) when the source was empty.
-- The loop uses `pairs()` (unordered iteration over all keys).
-- Rust: for (tattoo_type, count) in &alloc_tattoo_types { ... }
if allocatedTattooTypes then
    for type, count in pairs(allocatedTattooTypes) do
        env.modDB.multipliers[type] = count
    end
end
```

### PassiveSpec.lua line 1541–1562 — ReplaceNode

```lua
-- ReplaceNode(old, newNode): Overwrites a tree node's stats/mods with tattoo data.
-- old    = an allocated node from self.nodes (a tree node with metatable)
-- newNode = a tattoo entry from tree.tattoo.nodes (loaded from TattooPassives.lua)
--
-- Returns 1 (truthy) without modification if old.sd == newNode.sd (already identical).
-- In Rust: if old_stats == new_stats { return; }
function PassiveSpecClass:ReplaceNode(old, newNode)
    if old.sd == newNode.sd then
        return 1
    end
    -- These fields are replaced on the node in-place (mutating the live tree node):
    old.dn              = newNode.dn              -- display name
    old.sd              = newNode.sd              -- stat descriptions (Vec<String>)
    old.name            = newNode.name            -- internal name
    old.mods            = newNode.mods            -- parsed modifier records
    old.modKey          = newNode.modKey          -- cache key for ModList
    old.modList         = new("ModList")          -- fresh ModList
    old.modList:AddList(newNode.modList)          -- copy tattoo mods in
    old.sprites         = newNode.sprites         -- UI artwork (no Rust equiv)
    old.effectSprites   = newNode.effectSprites   -- UI artwork (no Rust equiv)
    old.isTattoo        = newNode.isTattoo        -- true for all tattoo nodes
    old.overrideType    = newNode.overrideType    -- e.g. "KeystoneTattoo", "StrTattoo"
    old.keystoneMod     = newNode.keystoneMod     -- if keystone tattoo, the keystone mod
    old.icon            = newNode.icon
    old.spriteId        = newNode.spriteId
    old.activeEffectImage = newNode.activeEffectImage
    old.reminderText    = newNode.reminderText or { }  -- `or {}` nil-coalesces to empty Vec
end
```

### PassiveTree.lua lines 689–717 — building tattoo node ModLists

```lua
-- Called during PassiveTree construction (before any build is loaded).
-- Iterates tree.tattoo.nodes (loaded from TattooPassives.lua) and:
-- 1. Assigns a type string ("Keystone", "Notable", "Mastery", "Normal")
-- 2. Resolves sprites from spriteMap (UI; no Rust equivalent)
-- 3. Creates idMap for breakdown sourcing
-- 4. Calls tree:ProcessStats(node) to parse the stat lines (sd) into
--    a ModList (equivalent to calling the mod parser on each stat line).
-- In Rust: when building TattooData, call mod_parser::parse_stats(node.sd)
--          on each tattoo node to populate its ModList.

for _, node in pairs(self.tattoo.nodes) do
    -- node.m, node.ks, node["not"] are boolean flags in TattooPassives.lua
    if node.m then
        node.type = "Mastery"
    elseif node.ks then
        node.type = "Keystone"
    elseif node["not"] then     -- note: Lua uses ["not"] because "not" is a keyword
        node.type = "Notable"
    else
        node.type = "Normal"
    end
    -- ... sprite code omitted (UI only) ...
    self:ProcessStats(node)  -- parses node.sd into node.modList
end
```

### TattooPassives.lua data structure

Each tattoo entry has:

```lua
["Acrobatics"] = {
    ["dn"]              = "Acrobatics",         -- display name (key into tree.tattoo.nodes)
    ["id"]              = "acrobatics1136",      -- internal string ID (NOT the u32 node ID)
    ["isTattoo"]        = true,
    ["ks"]              = true,                 -- is keystone
    ["m"]               = false,                -- is mastery
    ["not"]             = false,                -- is notable (["not"] avoids keyword)
    ["overrideType"]    = "KeystoneTattoo",     -- multiplier type string
    ["targetType"]      = "Keystone",           -- which node types can receive this tattoo
    ["targetValue"]     = "",                   -- specific target name, or ""
    ["MaximumConnected"]= 100,                  -- max adjacent node connections
    ["MinimumConnected"]= 0,
    ["activeEffectImage"] = "Art/.../bg.png",   -- UI asset paths
    ["icon"]              = "Art/.../icon.png",
    ["sd"] = {                                  -- stat description lines (1-indexed!)
        [1] = "Modifiers to Chance to Suppress Spell Damage...",
        [2] = "Maximum Chance to Dodge Spell Hits is 75%",
        [3] = "Limited to 1 Keystone Tattoo",
    },
    ["stats"] = {
        ["keystone_acrobatics"] = {
            ["fmt"] = "d", ["index"] = 1, ["max"] = 1, ["min"] = 1, ["statOrder"] = 10882,
        },
    },
}
```

The `overrideType` values observed in TattooPassives.lua include:
`"KeystoneTattoo"`, `"StrTattoo"`, `"DexTattoo"`, `"IntTattoo"`,
`"JourneyTattooBody"`, `"JourneyTattooSoul"`, `"JourneyTattooMind"`,
and others. The Multiplier key in mods like `"per Journey Tattoo of the Body"` is
`Multiplier:JourneyTattooBody` (already in `mod_parser_generated.rs`).

## Key Lua Semantics

### `env.modDB.multipliers[type] = count` vs `modDB:NewMod(...)`

The tattoo-type counts use a **different write path** from other multipliers:

- For `AllocatedNotable` / `AllocatedKeystone` / etc.:
  `modDB:NewMod("Multiplier:X", "BASE", count)` — goes through the normal
  `ModDb` pipeline; creates a `Mod` object with name `"Multiplier:X"`, type
  `"BASE"`, value `count`.

- For tattoo types:
  `env.modDB.multipliers[type] = count` — writes **directly** into the
  `multipliers` backing table, bypassing `NewMod`. This is equivalent to
  `mod_db.set_multiplier(type, count as f64)` in Rust. The `ModDb::Sum`
  implementation checks `multipliers[var]` when evaluating a
  `{ type = "Multiplier", var = "X" }` tag.

**Do not** use `db.add(Mod::new_base(...))` for tattoo types. Use
`mod_db.set_multiplier(...)`.

### `local type` variable shadowing Lua keyword

In the loop:
```lua
for type, count in pairs(allocatedTattooTypes) do
    env.modDB.multipliers[type] = count
end
```
`type` is used as a loop variable even though `type()` is a Lua built-in. This
works because `local type` in a `for` header creates a new local that shadows
the global. In Rust, use `tattoo_type` or `type_name` as the variable name to
avoid conflict with the `type` keyword.

### `copyTable(env.spec.allocatedTattooTypes)` is a snapshot

`copyTable` (PoB's utility) makes a shallow copy of the table. The loop that
follows then modifies the local copy when `override.addNodes`/`removeNodes` are
present. This snapshot pattern means:

1. `env.spec.allocatedTattooTypes` is **never modified** by CalcSetup — it is
   read-only here.
2. The local `allocatedTattooTypes` may differ from `env.spec.allocatedTattooTypes`
   if gear-swap overrides added or removed tattooed nodes.

In Rust: `let mut alloc_tattoo_types = env.spec.alloc_tattoo_types.clone();`

### `for node in pairs(override.addNodes)` — keys are nodes

Note the unusual iteration: `for node in pairs(override.addNodes)` iterates the
**keys** of `override.addNodes` (not the values), because `addNodes` is a set
keyed by node objects. The value is always `true` (or some truthy non-nil). The
equivalent Rust pattern is `for node in override.add_nodes.keys()`.

### `node["not"]` avoids the Lua `not` keyword

In TattooPassives.lua, notable tattoos are identified by `node["not"] == true`
(bracket syntax to avoid using `not` as an identifier). In Rust this field
would be named `is_notable: bool`.

## Existing Rust Code

**Files:**
- `crates/pob-calc/src/build/xml_parser.rs` — XML parsing
- `crates/pob-calc/src/build/types.rs` — PassiveSpec struct
- `crates/pob-calc/src/passive_tree/mod.rs` — PassiveTree, PassiveNode
- `crates/pob-calc/src/calc/setup.rs` — init_env and multiplier writes

### What exists

- `xml_parser.rs`: The `<Spec>` element is parsed (lines 139–163). The `<Sockets>`
  child is handled (lines 165–183). The `<Overrides>` element is **not handled** —
  it is silently skipped by the catch-all `_ => {}` branch (line 229).
- `types.rs`: `PassiveSpec` (lines 21–30) has `tree_version`, `allocated_nodes`,
  `class_id`, `ascend_class_id`, and `jewels`. **No `hash_overrides` field.**
- `passive_tree/mod.rs`: `PassiveNode` (lines 94–115) has `id`, `name`, `stats`,
  `linked_ids`, `node_type`, `ascendancy_name`, `icon`, `skill_points_granted`,
  `class_start_index`, `expansion_jewel`. **No `is_tattoo`, `override_type`, or
  `mod_list` fields.**
- `setup.rs`: Iterates `build.passive_spec.allocated_nodes` to build the mod
  database from passive node stats (via `passive_node_mods()`). Does not count
  node types per allocated node. **No `Multiplier:AllocatedNotable` etc. writes.**
  No `set_multiplier` calls for tattoo types.
- `mod_parser_generated.rs` (lines 7097–7099, 38606–38660): Three tattoo-related
  `Multiplier:JourneyTattooBody`, `Multiplier:JourneyTattooSoul`,
  `Multiplier:JourneyTattooMind` patterns exist, but the generated stubs have
  `ModFlags::NONE` — they do not evaluate correctly. The multiplier counts they
  reference are never populated.

### What's missing

1. **XML parsing of `<Overrides>`** — the `<Overrides>` and `<Override>` elements
   inside `<Spec>` are silently ignored. No tattoo override data is captured.
2. **`PassiveSpec.hash_overrides`** field — `types.rs` has no place to store the
   parsed override data.
3. **Tattoo data loading** — no `TattooData` struct or equivalent exists. The
   TattooPassives.lua data is not exposed to the Rust calculation engine.
4. **`PassiveNode.is_tattoo` and `PassiveNode.override_type`** — the `PassiveNode`
   struct has no fields to mark whether a node has been replaced by a tattoo, or
   what tattoo type it is. The `ReplaceNode` operation has no Rust equivalent.
5. **Node-type multiplier writes** — `Multiplier:AllocatedNotable`,
   `Multiplier:AllocatedKeystone`, `Multiplier:AllocatedMastery`,
   `Multiplier:AllocatedMasteryType`, `Multiplier:AllocatedLifeMastery` are not
   written to the `ModDb` anywhere in `setup.rs`.
6. **Tattoo-type multiplier writes** — `set_multiplier(tattoo_type, count)` is not
   called for any `overrideType` string.

### What's wrong

- The `mod_parser_generated.rs` stubs for `Multiplier:JourneyTattoo*` generate
  mods with `ModFlags::NONE` instead of evaluating against the multiplier
  backing table. However, this is a SETUP-04 (eval_mod stubs) issue — the fix
  there will make these consult `mod_db.multipliers["JourneyTattooBody"]` etc.,
  which will be zero/absent until this chunk populates them.
- Because `<Overrides>` is silently ignored, builds with tattoos will have all
  tattooed nodes retain their original tree stats (no override applied), and all
  tattoo-type multipliers will be zero.

## What Needs to Change

1. **Add `hash_overrides: HashMap<u32, TattooOverrideNode>` to `PassiveSpec`**
   (`crates/pob-calc/src/build/types.rs`):
   ```rust
   pub struct PassiveSpec {
       // ... existing fields ...
       /// Maps tree node ID → tattoo override data. Populated from <Overrides> XML.
       pub hash_overrides: HashMap<u32, TattooOverrideNode>,
   }

   /// Tattoo replacement data for a single passive tree node.
   /// Mirrors what ReplaceNode copies from tree.tattoo.nodes.
   pub struct TattooOverrideNode {
       pub node_id: u32,       // the tree node ID this tattoo replaces
       pub dn: String,         // display name ("Acrobatics", "Ancestral Bond", ...)
       pub is_tattoo: bool,    // always true for tattoo nodes
       pub override_type: String, // e.g. "KeystoneTattoo", "StrTattoo"
       pub is_keystone: bool,
       pub is_notable: bool,
       pub is_mastery: bool,
       pub stats: Vec<String>, // mod description lines (the "sd" field)
   }
   ```

2. **Parse `<Overrides>` in the XML parser** (`crates/pob-calc/src/build/xml_parser.rs`):
   - Add `in_spec_overrides: bool` state flag (like `in_spec_sockets`).
   - On `<Overrides>` start: set flag to `true`.
   - On `<Override>` start (while in overrides):
     - Parse `nodeId`, `dn`, `icon`, `activeEffectImage` attributes.
     - Look up `dn` in `data.tattoo_nodes`.
     - If not found, attempt fallback lookup by `(activeEffectImage, icon)`.
     - If found: push `TattooOverrideNode` into `passive_spec.hash_overrides`.
     - If not found: skip (mirrors the `ConPrintf(...)` fallback).
   - On `<Overrides>` end: reset flag.

3. **Load tattoo node data** (`crates/pob-calc/src/data/` or a new
   `tattoo_passives.rs`): Parse `TattooPassives.lua` (or a generated JSON
   equivalent) into `data.tattoo_nodes: HashMap<String, TattooNodeData>` keyed
   by `dn`. This requires either a Lua-to-JSON conversion step at build time, or
   a handwritten parser for the Lua table literal format.

4. **Track node type counts and tattoo type counts in `setup.rs`**:
   After building the effective allocated node set (currently done at
   `connected_passive_nodes()` and `passive_node_mods()`), iterate the final
   allocated nodes and accumulate:
   - `allocated_notable_count: u32` — count of `NodeType::Notable` nodes
   - `allocated_keystone_count: u32` — count of `NodeType::Keystone` nodes
   - `alloc_tattoo_types: HashMap<String, u32>` — counts per `override_type`

   Then write to `modDB`:
   ```rust
   // Mirrors CalcSetup.lua lines 658–676
   if allocated_notable_count > 0 {
       db.add(Mod::new_base("Multiplier:AllocatedNotable", allocated_notable_count as f64, src.clone()));
   }
   if allocated_keystone_count > 0 {
       db.add(Mod::new_base("Multiplier:AllocatedKeystone", allocated_keystone_count as f64, src.clone()));
   }
   // ... AllocatedMastery, AllocatedMasteryType, AllocatedLifeMastery (SETUP-09) ...

   // Tattoo types — use set_multiplier, NOT new_base:
   // (Mirrors: env.modDB.multipliers[type] = count)
   for (tattoo_type, count) in &alloc_tattoo_types {
       env.player.mod_db.set_multiplier(tattoo_type, *count as f64);
   }
   ```

5. **Apply tattoo overrides when building node mod lists** — after parsing
   `passive_spec.hash_overrides`, when iterating `allocated_nodes` in
   `passive_node_mods()`, replace the node's stats with the tattoo's stats for
   any node ID that appears in `hash_overrides`. This mirrors `ReplaceNode` in
   PassiveSpec.lua (lines 1541–1562). The override must also set `is_tattoo =
   true` and `override_type = ...` on the effective node so the counting in step
   4 works correctly.

6. **(Deferred) Support `override.addNodes` / `override.removeNodes` override mode**
   — the Calcs-tab gear-swap simulation path (CalcSetup.lua lines 594–656). Not
   needed for oracle builds. Can be left out of the initial implementation.
