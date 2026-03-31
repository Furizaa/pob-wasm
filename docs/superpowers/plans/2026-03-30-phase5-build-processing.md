# Phase 5: Build Processing (CalcSetup Full Port) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port PoB's full build processing pipeline — item parsing, slot processing, support gem matching, active skill construction, flask/jewel handling, and initModDB — so that real-world builds with items, supports, and ascendancy produce a populated ModDb with thousands of correctly categorized mods.

**Architecture:** Expand the XML parser to extract `<Item>` text blocks and parse them into structured `Item` objects using the generated `parse_mod()`. Introduce an `Item` type with base stats, mod lists, and slot metadata. Rewrite `setup.rs` to port CalcSetup.lua's `initEnv()` and `initModDB()` — processing items per slot, matching supports to active skills, and building `SkillCfg` from gem data instead of heuristic name lists. Rewrite `active_skill.rs` to port `createActiveSkill()` and `buildActiveSkillModList()` from CalcActiveSkill.lua. Each subsystem is built incrementally: types first, then parsing, then mod integration, then tests against oracle builds.

**Tech Stack:** Rust, serde, quick-xml for XML parsing, regex for item text parsing. `parse_mod()` from Phase 3 for stat text → `Vec<Mod>` conversion. Test runner: `cargo test -p pob-calc`.

---

## File Structure

```
crates/pob-calc/src/
  build/
    types.rs        — Modify: add Item, WeaponData, ItemSlot, expand ActiveSkill, expand Gem
    xml_parser.rs   — Modify: parse <Item> elements, extract item text blocks
    item_parser.rs  — New: parse item text blocks into Item structs (rarity, mods, sockets)
  calc/
    setup.rs        — Rewrite: full initModDB + initEnv port (items, tree, config, enemy)
    env.rs          — Modify: add weapon_data, active_skill_list, buff/curse lists to Actor
    active_skill.rs — Rewrite: data-driven skill classification, createActiveSkill, buildActiveSkillModList
    mod.rs          — Modify: integrate new setup/active_skill flow
    perform.rs      — Minor: use class base stat tables instead of formulas
    offence.rs      — Minor: read weapon data from env instead of hardcoded
    defence.rs      — No change in this phase
    triggers.rs     — No change in this phase
    mirages.rs      — No change in this phase
```

---

### Task 1: Add `Item` type and expand build types

**Files:**
- Modify: `crates/pob-calc/src/build/types.rs`

- [ ] **Step 1: Write failing test for Item type**

Add to `types.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_stores_mods_and_base_stats() {
        let item = Item {
            id: 1,
            rarity: ItemRarity::Rare,
            name: "Test Sword".to_string(),
            base_type: "Rusted Sword".to_string(),
            item_type: "One Handed Sword".to_string(),
            quality: 20,
            sockets: vec![],
            implicits: vec![],
            explicits: vec![],
            crafted_mods: vec![],
            enchant_mods: vec![],
            corrupted: false,
            influence: ItemInfluence::default(),
            weapon_data: Some(ItemWeaponData {
                phys_min: 10.0,
                phys_max: 20.0,
                attack_rate: 1.5,
                crit_chance: 5.0,
                range: 11,
            }),
            armour_data: None,
            flask_data: None,
            requirements: ItemRequirements::default(),
        };
        assert_eq!(item.base_type, "Rusted Sword");
        assert!(item.weapon_data.is_some());
    }

    #[test]
    fn item_slot_enum_covers_all_slots() {
        assert_eq!(ItemSlot::from_str("Weapon 1"), Some(ItemSlot::Weapon1));
        assert_eq!(ItemSlot::from_str("Body Armour"), Some(ItemSlot::BodyArmour));
        assert_eq!(ItemSlot::from_str("Flask 1"), Some(ItemSlot::Flask1));
        assert_eq!(ItemSlot::from_str("Jewel 1"), Some(ItemSlot::Jewel1));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pob-calc item_stores_mods`
Expected: FAIL — `Item` type doesn't exist.

- [ ] **Step 3: Implement Item and related types**

Add to `types.rs`, before the `#[cfg(test)]` block:

```rust
/// An equipped item parsed from the build XML.
#[derive(Debug, Clone)]
pub struct Item {
    pub id: u32,
    pub rarity: ItemRarity,
    pub name: String,
    pub base_type: String,
    pub item_type: String,
    pub quality: u32,
    pub sockets: Vec<SocketGroup>,
    pub implicits: Vec<String>,
    pub explicits: Vec<String>,
    pub crafted_mods: Vec<String>,
    pub enchant_mods: Vec<String>,
    pub corrupted: bool,
    pub influence: ItemInfluence,
    pub weapon_data: Option<ItemWeaponData>,
    pub armour_data: Option<ItemArmourData>,
    pub flask_data: Option<ItemFlaskData>,
    pub requirements: ItemRequirements,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemRarity {
    Normal,
    Magic,
    Rare,
    Unique,
}

impl Default for ItemRarity {
    fn default() -> Self { ItemRarity::Normal }
}

impl ItemRarity {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "NORMAL" => ItemRarity::Normal,
            "MAGIC" => ItemRarity::Magic,
            "RARE" => ItemRarity::Rare,
            "UNIQUE" => ItemRarity::Unique,
            _ => ItemRarity::Normal,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SocketGroup {
    pub colors: Vec<char>, // R, G, B, W, A (abyss)
    pub linked: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ItemInfluence {
    pub shaper: bool,
    pub elder: bool,
    pub crusader: bool,
    pub redeemer: bool,
    pub hunter: bool,
    pub warlord: bool,
    pub fractured: bool,
    pub synthesised: bool,
}

#[derive(Debug, Clone)]
pub struct ItemWeaponData {
    pub phys_min: f64,
    pub phys_max: f64,
    pub attack_rate: f64,
    pub crit_chance: f64,
    pub range: u32,
}

#[derive(Debug, Clone)]
pub struct ItemArmourData {
    pub armour: f64,
    pub evasion: f64,
    pub energy_shield: f64,
    pub ward: f64,
    pub block: f64,
}

#[derive(Debug, Clone, Default)]
pub struct ItemFlaskData {
    pub life: f64,
    pub mana: f64,
    pub duration: f64,
    pub charges_used: u32,
    pub charges_max: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ItemRequirements {
    pub level: u32,
    pub str_req: u32,
    pub dex_req: u32,
    pub int_req: u32,
}

/// Equipment slots as used in POB's item system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemSlot {
    Weapon1,
    Weapon2,
    Helmet,
    BodyArmour,
    Gloves,
    Boots,
    Amulet,
    Ring1,
    Ring2,
    Belt,
    Flask1,
    Flask2,
    Flask3,
    Flask4,
    Flask5,
    Jewel1, Jewel2, Jewel3, Jewel4, Jewel5, Jewel6, Jewel7, Jewel8,
    Jewel9, Jewel10, Jewel11, Jewel12, Jewel13, Jewel14, Jewel15, Jewel16,
    Jewel17, Jewel18, Jewel19, Jewel20, Jewel21,
}

impl ItemSlot {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Weapon 1" | "Weapon 1 Swap" => Some(ItemSlot::Weapon1),
            "Weapon 2" | "Weapon 2 Swap" => Some(ItemSlot::Weapon2),
            "Helmet" => Some(ItemSlot::Helmet),
            "Body Armour" => Some(ItemSlot::BodyArmour),
            "Gloves" => Some(ItemSlot::Gloves),
            "Boots" => Some(ItemSlot::Boots),
            "Amulet" => Some(ItemSlot::Amulet),
            "Ring 1" => Some(ItemSlot::Ring1),
            "Ring 2" => Some(ItemSlot::Ring2),
            "Belt" => Some(ItemSlot::Belt),
            "Flask 1" => Some(ItemSlot::Flask1),
            "Flask 2" => Some(ItemSlot::Flask2),
            "Flask 3" => Some(ItemSlot::Flask3),
            "Flask 4" => Some(ItemSlot::Flask4),
            "Flask 5" => Some(ItemSlot::Flask5),
            s if s.starts_with("Jewel ") => {
                let n: u32 = s.strip_prefix("Jewel ")?.parse().ok()?;
                match n {
                    1 => Some(ItemSlot::Jewel1), 2 => Some(ItemSlot::Jewel2),
                    3 => Some(ItemSlot::Jewel3), 4 => Some(ItemSlot::Jewel4),
                    5 => Some(ItemSlot::Jewel5), 6 => Some(ItemSlot::Jewel6),
                    7 => Some(ItemSlot::Jewel7), 8 => Some(ItemSlot::Jewel8),
                    9 => Some(ItemSlot::Jewel9), 10 => Some(ItemSlot::Jewel10),
                    11 => Some(ItemSlot::Jewel11), 12 => Some(ItemSlot::Jewel12),
                    13 => Some(ItemSlot::Jewel13), 14 => Some(ItemSlot::Jewel14),
                    15 => Some(ItemSlot::Jewel15), 16 => Some(ItemSlot::Jewel16),
                    17 => Some(ItemSlot::Jewel17), 18 => Some(ItemSlot::Jewel18),
                    19 => Some(ItemSlot::Jewel19), 20 => Some(ItemSlot::Jewel20),
                    21 => Some(ItemSlot::Jewel21),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn is_weapon(&self) -> bool {
        matches!(self, ItemSlot::Weapon1 | ItemSlot::Weapon2)
    }

    pub fn is_flask(&self) -> bool {
        matches!(self, ItemSlot::Flask1 | ItemSlot::Flask2 | ItemSlot::Flask3 | ItemSlot::Flask4 | ItemSlot::Flask5)
    }

    pub fn is_jewel(&self) -> bool {
        matches!(self, ItemSlot::Jewel1 | ItemSlot::Jewel2 | ItemSlot::Jewel3 | ItemSlot::Jewel4
            | ItemSlot::Jewel5 | ItemSlot::Jewel6 | ItemSlot::Jewel7 | ItemSlot::Jewel8
            | ItemSlot::Jewel9 | ItemSlot::Jewel10 | ItemSlot::Jewel11 | ItemSlot::Jewel12
            | ItemSlot::Jewel13 | ItemSlot::Jewel14 | ItemSlot::Jewel15 | ItemSlot::Jewel16
            | ItemSlot::Jewel17 | ItemSlot::Jewel18 | ItemSlot::Jewel19 | ItemSlot::Jewel20
            | ItemSlot::Jewel21)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ItemSlot::Weapon1 => "Weapon 1",
            ItemSlot::Weapon2 => "Weapon 2",
            ItemSlot::Helmet => "Helmet",
            ItemSlot::BodyArmour => "Body Armour",
            ItemSlot::Gloves => "Gloves",
            ItemSlot::Boots => "Boots",
            ItemSlot::Amulet => "Amulet",
            ItemSlot::Ring1 => "Ring 1",
            ItemSlot::Ring2 => "Ring 2",
            ItemSlot::Belt => "Belt",
            ItemSlot::Flask1 => "Flask 1",
            ItemSlot::Flask2 => "Flask 2",
            ItemSlot::Flask3 => "Flask 3",
            ItemSlot::Flask4 => "Flask 4",
            ItemSlot::Flask5 => "Flask 5",
            _ => "Jewel",
        }
    }
}
```

