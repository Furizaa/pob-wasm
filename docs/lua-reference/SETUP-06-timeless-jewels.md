# SETUP-06: Timeless Jewel Node Replacement

## Output Fields

None. SETUP-06 is a pure setup chunk: it modifies passive-tree node definitions
in-place (replacing stats, name, mod list) before any output-writing calculation
runs. The downstream effect is that downstream chunks see corrected `stats` on
allocated passive nodes and therefore produce correct numbers. Parity is verified
by checking that `PERF-01-attributes` (and all later chunks) pass for
`realworld_timeless_jewel`.

## Dependencies

- **SETUP-05-cluster-jewels** — cluster subgraphs must be built first so that
  `self.nodes` is complete before `BuildAllDependsAndPaths()` assigns
  `conqueredBy` flags and replaces nodes.

## Lua Source

**Primary file:** `src/Classes/PassiveSpec.lua`, lines 1064–1303  
**Helper file:** `src/Classes/PassiveSpec.lua`, lines 1541–1574 (`ReplaceNode`,
`ReconnectNodeToClassStart`)  
**Support function:** `src/Classes/PassiveSpec.lua`, lines 2104–2143
(`NodeAdditionOrReplacementFromString`)  
**LUT helper:** `src/Modules/DataLegionLookUpTableHelper.lua`, lines 292–331
(`readLUT`)  
**Conqueror list:** `src/Modules/ModParser.lua`, lines 41–65 (`conquerorList`)  
**Data constants:** `src/Modules/Data.lua`, lines 797–825 (`timelessJewelTypes`,
`timelessJewelSeedMin`, `timelessJewelSeedMax`, `timelessJewelAdditions`,
`readLUT`)

**Submodule commit:** `454eff8c85d24356d9b051d596983745ed367476`

---

## Annotated Lua

### Context: When this runs

`BuildAllDependsAndPaths()` is called from several `PassiveSpec` methods
(e.g., after `SetClass`, after `AllocNode`, after XML import). In the CalcSetup
path it is called via `build.spec:BuildAllDependsAndPaths()`.  The function has
two separate passes through `self.nodes`:

**Pass 1 (lines 1070–1115):** Resets `conqueredBy` on every node, then walks
each jewel socket. If the jewel in that socket has `item.jewelData.conqueredBy`
set, propagates that value to every tree node within the jewel's radius.

**Pass 2 (lines 1117–1303):** Applies replacements: tattoo hash-overrides first,
then timeless-jewel `conqueredBy` replacements.

---

### Pass 1 — Assigning `conqueredBy` (lines 1070–1115)

```lua
-- PASS 1: For each non-ClassStart, non-Socket, non-ascendancy node,
-- check every jewel socket to see if this node is within the jewel's radius
-- and the jewel has a conqueredBy value.
for id, node in pairs(self.nodes) do          -- pairs() → unordered; Rust: HashMap iter
    node.depends = wipeTable(node.depends)
    node.intuitiveLeapLikesAffecting = { }
    node.conqueredBy = nil                     -- reset from previous call

    -- Reset node to the original tree node data (un-do any prior replacement)
    if self.tree.nodes[id] then               -- tree.nodes is the *original* static tree
        self:ReplaceNode(node, self.tree.nodes[id])
                                               -- ReplaceNode is cheap when sd is identical
    end

    if node.type ~= "ClassStart" and node.type ~= "Socket" and not node.ascendancyName then
        for nodeId, itemId in pairs(self.jewels) do   -- self.jewels: socket nodeId → itemId
            local item = self.build.itemsTab.items[itemId]
            if item
                and item.jewelRadiusIndex        -- jewel has a radius (= is a jewel with radius)
                and self.allocNodes[nodeId]      -- the jewel socket is actually allocated
                and item.jewelData               -- jewel has computed jewelData
                and not item.jewelData.limitDisabled  -- within unique limit
            then
                local radiusIndex = item.jewelRadiusIndex
                -- nodesInRadius is a precomputed { [radiusIndex] = { [nodeId] = true } }
                -- table on each jewel-socket node.
                if self.nodes[nodeId].nodesInRadius
                    and self.nodes[nodeId].nodesInRadius[radiusIndex][node.id]
                then
                    if itemId ~= 0 then
                        -- (intuitiveLeapLike handling omitted — belongs to SETUP-08)
                        if item.jewelData.conqueredBy then
                            node.conqueredBy = item.jewelData.conqueredBy
                            -- conqueredBy = { id = <seed>, conqueror = { id = <int|str>, type = <str> } }
                            -- Populated by ModParser.lua's conquerorList when the jewel's
                            -- implicit line ("Bathed in the blood of N ...") is parsed.
                        end
                    end
                end
            end
        end
    end
    -- ...
end
```

