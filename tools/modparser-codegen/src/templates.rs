//! Classifies specialModList function entries into templates.

use crate::types::*;

/// Classify a specialModList value (table or function body) into a template.
///
/// `value_source` is the raw Lua source for the value (after the `=` sign).
/// `is_function` is true if the value starts with `function(` or `function (`.
/// `line_number` is the source line for diagnostics.
pub fn classify_special_mod(
    value_source: &str,
    is_function: bool,
    line_number: usize,
) -> SpecialModTemplate {
    let trimmed = value_source.trim();

    if !is_function {
        return classify_static_table(trimmed, line_number);
    }

    // Check for complex markers first — these always need manual handling
    if is_complex_function(trimmed) {
        return SpecialModTemplate::ManualRequired {
            lua_body: trimmed.to_string(),
            line_number,
        };
    }

    classify_function(trimmed, line_number)
}

/// Detect whether a function body contains markers that indicate complex logic
/// requiring manual implementation.
fn is_complex_function(source: &str) -> bool {
    // We need to be careful about word boundaries. For example, "if " at the start
    // of a line or after whitespace is a conditional, but "life" contains "if" as a
    // substring. We check for " if " (with spaces), or "if " at the start.
    //
    // Similarly "for " loops vs words containing "for".
    let complex_markers = [
        // Conditionals - check word boundaries
        "\nif ",
        "\n\tif ",
        " if ",
        // Loops
        "\nfor ",
        "\n\tfor ",
        " for ",
        " while ",
        // Iteration
        "ipairs(",
        "pairs(",
        // External data
        "data.",
        "gemIdLookup[",
        // List building
        "table.insert",
        "t_insert",
        // Closures
        "function(node",
        // Jewel helpers
        "getSimpleConv",
        "getPerStat",
        "getThreshold",
        "ExtraJewelFunc",
        // String iteration
        "gmatch",
        // Timeless jewel conqueror lookup (requires conquerorList table)
        "conquerorList[",
    ];

    for marker in &complex_markers {
        if source.contains(marker) {
            return true;
        }
    }

    // Special case: check if source starts with "if " (no leading space/newline)
    if source.starts_with("if ") {
        return true;
    }
    if source.starts_with("for ") {
        return true;
    }

    false
}

/// Classify a static table value (not a function).
fn classify_static_table(source: &str, line_number: usize) -> SpecialModTemplate {
    let trimmed = source.trim();

    // Empty table
    if trimmed == "{}" || trimmed == "{ }" {
        return SpecialModTemplate::StaticMods(vec![]);
    }

    // Check for EnemyModifier
    if trimmed.contains("\"EnemyModifier\"") {
        return SpecialModTemplate::EnemyModifier(parse_mod_calls(trimmed));
    }

    // Check for MinionModifier
    if trimmed.contains("\"MinionModifier\"") {
        return SpecialModTemplate::MinionModifier(parse_mod_calls(trimmed));
    }

    // Static table with mod/flag calls
    if trimmed.contains("mod(") || trimmed.contains("flag(") {
        return SpecialModTemplate::StaticMods(parse_mod_calls(trimmed));
    }

    // Anything else that's not a function but we can't classify
    SpecialModTemplate::ManualRequired {
        lua_body: trimmed.to_string(),
        line_number,
    }
}