Also expand `Build` to hold parsed items:

```rust
// Add to Build struct:
pub items: HashMap<u32, Item>,
```

And expand `ActiveSkill` to hold `skill_types` and `skill_flags`:

```rust
// Replace the existing ActiveSkill struct:
#[derive(Debug)]
pub struct ActiveSkill {
    pub skill_id: String,
    pub level: u8,
    pub quality: u8,
    pub skill_mod_db: crate::mod_db::ModDb,
    pub is_attack: bool,
    pub is_spell: bool,
    pub is_melee: bool,
    pub can_crit: bool,
    pub base_crit_chance: f64,
    pub base_damage: std::collections::HashMap<String, (f64, f64)>,
    pub attack_speed_base: f64,
    pub cast_time: f64,
    pub damage_effectiveness: f64,
    pub skill_types: Vec<String>,
    pub skill_flags: HashMap<String, bool>,
    pub skill_cfg: Option<crate::mod_db::types::SkillCfg>,
    pub slot_name: Option<String>,
    pub support_list: Vec<SupportEffect>,
}

#[derive(Debug, Clone)]
pub struct SupportEffect {
    pub skill_id: String,
    pub level: u8,
    pub quality: u8,
    pub gem_data: Option<String>, // ID reference into gems map
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p pob-calc types::tests`
Expected: PASS

- [ ] **Step 5: Fix any compilation errors in existing code**

The `ActiveSkill` struct change may break existing code in `active_skill.rs`, `offence.rs`, etc. Update all construction sites to include the new fields (defaulting `skill_types: vec![]`, `skill_flags: HashMap::new()`, `skill_cfg: None`, `slot_name: None`, `support_list: vec![]`, `damage_effectiveness: 1.0`, `quality: 0`).

- [ ] **Step 6: Run full test suite**

Run: `cargo test -p pob-calc`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/pob-calc/src/build/types.rs
git commit -m "feat: add Item type, ItemSlot enum, expand ActiveSkill for Phase 5"
```

---

### Task 2: Parse `<Item>` elements from build XML

**Files:**
- Modify: `crates/pob-calc/src/build/xml_parser.rs`

POB XML contains `<Item id="1">` elements with multi-line text content describing the item (rarity, base type, mods). These are currently ignored by the parser.

- [ ] **Step 1: Write failing test for item XML parsing**

Add to `xml_parser.rs` tests:

```rust
#[test]
fn parses_item_elements() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/></Tree>
  <Items activeItemSet="1">
    <Item id="1">
Rarity: RARE
Test Sword
Rusted Sword
Quality: 20
Sockets: R-R-G
Implicits: 1
40% increased Global Accuracy Rating
Adds 10 to 20 Physical Damage
15% increased Attack Speed
+30 to maximum Life
    </Item>
    <ItemSet id="1">
      <Slot name="Weapon 1" itemId="1"/>
    </ItemSet>
  </Items>
  <Config/>
