# SETUP-05-cluster-jewels: Cluster Jewel Subgraph Generation

## Output Fields

This chunk produces **no direct output fields**. Its correctness is verified
indirectly: once SETUP-05 is implemented, builds that use cluster jewels should
pass `PERF-01-attributes` (and all downstream chunks) for those builds.

The two oracle builds known to fail downstream chunks because of missing cluster
jewel support are:

- `realworld_cluster_jewel` (Elementalist, Large Cluster Jewel with Cold Damage
  small passives and 3 notables: Blanketed Snow, Prismatic Heart, Widespread
  Destruction)
- `realworld_coc_trigger` (also contains cluster jewels)

Without this chunk, the synthetic passive nodes generated from cluster jewels are
never added to the ModDb, so any attribute bonus, damage mod, or resist they
grant is silently dropped.

## Dependencies

- **SETUP-01**: Item mod parsing must be complete so cluster jewel implicit/explicit
  lines are available in `build.items`.
- **SETUP-04**: `eval_mod` stubs must be replaced before the synthetic node mods
  are queried in Tier 1+ chunks.

## Lua Source

Primary file: `third-party/PathOfBuilding/src/Classes/PassiveSpec.lua`
Commit: `454eff8c85d24356d9b051d596983745ed367476`

Supporting data file: `third-party/PathOfBuilding/src/Data/ClusterJewels.lua`

Key function: `PassiveSpecClass:BuildClusterJewelGraphs()` — lines 1576–1638
Key function: `PassiveSpecClass:BuildSubgraph(jewel, parentSocket, id, upSize, importedNodes, importedGroups)` — lines 1641–2061

Item-side jewel data assembly: `third-party/PathOfBuilding/src/Classes/Item.lua`
Key lines: 1631–1658 (builds `jewelData.clusterJewelNotables`, `clusterJewelSkill`,
`clusterJewelNodeCount`, `clusterJewelSocketCount`, `clusterJewelValid`, etc.)

Mod parsing for cluster jewel lines: `third-party/PathOfBuilding/src/Modules/ModParser.lua`
Key lines: 6346–6357 (dynamic table of `ClusterJewelNotable` and `JewelData` mods)

## Annotated Lua

### Step 1 — XML parsing: how jewels reach tree sockets

In PoB's XML, cluster jewels are connected to tree nodes via a `<Sockets>` child
element inside `<Spec>`:

```xml
<Spec treeVersion="3_13" nodes="..." classId="3" ascendClassId="2">
  <Sockets>
    <Socket nodeId="12345" itemId="11"/>
  </Sockets>
</Spec>
```

- `nodeId` is the passive-tree node ID of the large jewel socket
- `itemId` is the `id` attribute of the `<Item>` element carrying the cluster jewel

PoB reads this in `PassiveSpec:Load()` (lines 101–117):

```lua
elseif node.elem == "Sockets" then
    for _, child in ipairs(node) do
        if child.elem == "Socket" then
            -- ...validation omitted...
            jewelIdNum = tonumber(child.attrib.itemId)
            if jewelIdNum > 0 then
                self.jewels[tonumber(child.attrib.nodeId)] = jewelIdNum
                -- ↑ maps nodeId → itemId in the passive spec's jewel table
            end
        end
    end
end
```

**Rust gotcha**: `xml_parser.rs` currently handles `<Spec>` but does NOT parse the
inner `<Sockets>/<Socket>` children. The `PassiveSpec` struct has no `jewels` map
field at all. This is the first missing piece.

**Rust equivalent needed**:
```rust
// In PassiveSpec:
pub jewels: HashMap<u32, u32>,  // tree_node_id → item_id

// In xml_parser.rs, inside the Spec element handler:
"Socket" => {
    if let (Some(node_id), Some(item_id)) = (
        attrs.get("nodeId").and_then(|v| v.parse::<u32>().ok()),
        attrs.get("itemId").and_then(|v| v.parse::<u32>().ok()),
    ) {
        if item_id > 0 {
            passive_spec.jewels.insert(node_id, item_id);
        }
    }
}
```

---

### Step 2 — Item-side: parsing cluster jewel mod lines

When PoB processes a cluster jewel item (`item.clusterJewel` is set, detected by
matching `item.baseName` against `data.clusterJewels.jewels`), it extracts these
`jewelData` fields from the item's mod list:

