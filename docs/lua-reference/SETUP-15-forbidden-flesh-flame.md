# SETUP-15: Forbidden Flesh/Flame (Granted Ascendancy Node)

## Output Fields

This chunk writes **no direct output fields**. It extends `env.allocNodes` and
`env.grantedPassives` in-place with an ascendancy notable granted by a matched
Forbidden Flesh + Forbidden Flame jewel pair. The notable's mods are then merged
into `env.modDB` by the passive-merge call at CalcSetup.lua lines 1260–1265
(shared with SETUP-07). Every downstream chunk benefits from those mods.

**No oracle builds currently exercise this chunk.** The spec marks it
**LOW PRIORITY — combine with SETUP-07 when implementing** (spec §5.2, SETUP-15).

---

## Dependencies

- **SETUP-01** (item mod parsing): `env.modDB` must already contain the
  `GrantedAscendancyNode` LIST mods parsed from Forbidden Flesh/Flame jewels.
  The parser must store the composite `{ side, name }` value, not the current
  stub `Number(0.0)`.
- **SETUP-07** (anointments): The same `env.allocNodes` / `env.grantedPassives`
  bookkeeping and the same passive-merge call (lines 1260–1265) are shared.
  SETUP-15 logic runs immediately after SETUP-07's loop (line 1241 ends,
  line 1242 begins).