/// Classify a function value that has passed the complexity check.
fn classify_function(source: &str, line_number: usize) -> SpecialModTemplate {
    // Helper calls — check most specific first
    if source.contains("grantedExtraSkill(") {
        return SpecialModTemplate::HelperCall {
            helper: "grantedExtraSkill".to_string(),
            args: extract_helper_args(source, "grantedExtraSkill"),
        };
    }
    if source.contains("triggerExtraSkill(") {
        return SpecialModTemplate::HelperCall {
            helper: "triggerExtraSkill".to_string(),
            args: extract_helper_args(source, "triggerExtraSkill"),
        };
    }
    if source.contains("extraSupport(") {
        return SpecialModTemplate::HelperCall {
            helper: "extraSupport".to_string(),
            args: extract_helper_args(source, "extraSupport"),
        };
    }
    if source.contains("explodeFunc(")
        || source.contains("explodeFunc ")
        || source.contains("explodeFunc,")
    {
        return SpecialModTemplate::HelperCall {
            helper: "explodeFunc".to_string(),
            args: extract_helper_args(source, "explodeFunc"),
        };
    }

    // Damage conversion: firstToUpper + ConvertTo
    if source.contains("firstToUpper") && source.contains("ConvertTo") {
        return SpecialModTemplate::DamageConversion {
            stat_prefix: extract_stat_prefix(source, "ConvertTo"),
            capture_index: 0,
        };
    }

    // Damage gain as: firstToUpper + GainAs
    if source.contains("firstToUpper") && source.contains("GainAs") {
        return SpecialModTemplate::DamageGainAs {
            stat_prefix: extract_stat_prefix(source, "GainAs"),
            capture_index: 0,
        };
    }

    // Simple function: contains mod/flag calls with no complex logic
    if source.contains("mod(") || source.contains("flag(") {
        return SpecialModTemplate::SimpleFn(parse_mod_calls(source));
    }

    // Everything else
    SpecialModTemplate::ManualRequired {
        lua_body: source.to_string(),
        line_number,
    }
}

/// Extract helper function arguments as raw strings.
fn extract_helper_args(source: &str, helper: &str) -> Vec<String> {
    let pattern = format!("{}(", helper);
    if let Some(pos) = source.find(&pattern) {
        let after = &source[pos + pattern.len()..];
        // Find matching closing paren
        let mut depth = 1i32;
        let mut end = 0;
        for (i, ch) in after.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let args_str = &after[..end];
        // Split on commas at depth 0
        split_lua_args(args_str)
    } else {
        vec![]
    }
}