**Key Lua patterns:**
- `pairs(self.nodes)` — unordered iteration; Rust: `HashMap::iter()`. Order does not matter here.
- `not item.jewelData.limitDisabled` — `limitDisabled` is `nil` (falsy) when not set, `true` when over limit. Rust: `!item.jewel_data.limit_disabled`.
- `self.nodes[nodeId].nodesInRadius[radiusIndex][node.id]` — a nested table lookup that returns `true` or `nil`. Rust: `Option<bool>` — check `is_some()`.

---

### `conqueredBy` Structure (from `ModParser.lua` lines 41–65, 5592–5609)

The jewel's implicit line is parsed by rules such as:

```lua
["bathed in the blood of (%d+) sacrificed in the name of (.+)"] =
  function(num, _, name)
    return { mod("JewelData", "LIST",
      { key = "conqueredBy",
        value = { id = num,
                  conqueror = conquerorList[name:lower()] } }) }
  end,
```

`conquerorList` maps lowercased conqueror names to `{ id, type }`:

| Name       | id       | type      | Jewel type index |
|------------|----------|-----------|-----------------|
| xibaqua    | 1        | "vaal"    | 1 (Glorious Vanity)  |
| zerphi     | 2        | "vaal"    | 1 |
| doryani    | 3        | "vaal"    | 1 |
| ahuana     | "2_v2"   | "vaal"    | 1 |
| deshret    | 1        | "maraketh"| 3 (Brutal Restraint) |
| asenath    | 2        | "maraketh"| 3 |
| nasima     | 3        | "maraketh"| 3 |
| balbala    | "1_v2"   | "maraketh"| 3 |
| cadiro     | 1        | "eternal" | 5 (Elegant Hubris) |
| victario   | 2        | "eternal" | 5 |
| chitus     | 3        | "eternal" | 5 |
| caspiro    | "3_v2"   | "eternal" | 5 |
| kaom       | 1        | "karui"   | 2 (Lethal Pride) |
| rakiata    | 2        | "karui"   | 2 |
| kiloava    | 3        | "karui"   | 2 |
| akoya      | "3_v2"   | "karui"   | 2 |
| venarius   | 1        | "templar" | 4 (Militant Faith) |
| dominus    | 2        | "templar" | 4 |
| avarius    | 3        | "templar" | 4 |
| maxarius   | "1_v2"   | "templar" | 4 |
| vorana     | 1        | "kalguur" | 6 (Heroic Tragedy) |
| uhtred     | 2        | "kalguur" | 6 |
| medved     | 3        | "kalguur" | 6 |

`conqueredBy.id` is the jewel's numeric seed (the "N" captured from the implicit
text). `conqueredBy.conqueror` is the entry from this table.

**Note:** conqueror `id` can be a string like `"2_v2"` for variant conquerors —
this is used to index into LUT files for newer conquerors added after the original
six, so it must be handled as a string, not an integer.

---

### Pass 2 — Applying Replacements (lines 1117–1303)

```lua
-- PASS 2: Apply tattoo and timeless-jewel replacements
for id, node in pairs(self.nodes) do
    -- (tattoo / hash override handling omitted — belongs to SETUP-14)
    if self.hashOverrides[node.id] then
        self:ReplaceNode(node, self.hashOverrides[node.id])
    end

    -- TIMELESS JEWEL: if node was conquered in Pass 1
    if node.conqueredBy and node.type ~= "Socket" then
        local conqueredBy = node.conqueredBy
        local legionNodes    = self.tree.legion.nodes       -- indexed 1-based array
        local legionAdditions = self.tree.legion.additions  -- indexed 1-based array
```