| `jewelData` key | Source mod type | Example item line |
|-----------------|-----------------|-------------------|
| `clusterJewelSkill` | `JewelData` LIST `{key="clusterJewelSkill", value=skillId}` | `"Added Small Passive Skills grant: 12% increased Cold Damage"` |
| `clusterJewelNodeCount` | `JewelData` LIST `{key="clusterJewelNodeCount", value=N}` | `"{crafted}Adds 8 Passive Skills"` |
| `clusterJewelSocketCount` | `JewelData` LIST `{key="clusterJewelSocketCount", value=N}` | `"{crafted}2 Added Passive Skills are Jewel Sockets"` |
| `clusterJewelNotables` | `ClusterJewelNotable` LIST `notableName` | `"1 Added Passive Skill is Blanketed Snow"` |
| `clusterJewelAddedMods` | `AddToClusterJewelNode` LIST `line` | (rare, from anoint/enchant) |
| `clusterJewelIncEffect` | `JewelData` LIST `{key="clusterJewelIncEffect", value=N}` | `"Added Small Passive Skills have N% increased Effect"` |
| `clusterJewelSocketCountOverride` | `JewelData` LIST `{key="clusterJewelSocketCountOverride", value=N}` | (special jewels) |
| `clusterJewelNothingnessCount` | `JewelData` LIST `{key="clusterJewelNothingnessCount", value=N}` | (Nothingness fills) |
| `clusterJewelSmallsAreNothingness` | `JewelData` LIST `{key="clusterJewelSmallsAreNothingness", value=true}` | (special magic jewels) |
| `clusterJewelKeystone` | `JewelData` LIST `{key="clusterJewelKeystone", value=ks}` | `"Adds {Keystone}"` |

The mod-to-`jewelData` extraction in `Item.lua` (lines 1622–1624):

```lua
for _, value in ipairs(modList:List(nil, "JewelData")) do
    jewelData[value.key] = value.value
    -- Lua table dynamic assignment: sets the field named by value.key
    -- Rust: this is a heterogeneous map; needs typed variants (see below)
end
```

**Lua gotcha**: `jewelData[value.key] = value.value` is **dynamic table
assignment** — the key name is a string at runtime. Rust cannot replicate this
directly; it needs an explicit struct or enum for `JewelData` payloads.

`ClusterJewelNotable` mods accumulate into a `Vec<String>` (lines 1632–1635):

```lua
jewelData.clusterJewelNotables = { }
for _, name in ipairs(modList:List(nil, "ClusterJewelNotable")) do
    t_insert(jewelData.clusterJewelNotables, name)
    -- t_insert(list, val) → list.push(val)
end
```

Validation and clamping (lines 1649–1658):

```lua
-- 1. Clamp nodeCount to [minNodes, maxNodes] for the jewel size
if jewelData.clusterJewelNodeCount then
    jewelData.clusterJewelNodeCount = m_min(m_max(
        jewelData.clusterJewelNodeCount,
        self.clusterJewel.minNodes),    -- e.g. 8 for Large
        self.clusterJewel.maxNodes)     -- e.g. 12 for Large
    -- Rust: node_count.clamp(jewel_def.min_nodes, jewel_def.max_nodes)
end

-- 2. Validate clusterJewelSkill is a known skill for this size
if jewelData.clusterJewelSkill and
   not self.clusterJewel.skills[jewelData.clusterJewelSkill] then
    jewelData.clusterJewelSkill = nil
    -- Rust: if !jewel_def.skills.contains_key(&skill_id) { skill_id = None; }
end

-- 3. Small/Medium curse cluster skill correction (size mismatch fix)
if self.clusterJewel.size == "Small" and
   jewelData.clusterJewelSkill == "affliction_curse_effect" then
    jewelData.clusterJewelSkill = "affliction_curse_effect_small"
elseif self.clusterJewel.size == "Medium" and
   jewelData.clusterJewelSkill == "affliction_curse_effect_small" then
    jewelData.clusterJewelSkill = "affliction_curse_effect"
end

-- 4. Validity check: jewel is only "valid" (subgraph buildable) if:
jewelData.clusterJewelValid = jewelData.clusterJewelKeystone
    or ((jewelData.clusterJewelSkill or jewelData.clusterJewelSmallsAreNothingness)
        and jewelData.clusterJewelNodeCount)
    or (jewelData.clusterJewelSocketCountOverride
        and jewelData.clusterJewelNothingnessCount)
-- Rust: bool flag computed from the above conditions
```

