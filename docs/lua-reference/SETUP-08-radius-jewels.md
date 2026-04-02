# SETUP-08: Radius Jewel Framework

## Output Fields

This chunk produces **no output fields directly**. Like SETUP-05 through SETUP-07, it
populates the ModDb by controlling which passive node mods are applied, scaled, or
suppressed. Correct radius jewel processing is a prerequisite for every downstream chunk.

The one oracle build affected is `realworld_coc_trigger` (Thread of Hope).

## Dependencies

- **SETUP-01** — items must be parsed and slots populated so `item.jewelRadiusIndex` is set
- **SETUP-05** — cluster jewel subgraph must exist so radius jewels can see those nodes
- **SETUP-06** — timeless jewel node replacement must have run before radius processing

## Lua Source

File: `third-party/PathOfBuilding/src/Modules/CalcSetup.lua`, lines 113–210, 751–808, 1262–1275  
Commit: `454eff8c85d24356d9b051d596983745ed367476`

## Overview

The radius jewel system operates in two passes during `buildModListForNodeList`:

1. **First pass ("Other" type jewels):** Called per-node inside `buildModListForNode`.
   Jewels with `type == "Other"` (e.g. Glorious Vanity — timeless jewels) are processed
   before effect scaling.

2. **Effect scaling:** `calcLib.mod(modList, nil, "PassiveSkillEffect")` scales all node
   mods by a factor if non-1. This is where "% increased effect of small passive skills"
   from jewels manifests.

3. **PassiveSkillHasNoEffect / AllocatedPassiveSkillHasNoEffect suppression:** Wipes the
   modList entirely if flagged (Thread of Hope makes unallocated nodes count but not
   their mods, while Impossible Escape keystones are a special case).

4. **Second pass ("Threshold", "Self", "SelfUnalloc" types):** The main radius jewel
   dispatch — runs per-node after scaling, with type-gated logic:
   - `"Self"`: only fires when the node is **allocated** (`env.allocNodes[node.id]`)
   - `"SelfUnalloc"`: only fires when the node is **not allocated**
   - `"Threshold"`: fires unconditionally for any node in radius

5. **PassiveSkillHasOtherEffect:** Replaces the entire modList with `NodeModifier` list
   mods if flagged.

6. **finishJewels:** When the outer call sets `finishJewels=true` (lines 189–208):
   - Processes `env.extraRadiusNodeList` (unallocated nodes pulled in by non-Self jewels)
   - Finalises all radius jewels by calling `rad.func(nil, modList, rad.data)` with
     `node=nil` — this is the jewel's aggregation/summary callback