- **`ascendancy_map`** in `PassiveTree`: needed to look up the ascendancy
  notable by lowercase name (built as part of SETUP-07's tree infrastructure
  work, item 3 in SETUP-07's "What Needs to Change" list).

---

## Lua Source

**File:** `third-party/PathOfBuilding/src/Modules/CalcSetup.lua`  
**Lines:** 1242–1258  
**Commit:** `454eff8c85d24356d9b051d596983745ed367476`

**Supporting parser rule:**  
**File:** `third-party/PathOfBuilding/src/Modules/ModParser.lua`  
**Line:** 5310

---

## Annotated Lua

### ModParser.lua — Pattern Registration (line 5310)

```lua
-- ModParser.lua line 5310
-- This is the single parser pattern that produces GrantedAscendancyNode mods.
-- It fires on item mod text like:
--   "Allocates Endless Hunger if you have the matching modifier on Forbidden Flesh"
--   "Allocates Endless Hunger if you have the matching modifier on Forbidden Flame"

["allocates (.+) if you have the matching modifiers? on forbidden (.+)"] =
    function(_, ascendancy, side)
        return { mod("GrantedAscendancyNode", "LIST", { side = side, name = ascendancy }) }
        --            ^^^^^^^^^^^^^^^^^^^^^^  ^^^^^^  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        --            mod name               type    value = Lua TABLE { side, name }
        --
        -- Capture groups (from the regex):
        --   ascendancy = cap(1) = the notable name, e.g. "endless hunger" (already
        --                         lowercased by PoB's mod-text normalisation pipeline)
        --   side       = cap(2) = "flesh" or "flame" (also lowercased)
        --
        -- The value is a Lua TABLE, not a scalar.  PoB's LIST mod system stores
        -- arbitrary Lua values; `mod("GrantedAscendancyNode", "LIST", tbl)` just
        -- shoves `tbl` into the list.  CalcSetup then reads `ascTbl.name` and
        -- `ascTbl.side` from that table (lines 1245-1246).
        --
        -- Rust issue: ModValue has no Table variant.
        -- Current stub: ModValue::Number(0.0) — both name and side are discarded.
        -- Fix: encode as ModValue::String("<side>:<name>"), e.g. "flesh:endless hunger".
    end,
```

**Note:** The regex `"modifiers? on forbidden (.+)"` matches both singular and
plural ("modifier" and "modifiers") and both jewels ("flesh", "flame"). The
captured `side` string is the full suffix after "forbidden ", so it is exactly
`"flesh"` or `"flame"`.

---

### CalcSetup.lua — Forbidden Flesh/Flame (lines 1242–1258)

```lua
-- CalcSetup.lua lines 1242-1258
-- Immediately follows the anointment block (SETUP-07, lines 1230-1240).
-- Both blocks share the same env.allocNodes / env.grantedPassives targets.

-- Add granted ascendancy node (e.g., Forbidden Flame/Flesh combo)
local matchedName = { }
--    ^^^^^^^^^^^
-- Accumulator table.  After iterating all GrantedAscendancyNode mods, entries
-- look like:  matchedName["endless hunger"] = { side = "flesh", matched = false/true }
--
-- Lua: { } creates an empty table (equivalent of HashMap::new()).
-- Rust: let mut matched: HashMap<String, MatchEntry> = HashMap::new();
--       where MatchEntry = struct { side: String, matched: bool }

for _, ascTbl in pairs(env.modDB:List(nil, "GrantedAscendancyNode")) do
--  ^   ^^^^^^
--  _       = loop index (discarded; `pairs` iterates arbitrary order)
--  ascTbl  = the Lua TABLE value stored by the parser: { side = "flesh"/"flame",
--                                                        name = "endless hunger" }
--
-- env.modDB:List(nil, "GrantedAscendancyNode")
--   = collect all mods named "GrantedAscendancyNode" from modDB with no skill cfg.
--   Returns a Lua list of the mod VALUES (i.e., the tables stored by the parser).
--
-- Rust: env.player.mod_db.list(None, "GrantedAscendancyNode")
--       returns Vec<&Mod>; each m.value should be ModValue::String("<side>:<name>").
--       Parse: let (side, name) = encoded.split_once(':').unwrap_or(("", ""));

    local name = ascTbl.name
    --            ^^^^^^^^^^
    -- Lowercase notable name string, e.g. "endless hunger".
    -- Rust: let name = &encoded_name_part;  (the part after the ':')

    if matchedName[name] and matchedName[name].side ~= ascTbl.side
        and matchedName[name].matched == false then
    --  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    -- Three conditions must ALL be true:
    --   1. We have already seen a jewel for this notable name (entry exists).
    --   2. The new jewel is the OPPOSITE side from the first  (side ~= = "not equal").
    --      Lua: ~= is Rust !=.
    --   3. The pair has not been matched yet (prevents double-allocation if somehow
    --      three jewels are equipped).
    --
    -- Rust:
    --   if let Some(entry) = matched.get_mut(name) {
    --       if entry.side != side && !entry.matched {

        matchedName[name].matched = true
        -- Mark as matched to prevent re-processing.
        -- Rust: entry.matched = true;

        local node = env.spec.tree.ascendancyMap[name] or build.latestTree.ascendancyMap[name]
        --            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        -- Look up the ascendancy notable node by lowercase name.
        --
        -- `env.spec.tree.ascendancyMap`:  the per-version tree's ascendancy map.
        --   Maps lowercase notable name → PassiveNode for all ascendancy notables.
        --   This is a SEPARATE map from notableMap (used by SETUP-07 anointments).
        --   Only ascendancy nodes (those with node.ascendancyName set in the tree data)
        --   appear in ascendancyMap.
        --
        -- `build.latestTree.ascendancyMap`: fallback to the newest tree version.
        --   Used when the build's spec tree version pre-dates a newly added ascendancy
        --   notable.  In practice this fallback is rarely needed.
        --
        -- Rust: let node_id = tree.ascendancy_map.get(name).copied()
        --                       .or_else(|| env.data.latest_tree().ascendancy_map.get(name).copied());
        --       (PassiveTree needs an `ascendancy_map: HashMap<String, u32>` field —
        --        see SETUP-07 "What Needs to Change" item 3.)

        if node and (not override.removeNodes or not override.removeNodes[node.id]) then
        --  ^^^^
        -- Lua nil check: `node` is nil if the name isn't found in either ascendancyMap.
        -- Rust: if let Some(node_id) = node_id { ... }
        --
        -- `override.removeNodes`: a set of node IDs suppressed by the radius jewel
        -- override system (SETUP-08). Not yet present in Rust.  For SETUP-15, treat
        -- as always nil/empty — i.e., never suppress.

            if env.itemModDB.conditions["ForbiddenFlesh"] == env.spec.curClassName
               and env.itemModDB.conditions["ForbiddenFlame"] == env.spec.curClassName then
            --  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
            -- Class-safety double-guard.
            --
            -- `env.itemModDB.conditions["ForbiddenFlesh"]`:
            --   This is a STRING condition set by the Forbidden Flesh jewel's item mod.
            --   The jewel text is something like "Forbidden Flesh (Berserker)" and the
            --   item parser sets conditions["ForbiddenFlesh"] = "Berserker".  Both jewels
            --   must resolve to the SAME class name.
            --
            -- `env.spec.curClassName`:
            --   The character's current ascendancy class name, e.g. "Berserker".
            --   In Rust: build.ascend_class_name (from Build struct).
            --
            -- Purpose: prevents a mismatched pair (Flesh=Berserker, Flame=Slayer) from
            -- granting a node.  Both must match the character's actual ascendancy class.
            --
            -- Rust gaps:
            --   (a) `env.itemModDB.conditions["ForbiddenFlesh"]` condition is not yet
            --       stored anywhere.  The item parser must set a STRING condition on the
            --       item ModDb when it parses a Forbidden jewel.  This requires a new
            --       ModValue::String condition or a dedicated field in the item parser.
            --   (b) The comparison `== env.spec.curClassName` maps to comparing the
            --       condition string to build.ascend_class_name.

                env.allocNodes[node.id] = node
                --  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
                -- Add the ascendancy node to the allocated set so its mods will be
                -- merged in lines 1260-1265.
                -- Note: unlike anointments (SETUP-07 line 1171), there is NO
                -- `env.spec.nodes[node.id] or node` fallback here — ascendancy nodes
                -- are never in `env.spec.nodes` (they are not on the main tree).
                --
                -- Rust: env.alloc_nodes.insert(node_id);

                env.grantedPassives[node.id] = true
                -- Track it as a granted passive (used by SETUP-08 radius jewels).
                -- Rust: env.granted_passives.insert(node_id);

                -- Note: NO `env.extraRadiusNodeList[node.id] = nil` here.
                -- Ascendancy nodes are never in the radius node list, so the cleanup
                -- step from SETUP-07 anointments is not needed.
            end
        end
    else
        matchedName[name] = { side = ascTbl.side, matched = false }
        --                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        -- First time we see this notable: record the side and mark unmatched.
        -- Rust: matched.insert(name.to_string(), MatchEntry { side: side.to_string(), matched: false });
    end
end
```

---

### CalcSetup.lua — Merge (lines 1260–1265, shared with SETUP-07)

```lua
-- CalcSetup.lua lines 1260-1265
-- Runs immediately after both the anointment loop AND the Forbidden Flesh/Flame loop.
-- Merges mods from ALL newly-allocated nodes (anointments + Forbidden jewels) into
-- env.modDB.

do
    local modList, explodeSources = calcs.buildModListForNodeList(env, env.allocNodes, true)
    env.modDB:AddList(modList)
    env.explodeSources = tableConcat(explodeSources, env.explodeSources)
end
```

In Rust, this corresponds to the existing `add_passive_mods` call in `init_env`.
However, `add_passive_mods` currently only covers `build.passive_spec.allocated_nodes`
(from the character's passive tree). Nodes added by SETUP-07/SETUP-15 must also have
their stats merged — see SETUP-07 "What Needs to Change" item 5.

---

## Existing Rust Code

### `crates/pob-calc/src/build/mod_parser_generated.rs`

**Lines 35855–35868 (pattern 1803):**

```rust
1803 => {
    let num_str = caps.get(1).map(|m| m.as_str()).unwrap_or("0");
    let num: f64 = num_str.parse().unwrap_or(0.0);
    vec![
        Mod {
            name: "GrantedAscendancyNode".to_string(),
            mod_type: ModType::List,
            value: ModValue::Number(0.0) /* TODO: { side = side, name = ascendancy } */,
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: source.clone(),
        },
    ]
}
```

**Status: completely stubbed.** The regex captures two groups:
- `caps.get(1)` = the ascendancy notable name (e.g. `"endless hunger"`)
- `caps.get(2)` = the jewel side (e.g. `"flesh"` or `"flame"`)

Neither is stored. `num_str` and `num` are dead code from copy-paste. The stub
discards both the name and side, making it impossible for CalcSetup to do the
pair-matching and node lookup.

**Fix required:** Store both as a `ModValue::String("<side>:<name>")` encoding:
```rust
let asc_name = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_lowercase();
let side     = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_lowercase();
// side is the full text after "forbidden " — will be "flesh" or "flame".
ModValue::String(format!("{}:{}", side, asc_name))
```

### `crates/pob-calc/src/passive_tree/mod.rs`

**`PassiveTree` struct (line 138):** Has `nodes: HashMap<u32, PassiveNode>` but
**no `ascendancy_map: HashMap<String, u32>`**. Looked-up ascendancy notables require
an O(n) scan of all nodes, filtering by `node_type == NodeType::Notable` and
`ascendancy_name.is_some()`. A dedicated map is needed for O(1) lookup matching
PoB's `ascendancyMap`.

**`PassiveNode` struct (line 95):** Has `ascendancy_name: Option<String>` and
`node_type: NodeType`. `NodeType::Notable` covers all notables including ascendancy
ones. The `ascendancy_name` field distinguishes ascendancy notables from main-tree
notables. This is sufficient to build the map.

### `crates/pob-calc/src/calc/setup.rs`

**`init_env` (line 47):** No call to any Forbidden Flesh/Flame processing. The
entire SETUP-15 block is absent.

**`add_passive_mods` (line 317):** Only iterates `connected_passive_nodes(build, tree)`
— the main tree's allocated nodes. Ascendancy nodes from Forbidden jewels are not
included even if they were added to `alloc_nodes`.

### `crates/pob-calc/src/build/types.rs`

**`Build` struct (line 4):** Has `ascend_class_name: String` (line 6), which is the
Rust equivalent of `env.spec.curClassName`. This field is available for the class
guard comparison.

### `crates/pob-calc/src/calc/env.rs`

No `alloc_nodes: HashSet<u32>` or `granted_passives: HashSet<u32>` fields exist.
Both are needed (see SETUP-07 for the full env.rs gap analysis).

---

## What Needs to Change

These changes are a strict subset of SETUP-07's "What Needs to Change" list plus the
mod-parser fix specific to the `GrantedAscendancyNode` pattern. **SETUP-15 should be
implemented in the same session as SETUP-07** to avoid doing the infrastructure work
twice.

1. **`mod_parser_generated.rs` — fix `GrantedAscendancyNode` value (pattern 1803):**
   Replace `ModValue::Number(0.0) /* TODO */` with:
   ```rust
   let asc_name = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_lowercase();
   let side     = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_lowercase();
   ModValue::String(format!("{}:{}", side, asc_name))
   ```
   Remove the dead `num_str` / `num` locals.

2. **`passive_tree/mod.rs` — add `ascendancy_map`:**
   (Shared with SETUP-07 item 3.) Add:
   ```rust
   pub ascendancy_map: HashMap<String, u32>,  // lowercase_name → node_id
   ```
   Populate in `PassiveTree::from_json`: for every node where
   `node_type == NodeType::Notable && ascendancy_name.is_some()`, insert
   `node.name.to_lowercase()` → `node.id` into `ascendancy_map`.

3. **`calc/setup.rs` — implement Forbidden Flesh/Flame logic:**
   Within the `apply_granted_passives` function introduced by SETUP-07, add the
   pair-matching loop after the anointment loop. Pseudocode:
   ```rust
   let mut matched: HashMap<String, (String, bool)> = HashMap::new();
   for m in player_mod_db.list(None, "GrantedAscendancyNode") {
       let ModValue::String(encoded) = &m.value else { continue };
       let Some((side, name)) = encoded.split_once(':') else { continue };
       if let Some(entry) = matched.get_mut(name) {
           if entry.0 != side && !entry.1 {
               entry.1 = true;
               let node_id = tree.ascendancy_map.get(name).copied()
                   .or_else(|| env.data.latest_tree().ascendancy_map.get(name).copied());
               if let Some(node_id) = node_id {
                   // Class guard: check ForbiddenFlesh and ForbiddenFlame conditions
                   // match build.ascend_class_name.  (Requires item parser to set
                   // STRING conditions — see item 4 below.)
                   let flesh_class = get_string_condition(&env.player.mod_db, "ForbiddenFlesh");
                   let flame_class = get_string_condition(&env.player.mod_db, "ForbiddenFlame");
                   if flesh_class.as_deref() == Some(&build.ascend_class_name)
                       && flame_class.as_deref() == Some(&build.ascend_class_name)
                   {
                       env.alloc_nodes.insert(node_id);
                       env.granted_passives.insert(node_id);
                   }
               }
           }
       } else {
           matched.insert(name.to_string(), (side.to_string(), false));
       }
   }
   ```

4. **Item parser — `ForbiddenFlesh`/`ForbiddenFlame` STRING conditions:**
   The Forbidden jewels have item text like:
   `"Allocates Endless Hunger if you have the matching modifier on Forbidden Flesh"`
   plus an implicit condition binding: the jewel itself sets
   `conditions["ForbiddenFlesh"] = curClassName` (see `env.itemModDB.conditions`
   usage at line 1250).

   This condition is set not by the mod parser but by the item's unique item
   handling code (the PoB unique item database encodes the class restriction).
   In Rust, this is part of the SETUP-16 special unique item handling.
   **For initial SETUP-15 implementation:** the class guard can be skipped (treat
   as always matching) since no oracle builds use Forbidden jewels. Add a
   `// TODO(SETUP-16): implement ForbiddenFlesh/ForbiddenFlame condition check`
   comment.

5. **`calc/env.rs` and `calc/setup.rs` — `alloc_nodes`, `granted_passives`:**
   (Shared with SETUP-07 items 4 and 6.) These fields must exist on `CalcEnv`
   before SETUP-15 logic can run.

---

## Notable Lua Patterns in This Chunk

| Lua Pattern | Rust Equivalent | Notes |
|---|---|---|
| `env.modDB:List(nil, "GrantedAscendancyNode")` | `env.player.mod_db.list(None, "GrantedAscendancyNode")` | Returns `Vec<&Mod>` |
| `ascTbl.side` | `side` from `encoded.split_once(':')` | Lua table field → encode in string |
| `ascTbl.name` | `name` from `encoded.split_once(':')` | Lua table field → encode in string |
| `matchedName[name]` | `matched.get(name)` | HashMap lookup |
| `matchedName[name].side ~= ascTbl.side` | `entry.side != side` | `~=` is Lua's `!=` |
| `matchedName[name].matched == false` | `!entry.matched` | Bool negation |
| `env.spec.tree.ascendancyMap[name]` | `tree.ascendancy_map.get(name)` | Separate from `notable_map` |
| `build.latestTree.ascendancyMap[name]` | `env.data.latest_tree().ascendancy_map.get(name)` | Fallback for old tree version |
| `env.itemModDB.conditions["ForbiddenFlesh"] == env.spec.curClassName` | _(condition lookup TBD)_ | Class-matching guard; safe to skip for now |
| `env.allocNodes[node.id] = node` | `env.alloc_nodes.insert(node_id)` | No spec-node fallback (ascendancy nodes are never in spec.nodes) |
| `env.grantedPassives[node.id] = true` | `env.granted_passives.insert(node_id)` | Track for SETUP-08 radius jewels |
