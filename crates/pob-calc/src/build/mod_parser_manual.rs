//! Hand-written handlers for specialModList entries that codegen can't template.
//!
//! Each handler is identified by a string ID (emitted by the codegen manifest).
//! See mod_parser_manual_manifest.rs for the full list of entries with their
//! Lua source as comments for reference.

use crate::mod_db::types::*;

/// Number of manual handlers implemented — used for compile-time assertion.
pub const IMPLEMENTED_MANUAL_COUNT: usize = 25;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Capitalize first letter (Lua's firstToUpper equivalent).
fn first_to_upper(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

/// Title case: capitalize each word, remove spaces (e.g. "fire damage" → "FireDamage").
fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| first_to_upper(w))
        .collect::<Vec<_>>()
        .join("")
}

/// Non-damaging ailment types from PoB's data.
const NON_DAMAGING_AILMENTS: &[&str] = &["Chill", "Freeze", "Shock", "Scorch", "Brittle", "Sap"];

/// Build a simple numeric mod.
fn make_mod(name: &str, mod_type: ModType, value: f64, source: &ModSource) -> Mod {
    Mod {
        name: name.into(),
        mod_type,
        value: ModValue::Number(value),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags: vec![],
        source: source.clone(),
    }
}

/// Build a LIST mod with a string value.
fn make_list_mod(name: &str, value: &str, source: &ModSource) -> Mod {
    Mod {
        name: name.into(),
        mod_type: ModType::List,
        value: ModValue::String(value.into()),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags: vec![],
        source: source.clone(),
    }
}

/// Build a FLAG mod.
fn make_flag(name: &str, source: &ModSource) -> Mod {
    Mod::new_flag(name, source.clone())
}

/// Build a FLAG mod with tags.
fn make_flag_with_tags(name: &str, tags: Vec<ModTag>, source: &ModSource) -> Mod {
    Mod {
        name: name.into(),
        mod_type: ModType::Flag,
        value: ModValue::Bool(true),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags,
        source: source.clone(),
    }
}

/// Build a LIST mod with a string value and tags.
fn make_list_mod_with_tags(name: &str, value: &str, tags: Vec<ModTag>, source: &ModSource) -> Mod {
    Mod {
        name: name.into(),
        mod_type: ModType::List,
        value: ModValue::String(value.into()),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags,
        source: source.clone(),
    }
}

/// Build an INC mod with tags.
fn make_inc_with_tags(name: &str, value: f64, tags: Vec<ModTag>, source: &ModSource) -> Mod {
    Mod {
        name: name.into(),
        mod_type: ModType::Inc,
        value: ModValue::Number(value),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags,
        source: source.clone(),
    }
}

/// Get a capture group as &str, defaulting to "" if missing.
fn cap_str<'a>(caps: &'a regex::Captures, idx: usize) -> &'a str {
    caps.get(idx).map(|m| m.as_str()).unwrap_or("")
}

/// Get a capture group parsed as f64, defaulting to 0.
fn cap_num(caps: &regex::Captures, idx: usize) -> f64 {
    cap_str(caps, idx).parse::<f64>().unwrap_or(0.0)
}

/// Remap slot name from item description text to PoB's internal slot names.
/// Lua: if slot == "shield" then slot = "Weapon 2" end
fn remap_slot(slot: &str) -> &str {
    match slot.to_lowercase().as_str() {
        "shield" => "Weapon 2",
        "main hand" | "mainhand" => "Weapon 1",
        "off hand" | "offhand" => "Weapon 2",
        "body armour" => "Body Armour",
        "helmet" | "helm" => "Helmet",
        "gloves" => "Gloves",
        "boots" => "Boots",
        "amulet" => "Amulet",
        "ring" | "ring 1" => "Ring 1",
        "ring 2" => "Ring 2",
        "belt" => "Belt",
        _ => slot,
    }
}