**Rust gotcha — `or` with tables**: In Lua, `x or y` where `x` is a table (not
nil/false) returns `x`. Here `clusterJewelSkill or clusterJewelSmallsAreNothingness`
returns the skill string if set, otherwise the bool. In Rust, model this as
`skill_id.is_some() || smalls_are_nothingness`.

**Rust gotcha — `mod_parser_generated.rs` TODOs**: Rules 1632–1638 in the
generated parser emit `ModValue::Number(0.0) /* TODO: ... */`. They recognise the
line patterns but produce unusable values. Each must be changed to emit a
properly typed `JewelData` payload. A new enum variant is needed:

```rust
// Proposed:
pub enum ModValue {
    Number(f64),
    Bool(bool),
    Str(String),
    JewelData(JewelDataPayload),   // new variant
    // ...
}

pub enum JewelDataPayload {
    ClusterJewelNodeCount(u32),
    ClusterJewelSocketCount(u32),
    ClusterJewelSocketCountOverride(u32),
    ClusterJewelNothingnessCount(u32),
    ClusterJewelSmallsAreNothingness,
    ClusterJewelIncEffect(f64),
    ClusterJewelSkill(String),
    ClusterJewelKeystone(String),
    JewelIncEffectFromClassStart(f64),
}
```

The `ClusterJewelNotable` mod type also needs a `ModType::List` variant with a
`String` value for the notable name.

---

### Step 3 — Data side: ClusterJewels static data

`Data/ClusterJewels.lua` defines three jewel sizes. Key fields per size entry:

```lua
["Large Cluster Jewel"] = {
    size = "Large",
    sizeIndex = 2,          -- used to compute subgraph node IDs (bit-shifted)
    minNodes = 8,           -- minimum passives allowed (clamp lower bound)
    maxNodes = 12,          -- maximum passives allowed (clamp upper bound)
    smallIndicies = { 0, 4, 6, 8, 10, 2, 7, 5, 9, 3, 11, 1 },
    -- ^ preferred order for placing Small fill nodes (0-based orbit indices)
    notableIndicies = { 6, 4, 8, 10, 2 },
    -- ^ preferred order for placing Notable nodes
    socketIndicies = { 4, 8, 6 },
    -- ^ preferred order for placing Socket (sub-jewel) nodes
    totalIndicies = 12,     -- the orbit ring size (12 for Large/Medium, 6 for Small)
    skills = { ... },       -- keyed by skill tag ID
}
```

```lua
["Medium Cluster Jewel"] = { sizeIndex = 1, minNodes = 4, maxNodes = 6, totalIndicies = 12, ... }
["Small Cluster Jewel"]  = { sizeIndex = 0, minNodes = 2, maxNodes = 3, totalIndicies = 6,  ... }
```

`notableSortOrder` is a flat map from notable display name → integer sort priority,
used to order notables around the ring when multiple notables are on one jewel.

`clusterNodeMap` is built in `PassiveTree.lua` from tree node data —
each tree node with a `clusterPassive` flag gets added to `tree.clusterNodeMap[name]`.
It maps notable/keystone names to their template node objects (which have `sd`,
`dn`, `icon` etc.). This data lives in the passive tree JSON.

**Rust equivalent needed**: a `ClusterJewelData` struct loaded from a JSON/Lua
equivalent that mirrors the three jewel size entries, `notableSortOrder`, and a
`clusterNodeMap` looked up from the passive tree data.

---

### Step 4 — BuildClusterJewelGraphs: the outer loop

