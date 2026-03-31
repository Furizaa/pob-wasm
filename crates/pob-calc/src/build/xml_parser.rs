use super::types::*;
use crate::error::ParseError;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

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
    let mut main_socket_group: usize;

    // Parser state
    let mut current_skill_set: Option<SkillSet> = None;
    let mut current_skill: Option<Skill> = None;
    let mut current_item_set: Option<ItemSet> = None;

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
                        });
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
                            skill.gems.push(Gem {
                                skill_id,
                                level,
                                quality,
                                enabled,
                                is_support: false,
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
                        };
                    }
                    "Items" => {
                        active_item_set = attrs
                            .get("activeItemSet")
                            .and_then(|v| v.parse::<usize>().ok())
                            .map(|v| v.saturating_sub(1))
                            .unwrap_or(0);
                    }
                    "ItemSet" => {
                        let id = attrs.get("id").and_then(|v| v.parse().ok()).unwrap_or(1);
                        current_item_set = Some(ItemSet {
                            id,
                            slots: HashMap::new(),
                        });
                    }
                    "Slot" => {
                        if let Some(ref mut iset) = current_item_set {
                            if let (Some(name), Some(item_id)) =
                                (attrs.get("name"), attrs.get("itemId"))
                            {
                                if let Ok(id) = item_id.parse::<u32>() {
                                    iset.slots.insert(name.clone(), id);
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
            Ok(Event::End(ref e)) => {
                let end_name_bytes = e.name();
                let name = std::str::from_utf8(end_name_bytes.as_ref()).unwrap_or("");
                match name {
                    "Skill" => {
                        if let (Some(ref mut ss), Some(skill)) =
                            (&mut current_skill_set, current_skill.take())
                        {
                            ss.skills.push(skill);
                        }
                    }
                    "SkillSet" => {
                        if let Some(ss) = current_skill_set.take() {
                            skill_sets.push(ss);
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
    Ok(b)
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
}
