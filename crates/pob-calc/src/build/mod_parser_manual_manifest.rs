//! Auto-generated manifest of specialModList entries requiring manual handlers.
//!
//! Each entry lists: handler ID, regex pattern, capture count, Lua source line.
//! Implement handlers in mod_parser_manual.rs.

/// Number of entries requiring manual handlers.
pub const MANUAL_ENTRY_COUNT: usize = 31;

/// Manual handler stubs.
/// Format: (handler_id, regex_pattern, capture_count, lua_line)
pub const MANUAL_ENTRIES: &[(&str, &str, usize, usize)] = &[
    // Lua line 2050: explodeFunc
    ("manual_6", r#"^enemies killed with attack hits have a (\d+)% chance to explode, dealing a (.+) of their life as (.+) damage$"#, 3, 2050),
    // Lua line 2277: function(_, stat, slot)\n\t\tif slot == \"shield\" then slot = \"Weapon 2\" end
    ("manual_116", r#"^gain no (.+) from equipped (.+)$"#, 2, 2277),
    // Lua line 2519: function(_, radius, dmgType) return {\n\t\tmod(\"ExtraJewelFunc\", \"LIST\", {radius = firstToUpper(radius), type = \"Other\", fu
    ("manual_226", r#"^non\-unique jewels cause increases and reductions to other damage types in a ([a-zA-Z]+) radius to be transformed to apply to ([a-zA-Z]+) damage$"#, 2, 2519),
    // Lua line 2527: function(_, radius, val, attr) return {\n\t\tmod(\"ExtraJewelFunc\", \"LIST\", {radius = (radius:gsub(\"^%l\", string.upper)), ty
    ("manual_227", r#"^non\-unique jewels cause small and notable passive skills in a ([a-zA-Z]+) radius to also grant \+(\d+) to ([a-zA-Z]+)$"#, 3, 2527),
    // Lua line 2772: function(num)\n\t\tlocal mods = { }\n\t\tfor i, ailment in ipairs(data.nonDamagingAilmentTypeList) do\n\t\t\tmods[i] = mod(\"Self\".
    ("manual_311", r#"^non\-damaging ailments have (\d+)% reduced effect on you while you have arcane surge$"#, 1, 2772),
    // Lua line 2966: {\n\t\t\n\t}
    ("manual_417", r#"^reflects your o[tp][hp][eo][rs]i?t?e? ring$"#, 0, 2966),
    // Lua line 3045: function(num, _, property, type)\n\t\tif type == \"\" then type = \"all\" end
    ("manual_469", r#"^([+\-]\d+)%? to ([a-zA-Z]+) of socketed ?([a-zA-Z\- ]*) gems$"#, 3, 3045),
    // Lua line 3078: {\n\t}
    ("manual_493", r#"^socketed non\-channelling bow skills are triggered by snipe$"#, 0, 3078),
    // Lua line 3094: function(num, _, property, skill)\n\t\tif gemIdLookup[skill] then\n\t\t\treturn { mod(\"GemProperty\", \"LIST\", {keyword = skill, 
    ("manual_501", r#"^([+\-]\d+)%? to ([a-zA-Z]+) of all (.+) gems$"#, 3, 3094),
    // Lua line 3203: function(_, skill, num) return {\n\t\tmod(\"ExtraSkill\", \"LIST\", { skillId = gemIdLookup[skill], level = 1, noSupports = tru
    ("manual_581", r#"^curse enemies with (\D+) on [a-zA-Z]+, with (\d+)% increased effect$"#, 2, 3203),
    // Lua line 3207: function(_, skill, num) return {\n\t\tmod(\"ExtraSkill\", \"LIST\", { skillId = gemIdLookup[skill], level = 1, noSupports = tru
    ("manual_582", r#"^curse enemies with (\D+) on [a-zA-Z]+, with (\d+)% reduced effect$"#, 2, 3207),
    // Lua line 3211: function(_, skill, num) return {\n\t\tmod(\"ExtraSkill\", \"LIST\", { skillId = gemIdLookup[skill], level = 1, noSupports = tru
    ("manual_583", r#"^\d+% chance to curse n?o?n?\-?c?u?r?s?e?d? ?enemies with (\D+) on [a-zA-Z]+, with (\d+)% increased effect$"#, 2, 3211),
    // Lua line 3215: function(_, skill) return {\n\t\tmod(\"ExtraSkill\", \"LIST\", { skillId = gemIdLookup[skill], level = 1, noSupports = true, tr
    ("manual_584", r#"^\d+% chance to curse n?o?n?\-?c?u?r?s?e?d? ?enemies with (\D+) on [a-zA-Z]+$"#, 1, 3215),
    // Lua line 3236: function (_, itemSlotName)\n\t\tlocal targetItemSlotName = \"Body Armour\"\n\t\tif itemSlotName == \"main hand\" then\n\t\t\ttargetIte
    ("manual_601", r#"^socketed support gems can also support skills from y?o?u?r? ?e?q?u?i?p?p?e?d? ?([a-zA-Z\s]+)$"#, 1, 3236),
    // Lua line 3248: function(num, _, skill1, skill2, skill3) return {\n\t\tmod(\"ExtraSkill\", \"LIST\", { skillId = gemIdLookup[skill1], level = n
    ("manual_605", r#"^trigger level (\d+) (.+), (.+) or (.+) every (\d+) seconds$"#, 5, 3248),
    // Lua line 3920: function(num, _, name) return { mod(\"ExtraCurse\", \"LIST\", { skillId = gemIdLookup[name], level = num, applyToPlayer = tr
    ("manual_941", r#"^you are cursed with level (\d+) (\D+)$"#, 2, 3920),
    // Lua line 3921: function(_, skill) return { mod(\"ExtraCurse\", \"LIST\", { skillId = gemIdLookup[skill], level = 1, applyToPlayer = true })
    ("manual_942", r#"^you are cursed with (\D+)$"#, 1, 3921),
    // Lua line 3922: function(_, skill, num) return {\n\t\tmod(\"ExtraCurse\", \"LIST\", { skillId = gemIdLookup[skill], level = 1, applyToPlayer = 
    ("manual_943", r#"^you are cursed with (\D+), with (\d+)% increased effect$"#, 2, 3922),
    // Lua line 4133: function(_, skill, num) return { mod(\"BuffEffect\", \"INC\", num, { type = \"SkillId\", skillId = gemIdLookup[skill]}) } end
    ("manual_1057", r#"^([a-zA-Z\s]+) has (\d+)% increased effect$"#, 2, 4133),
    // Lua line 4893: function(num, _, skill) return { mod(\"ExtraCurse\", \"LIST\", { skillId = gemIdLookup[skill:gsub(\" skill\",\"\")] or \"Unknown\"
    ("manual_1559", r#"^grants level (\d+) (.+) curse aura during f?l?a?s?k? ?effect$"#, 2, 4893),
    // Lua line 5127: function(_, name) return {\n\t\tmod(\"SkillData\", \"LIST\", { key = \"ignoreHexproof\", value = true }, { type = \"SkillId\", skil
    ("manual_1712", r#"^([a-zA-Z\s]+) can affect hexproof enemies$"#, 1, 5127),
    // Lua line 5145: function(_, name) return {\n\t\tmod(\"SkillData\", \"LIST\", { key = \"manaReservationFlat\", value = 0 }, { type = \"SkillId\", sk
    ("manual_1722", r#"^([a-zA-Z\s]+) reserves no mana$"#, 1, 5145),
    // Lua line 5151: function(_, name) return {\n\t\tmod(\"SkillData\", \"LIST\", { key = \"manaReservationFlat\", value = 0 }, { type = \"SkillId\", sk
    ("manual_1723", r#"^([a-zA-Z\s]+) has no reservation$"#, 1, 5151),
    // Lua line 5157: function(_, name) return {\n\t\tmod(\"SkillData\", \"LIST\", { key = \"manaReservationFlat\", value = 0 }, { type = \"SkillId\", sk
    ("manual_1724", r#"^([a-zA-Z\s]+) has no reservation if cast as an aura$"#, 1, 5157),
    // Lua line 5186: function(_, name) return {\n\t\tflag(\"DisableSkill\", { type = \"SkillType\", skillType = SkillType.Travel }),\n\t\tflag(\"EnableS
    ("manual_1736", r#"^travel skills other than ([a-zA-Z\s]+) are disabled$"#, 1, 5186),
    // Lua line 5592: function(num, _, name)\n\t\treturn { mod(\"JewelData\", \"LIST\",\n\t\t\t\t{ key = \"conqueredBy\", value = { id = num, conqueror = co
    ("manual_1976", r#"^bathed in the blood of (\d+) sacrificed in the name of (.+)$"#, 2, 5592),
    // Lua line 5595: function(num, _, name)\n\t\treturn { mod(\"JewelData\", \"LIST\",\n\t\t\t\t{ key = \"conqueredBy\", value = { id = num, conqueror = co
    ("manual_1977", r#"^carved to glorify (\d+) new faithful converted by high templar (.+)$"#, 2, 5595),
    // Lua line 5598: function(num, _, name)\n\t\treturn { mod(\"JewelData\", \"LIST\",\n\t\t\t\t{ key = \"conqueredBy\", value = { id = num, conqueror = co
    ("manual_1978", r#"^commanded leadership over (\d+) warriors under (.+)$"#, 2, 5598),
    // Lua line 5601: function(num, _, name)\n\t\treturn { mod(\"JewelData\", \"LIST\",\n\t\t\t\t{ key = \"conqueredBy\", value = { id = num, conqueror = co
    ("manual_1979", r#"^commissioned (\d+) coins to commemorate (.+)$"#, 2, 5601),
    // Lua line 5604: function(num, _, name)\n\t\treturn { mod(\"JewelData\", \"LIST\",\n\t\t\t\t{ key = \"conqueredBy\", value = { id = num, conqueror = co
    ("manual_1980", r#"^denoted service of (\d+) dekhara in the akhara of (.+)$"#, 2, 5604),
    // Lua line 5607: function(num, _, name)\n\t\treturn { mod(\"JewelData\", \"LIST\",\n\t\t\t\t{ key = \"conqueredBy\", value = { id = num, conqueror = co
    ("manual_1981", r#"^remembrancing (\d+) songworthy deeds by the line of (.+)$"#, 2, 5607),
];