```lua
-- PassiveSpec.lua:1576
function PassiveSpecClass:BuildClusterJewelGraphs()
    -- (A) Tear down old subgraphs (orphaning previously-generated nodes)
    for id, subGraph in pairs(self.subGraphs) do
        for _, node in ipairs(subGraph.nodes) do
            if node.id then
                self.nodes[node.id] = nil          -- remove from live node table
                -- if was allocated: reserve for re-allocation after rebuild
                if self.allocNodes[node.id] then
                    self.allocNodes[node.id] = nil
                    if not self.ignoreAllocatingSubgraph then
                        t_insert(self.allocSubgraphNodes, node.id)
                    end
                end
            end
        end
        -- Disconnect entrance from parent socket
        local index = isValueInArray(subGraph.parentSocket.linked, subGraph.entranceNode)
        t_remove(subGraph.parentSocket.linked, index)
    end
    wipeTable(self.subGraphs)   -- clear the subgraph registry

    -- (B) Pre-populate from imported PoB account data (jewel_data from URL import)
    local importedGroups = { }
    local importedNodes = { }
    if self.jewel_data then
        for _, value in pairs(self.jewel_data) do
            if value.subgraph then
                for groupId, groupData in pairs(value.subgraph.groups) do
                    importedGroups[groupId] = groupData
                end
                for nodeId, nodeValue in pairs(value.subgraph.nodes) do
                    importedNodes[nodeId] = nodeValue
                end
            end
        end
    end

    -- (C) For each tree socket node, check if a valid cluster jewel is socketed
    for nodeId in pairs(self.tree.sockets) do
        -- self.tree.sockets: Set<nodeId> of all jewel socket nodes in the tree
        local node = self.tree.nodes[nodeId]
        local jewel = self:GetJewel(self.jewels[nodeId])
        -- self.jewels[nodeId]: itemId for the cluster jewel in this socket
        -- GetJewel(itemId): returns the item if it's a jewel with valid jewelData

        if node
            and node.expansionJewel           -- this is a "Large" tree socket (size 2)
            and node.expansionJewel.size == 2 -- size 2 = Large; size 1 = Medium; size 0 = Small
            and jewel
            and jewel.jewelData.clusterJewelValid  -- jewel is fully configured
        then
            self:BuildSubgraph(jewel, self.nodes[nodeId], nil, nil,
                               importedNodes, importedGroups)
        end
    end

    -- (D) Re-allocate nodes that were previously allocated before the rebuild
    for _, nodeId in ipairs(self.allocSubgraphNodes) do
        local node = self.nodes[nodeId]
        if node then
            node.alloc = true
            if not self.allocNodes[nodeId] then
                self.allocNodes[nodeId] = node
                t_insert(self.allocExtendedNodes, nodeId)
            end
        end
    end
    wipeTable(self.allocSubgraphNodes)

    -- (E) Rebuild path/dependency graph to account for new nodes
    self:BuildAllDependsAndPaths()
end
```

**Rust scope**: The Rust equivalent does not need to replicate the full
visual/interactive subgraph (connectors, groups for rendering). It only needs to
produce the set of synthetic `PassiveNode`s with their `stats` filled in, then
add those nodes' mods to the player `ModDb` — exactly what `add_passive_mods`
already does for static tree nodes.

Steps B (`importedGroups`/`importedNodes` from account import) can be skipped
initially — oracle builds use only XML-defined jewels, not URL-imported data.

---

### Step 5 — BuildSubgraph: generating synthetic nodes