**`self.tree.legion`** is a table on the `PassiveTree` with two fields:
- `legion.nodes` — array of replacement-node templates (each has `sd`, `stats`,
  `sortedStats`, `name`, `modList`, etc.)
- `legion.additions` — array of additive-stat templates (same structure)

`data.timelessJewelAdditions = 96` is the boundary: LUT values < 96 are
"addition" indices, values ≥ 96 are "replacement" indices (offset by 96 − 1).

```lua
        -- Determine jewel type (1-based index into timelessJewelTypes)
        -- FIXME comment in source: "continue implementing"
        local jewelType = 5            -- default: Elegant Hubris
        if conqueredBy.conqueror.type == "vaal"     then jewelType = 1
        elseif conqueredBy.conqueror.type == "karui"    then jewelType = 2
        elseif conqueredBy.conqueror.type == "maraketh" then jewelType = 3
        elseif conqueredBy.conqueror.type == "templar"  then jewelType = 4
        elseif conqueredBy.conqueror.type == "kalguur"  then jewelType = 6
        end
        -- Note: "eternal" falls through to the default jewelType = 5

        local seed = conqueredBy.id    -- numeric seed (N from implicit text)
        if jewelType == 5 then
            seed = seed / 20           -- Elegant Hubris seed is stored /20 in conqueredBy
        end
```

**Rust gotcha — `seed = seed / 20`:**
In Lua, all numbers are doubles; `seed / 20` is exact float division. In Rust,
if `seed` is stored as `u32` or `i64`, perform `seed as f64 / 20.0` and convert
back to `u64` (the LUT offset calculation uses it as an integer offset).

```lua
        -- Seed range validation (out-of-range seeds are silently skipped via ConPrintf)
        if seed ~= m_max(m_min(seed, seedMax), seedMin) then
            -- log error, but do NOT crash — just skip this node
        end
```

**Rust equivalent:** `if !(seed_min..=seed_max).contains(&seed) { log; continue; }`

---

### Notable Node Branch (lines 1166–1251)

```lua
        if node.type == "Notable" then
            local jewelDataTbl = { }           -- default: empty
            if seed_in_range then
                jewelDataTbl = data.readLUT(conqueredBy.id, node.id, jewelType)
                                               -- returns a Lua table of bytes (integers)
                                               -- [] if no entry exists for this (seed, node) pair
            end

            if not next(jewelDataTbl) then     -- next(t) returns nil for empty table
                ConPrintf("Missing LUT")       -- log warning, skip node
            else
```

`next(t)` in Lua returns `nil` when the table is empty. Rust: `jewel_data_tbl.is_empty()`.
`not next(jewelDataTbl)` → `jewel_data_tbl.is_empty()`.

#### Glorious Vanity (jewelType == 1) — complex notable replacement

```lua
                if jewelType == 1 then
                    local headerSize = #jewelDataTbl   -- length of result bytes
```

The LUT for Glorious Vanity returns a variable-length byte array. `headerSize`
determines which sub-case applies:

**Case headerSize == 2 or 3** (simple replacement):
```lua
                    if headerSize == 2 or headerSize == 3 then
                        -- Replace node with legionNodes[jewelDataTbl[1] + 1 - timelessJewelAdditions]
                        -- jewelDataTbl[1] is the "replacement" ID (≥ timelessJewelAdditions)
                        -- +1 converts from 0-based to 1-based Lua indexing
                        -- -timelessJewelAdditions maps to the legion node array index
                        self:ReplaceNode(node, legionNodes[jewelDataTbl[1] + 1 - data.timelessJewelAdditions])
                        -- Rust: legion_nodes[jewel_data[0] as usize + 1 - ADDITIONS]
                        --        (0-based Rust index = Lua index - 1)

                        for i, repStat in ipairs(legionNodes[...].sd) do
                            -- Replace stat values using format info from the source node
                            local statKey = legionNodes[...].sortedStats[i]
                            local statMod = legionNodes[...].stats[statKey]
                            repStat = replaceHelperFunc(repStat, statKey, statMod, jewelDataTbl[statMod.index + 1])
                            self:NodeAdditionOrReplacementFromString(node, repStat, i == 1)
                            -- i == 1 → replacement=true → wipe existing mods first
                        end
```