7. **ExtraJewelFunc re-entry loop (lines 1266–1275):** After the second
   `buildModListForNodeList` call, if `ExtraJewelFunc` mods appeared in the modDB
   (from items like Mjolner's "Socketed Lightning Spells have N% increased Effect in
   radius"), the entire `initEnv` is called recursively with those funcs registered.

## Annotated Lua

### `calcs.buildModListForNode(env, node)` — lines 113–167

```lua
function calcs.buildModListForNode(env, node)
    -- Creates a fresh ModList for this single passive node.
    -- In Rust: Vec<Mod> or a local ModDb.
    local modList = new("ModList")

    -- Keystones contribute via the single keystoneMod; everything else via modList.
    -- In PoB, node.type is a string: "Keystone", "Notable", "Small", "Mastery", etc.
    if node.type == "Keystone" then
        modList:AddMod(node.keystoneMod)       -- single Mod object
    else
        modList:AddList(node.modList)          -- list of Mod objects
    end

    -- ── FIRST PASS: "Other"-type radius jewels ────────────────────────────────
    -- `env.radiusJewelList` is a Lua array of jewel descriptors built during item
    -- processing (see lines 751-808). Each descriptor is a table:
    --   { nodes = {nodeId→node, ...},   -- set of nodeIds in this jewel's radius
    --     func  = function(node, out, data) ... end,
    --     type  = "Self"|"SelfUnalloc"|"Threshold"|"Other",
    --     item  = <item table>,
    --     nodeId = <socket node id>,
    --     attributes = {nodeId→attrTable, ...},
    --     data  = { }  -- mutable per-run accumulator
    --   }
    --
    -- In Rust: `env.radius_jewel_list: Vec<RadiusJewelEntry>`.
    for _, rad in pairs(env.radiusJewelList) do
        -- Only "Other" type jewels run in the first pass.
        -- `rad.nodes[node.id]` checks if this specific passive node is within the
        -- jewel's radius. The check `~= "Mastery"` skips mastery nodes (they have
        -- no real stats to transform).
        -- In Rust: `rad.nodes.contains_key(&node.id) && node.node_type != NodeType::Mastery`
        if rad.type == "Other" and rad.nodes[node.id] and rad.nodes[node.id].type ~= "Mastery" then
            rad.func(node, modList, rad.data)
            -- `rad.func` mutates `modList` and/or `rad.data` in-place.
            -- The function signature is always (node, out_modList, data_accumulator).
            -- When node==nil (finalise call), it may write summary stats.
        end
    end

    -- ── SUPPRESSION CHECK ────────────────────────────────────────────────────
    -- PassiveSkillHasNoEffect: set by some radius jewels (e.g. Glorious Vanity
    -- in Maraketh mode) to zero out the node's normal stats.
    -- AllocatedPassiveSkillHasNoEffect: only zeroes if this node is allocated.
    -- `wipeTable(modList)` empties the modList but keeps the Lua table object alive.
    -- In Rust: `mod_list.clear()` — just truncate/drain the Vec.
    if modList:Flag(nil, "PassiveSkillHasNoEffect") or
       (env.allocNodes[node.id] and modList:Flag(nil, "AllocatedPassiveSkillHasNoEffect")) then
        wipeTable(modList)
    end

    -- ── EFFECT SCALING ───────────────────────────────────────────────────────
    -- `calcLib.mod(modList, nil, "PassiveSkillEffect")` computes:
    --   (1 + modList:Sum("INC", nil, "PassiveSkillEffect") / 100)
    --   * modList:More(nil, "PassiveSkillEffect")
    -- This is the passive skill effect multiplier (e.g. from Lethal Pride
    -- granting "10% increased effect of small passive skills in radius").
    -- Default is 1.0 (no scaling). Only build a new scaled list if non-1.
    --
    -- In Rust: `calc_mod(&mod_list, None, "PassiveSkillEffect")`.
    -- If result != 1.0, create a new Vec<Mod> with all values scaled.
    -- `ScaleAddList(src, scale)` multiplies BASE mod values by `scale` then adds them.
    local scale = calcLib.mod(modList, nil, "PassiveSkillEffect")
    if scale ~= 1 then
        local scaledList = new("ModList")
        scaledList:ScaleAddList(modList, scale)  -- multiplies all BASE values by scale
        modList = scaledList                      -- shadow the old modList; Lua GC reclaims it
    end

    -- ── SECOND PASS: Threshold / Self / SelfUnalloc jewels ──────────────────
    -- These jewels do their per-node work AFTER effect scaling.
    -- Gate logic:
    --   "Threshold" → always fires for any node in this jewel's radius
    --   "Self"      → only fires for ALLOCATED nodes
    --   "SelfUnalloc" → only fires for UNALLOCATED nodes
    --
    -- The mastery-type exclusion applies here too.
    for _, rad in pairs(env.radiusJewelList) do
        if rad.nodes[node.id] and rad.nodes[node.id].type ~= "Mastery" and
           (rad.type == "Threshold" or
            (rad.type == "Self" and env.allocNodes[node.id]) or
            (rad.type == "SelfUnalloc" and not env.allocNodes[node.id])) then
            rad.func(node, modList, rad.data)
        end
    end

    -- ── PassiveSkillHasOtherEffect ────────────────────────────────────────────
    -- Some jewels replace the normal node stats entirely with custom NodeModifier
    -- list entries. When flagged:
    --   1. Wipe the modList (on i==1 i.e. the first iteration)
    --   2. Re-populate it with mod.mod values from "NodeModifier" LIST entries
    -- `modList:List(skillCfg, "NodeModifier")` — note it passes `skillCfg` not `nil`.
    -- `skillCfg` here is the outer function's closure variable (the active skill cfg).
    -- In Rust: query mod_list for NodeModifier LIST entries, clear, re-add.
    if modList:Flag(nil, "PassiveSkillHasOtherEffect") then
        for i, mod in ipairs(modList:List(skillCfg, "NodeModifier")) do
            if i == 1 then wipeTable(modList) end
            modList:AddMod(mod.mod)
        end
    end

    -- ── ExtraSkill grants ────────────────────────────────────────────────────
    -- Some allocated nodes grant extra skills (e.g. "Grants Summon Harbinger Skill").
    -- These are collected per node into node.grantedSkills for downstream processing.
    -- "Unknown" skills are intentionally skipped.
    node.grantedSkills = { }
    for _, skill in ipairs(modList:List(nil, "ExtraSkill")) do
        if skill.name ~= "Unknown" then
            t_insert(node.grantedSkills, {
                skillId   = skill.skillId,
                level     = skill.level,
                noSupports = true,
                source    = "Tree:"..node.id  -- string concat: "Tree:" + nodeId
                -- In Rust: format!("Tree:{}", node.id)
            })
        end
    end

    -- Returns (modList, canExplode_node_or_nil)
    -- Second return is used to accumulate explodeSources.
    -- `modList:Flag(nil, "CanExplode") and node` — Lua: if flag true, returns node; else nil
    -- In Rust: if mod_list.flag(None, "CanExplode") { Some(node) } else { None }
    return modList, modList:Flag(nil, "CanExplode") and node
end
```

### `calcs.buildModListForNodeList(env, nodeList, finishJewels)` — lines 170–211

```lua
function calcs.buildModListForNodeList(env, nodeList, finishJewels)
    -- ── Reset jewel data accumulators ───────────────────────────────────────
    -- Each jewel's `data` table is a mutable accumulator reset at the start of
    -- every build pass. `wipeTable` in Lua empties a table in-place (all keys → nil).
    -- In Rust: for rad in &mut env.radius_jewel_list { rad.data.clear(); }
    for _, rad in pairs(env.radiusJewelList) do
        wipeTable(rad.data)
        -- `modSource` is set on data so the jewel func can tag mods with their source.
        -- "Tree:" .. rad.nodeId produces e.g. "Tree:12345"
        rad.data.modSource = "Tree:"..rad.nodeId
    end

    -- ── Per-node mod collection ──────────────────────────────────────────────
    local modList = new("ModList")
    local explodeSources = {}
    for _, node in pairs(nodeList) do
        -- `pairs` on nodeList iterates all k,v — nodeList is {nodeId → nodeObject}.
        -- In Rust: for (_, node) in &node_list { ... }
        local nodeModList, explode = calcs.buildModListForNode(env, node)
        t_insert(explodeSources, explode)     -- may insert nil; Lua ignores that
        modList:AddList(nodeModList)          -- merge into combined list

        -- Store per-node final modList for UI breakdowns only when in MAIN mode.
        -- In Rust: if env.mode == CalcMode::Main { node.final_mod_list = ...; }
        if env.mode == "MAIN" then
            node.finalModList = nodeModList
        end
    end

    -- ── finishJewels: extra radius nodes + finalise jewel funcs ─────────────
    -- Called with finishJewels=true at lines 654 and 1262.
    if finishJewels then
        -- extraRadiusNodeList contains unallocated nodes that were added because a
        -- non-Self jewel (Threshold, SelfUnalloc) has them in its radius.
        -- These nodes don't contribute to the player's modDB directly, but their
        -- mods ARE processed so the jewel func can read them.
        for _, node in pairs(env.extraRadiusNodeList) do
            local nodeModList = calcs.buildModListForNode(env, node)
            if env.mode == "MAIN" then
                node.finalModList = nodeModList
            end
        end

        -- Finalise each radius jewel: call func(nil, modList, data).
        -- `nil` as first arg signals "no specific node — do summary/aggregation".
        -- The func may write summary mods into `modList` at this point.
        -- In MAIN mode, store the accumulated data on the item for UI display.
        for _, rad in pairs(env.radiusJewelList) do
            rad.func(nil, modList, rad.data)
            if env.mode == "MAIN" then
                if not rad.item.jewelRadiusData then
                    rad.item.jewelRadiusData = { }
                end
                -- Keyed by the socket node id, so multiple jewels in different
                -- sockets don't overwrite each other.
                rad.item.jewelRadiusData[rad.nodeId] = rad.data
            end
        end
    end

    return modList, explodeSources
end
```

### Radius jewel list construction — lines 751–808

```lua
-- Inside calcs.initEnv, during the item-slot iteration loop:
if item and ( item.jewelRadiusIndex or
             (override and override.extraJewelFuncs and #override.extraJewelFuncs > 0) ) then
    -- `item.jewelRadiusIndex` is set by the item parser when the item has a JewelData
    -- LIST mod with key="radiusIndex". Currently stubbed to 0.0 in Rust (TODO in
    -- mod_parser_generated.rs lines 32774-32891).
    --
    -- `item.jewelData.funcList` contains the per-jewel-type callbacks that implement
    -- the jewel's effect on nodes in radius. Each entry: { type=..., func=... }
    -- If not present, falls back to a default "tally Str/Dex/Int" function.
    local funcList = (item.jewelData and item.jewelData.funcList) or {
        { type = "Self", func = function(node, out, data)
            -- Default function: just count Str/Dex/Int in radius for display.
            -- `out` here is the node's modList, not the output table!
            -- In Rust: for stat in ["Str","Dex","Int"] { data[stat] += out.sum(None, stat); }
            if node then
                for _, stat in pairs({"Str","Dex","Int"}) do
                    data[stat] = (data[stat] or 0) + out:Sum("BASE", nil, stat)
                end
            end
        end }
    }

    for _, func in ipairs(funcList) do
        local node = env.spec.nodes[slot.nodeId]   -- the jewel socket node
        t_insert(env.radiusJewelList, {
            -- `node.nodesInRadius` is a table keyed by radiusIndex → {nodeId→node}.
            -- When nil (no radius data), falls back to empty table {}.
            -- In Rust: `node.nodes_in_radius.get(radius_index).cloned().unwrap_or_default()`
            nodes      = node.nodesInRadius and node.nodesInRadius[item.jewelRadiusIndex] or { },
            func       = func.func,
            type       = func.type,
            item       = item,
            nodeId     = slot.nodeId,
            attributes = node.attributesInRadius and node.attributesInRadius[item.jewelRadiusIndex] or { },
            data       = { }
        })

        -- Non-Self jewels need to process unallocated nodes in their radius too.
        -- These are added to extraRadiusNodeList (processed by buildModListForNodeList
        -- when finishJewels=true, but their mods DON'T go into the player modDB).
        if func.type ~= "Self" and node.nodesInRadius then
            for nodeId, node in pairs(node.nodesInRadius[item.jewelRadiusIndex]) do
                -- `not env.allocNodes[nodeId]` — add only UNallocated nodes.
                -- Allocated nodes are already in nodeList passed to buildModListForNodeList.
                if not env.allocNodes[nodeId] then
                    env.extraRadiusNodeList[nodeId] = env.spec.nodes[nodeId]
                end
            end
        end
    end

    -- ExtraJewelFunc: mods parsed from items like Mjolner can inject additional
    -- jewel functions dynamically. These are handled by the re-entry loop below.
    for _, funcData in ipairs(override and override.extraJewelFuncs and
                              override.extraJewelFuncs:List({item = item}, "ExtraJewelFunc") or {}) do
        local node = env.spec.nodes[slot.nodeId]
        local radius
        -- Resolve label string (e.g. "Medium") to a radius index integer.
        for index, data in pairs(data.jewelRadius) do
            if funcData.radius == data.label then
                radius = index
                break
            end
        end
        t_insert(env.radiusJewelList, {
            nodes      = node.nodesInRadius and node.nodesInRadius[radius] or { },
            func       = funcData.func,
            type       = funcData.type,
            item       = item,
            nodeId     = slot.nodeId,
            attributes = node.attributesInRadius and node.attributesInRadius[radius] or { },
            data       = { }
        })
        if funcData.type ~= "Self" and node.nodesInRadius then
            for nodeId, node in pairs(node.nodesInRadius[radius]) do
                if not env.allocNodes[nodeId] then
                    env.extraRadiusNodeList[nodeId] = env.spec.nodes[nodeId]
                end
            end
        end
    end
end
```

### ExtraJewelFunc re-entry loop — lines 1266–1275

```lua
-- After the second buildModListForNodeList call (line 1262), check if any
-- ExtraJewelFunc mods were registered by items. If so, re-run initEnv from scratch
-- with those funcs included. This handles items like Mjolner that inject radius
-- functions dynamically based on socketed gems.
if not override or (override and not override.extraJewelFuncs) then
    override = override or {}
    override.extraJewelFuncs = new("ModList")
    override.extraJewelFuncs.actor = env.player
    for _, mod in ipairs(env.modDB:Tabulate("LIST", nil, "ExtraJewelFunc")) do
        override.extraJewelFuncs:AddMod(mod.mod)
    end
    -- Only recurse if there are actually ExtraJewelFunc mods to process.
    -- `#override.extraJewelFuncs` is the length of the modList (number of mods).
    if #override.extraJewelFuncs > 0 then
        return calcs.initEnv(build, mode, override, specEnv)
        -- This RETURNS the result of the recursive call — the caller gets the
        -- re-initialised env. No data from this call is preserved.
    end
end
```

## Key Lua Patterns and Rust Equivalents

| Lua | Rust Equivalent | Notes |
|-----|-----------------|-------|
| `env.radiusJewelList` | `env.radius_jewel_list: Vec<RadiusJewelEntry>` | List of jewel descriptors |
| `rad.nodes[node.id]` | `rad.nodes.contains_key(&node.id)` | Check if node in radius |
| `wipeTable(rad.data)` | `rad.data.clear()` | Reset accumulator per pass |
| `rad.func(node, modList, rad.data)` | `(rad.func)(node, &mut mod_list, &mut rad.data)` | Closure/fn pointer call |
| `rad.func(nil, modList, rad.data)` | `(rad.func)(None, &mut mod_list, &mut rad.data)` | Finalise call |
| `calcLib.mod(modList, nil, "PassiveSkillEffect")` | `calc_mod(&mod_list, None, "PassiveSkillEffect")` | Effect scale factor |
| `scaledList:ScaleAddList(modList, scale)` | `for m in mod_list { scaled.push(m.scaled(scale)); }` | Multiply BASE values |
| `modList = scaledList` | Shadow via rebind | Lua rebinds local; old table is GC'd |
| `wipeTable(modList)` | `mod_list.clear()` | In-place empty |
| `modList:Flag(nil, "X")` | `mod_list.flag(None, "X")` | Boolean mod check |
| `modList:List(skillCfg, "NodeModifier")` | `mod_list.list(skill_cfg, "NodeModifier")` | Vec of list-valued mods |
| `node.nodesInRadius and node.nodesInRadius[idx] or {}` | `.get(idx).cloned().unwrap_or_default()` | Optional radius map |
| `"Tree:"..rad.nodeId` | `format!("Tree:{}", rad.node_id)` | ModSource tag string |
| `#override.extraJewelFuncs > 0` | `!extra_jewel_funcs.is_empty()` | Non-empty check |

### `calcLib.mod` expanded

```lua
-- calcLib.mod(modList, cfg, name) expands to:
-- (1 + modList:Sum("INC", cfg, name) / 100) * modList:More(cfg, name)
-- Returns 1.0 when no mods modify the named stat.
```

In Rust this is `calc_mod(mod_list, cfg, "PassiveSkillEffect")` in `calc_tools.rs`.

## Existing Rust Code

**File:** `crates/pob-calc/src/calc/setup.rs`, lines 317–415 (passive mods) and 70–74 (init call)  
**File:** `crates/pob-calc/src/passive_tree/mod.rs`, full file

### What exists

- `add_passive_mods` (setup.rs:317–338) iterates `build.passive_spec.allocated_nodes`,
  looks each node up in the tree, parses all `node.stats` strings, and adds the resulting
  mods to the player ModDb. This is a **flat** port — it applies mods directly without
  any radius jewel processing.

- `connected_passive_nodes` (setup.rs:340–415) performs BFS to prune orphaned passive
  nodes. Correct behavior.

- `PassiveTree` and `PassiveNode` (passive_tree/mod.rs) have node types, stats, linked
  IDs, and `ExpansionJewelMeta` for cluster jewels. The tree has no `nodesInRadius`
  field — PoB computes this in `PassiveSpec.lua` via BFS from each jewel socket node.

- `mod_parser_generated.rs` lines 32710–32900: `JewelData` LIST mods for radius jewels
  are parsed into `Mod { name: "JewelData", value: ModValue::Number(0.0) /* TODO */ }`.
  The radius index value and special flags (`intuitiveLeapLike`, `intuitiveLeapKeystoneOnly`,
  `ImpossibleEscapeKeystones`) are lost — all stubbed as `Number(0.0)`.

- `mod_parser_generated.rs` line 32724: `ImpossibleEscapeKeystones` mod exists as a stub
  with `Number(0.0)` and a TODO for `{ key = name, value = true }`.

### What's missing

1. **`RadiusJewelEntry` struct** — No struct or Vec for radius jewel descriptors exists.
   The `env.radius_jewel_list` concept is absent from `CalcEnv`.

2. **`env.extra_radius_node_list`** — No accumulator for unallocated nodes that need to
   be processed by non-Self jewels.

3. **`nodesInRadius` on PassiveNode** — `PassiveTree`/`PassiveNode` have no field for
   pre-computed radius node sets. PoB builds this via BFS in `PassiveSpec.lua`
   (lines 1038+). Either the tree data JSON needs to include this, or Rust must compute
   it at build time from node positions.

4. **`jewelRadiusIndex` on items** — The `JewelData` mod with `key="radiusIndex"` is
   stubbed. The item struct has no `jewel_radius_index: Option<u32>` field derived from
   this mod.

5. **`buildModListForNode` equivalent** — `add_passive_mods` does a flat stat parse with
   no two-pass radius jewel logic, no effect scaling, no suppression checks.

6. **`PassiveSkillEffect` scaling** — Not applied anywhere in Rust.

7. **`PassiveSkillHasNoEffect` suppression** — Not checked; nodes always contribute mods.

8. **`AllocatedPassiveSkillHasNoEffect` suppression** — Not checked.

9. **`PassiveSkillHasOtherEffect` replacement** — Not implemented.

10. **`funcList` / jewel callback system** — No Rust equivalent. The per-jewel callback
    functions that implement Thread of Hope, Intuitive Leap, etc. are defined in PoB's
    item data and would need to be ported to Rust closures / match arms keyed by jewel name.

11. **`ExtraJewelFunc` re-entry loop** — Not implemented (depends on #10).

12. **`node.grantedSkills` per node** — `buildModListForNode` populates `node.grantedSkills`
    from `ExtraSkill` LIST mods. `add_passive_mods` does not collect these.

### What's wrong

- `add_passive_mods` applies node stats without checking `PassiveSkillHasNoEffect`,
  meaning nodes that should be suppressed (e.g. those replaced by timeless jewels) still
  contribute their raw stats. **This causes incorrect values in every chunk for builds
  with timeless jewels.**

- The `JewelData` mod stubs (radiusIndex, intuitiveLeapLike, etc.) drop information
  silently — no warning or assertion. Any jewel-related code added in the future will
  silently get `0.0` for the radius index, causing bugs.

## What Needs to Change

1. **Add `nodes_in_radius` to `PassiveNode`** (passive_tree/mod.rs)  
   Add `pub nodes_in_radius: HashMap<u32, HashSet<u32>>` — keyed by radius index (1=Tiny,
   2=Small, 3=Medium, 4=Large, 5=Massive; jewel-specific indices 6-10 for Thread of Hope
   variants etc.), value is set of node IDs within that radius.  
   This must be computed via BFS/Euclidean-distance from each jewel socket node after
   the tree is loaded. PoB stores `(x, y)` coords per node; the Rust tree data JSON
   must include these for BFS to work.

2. **Port `JewelData` mod values** (mod_parser_generated.rs, lines 32710–32900)  
   Replace `ModValue::Number(0.0) /* TODO */` stubs with a real `ModValue::JewelData`
   variant or a structured value. At minimum, radius index and special boolean flags
   (`intuitiveLeapLike`, `intuitiveLeapKeystoneOnly`) must be preserved so the item
   struct can extract them.

3. **Add `jewel_radius_index: Option<u32>` to item types** (build/types.rs)  
   Extracted from the `JewelData` LIST mod with `key="radiusIndex"`.

4. **Add `RadiusJewelEntry` struct and `radius_jewel_list` to `CalcEnv`**  
   ```rust
   pub struct RadiusJewelEntry {
       pub nodes: HashMap<u32, PassiveNodeRef>, // nodes in radius
       pub func: RadiusJewelFn,                 // per-node callback
       pub jewel_type: RadiusJewelType,         // Self/SelfUnalloc/Threshold/Other
       pub item_name: String,
       pub node_id: u32,                        // socket node id
       pub data: HashMap<String, f64>,          // mutable accumulator
   }
   ```

5. **Add `extra_radius_node_list: HashMap<u32, ...>` to `CalcEnv`**  
   Populated during radius jewel list construction for non-Self jewels.

6. **Replace `add_passive_mods` with `build_mod_list_for_node_list`** (setup.rs)  
   Port the full two-pass logic: first-pass Other jewels → effect scaling →
   suppression check → second-pass Threshold/Self/SelfUnalloc → PassiveSkillHasOtherEffect.

7. **Port per-jewel callback functions**  
   Each unique jewel type has its own `funcList` in PoB's item data. The Rust equivalents
   need to be match arms or registered callbacks:
   - **Thread of Hope** (`type="SelfUnalloc"`): Removes the path-connectivity requirement
     for unallocated nodes within radius, allowing them to be allocated without pathing.
   - **Intuitive Leap** (`type="Self"`, `intuitiveLeapLike=true`): Allows allocating nodes
     in radius without connection. The allocated nodes' mods DO apply.
   - **Impossible Escape** (`type="Self"`): Grants a specific keystone without tree connection.
   - **Lethal Pride / Brutal Restraint / etc.** (`type="Other"`): Replace node stats
     with seed-derived stat tables (handled by SETUP-06 Timeless Jewels).
   - **Default tally** (`type="Self"`): Counts Str/Dex/Int in radius for display only.

8. **Implement `ExtraJewelFunc` re-entry loop** (setup.rs:init_env)  
   After the second pass, check `mod_db` for `ExtraJewelFunc` LIST mods; if present,
   re-run `init_env` with those registered. Required for items like Mjolner.

9. **Collect `grantedSkills` from ExtraSkill mods per node**  
   In `build_mod_list_for_node`, after processing, query the node's modList for
   `ExtraSkill` LIST mods and populate `node.granted_skills`. Feed these into
   `env.granted_skills_nodes` for downstream skill construction.