```lua
-- PassiveSpec.lua:1641
function PassiveSpecClass:BuildSubgraph(jewel, parentSocket, id, upSize, importedNodes, importedGroups)
    local clusterJewel = jewel.clusterJewel  -- the ClusterJewels.lua entry for this size
    local jewelData    = jewel.jewelData     -- the parsed jewelData table from Item.lua

    -- (1) Compute base ID for this subgraph
    -- Encoding:
    --   bits  0-3:  node index within ring (0-11)
    --   bits  4-5:  group size (sizeIndex: 0=Small 1=Medium 2=Large)
    --   bits  6-8:  large slot index (0-5)
    --   bits  9-10: medium slot index (0-2)
    --   bit  16:    signal bit (avoids collision with real node IDs)
    id = id or 0x10000
    if expansionJewel.size == 2 then           -- large parent socket
        id = id + b_lshift(expansionJewel.index, 6)
    elseif expansionJewel.size == 1 then       -- medium parent socket
        id = id + b_lshift(expansionJewel.index, 9)
    end
    local nodeId = id + b_lshift(clusterJewel.sizeIndex, 4)
    -- ^ nodeId is the base ID; individual nodes get nodeId + oidx

    -- (2) Handle keystone cluster jewels (special case — only one node, no ring)
    if jewelData.clusterJewelKeystone then
        local keystoneNode = self.tree.clusterNodeMap[jewelData.clusterJewelKeystone]
        -- Build a single Keystone node and link it to parentSocket
        -- ... (see lines 1741-1766)
        return
    end

    -- (3) Determine effective group size (downsize if jewel is smaller than socket)
    local groupSize = expansionJewel.size
    upSize = upSize or 0
    while clusterJewel.sizeIndex < groupSize do
        -- Walk proxy nodes until we find a socket of the right size
        local socket = findSocket(proxyGroup, 1) or findSocket(proxyGroup, 0)
        proxyNode = self.tree.nodes[tonumber(socket.expansionJewel.proxy)]
        proxyGroup = proxyNode.group
        groupSize = socket.expansionJewel.size
        upSize = upSize + 1
    end
    -- Rust gotcha: upSize only matters for rendering; skip for parity purposes.

    -- (4) Resolve the "skill" (small passive type)
    local skill = clusterJewel.skills[jewelData.clusterJewelSkill] or {
        name = "Nothingness",
        icon = "...",
        stats = { },   -- empty stat list = no mods from small nodes
    }
    -- Rust: if skill_id is None, use empty stats for small fills.

    -- (5) Count nodes by role
    local socketCount  = jewelData.clusterJewelSocketCountOverride
                      or jewelData.clusterJewelSocketCount or 0
    local notableCount = #notableList   -- sorted list of selected notables
    local nodeCount    = jewelData.clusterJewelNodeCount
                      or (socketCount + notableCount
                          + (jewelData.clusterJewelNothingnessCount or 0))
    -- ↑ clamp was already applied in Item.lua, so this is already in [min,max]
    local smallCount   = nodeCount - socketCount - notableCount

    -- (6) First pass: place Socket nodes (sub-jewel sockets)
    if clusterJewel.size == "Large" and socketCount == 1 then
        makeJewel(6, 1)  -- Large single socket always at index 6
    else
        -- socketIndicies = { 4, 8, 6 } for Large; { 6 } for Medium; { 4 } for Small
        local getJewels = { 0, 2, 1 }  -- which physical socket position to use
        for i = 1, socketCount do
            makeJewel(clusterJewel.socketIndicies[i], getJewels[i])
            -- Lua 1-based: socketIndicies[1] = first socket index
            -- Rust: socket_indicies[i - 1]
        end
    end

    -- (7) Second pass: place Notable nodes
    local notableIndexList = { }
    for _, nodeIndex in ipairs(clusterJewel.notableIndicies) do
        if #notableIndexList == notableCount then break end
        -- Medium cluster special rules (override indices for specific counts):
        if clusterJewel.size == "Medium" then
            if socketCount == 0 and notableCount == 2 then
                if nodeIndex == 6 then nodeIndex = 4
                elseif nodeIndex == 10 then nodeIndex = 8 end
            elseif nodeCount == 4 then
                if nodeIndex == 10 then nodeIndex = 9
                elseif nodeIndex == 2 then nodeIndex = 3 end
            end
        end
        if not indicies[nodeIndex] then
            t_insert(notableIndexList, nodeIndex)
        end
    end
    table.sort(notableIndexList)  -- sort ascending so ring order is consistent

    for index, baseNode in ipairs(notableList) do
        local nodeIndex = notableIndexList[index]
        if not nodeIndex then break end  -- silently handle too many notables
        local node = {
            type = "Notable",
            id   = nodeId + nodeIndex,   -- unique ID for this specific notable node
            dn   = baseNode.dn,          -- display name
            sd   = baseNode.sd,          -- stat descriptions → will be parsed to mods
            icon = baseNode.icon,
            expansionSkill = true,       -- marks this as a generated node
            ...
        }
        -- Rust: create PassiveNode with id = base_id + node_index, stats = base_node.stats
    end

    -- (8) Third pass: place Small fill nodes
    local smallIndexList = { }
    for _, nodeIndex in ipairs(clusterJewel.smallIndicies) do
        if #smallIndexList == smallCount then break end
        -- Medium cluster small-index special rules:
        if clusterJewel.size == "Medium" then
            if nodeCount == 5 and nodeIndex == 4 then nodeIndex = 3
            elseif nodeCount == 4 then
                if nodeIndex == 8 then nodeIndex = 9
                elseif nodeIndex == 4 then nodeIndex = 3 end
            end
        end
        if not indicies[nodeIndex] then
            t_insert(smallIndexList, nodeIndex)
        end
    end

    for index = 1, smallCount do
        local nodeIndex = smallIndexList[index]
        if not nodeIndex then break end
        local node = {
            type = "Normal",
            id   = nodeId + nodeIndex,
            dn   = skill.name,
            sd   = copyTable(skill.stats),   -- copy: each node gets its own stat list
            ...
        }
        -- Apply any added mods from the jewel enchant/anoint
        for _, line in ipairs(jewelData.clusterJewelAddedMods) do
            t_insert(node.sd, line)
        end
        -- Rust: stats = skill.stats.clone() + jewel_data.added_mods
    end

    -- (9) Apply clusterJewelIncEffect to Small nodes
    -- (lines 2015–2022)
    for _, node in ipairs(subGraph.nodes) do
        self.tree:ProcessNode(node)  -- parses node.sd into node.modList
        if node.modList and jewelData.clusterJewelIncEffect and node.type == "Normal" then
            node.modList:NewMod("PassiveSkillEffect", "INC", jewelData.clusterJewelIncEffect)
            -- ^ adds "X% increased effect of this passive skill" to small nodes
            -- Rust: after parsing small node stats, add an INC mod to PassiveSkillEffect
        end
    end

    -- (10) Recurse into Socket nodes (Medium and Small sub-jewels)
    -- (lines 2046–2058)
    for _, node in ipairs(subGraph.nodes) do
        self.nodes[node.id] = node   -- register in live node table
        if node.type == "Socket" then
            local jewel = self:GetJewel(self.jewels[node.id])
            if jewel and jewel.jewelData.clusterJewelValid then
                self:BuildSubgraph(jewel, node, id, upSize, importedNodes, importedGroups)
                -- ^ recurse: Medium sockets can hold Medium or Small cluster jewels
            end
        end
    end
end
```