**Case headerSize == 6 or 8** (Might/Legacy of the Vaal with additions):
```lua
                    elseif headerSize == 6 or headerSize == 8 then
                        -- First half of bytes are "addition type" IDs
                        -- Second half are "roll values" for each addition
                        local bias = 0
                        for i, val in ipairs(jewelDataTbl) do
                            if i > (headerSize / 2) then break end
                            -- Count of "low" (≤21) vs "high" (>21) IDs determines
                            -- whether the node becomes Might (bias≥0) or Legacy (bias<0)
                            if val <= 21 then bias = bias + 1 else bias = bias - 1 end
                        end
                        if bias >= 0 then
                            self:ReplaceNode(node, legionNodes[77]) -- might of the vaal
                        else
                            self:ReplaceNode(node, legionNodes[78]) -- legacy of the vaal
                        end
                        -- Rust: legion_nodes[76] and legion_nodes[77] (0-based)
                        -- Note: legionNodes is 1-indexed in Lua!

                        -- Aggregate additions by type, summing rolls for duplicates
                        local additions = {}
                        for i, val in ipairs(jewelDataTbl) do
                            if i <= (headerSize / 2) then
                                local roll = jewelDataTbl[i + headerSize / 2]
                                if not additions[val] then
                                    additions[val] = roll
                                else
                                    additions[val] = additions[val] + roll
                                end
                            else break end
                        end
                        for add, val in pairs(additions) do
                            local addition = legionAdditions[add + 1]   -- 1-based
                            for _, addStat in ipairs(addition.sd) do
                                for k, statMod in pairs(addition.stats) do
                                    addStat = replaceHelperFunc(addStat, k, statMod, val)
                                end
                                self:NodeAdditionOrReplacementFromString(node, addStat)
                            end
                        end
```

**`replaceHelperFunc` (lines 1147–1164):**
```lua
local replaceHelperFunc = function(statToFix, statKey, statMod, value)
    -- statMod.fmt == "g" → game-format (requires unit conversion)
    if statMod.fmt == "g" then
        if statKey:find("per_minute") then value = round(value / 60, 1) end
        elseif statKey:find("permyriad") then value = value / 100 end
        elseif statKey:find("_ms") then value = value / 1000 end
    end
    -- Replace the placeholder range "(min-max)" or fixed value in the stat string
    if statMod.min ~= statMod.max then
        return statToFix:gsub("%(min%-max%)", value)  -- pattern with parens escaped
        -- Actually: :gsub("%(" .. statMod.min .. "%-" .. statMod.max .. "%)", value)
    elseif statMod.min ~= value then
        return statToFix:gsub(statMod.min, value)
    end
    return statToFix
end
```

**Rust equivalent:** String pattern replacement. PoB uses Lua pattern matching (not
regex) — `%d` is a digit, `%(` is literal `(`. Use `str::replace()` or a regex.

---

#### All other jewels (types 2–6 for notables, lines 1232–1251)

```lua
                else  -- jewelType != 1
                    for _, jewelData in ipairs(jewelDataTbl) do
                        if jewelData >= data.timelessJewelAdditions then  -- replace
                            jewelData = jewelData + 1 - data.timelessJewelAdditions
                            -- +1: convert 0-based offset to 1-based Lua array index
                            -- -timelessJewelAdditions: map to legionNodes index
                            local legionNode = legionNodes[jewelData]
                            if legionNode then
                                self:ReplaceNode(node, legionNode)
                            else
                                ConPrintf("Unhandled 'replace' ID: " .. jewelData)
                            end
                        elseif jewelData then  -- add (jewelData < timelessJewelAdditions)
                            local addition = legionAdditions[jewelData + 1]
                            for _, addStat in ipairs(addition.sd) do
                                self:NodeAdditionOrReplacementFromString(node, " \n" .. addStat)
                                -- " \n" prefix causes NodeAdditionOrReplacementFromString to
                                -- *add* the stat (newline split) rather than replace
                            end
                        elseif next(jewelData) then  -- unexpected: table value in array
                            ConPrintf("Unhandled OP: " .. jewelData + 1)
                        end
                    end
                end
```

