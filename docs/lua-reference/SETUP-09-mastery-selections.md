# SETUP-09-mastery-selections: Mastery Selections

## Output Fields

This chunk produces **no direct output fields**. Its correctness manifests
indirectly: mastery selections replace a mastery node's generic stats with the
player-chosen effect, which contributes mods to the ModDb. Those mods feed into
every downstream chunk.

Additionally, CalcSetup.lua writes three `Multiplier:*` mods that mods can scale
off of:

| ModDb key | Meaning |
|---|---|
| `Multiplier:AllocatedMastery` | Total number of allocated mastery nodes |
| `Multiplier:AllocatedMasteryType` | Number of distinct mastery types allocated |
| `Multiplier:AllocatedLifeMastery` | Number of Life Mastery nodes specifically |

These multipliers are not output fields (they don't appear in `output[]`) but
they must be set correctly in the ModDb before any mod with a
`Multiplier:AllocatedMastery` tag is evaluated.

## Dependencies

- **SETUP-01**: XML parser must have parsed `<Spec masteryEffects="..."/>` into
  `PassiveSpec.mastery_selections` (a `HashMap<u32, u32>` mapping node ID →
  effect ID). This field does not yet exist in `PassiveSpec`.
- **SETUP-04**: `eval_mod` stubs for tag types must be correct before mastery
  mods (which use `Multiplier:*` tags) are evaluated.

## Lua Source

Primary file: `third-party/PathOfBuilding/src/Classes/PassiveSpec.lua`
Commit: `454eff8c85d24356d9b051d596983745ed367476`

Supporting file: `third-party/PathOfBuilding/src/Classes/PassiveTree.lua`
Commit: `454eff8c85d24356d9b051d596983745ed367476`

Supporting file: `third-party/PathOfBuilding/src/Modules/CalcSetup.lua`
Commit: `454eff8c85d24356d9b051d596983745ed367476`

Key sections:

| File | Lines | What happens |
|------|-------|---|
| `PassiveSpec.lua` | 137–141 | XML attribute `masteryEffects` parsed from `<Spec>` |
| `PassiveSpec.lua` | 239–288 | `ImportFromNodeList` — mastery selections stored, mastery nodes gated on having a selection |
| `PassiveSpec.lua` | 309–350 | `AllocateMasteryEffects` — URL decode path (binary mastery effect encoding) |
| `PassiveSpec.lua` | 1306–1363 | `BuildAllDependsAndPaths` mastery loop — per-node effect application + counting |
| `PassiveTree.lua` | 460, 497–506 | `self.masteryEffects` lookup table built from `node.masteryEffects` |
| `CalcSetup.lua` | 582–671 | Read `spec.allocatedMastery*` and write `Multiplier:*` mods |

## Annotated Lua

### Step 1 — XML attribute format (`PassiveSpec.lua:137–141`)

```lua
local masteryEffects = { }
if xml.attrib.masteryEffects then
    for mastery, effect in xml.attrib.masteryEffects:gmatch("{(%d+),(%d+)}") do
        masteryEffects[tonumber(mastery)] = tonumber(effect)
    end
end
```

The `masteryEffects` attribute in `<Spec>` is a comma-separated list of
`{masteryNodeId,effectId}` pairs.  Example:

```xml
<Spec treeVersion="3_25" nodes="12,34,56" classId="1" ascendClassId="2"
      masteryEffects="{12345,48385},{67890,4119}"/>
```

**Lua pattern `gmatch("{(%d+),(%d+)}")`**: captures two digit-groups inside
`{…}`, ignoring the surrounding `{` and `}`. In Rust, use a regex like
`\{(\d+),(\d+)\}` and iterate all matches.

**`tonumber(mastery)`**: Lua converts the captured string to a number
automatically when used as a table key. In Rust, `s.parse::<u32>().unwrap_or(0)`.

After this loop: `masteryEffects` is a Lua table `{ [nodeId] = effectId, … }`,
equivalent to Rust's `HashMap<u32, u32>`.

### Step 2 — `ImportFromNodeList`: storing selections and gating allocation (`PassiveSpec.lua:239–277`)

```lua
function PassiveSpecClass:ImportFromNodeList(
    classId, ascendClassId, secondaryAscendClassId,
    hashList, hashOverrides, masteryEffects, treeVersion)

    -- ...
    wipeTable(self.masterySelections)          -- clears the map (Rust: HashMap::clear())
    for mastery, effect in pairs(masteryEffects) do
        -- ignore ggg codes from profile import (effectId >= 65536 are reserved)
        if (tonumber(effect) < 65536) then
            self.masterySelections[mastery] = effect
        end
    end

    -- ... (hash overrides applied)

    for _, id in pairs(hashList) do
        local node = self.nodes[id]
        if node then
            -- KEY: mastery nodes are only allocated if they have a selection
            if node.type ~= "Mastery" or (node.type == "Mastery" and self.masterySelections[id]) then
                node.alloc = true
                self.allocNodes[id] = node
            end
        else
            t_insert(self.allocSubgraphNodes, id)
        end
    end
end
```

**`wipeTable(t)`**: a PoB utility that clears all keys from the table. Rust:
`self.mastery_selections.clear()`.

**`pairs(masteryEffects)`**: iterates all key-value pairs of the table
(unordered). Rust: `.iter()`.

**`tonumber(effect) < 65536`**: GGG's profile import API sometimes returns
large numeric codes for masteries that don't map to a local effect ID. These are
filtered out. Rust: `effect < 65536`.

**Critical gate**: if a node ID is in `hashList` (the raw allocated-node list
from XML) but its type is `"Mastery"` and there is no matching entry in
`masterySelections`, the node is **not allocated**. In the current Rust code,
`PassiveSpec::allocated_nodes` is populated from the XML `nodes` attribute
without any mastery check — mastery nodes in that set with no selection will
incorrectly contribute their empty `stats` array (or whatever the tree JSON
stores for them).

### Step 3 — `BuildAllDependsAndPaths`: per-node effect application + counting (`PassiveSpec.lua:1306–1363`)

```lua
-- Called after ImportFromNodeList; called by BuildAllDependsAndPaths
self.allocatedMasteryCount = 0
self.allocatedNotableCount = 0
self.allocatedKeystoneCount = 0
self.allocatedMasteryTypes = { }        -- map: mastery group name → count
self.allocatedMasteryTypeCount = 0
self.allocatedTattooTypes = { }

for id, node in pairs(self.nodes) do    -- ALL nodes, not just allocated
    if self.ignoredNodes[id] and self.allocNodes[id] then
        -- remove ignored (Intuitive Leap etc) nodes from allocation
        self.nodes[id].alloc = false
        self.allocNodes[id] = nil
        self.ignoredNodes[id] = nil
    else
        if node.type == "Mastery" and self.masterySelections[id] then
            -- Node has a selected effect
            local effect = self.tree.masteryEffects[self.masterySelections[id]]
            if effect and self.allocNodes[id] then
                if self.hashOverrides and self.hashOverrides[id] then
                    self:ReplaceNode(node, self.hashOverrides[id])
                else
                    node.sd = effect.sd    -- REPLACE generic stats with selected effect stats
                end
                node.allMasteryOptions = false
                node.reminderText = { "Tip: Right click to select a different effect" }
                self.tree:ProcessStats(node)   -- parse mod lines from effect.sd
                -- Count for multipliers
                self.allocatedMasteryCount = self.allocatedMasteryCount + 1
                if not self.allocatedMasteryTypes[self.allocNodes[id].name] then
                    self.allocatedMasteryTypes[self.allocNodes[id].name] = 1
                    self.allocatedMasteryTypeCount = self.allocatedMasteryTypeCount + 1
                else
                    local prevCount = self.allocatedMasteryTypes[self.allocNodes[id].name]
                    self.allocatedMasteryTypes[self.allocNodes[id].name] = prevCount + 1
                    if prevCount == 0 then
                        self.allocatedMasteryTypeCount = self.allocatedMasteryTypeCount + 1
                    end
                end
            else
                -- unrecognized effect ID or node not actually allocated — dealloc
                self.nodes[id].alloc = false
                self.allocNodes[id] = nil
                self.masterySelections[id] = nil
            end
        elseif node.type == "Mastery" then
            -- Mastery with no selection: show all options (UI only, not relevant to calc)
            self:AddMasteryEffectOptionsToNode(node)
        elseif node.type == "Notable" and node.alloc then
            self.allocatedNotableCount = self.allocatedNotableCount + 1
        elseif node.type == "Keystone" and node.alloc then
            self.allocatedKeystoneCount = self.allocatedKeystoneCount + 1
        end
        -- (tattoo counting also happens here — see SETUP-14)
    end
end
```

**`node.sd = effect.sd`**: This is the core substitution. The mastery node's
human-readable stat list (`sd` = "stat descriptions") is replaced in-place with
the selected effect's stats. In Rust, this means the `PassiveNode.stats` vec
must be replaced with the selected effect's stat strings before the node's mods
are parsed and added to the ModDb.

**`self.tree.masteryEffects[self.masterySelections[id]]`**: Two-level lookup.
`masterySelections[id]` gives the effect ID (e.g. `48385`). Then
`tree.masteryEffects[48385]` gives a table `{ id=48385, sd={"some stat string"} }`.
In Rust: first look up the `effectId` in `PassiveSpec::mastery_selections`, then
look up the effect in a `HashMap<u32, MasteryEffect>` (which doesn't exist yet —
see gaps section).

**`allocatedMasteryTypes[node.name]`**: grouped by the mastery node's display
name (e.g. `"Life Mastery"`, `"Elemental Mastery"`). A "type" is first seen
when `allocatedMasteryTypes[name]` is `nil`; a new first allocation sets count
to 1 and increments `allocatedMasteryTypeCount`. Subsequent allocations just
increment the per-name count; `allocatedMasteryTypeCount` only increments again
if the previous count was 0 (which can happen due to the add/remove logic in
the `override.addNodes`/`override.removeNodes` path).

**`prevCount == 0` branch**: This handles the case where `override.removeNodes`
previously decremented a type's count to 0 (making it "inactive") and we're
now re-adding one. This only matters for the `addNodes`/`removeNodes` code path
(used by PoB's UI for previewing node changes). In the normal calculation path
`allocatedMasteryTypes[name]` is either nil (first allocation) or positive.

### Step 4 — CalcSetup: write Multiplier mods (`CalcSetup.lua:582–671`)

```lua
-- At the start of CalcSetup (each recalculation), copy from spec:
local allocatedMasteryCount     = env.spec.allocatedMasteryCount
local allocatedMasteryTypeCount = env.spec.allocatedMasteryTypeCount
local allocatedMasteryTypes     = copyTable(env.spec.allocatedMasteryTypes)
```

**`copyTable(t)`**: PoB utility for shallow-copying a table. In Rust:
`env.spec.allocated_mastery_types.clone()` (or just read-only access if not
modified).

These values are then potentially adjusted by `override.addNodes` /
`override.removeNodes` (lines 597–648) before being used for the mod writes.
The `addNodes`/`removeNodes` logic applies the same counting as Step 3 but in
the forward/reverse direction. This code path only fires when PoB is computing
diffs for the UI (e.g. hovering over an unallocated node). During normal
calculation, neither `override.addNodes` nor `override.removeNodes` is set, so
the initial values from `env.spec` are used as-is.

```lua
-- Write Multiplier mods (CalcSetup.lua:664–671):
if allocatedMasteryCount and allocatedMasteryCount > 0 then
    modDB:NewMod("Multiplier:AllocatedMastery", "BASE", allocatedMasteryCount)
end
if allocatedMasteryTypeCount and allocatedMasteryTypeCount > 0 then
    modDB:NewMod("Multiplier:AllocatedMasteryType", "BASE", allocatedMasteryTypeCount)
end
if allocatedMasteryTypes["Life Mastery"] and allocatedMasteryTypes["Life Mastery"] > 0 then
    modDB:NewMod("Multiplier:AllocatedLifeMastery", "BASE", allocatedMasteryTypes["Life Mastery"])
end
```

**`modDB:NewMod("Multiplier:AllocatedMastery", "BASE", count)`**: writes a BASE
mod whose name is `"Multiplier:AllocatedMastery"`. In Rust's ModDb, this is set
via `db.set_multiplier("AllocatedMastery", count as f64)` (the `Multiplier:`
prefix is stripped by convention in the current code, see `eval_mod.rs`
`ModTag::Multiplier` handling).

**`allocatedMasteryCount and allocatedMasteryCount > 0`**: Lua nil-and-positive
guard. If `allocatedMasteryCount` is nil (not set), the whole expression
short-circuits to `nil` (falsy) without erroring. In Rust this is
`if count > 0`. Since the count is always initialized (from `env.spec`), it
cannot be nil in practice.

**Only "Life Mastery" gets a specific multiplier** in the current PoB code. Other
mastery types are counted in `allocatedMasteryTypeCount` but don't get their own
`Multiplier:AllocatedXxxMastery` mod. "Life Mastery" is special-cased here
because some mods reference it explicitly (e.g. "Recover 2% of Life on Kill for
each Life Mastery you have Allocated").

### Step 5 — `AllocateMasteryEffects` URL decode path (`PassiveSpec.lua:309–350`)

```lua
function PassiveSpecClass:AllocateMasteryEffects(masteryEffects, endian)
    for i = 1, #masteryEffects - 1, 4 do
        local effectId, id
        if endian == "big" then
            effectId = masteryEffects:byte(i) * 256 + masteryEffects:byte(i + 1)
            id       = masteryEffects:byte(i + 2) * 256 + masteryEffects:byte(i + 3)
        else
            -- little endian: poeplanner URL format
            effectId = masteryEffects:byte(i + 2) + masteryEffects:byte(i + 3) * 256
            id       = masteryEffects:byte(i)     + masteryEffects:byte(i + 1) * 256
            local node = self.nodes[id]
            if node then
                node.alloc = true
                self.allocNodes[id] = node
            end
        end
        local effect = self.tree.masteryEffects[effectId]
        if effect then
            self.allocNodes[id].sd = effect.sd
            self.allocNodes[id].allMasteryOptions = false
            self.allocNodes[id].reminderText = { "…" }
            self.tree:ProcessStats(self.allocNodes[id])
            self.masterySelections[id] = effectId
            -- … (counting same as Step 3)
        else
            -- effect not in current tree: dealloc
            self.allocNodes[id] = nil
            self.nodes[id].alloc = false
        end
    end
end
```

This function is **only called from URL decode paths** (GGG passive tree URL
import or poeplanner URL import), not from the standard XML load path. The Rust
XML parser only needs to handle the `masteryEffects` XML attribute (Step 1).
URL decode is not exercised by any oracle build and is **out of scope** for this
chunk.

### Step 6 — `PassiveTree.lua`: building `self.masteryEffects` lookup (`PassiveTree.lua:460, 497–506`)

```lua
self.masteryEffects = { }    -- initialized in PassiveTree:Load()

-- For each node in tree data:
elseif node.m or node.isMastery then
    node.type = "Mastery"
    if node.masteryEffects then
        for _, effect in pairs(node.masteryEffects) do
            if not self.masteryEffects[effect.effect] then
                -- First time seeing this effect ID: parse its stat lines
                self.masteryEffects[effect.effect] = { id = effect.effect, sd = effect.stats }
                self:ProcessStats(self.masteryEffects[effect.effect])
            else
                -- Already processed: copy the parsed sd back (for deduplication)
                effect.stats = self.masteryEffects[effect.effect].sd
            end
        end
    end
```

`self.masteryEffects` is a flat lookup: `effectId (u32)` → `{ id, sd }` where
`sd` is the list of stat-description strings. In Rust this needs to be a
`HashMap<u32, Vec<String>>` keyed on effect ID.

**`node.masteryEffects` in tree data**: Each mastery node in PoB's `tree.lua`
(not the same as the Rust `poe1_current.json`) has a `masteryEffects` array of
`{ effect=<u32>, stats={"…"} }` entries. The PoB tree JSON data files (e.g.
`3_27_alternate/tree.lua`) include these, but the Rust `poe1_current.json` tree
format (produced by the data extractor) does **not** include `masteryEffects`
per-node or the global `tree.masteryEffects` lookup table. This is the main data
gap.

## Existing Rust Code

### `crates/pob-calc/src/build/types.rs` (lines 20–30)

```rust
pub struct PassiveSpec {
    pub tree_version: String,
    pub allocated_nodes: HashSet<u32>,
    pub class_id: u32,
    pub ascend_class_id: u32,
    pub jewels: HashMap<u32, u32>,
}
```

**Missing**: `mastery_selections: HashMap<u32, u32>` (node ID → effect ID).
Without this field, the XML parser has nowhere to store parsed mastery
selections, and `add_passive_mods()` cannot apply them.

### `crates/pob-calc/src/build/xml_parser.rs` (lines 140–163)

The XML parser reads `nodes`, `classId`, `ascendClassId`, `treeVersion`, and
`<Sockets>/<Socket>`, but does **not** read `masteryEffects`. No code exists to
parse the `{masteryNodeId,effectId}` pairs.

**Missing**: parsing of `attrs.get("masteryEffects")` and populating
`PassiveSpec::mastery_selections`.

### `crates/pob-calc/src/passive_tree/mod.rs`

`PassiveNode` stores `is_mastery: bool` (correctly classified from tree JSON).
`PassiveNode.stats` stores the node's stat strings. However, for a mastery node,
`stats` in the tree JSON is always `[]` (empty) because the Rust data extractor
does not extract `masteryEffects` per-node. There is **no** `mastery_effects`
field on `PassiveNode` and **no** global `mastery_effects: HashMap<u32, Vec<String>>`
on `PassiveTree`.

### `crates/pob-calc/src/calc/setup.rs` (lines 317–337)

`add_passive_mods()` iterates `connected_passive_nodes()` and applies each node's
`stats` to the ModDb. For mastery nodes, `stats` is empty, so they contribute
nothing. The function has no special handling for `NodeType::Mastery` — it does
not:
1. Skip unselected mastery nodes (they should be excluded from `connected_passive_nodes`
   output, which requires the gate in `ImportFromNodeList` — Step 2 above)
2. Replace a mastery node's stats with the selected effect's stats before parsing
3. Write `Multiplier:AllocatedMastery`, `Multiplier:AllocatedMasteryType`, or
   `Multiplier:AllocatedLifeMastery` mods

### `crates/data-extractor/src/transform/tree.rs` (lines 323–325)

```rust
// IsMastery / IsJustIcon is not yet mapped to an offset;
// default false (can be calibrated later if needed).
is_mastery: false,
```

The data extractor **hard-codes `is_mastery: false`** for all nodes extracted
from the binary GGPK. However, the `poe1_current.json` tree file used in
production is generated from PoB's Lua tree data (not from GGPK), so this
extractor gap does not affect the current tree JSON. The `poe1_current.json`
tree file *does* have `is_mastery: true` on the 351 mastery nodes (confirmed
empirically), but it lacks the `mastery_effects` array for each node.

## What Needs to Change

1. **Add `mastery_selections` to `PassiveSpec`** (`types.rs`):
   ```rust
   pub mastery_selections: HashMap<u32, u32>, // node_id → effect_id
   ```

2. **Add mastery effect data to tree JSON and `PassiveTree`**:
   - Add `mastery_effects: HashMap<u32, Vec<String>>` to `PassiveTree` struct
     (global effect-id → stat strings lookup).
   - Add `mastery_effects: Vec<(u32, Vec<String>)>` to `RawNode` and
     `PassiveNode` so each mastery node carries its per-node effect options.
   - Update the tree JSON serialization in the data pipeline (or add a separate
     mastery effects JSON sidecar) to include mastery effect data from PoB's
     `tree.lua` files. The PoB submodule's `TreeData/3_*/tree.lua` files are the
     source of truth.

3. **Parse `masteryEffects` attribute in XML parser** (`xml_parser.rs`):
   - When parsing a `<Spec>` element, read `attrs.get("masteryEffects")`.
   - Match all `{(\d+),(\d+)}` pairs.
   - Store as `passive_spec.mastery_selections[node_id] = effect_id` (skipping
     effect IDs ≥ 65536 per the GGG import filter in Step 2).

4. **Gate mastery node allocation on having a selection** (`xml_parser.rs` or
   `setup.rs`):
   - After parsing the `nodes` attribute, for any node ID that maps to a
     `NodeType::Mastery` in the tree, only insert it into `allocated_nodes` if
     `mastery_selections.contains_key(&node_id)`.
   - Alternatively, handle this in `add_passive_mods()` by skipping mastery
     nodes that have no selection.

5. **Apply selected effect stats in `add_passive_mods()`** (`setup.rs`):
   - For each reachable mastery node, look up `passive_spec.mastery_selections[node_id]`.
   - If found, look up `tree.mastery_effects[effect_id]` to get the stat strings.
   - Parse those stat strings (not `node.stats`) and add them to the ModDb.
   - If not found (node in allocated but no selection — shouldn't happen after
     change 4, but be defensive), skip the node.

6. **Write Multiplier mods** (`setup.rs`, in `add_passive_mods()` or a new
   `add_mastery_multiplier_mods()` called from `initEnv`):
   - Count allocated mastery nodes and distinct mastery types by iterating
     `mastery_selections` and looking up `node.name` for each selected node.
   - `db.set_multiplier("AllocatedMastery", count)` if count > 0.
   - `db.set_multiplier("AllocatedMasteryType", type_count)` if > 0.
   - `db.set_multiplier("AllocatedLifeMastery", life_mastery_count)` if > 0.
   - Use `node.name` (the mastery group name, e.g. `"Life Mastery"`) to
     determine which specific type multiplier to set.

## Priority and Oracle Coverage

**MEDIUM PRIORITY** per the spec. No current oracle builds use masteries
(all test builds use pre-3.16 passive trees). However:

- Any modern PoE 1 build (3.16+) will use masteries; without this, those builds
  will silently drop all mastery effects.
- The `Multiplier:AllocatedMastery` and related mods are referenced in the
  generated mod parser (`mod_parser_generated.rs:3315, 3321`) and in
  several specific mod patterns for Life/Mana/ES recovery on kill.
- This chunk is **blocked** until oracle builds are updated to include 3.16+
  builds with masteries, as noted in the spec (section 5.2).

## Notable Gaps / Issues Summary

| Gap | Severity | Location |
|-----|----------|----------|
| `PassiveSpec` has no `mastery_selections` field | Blocker | `types.rs` |
| XML parser ignores `masteryEffects` attribute | Blocker | `xml_parser.rs` |
| Tree JSON lacks `masteryEffects` per-node array | Blocker | `poe1_current.json`, data pipeline |
| `PassiveTree` has no global `mastery_effects` lookup | Blocker | `passive_tree/mod.rs` |
| `add_passive_mods` does not apply effect stats | Missing | `setup.rs` |
| No mastery gating in node allocation | Bug | `xml_parser.rs` / `setup.rs` |
| `Multiplier:AllocatedMastery*` mods never written | Missing | `setup.rs` |
| Data extractor hard-codes `is_mastery: false` | Minor (extractor only) | `data-extractor/transform/tree.rs` |