**Rust gotcha — `copyTable(skill.stats)`**: Lua's `copyTable` does a shallow
copy. Each small node must have its own `Vec<String>` for stats so that
`clusterJewelAddedMods` lines can be appended without affecting other nodes.
In Rust: `skill.stats.clone()`.

**Rust gotcha — oidx translation (lines 1991–2013)**: PoB normalises node
`oidx` values from cluster-jewel-relative coordinates (6 or 12 positions) to the
tree's `skillsPerOrbit` coordinates (which may be 12 or 16 for certain orbits).
This translation only affects rendering and position. **For parity purposes, the
oidx values are not needed** — we only care about the stat strings on each node.
Skip this section.

**Rust gotcha — `b_lshift`**: This is `bit.lshift(x, n)` from LuaJIT's bit
library — a left bitshift. In Rust: `x << n`. The subgraph ID encoding is only
needed to ensure unique node IDs (to avoid collisions). Use the same encoding in
Rust.

---

### Step 6 — Allocation: which generated nodes are "active"

After `BuildSubgraph`, nodes must still be *allocated* (i.e., the character has
spent the passive point). In the full PoB flow, `allocSubgraphNodes` is populated
during the build import when the XML's `<Sockets>` entries intersect with the
allocated `nodes` list in `<Spec>`.

For the Rust implementation, the simplification is:

1. Any synthetic node created by `BuildSubgraph` whose ID appears in
   `build.passive_spec.allocated_nodes` should have its mods applied.
2. The sub-jewel (Socket node) IDs are the same as the physical tree socket node
   IDs — so if the tree socket node ID is in `allocated_nodes`, the Socket node
   is active and recursion should continue.

**Important**: The `allocSubgraphNodes` / `allocExtendedNodes` bookkeeping in Lua
exists for interactive re-allocation after user edits. In the Rust read-only
calculation path, simply use the original `allocated_nodes` set from the XML.

---

### ModParser note: ClusterJewelNotable / clusterJewelSkill parsing

PoB's ModParser builds dynamic lookup tables at startup:

```lua
-- ModParser.lua:6346-6357
local clusterJewelSkills = {}

-- (A) skill enchant lines → clusterJewelSkill
for baseName, jewel in pairs(data.clusterJewels.jewels) do
    for skillId, skill in pairs(jewel.skills) do
        clusterJewelSkills[table.concat(skill.enchant, " "):lower()]
            = { mod("JewelData", "LIST", { key = "clusterJewelSkill", value = skillId }) }
        -- e.g. "added small passive skills grant: 12% increased cold damage"
        --   → JewelData { key="clusterJewelSkill", value="affliction_cold_damage" }
    end
end

-- (B) notable names → ClusterJewelNotable
for notable in pairs(data.clusterJewels.notableSortOrder) do
    clusterJewelSkills["1 added passive skill is "..notable:lower()]
        = { mod("ClusterJewelNotable", "LIST", notable) }
        -- e.g. "1 added passive skill is blanketed snow"
        --   → ClusterJewelNotable { value = "Blanketed Snow" }
end

-- (C) keystone names → clusterJewelKeystone
for _, keystone in ipairs(data.clusterJewels.keystones) do
    clusterJewelSkills["adds "..keystone:lower()]
        = { mod("JewelData", "LIST", { key = "clusterJewelKeystone", value = keystone }) }
end
```

**Rust implication**: The generated mod parser (rules 1632–1638) already
recognises the numerical patterns (`Adds N Passive Skills`, `N Added Passive
Skills are Jewel Sockets`, etc.) but produces `ModValue::Number(0.0)` stub
values. The notable name parsing (`1 Added Passive Skill is {Name}`) and skill
enchant parsing (`Added Small Passive Skills grant: ...`) are **not represented in
`mod_parser_generated.rs` at all** — they need new rules in
`mod_parser_manual.rs` backed by a static table loaded from cluster jewel data.

---

## Existing Rust Code

### `crates/pob-calc/src/build/types.rs`

`PassiveSpec` struct (lines 21–26):
```rust
pub struct PassiveSpec {
    pub tree_version: String,
    pub allocated_nodes: HashSet<u32>,
    pub class_id: u32,
    pub ascend_class_id: u32,
}
```
**Missing**: `pub jewels: HashMap<u32, u32>` (tree_node_id → item_id).

`Item` struct has no `jewel_data` field of any kind. There is no `ClusterJewelData`
type anywhere in the codebase.

### `crates/pob-calc/src/build/xml_parser.rs`

The `Spec` handler (lines 140–162) reads `nodes`, `classId`, `ascendClassId` from
attributes. It does **not** parse the `<Sockets>/<Socket nodeId="..." itemId="..."/>`
child elements.

### `crates/pob-calc/src/calc/setup.rs`

`add_jewel_mods` (lines 1868–1906) iterates over `item_set.slots` for `is_jewel()`
slots and adds their mods. This handles regular jewels in numbered slots (`Jewel 1`
through `Jewel 21`). Cluster jewels are not slotted in `<Slot>` entries — they are
in `<Sockets>` within `<Spec>` — so **cluster jewels are completely ignored** by
this function.

`add_passive_mods` (lines 283–303) only looks up nodes in `data.passive_tree.nodes`
(the static tree JSON). Generated subgraph nodes never appear there, so their mods
are never added.

### `crates/pob-calc/src/passive_tree/mod.rs`

`PassiveTree` (lines 107–112) holds a static `HashMap<u32, PassiveNode>` loaded from
JSON. There is no mechanism to inject dynamically-generated nodes. No `clusterNodeMap`.

### `crates/pob-calc/src/build/mod_parser_generated.rs`

Rules 1632–1638 (lines 33204–33304) match cluster jewel numerical mod lines but all
produce `ModValue::Number(0.0)` stubs with TODO comments. No rule matches `"1 Added
Passive Skill is {Notable}"` or the skill enchant pattern (e.g. `"Added Small Passive
Skills grant: N% increased Cold Damage"`).

### `crates/pob-calc/src/data/`

No `ClusterJewels` data structure exists. `GameData` has `misc`, `gems`, `bases`,
`uniques` — no cluster jewel definitions, no `clusterNodeMap`.

## What Needs to Change

1. **Add `jewels: HashMap<u32, u32>` to `PassiveSpec`** in `types.rs`.
   This maps tree socket node ID → cluster jewel item ID (from `<Sockets>/<Socket>`
   elements in the XML `<Spec>`).

