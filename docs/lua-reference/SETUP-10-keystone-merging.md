# SETUP-10: Keystone Merging (Non-Tree Grants)

## Output Fields

This chunk writes **no output fields directly**. Its effect is entirely in the
`ModDb`: it injects the mods from keystones granted by items (or other non-tree
sources) so that downstream chunks (PERF-01 onwards) see those mods.

Verified by running downstream chunks on builds that equip keystone-granting
uniques (e.g. Acuity, Conduit, Badge of the Brotherhood, Eye of Chayula,
Storm's Gift, Kaom's Roots, Brine Crown, The Taming, Hrimnor's Resolve, etc.)
and checking that their effects appear in the expected output fields.

## Dependencies

- SETUP-01 through SETUP-09 must be complete (ModDb must be fully populated
  before `mergeKeystones` is called, so all "Keystone" LIST mods are present).

## Lua Source

**Primary function:** `ModTools.lua`, lines 225–237  
**Call sites:**
| Site | File | Line | When |
|------|------|------|------|
| CalcSetup (initial node pass) | `CalcSetup.lua` | 655 | Once, after `buildModListForNodeList` for the alloc set |
| CalcPerform (start of perform) | `CalcPerform.lua` | 1097 | First thing in `calcs.perform()`, resets `keystonesAdded` |
| CalcPerform (after flasks/tinctures) | `CalcPerform.lua` | 1779 | Re-run to catch keystones added by flasks |
| CalcPerform (after aura application) | `CalcPerform.lua` | 3257 | Re-run to catch keystones added by buffs/auras |

**Commit:** `454eff8c85d24356d9b051d596983745ed367476`

## Annotated Lua

### ModTools.lua lines 225–237

```lua
-- Merge keystone modifiers
function modLib.mergeKeystones(env, modDB)
    -- env.keystonesAdded is a set (table used as a set) tracking which keystone
    -- names have already been merged in this calculation pass. Guards against
    -- double-applying a keystone if mergeKeystones() is called multiple times.
    -- Lua `or { }` means: if env.keystonesAdded is nil, initialise it to empty table.
    -- In Rust: env.keystones_added: HashSet<String> — initialise once, never reset
    --          except at the start of calcs.perform() (see CalcPerform.lua line 1096).
    env.keystonesAdded = env.keystonesAdded or { }

    -- modDB:Tabulate("LIST", nil, "Keystone") returns every LIST mod named "Keystone"
    -- that passes the nil (no-skill) cfg filter.
    -- Each `modObj` is a { value = <keystone name string>, mod = <the Mod object> } table.
    -- In Rust this is mod_db.list("Keystone", None, &empty_output), which returns Vec<&Mod>.
    -- The keystone name is modObj.value → in Rust mod.value is ModValue::String(name).
    for _, modObj in ipairs(modDB:Tabulate("LIST", nil, "Keystone")) do

        -- Skip if this keystone was already processed this pass.
        -- `env.keystonesAdded[modObj.value]` is either nil (not seen) or true (seen).
        -- In Rust: keystones_added.contains(&name)
        --
        -- Also skip if the keystone name is NOT found in the tree's keystoneMap.
        -- `env.spec.tree.keystoneMap` is a table keyed by keystone name → passive node.
        -- In Rust: data.tree.nodes.values().find(|n| n.name == name && n.node_type == NodeType::Keystone)
        if not env.keystonesAdded[modObj.value] and env.spec.tree.keystoneMap[modObj.value] then
            -- Mark this keystone as processed.
            env.keystonesAdded[modObj.value] = true

            -- Determine the source for the injected mods.
            -- `modObj.mod` is the Mod object that carries the "Keystone" LIST value.
            -- The source field is the item/passive that granted the keystone.
            --
            -- `fromTree` is TRUE when the mod did NOT come from the tree
            -- (confusingly named — it means "we should override the source to point
            -- to the granting item, not the passive tree").
            -- Logic: if modObj.mod.source exists AND does NOT contain "tree" (case-insensitive),
            --         then the keystone was granted by a non-tree source → override source.
            -- In Rust:
            --   let from_tree = mod.source.category.to_lowercase().contains("tree");
            --   let override_source = !from_tree;
            local fromTree = modObj.mod.source and not modObj.mod.source:lower():match("tree")

            -- Iterate all mods on the keystone node (from the tree's keystoneMap).
            -- `env.spec.tree.keystoneMap[modObj.value].modList` is the array of mods
            -- that the keystone passive node contributes.
            -- In Rust: the passive node's stats are parsed via parse_mod() and already
            --          stored. However for mergeKeystones we need the parsed Mod objects,
            --          not raw stat strings — so we parse them on-the-fly here.
            for _, mod in ipairs(env.spec.tree.keystoneMap[modObj.value].modList) do
                -- If fromTree (non-tree source), replace each keystone mod's source
                -- with the source of the granting item mod (modObj.mod.source).
                -- modLib.setSource() sets mod.source = source (in-place mutation of a copy).
                -- If NOT fromTree (the keystone is directly from the tree itself),
                -- use the mod as-is with its original tree source.
                modDB:AddMod(fromTree and modLib.setSource(mod, modObj.mod.source) or mod)
            end
        end
    end
end
```

### CalcSetup.lua line 655 (call site 1)

```lua
-- After buildModListForNodeList builds env.initialNodeModDB from allocated nodes,
-- mergeKeystones is called to inject non-tree keystone mods into initialNodeModDB.
-- This is SEPARATE from the player's main modDB — it affects radius jewel processing
-- (SETUP-08) and the initialNodeModDB used for certain conditional checks.
env.initialNodeModDB = calcs.buildModListForNodeList(env, env.allocNodes, true)
modLib.mergeKeystones(env, env.initialNodeModDB)   -- line 655
```

**Rust relevance:** The `env.initialNodeModDB` concept has no current equivalent in
the Rust `CalcEnv`. This call site is entangled with SETUP-08 (radius jewels) and
SETUP-05 (cluster jewels). For the purposes of this chunk, the critical call sites
are the three in `CalcPerform.lua`.

### CalcPerform.lua line 1096–1097 (call site 2, primary)

```lua
function calcs.perform(env, skipEHP)
    local modDB = env.modDB
    -- IMPORTANT: Reset keystonesAdded at the START of every perform pass.
    -- Without this reset, keystones merged on a previous perform() call would
    -- be skipped on subsequent calls (e.g. after buff recalculation).
    env.keystonesAdded = { }            -- line 1096 — reset the dedup set
    modLib.mergeKeystones(env, env.modDB)  -- line 1097 — primary merge
```

**Rust relevance:** `calcs.perform()` maps to `perform::run()` in Rust. The reset
of `keystonesAdded` and first call to `mergeKeystones` should happen at the top
of `perform::run()`, before `do_actor_attribs_conditions()`.

### CalcPerform.lua lines 1772–1779 (call site 3)

```lua
if env.mode_combat then
    -- Flask and tincture mods are applied (lines 1775-1776).
    -- Flasks can grant keystones (e.g. a unique flask with "grants Perfect Agony").
    -- Re-run mergeKeystones to pick up any newly-added "Keystone" LIST mods.
    mergeFlasks(env.flasks, false, true)
    mergeTinctures(env.tinctures)
    -- Merge keystones again to catch any that were added by flasks
    modLib.mergeKeystones(env, env.modDB)  -- line 1779
end
```

**Rust relevance:** This re-merge currently has no equivalent. When SETUP-13
(buff mode / `mode_combat`) is implemented, this call must be added after flask
mods are applied.

### CalcPerform.lua lines 3250–3257 (call site 4)

```lua
-- After applying aura/curse AffectedByAuraMod modifiers to actor ModDbs:
for _, value in ipairs(modDB:List(nil, "AffectedByAuraMod")) do
    for actor in pairs(affectedByAura) do
        actor.modDB:AddMod(value.mod)
    end
end
-- Merge keystones again to catch any that were added by buffs
modLib.mergeKeystones(env, env.modDB)  -- line 3257
```

**Rust relevance:** Buffs and auras can also grant keystones (e.g. a support gem
or aura that adds "Keystone" LIST mods). This third re-merge must be added after
`apply_buffs()` / `apply_curses()` / aura application is complete.

## Key Lua Semantics

### `modDB:Tabulate("LIST", nil, "Keystone")` vs `modDB:List`

In Lua, `Tabulate` returns a richer structure `{ value, mod }` where `value` is
the keystone name string and `mod` is the originating Mod object (needed for its
`.source`). In Rust, `mod_db.list("Keystone", None, &empty_output)` returns
`Vec<&Mod>` where each `Mod` has:
- `mod.value` = `ModValue::String(keystone_name)` — the keystone to look up
- `mod.source` = `ModSource { category, name }` — the granting source

### `env.spec.tree.keystoneMap`

In Lua, `keystoneMap` is a table indexed by keystone name (e.g. `"Vaal Pact"`,
`"Blood Magic"`) → passive node object. The node's `modList` is the pre-parsed
list of Mod objects.

In Rust, there is **no `keystoneMap`** on `PassiveTree`. The equivalent lookup is:
```rust
tree.nodes.values().find(|n| {
    n.node_type == NodeType::Keystone && n.name == keystone_name
})
```
The node's `stats: Vec<String>` must be run through `parse_mod()` to get the
actual Mod objects to inject. This is the same pattern used in `add_passive_mods()`.

### Source override logic

```lua
local fromTree = modObj.mod.source and not modObj.mod.source:lower():match("tree")
modDB:AddMod(fromTree and modLib.setSource(mod, modObj.mod.source) or mod)
```

- `modObj.mod.source` is a string like `"Item:Acuity"` or `"Passive:Blood Magic"`.
- The `:match("tree")` check is a Lua pattern match (substring search, case-insensitive
  after the `:lower()` call). `("tree"):match("tree")` returns a truthy string match.
- `fromTree` is **truthy** (non-nil) when:
  1. `modObj.mod.source` is non-nil AND
  2. it does NOT contain "tree" (case-insensitive).
  - So `fromTree` is true for `"Item:Acuity"` (item grant) but nil/false for
    `"Passive:Blood Magic"` (tree node grant).
- When `fromTree` is true, each keystone mod's `.source` is replaced with the
  granting item's source string before adding to modDB.
- When `fromTree` is false/nil, the keystone mod is added with its original
  passive-tree source unchanged.

In Rust, `ModSource` has `category` and `name` fields. The "from tree" check is:
```rust
let from_tree = mod_obj.source.category.to_lowercase().contains("tree")
    || mod_obj.source.name.to_lowercase().contains("tree");
let override_source = !from_tree;
```
When `override_source`, clone each keystone mod and replace its `.source` with
the granting mod's source.

### `env.keystonesAdded` dedup set

Lua `env.keystonesAdded = env.keystonesAdded or { }` is lazy init. The critical
reset is at line 1096: `env.keystonesAdded = { }`. This means each call to
`calcs.perform()` starts fresh. The `or { }` guard handles re-entrant calls
from the same perform() (the CalcSetup call site 1 uses a different modDB and
does NOT reset this field).

In Rust: a `HashSet<String>` field on `CalcEnv` named `keystones_added`.
Reset to empty at the start of `perform::run()`.

### `and/or` ternary pattern

```lua
modDB:AddMod(fromTree and modLib.setSource(mod, modObj.mod.source) or mod)
```

This is the Lua `a and b or c` ternary idiom. Safe here because `b`
(`modLib.setSource(...)`) always returns a non-nil mod table. In Rust:
```rust
let mod_to_add = if override_source {
    let mut m = keystone_mod.clone();
    m.source = granting_mod.source.clone();
    m
} else {
    keystone_mod.clone()
};
mod_db.add(mod_to_add);
```

## Existing Rust Code

**File:** `crates/pob-calc/src/calc/setup.rs`  
**Related:** `crates/pob-calc/src/calc/perform.rs`, `crates/pob-calc/src/passive_tree/mod.rs`

### What exists

- `PassiveTree` (`passive_tree/mod.rs`) has `NodeType::Keystone` and stores each
  node's `name` and `stats: Vec<String>`. Keystone nodes are identifiable by
  `node.node_type == NodeType::Keystone`.
- The mod parser generates `Mod { name: "Keystone", mod_type: List, value: ModValue::String("...") }`
  for item affixes that grant keystones (e.g. Acuity, Badge of the Brotherhood).
  These are correctly emitted in `mod_parser_generated.rs` (lines 8530–8538 for
  Perfect Agony, 15399–15407 for Elemental Equilibrium, etc.).
- `ModDb::list()` can retrieve these LIST mods by name "Keystone".
- `CalcEnv` has no `keystones_added` field.
- `perform::run()` has no call to `mergeKeystones`.
- `setup::init_env()` has no call to `mergeKeystones`.
- `PassiveTree` has no `keystone_map()` helper — but nodes can be searched by name.

### What's missing

1. **`CalcEnv::keystones_added: HashSet<String>`** — dedup set, must be added to
   `CalcEnv` struct and reset at the start of each `perform::run()` call.

2. **`merge_keystones(env: &mut CalcEnv)` function** in `setup.rs` (or a shared
   `calc_tools.rs`) — iterates `Keystone` LIST mods, looks up each keystone node
   by name in the tree, parses its stats, and adds the resulting mods to the
   player's `mod_db` with correct source attribution.

3. **Three call sites in `perform::run()`:**
   - At the very start (before `do_actor_attribs_conditions`): reset
     `env.keystones_added` and call `merge_keystones(env)`.
   - After flask mods are applied (when `mode_combat` is implemented in SETUP-13):
     call `merge_keystones(env)` again.
   - After aura/buff application: call `merge_keystones(env)` again.

4. **`PassiveTree::keystone_by_name()` helper** — convenience method returning
   `Option<&PassiveNode>` for a keystone with a given name. Not strictly required
   but avoids repeated O(n) scans of `tree.nodes.values()`.

### What's wrong / notable gaps

- Several Keystone LIST mods in `mod_parser_generated.rs` have **TODO stubs**:
  - Line 15734: `GroupProperty` mod with `TODO: { value = mod("Keystone", "LIST", "Vaal Pact") }`
  - Line 15747: `GroupProperty` mod with `TODO: { value = mod("Keystone", "LIST", "Immortal Ambition") }`
  - Line 25803: `TODO: { mod = mod("Keystone", "LIST", "Avatar of Fire") }`
  - Line 26144: `TODO: { mod = mod("Keystone", "LIST", "Avatar of Fire") }`
  
  These stubs emit `ModValue::Number(0.0)` instead of `ModValue::String("Vaal Pact")`,
  meaning those keystone grants will silently fail to resolve (the lookup
  `"Keystone" → "0"` finds nothing in the tree). These stubs must be fixed as part
  of this chunk or as a dependency.

- The `Condition` tag guards on some Keystone mods (e.g. `ModFlags::NONE /* { type = "Condition", var = "CritRecently" } */`
  at line 8534) are currently stubbed as `ModFlags::NONE`. This means the condition
  is not evaluated — the keystone is always granted regardless of the condition.
  This is a SETUP-04 (eval_mod stubs) issue, not specific to this chunk.

- The CalcSetup line-655 call site (merging keystones into `initialNodeModDB`)
  is entangled with SETUP-08 (radius jewels). It can be deferred until SETUP-08
  is implemented.

## What Needs to Change

1. **Add `keystones_added: HashSet<String>` to `CalcEnv`** (`env.rs`).
   Initialize to empty in `CalcEnv::new()`.

2. **Add `PassiveTree::keystone_by_name(&self, name: &str) -> Option<&PassiveNode>`**
   (`passive_tree/mod.rs`) — returns the first node with `node_type == Keystone`
   and `name == name`. Used by `merge_keystones`.

3. **Implement `pub fn merge_keystones(env: &mut CalcEnv)`** (in `setup.rs`):
   ```
   a. Collect all Mod objects from env.player.mod_db.list("Keystone", None, &empty_output)
   b. For each mod where mod.value == ModValue::String(keystone_name):
      - Skip if env.keystones_added contains keystone_name
      - Look up env.data.tree.keystone_by_name(keystone_name)
      - If not found, skip
      - Mark keystones_added.insert(keystone_name)
      - Determine override_source: source does NOT contain "tree" (case-insensitive)
      - For each stat in keystone_node.stats:
          parse_mod(stat, source) where source = if override_source { mod.source.clone() }
                                                  else { ModSource::new("Passive", keystone_name) }
          add each resulting Mod to env.player.mod_db
   ```

4. **Call `merge_keystones` in `perform::run()`** (`perform.rs`):
   ```rust
   // At the very start of perform::run(), before do_actor_attribs_conditions:
   env.keystones_added.clear();
   crate::calc::setup::merge_keystones(env);
   ```

5. **Add the post-flask and post-buff re-merge calls** once SETUP-13 (buff mode)
   and flask application are implemented.

6. **Fix TODO stubs in `mod_parser_generated.rs`** for Vaal Pact, Immortal Ambition,
   and Avatar of Fire keystone grants — replace `ModValue::Number(0.0)` with
   `ModValue::String("Vaal Pact")` etc.

7. **Defer CalcSetup line-655 call site** until SETUP-08 (radius jewels) is
   implemented, as it requires `env.initialNodeModDB` to exist.