For non-GV jewels the LUT (`readLUT` line 323–325) returns a 1-element array
`{ localId }` for notable nodes, or an empty table `{}` for normal nodes.
`localId` is the global-mapped replacement/addition index.

**Lua gotcha — `elseif jewelData then`:** In Lua, `0` is truthy. So `elseif 0 then` enters the add branch. But `elseif jewelData then` guards against `nil` (which would panic on arithmetic). Rust: just check `if jewel_data != 0`.

---

### Keystone Node Branch (lines 1253–1260)

```lua
        elseif node.type == "Keystone" then
            -- No LUT lookup — keystones are replaced by a named conqueror keystone
            local matchStr = conqueredBy.conqueror.type .. "_keystone_" .. conqueredBy.conqueror.id
            -- e.g. "vaal_keystone_1", "templar_keystone_2"
            for _, legionNode in ipairs(legionNodes) do
                if legionNode.id == matchStr then
                    self:ReplaceNode(node, legionNode)
                    break
                end
            end
```

**Rust note:** `legionNode.id` here is a **string** (like `"vaal_keystone_1"`),
not a numeric `u32`. The `legion.nodes` table has string IDs for conqueror-
specific keystones. Rust must store `legion_nodes` with a `String` id field,
OR build a HashMap keyed by that string.

`conqueredBy.conqueror.id` can be a string like `"2_v2"` (for variant conquerors)
so `matchStr` will be something like `"vaal_keystone_2_v2"`.

---

### Normal Node Branch (lines 1261–1300)

```lua
        elseif node.type == "Normal" then
            if conqueredBy.conqueror.type == "vaal" then
                -- Vaal (Glorious Vanity) normal nodes get LUT replacement + stat roll
                local jewelDataTbl = { }
                if seed_in_range then
                    jewelDataTbl = data.readLUT(conqueredBy.id, node.id, jewelType)
                end
                if not next(jewelDataTbl) then
                    ConPrintf("Missing LUT")
                else
                    self:ReplaceNode(node, legionNodes[jewelDataTbl[1] + 1 - timelessJewelAdditions])
                    for i, repStat in ipairs(node.sd) do    -- note: node.sd after ReplaceNode
                        local statKey = legionNodes[...].sortedStats[i]
                        local statMod = legionNodes[...].stats[statKey]
                        repStat = replaceHelperFunc(repStat, statKey, statMod, jewelDataTbl[2])
                        self:NodeAdditionOrReplacementFromString(node, repStat, true) -- replace
                    end
                end
            elseif conqueredBy.conqueror.type == "karui" then
                -- Lethal Pride: +2 or +4 Strength
                -- +2 if node.dn is in {"Dexterity","Intelligence","Strength"} OR isTattoo
                -- +4 otherwise
                local str = (isValueInArray(attributes, node.dn) or node.isTattoo) and "2" or "4"
                self:NodeAdditionOrReplacementFromString(node, " \n+" .. str .. " to Strength")

            elseif conqueredBy.conqueror.type == "maraketh" then
                -- Brutal Restraint: +2 or +4 Dexterity
                local dex = (isValueInArray(attributes, node.dn) or node.isTattoo) and "2" or "4"
                self:NodeAdditionOrReplacementFromString(node, " \n+" .. dex .. " to Dexterity")

            elseif conqueredBy.conqueror.type == "kalguur" then
                -- Heroic Tragedy: 1% or 2% increased Ward
                local ward = (isValueInArray(attributes, node.dn) or node.isTattoo) and "1" or "2"
                self:NodeAdditionOrReplacementFromString(node, " \n" .. ward .. "% increased Ward")

            elseif conqueredBy.conqueror.type == "templar" then
                -- Militant Faith: either replace with devotion node or add +5 Devotion
                if isValueInArray(attributes, node.dn) or node.isTattoo then
                    local legionNode = legionNodes[91] -- templar_devotion_node (1-indexed!)
                    self:ReplaceNode(node, legionNode)
                else
                    self:NodeAdditionOrReplacementFromString(node, " \n+5 to Devotion")
                end

            elseif conqueredBy.conqueror.type == "eternal" then
                -- Elegant Hubris: replace with blank node
                local legionNode = legionNodes[110] -- eternal_small_blank (1-indexed!)
                self:ReplaceNode(node, legionNode)
            end
            -- Note: no branch for "karui" and "maraketh" notables; those go through the
            -- generic notable LUT path above.
        end

        self:ReconnectNodeToClassStart(node)
```