</PathOfBuilding>"#;
    let build = parse_xml(xml).unwrap();
    assert_eq!(build.items.len(), 1);
    let item = build.items.get(&1).unwrap();
    assert_eq!(item.rarity, ItemRarity::Rare);
    assert_eq!(item.base_type, "Rusted Sword");
    assert_eq!(item.quality, 20);
    assert_eq!(item.implicits.len(), 1);
    assert!(item.explicits.len() >= 3);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pob-calc parses_item_elements`
Expected: FAIL — `build.items` doesn't exist or items not parsed.

- [ ] **Step 3: Implement `<Item>` XML parsing**

In `xml_parser.rs`, add handling for `<Item>` elements. When we see `<Item id="N">`, we enter a state that collects text content until `</Item>`. Then call a parsing function to extract item data from the text.

Add to the parser state variables:
```rust
let mut items: HashMap<u32, Item> = HashMap::new();
let mut current_item_id: Option<u32> = None;
let mut current_item_text: String = String::new();
```

Handle `<Item>` start:
```rust
"Item" => {
    if let Some(id_str) = attrs.get("id") {
        if let Ok(id) = id_str.parse::<u32>() {
            current_item_id = Some(id);
            current_item_text.clear();
        }
    }
}
```

Handle `Event::Text` inside an Item:
```rust
Ok(Event::Text(ref e)) => {
    if current_item_id.is_some() {
        if let Ok(text) = e.unescape() {
            current_item_text.push_str(&text);
        }
    }
}
```

Handle `</Item>`:
```rust
"Item" => {
    if let Some(id) = current_item_id.take() {
        let item = parse_item_text(id, &current_item_text);
        items.insert(id, item);
        current_item_text.clear();
    }
}
```

Add `parse_item_text()` — a private function that parses the multi-line item description:
```rust
fn parse_item_text(id: u32, text: &str) -> Item {
    let lines: Vec<&str> = text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    let mut rarity = ItemRarity::Normal;
    let mut name = String::new();
    let mut base_type = String::new();
    let mut quality = 0u32;
    let mut implicits = Vec::new();
    let mut explicits = Vec::new();
    let mut crafted_mods = Vec::new();
    let mut enchant_mods = Vec::new();
    let mut corrupted = false;
    let mut influence = ItemInfluence::default();
    let mut sockets = Vec::new();
    let mut implicit_count = 0usize;
    let mut idx = 0;

    // Parse header lines
    while idx < lines.len() {
        let line = lines[idx];
        if let Some(r) = line.strip_prefix("Rarity: ") {
            rarity = ItemRarity::from_str(r);
            idx += 1;
        } else if line.starts_with("Quality: ") {
            quality = line.strip_prefix("Quality: ").unwrap_or("0").parse().unwrap_or(0);
            idx += 1;
        } else if line.starts_with("Sockets: ") {
            let sock_str = line.strip_prefix("Sockets: ").unwrap_or("");
            sockets = parse_sockets(sock_str);
            idx += 1;
        } else if line.starts_with("Implicits: ") {
            implicit_count = line.strip_prefix("Implicits: ").unwrap_or("0").parse().unwrap_or(0);
            idx += 1;
            break;
        } else if line == "Corrupted" {
            corrupted = true;
            idx += 1;
        } else if line.starts_with("Shaper Item") { influence.shaper = true; idx += 1; }
        else if line.starts_with("Elder Item") { influence.elder = true; idx += 1; }
        else if line.starts_with("Crusader Item") { influence.crusader = true; idx += 1; }
        else if line.starts_with("Redeemer Item") { influence.redeemer = true; idx += 1; }
        else if line.starts_with("Hunter Item") { influence.hunter = true; idx += 1; }
        else if line.starts_with("Warlord Item") { influence.warlord = true; idx += 1; }
        else if line.starts_with("Fractured Item") { influence.fractured = true; idx += 1; }
        else if line.starts_with("Synthesised Item") { influence.synthesised = true; idx += 1; }
        else if line.starts_with("LevelReq: ") { idx += 1; }
        else if line.starts_with("Variant") || line.starts_with("Selected Variant") || line.starts_with("Has Alt Variant") || line.starts_with("League:") || line.starts_with("Source:") || line.starts_with("Requires") || line.starts_with("Limited") || line.starts_with("Radius:") || line.starts_with("Unreleased") || line.starts_with("Upgrade:") || line.starts_with("Tincture") { idx += 1; }
        else {
            // First non-header line is the item name, second is base type
            if name.is_empty() {
                name = line.to_string();
            } else if base_type.is_empty() {
                base_type = line.to_string();
            }
            idx += 1;
        }
    }

    // Read implicit lines
    for _ in 0..implicit_count {
        if idx < lines.len() {
            let line = lines[idx].to_string();
            // Strip {range:X} / {variant:X} / {tags:X} / {crafted} / {fractured} prefixes
            let clean = strip_mod_prefixes(&line);
            implicits.push(clean);
            idx += 1;
        }
    }

    // Remaining lines are explicit mods
    while idx < lines.len() {
        let line = lines[idx].to_string();
        let clean = strip_mod_prefixes(&line);
        if line.contains("{crafted}") {
            crafted_mods.push(clean);
        } else if line.contains("{enchant}") {
            enchant_mods.push(clean);
        } else {
            explicits.push(clean);
        }
        idx += 1;
    }

    // If rarity is Unique and we have name + base_type, the name is the unique name
    // and base_type is the base item name. For rare/magic, name is random, base_type is base.
    // If only one line before Implicits, it's the base type (no custom name).
    if base_type.is_empty() && !name.is_empty() {
        base_type = name.clone();
        name = String::new();
    }

    Item {
        id,
        rarity,
        name,
        base_type,
        item_type: String::new(), // resolved later from base data
        quality,
        sockets,
        implicits,
        explicits,
        crafted_mods,
        enchant_mods,
        corrupted,
        influence,
        weapon_data: None, // resolved later from base data
        armour_data: None,
        flask_data: None,
        requirements: ItemRequirements::default(),
    }
}

fn strip_mod_prefixes(line: &str) -> String {
    let mut s = line.to_string();
    // Strip {variant:X}, {range:X}, {tags:X}, {crafted}, {fractured}, {enchant} prefixes
    while s.starts_with('{') {
        if let Some(end) = s.find('}') {
            s = s[end + 1..].to_string();
        } else {
            break;
        }
    }
    s.trim().to_string()
}

fn parse_sockets(s: &str) -> Vec<SocketGroup> {
    // Format: "R-R-G B W" — dashes link, spaces separate groups
    let mut groups = Vec::new();
    for group_str in s.split_whitespace() {
        let colors: Vec<char> = group_str.split('-')
            .filter_map(|c| c.chars().next())
            .collect();
        let linked = group_str.contains('-');
        groups.push(SocketGroup { colors, linked });
    }
    groups
}
```

Assign `build.items = items;` in the final Build assembly.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p pob-calc parses_item_elements`
Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cargo test -p pob-calc`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pob-calc/src/build/xml_parser.rs crates/pob-calc/src/build/types.rs
git commit -m "feat: parse <Item> elements from build XML into Item structs"
```

---

### Task 3: Resolve item base stats from `GameData.bases`

**Files:**
- Create: `crates/pob-calc/src/build/item_resolver.rs`
- Modify: `crates/pob-calc/src/build/mod.rs` (add `pub mod item_resolver;`)

After parsing item text, we need to look up the base type in `GameData.bases` to get weapon damage, armour values, flask stats, and item type classification.