/// Split a Lua argument list by commas at depth 0.
fn split_lua_args(s: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    let mut in_string = false;

    for (i, ch) in s.char_indices() {
        match ch {
            '"' if !in_string => in_string = true,
            '"' if in_string => in_string = false,
            '{' | '(' if !in_string => depth += 1,
            '}' | ')' if !in_string => depth -= 1,
            ',' if depth == 0 && !in_string => {
                let arg = s[start..i].trim().to_string();
                if !arg.is_empty() {
                    args.push(arg);
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = s[start..].trim().to_string();
    if !last.is_empty() {
        args.push(last);
    }
    args
}

/// Try to extract a stat prefix from a damage conversion/gain pattern.
fn extract_stat_prefix(source: &str, keyword: &str) -> String {
    // Look for patterns like `"PhysicalDamageConvertTo"` or dynamic construction
    if let Some(pos) = source.find(keyword) {
        // Walk backwards to find the stat prefix
        let before = &source[..pos];
        // Look for a quoted string ending just before the keyword
        if let Some(quote_end) = before.rfind('"') {
            let before_quote = &before[..quote_end];
            if let Some(quote_start) = before_quote.rfind('"') {
                return before[quote_start + 1..quote_end].to_string();
            }
        }
    }
    String::new()
}

/// Parse mod() and flag() calls from Lua source text.
///
/// Extracts basic structure: name, type, value. This is a rough extraction —
/// it handles the common patterns found in ModParser.lua.
fn parse_mod_calls(source: &str) -> Vec<LuaModCall> {
    let mut calls = Vec::new();
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Look for `mod(` or `flag(`
        let is_mod = i + 4 <= len && &source[i..i + 4] == "mod(";
        let is_flag = i + 5 <= len && &source[i..i + 5] == "flag(";

        if !is_mod && !is_flag {
            i += 1;
            continue;
        }

        // Ensure it's at a word boundary (not part of a longer identifier like "modSource")
        if i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_') {
            i += 1;
            continue;
        }

        let call_start = if is_mod { i + 4 } else { i + 5 };

        // Find matching closing paren
        let mut depth = 1i32;
        let mut j = call_start;
        let mut in_str = false;
        while j < len && depth > 0 {
            if bytes[j] == b'"' {
                in_str = !in_str;
            } else if !in_str {
                if bytes[j] == b'(' || bytes[j] == b'{' {
                    depth += 1;
                } else if bytes[j] == b')' || bytes[j] == b'}' {
                    depth -= 1;
                }
            }
            if depth > 0 {
                j += 1;
            }
        }

        let args_str = &source[call_start..j];
        let args = split_lua_args(args_str);

        if is_flag {
            // flag("Name", ...) or flag("Name")
            let name = args
                .first()
                .map(|s| s.trim_matches('"').to_string())
                .unwrap_or_default();
            let dynamic_name = args
                .first()
                .map(|s| s.contains("..") || s.contains("firstToUpper"))
                .unwrap_or(false);
            calls.push(LuaModCall {
                name,
                mod_type: "FLAG".to_string(),
                value: "true".to_string(),
                flags: args.get(1).cloned(),
                keyword_flags: args.get(2).cloned(),
                tags: vec![],
                dynamic_name,
            });
        } else {
            // mod("Name", "TYPE", value, ...)
            let raw_name = args.first().cloned().unwrap_or_default();
            let name = raw_name.trim_matches('"').to_string();
            let dynamic_name = raw_name.contains("..") || raw_name.contains("firstToUpper");
            let mod_type = args
                .get(1)
                .map(|s| s.trim_matches('"').to_string())
                .unwrap_or_default();
            let value = args.get(2).cloned().unwrap_or_default();

            calls.push(LuaModCall {
                name,
                mod_type,
                value,
                flags: args.get(3).cloned(),
                keyword_flags: args.get(4).cloned(),
                tags: vec![],
                dynamic_name,
            });
        }

        i = j + 1;
    }

    calls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_empty_table() {
        let t = classify_special_mod("{ }", false, 0);
        assert!(matches!(
            t,
            SpecialModTemplate::StaticMods(ref v) if v.is_empty()
        ));
    }

    #[test]
    fn classifies_empty_table_no_space() {
        let t = classify_special_mod("{}", false, 0);
        assert!(matches!(
            t,
            SpecialModTemplate::StaticMods(ref v) if v.is_empty()
        ));
    }

    #[test]
    fn classifies_static_flag_table() {
        let t = classify_special_mod(r#"{ flag("CannotBeEvaded") }"#, false, 0);
        assert!(matches!(t, SpecialModTemplate::StaticMods(_)));
        if let SpecialModTemplate::StaticMods(ref calls) = t {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].name, "CannotBeEvaded");
            assert_eq!(calls[0].mod_type, "FLAG");
        }
    }

    #[test]
    fn classifies_static_multi_mod() {
        let t = classify_special_mod(
            r#"{ mod("AttackDodgeChance", "BASE", 30), mod("Armour", "MORE", -50) }"#,
            false,
            0,
        );
        assert!(matches!(t, SpecialModTemplate::StaticMods(_)));
        if let SpecialModTemplate::StaticMods(ref calls) = t {
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0].name, "AttackDodgeChance");
            assert_eq!(calls[1].name, "Armour");
        }
    }

    #[test]
    fn classifies_enemy_modifier() {
        let t = classify_special_mod(
            r#"{ mod("EnemyModifier", "LIST", { mod = mod("FireResist", "BASE", -10) }) }"#,
            false,
            0,
        );
        assert!(matches!(t, SpecialModTemplate::EnemyModifier(_)));
    }

    #[test]
    fn classifies_minion_modifier() {
        let t = classify_special_mod(
            r#"{ mod("MinionModifier", "LIST", { mod = flag("CannotBeEvaded") }) }"#,
            false,
            0,
        );
        assert!(matches!(t, SpecialModTemplate::MinionModifier(_)));
    }

    #[test]
    fn classifies_simple_function() {
        let t = classify_special_mod(
            r#"function(num) return { mod("CritChance", "INC", num) } end"#,
            true,
            0,
        );
        assert!(matches!(t, SpecialModTemplate::SimpleFn(_)));
        if let SpecialModTemplate::SimpleFn(ref calls) = t {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].name, "CritChance");
        }
    }

    #[test]
    fn classifies_granted_skill() {
        let t = classify_special_mod(
            r#"function(num, _, skill) return grantedExtraSkill(skill, num) end"#,
            true,
            0,
        );
        assert!(matches!(
            t,
            SpecialModTemplate::HelperCall { ref helper, .. } if helper == "grantedExtraSkill"
        ));
    }

    #[test]
    fn classifies_trigger_skill() {
        let t = classify_special_mod(
            r#"function(num, _, skill) return triggerExtraSkill(skill, num) end"#,
            true,
            0,
        );
        assert!(matches!(
            t,
            SpecialModTemplate::HelperCall { ref helper, .. } if helper == "triggerExtraSkill"
        ));
    }

    #[test]
    fn classifies_extra_support() {
        let t = classify_special_mod(
            r#"function(num, _, support) return extraSupport(support, num) end"#,
            true,
            0,
        );
        assert!(matches!(
            t,
            SpecialModTemplate::HelperCall { ref helper, .. } if helper == "extraSupport"
        ));
    }

    #[test]
    fn classifies_explode_func() {
        let t = classify_special_mod(
            r#"function(chance, _, amount, type) return explodeFunc(chance, amount, type) end"#,
            true,
            0,
        );
        assert!(matches!(
            t,
            SpecialModTemplate::HelperCall { ref helper, .. } if helper == "explodeFunc"
        ));
    }

    #[test]
    fn classifies_bare_explode_func() {
        // The bare `explodeFunc` reference (not a function call)
        let t = classify_special_mod("explodeFunc", false, 0);
        assert!(
            matches!(t, SpecialModTemplate::HelperCall { ref helper, .. } if helper == "explodeFunc")
                || matches!(t, SpecialModTemplate::ManualRequired { .. }),
            "bare explodeFunc should be classified as HelperCall or ManualRequired, got {:?}",
            t
        );
    }

    #[test]
    fn classifies_damage_conversion() {
        let t = classify_special_mod(
            r#"function(num, _, type) return { mod("PhysicalDamageConvertTo"..firstToUpper(type), "BASE", num) } end"#,
            true,
            0,
        );
        assert!(matches!(t, SpecialModTemplate::DamageConversion { .. }));
    }

    #[test]
    fn classifies_damage_gain_as() {
        let t = classify_special_mod(
            r#"function(num, _, type) return { mod("PhysicalDamageGainAs"..firstToUpper(type), "BASE", num) } end"#,
            true,
            0,
        );
        assert!(matches!(t, SpecialModTemplate::DamageGainAs { .. }));
    }

    #[test]
    fn classifies_complex_as_manual() {
        let t = classify_special_mod(
            r#"function(num) local mods = {} for i, ailment in ipairs(data.nonDamagingAilmentTypeList) do mods[i] = mod("Self"..ailment.."Effect", "INC", -num) end return mods end"#,
            true,
            0,
        );
        assert!(matches!(t, SpecialModTemplate::ManualRequired { .. }));
    }

    #[test]
    fn classifies_jewel_func_as_manual() {
        let t = classify_special_mod(
            r#"function(_, radius) return { mod("ExtraJewelFunc", "LIST", {radius = firstToUpper(radius)}) } end"#,
            true,
            0,
        );
        assert!(matches!(t, SpecialModTemplate::ManualRequired { .. }));
    }

    #[test]
    fn classifies_if_condition_as_manual() {
        let t = classify_special_mod(
            r#"function(num) if num > 10 then return { mod("Str", "BASE", num) } end return {} end"#,
            true,
            0,
        );
        assert!(matches!(t, SpecialModTemplate::ManualRequired { .. }));
    }

    #[test]
    fn classifies_data_dependency_as_manual() {
        let t = classify_special_mod(
            r#"function(num) return { mod("Foo", "BASE", data.someValue) } end"#,
            true,
            0,
        );
        assert!(matches!(t, SpecialModTemplate::ManualRequired { .. }));
    }

    #[test]
    fn classifies_get_threshold_as_manual() {
        let t = classify_special_mod(
            r#"getThreshold("Str", "Accuracy", "INC", 40, { type = "SkillName", skillName = "Power Siphon" })"#,
            true,
            0,
        );
        assert!(matches!(t, SpecialModTemplate::ManualRequired { .. }));
    }

    #[test]
    fn classifies_function_with_enemy_modifier() {
        let t = classify_special_mod(
            r#"function(num) return { mod("EnemyModifier", "LIST", { mod = mod("BlindEffect", "INC", num) }) } end"#,
            true,
            0,
        );
        // This is a simple function that returns EnemyModifier — should be SimpleFn
        // because it's a function (not a static table), and doesn't match the helper patterns.
        // Actually, it contains mod( and flag( so it's SimpleFn.
        assert!(matches!(t, SpecialModTemplate::SimpleFn(_)));
    }

    #[test]
    fn parse_mod_calls_extracts_single_mod() {
        let calls = parse_mod_calls(r#"mod("CritChance", "INC", num)"#);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "CritChance");
        assert_eq!(calls[0].mod_type, "INC");
        assert_eq!(calls[0].value, "num");
    }

    #[test]
    fn parse_mod_calls_extracts_flag() {
        let calls = parse_mod_calls(r#"flag("CannotBeEvaded")"#);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "CannotBeEvaded");
        assert_eq!(calls[0].mod_type, "FLAG");
    }

    #[test]
    fn parse_mod_calls_handles_nested() {
        let calls = parse_mod_calls(
            r#"mod("EnemyModifier", "LIST", { mod = mod("FireResist", "BASE", -10) })"#,
        );
        // Should find the outer mod call; the inner one is inside a nested table argument
        assert!(!calls.is_empty());
        assert_eq!(calls[0].name, "EnemyModifier");
    }

    #[test]
    fn parse_mod_calls_multiple() {
        let calls = parse_mod_calls(r#"{ mod("A", "BASE", 1), flag("B"), mod("C", "INC", num) }"#);
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].name, "A");
        assert_eq!(calls[1].name, "B");
        assert_eq!(calls[2].name, "C");
    }

    #[test]
    fn classifies_trigger_with_options_as_helper() {
        let t = classify_special_mod(
            r#"function(num, _, skill) return triggerExtraSkill(skill, num, {onCrit = true}) end"#,
            true,
            0,
        );
        assert!(matches!(
            t,
            SpecialModTemplate::HelperCall { ref helper, .. } if helper == "triggerExtraSkill"
        ));
    }

    // Coverage test against real data
    #[test]
    fn coverage_at_least_80_percent() {
        let source =
            std::fs::read_to_string("../../third-party/PathOfBuilding/src/Modules/ModParser.lua")
                .unwrap();
        let parsed = crate::lua_parser::parse_mod_parser_lua(&source).unwrap();

        let total = parsed.special_mods.len();
        let manual = parsed
            .special_mods
            .iter()
            .filter(|e| matches!(e.template, SpecialModTemplate::ManualRequired { .. }))
            .count();
        let templated = total - manual;
        let pct = templated as f64 / total as f64 * 100.0;

        eprintln!("specialModList coverage: {templated}/{total} ({pct:.1}%), {manual} manual");

        // Print breakdown by template type
        let static_count = parsed
            .special_mods
            .iter()
            .filter(|e| matches!(e.template, SpecialModTemplate::StaticMods(_)))
            .count();
        let simple_fn = parsed
            .special_mods
            .iter()
            .filter(|e| matches!(e.template, SpecialModTemplate::SimpleFn(_)))
            .count();
        let helper = parsed
            .special_mods
            .iter()
            .filter(|e| matches!(e.template, SpecialModTemplate::HelperCall { .. }))
            .count();
        let enemy = parsed
            .special_mods
            .iter()
            .filter(|e| matches!(e.template, SpecialModTemplate::EnemyModifier(_)))
            .count();
        let minion = parsed
            .special_mods
            .iter()
            .filter(|e| matches!(e.template, SpecialModTemplate::MinionModifier(_)))
            .count();
        let conv = parsed
            .special_mods
            .iter()
            .filter(|e| matches!(e.template, SpecialModTemplate::DamageConversion { .. }))
            .count();
        let gain = parsed
            .special_mods
            .iter()
            .filter(|e| matches!(e.template, SpecialModTemplate::DamageGainAs { .. }))
            .count();

        eprintln!("  StaticMods: {static_count}");
        eprintln!("  SimpleFn: {simple_fn}");
        eprintln!("  HelperCall: {helper}");
        eprintln!("  EnemyModifier: {enemy}");
        eprintln!("  MinionModifier: {minion}");
        eprintln!("  DamageConversion: {conv}");
        eprintln!("  DamageGainAs: {gain}");
        eprintln!("  ManualRequired: {manual}");

        assert!(
            pct >= 80.0,
            "Expected ≥80% coverage, got {pct:.1}% ({manual} manual / {total} total)"
        );
    }
}
