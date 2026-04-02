# SETUP-12: Bandit & Pantheon Mods

## Output Fields

This chunk writes **no output fields directly**. Its effect is entirely in the
`ModDb`: it injects mods from the player's bandit reward choice and pantheon
god selections so that downstream chunks (DEF-01 onwards) see those mods.

Verified by checking that bandit-specific effects (e.g. Alira's +15 elemental
resistance, Oak's +40 life) and pantheon passive effects appear correctly in the
downstream output fields they affect.

## Dependencies

- SETUP-01 through SETUP-11 must be complete (player `modDB` must be fully
  populated before these mods are appended to it).

## Lua Source

**File:** `CalcSetup.lua`, lines 531–553  
**Also:** `PantheonTools.lua`, lines 1–19 (the `pantheon.applySoulMod` function)  
**Also:** `Data/Pantheons.lua` (data table for all pantheon gods)  
**Commit:** `454eff8c85d24356d9b051d596983745ed367476`

## Annotated Lua

### CalcSetup.lua lines 531–553 — Bandit and Pantheon block

```lua
		-- Add bandit mods
		-- env.configInput.bandit is a string: "Alira", "Kraityn", "Oak", or anything else
		-- (nil, "None", absent) means the player killed all bandits → +2 passive points.
		-- In Rust: build.bandit is a String field, parsed from <Build bandit="..."/>.
		if env.configInput.bandit == "Alira" then
			-- +15 to all elemental resistances.
			-- "ElementalResist" BASE mod — applies to FireResist, ColdResist, LightningResist
			-- via the "ElementalResist" alias that CalcDefence sums alongside per-element resists.
			-- In Rust: db.add(Mod::new_base("ElementalResist", 15.0, src))
			modDB:NewMod("ElementalResist", "BASE", 15, "Bandit")

		elseif env.configInput.bandit == "Kraityn" then
			-- 8% increased Movement Speed.
			-- In Rust: db.add(Mod { name: "MovementSpeed", mod_type: Inc, value: 8.0, src })
			modDB:NewMod("MovementSpeed", "INC", 8, "Bandit")

		elseif env.configInput.bandit == "Oak" then
			-- +40 to maximum Life.
			-- In Rust: db.add(Mod::new_base("Life", 40.0, src))
			modDB:NewMod("Life", "BASE", 40, "Bandit")

		else
			-- Kill all bandits (bandit == "None" / nil / any other string).
			-- Grants +2 extra passive skill points via the "ExtraPoints" BASE mod.
			-- "ExtraPoints" is summed in CalcPerform to increase the total passive point pool.
			-- In Rust: db.add(Mod::new_base("ExtraPoints", 1.0, src))
			-- NOTE: The Lua adds 1 here (not 2). PoB models "kill all" as 1 ExtraPoints mod
			-- from this Bandit source. The actual 2 extra points come from the base passive
			-- point system elsewhere. Confirm from PoB behaviour: this is 1 bonus point.
			modDB:NewMod("ExtraPoints", "BASE", 1, "Bandit")
		end

		-- Add Pantheon mods
		-- modLib.parseMod is the mod parser function (same as mod_parser.parse_mod in Rust).
		-- It is captured in a local here for readability and slight performance.
		local parser = modLib.parseMod
		-- Major Gods
		-- env.configInput.pantheonMajorGod is a string key into env.data.pantheons, or "None".
		-- In Rust: build.config.strings.get("pantheonMajorGod") (parsed from <Build pantheonMajorGod="..."/>)
		if env.configInput.pantheonMajorGod ~= "None" then
			-- env.data.pantheons[key] is a table from Data/Pantheons.lua:
			-- { isMajorGod = true, souls = { [1] = { name = "...", mods = { [1] = { line = "...", value = {...} } } } } }
			-- applySoulMod iterates all soul upgrades and all their mods for the selected god.
			local majorGod = env.data.pantheons[env.configInput.pantheonMajorGod]
			pantheon.applySoulMod(modDB, parser, majorGod)
		end
		-- Minor Gods
		if env.configInput.pantheonMinorGod ~= "None" then
			local minorGod = env.data.pantheons[env.configInput.pantheonMinorGod]
			pantheon.applySoulMod(modDB, parser, minorGod)
		end
```

### PantheonTools.lua lines 1–19 — `pantheon.applySoulMod`

```lua
-- Called with (modDB, parseMod_function, god_data_table).
-- god is the table from Data/Pantheons.lua, e.g. { isMajorGod = true, souls = { ... } }
function pantheon.applySoulMod(db, modParser, god)
    -- pairs() iterates all key-value entries of god.souls.
    -- Keys are integers 1..N (soul tiers), values are { name, mods } tables.
    -- NOTE: uses `pairs` not `ipairs` — order is unspecified, but in practice Lua
    -- integer-keyed tables iterate in integer order. All souls are applied (all tiers),
    -- not just the first.
    -- In Rust: for soul in god.souls.values() { ... }
    for _, soul in pairs(god.souls) do

        -- soul.mods is an integer-keyed table of { line, value } pairs.
        -- Each mod has a "line" string (the stat text) and a "value" table.
        -- pairs() is used again — order is unspecified but Lua integer keys iterate in order.
        for _, soulMod in pairs(soul.mods) do

            -- modParser(soulMod.line) parses the stat text into a list of Mod objects.
            -- In Lua this is modLib.parseMod(line) → returns (modList, extra).
            -- `extra` is truthy if there was leftover text that couldn't be parsed.
            -- In Rust: mod_parser::parse_mod(line, source) → Vec<Mod>
            local modList, extra = modParser(soulMod.line)

            -- Only add mods if parsing succeeded AND there was no leftover text.
            -- Silently discards unparseable lines.
            -- In Rust: if !mod_list.is_empty() { /* add them */ }
            -- (parse_mod in Rust doesn't return an "extra" flag; unparseable lines
            -- produce an empty Vec. If parse_mod has partial-parse semantics, mimic
            -- the `not extra` guard by checking that the full line was consumed.)
            if modList and not extra then

                -- Set the source of each mod to "Pantheon:<GodName>".
                -- god.souls[1].name is always the *primary* soul name (soul tier 1),
                -- regardless of which soul we're currently iterating.
                -- This means ALL mod sources for this god use the primary soul name.
                -- e.g. for Arakaali all mods get source "Pantheon:Soul of Arakaali"
                -- even for mods from Hybrid Widow (soul tier 2).
                -- In Rust: source = ModSource::new("Pantheon", god.souls[0].name)
                for _, mod in pairs(modList) do
                    local godName = god.souls[1].name
                    mod.source = "Pantheon:"..godName
                    -- `..` is Lua string concatenation. In Rust: format!("Pantheon:{}", god_name)
                end

                -- Add all parsed mods to the modDB.
                -- db:AddList(modList) is equivalent to calling db:AddMod(m) for each mod in modList.
                -- In Rust: for m in mod_list { player_db.add(m); }
                db:AddList(modList)
            end
        end
    end
end
```

### Key Lua Semantics for This Chunk

**Bandit comparison:** `env.configInput.bandit == "Alira"` is an exact string
equality check. The `else` branch fires for any non-matching value including
`nil`, `"None"`, `"Kill"`, etc. In Rust: `build.bandit.as_str() == "Alira"`.

**`modDB:NewMod(name, type, value, source)` 4-argument form:**  
The 4th argument is the source string. In Rust, this maps to:
```rust
db.add(Mod {
    name: "ElementalResist".to_string(),
    mod_type: ModType::Base,
    value: ModValue::Number(15.0),
    flags: ModFlags::NONE,
    keyword_flags: KeywordFlags::NONE,
    tags: vec![],
    source: ModSource::new("Bandit", build.bandit.as_str()),
    // or for the default case: ModSource::new("Bandit", "Kill All")
});
```

**`modDB:NewMod` vs `modDB:AddMod`:**  
`NewMod` creates a new Mod object from scratch (name + type + value + source).  
`AddMod` adds a pre-existing Mod object. Both ultimately insert a Mod into the DB.

**`god.souls[1].name` for source attribution:**  
Lua integer keys start at 1. `god.souls[1]` is the first element. This is always
the primary soul upgrade name (the base god, e.g. `"Soul of Arakaali"`). All
mods from any soul tier are stamped with this primary name as the source. In Rust:
```rust
let god_name = &god.souls[0].name;  // 0-based in Rust
let source = ModSource::new("Pantheon", god_name);
```

**`pairs` on integer-keyed tables:**  
The `souls` table uses integer keys `[1]`, `[2]`, etc. `pairs()` (not `ipairs()`)
is used. Both iterate all integer keys for this kind of table; `pairs` is slightly
less idiomatic but works correctly. In Rust this maps to `.values()` on a `Vec`
or `.iter()` on a `HashMap`. Since the Lua data is defined with consecutive integer
keys starting at 1, a `Vec` is the natural Rust equivalent.

**Silent discard of unparseable pantheon lines:**  
The `if modList and not extra then` guard means mods with complex stat text that
`modLib.parseMod` can't fully parse are silently dropped. In Rust, `parse_mod`
returns an empty `Vec` for completely unrecognised lines (silent skip). For partial
parses the `extra` guard is harder to replicate — but in practice all pantheon mod
lines in `Data/Pantheons.lua` are parseable by PoB's parser.

## Existing Rust Code

**Primary file:** `crates/pob-calc/src/calc/setup.rs`  
**Build types:** `crates/pob-calc/src/build/types.rs`  
**XML parser:** `crates/pob-calc/src/build/xml_parser.rs`

### What exists

- **`Build::bandit: String`** (`types.rs` line 8) — parsed from the `<Build
  bandit="..."/>` XML attribute in `xml_parser.rs` lines 55–58. Defaults to
  `"None"` if absent.
- The oracle XML builds already emit `bandit="None"` and `pantheonMajorGod`/
  `pantheonMinorGod` as attributes on `<Build>` (confirmed from
  `realworld_bow_deadeye.xml`: `pantheonMajorGod="Arakaali"`,
  `pantheonMinorGod="Shakari"`).
- `init_env()` (`setup.rs` lines 47–85) calls `add_base_constants`,
  `add_class_base_stats`, `add_passive_mods`, etc. — but **does not call any
  bandit or pantheon function**.
- `BuildConfig` (`types.rs` lines 421–425) has a `strings: HashMap<String,
  String>` map parsed from `<Input name="..." string="..."/>` nodes inside
  `<Config>`. `pantheonMajorGod` and `pantheonMinorGod` would be found here
  **if stored in `<Config>`**, but PoB actually stores them as attributes on
  `<Build>`, not as `<Input>` nodes.

### What's missing

1. **`Build::pantheon_major_god` and `Build::pantheon_minor_god` fields** —
   the `<Build>` XML attributes `pantheonMajorGod` and `pantheonMinorGod` are
   **not parsed** in `xml_parser.rs`. They exist on the XML element but are
   silently ignored. Both fields must be added to the `Build` struct and parsed.

2. **Bandit mod injection in `init_env()`** — `setup.rs` has no call to any
   function that adds bandit mods to the player's `ModDb`. The `Build::bandit`
   field is parsed but never acted upon during calculation setup.

3. **Pantheon mod injection in `init_env()`** — no pantheon support at all:
   no parsing of pantheon god names, no lookup of pantheon data, no mod injection.

4. **Pantheon data in `GameData`** — `GameData` does not currently contain the
   `pantheons` table from `Data/Pantheons.lua`. This data must be loaded and
   made available so the mod lines for each god can be parsed and injected.

### What's wrong / notable gaps

- **`ExtraPoints` from bandit kill:** The Lua adds `1` ExtraPoints (not `2`)
  from the "kill all" path. Verify: does PoB treat this as 1 extra point (in
  addition to a base grant of 1 elsewhere), or literally 1? From the oracle
  builds all use `bandit="None"` so the test impact is zero, but correctness
  matters for any future build with Alira/Kraityn/Oak.

- **All 30 oracle builds use `bandit="None"` and most use `pantheon="None"`:**
  The spec notes "All oracle builds use bandit=None and pantheon=None" but
  `realworld_bow_deadeye.xml` uses `pantheonMajorGod="Arakaali"` and
  `pantheonMinorGod="Shakari"`. This means at least one oracle build is
  affected by the missing pantheon support. Without pantheon injection, the
  deadeye build's mod effects (e.g. Arakaali's 10% reduced DoT taken) are
  absent from the calculation.