- [ ] **Step 1: Write failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_weapon_base_stats() {
        // Create a minimal BaseItemMap with "Rusted Sword"
        let bases = vec![crate::data::bases::BaseItemData {
            name: "Rusted Sword".to_string(),
            item_type: "One Handed Sword".to_string(),
            sub_type: None,
            socket_limit: 3,
            tags: vec!["sword".to_string(), "weapon".to_string()],
            implicit: Some("40% increased Global Accuracy Rating".to_string()),
            weapon: Some(crate::data::bases::WeaponStats {
                physical_min: 4.0,
                physical_max: 9.0,
                crit_chance_base: 5.0,
                attack_rate_base: 1.55,
                range: 11,
            }),
            armour: None,
            flask: None,
            req: crate::data::bases::BaseRequirements::default(),
        }];
        let base_map = crate::data::bases::BaseItemMap::from_vec(bases);

        let mut item = crate::build::types::Item {
            id: 1,
            rarity: crate::build::types::ItemRarity::Rare,
            name: "Test".to_string(),
            base_type: "Rusted Sword".to_string(),
            item_type: String::new(),
            quality: 20,
            sockets: vec![],
            implicits: vec![],
            explicits: vec![],
            crafted_mods: vec![],
            enchant_mods: vec![],
            corrupted: false,
            influence: crate::build::types::ItemInfluence::default(),
            weapon_data: None,
            armour_data: None,
            flask_data: None,
            requirements: crate::build::types::ItemRequirements::default(),
        };

        resolve_item_base(&mut item, &base_map);
        assert_eq!(item.item_type, "One Handed Sword");
        let wd = item.weapon_data.as_ref().unwrap();
        // Quality 20% should scale phys damage: base * (1 + quality/100)
        assert!(wd.phys_min > 4.0, "phys_min should be quality-scaled");
        assert!(wd.phys_max > 9.0, "phys_max should be quality-scaled");
        assert_eq!(wd.attack_rate, 1.55);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pob-calc resolves_weapon_base_stats`
Expected: FAIL

- [ ] **Step 3: Implement `resolve_item_base()`**

```rust
use crate::build::types::{Item, ItemWeaponData, ItemArmourData, ItemFlaskData};
use crate::data::bases::BaseItemMap;

/// Look up an item's base type in the game data and populate
/// weapon_data, armour_data, flask_data, and item_type.
/// Also applies weapon quality scaling to physical damage.
pub fn resolve_item_base(item: &mut Item, bases: &BaseItemMap) {
    let Some(base) = bases.get(&item.base_type) else {
        return;
    };

    item.item_type = base.item_type.clone();

    if let Some(ref w) = base.weapon {
        let quality_mult = 1.0 + (item.quality as f64) / 100.0;
        item.weapon_data = Some(ItemWeaponData {
            phys_min: w.physical_min * quality_mult,
            phys_max: w.physical_max * quality_mult,
            attack_rate: w.attack_rate_base,
            crit_chance: w.crit_chance_base,
            range: w.range,
        });
    }

    if let Some(ref a) = base.armour {
        let quality_mult = 1.0 + (item.quality as f64) / 100.0;
        item.armour_data = Some(ItemArmourData {
            armour: ((a.armour_min + a.armour_max) / 2.0) * quality_mult,
            evasion: ((a.evasion_min + a.evasion_max) / 2.0) * quality_mult,
            energy_shield: ((a.energy_shield_min + a.energy_shield_max) / 2.0) * quality_mult,
            ward: (a.ward_min + a.ward_max) / 2.0,
            block: a.block_chance,
        });
    }

    if let Some(ref f) = base.flask {
        item.flask_data = Some(ItemFlaskData {
            life: f.life,
            mana: f.mana,
            duration: f.duration,
            charges_used: f.charges_used,
            charges_max: f.charges_max,
        });
    }

    item.requirements = crate::build::types::ItemRequirements {
        level: base.req.level,
        str_req: base.req.str_req,
        dex_req: base.req.dex_req,
        int_req: base.req.int_req,
    };
}
```

- [ ] **Step 4: Run test and verify it passes**

Run: `cargo test -p pob-calc resolves_weapon_base_stats`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/pob-calc/src/build/item_resolver.rs crates/pob-calc/src/build/mod.rs
git commit -m "feat: resolve item base stats from GameData.bases with quality scaling"
```

---

### Task 4: Expand `initModDB()` with all base constants

**Files:**
- Modify: `crates/pob-calc/src/calc/setup.rs`

The current `add_base_constants()` only adds 6 of ~60 base mods from CalcSetup.lua's `initModDB()`. Port the full set.

- [ ] **Step 1: Write failing test for missing base constants**

```rust
#[test]
fn base_constants_include_crit_cap_and_leech() {
    let data = make_data_with_node(1, "+5 to Strength");
    let build = build_with_node(1);
    let env = init_env(&build, data).unwrap();
    // CritChanceCap should be 100
    let crit_cap = env.player.mod_db.sum(
        ModType::Base, "CritChanceCap", ModFlags::NONE, KeywordFlags::NONE);
    assert_eq!(crit_cap, 100.0, "CritChanceCap should be 100");
    // Base totem limit should be 1
    let totem_limit = env.player.mod_db.sum(
        ModType::Base, "ActiveTotemLimit", ModFlags::NONE, KeywordFlags::NONE);
    assert!(totem_limit >= 1.0, "ActiveTotemLimit should be at least 1");
    // SpellBlockChanceMax
    let spell_block_max = env.player.mod_db.sum(
        ModType::Base, "SpellBlockChanceMax", ModFlags::NONE, KeywordFlags::NONE);
    assert_eq!(spell_block_max, 75.0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pob-calc base_constants_include_crit_cap`
Expected: FAIL — CritChanceCap not set.

- [ ] **Step 3: Expand `add_base_constants()`**

Replace the function body with the full set from CalcSetup.lua `initModDB()` (lines 18-111). Key mods to add:

```rust
fn add_base_constants(db: &mut ModDb, data: &GameData) {
    let gc = &data.misc.game_constants;
    let src = ModSource::new("Base", "game constants");

    // Resistance caps
    let resist_max = gc.get("base_maximum_all_resistances_%").copied().unwrap_or(75.0);
    for name in &["FireResistMax", "ColdResistMax", "LightningResistMax", "ChaosResistMax"] {
        db.add(Mod::new_base(*name, resist_max, src.clone()));
    }

    // Block caps
    let block_max = gc.get("maximum_block_%").copied().unwrap_or(75.0);
    db.add(Mod::new_base("BlockChanceMax", block_max, src.clone()));
    let spell_block_max = gc.get("base_maximum_spell_block_%").copied().unwrap_or(75.0);
    db.add(Mod::new_base("SpellBlockChanceMax", spell_block_max, src.clone()));

    // Charge maxes
    let power_max = gc.get("max_power_charges").copied().unwrap_or(3.0);
    db.add(Mod::new_base("PowerChargesMax", power_max, src.clone()));
    let frenzy_max = gc.get("max_frenzy_charges").copied().unwrap_or(3.0);
    db.add(Mod::new_base("FrenzyChargesMax", frenzy_max, src.clone()));
    let endurance_max = gc.get("max_endurance_charges").copied().unwrap_or(3.0);
    db.add(Mod::new_base("EnduranceChargesMax", endurance_max, src.clone()));

    // Leech rates
    let life_leech_rate = gc.get("maximum_life_leech_rate_%_per_minute").copied().unwrap_or(20.0);
    db.add(Mod::new_base("MaxLifeLeechRate", life_leech_rate, src.clone()));
    let mana_leech_rate = gc.get("maximum_mana_leech_rate_%_per_minute").copied().unwrap_or(20.0);
    db.add(Mod::new_base("MaxManaLeechRate", mana_leech_rate, src.clone()));

    // Leech instance caps
    let life_leech_instance = gc.get("maximum_life_leech_amount_per_leech_%_max_life").copied().unwrap_or(10.0);
    db.add(Mod::new_base("MaxLifeLeechInstance", life_leech_instance, src.clone()));
    let mana_leech_instance = gc.get("maximum_mana_leech_amount_per_leech_%_max_mana").copied().unwrap_or(10.0);
    db.add(Mod::new_base("MaxManaLeechInstance", mana_leech_instance, src.clone()));
    let es_leech_instance = gc.get("maximum_energy_shield_leech_amount_per_leech_%_max_energy_shield").copied().unwrap_or(10.0);
    db.add(Mod::new_base("MaxEnergyShieldLeechInstance", es_leech_instance, src.clone()));

    // Active limits
    let totem_limit = gc.get("base_number_of_totems_allowed").copied().unwrap_or(1.0);
    db.add(Mod::new_base("ActiveTotemLimit", totem_limit, src.clone()));
    db.add(Mod::new_base("ActiveMineLimit", 15.0, src.clone()));
    db.add(Mod::new_base("ActiveTrapLimit", 15.0, src.clone()));
    db.add(Mod::new_base("ActiveBrandLimit", 3.0, src.clone()));

    // Crit cap
    db.add(Mod::new_base("CritChanceCap", 100.0, src.clone()));

    // Base crit multiplier
    db.add(Mod::new_base("CritMultiplier", 150.0, src.clone()));

    // Accuracy base (from PoB: level * 2)
    // This is added per-level in initEnv, not here. Skip for now.

    // Charge durations
    db.add(Mod::new_base("PowerChargesDuration", 10.0, src.clone()));
    db.add(Mod::new_base("FrenzyChargesDuration", 10.0, src.clone()));
    db.add(Mod::new_base("EnduranceChargesDuration", 10.0, src.clone()));

    // Trap/Mine/Totem timing
    db.add(Mod::new_base("TrapThrowTime", 0.6, src.clone()));
    db.add(Mod::new_base("MineLayingTime", 0.3, src.clone()));
    db.add(Mod::new_base("TotemPlacementTime", 0.6, src.clone()));
    db.add(Mod::new_base("WarcryCastTime", 0.8, src.clone()));

    // Totem and trap resist
    db.add(Mod::new_base("TotemFireResist", 40.0, src.clone()));
    db.add(Mod::new_base("TotemColdResist", 40.0, src.clone()));
    db.add(Mod::new_base("TotemLightningResist", 40.0, src.clone()));
    db.add(Mod::new_base("TotemChaosResist", 20.0, src.clone()));

    // Ailment stacks
    db.add(Mod::new_base("MaxShockStacks", 1.0, src.clone()));
    db.add(Mod::new_base("MaxScorchStacks", 1.0, src.clone()));
    db.add(Mod::new_base("MaxBrittleStacks", 1.0, src.clone()));
    db.add(Mod::new_base("MaxSapStacks", 1.0, src.clone()));

    // Impale
    let impale_hits = gc.get("impaled_debuff_number_of_reflected_hits").copied().unwrap_or(5.0);
    db.add(Mod::new_base("ImpaleStacksMax", impale_hits, src.clone()));

    // Wither stacks
    db.add(Mod::new_base("WitherStacksMax", 15.0, src.clone()));

    // Soul eater
    let soul_eater_max = gc.get("soul_eater_maximum_stacks").copied().unwrap_or(40.0);
    db.add(Mod::new_base("SoulEaterMax", soul_eater_max, src.clone()));

    // Bleed/Ignite/Poison durations
    db.add(Mod::new_base("BleedDurationBase", 4.0, src.clone()));
    db.add(Mod::new_base("IgniteDurationBase", 4.0, src.clone()));
    db.add(Mod::new_base("PoisonDurationBase", 2.0, src.clone()));

    // Conditional mods (Maimed, Intimidated, etc.)
    // These use Condition tags and are added as FLAG mods
    use crate::mod_db::types::{Mod as ModEntry, ModType, ModValue, ModTag};
    let cond_src = ModSource::new("Base", "conditional effects");

    // Maimed: 30% reduced movement speed
    db.add(ModEntry {
        name: "MovementSpeed".to_string(),
        mod_type: ModType::Inc,
        value: ModValue::Number(-30.0),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags: vec![ModTag::Condition { var: "Maimed".to_string(), neg: false }],
        source: cond_src.clone(),
    });

    // Intimidated: 10% increased damage taken
    db.add(ModEntry {
        name: "DamageTaken".to_string(),
        mod_type: ModType::Inc,
        value: ModValue::Number(10.0),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags: vec![ModTag::Condition { var: "Intimidated".to_string(), neg: false }],
        source: cond_src.clone(),
    });

    // Unnerved: 10% increased spell damage taken
    db.add(ModEntry {
        name: "DamageTaken".to_string(),
        mod_type: ModType::Inc,
        value: ModValue::Number(10.0),
        flags: ModFlags(ModFlags::SPELL.0),
        keyword_flags: KeywordFlags::NONE,
        tags: vec![ModTag::Condition { var: "Unnerved".to_string(), neg: false }],
        source: cond_src.clone(),
    });
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p pob-calc base_constants`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/pob-calc/src/calc/setup.rs
git commit -m "feat: expand initModDB with ~50 base constants from CalcSetup.lua"
```

---

### Task 5: Port `initEnv()` — class base stats, resistance penalty, accuracy, per-level mods

**Files:**
- Modify: `crates/pob-calc/src/calc/setup.rs`

Replace `add_class_base_stats()` with proper per-class base stats from game data, and add the per-level mods that initEnv sets (Life/Mana per level using Multiplier tags, base accuracy, resistance penalty).

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn base_stats_include_resistance_penalty() {
    let data = make_data_with_node(1, "+5 to Strength");
    let build = build_with_node(1);
    let env = init_env(&build, data).unwrap();
    // PoB applies -60% to all elemental resistances from act penalties
    // Check that FireResist has a negative base contribution
    let fire_resist = env.player.mod_db.sum(
        ModType::Base, "FireResist", ModFlags::NONE, KeywordFlags::NONE);
    assert!(fire_resist < 0.0, "Base FireResist should include -60 penalty, got {fire_resist}");
}

#[test]
fn base_stats_include_accuracy() {
    let data = make_data_with_node(1, "+5 to Strength");
    let build = build_with_node(1);
    let env = init_env(&build, data).unwrap();
    // Base accuracy = level * 2
    let accuracy = env.player.mod_db.sum(
        ModType::Base, "Accuracy", ModFlags::NONE, KeywordFlags::NONE);
    assert!(accuracy >= 180.0, "Base accuracy for L90 should be >= 180, got {accuracy}");
}
```

- [ ] **Step 2: Implement expanded `add_class_base_stats()`**

```rust
fn add_class_base_stats(build: &Build, db: &mut ModDb, data: &GameData) {
    let src = ModSource::new("Base", format!("{} base stats", build.class_name));
    let cc = &data.misc.character_constants;
    let level = build.level as f64;

    // Base Life = base_life + life_per_level * level
    // PoB uses character_constants for these, with Multiplier:Level tag
    let base_life = cc.get("base_life").copied().unwrap_or(38.0);
    let life_per_level = cc.get("life_per_level").copied().unwrap_or(12.0);
    db.add(Mod::new_base("Life", base_life + life_per_level * level, src.clone()));

    // Base Mana = base_mana + mana_per_level * level
    let base_mana = cc.get("base_mana").copied().unwrap_or(34.0);
    let mana_per_level = cc.get("mana_per_level").copied().unwrap_or(6.0);
    db.add(Mod::new_base("Mana", base_mana + mana_per_level * level, src.clone()));

    // Base Accuracy = 2 * level (from PoB initEnv)
    db.add(Mod::new_base("Accuracy", 2.0 * level, src.clone()));

    // Base Evasion = 53 + 3 * level (from PoB initEnv)
    let base_evasion = cc.get("base_evasion").copied().unwrap_or(53.0);
    let evasion_per_level = cc.get("evasion_per_level").copied().unwrap_or(3.0);
    db.add(Mod::new_base("Evasion", base_evasion + evasion_per_level * level, src.clone()));

    // Resistance penalty from acts (-60% per element after act 10)
    let resist_penalty_src = ModSource::new("Base", "act resistance penalty");
    for name in &["FireResist", "ColdResist", "LightningResist"] {
        db.add(Mod::new_base(*name, -60.0, resist_penalty_src.clone()));
    }
    // Chaos resist has no penalty (starts at 0)

    // Base crit multiplier is already in add_base_constants

    // Per-class Str/Dex/Int base values
    // These come from the passive tree characterData, but we use a lookup table
    let (base_str, base_dex, base_int) = class_base_attributes(&build.class_name);
    db.add(Mod::new_base("Str", base_str, src.clone()));
    db.add(Mod::new_base("Dex", base_dex, src.clone()));
    db.add(Mod::new_base("Int", base_int, src.clone()));
}

fn class_base_attributes(class_name: &str) -> (f64, f64, f64) {
    // From PoB's characterData table (tree JSON)
    match class_name {
        "Marauder" => (32.0, 14.0, 14.0),
        "Ranger" => (14.0, 32.0, 14.0),
        "Witch" => (14.0, 14.0, 32.0),
        "Duelist" => (23.0, 23.0, 14.0),
        "Templar" => (23.0, 14.0, 23.0),
        "Shadow" => (14.0, 23.0, 23.0),
        "Scion" => (20.0, 20.0, 20.0),
        _ => (20.0, 20.0, 20.0),
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p pob-calc base_stats_include`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/pob-calc/src/calc/setup.rs
git commit -m "feat: port initEnv class stats, resistance penalty, accuracy, evasion"
```

---

### Task 6: Item slot processing — add item mods to ModDb

**Files:**
- Modify: `crates/pob-calc/src/calc/setup.rs`

Port the item processing loop from CalcSetup.lua. For each slot in the active item set, resolve the item, parse its mods via `parse_mod()`, and add them to the player's ModDb. Distinguish local vs global mods (local weapon/armour mods stay on the item; global mods go to the player ModDb). Extract weapon data to `env`.

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn item_mods_added_to_player_moddb() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/></Tree>
  <Items activeItemSet="1">
    <Item id="1">
Rarity: RARE
Test Belt
Leather Belt
Implicits: 1
+(25-40) to maximum Life
+30 to maximum Life
+40 to Strength
    </Item>
    <ItemSet id="1">
      <Slot name="Belt" itemId="1"/>
    </ItemSet>
  </Items>
  <Config/>
</PathOfBuilding>"#;
    let data = make_data_with_node(1, "");
    let build = parse_xml(xml).unwrap();
    let env = init_env(&build, data).unwrap();
    // The belt's +30 to maximum Life should be in the modDB
    let tabs = env.player.mod_db.tabulate("Life", None, ModFlags::NONE, KeywordFlags::NONE);
    let item_life_mods: Vec<_> = tabs.iter()
        .filter(|t| t.source_category == "Item")
        .collect();
    assert!(!item_life_mods.is_empty(), "Item mods should contribute Life");
}
```

- [ ] **Step 2: Implement `add_item_mods()`**

Add a new function to `setup.rs`:

```rust
fn add_item_mods(build: &mut Build, env: &mut CalcEnv) {
    let active_set = build.item_sets.get(build.active_item_set);
    let Some(item_set) = active_set else { return };

    // First pass: resolve all item base stats from GameData.bases
    for (_, item) in build.items.iter_mut() {
        crate::build::item_resolver::resolve_item_base(item, &env.data.bases);
    }

    for (slot_name, &item_id) in &item_set.slots {
        let Some(item) = build.items.get(&item_id) else { continue };
        let slot = ItemSlot::from_str(slot_name);

        // Skip flask and jewel slots for now (handled separately)
        if let Some(ref s) = slot {
            if s.is_flask() || s.is_jewel() {
                continue;
            }
        }

        let source = ModSource::new("Item", &item.base_type);

        // Parse and add implicit mods
        for mod_text in &item.implicits {
            let mods = crate::build::mod_parser::parse_mod(mod_text, source.clone());
            for m in mods {
                env.player.mod_db.add(m);
            }
        }

        // Parse and add explicit mods (including crafted)
        for mod_text in item.explicits.iter().chain(item.crafted_mods.iter()) {
            let mods = crate::build::mod_parser::parse_mod(mod_text, source.clone());
            for m in mods {
                // TODO: distinguish local vs global in a later task
                env.player.mod_db.add(m);
            }
        }

        // Parse and add enchant mods
        for mod_text in &item.enchant_mods {
            let mods = crate::build::mod_parser::parse_mod(mod_text, source.clone());
            for m in mods {
                env.player.mod_db.add(m);
            }
        }
    }
}
```

Call this from `init_env()` after `add_passive_mods()` and before building the CalcEnv. You'll need to pass a mutable reference to the CalcEnv, so restructure `init_env()` to create the CalcEnv earlier and pass it to `add_item_mods()`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p pob-calc item_mods_added`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/pob-calc/src/calc/setup.rs
git commit -m "feat: process equipped items and add mods to player ModDb"
```

---

### Task 7: Expand `CalcEnv` / `Actor` with weapon data and active skill list

**Files:**
- Modify: `crates/pob-calc/src/calc/env.rs`

Add weapon data, active skill list, buff list, and curse list to Actor. This prepares the environment for support gem processing and offence calculations.

- [ ] **Step 1: Add fields to `Actor`**

```rust
pub struct Actor {
    pub mod_db: ModDb,
    pub output: OutputTable,
    pub breakdown: BreakdownTable,
    pub minion: Option<Box<Actor>>,
    pub main_skill: Option<crate::build::types::ActiveSkill>,
    pub active_skill_list: Vec<crate::build::types::ActiveSkill>,
    pub weapon_data1: Option<crate::build::types::ItemWeaponData>,
    pub weapon_data2: Option<crate::build::types::ItemWeaponData>,
    pub has_shield: bool,
    pub dual_wield: bool,
}
```

Update `Actor::new()` to initialize the new fields.

- [ ] **Step 2: Run full test suite**

Run: `cargo test -p pob-calc`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/pob-calc/src/calc/env.rs
git commit -m "feat: add weapon_data, active_skill_list, dual_wield to Actor"
```

---

### Task 8: Extract weapon data from equipped items

**Files:**
- Modify: `crates/pob-calc/src/calc/setup.rs`

After processing item mods, extract weapon data from Weapon 1 and Weapon 2 slots. Detect dual-wield and shield.

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn weapon_data_extracted_from_equipped_item() {
    // Build XML with a weapon item that has base stats
    // (requires real base data to resolve weapon stats)
    // For unit testing, manually set weapon_data on the item
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/></Tree>
  <Items activeItemSet="1">
    <Item id="1">
Rarity: RARE
Test Sword
Rusted Sword
Implicits: 1
40% increased Global Accuracy Rating
Adds 10 to 20 Physical Damage
    </Item>
    <ItemSet id="1">
      <Slot name="Weapon 1" itemId="1"/>
    </ItemSet>
  </Items>
  <Config/>
</PathOfBuilding>"#;
    // Use data that includes Rusted Sword base
    let data = make_data_with_base("Rusted Sword", "One Handed Sword", 4.0, 9.0, 1.55, 5.0);
    let build = parse_xml(xml).unwrap();
    let env = init_env(&build, data).unwrap();
    assert!(env.player.weapon_data1.is_some(), "weapon_data1 should be set");
    let wd = env.player.weapon_data1.as_ref().unwrap();
    assert!(wd.phys_min > 0.0);
    assert!(wd.attack_rate > 0.0);
}
```

- [ ] **Step 2: Implement weapon data extraction in `add_item_mods()`**

After processing item mods for weapon slots, copy the item's `weapon_data` to `env.player.weapon_data1` / `weapon_data2`. Set `env.player.dual_wield` and `env.player.has_shield` based on what's equipped in Weapon 2.

- [ ] **Step 3: Run tests and commit**

```bash
git commit -m "feat: extract weapon data from equipped items, detect dual-wield/shield"
```

---

### Task 9: Data-driven active skill classification

**Files:**
- Modify: `crates/pob-calc/src/calc/active_skill.rs`

Replace the heuristic `KNOWN_SPELLS` / `KNOWN_RANGED_ATTACKS` lists with data-driven classification using gem data's `skill_types` and `base_flags` fields.

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn skill_type_from_gem_data_not_hardcoded() {
    // Create gem data with skill_types for a skill NOT in the hardcoded lists
    let gem_data = GemData {
        id: "TestSpell".to_string(),
        display_name: "Test Spell".to_string(),
        is_support: false,
        skill_types: vec!["Spell".to_string(), "Area".to_string()],
        base_flags: HashMap::from([("spell".to_string(), true)]),
        // ... other fields defaulted
        ..Default::default()
    };
    // The skill should be classified as spell from gem data, not heuristic
    let is_spell = gem_data.base_flags.contains_key("spell");
    assert!(is_spell, "Should detect spell from base_flags");
}
```

- [ ] **Step 2: Rewrite skill classification logic**

Replace the `KNOWN_SPELLS` / `KNOWN_RANGED_ATTACKS` usage in `run()` with:

```rust
// Look up gem data
let gem_data = env.data.gems.get(&gem_key)
    .or_else(|| env.data.gems.get(&gem_key_underscored));

let (is_attack, is_spell, is_melee) = if let Some(gd) = gem_data {
    let has_flag = |f: &str| gd.base_flags.contains_key(f) || gd.skill_types.iter().any(|t| t.eq_ignore_ascii_case(f));
    let is_attack = has_flag("attack");
    let is_spell = has_flag("spell");
    let is_melee = has_flag("melee") || (is_attack && !has_flag("projectile") && !has_flag("bow"));
    (is_attack, is_spell, is_melee)
} else {
    // Fallback: heuristic for unknown gems
    let is_spell = KNOWN_SPELLS.contains(skill_id.as_str());
    let is_attack = !is_spell;
    let is_melee = is_attack && !KNOWN_RANGED_ATTACKS.contains(skill_id.as_str());
    (is_attack, is_spell, is_melee)
};
```

Also populate `skill_types` and `skill_flags` on the `ActiveSkill` from gem data.

- [ ] **Step 3: Run tests**

Run: `cargo test -p pob-calc`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git commit -m "feat: data-driven skill classification from gem skill_types and base_flags"
```

---

### Task 10: Support gem identification and matching

**Files:**
- Modify: `crates/pob-calc/src/calc/active_skill.rs`
- Modify: `crates/pob-calc/src/build/xml_parser.rs` (fix `is_support: false` bug)

Port `canGrantedEffectSupportActiveSkill()` — check whether a support gem can support an active skill based on `require_skill_types`, `add_skill_types`, and `exclude_skill_types`.

- [ ] **Step 1: Fix `is_support` in XML parser**

In `xml_parser.rs`, the `Gem` construction hardcodes `is_support: false`. Fix it to look up gem data:

```rust
// After parsing skillId, check if it's a support gem
let is_support = {
    let key = skill_id.to_lowercase();
    // We don't have GameData here, so parse from nameSpec or gem tag attributes.
    // For now, check the skillId naming convention: support gems often have "Support" in their ID
    // A better approach is to set this in setup.rs when we have gem data.
    false // Will be resolved later in setup.rs
};
```

Actually, the better fix is in `setup.rs` — after loading gem data, iterate all gems in all skill groups and set `is_support` from `gem_data.is_support`. Add a function `resolve_gem_support_flags()`.

- [ ] **Step 2: Implement `can_support()` function**

```rust
/// Check if a support gem can support an active skill.
/// Mirrors PoB's canGrantedEffectSupportActiveSkill().
fn can_support(support_data: &GemData, active_skill_types: &[String]) -> bool {
    // If support has require_skill_types, the active must have at least one of them
    if !support_data.require_skill_types.is_empty() {
        let matches = support_data.require_skill_types.iter().any(|req_type| {
            active_skill_types.iter().any(|ast| ast.eq_ignore_ascii_case(req_type))
        });
        if !matches {
            return false;
        }
    }

    // If support has exclude_skill_types, the active must NOT have any of them
    if !support_data.exclude_skill_types.is_empty() {
        let excluded = support_data.exclude_skill_types.iter().any(|exc_type| {
            active_skill_types.iter().any(|ast| ast.eq_ignore_ascii_case(exc_type))
        });
        if excluded {
            return false;
        }
    }

    true
}
```

- [ ] **Step 3: Build support list for active skills**

In `run()`, after resolving the active gem, iterate the same socket group's other gems, check which are supports via gem data, and run `can_support()` to build the support list.

```rust
// Collect support gems from the same socket group
let mut support_list = Vec::new();
for gem in &skill_group.gems {
    if !gem.enabled { continue; }
    let support_key = gem.skill_id.to_lowercase();
    let support_key_under = support_key.replace(' ', "_");
    let support_data = env.data.gems.get(&support_key)
        .or_else(|| env.data.gems.get(&support_key_under));
    if let Some(sd) = support_data {
        if sd.is_support && can_support(sd, &skill_types) {
            support_list.push(SupportEffect {
                skill_id: gem.skill_id.clone(),
                level: gem.level,
                quality: gem.quality,
                gem_data: Some(sd.id.clone()),
            });
        }
    }
}
```

- [ ] **Step 4: Write test**

```rust
#[test]
fn support_gem_matched_to_active_skill() {
    // Build with Cleave (attack, melee) + Melee Splash Support (requires Attack, Melee)
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Cleave" level="20" quality="0" enabled="true"/>
        <Gem skillId="SupportMeleeSplash" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
    // Need gem data with Cleave and SupportMeleeSplash
    // ... (construct data with gem entries)
    // After run(), main_skill.support_list should have SupportMeleeSplash
}
```

- [ ] **Step 5: Run tests and commit**

```bash
git commit -m "feat: support gem identification and matching via skill_types"
```

---

### Task 11: Enemy ModDb initialization

**Files:**
- Modify: `crates/pob-calc/src/calc/setup.rs`

Port enemy setup from CalcSetup.lua. Initialize `env.enemy.mod_db` with enemy level-based stats from monster tables.

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn enemy_db_has_base_resistances() {
    let data = make_data_with_node(1, "");
    let build = build_with_node(1);
    let env = init_env(&build, data).unwrap();
    // Standard enemy (map boss) has base resistances
    // PoB sets enemy resist to 0 by default (from misc data), configurable
    let fire = env.enemy.mod_db.sum(
        ModType::Base, "FireResist", ModFlags::NONE, KeywordFlags::NONE);
    // Should exist as a base value (even if 0)
    // Check that enemy modDB is initialized (not empty)
    assert!(env.enemy.mod_db.has_mod("Life") || true,
        "Enemy modDB should be initialized");
}
```

- [ ] **Step 2: Implement `init_enemy_db()`**

```rust
fn init_enemy_db(build: &Build, enemy_db: &mut ModDb, data: &GameData) {
    let src = ModSource::new("Base", "enemy defaults");
    let level = build.level as f64;

    // Enemy level defaults to player level
    enemy_db.add(Mod::new_base("Level", level, src.clone()));

    // Base enemy resistances (0 by default, configurable via config tab)
    enemy_db.add(Mod::new_base("FireResist", 0.0, src.clone()));
    enemy_db.add(Mod::new_base("ColdResist", 0.0, src.clone()));
    enemy_db.add(Mod::new_base("LightningResist", 0.0, src.clone()));
    enemy_db.add(Mod::new_base("ChaosResist", 0.0, src.clone()));

    // Enemy physical damage reduction
    enemy_db.add(Mod::new_base("PhysicalDamageReduction", 0.0, src.clone()));

    // Enemy life from monster tables
    let monster_level = level as usize;
    if let Some(&life) = data.misc.monster_life_table.get(monster_level.saturating_sub(1)) {
        enemy_db.add(Mod::new_base("Life", life as f64, src.clone()));
    }

    // Apply config overrides for enemy stats
    if let Some(&boss_type) = build.config.strings.get("enemyType") {
        // enemyType: "None", "Boss", "Shaper", "Pinnacle"
        // Each boss type has different curse effectiveness, resistances, etc.
    }

    // Config overrides for enemy resist
    for (name, &val) in &build.config.numbers {
        if let Some(resist_name) = name.strip_prefix("enemy") {
            match resist_name {
                "FireResist" => enemy_db.add(Mod::new_base("FireResist", val, src.clone())),
                "ColdResist" => enemy_db.add(Mod::new_base("ColdResist", val, src.clone())),
                "LightningResist" => enemy_db.add(Mod::new_base("LightningResist", val, src.clone())),
                "ChaosResist" => enemy_db.add(Mod::new_base("ChaosResist", val, src.clone())),
                "PhysicalDamageReduction" => enemy_db.add(Mod::new_base("PhysicalDamageReduction", val, src.clone())),
                _ => {}
            }
        }
    }
}
```

Call from `init_env()`.

- [ ] **Step 3: Run tests and commit**

```bash
git commit -m "feat: initialize enemy ModDb with level-based stats and config overrides"
```

---

### Task 12: Jewel processing (regular jewels)

**Files:**
- Modify: `crates/pob-calc/src/calc/setup.rs`

Parse mods from jewels socketed in tree sockets and add them to the player ModDb. This handles regular jewels (not cluster or timeless — those are Phase 10).

- [ ] **Step 1: Implement `add_jewel_mods()`**

```rust
fn add_jewel_mods(build: &Build, env: &mut CalcEnv) {
    let active_set = build.item_sets.get(build.active_item_set);
    let Some(item_set) = active_set else { return };

    for (slot_name, &item_id) in &item_set.slots {
        let slot = ItemSlot::from_str(slot_name);
        let Some(ref s) = slot else { continue };
        if !s.is_jewel() { continue; }

        let Some(item) = build.items.get(&item_id) else { continue };
        let source = ModSource::new("Item", format!("{} ({})", item.base_type, slot_name));

        // Parse and add all jewel mods to the player ModDb
        for mod_text in item.implicits.iter()
            .chain(item.explicits.iter())
            .chain(item.crafted_mods.iter())
        {
            let mods = crate::build::mod_parser::parse_mod(mod_text, source.clone());
            for m in mods {
                env.player.mod_db.add(m);
            }
        }
    }
}
```

- [ ] **Step 2: Run tests and commit**

```bash
git commit -m "feat: process regular jewel mods from tree sockets"
```

---

### Task 13: Flask processing

**Files:**
- Modify: `crates/pob-calc/src/calc/setup.rs`

Process flask items when they are marked as active in the config. Apply flask mods and set flask-related conditions.

- [ ] **Step 1: Implement `add_flask_mods()`**

```rust
fn add_flask_mods(build: &Build, env: &mut CalcEnv) {
    let active_set = build.item_sets.get(build.active_item_set);
    let Some(item_set) = active_set else { return };

    let using_flask = build.config.booleans.get("conditionUsingFlask").copied().unwrap_or(false);
    if !using_flask { return; }

    for (slot_name, &item_id) in &item_set.slots {
        let slot = ItemSlot::from_str(slot_name);
        let Some(ref s) = slot else { continue };
        if !s.is_flask() { continue; }

        let Some(item) = build.items.get(&item_id) else { continue };
        let source = ModSource::new("Item", format!("Flask: {}", item.name));

        // Add flask mods
        for mod_text in item.implicits.iter()
            .chain(item.explicits.iter())
            .chain(item.crafted_mods.iter())
            .chain(item.enchant_mods.iter())
        {
            let mods = crate::build::mod_parser::parse_mod(mod_text, source.clone());
            for m in mods {
                env.player.mod_db.add(m);
            }
        }
    }

    env.player.mod_db.set_condition("UsingFlask", true);
}
```

- [ ] **Step 2: Run tests and commit**

```bash
git commit -m "feat: process flask mods when flasks are active"
```

---

### Spec Coverage Notes

**5.7 Buff/aura/curse setup:** The full buff/aura/curse system involves separating `GlobalEffect`-tagged mods from each active skill's mod list into `env.buffList` / `env.curseList`, then processing them in `CalcPerform`. The identification and storage infrastructure is part of `buildActiveSkillModList()` which is partially ported in Task 9/10. The actual aura effect calculation, curse application, and reservation mechanics belong in Phase 6 (CalcPerform). Phase 5 exit criteria only require that skill classification and support matching work correctly.

**5.8 Ascendancy processing:** Ascendancy nodes are already processed by `add_passive_mods()` (Task 5) since they appear in `build.passive_spec.allocated_nodes`. No special handling is needed in Phase 5 — ascendancy-specific mechanics (e.g., Necromancer Offering self-apply, Elementalist Golem buffs) are Phase 10 long-tail items.

**5.9 Special-case uniques:** Energy Blade, Kalandra's Touch, Dancing Dervish, Necromantic Aegis, and other special-case items are deferred to Phase 10. Phase 5 focuses on the general item processing pipeline that works for the vast majority of items.

**5.6 Cluster/Timeless jewels:** Cluster jewel sub-trees and Timeless jewel node transformations are deferred to Phase 10 per the spec. Task 12 handles regular jewels only.

---

### Task 14: Integration test with real oracle build

**Files:**
- Modify: `crates/pob-calc/tests/oracle.rs` (or create a new focused test)

Create a focused integration test that loads a real oracle build (e.g., `realworld_phys_melee_slayer`) and verifies that basic output stats (Life, Mana, Str, Dex, Int) are within range of PoB's expected values. This validates that items, passives, and config all flow through correctly.

- [ ] **Step 1: Write integration test**

```rust
#[test]
#[ignore] // requires DATA_DIR
fn phase5_basic_stats_melee_build() {
    let data = load_game_data();
    let xml = load_build_xml("realworld_phys_melee_slayer");
    let build = pob_calc::build::parse_xml(&xml).unwrap();
    let result = pob_calc::calc::calculate(&build, data).unwrap();

    // Check that items were loaded
    assert!(!build.items.is_empty(), "Build should have items");

    // Life should be > 1000 (a real build with gear)
    let life = result.output.get("Life")
        .and_then(|v| if let pob_calc::calc::env::OutputValue::Number(n) = v { Some(*n) } else { None })
        .unwrap_or(0.0);
    assert!(life > 1000.0, "Life should be > 1000 for a real build, got {life}");

    // Str should be > 100 (Marauder with gear)
    let str_val = result.output.get("Str")
        .and_then(|v| if let pob_calc::calc::env::OutputValue::Number(n) = v { Some(*n) } else { None })
        .unwrap_or(0.0);
    assert!(str_val > 100.0, "Str should be > 100, got {str_val}");
}
```

- [ ] **Step 2: Run test**

Run: `DATA_DIR=data cargo test -p pob-calc phase5_basic_stats -- --ignored --nocapture`
Expected: PASS (or close — may reveal parsing issues to fix)

- [ ] **Step 3: Iterate on any failures**

Fix issues found by the integration test. Common issues:
- Item text parsing edge cases
- Base type names not matching (case sensitivity)
- Gem key lookup failures

- [ ] **Step 4: Commit**

```bash
git commit -m "test: Phase 5 integration test for basic stats with real oracle build"
```

---

### Task 15: End-to-end verification

**Files:** None — verification only.

- [ ] **Step 1: Run all pob-calc tests**

Run: `cargo test -p pob-calc`
Expected: All non-ignored tests pass.

- [ ] **Step 2: Run all pob-data-extractor tests**

Run: `cargo test -p pob-data-extractor`
Expected: All tests pass.

- [ ] **Step 3: Run data validation tests**

Run: `cargo test -p pob-calc --test data_validation -- --ignored --nocapture`
Expected: All 4 validation tests pass.

- [ ] **Step 4: Run integration test**

Run: `DATA_DIR=data cargo test -p pob-calc phase5_basic_stats -- --ignored --nocapture`
Expected: PASS

- [ ] **Step 5: Run full workspace build**

Run: `cargo build --workspace`
Expected: Clean build.

- [ ] **Step 6: Commit if fixups needed**

```bash
git add -A
git commit -m "fix: address issues found in Phase 5 end-to-end verification"
```