**`attributes` array** (line 1068): `{ "Dexterity", "Intelligence", "Strength" }`.
`isValueInArray(attributes, node.dn)` returns `true` if the node's display name
(`dn`) is one of the three attribute strings. These attribute nodes get *smaller*
bonuses (+2 instead of +4) from Lethal Pride / Brutal Restraint.

**`node.isTattoo`** — true if this node was a tattoo replacement; also gets
the smaller bonus. These nodes have already been marked by `ReplaceNode` from
tattoo processing if SETUP-14 is implemented.

---

### `ReplaceNode` (lines 1541–1562)

```lua
function PassiveSpecClass:ReplaceNode(old, newNode)
    if old.sd == newNode.sd then return 1 end  -- already identical, skip
    old.dn = newNode.dn          -- display name
    old.sd = newNode.sd          -- stat descriptions (array of strings)
    old.name = newNode.name      -- internal name
    old.mods = newNode.mods      -- parsed modifier definitions
    old.modKey = newNode.modKey  -- cache key
    old.modList = new("ModList") -- fresh ModList
    old.modList:AddList(newNode.modList)  -- copy mods from newNode
    old.sprites = newNode.sprites
    old.effectSprites = newNode.effectSprites
    old.isTattoo = newNode.isTattoo
    old.overrideType = newNode.overrideType
    old.keystoneMod = newNode.keystoneMod
    old.icon = newNode.icon
    old.spriteId = newNode.spriteId
    old.activeEffectImage = newNode.activeEffectImage
    old.reminderText = newNode.reminderText or { }
end
```

In Rust, `PassiveNode` will need to be extended with these additional fields
(currently only `stats`, `name`, `node_type`, `linked_ids` are stored). The
key field for the calculation is `mod_list`/`stats` — these determine what mods
flow into the ModDb from allocated nodes.

---

### `ReconnectNodeToClassStart` (lines 1564–1574)

```lua
function PassiveSpecClass:ReconnectNodeToClassStart(node)
    for _, linkedNodeId in ipairs(node.linkedId) do
        for classId, class in pairs(self.tree.classes) do
            if linkedNodeId == class.startNodeId and node.type == "Normal" then
                -- Pure Talent keystone reads "Condition:ConnectedTo<Class>Start" flags
                node.modList:NewMod(
                    "Condition:ConnectedTo" .. class.name .. "Start",
                    "FLAG", true,
                    "Tree:" .. linkedNodeId
                )
            end
        end
    end
end
```

This fires after every timeless-jewel replacement. For nodes directly adjacent to
a class start node, it adds a `Condition:ConnectedTo<Class>Start` flag mod. This
is used by the **Pure Talent** keystone unique jewel to grant bonuses. Required
for correctness but only relevant when Pure Talent is equipped.

---

### `NodeAdditionOrReplacementFromString` (lines 2104–2143)

```lua
function PassiveSpecClass:NodeAdditionOrReplacementFromString(node, sd, replacement)
    local addition = { sd = {sd}, mods = {}, modList = new("ModList"), modKey = "" }
    -- Split "text\nmore text" into separate stat lines (handles embedded newlines)
    local i = 1
    while addition.sd[i] do
        if addition.sd[i]:match("\n") then
            -- Split on \n, insert each line back into sd at same position
            ...
        end
        -- Parse the stat line into mod(s)
        local parsedMod = modLib.parseMod(addition.sd[i])
        -- Attempt multi-line combinations if parse fails
        ...
        -- Add to node's modList
        if replacement and i == 1 then
            -- wipe existing mods, then add
            node.modList = new("ModList")
        end
        node.modList:AddList(addition.modList)
        i = i + 1
    end
end
```

**Rust equivalent:** Call `mod_parser::parse_mod()` on each stat line and add the
resulting mods to the node's mod list. The `replacement=true` path clears the
existing mod list first.

---

### `data.readLUT` (DataLegionLookUpTableHelper.lua, lines 292–331)