/// Map itemSlotName to target slot (Lua line 3236).
/// "main hand" → "Weapon 1", "off hand" → "Weapon 2", else "Body Armour"
fn remap_support_slot(slot_name: &str) -> &str {
    match slot_name.trim().to_lowercase().as_str() {
        "main hand" => "Weapon 1",
        "off hand" => "Weapon 2",
        _ => "Body Armour",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// explodeFunc helper — shared logic for all explode variants
// ─────────────────────────────────────────────────────────────────────────────

/// Build ExplodeMod + CanExplode for an explode pattern.
/// `chance`: explode chance (percent); `amount`: the text for amount (often "10th" or "%");
/// `damage_type`: damage type string; `extra_tags`: additional condition tags.
fn explode_func(
    chance: f64,
    amount: &str,
    damage_type: &str,
    extra_tags: Vec<ModTag>,
    source: &ModSource,
) -> Vec<Mod> {
    // The ExplodeMod is a LIST mod carrying the explode parameters as a string
    // Format: "chance:amount:type" — the calc engine parses this.
    let explode_value = format!("{}:{}:{}", chance, amount, first_to_upper(damage_type));
    let mut explode_mod = make_list_mod("ExplodeMod", &explode_value, source);
    explode_mod.tags = extra_tags.clone();

    let mut can_explode = make_flag("CanExplode", source);
    can_explode.tags = extra_tags;

    vec![explode_mod, can_explode]
}

// ─────────────────────────────────────────────────────────────────────────────
// triggerExtraSkill / grantedExtraSkill / extraSupport helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build ExtraSkill LIST mod for triggered skills.
fn trigger_extra_skill(
    skill_name: &str,
    level: f64,
    extra_tags: Vec<ModTag>,
    source: &ModSource,
) -> Vec<Mod> {
    // Format: "skillName:level:triggered:noSupports"
    let value = format!("{}:{}:true:true", skill_name.trim(), level);
    vec![make_list_mod_with_tags(
        "ExtraSkill",
        &value,
        extra_tags,
        source,
    )]
}

/// Build ExtraSkill LIST mod for granted skills.
fn granted_extra_skill(
    skill_name: &str,
    level: f64,
    extra_tags: Vec<ModTag>,
    source: &ModSource,
) -> Vec<Mod> {
    let value = format!("{}:{}:false:false", skill_name.trim(), level);
    vec![make_list_mod_with_tags(
        "ExtraSkill",
        &value,
        extra_tags,
        source,
    )]
}

/// Build ExtraSupport LIST mod.
fn extra_support(
    support_name: &str,
    level: f64,
    extra_tags: Vec<ModTag>,
    source: &ModSource,
) -> Vec<Mod> {
    let value = format!("{}:{}", support_name.trim(), level);
    vec![make_list_mod_with_tags(
        "ExtraSupport",
        &value,
        extra_tags,
        source,
    )]
}

// ─────────────────────────────────────────────────────────────────────────────
// Main dispatch
// ─────────────────────────────────────────────────────────────────────────────

/// Handle a manually-implemented special mod pattern.
///
/// `id` is the handler ID assigned by the codegen manifest.
/// `caps` contains the regex captures from the pattern match.
/// `source` is the ModSource for the resulting mods.
///
/// Returns the mods for this pattern, or empty vec if the handler is not yet implemented.
pub fn handle_manual_special(id: &str, caps: &regex::Captures, source: &ModSource) -> Vec<Mod> {
    match id {
        // ═══════════════════════════════════════════════════════════════════
        // 25 manual handlers
        // ═══════════════════════════════════════════════════════════════════

        // ── manual_6 ─────────────────────────────────────────────────────
        // Lua line 2050: explodeFunc
        // Pattern: enemies killed with attack hits have a (%d+)% chance to explode,
        //          dealing a (.+) of their life as (.+) damage
        // Captures: 1=chance, 2=amount, 3=damageType
        "manual_6" => {
            let chance = cap_num(caps, 1);
            let amount = cap_str(caps, 2);
            let dmg_type = cap_str(caps, 3);
            explode_func(chance, amount, dmg_type, vec![], source)
        }

        // ── manual_116 ──────────────────────────────────────────────────
        // Lua line 2277: gain no (.+) from equipped (.+)
        // Captures: 1=stat, 2=slot
        // If slot == "shield" → slot = "Weapon 2". Emit flag with dynamic name.
        "manual_116" => {
            let stat = cap_str(caps, 1);
            let slot = cap_str(caps, 2);
            let remapped_slot = remap_slot(slot);
            // Lua: flag("GainNo"..firstToUpper(stat):gsub(" ","").."From"..slot, {type="SlotName", slotName=slot})
            let flag_name = format!(
                "GainNo{}From{}",
                title_case(stat),
                remapped_slot.replace(' ', "")
            );
            vec![make_flag_with_tags(
                &flag_name,
                vec![ModTag::SlotName {
                    slot_name: remapped_slot.to_string(),
                    neg: false,
                }],
                source,
            )]
        }

        // ── manual_226 ──────────────────────────────────────────────────
        // Lua line 2519: ExtraJewelFunc LIST — jewel radius damage type conversion
        // Pattern: non-unique jewels cause increases and reductions to other damage types
        //          in a (%a+) radius to be transformed to apply to (%a+) damage
        // Captures: 1=radius, 2=dmgType
        "manual_226" => {
            let radius = first_to_upper(cap_str(caps, 1));
            let dmg_type = first_to_upper(cap_str(caps, 2));
            // Emit ExtraJewelFunc LIST with data for the calc engine
            let value = format!("radiusConvertDmg:{}:{}", radius, dmg_type);
            vec![make_list_mod("ExtraJewelFunc", &value, source)]
        }

        // ── manual_227 ──────────────────────────────────────────────────
        // Lua line 2527: ExtraJewelFunc LIST — jewel radius attribute grant
        // Pattern: non-unique jewels cause small and notable passive skills
        //          in a (%a+) radius to also grant +(%d+) to (%a+)
        // Captures: 1=radius, 2=value, 3=attribute
        "manual_227" => {
            let radius = first_to_upper(cap_str(caps, 1));
            let val = cap_num(caps, 2);
            let attr = first_to_upper(cap_str(caps, 3));
            // Emit ExtraJewelFunc LIST with data for the calc engine
            let value = format!("radiusGrantAttr:{}:{}:{}", radius, val, attr);
            vec![make_list_mod("ExtraJewelFunc", &value, source)]
        }

        // ── manual_311 ──────────────────────────────────────────────────
        // Lua line 2772: non-damaging ailments have (%d+)% reduced effect on you
        //                while you have arcane surge
        // Captures: 1=num (percent)
        // Emit one mod per ailment in NON_DAMAGING_AILMENTS
        "manual_311" => {
            let num = cap_num(caps, 1);
            let mut mods = Vec::with_capacity(NON_DAMAGING_AILMENTS.len());
            for ailment in NON_DAMAGING_AILMENTS {
                // Lua: mod("Self"..ailment.."Effect", "INC", -num, { type="Condition", var="AffectedByArcaneSurge" })
                let name = format!("Self{}Effect", ailment);
                mods.push(make_inc_with_tags(
                    &name,
                    -num,
                    vec![ModTag::Condition {
                        var: "AffectedByArcaneSurge".into(),
                        neg: false,
                    }],
                    source,
                ));
            }
            mods
        }

        // ── manual_417 ──────────────────────────────────────────────────
        // Lua line 2966: reflects your opposite ring — empty/noop
        "manual_417" => {
            vec![]
        }

        // ── manual_469 ──────────────────────────────────────────────────
        // Lua line 3045: ([+-]%d+)%? to (%a+) of socketed (%a+) gems
        // Captures: 1=num, 2=property, 3=keyword (gem type; "" means "all")
        // Emit GemProperty LIST
        "manual_469" => {
            let num = cap_num(caps, 1);
            let property = cap_str(caps, 2).to_lowercase();
            let keyword_raw = cap_str(caps, 3).trim();
            let keyword = if keyword_raw.is_empty() {
                "all"
            } else {
                keyword_raw
            };
            // Lua: mod("GemProperty", "LIST", {keyword=type, key=property, value=num})
            let value = format!("keyword:{}:{}:{}", keyword, property, num);
            vec![make_list_mod("GemProperty", &value, source)]
        }

        // ── manual_493 ──────────────────────────────────────────────────
        // Lua line 3078: socketed non-channelling bow skills are triggered by snipe — noop
        "manual_493" => {
            vec![]
        }

        // ── manual_501 ──────────────────────────────────────────────────
        // Lua line 3094: ([+-]%d+)%? to (%a+) of all (.+) gems
        // Captures: 1=num, 2=property, 3=skill name or keyword list
        // Emit GemProperty LIST with keyword or skill reference
        "manual_501" => {
            let num = cap_num(caps, 1);
            let property = cap_str(caps, 2).to_lowercase();
            let skill = cap_str(caps, 3).trim();
            // Lua: if gemIdLookup[skill] → keyword=skill; else split by "," and emit per keyword
            // We emit with the skill name; calc engine resolves later
            let value = format!("keyword:{}:{}:{}", skill, property, num);
            vec![make_list_mod("GemProperty", &value, source)]
        }

        // ── manual_581 ──────────────────────────────────────────────────
        // Lua line 3203: curse enemies with (%D+) on %a+, with (%d+)% increased effect
        // Captures: 1=skill name, 2=num (effect percent)
        "manual_581" => {
            let skill = cap_str(caps, 1).trim();
            let num = cap_num(caps, 2);
            // ExtraSkill LIST + CurseEffect INC
            let skill_value = format!("{}:1:false:true", skill);
            let skill_id_tag = vec![ModTag::SkillName {
                name: skill.to_string(),
            }];
            vec![
                make_list_mod("ExtraSkill", &skill_value, source),
                make_inc_with_tags("CurseEffect", num, skill_id_tag, source),
            ]
        }

        // ── manual_582 ──────────────────────────────────────────────────
        // Lua line 3207: curse enemies with (%D+) on %a+, with (%d+)% reduced effect
        // Same as 581 but negated
        "manual_582" => {
            let skill = cap_str(caps, 1).trim();
            let num = cap_num(caps, 2);
            let skill_value = format!("{}:1:false:true", skill);
            let skill_id_tag = vec![ModTag::SkillName {
                name: skill.to_string(),
            }];
            vec![
                make_list_mod("ExtraSkill", &skill_value, source),
                make_inc_with_tags("CurseEffect", -num, skill_id_tag, source),
            ]
        }

        // ── manual_583 ──────────────────────────────────────────────────
        // Lua line 3211: %d+% chance to curse non-cursed enemies with (%D+) on %a+,
        //                with (%d+)% increased effect
        // Same output as 581
        "manual_583" => {
            let skill = cap_str(caps, 1).trim();
            let num = cap_num(caps, 2);
            let skill_value = format!("{}:1:false:true", skill);
            let skill_id_tag = vec![ModTag::SkillName {
                name: skill.to_string(),
            }];
            vec![
                make_list_mod("ExtraSkill", &skill_value, source),
                make_inc_with_tags("CurseEffect", num, skill_id_tag, source),
            ]
        }

        // ── manual_584 ──────────────────────────────────────────────────
        // Lua line 3215: %d+% chance to curse non-cursed enemies with (%D+) on %a+
        // Captures: 1=skill name
        "manual_584" => {
            let skill = cap_str(caps, 1).trim();
            let skill_value = format!("{}:1:false:true", skill);
            vec![make_list_mod("ExtraSkill", &skill_value, source)]
        }

        // ── manual_601 ──────────────────────────────────────────────────
        // Lua line 3236: socketed support gems can also support skills from your equipped (%a+)
        // Captures: 1=itemSlotName
        // Emit LinkedSupport LIST with slot remapping
        "manual_601" => {
            let slot_name = cap_str(caps, 1).trim();
            let target_slot = remap_support_slot(slot_name);
            let value = format!("{}", target_slot);
            vec![make_list_mod("LinkedSupport", &value, source)]
        }

        // ── manual_605 ──────────────────────────────────────────────────
        // Lua line 3248: trigger level (%d+) (.+), (.+) or (.+) every (%d+) seconds
        // Captures: 1=level, 2=skill1, 3=skill2, 4=skill3, 5=seconds (unused)
        // Three triggerExtraSkill calls
        "manual_605" => {
            let level = cap_num(caps, 1);
            let skill1 = cap_str(caps, 2).trim();
            let skill2 = cap_str(caps, 3).trim();
            let skill3 = cap_str(caps, 4).trim();
            let mut mods = Vec::new();
            for skill in [skill1, skill2, skill3] {
                let value = format!("{}:{}:true:true", skill, level);
                mods.push(make_list_mod("ExtraSkill", &value, source));
            }
            mods
        }

        // ── manual_941 ──────────────────────────────────────────────────
        // Lua line 3920: you are cursed with level (%d+) (%D+)
        // Captures: 1=level, 2=curse name
        // ExtraCurse LIST with applyToPlayer=true
        "manual_941" => {
            let level = cap_num(caps, 1);
            let name = cap_str(caps, 2).trim();
            let value = format!("{}:{}:applyToPlayer", name, level);
            vec![make_list_mod("ExtraCurse", &value, source)]
        }

        // ── manual_942 ──────────────────────────────────────────────────
        // Lua line 3921: you are cursed with (%D+)
        // Captures: 1=curse name
        // ExtraCurse LIST with applyToPlayer=true, level=1
        "manual_942" => {
            let name = cap_str(caps, 1).trim();
            let value = format!("{}:1:applyToPlayer", name);
            vec![make_list_mod("ExtraCurse", &value, source)]
        }

        // ── manual_943 ──────────────────────────────────────────────────
        // Lua line 3922: you are cursed with (%D+), with (%d+)% increased effect
        // Captures: 1=curse name, 2=effect percent
        // ExtraCurse LIST + CurseEffectAgainstPlayer INC
        "manual_943" => {
            let name = cap_str(caps, 1).trim();
            let num = cap_num(caps, 2);
            let value = format!("{}:1:applyToPlayer", name);
            let skill_id_tag = vec![ModTag::SkillName {
                name: name.to_string(),
            }];
            vec![
                make_list_mod("ExtraCurse", &value, source),
                make_inc_with_tags("CurseEffectAgainstPlayer", num, skill_id_tag, source),
            ]
        }

        // ── manual_1057 ─────────────────────────────────────────────────
        // Lua line 4133: (%a+) has (%d+)% increased effect
        // Captures: 1=skill name, 2=num
        // BuffEffect INC with SkillId tag (needs gemIdLookup — use name)
        "manual_1057" => {
            let skill = cap_str(caps, 1).trim();
            let num = cap_num(caps, 2);
            // Lua: mod("BuffEffect", "INC", num, { type="SkillId", skillId=gemIdLookup[skill] })
            // We use SkillName tag since we can't resolve gemId at parse time
            vec![make_inc_with_tags(
                "BuffEffect",
                num,
                vec![ModTag::SkillName {
                    name: skill.to_string(),
                }],
                source,
            )]
        }

        // ── manual_1559 ─────────────────────────────────────────────────
        // Lua line 4893: grants level (%d+) (.+) curse aura during flask effect
        // Captures: 1=level, 2=skill name
        // ExtraCurse LIST (flask context)
        "manual_1559" => {
            let level = cap_num(caps, 1);
            let skill = cap_str(caps, 2).trim();
            // Lua strips " skill" from the name
            let clean_skill = skill.replace(" skill", "");
            let value = format!("{}:{}", clean_skill, level);
            vec![make_list_mod("ExtraCurse", &value, source)]
        }

        // ── manual_1712 ─────────────────────────────────────────────────
        // Lua line 5127: (%a+) can affect hexproof enemies
        // Captures: 1=skill name
        // SkillData LIST {key="ignoreHexproof", value=true}
        "manual_1712" => {
            let skill = cap_str(caps, 1).trim();
            let value = "ignoreHexproof:true";
            vec![make_list_mod_with_tags(
                "SkillData",
                value,
                vec![ModTag::SkillName {
                    name: skill.to_string(),
                }],
                source,
            )]
        }

        // ── manual_1722 ─────────────────────────────────────────────────
        // Lua line 5145: (%a+) reserves no mana
        // Captures: 1=skill name
        // SkillData LIST {key="manaReservationFlat", value=0}
        // + SkillData LIST {key="lifeReservationFlat", value=0}
        "manual_1722" => {
            let skill = cap_str(caps, 1).trim();
            let tag = vec![ModTag::SkillName {
                name: skill.to_string(),
            }];
            vec![
                make_list_mod_with_tags("SkillData", "manaReservationFlat:0", tag.clone(), source),
                make_list_mod_with_tags("SkillData", "lifeReservationFlat:0", tag, source),
            ]
        }

        // ── manual_1723 ─────────────────────────────────────────────────
        // Lua line 5151: (%a+) has no reservation
        // Same as 1722
        "manual_1723" => {
            let skill = cap_str(caps, 1).trim();
            let tag = vec![ModTag::SkillName {
                name: skill.to_string(),
            }];
            vec![
                make_list_mod_with_tags("SkillData", "manaReservationFlat:0", tag.clone(), source),
                make_list_mod_with_tags("SkillData", "lifeReservationFlat:0", tag, source),
            ]
        }

        // ── manual_1724 ─────────────────────────────────────────────────
        // Lua line 5157: (%a+) has no reservation if cast as an aura
        // Same as 1722 + SkillType.Aura condition
        // Lua uses SkillType.Aura = 0x01 (PoB's aura skill type flag)
        "manual_1724" => {
            let skill = cap_str(caps, 1).trim();
            // Aura skill type constant from PoB data (SkillType.Aura)
            const SKILL_TYPE_AURA: u32 = 11;
            let tags = vec![
                ModTag::SkillName {
                    name: skill.to_string(),
                },
                ModTag::SkillType {
                    skill_type: SKILL_TYPE_AURA,
                },
            ];
            vec![
                make_list_mod_with_tags("SkillData", "manaReservationFlat:0", tags.clone(), source),
                make_list_mod_with_tags("SkillData", "lifeReservationFlat:0", tags, source),
            ]
        }

        // ── manual_1736 ─────────────────────────────────────────────────
        // Lua line 5186: travel skills other than (%a+) are disabled
        // Captures: 1=skill name
        // DisableSkill flag (for travel type) + EnableSkill flag (for named skill)
        "manual_1736" => {
            let skill = cap_str(caps, 1).trim();
            // Lua: flag("DisableSkill", { type="SkillType", skillType=SkillType.Travel })
            //      flag("EnableSkill", { type="SkillName", skillName=name })
            const SKILL_TYPE_TRAVEL: u32 = 51;
            vec![
                make_flag_with_tags(
                    "DisableSkill",
                    vec![ModTag::SkillType {
                        skill_type: SKILL_TYPE_TRAVEL,
                    }],
                    source,
                ),
                make_flag_with_tags(
                    "EnableSkill",
                    vec![ModTag::SkillName {
                        name: skill.to_string(),
                    }],
                    source,
                ),
            ]
        }

        // ═══════════════════════════════════════════════════════════════════
        // Helper handlers (shared templates used by generated code)
        // ═══════════════════════════════════════════════════════════════════

        // ── explodeFunc helpers ──────────────────────────────────────────
        // Most explode variants follow: explodeFunc(chance, amount, type, [extraTag])
        // The generated code passes different IDs for each variant, but the capture
        // groups always provide: 1=chance, 2=amount, 3=type (sometimes 2 captures for 100% variants)
        id if id.starts_with("helper_explodeFunc_") => {
            // Determine which variant: check if there's an extra condition
            let extra_tags = match id {
                "helper_explodeFunc_3" => vec![ModTag::Condition {
                    var: "AffectedByPride".into(),
                    neg: false,
                }],
                "helper_explodeFunc_4" => vec![ModTag::Condition {
                    var: "UsingFlask".into(),
                    neg: false,
                }],
                "helper_explodeFunc_5" => vec![ModTag::Condition {
                    var: "AffectedByGloriousMadness".into(),
                    neg: false,
                }],
                "helper_explodeFunc_7" => vec![ModTag::Condition {
                    var: "UsingWand".into(),
                    neg: false,
                }],
                "helper_explodeFunc_8" => vec![ModTag::ActorCondition {
                    actor: "enemy".into(),
                    var: "Cursed".into(),
                    neg: false,
                }],
                "helper_explodeFunc_11" => vec![ModTag::ActorCondition {
                    actor: "enemy".into(),
                    var: "OnFungalGround".into(),
                    neg: false,
                }],
                "helper_explodeFunc_13" => vec![ModTag::ActorCondition {
                    actor: "enemy".into(),
                    var: "Shocked".into(),
                    neg: false,
                }],
                "helper_explodeFunc_14" => vec![ModTag::ActorCondition {
                    actor: "enemy".into(),
                    var: "Ignited".into(),
                    neg: false,
                }],
                "helper_explodeFunc_15" => vec![ModTag::ActorCondition {
                    actor: "enemy".into(),
                    var: "Bleeding".into(),
                    neg: false,
                }],
                "helper_explodeFunc_16" => vec![ModTag::ActorCondition {
                    actor: "enemy".into(),
                    var: "Burning".into(),
                    neg: false,
                }],
                _ => vec![],
            };

            // Some variants use 100% chance (caps only have 2 groups: amount, type).
            // Others have 3 captures: chance, amount, type.
            // For the randomElement variant (4), caps have 2 groups: chance, amount.
            let is_100_percent = matches!(
                id,
                "helper_explodeFunc_9"
                    | "helper_explodeFunc_10"
                    | "helper_explodeFunc_11"
                    | "helper_explodeFunc_12"
                    | "helper_explodeFunc_13"
                    | "helper_explodeFunc_14"
                    | "helper_explodeFunc_15"
                    | "helper_explodeFunc_16"
                    | "helper_explodeFunc_19"
                    | "helper_explodeFunc_20"
                    | "helper_explodeFunc_21"
                    | "helper_explodeFunc_22"
            );

            if id == "helper_explodeFunc_4" {
                // randomElement variant: 2 caps = chance, amount
                let chance = cap_num(caps, 1);
                let amount = cap_str(caps, 2);
                explode_func(chance, amount, "randomElement", extra_tags, source)
            } else if is_100_percent {
                // 100% variants: 2 captures = amount, type
                let amount = cap_str(caps, 1);
                let dmg_type = cap_str(caps, 2);
                explode_func(100.0, amount, dmg_type, extra_tags, source)
            } else {
                // Standard: 3 captures = chance, amount, type
                let chance = cap_num(caps, 1);
                let amount = cap_str(caps, 2);
                let dmg_type = cap_str(caps, 3);
                explode_func(chance, amount, dmg_type, extra_tags, source)
            }
        }

        // ── triggerExtraSkill helpers ────────────────────────────────────
        // Pattern: captures vary — generally 1=skill/level, 2=skill/level
        // The generated code passes skill name + level in captures
        id if id.starts_with("helper_triggerExtraSkill_") => {
            // Most trigger patterns have captures: (skill, num) or (num, skill)
            // or (level, skill1, skill2, ...) depending on the regex
            // We check capture 1: if it parses as a number, it's (level, skill);
            // otherwise it's (skill, level)
            let c1 = cap_str(caps, 1);
            let c2 = cap_str(caps, 2);

            let (skill, level) = if let Ok(num) = c1.parse::<f64>() {
                // cap1 is level, cap2 is skill name
                (c2, num)
            } else if let Ok(num) = c2.parse::<f64>() {
                // cap1 is skill, cap2 is level
                (c1, num)
            } else {
                // Fallback: cap1 is skill, level=1
                (c1, 1.0)
            };

            // Check if this is a noSupports variant
            let extra_tags = vec![];
            trigger_extra_skill(skill, level, extra_tags, source)
        }

        // ── grantedExtraSkill helpers ───────────────────────────────────
        id if id.starts_with("helper_grantedExtraSkill_") => {
            let c1 = cap_str(caps, 1);
            let c2 = cap_str(caps, 2);

            let (skill, level) = if let Ok(num) = c1.parse::<f64>() {
                (c2, num)
            } else if let Ok(num) = c2.parse::<f64>() {
                (c1, num)
            } else {
                (c1, 1.0)
            };

            granted_extra_skill(skill, level, vec![], source)
        }

        // ── extraSupport helpers ────────────────────────────────────────
        id if id.starts_with("helper_extraSupport_") => {
            let c1 = cap_str(caps, 1);
            let c2 = cap_str(caps, 2);

            let (support, level) = if let Ok(num) = c1.parse::<f64>() {
                (c2, num)
            } else if let Ok(num) = c2.parse::<f64>() {
                (c1, num)
            } else {
                (c1, 1.0)
            };

            extra_support(support, level, vec![], source)
        }

        // ═══════════════════════════════════════════════════════════════════
        // Fallback — unknown handler
        // ═══════════════════════════════════════════════════════════════════
        _ => {
            // Unknown manual handler — return empty
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    fn src() -> ModSource {
        ModSource::new("Test", "test")
    }

    #[test]
    fn test_first_to_upper() {
        assert_eq!(first_to_upper("hello"), "Hello");
        assert_eq!(first_to_upper(""), "");
        assert_eq!(first_to_upper("a"), "A");
        assert_eq!(first_to_upper("Hello"), "Hello");
    }

    #[test]
    fn test_title_case() {
        assert_eq!(title_case("fire damage"), "FireDamage");
        assert_eq!(title_case("cold"), "Cold");
        assert_eq!(
            title_case("physical damage reduction"),
            "PhysicalDamageReduction"
        );
    }

    #[test]
    fn test_manual_6_explode() {
        let re = Regex::new(r"^enemies killed with attack hits have a (\d+)% chance to explode, dealing a (.+) of their life as (.+) damage$").unwrap();
        let text = "enemies killed with attack hits have a 20% chance to explode, dealing a tenth of their life as fire damage";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_6", &caps, &src());
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0].name, "ExplodeMod");
        assert_eq!(mods[0].mod_type, ModType::List);
        assert_eq!(mods[1].name, "CanExplode");
        assert_eq!(mods[1].mod_type, ModType::Flag);
    }

    #[test]
    fn test_manual_116_gain_no() {
        let re = Regex::new(r"^gain no (.+) from equipped (.+)$").unwrap();
        let text = "gain no defences from equipped shield";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_116", &caps, &src());
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "GainNoDefencesFromWeapon2");
        assert_eq!(mods[0].mod_type, ModType::Flag);
        assert!(
            matches!(&mods[0].tags[0], ModTag::SlotName { slot_name, .. } if slot_name == "Weapon 2")
        );
    }

    #[test]
    fn test_manual_311_ailments() {
        let re = Regex::new(r"^non-damaging ailments have (\d+)% reduced effect on you while you have arcane surge$").unwrap();
        let text =
            "non-damaging ailments have 50% reduced effect on you while you have arcane surge";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_311", &caps, &src());
        assert_eq!(mods.len(), 6);
        assert_eq!(mods[0].name, "SelfChillEffect");
        assert_eq!(mods[0].mod_type, ModType::Inc);
        assert!((mods[0].value.as_f64() - (-50.0)).abs() < 0.001);
        assert_eq!(mods[5].name, "SelfSapEffect");
    }

    #[test]
    fn test_manual_417_noop() {
        let re = Regex::new(r"^reflects your opposite ring$").unwrap();
        let text = "reflects your opposite ring";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_417", &caps, &src());
        assert!(mods.is_empty());
    }

    #[test]
    fn test_manual_493_noop() {
        let re =
            Regex::new(r"^socketed non-channelling bow skills are triggered by snipe$").unwrap();
        let text = "socketed non-channelling bow skills are triggered by snipe";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_493", &caps, &src());
        assert!(mods.is_empty());
    }

    #[test]
    fn test_manual_469_gem_property() {
        let re =
            Regex::new(r"^([+-]\d+)%? to ([a-zA-Z]+) of socketed ?([a-zA-Z\- ]*) gems$").unwrap();
        let text = "+2 to level of socketed fire gems";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_469", &caps, &src());
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "GemProperty");
        assert_eq!(mods[0].mod_type, ModType::List);
    }

    #[test]
    fn test_manual_581_curse_with_effect() {
        let re =
            Regex::new(r"^curse enemies with (\D+) on [a-zA-Z]+, with (\d+)% increased effect$")
                .unwrap();
        let text = "curse enemies with vulnerability on hit, with 20% increased effect";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_581", &caps, &src());
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0].name, "ExtraSkill");
        assert_eq!(mods[1].name, "CurseEffect");
        assert_eq!(mods[1].mod_type, ModType::Inc);
        assert!((mods[1].value.as_f64() - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_manual_582_curse_reduced_effect() {
        let re = Regex::new(r"^curse enemies with (\D+) on [a-zA-Z]+, with (\d+)% reduced effect$")
            .unwrap();
        let text = "curse enemies with vulnerability on hit, with 20% reduced effect";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_582", &caps, &src());
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[1].name, "CurseEffect");
        assert!((mods[1].value.as_f64() - (-20.0)).abs() < 0.001);
    }

    #[test]
    fn test_manual_605_trigger_three_skills() {
        let re =
            Regex::new(r"^trigger level (\d+) (.+), (.+) or (.+) every (\d+) seconds$").unwrap();
        let text = "trigger level 20 ice nova, shock nova or fire nova every 5 seconds";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_605", &caps, &src());
        assert_eq!(mods.len(), 3);
        for m in &mods {
            assert_eq!(m.name, "ExtraSkill");
            assert_eq!(m.mod_type, ModType::List);
        }
    }

    #[test]
    fn test_manual_941_cursed_with_level() {
        let re = Regex::new(r"^you are cursed with level (\d+) (\D+)$").unwrap();
        let text = "you are cursed with level 10 vulnerability";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_941", &caps, &src());
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "ExtraCurse");
        assert_eq!(mods[0].mod_type, ModType::List);
    }

    #[test]
    fn test_manual_942_cursed_with() {
        let re = Regex::new(r"^you are cursed with (\D+)$").unwrap();
        let text = "you are cursed with vulnerability";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_942", &caps, &src());
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "ExtraCurse");
    }

    #[test]
    fn test_manual_943_cursed_with_effect() {
        let re = Regex::new(r"^you are cursed with (\D+), with (\d+)% increased effect$").unwrap();
        let text = "you are cursed with vulnerability, with 30% increased effect";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_943", &caps, &src());
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0].name, "ExtraCurse");
        assert_eq!(mods[1].name, "CurseEffectAgainstPlayer");
        assert!((mods[1].value.as_f64() - 30.0).abs() < 0.001);
    }

    #[test]
    fn test_manual_1057_buff_effect() {
        let re = Regex::new(r"^([a-zA-Z\s]+) has (\d+)% increased effect$").unwrap();
        let text = "hatred has 25% increased effect";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_1057", &caps, &src());
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "BuffEffect");
        assert_eq!(mods[0].mod_type, ModType::Inc);
        assert!((mods[0].value.as_f64() - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_manual_1559_flask_curse() {
        let re = Regex::new(r"^grants level (\d+) (.+) curse aura during flask effect$").unwrap();
        let text = "grants level 5 despair curse aura during flask effect";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_1559", &caps, &src());
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "ExtraCurse");
    }

    #[test]
    fn test_manual_1712_hexproof() {
        let re = Regex::new(r"^([a-zA-Z\s]+) can affect hexproof enemies$").unwrap();
        let text = "despair can affect hexproof enemies";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_1712", &caps, &src());
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "SkillData");
        assert_eq!(mods[0].mod_type, ModType::List);
    }

    #[test]
    fn test_manual_1722_reserves_no_mana() {
        let re = Regex::new(r"^([a-zA-Z\s]+) reserves no mana$").unwrap();
        let text = "hatred reserves no mana";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_1722", &caps, &src());
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0].name, "SkillData");
        assert_eq!(mods[1].name, "SkillData");
    }

    #[test]
    fn test_manual_1723_no_reservation() {
        let re = Regex::new(r"^([a-zA-Z\s]+) has no reservation$").unwrap();
        let text = "hatred has no reservation";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_1723", &caps, &src());
        assert_eq!(mods.len(), 2);
    }

    #[test]
    fn test_manual_1724_no_reservation_aura() {
        let re = Regex::new(r"^([a-zA-Z\s]+) has no reservation if cast as an aura$").unwrap();
        let text = "hatred has no reservation if cast as an aura";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_1724", &caps, &src());
        assert_eq!(mods.len(), 2);
        // Should have SkillType tag
        assert!(mods[0]
            .tags
            .iter()
            .any(|t| matches!(t, ModTag::SkillType { .. })));
    }

    #[test]
    fn test_manual_1736_travel_skills() {
        let re = Regex::new(r"^travel skills other than ([a-zA-Z\s]+) are disabled$").unwrap();
        let text = "travel skills other than leap slam are disabled";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("manual_1736", &caps, &src());
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0].name, "DisableSkill");
        assert_eq!(mods[0].mod_type, ModType::Flag);
        assert_eq!(mods[1].name, "EnableSkill");
        assert_eq!(mods[1].mod_type, ModType::Flag);
    }

    #[test]
    fn test_helper_explode_basic() {
        let re = Regex::new(r"^enemies you kill have a (\d+)% chance to explode, dealing a (.+) of their maximum life as (.+) damage$").unwrap();
        let text = "enemies you kill have a 20% chance to explode, dealing a tenth of their maximum life as physical damage";
        let caps = re.captures(text).unwrap();
        let mods = handle_manual_special("helper_explodeFunc_0", &caps, &src());
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0].name, "ExplodeMod");
        assert_eq!(mods[1].name, "CanExplode");
    }

    #[test]
    fn test_implemented_manual_count() {
        assert_eq!(IMPLEMENTED_MANUAL_COUNT, 25);
    }
}