- **`Data/Pantheons.lua` is pure Lua data** — it uses the return-table pattern
  (`return { ["TheBrineKing"] = { ... } }`). This data needs to be either
  serialised to JSON as part of the game data pipeline, or parsed at runtime.
  The simplest approach is to add it to the game data JSON (`data.json`) as a
  `pantheons` object keyed by god name, with each value containing the
  soul mod lines. This mirrors how `misc.game_constants` is currently handled.

- **Source format difference:** PoB sets `mod.source = "Pantheon:Soul of Arakaali"`.
  In Rust, `ModSource` is `{ category: String, name: String }`. The equivalent
  is `ModSource { category: "Pantheon".into(), name: "Soul of Arakaali".into() }`.
  The category `"Pantheon"` is a new category not currently used in the Rust code.

## What Needs to Change

1. **Add `pantheon_major_god: String` and `pantheon_minor_god: String` to
   `Build`** (`types.rs`). Default to `"None"` when absent.

2. **Parse `pantheonMajorGod` and `pantheonMinorGod`** from the `<Build>`
   element in `xml_parser.rs` (alongside the existing `bandit` parse at lines
   55–58). These are XML attributes on `<Build>`, not `<Config><Input>` nodes.

3. **Implement `add_bandit_mods(build: &Build, db: &mut ModDb)`** in `setup.rs`:
   ```
   match build.bandit.as_str() {
       "Alira"  => db.add(Mod::new_base("ElementalResist", 15.0, ModSource::new("Bandit", "Alira")))
       "Kraityn"=> db.add(Mod { name: "MovementSpeed", mod_type: Inc, value: 8.0, ..., source: ModSource::new("Bandit", "Kraityn") })
       "Oak"    => db.add(Mod::new_base("Life", 40.0, ModSource::new("Bandit", "Oak")))
       _        => db.add(Mod::new_base("ExtraPoints", 1.0, ModSource::new("Bandit", "Kill All")))
   }
   ```