```lua
local function readLUT(seed, nodeID, jewelType)
    loadTimelessJewel(jewelType, nodeID)   -- lazy-load binary LUT file
    if jewelType == 5 then
        seed = seed / 20                   -- Elegant Hubris seeds are /20
    end
    local seedOffset = (seed - timelessJewelSeedMin[jewelType])
    local seedSize   = (timelessJewelSeedMax[jewelType] - timelessJewelSeedMin[jewelType]) + 1
    local index = data.nodeIDList[nodeID] and data.nodeIDList[nodeID].index or nil
    if index then
        if jewelType == 1 then  -- Glorious Vanity
            -- Result = slice of binary data, returned as array of byte values
            local result = { }
            local dataLength = sizes:byte(index * seedSize + seedOffset + 1)
            for i = 1, dataLength do
                result[i] = data[index + 1][seedOffset + 1]:byte(i)
            end
            -- Convert local IDs to global IDs for replacements/additions
            ...
            return result
        elseif index <= nodeIDList["sizeNotable"] then  -- notable node in other jewels
            local localId = data:byte(index * seedSize + seedOffset + 1)
            return { convertLocalIdToGlobalId(jewelType, localId) }
        end
        -- Normal (non-notable) nodes return {} (no replacement data)
    end
    return { }
end
```

**Rust equivalent:** Pre-load each LUT file at startup as a byte array. Index
using `(seed - seed_min) + index * seed_size` (all integer arithmetic). For
Glorious Vanity the LUT has a "sizes" prefix followed by per-node data; for other
types it is a flat 2D array of 1 byte per (node, seed) pair.

---

## Existing Rust Code

**File:** `crates/pob-calc/src/passive_tree/mod.rs` (323 lines)  
**File:** `crates/pob-calc/src/calc/setup.rs` (2658 lines)  
**File:** `crates/pob-calc/src/build/mod_parser_generated.rs` (lines 7089–7095,
38502–38591)

### What exists

1. **`PassiveNode` struct** (`passive_tree/mod.rs:95–115`) — stores `id`, `name`,
   `stats` (Vec<String>), `linked_ids`, `node_type`, `ascendancy_name`, `icon`,
   `skill_points_granted`, `class_start_index`, `expansion_jewel`. No `mod_list`
   field; no `conqueredBy` field; no `legion` field on `PassiveTree`.

2. **`add_jewel_mods()`** (`setup.rs:1902–1940`) — iterates jewel slots, parses
   mod lines, adds to `player.mod_db`. Handles regular jewel mods. Does NOT:
   - Check whether the jewel has a `conqueredBy` value
   - Walk the radius nodes to assign `conqueredBy`
   - Perform any tree-node replacement

3. **`conqueredBy` mod parser** (`mod_parser_generated.rs:38502–38591`) — the
   six regex rules for timeless jewel implicit lines (rules 1976–1981) each emit
   a `JewelData` mod with `ModValue::Number(0.0)` and a `/* TODO */` comment.
   The actual `conqueredBy` data structure (seed + conqueror type + id) is not
   implemented. The conqueror name is captured in `caps.get(2)` but discarded.

4. **`nodesInRadius`** — not present on any Rust struct. Radius computation for
   jewels is entirely missing from the Rust code.

### What's missing

1. **`conqueredBy` data type** — need a `ConqueredBy { seed: u64, conqueror_type: String, conqueror_id: String }` struct (or equivalent) stored as a `ModValue::JewelData` variant.

2. **`nodesInRadius` precomputation** — for each jewel socket node, precompute
   which other nodes fall within each radius ring. Currently this data does not
   exist; it is built in PoB's `PassiveTree` class when node positions are
   computed. Required for Pass 1 (assigning `conqueredBy` to radius nodes).

3. **`PassiveNode` mod list** — `PassiveNode` currently stores only `stats: Vec<String>` (raw stat text). For timeless jewels (and radius jewels in SETUP-08) nodes need a live `ModList`-equivalent that can be mutated (stats cleared, new mods added). The existing string-based pipeline re-parses stats each time from the build's passive-node list; this needs to support per-node overrides.

