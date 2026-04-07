use super::types::*;
use crate::error::ParseError;
use quick_xml::events::Event;
use quick_xml::Reader;
use regex::Regex;
use std::collections::HashMap;

/// Compiled regex for parsing `masteryEffects` attribute values.
/// Matches `{nodeId,effectId}` pairs.
static RE_MASTERY_EFFECTS: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"\{(\d+),(\d+)\}").unwrap());

pub fn parse_xml(xml: &str) -> Result<Build, ParseError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut build: Option<Build> = None;
    let mut skill_sets: Vec<SkillSet> = Vec::new();
    let mut item_sets: Vec<ItemSet> = Vec::new();
    let mut config = BuildConfig::default();
    let mut passive_spec = PassiveSpec::default();
    let mut active_skill_set: usize = 0;
    let mut active_item_set: usize = 0;
    let mut items_use_second_weapon_set = false;
    let mut main_socket_group: usize;

    // Parser state
    let mut current_skill_set: Option<SkillSet> = None;
    let mut current_skill: Option<Skill> = None;
    let mut current_item_set: Option<ItemSet> = None;
    let mut current_item_id: Option<u32> = None;
    let mut current_item_text = String::new();
    let mut items: HashMap<u32, Item> = HashMap::new();
    let mut in_spec_sockets = false; // true when parsing <Sockets> inside <Spec>
    let mut in_spec_overrides = false; // true when parsing <Overrides> inside <Spec>

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name_bytes = e.name();
                let name = std::str::from_utf8(name_bytes.as_ref())
                    .map_err(|_| ParseError::Xml("invalid UTF-8 in element name".into()))?;
                let attrs: HashMap<String, String> = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .filter_map(|a| {
                        let k = std::str::from_utf8(a.key.as_ref()).ok()?.to_string();
                        let v = std::str::from_utf8(a.value.as_ref()).ok()?.to_string();
                        Some((k, v))
                    })
                    .collect();

                match name {
                    "Build" => {
                        let level = attrs
                            .get("level")
                            .and_then(|v| v.parse::<u8>().ok())
                            .unwrap_or(1);
                        let class_name = attrs.get("className").cloned().unwrap_or_default();
                        let ascend_class_name =
                            attrs.get("ascendClassName").cloned().unwrap_or_default();
                        let bandit = attrs
                            .get("bandit")
                            .cloned()
                            .unwrap_or_else(|| "None".into());
                        let pantheon_major_god = attrs
                            .get("pantheonMajorGod")
                            .cloned()
                            .unwrap_or_else(|| "None".into());
                        let pantheon_minor_god = attrs
                            .get("pantheonMinorGod")
                            .cloned()
                            .unwrap_or_else(|| "None".into());
                        let target_version =
                            attrs.get("targetVersion").cloned().unwrap_or_default();
                        main_socket_group = attrs
                            .get("mainSocketGroup")
                            .and_then(|v| v.parse::<usize>().ok())
                            .map(|v| v.saturating_sub(1))
                            .unwrap_or(0);
                        build = Some(Build {
                            class_name,
                            ascend_class_name,
                            level,
                            bandit,
                            pantheon_major_god,
                            pantheon_minor_god,
                            target_version,
                            passive_spec: PassiveSpec::default(),
                            skill_sets: Vec::new(),
                            active_skill_set: 0,
                            main_socket_group,
                            item_sets: Vec::new(),
                            active_item_set: 0,
                            config: BuildConfig::default(),
                            items: HashMap::new(),
                        });
                    }
                    "Skills" => {
                        active_skill_set = attrs
                            .get("activeSkillSet")
                            .and_then(|v| v.parse::<usize>().ok())
                            .map(|v| v.saturating_sub(1))
                            .unwrap_or(0);
                    }
                    "SkillSet" => {
                        let id = attrs.get("id").and_then(|v| v.parse().ok()).unwrap_or(1);
                        current_skill_set = Some(SkillSet {
                            id,
                            skills: Vec::new(),
                        });
                    }
                    "Skill" => {
                        let slot = attrs.get("slot").cloned().unwrap_or_default();
                        let enabled = attrs.get("enabled").map(|v| v == "true").unwrap_or(true);
                        let main_active_skill = attrs
                            .get("mainActiveSkill")
                            .and_then(|v| v.parse::<usize>().ok())
                            .map(|v| v.saturating_sub(1))
                            .unwrap_or(0);
                        current_skill = Some(Skill {
                            slot,
                            enabled,
                            main_active_skill,
                            gems: Vec::new(),
                            source: None,
                            no_supports: false,
                            slot_enabled: true,
                        });
                        // If there's no active SkillSet, create an implicit one.
                        // This handles builds where <Skill> elements are directly under <Skills>
                        // without a <SkillSet> wrapper (newer PoB build format).
                        if current_skill_set.is_none() {
                            current_skill_set = Some(SkillSet {
                                id: 1,
                                skills: Vec::new(),
                            });
                        }
                    }
                    "Gem" => {
                        if let Some(ref mut skill) = current_skill {
                            let skill_id = attrs.get("skillId").cloned().unwrap_or_default();
                            let level =
                                attrs.get("level").and_then(|v| v.parse().ok()).unwrap_or(1);
                            let quality = attrs
                                .get("quality")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0);
                            let enabled = attrs.get("enabled").map(|v| v == "true").unwrap_or(true);
                            let enable_global1 = attrs
                                .get("enableGlobal1")
                                .map(|v| v == "true")
                                .unwrap_or(true);
                            let enable_global2 = attrs
                                .get("enableGlobal2")
                                .map(|v| v == "true")
                                .unwrap_or(false);
                            skill.gems.push(Gem {
                                skill_id,
                                level,
                                quality,
                                enabled,
                                is_support: false,
                                enable_global1,
                                enable_global2,
                            });
                        }
                    }
                    "Spec" => {
                        let tree_version = attrs.get("treeVersion").cloned().unwrap_or_default();
                        let class_id = attrs
                            .get("classId")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0);
                        let ascend_class_id = attrs
                            .get("ascendClassId")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0);

                        // Parse masteryEffects attribute: "{nodeId,effectId},{nodeId,effectId},..."
                        // Mirrors PassiveSpec.lua:137-141 and ImportFromNodeList:251-257.
                        // Effect IDs >= 65536 are GGG profile import codes and are filtered out.
                        let mut mastery_selections: HashMap<u32, u32> = HashMap::new();
                        if let Some(mastery_str) = attrs.get("masteryEffects") {
                            for cap in RE_MASTERY_EFFECTS.captures_iter(mastery_str) {
                                if let (Ok(node_id), Ok(effect_id)) =
                                    (cap[1].parse::<u32>(), cap[2].parse::<u32>())
                                {
                                    // Filter out GGG profile import codes (effect_id >= 65536)
                                    if effect_id < 65536 {
                                        mastery_selections.insert(node_id, effect_id);
                                    }
                                }
                            }
                        }

                        // Parse nodes attribute and gate mastery nodes on having a selection.
                        // Mirrors ImportFromNodeList:266-277:
                        //   if node.type ~= "Mastery" or (node.type == "Mastery" and self.masterySelections[id])
                        //     then node.alloc = true
                        // At XML parse time we don't have tree data, so we conservatively
                        // admit all nodes; the mastery gate is enforced in setup.rs
                        // where tree data is available. Mastery nodes without a selection
                        // will be skipped in apply_passive_mods.
                        let mut allocated_nodes = std::collections::HashSet::new();
                        if let Some(nodes_str) = attrs.get("nodes") {
                            for n in nodes_str.split(',') {
                                if let Ok(id) = n.trim().parse::<u32>() {
                                    allocated_nodes.insert(id);
                                }
                            }
                        }

                        passive_spec = PassiveSpec {
                            tree_version,
                            allocated_nodes,
                            class_id,
                            ascend_class_id,
                            jewels: HashMap::new(),
                            mastery_selections,
                            hash_overrides: HashMap::new(),
                        };
                    }
                    "Sockets" => {
                        // <Sockets> is a child element of <Spec> that maps
                        // tree socket node IDs to socketed cluster jewel item IDs.
                        in_spec_sockets = true;
                    }
                    "Socket" => {
                        // <Socket nodeId="12345" itemId="11"/>
                        // Only process if inside a <Sockets> block within <Spec>.
                        if in_spec_sockets {
                            if let (Some(node_id), Some(item_id)) = (
                                attrs.get("nodeId").and_then(|v| v.parse::<u32>().ok()),
                                attrs.get("itemId").and_then(|v| v.parse::<u32>().ok()),
                            ) {
                                if item_id > 0 {
                                    passive_spec.jewels.insert(node_id, item_id);
                                }
                            }
                        }
                    }
                    "Overrides" => {
                        // <Overrides> is a child element of <Spec> that contains
                        // tattoo overrides for passive tree nodes.
                        // Mirrors PassiveSpec.lua:117-115: loop over xml children for
                        // `node.elem == "Overrides"` block.
                        in_spec_overrides = true;
                    }
                    "Override" => {
                        // <Override nodeId="12345" dn="Acrobatics" icon="Art/.../icon.png"
                        //           activeEffectImage="Art/.../bg.png"/>
                        // Mirrors PassiveSpec.lua:146-170 inner loop.
                        //
                        // The tattoo type data (overrideType, isKeystone, stats, etc.) is not
                        // in the XML — it comes from a tree.tattoo.nodes lookup by `dn`.
                        // We store all XML attributes needed for both the primary lookup (by dn)
                        // and the fallback lookup (by activeEffectImage + icon).
                        // The override_type / stats fields remain empty until enrichment via
                        // `enrich_hash_overrides_from_tattoo_data` in setup.rs (which requires
                        // TattooPassives.lua data to be loaded).
                        //
                        // Mirrors PassiveSpec.lua:163-169: only store if the tattoo lookup
                        // succeeds. We store unconditionally here and skip unknown tattoos
                        // during enrichment in setup.rs (same effect: no crash, just a log).
                        if in_spec_overrides {
                            if let Some(node_id_str) = attrs.get("nodeId") {
                                if let Ok(node_id) = node_id_str.parse::<u32>() {
                                    let dn = attrs.get("dn").cloned().unwrap_or_default();
                                    let active_effect_image =
                                        attrs.get("activeEffectImage").cloned().unwrap_or_default();
                                    let icon = attrs.get("icon").cloned().unwrap_or_default();
                                    let override_node = TattooOverrideNode {
                                        node_id,
                                        dn,
                                        is_tattoo: true,
                                        override_type: String::new(),
                                        is_keystone: false,
                                        is_notable: false,
                                        is_mastery: false,
                                        stats: Vec::new(),
                                        active_effect_image,
                                        icon,
                                    };
                                    passive_spec.hash_overrides.insert(node_id, override_node);
                                }
                            }
                        }
                    }
                    "Items" => {
                        active_item_set = attrs
                            .get("activeItemSet")
                            .and_then(|v| v.parse::<usize>().ok())
                            .map(|v| v.saturating_sub(1))
                            .unwrap_or(0);
                        items_use_second_weapon_set = attrs
                            .get("useSecondWeaponSet")
                            .map(|v| v == "true")
                            .unwrap_or(false);
                    }
                    "Item" => {
                        if let Some(id) = attrs.get("id").and_then(|v| v.parse::<u32>().ok()) {
                            current_item_id = Some(id);
                            current_item_text.clear();
                        }
                    }
                    "ItemSet" => {
                        let id = attrs.get("id").and_then(|v| v.parse().ok()).unwrap_or(1);
                        let use_second_weapon_set = attrs
                            .get("useSecondWeaponSet")
                            .map(|v| v == "true")
                            .unwrap_or(items_use_second_weapon_set);
                        current_item_set = Some(ItemSet {
                            id,
                            slots: HashMap::new(),
                            use_second_weapon_set,
                            ordered_slots: Vec::new(),
                        });
                    }
                    "Slot" => {
                        if let Some(ref mut iset) = current_item_set {
                            if let (Some(name), Some(item_id)) =
                                (attrs.get("name"), attrs.get("itemId"))
                            {
                                if let Ok(id) = item_id.parse::<u32>() {
                                    let active =
                                        attrs.get("active").map(|v| v == "true").unwrap_or(true);
                                    let node_id =
                                        attrs.get("nodeId").and_then(|v| v.parse::<u32>().ok());
                                    iset.slots.insert(name.clone(), id);
                                    iset.ordered_slots.push(ItemSetSlot {
                                        name: name.clone(),
                                        item_id: id,
                                        active,
                                        node_id,
                                    });
                                }
                            }
                        }
                    }
                    "Input" => {
                        if let Some(name) = attrs.get("name") {
                            let name = name.clone();
                            if let Some(v) = attrs.get("number") {
                                if let Ok(n) = v.parse::<f64>() {
                                    config.numbers.insert(name, n);
                                }
                            } else if let Some(v) = attrs.get("boolean") {
                                config.booleans.insert(name, v == "true");
                            } else if let Some(v) = attrs.get("string") {
                                config.strings.insert(name, v.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if current_item_id.is_some() {
                    if let Ok(text) = e.unescape() {
                        current_item_text.push_str(&text);
                    }
                }
            }
            Ok(Event::CData(ref e)) => {
                if current_item_id.is_some() {
                    if let Ok(text) = std::str::from_utf8(e.as_ref()) {
                        current_item_text.push_str(text);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let end_name_bytes = e.name();
                let name = std::str::from_utf8(end_name_bytes.as_ref()).unwrap_or("");
                match name {
                    "Sockets" => {
                        in_spec_sockets = false;
                    }
                    "Overrides" => {
                        in_spec_overrides = false;
                    }
                    "Skill" => {
                        if let (Some(ref mut ss), Some(skill)) =
                            (&mut current_skill_set, current_skill.take())
                        {
                            ss.skills.push(skill);
                        }
                    }
                    "Skills" => {
                        // Close any implicit SkillSet that was created for builds
                        // without explicit <SkillSet> elements.
                        if let Some(ss) = current_skill_set.take() {
                            if !ss.skills.is_empty() {
                                skill_sets.push(ss);
                            }
                        }
                    }
                    "SkillSet" => {
                        if let Some(ss) = current_skill_set.take() {
                            skill_sets.push(ss);
                        }
                    }
                    "Item" => {
                        if let Some(id) = current_item_id.take() {
                            let item = parse_item_text(id, &current_item_text);
                            items.insert(id, item);
                            current_item_text.clear();
                        }
                    }
                    "ItemSet" => {
                        if let Some(iset) = current_item_set.take() {
                            item_sets.push(iset);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(ParseError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    let mut b = build.ok_or_else(|| ParseError::MissingAttr {
        element: "PathOfBuilding".into(),
        attr: "Build".into(),
    })?;
    b.passive_spec = passive_spec;
    b.skill_sets = skill_sets;
    b.active_skill_set = active_skill_set;
    b.item_sets = item_sets;
    b.active_item_set = active_item_set;
    b.config = config;
    b.items = items;
    Ok(b)
}

/// Strip curly-brace prefixes like `{variant:1,2}`, `{range:0.5}`, `{tags:...}`,
/// `{crafted}`, `{fractured}`, `{enchant}` from a mod line.
/// Returns (stripped_text, is_crafted, is_enchant, is_fractured).
fn strip_mod_prefixes(line: &str) -> (String, bool, bool, bool) {
    let mut remaining = line;
    let mut is_crafted = false;
    let mut is_enchant = false;
    let mut is_fractured = false;

    // Strip all {prefix} tags from the start
    while remaining.starts_with('{') {
        if let Some(end) = remaining.find('}') {
            let tag = &remaining[1..end];
            match tag {
                "crafted" => is_crafted = true,
                "enchant" => is_enchant = true,
                "fractured" => is_fractured = true,
                _ => {} // variant:..., range:..., tags:... — just strip
            }
            remaining = &remaining[end + 1..];
        } else {
            break;
        }
    }

    (remaining.to_string(), is_crafted, is_enchant, is_fractured)
}

/// Parse socket string like "R-R-G B" into SocketGroups.
/// Dash means linked, space means new group.
fn parse_sockets(s: &str) -> Vec<SocketGroup> {
    s.split_whitespace()
        .map(|group_str| {
            let colors: Vec<char> = group_str
                .split('-')
                .filter_map(|c| c.chars().next())
                .collect();
            let linked = colors.len() > 1;
            SocketGroup { colors, linked }
        })
        .collect()
}

/// Lines to skip in the item header section.
fn is_skip_header(line: &str) -> bool {
    line.starts_with("Variant:")
        || line.starts_with("Selected Variant:")
        || line.starts_with("LevelReq:")
        || line.starts_with("League:")
        || line.starts_with("Source:")
        || line.starts_with("Requires")
        || line.starts_with("Radius:")
        || line.starts_with("Unreleased")
        || line.starts_with("Upgrade:")
        || line.starts_with("Tincture")
        || line.starts_with("Has Alt Variant")
}

/// Parse the text content of an `<Item>` element into an Item struct.
fn parse_item_text(id: u32, text: &str) -> Item {
    let mut rarity = ItemRarity::Normal;
    let mut name = String::new();
    let mut base_type = String::new();
    let mut quality: u32 = 0;
    let mut sockets: Vec<SocketGroup> = Vec::new();
    let mut corrupted = false;
    let mut class_restriction: Option<String> = None;
    let mut influence = ItemInfluence::default();
    let mut implicits: Vec<String> = Vec::new();
    let mut explicits: Vec<String> = Vec::new();
    let mut crafted_mods: Vec<String> = Vec::new();
    let mut enchant_mods: Vec<String> = Vec::new();
    let mut radius: Option<String> = None;

    let mut limit: Option<u32> = None;
    let mut implicits_remaining: Option<usize> = None;
    let mut in_mods = false; // true once we've passed the Implicits: line and consumed all implicits
    let mut name_lines: Vec<String> = Vec::new(); // collect name/base_type candidates

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // If we're counting implicit lines
        if let Some(ref mut remaining) = implicits_remaining {
            if *remaining > 0 {
                let (stripped, _crafted, _enchant, _fractured) = strip_mod_prefixes(line);
                implicits.push(stripped);
                *remaining -= 1;
                if *remaining == 0 {
                    in_mods = true;
                    implicits_remaining = None;
                }
                continue;
            }
        }

        // Once past implicits, everything is a mod line
        if in_mods {
            // Check influence/corrupted markers that can appear after mods
            match line {
                "Corrupted" => {
                    corrupted = true;
                    continue;
                }
                "Shaper Item" => {
                    influence.shaper = true;
                    continue;
                }
                "Elder Item" => {
                    influence.elder = true;
                    continue;
                }
                "Crusader Item" => {
                    influence.crusader = true;
                    continue;
                }
                "Redeemer Item" => {
                    influence.redeemer = true;
                    continue;
                }
                "Hunter Item" => {
                    influence.hunter = true;
                    continue;
                }
                "Warlord Item" => {
                    influence.warlord = true;
                    continue;
                }
                "Fractured Item" => {
                    influence.fractured = true;
                    continue;
                }
                "Synthesised Item" => {
                    influence.synthesised = true;
                    continue;
                }
                _ => {}
            }

            let (stripped, is_crafted, is_enchant, _is_fractured) = strip_mod_prefixes(line);
            if is_crafted {
                crafted_mods.push(stripped);
            } else if is_enchant {
                enchant_mods.push(stripped);
            } else {
                explicits.push(stripped);
            }
            continue;
        }

        // Header section (before Implicits:)
        if let Some(rest) = line.strip_prefix("Rarity: ") {
            rarity = ItemRarity::from_str(rest).unwrap_or(ItemRarity::Normal);
            continue;
        }
        if let Some(rest) = line.strip_prefix("Quality: ") {
            quality = rest.parse().unwrap_or(0);
            continue;
        }
        if let Some(rest) = line.strip_prefix("Sockets: ") {
            sockets = parse_sockets(rest);
            continue;
        }
        if let Some(rest) = line.strip_prefix("Implicits: ") {
            let count: usize = rest.parse().unwrap_or(0);
            if count == 0 {
                in_mods = true;
            } else {
                implicits_remaining = Some(count);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("Radius: ") {
            radius = Some(rest.to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("Limited to: ") {
            limit = rest.parse::<u32>().ok();
            continue;
        }
        if let Some(rest) = line.strip_prefix("Requires Class ") {
            class_restriction = Some(rest.to_string());
            continue;
        }
        if line == "Corrupted" {
            corrupted = true;
            continue;
        }

        // Influence markers in header
        match line {
            "Shaper Item" => {
                influence.shaper = true;
                continue;
            }
            "Elder Item" => {
                influence.elder = true;
                continue;
            }
            "Crusader Item" => {
                influence.crusader = true;
                continue;
            }
            "Redeemer Item" => {
                influence.redeemer = true;
                continue;
            }
            "Hunter Item" => {
                influence.hunter = true;
                continue;
            }
            "Warlord Item" => {
                influence.warlord = true;
                continue;
            }
            "Fractured Item" => {
                influence.fractured = true;
                continue;
            }
            "Synthesised Item" => {
                influence.synthesised = true;
                continue;
            }
            _ => {}
        }

        // Skip known header lines
        if is_skip_header(line) {
            continue;
        }

        // Otherwise it's a name or base_type line
        name_lines.push(line.to_string());
    }

    // First non-header line is name, second is base_type
    if let Some(n) = name_lines.first() {
        name = n.clone();
    }
    if let Some(bt) = name_lines.get(1) {
        base_type = bt.clone();
    }

    // Foulborn is determined by whether the item title/name contains "Foulborn"
    // (mirrors Lua: `if self.title and self.title:find("Foulborn") then self.foulborn = true end`)
    let foulborn = name.contains("Foulborn");

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
        foulborn,
        class_restriction,
        influence,
        weapon_data: None,
        armour_data: None,
        flask_data: None,
        requirements: ItemRequirements::default(),
        radius,
        limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="Juggernaut">
  </Build>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Cleave" level="20" quality="20" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="50459,47175" classId="1" ascendClassId="1"/>
  </Tree>
  <Items activeItemSet="1">
    <ItemSet id="1"/>
  </Items>
  <Config>
    <Input name="enemyLevel" number="84"/>
    <Input name="conditionFullLife" boolean="true"/>
  </Config>
</PathOfBuilding>"#;

    #[test]
    fn parses_character_level() {
        let build = parse_xml(MINIMAL_XML).unwrap();
        assert_eq!(build.level, 90);
    }

    #[test]
    fn parses_class_name() {
        let build = parse_xml(MINIMAL_XML).unwrap();
        assert_eq!(build.class_name, "Marauder");
    }

    #[test]
    fn parses_passive_nodes() {
        let build = parse_xml(MINIMAL_XML).unwrap();
        assert!(build.passive_spec.allocated_nodes.contains(&50459));
        assert!(build.passive_spec.allocated_nodes.contains(&47175));
    }

    #[test]
    fn parses_skill_gem() {
        let build = parse_xml(MINIMAL_XML).unwrap();
        assert_eq!(build.skill_sets.len(), 1);
        let skill = &build.skill_sets[0].skills[0];
        assert_eq!(skill.gems[0].skill_id, "Cleave");
        assert_eq!(skill.gems[0].level, 20);
    }

    #[test]
    fn parses_config_flags() {
        let build = parse_xml(MINIMAL_XML).unwrap();
        assert_eq!(build.config.numbers.get("enemyLevel"), Some(&84.0));
        assert_eq!(build.config.booleans.get("conditionFullLife"), Some(&true));
    }

    #[test]
    fn rejects_missing_build_element() {
        assert!(parse_xml("<PathOfBuilding/>").is_err());
    }

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
        assert_eq!(item.name, "Test Sword");
    }

    #[test]
    fn parses_item_with_crafted_and_enchant() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/></Tree>
  <Items activeItemSet="1">
    <Item id="2">
Rarity: RARE
Test Helmet
Eternal Burgonet
Quality: 20
Implicits: 0
+80 to maximum Life
{crafted}+25% to Fire Resistance
{enchant}40% increased Fireball Damage
    </Item>
    <ItemSet id="1"/>
  </Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let item = build.items.get(&2).unwrap();
        assert_eq!(item.explicits.len(), 1, "should have 1 regular explicit");
        assert_eq!(item.crafted_mods.len(), 1, "should have 1 crafted mod");
        assert_eq!(item.enchant_mods.len(), 1, "should have 1 enchant mod");
        // Crafted/enchant prefix should be stripped
        assert_eq!(item.crafted_mods[0], "+25% to Fire Resistance");
        assert_eq!(item.enchant_mods[0], "40% increased Fireball Damage");
    }

    #[test]
    fn parses_unique_item() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/></Tree>
  <Items activeItemSet="1">
    <Item id="3">
Rarity: UNIQUE
Goldrim
Leather Cap
Implicits: 0
+(30-50) to Evasion Rating
10% increased Rarity of Items found
+(30-40)% to all Elemental Resistances
    </Item>
    <ItemSet id="1"/>
  </Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let item = build.items.get(&3).unwrap();
        assert_eq!(item.rarity, ItemRarity::Unique);
        assert_eq!(item.name, "Goldrim");
        assert_eq!(item.base_type, "Leather Cap");
    }
}

#[cfg(test)]
mod mastery_tests {
    use super::*;

    #[test]
    fn parses_mastery_effects_attribute() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_27" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="Juggernaut"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_27" nodes="12345,67890,99999" classId="1" ascendClassId="1"
          masteryEffects="{12345,48385},{67890,4119}"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let spec = &build.passive_spec;

        // Both mastery selections should be parsed
        assert_eq!(
            spec.mastery_selections.len(),
            2,
            "Should have 2 mastery selections"
        );
        assert_eq!(spec.mastery_selections.get(&12345), Some(&48385));
        assert_eq!(spec.mastery_selections.get(&67890), Some(&4119));

        // Node 99999 has no selection — still in allocated_nodes (mastery gate is in setup.rs)
        assert!(spec.allocated_nodes.contains(&99999));
        // Nodes with selections are in allocated_nodes
        assert!(spec.allocated_nodes.contains(&12345));
        assert!(spec.allocated_nodes.contains(&67890));
    }

    #[test]
    fn filters_ggg_profile_mastery_codes() {
        // Effect IDs >= 65536 are GGG profile import codes and should be filtered
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_27" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_27" nodes="12345" classId="1" ascendClassId="0"
          masteryEffects="{12345,65536},{67890,100}"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let spec = &build.passive_spec;

        // Effect 65536 (GGG code) should be filtered; effect 100 should be kept
        assert!(
            !spec.mastery_selections.contains_key(&12345),
            "Effect >= 65536 should be filtered"
        );
        assert_eq!(spec.mastery_selections.get(&67890), Some(&100));
        assert_eq!(spec.mastery_selections.len(), 1);
    }

    #[test]
    fn handles_missing_mastery_effects_attribute() {
        // Builds without masteryEffects attribute should work fine
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_13" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_13" nodes="50459,47175" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        assert!(
            build.passive_spec.mastery_selections.is_empty(),
            "No masteryEffects attribute should result in empty mastery_selections"
        );
    }
}

#[cfg(test)]
mod socket_tests {
    use super::*;

    #[test]
    fn parses_sockets_from_spec() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_0" bandit="None" className="Witch" ascendClassName="Elementalist" mainSocketGroup="1">
  </Build>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_13" nodes="21984,7960" classId="3" ascendClassId="2">
      <Sockets>
        <Socket nodeId="7960" itemId="8"/>
        <Socket nodeId="21984" itemId="2"/>
        <Socket nodeId="64583" itemId="9"/>
        <Socket nodeId="0" itemId="0"/>
      </Sockets>
    </Spec>
  </Tree>
  <Items activeItemSet="1">
    <ItemSet id="1"/>
  </Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        assert_eq!(
            build.passive_spec.jewels.len(),
            3,
            "Should have 3 non-zero jewels"
        );
        assert_eq!(build.passive_spec.jewels.get(&7960), Some(&8));
        assert_eq!(build.passive_spec.jewels.get(&21984), Some(&2));
        assert_eq!(build.passive_spec.jewels.get(&64583), Some(&9));
        assert!(
            !build.passive_spec.jewels.contains_key(&0),
            "nodeId=0 with itemId=0 should not be stored"
        );
    }
}