4. **Add pantheon data to `GameData`** — deserialise `Data/Pantheons.lua` into
   a JSON representation and load it as `data.pantheons: HashMap<String,
   PantheonGod>`. Each `PantheonGod` contains a `Vec<PantheonSoul>`, each soul
   has a `name: String` and `mods: Vec<PantheonMod>` where each mod has a
   `line: String`.

5. **Implement `add_pantheon_mods(build: &Build, db: &mut ModDb, data: &GameData)`**
   in `setup.rs`:
   ```
   for god_key in [&build.pantheon_major_god, &build.pantheon_minor_god] {
       if god_key == "None" { continue; }
       let Some(god) = data.pantheons.get(god_key) else { continue; };
       let god_name = &god.souls[0].name;  // primary soul name
       let source = ModSource::new("Pantheon", god_name);
       for soul in &god.souls {
           for soul_mod in &soul.mods {
               let mods = parse_mod(&soul_mod.line, source.clone());
               for m in mods { db.add(m); }
           }
       }
   }
   ```

6. **Call `add_bandit_mods` and `add_pantheon_mods`** from `init_env()` in
   `setup.rs`, after `add_base_constants` and `add_class_base_stats`, before
   `add_passive_mods`. This mirrors the position of lines 531–553 in CalcSetup.lua
   (after the base constants block, before the passive tree processing at line ~590).