4. **`PassiveTree::legion` field** — `legionNodes` and `legionAdditions` tables
   from `tree.legion` must be parsed from the game data and stored on `PassiveTree`.
   Currently absent.

5. **LUT files** — binary files in `third-party/PathOfBuilding/src/Data/TimelessJewelData/`
   (e.g. `GloriousVanity.zip`, `LethalPride.zip`, etc.) and the
   `NodeIndexMapping.lua` must be read and parsed at startup. No Rust code reads
   these files.

6. **`NodeAdditionOrReplacementFromString` equivalent** — need a function that
   takes a stat string, parses it into mods, and merges them into a node's mod
   list (with optional clear-first for replacement mode).

7. **`ReconnectNodeToClassStart`** — needs a post-replacement pass that adds
   `Condition:ConnectedTo<Class>Start` flags for nodes adjacent to class starts.

### What's wrong

- `mod_parser_generated.rs` rules 1976–1981 capture `num` (the seed) but throw
  it away. They emit `ModValue::Number(0.0)` instead of the structured
  `conqueredBy` value. As a result, `item.jewelData.conqueredBy` is never
  populated, Pass 1 never sets `node.conqueredBy`, and no node replacements
  occur. The entire timeless jewel pipeline is a no-op.

---

## What Needs to Change

1. **Add `ConqueredBy` to `ModValue`** — add a new variant
   `ModValue::JewelData(JewelDataValue)` (or a specific `ModValue::ConqueredBy`)
   carrying `{ seed: u64, conqueror_type: ConquerorType, conqueror_id: String }`.
   `ConquerorType` is an enum of `Vaal`, `Karui`, `Maraketh`, `Templar`, `Eternal`,
   `Kalguur`.

2. **Fix mod_parser_generated.rs rules 1976–1981** — parse `caps.get(1)` as seed
   and `caps.get(2)` as conqueror name, look up name in the conqueror list, emit
   the correct `ModValue::ConqueredBy`. (Or regenerate from the Lua source
   generator once step 1 is done.)

3. **Build `nodesInRadius` on `PassiveNode`/`PassiveTree`** — during tree loading,
   for each jewel-socket node compute which other nodes fall within each of the
   four radius rings (small/medium/large/extra-large, typically indices 1–4).
   Requires knowing node positions (x, y coordinates from the tree JSON).

4. **Load `tree.legion` data** — parse `legionNodes` and `legionAdditions` from
   the game data. These are in the passive tree data files (PoB loads them from
   `PassiveTree.lua`). Add `legion_nodes: Vec<LegionNode>` and
   `legion_additions: Vec<LegionAddition>` to `PassiveTree`.

5. **Load and index LUT files** — load the 5–6 timeless jewel binary LUT files
   at startup (or lazily on first use). Parse `NodeIndexMapping.lua` for the
   Glorious Vanity node index mapping. Implement `read_lut(seed, node_id, jewel_type)`
   returning `Vec<u8>`.

6. **Add `mod_list` to `PassiveNode`** — store a mutable `Vec<Mod>` (or equivalent)
   on each passive node that can be replaced/augmented per-build. Currently only
   `stats: Vec<String>` is stored; the mods are re-parsed from those strings at
   calculation time. Timeless jewel replacement requires node-level mod override.

7. **Implement `apply_timeless_jewels()`** in `setup.rs` — two-pass algorithm:
   - Pass 1: for each jewel slot, if jewel has `conqueredBy` mod: walk
     `nodesInRadius` and set `node.conquered_by`.
   - Pass 2: for each node with `conquered_by` set, call appropriate replacement
     logic (Notable/Keystone/Normal × jewel type).

8. **Implement `replace_node()`** in `passive_tree/mod.rs` — copies name, stats,
   mod_list from `new_node` into `old_node`.

9. **Implement `node_addition_or_replacement_from_string()`** — parse stat string
   into mods, optionally clearing the node's mod list first.

10. **Implement `reconnect_node_to_class_start()`** — after each replacement, add
    `Condition:ConnectedTo<Class>Start` flag mods to nodes adjacent to class starts.

11. **`replaceHelperFunc` stat-value substitution** — implement string pattern
    replacement of `(min-max)` ranges and fixed values in stat strings for Glorious
    Vanity and Vaal Normal node branches.