2. **Parse `<Sockets>/<Socket>` in `xml_parser.rs`**.
   Inside the `"Spec"` handler, detect child `<Sockets>` and `<Socket>` elements
   and populate `passive_spec.jewels`.

3. **Add `ClusterJewelData` to `GameData`** in `data/mod.rs` and load it.
   Port the structure of `Data/ClusterJewels.lua` to a Rust type. Needs:
   - Three entries: Small (sizeIndex=0), Medium (sizeIndex=1), Large (sizeIndex=2)
   - Per-entry: `min_nodes`, `max_nodes`, `small_indicies`, `notable_indicies`,
     `socket_indicies`, `total_indicies`
   - Per-entry `skills`: map from skill tag ID → `ClusterJewelSkill { stats: Vec<String> }`
   - `notable_sort_order`: map from notable name → integer (for ordering)
   - `cluster_node_map`: map from notable/keystone name → `Vec<String>` stat lines
     (can be derived from tree node data or embedded from a static JSON export)

4. **Fix `mod_parser_generated.rs` rules 1632–1638** to emit properly-typed
   `JewelData` payloads instead of `Number(0.0)` stubs. Requires a new
   `ModValue::JewelData(JewelDataPayload)` variant (or an alternative encoding
   using an existing variant).

5. **Add cluster notable / skill enchant parsing to `mod_parser_manual.rs`**.
   At startup, build a static lookup table from `ClusterJewelData`:
   - `"1 added passive skill is {notable_lower}"` → `ClusterJewelNotable(notable_name)`
   - `skill.enchant lines` → `JewelData { clusterJewelSkill: skill_tag_id }`
   These patterns are data-driven and cannot be expressed as simple regexes; they
   require a lookup table populated from `ClusterJewelData`.

6. **Add `ClusterJewelInfo` extraction to the item resolver** (or a new
   `resolve_cluster_jewel` step in `setup.rs`).
   For each item whose `base_type` matches a cluster jewel size name, extract
   `ClusterJewelInfo` from its parsed mods:
   - `skill_id: Option<String>`
   - `node_count: Option<u32>`
   - `socket_count: Option<u32>`
   - `notable_names: Vec<String>`
   - `added_mods: Vec<String>` (from `AddToClusterJewelNode`)
   - `inc_effect: Option<f64>`
   - `socket_count_override: Option<u32>`
   - `nothingness_count: Option<u32>`
   - `smalls_are_nothingness: bool`
   - `keystone: Option<String>`
   - `is_valid: bool` (computed per validation rules)
   Apply clamping of `node_count` to `[min_nodes, max_nodes]` and the
   curse-cluster size correction during this step.

7. **Add `build_cluster_subgraphs` to `setup.rs`**.
   Called after `add_jewel_mods`. For each `(node_id, item_id)` in
   `build.passive_spec.jewels`:
   - Look up the tree node; if it has `node_type == NodeType::JewelSocket` and
     `expansionJewel.size == 2` (Large), proceed.
   - Look up the item; if `is_valid`, call `build_subgraph(...)`.
   - Collect all generated synthetic `PassiveNode`s.
   - For each such node whose ID is in `build.passive_spec.allocated_nodes`,
     add its stat mods to the player `ModDb`.

8. **Add `ExpansionJewelMeta` to `PassiveNode`** in `passive_tree/mod.rs`.
   The tree JSON for Large jewel socket nodes carries an `expansionJewel` object
   with fields: `size` (0/1/2), `index` (position index), `proxy` (proxy node ID).
   This metadata is needed to compute subgraph IDs and locate proxy groups.
   Deserialise it from the tree JSON (key `"expansionJewel"` on tree node objects).

9. **Handle recursive sub-jewels** (Socket nodes within the subgraph can hold
   Medium or Small cluster jewels). After generating a Socket node in the subgraph,
   check if `passive_spec.jewels[socket_node_id]` points to another cluster jewel,
   and recurse with `build_subgraph(...)`. This covers nested Medium-in-Large and
   Small-in-Medium configurations.

10. **Verify with `PERF-01-attributes` chunk test** on the two affected oracle
    builds (`realworld_cluster_jewel`, `realworld_coc_trigger`). These are the
    minimal smoke tests that SETUP-05 unlocks.
