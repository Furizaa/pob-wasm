//! Extracts the 6 major tables from ModParser.lua as structured data.

use crate::types::*;

/// Parse the entire ModParser.lua file and extract all tables.
pub fn parse_mod_parser_lua(source: &str) -> Result<ParsedModParser, String> {
    let lines: Vec<&str> = source.lines().collect();

    let forms = parse_form_list(&lines)?;
    let mod_names = parse_mod_name_list(&lines)?;
    let mod_flags = parse_mod_flag_list(&lines)?;
    let pre_flags = parse_pre_flag_list(&lines)?;
    let mod_tags = parse_mod_tag_list(&lines)?;
    let mut special_mods = parse_special_mod_list(&lines)?;

    // Add keystones (56 entries)
    add_keystones(&mut special_mods);
    // Add cluster jewel keystones (8 entries)
    add_cluster_keystones(&mut special_mods);

    // Wrap all specialModList patterns with ^...$ anchors (lines 5692-5696)
    for entry in &mut special_mods {
        let pat = &entry.pattern.0;
        if !pat.starts_with('^') {
            entry.pattern.0 = format!("^{}$", pat);
        } else if !pat.ends_with('$') {
            entry.pattern.0 = format!("{}$", pat);
        }
    }

    Ok(ParsedModParser {
        forms,
        mod_names,
        mod_flags,
        pre_flags,
        mod_tags,
        special_mods,
    })
}

// ---------------------------------------------------------------------------
// Helpers for locating table boundaries
// ---------------------------------------------------------------------------

/// Find the line index (0-based) where the given table opening marker appears.
/// E.g. `local formList = {`
fn find_table_start(lines: &[&str], marker: &str) -> Option<usize> {
    lines.iter().position(|line| {
        let trimmed = line.trim();
        trimmed.starts_with(marker)
    })
}

/// Starting from `start_line` (which should contain the opening `{`),
/// find the matching closing `}` using brace counting.
/// Returns the line index of the closing brace.
fn find_matching_brace(lines: &[&str], start_line: usize) -> Result<usize, String> {
    let mut depth: i32 = 0;
    for i in start_line..lines.len() {
        let line = strip_lua_line_comment(lines[i]);
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    return Ok(i);
                }
            }
        }
    }
    Err(format!(
        "Could not find matching brace starting from line {}",
        start_line + 1
    ))
}

/// Strip a Lua single-line comment (`--`) from a line, being careful about
/// strings. This is a simple heuristic: we don't track string state perfectly,
/// but for the ModParser.lua patterns it's sufficient.
fn strip_lua_line_comment(line: &str) -> &str {
    // Find `--` that is not inside a quoted string.
    // Simple approach: scan for `--` outside of double-quoted strings.
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            in_string = !in_string;
        } else if !in_string && i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            return &line[..i];
        }
        i += 1;
    }
    line
}

/// Extract the key from a `["some key"]` pattern at the start of a trimmed line.
/// Returns (key_content, rest_of_line_after_closing_bracket).
fn extract_bracket_key(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    if !trimmed.starts_with("[\"") {
        return None;
    }
    // Find the closing `"]`
    let after_open = &trimmed[2..]; // skip `["`
                                    // Find `"]` — the key may contain escaped quotes but in practice doesn't
    let close_pos = after_open.find("\"]")?;
    let key = &after_open[..close_pos];
    let rest = &after_open[close_pos + 2..]; // skip `"]`
    Some((key, rest))
}

/// Given a line after the key, strip the `=` and leading whitespace to get the value part.
fn strip_equals(rest: &str) -> Option<&str> {
    let trimmed = rest.trim_start();
    if trimmed.starts_with('=') {
        Some(trimmed[1..].trim_start())
    } else {
        None
    }
}

/// Collect lines for a multi-line value, starting from `line_idx`.
/// `value_start` is the beginning of the value on the current line.
/// This handles brace-counting for `{ ... }` and `function(...) ... end` values.
/// Returns (full_value_string, next_line_index_after_value).
fn collect_value(
    lines: &[&str],
    line_idx: usize,
    value_start: &str,
    table_end: usize,
) -> (String, usize) {
    // Check if this is a function value
    let comment_stripped = strip_lua_line_comment(value_start);
    let trimmed = comment_stripped.trim();

    if trimmed.starts_with("function") || trimmed.trim_end_matches(',') == "explodeFunc" {
        return collect_function_value(lines, line_idx, value_start, table_end);
    }

    // For table values or simple string values
    collect_table_or_simple_value(lines, line_idx, value_start, table_end)
}

/// Collect a function value: `function(...) ... end,`
fn collect_function_value(
    lines: &[&str],
    line_idx: usize,
    value_start: &str,
    table_end: usize,
) -> (String, usize) {
    let comment_stripped = strip_lua_line_comment(value_start);
    let trimmed = comment_stripped.trim();

    // Handle bare references like `explodeFunc,`
    if !trimmed.starts_with("function") {
        return (trimmed.trim_end_matches(',').to_string(), line_idx + 1);
    }

    // Count function/end nesting
    let mut result = String::new();
    let mut func_depth: i32 = 0;
    let mut first = true;

    for i in line_idx..=table_end {
        let raw_line = if first {
            first = false;
            value_start
        } else {
            lines[i]
        };
        let line = strip_lua_line_comment(raw_line);

        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(line);

        // Count function/end keywords (simple token scan)
        func_depth += count_function_keywords(line);

        if func_depth <= 0 {
            // Strip trailing comma
            let trimmed = result.trim_end().trim_end_matches(',');
            return (
                trimmed.to_string(),
                if first { line_idx + 1 } else { i + 1 },
            );
        }
    }

    (
        result.trim_end().trim_end_matches(',').to_string(),
        table_end,
    )
}

/// Count net function depth change in a line.
/// `function` adds +1, standalone `end` adds -1.
fn count_function_keywords(line: &str) -> i32 {
    let mut depth = 0i32;
    // Simple word-boundary scan
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut in_string = false;

    while i < len {
        if bytes[i] == b'"' {
            in_string = !in_string;
            i += 1;
            continue;
        }
        if in_string {
            i += 1;
            continue;
        }

        // Check for "function" keyword
        if i + 8 <= len && &line[i..i + 8] == "function" {
            // Check it's at a word boundary
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            let after_ok = i + 8 >= len || !bytes[i + 8].is_ascii_alphanumeric();
            if before_ok && after_ok {
                depth += 1;
                i += 8;
                continue;
            }
        }

        // Check for "end" keyword
        if i + 3 <= len && &line[i..i + 3] == "end" {
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            let after_ok = i + 3 >= len || !bytes[i + 3].is_ascii_alphanumeric();
            if before_ok && after_ok {
                depth -= 1;
                i += 3;
                continue;
            }
        }

        i += 1;
    }
    depth
}

/// Collect a table `{ ... }` or simple string value.
fn collect_table_or_simple_value(
    lines: &[&str],
    line_idx: usize,
    value_start: &str,
    table_end: usize,
) -> (String, usize) {
    // Strip any trailing comment from the value start
    let comment_stripped = strip_lua_line_comment(value_start);
    let trimmed = comment_stripped.trim();

    // Simple string value like `"INC",`
    if trimmed.starts_with('"') {
        return (trimmed.trim_end_matches(',').to_string(), line_idx + 1);
    }

    // Check if it's a single-line variable reference (like `explodeFunc,`)
    if !trimmed.starts_with('{') && !trimmed.starts_with("function") {
        return (trimmed.trim_end_matches(',').to_string(), line_idx + 1);
    }

    // Table value - use brace counting
    let mut result = String::new();
    let mut depth: i32 = 0;
    let mut first = true;

    for i in line_idx..=table_end {
        let raw_line = if first {
            first = false;
            value_start
        } else {
            lines[i]
        };
        let line = strip_lua_line_comment(raw_line);

        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(line);

        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    let trimmed_result = result.trim_end().trim_end_matches(',');
                    return (
                        trimmed_result.to_string(),
                        if first { line_idx + 1 } else { i + 1 },
                    );
                }
            }
        }
    }

    (
        result.trim_end().trim_end_matches(',').to_string(),
        table_end,
    )
}

// ---------------------------------------------------------------------------
// formList parser
// ---------------------------------------------------------------------------

fn parse_form_list(lines: &[&str]) -> Result<Vec<FormEntry>, String> {
    let start = find_table_start(lines, "local formList = {").ok_or("Could not find formList")?;
    let end = find_matching_brace(lines, start)?;

    let mut entries = Vec::new();
    let mut i = start + 1;
    while i < end {
        let line = strip_lua_line_comment(lines[i]).trim();
        if line.is_empty() || line.starts_with("--") {
            i += 1;
            continue;
        }

        if let Some((key, rest)) = extract_bracket_key(lines[i].trim()) {
            if let Some(value_part) = strip_equals(rest) {
                let (value, next_i) = collect_value(lines, i, value_part, end);
                let form_str = value.trim().trim_matches('"');
                let form = parse_form_type(form_str)?;
                entries.push(FormEntry {
                    pattern: LuaPattern(key.to_string()),
                    form,
                });
                i = next_i;
                continue;
            }
        }
        i += 1;
    }

    Ok(entries)
}

fn parse_form_type(s: &str) -> Result<FormType, String> {
    match s {
        "INC" => Ok(FormType::Inc),
        "RED" => Ok(FormType::Red),
        "MORE" => Ok(FormType::More),
        "LESS" => Ok(FormType::Less),
        "BASE" => Ok(FormType::Base),
        "GAIN" => Ok(FormType::Gain),
        "LOSE" => Ok(FormType::Lose),
        "GRANTS" => Ok(FormType::Grants),
        "REMOVES" => Ok(FormType::Removes),
        "CHANCE" => Ok(FormType::Chance),
        "FLAG" => Ok(FormType::Flag),
        "TOTALCOST" => Ok(FormType::TotalCost),
        "BASECOST" => Ok(FormType::BaseCost),
        "PEN" => Ok(FormType::Pen),
        "REGENFLAT" => Ok(FormType::RegenFlat),
        "REGENPERCENT" => Ok(FormType::RegenPercent),
        "DEGENFLAT" => Ok(FormType::DegenFlat),
        "DEGENPERCENT" => Ok(FormType::DegenPercent),
        "DEGEN" => Ok(FormType::Degen),
        "DMG" => Ok(FormType::Dmg),
        "DMGATTACKS" => Ok(FormType::DmgAttacks),
        "DMGSPELLS" => Ok(FormType::DmgSpells),
        "DMGBOTH" => Ok(FormType::DmgBoth),
        "OVERRIDE" => Ok(FormType::Override),
        "DOUBLED" => Ok(FormType::Doubled),
        other => Err(format!("Unknown form type: {other}")),
    }
}

// ---------------------------------------------------------------------------
// modNameList parser
// ---------------------------------------------------------------------------

fn parse_mod_name_list(lines: &[&str]) -> Result<Vec<ModNameEntry>, String> {
    let start =
        find_table_start(lines, "local modNameList = {").ok_or("Could not find modNameList")?;
    let end = find_matching_brace(lines, start)?;

    let mut entries = Vec::new();
    let mut i = start + 1;
    while i < end {
        let raw_line = lines[i];
        let line = strip_lua_line_comment(raw_line).trim().to_string();
        if line.is_empty() || line.starts_with("--") {
            i += 1;
            continue;
        }

        if let Some((key, rest)) = extract_bracket_key(&line) {
            if let Some(value_part) = strip_equals(rest) {
                let (value, next_i) = collect_value(lines, i, value_part, end);
                let (names, tags) = parse_mod_name_value(&value);
                entries.push(ModNameEntry {
                    key: key.to_string(),
                    names,
                    tags,
                });
                i = next_i;
                continue;
            }
        }
        i += 1;
    }

    Ok(entries)
}

/// Parse a modNameList value. Can be:
/// - `"Str"` — single string
/// - `{ "Str", "Dex", "Int", "All" }` — list of strings
/// - `{ "ManaCost", tag = { type = "SkillType", skillType = SkillType.Attack } }` — list + tags
fn parse_mod_name_value(value: &str) -> (Vec<String>, Vec<LuaTag>) {
    let trimmed = value.trim();

    // Simple string
    if trimmed.starts_with('"') && !trimmed.starts_with('{') {
        let s = trimmed.trim_matches('"');
        return (vec![s.to_string()], vec![]);
    }

    // Table value
    if trimmed.starts_with('{') {
        let inner = trim_outer_braces(trimmed);
        let mut names = Vec::new();
        let mut tags = Vec::new();

        // Extract quoted strings (stat names)
        let mut pos = 0;
        while pos < inner.len() {
            // Skip whitespace and commas
            let ch = inner.as_bytes()[pos];
            if ch == b' ' || ch == b',' || ch == b'\t' || ch == b'\n' || ch == b'\r' {
                pos += 1;
                continue;
            }

            // Quoted string
            if ch == b'"' {
                if let Some(end) = inner[pos + 1..].find('"') {
                    names.push(inner[pos + 1..pos + 1 + end].to_string());
                    pos = pos + 1 + end + 1;
                    continue;
                }
            }

            // tag = { ... } or tagList = { ... }
            if inner[pos..].starts_with("tag") {
                // Extract raw tag source
                if let Some(tag) = extract_lua_tag_from(&inner[pos..]) {
                    tags.push(tag);
                }
                // Skip past this tag
                if let Some(brace_start) = inner[pos..].find('{') {
                    let abs_start = pos + brace_start;
                    if let Some(brace_end) = find_matching_brace_in_str(&inner, abs_start) {
                        pos = brace_end + 1;
                        continue;
                    }
                }
            }

            // Skip anything else (variable references, etc.)
            pos += 1;
        }
        return (names, tags);
    }

    // Fallback: treat as single name
    (vec![trimmed.trim_matches('"').to_string()], vec![])
}

// ---------------------------------------------------------------------------
// modFlagList parser
// ---------------------------------------------------------------------------

fn parse_mod_flag_list(lines: &[&str]) -> Result<Vec<ModFlagEntry>, String> {
    let start =
        find_table_start(lines, "local modFlagList = {").ok_or("Could not find modFlagList")?;
    let end = find_matching_brace(lines, start)?;

    let mut entries = Vec::new();
    let mut i = start + 1;
    while i < end {
        let raw_line = lines[i];
        let line = strip_lua_line_comment(raw_line).trim().to_string();
        if line.is_empty() || line.starts_with("--") {
            i += 1;
            continue;
        }

        if let Some((key, rest)) = extract_bracket_key(&line) {
            if let Some(value_part) = strip_equals(rest) {
                let (value, next_i) = collect_value(lines, i, value_part, end);
                let entry = parse_mod_flag_value(key, &value);
                entries.push(entry);
                i = next_i;
                continue;
            }
        }
        i += 1;
    }

    Ok(entries)
}

fn parse_mod_flag_value(key: &str, value: &str) -> ModFlagEntry {
    let inner = trim_outer_braces(value.trim());
    let flags = extract_field_values(&inner, "flags");
    let keyword_flags = extract_field_values(&inner, "keywordFlags");
    let tags = extract_all_tags(&inner);

    ModFlagEntry {
        key: key.to_string(),
        flags,
        keyword_flags,
        tags,
    }
}

// ---------------------------------------------------------------------------
// preFlagList parser
// ---------------------------------------------------------------------------

fn parse_pre_flag_list(lines: &[&str]) -> Result<Vec<PreFlagEntry>, String> {
    let start =
        find_table_start(lines, "local preFlagList = {").ok_or("Could not find preFlagList")?;
    let end = find_matching_brace(lines, start)?;

    let mut entries = Vec::new();
    let mut i = start + 1;
    while i < end {
        let raw_line = lines[i];
        let line = strip_lua_line_comment(raw_line).trim().to_string();
        if line.is_empty() || line.starts_with("--") {
            i += 1;
            continue;
        }

        if let Some((key, rest)) = extract_bracket_key(&line) {
            if let Some(value_part) = strip_equals(rest) {
                let (value, next_i) = collect_value(lines, i, value_part, end);
                let entry = parse_pre_flag_value(key, &value);
                entries.push(entry);
                i = next_i;
                continue;
            }
        }
        i += 1;
    }

    Ok(entries)
}

fn parse_pre_flag_value(key: &str, value: &str) -> PreFlagEntry {
    let trimmed = value.trim();

    // Check if it's a function
    if trimmed.starts_with("function") {
        return PreFlagEntry {
            pattern: LuaPattern(key.to_string()),
            flags: vec![],
            keyword_flags: vec![],
            tags: vec![],
            add_to_minion: false,
            add_to_skill: false,
            add_to_aura: false,
            new_aura: false,
            apply_to_enemy: false,
            func_body: Some(trimmed.to_string()),
        };
    }

    let inner = trim_outer_braces(trimmed);
    let flags = extract_field_values(&inner, "flags");
    let keyword_flags = extract_field_values(&inner, "keywordFlags");
    let tags = extract_all_tags(&inner);
    let add_to_minion = inner.contains("addToMinion = true");
    let add_to_skill = inner.contains("addToSkill =");
    let add_to_aura = inner.contains("addToAura = true");
    let new_aura = inner.contains("newAura = true");
    let apply_to_enemy = inner.contains("applyToEnemy = true");

    PreFlagEntry {
        pattern: LuaPattern(key.to_string()),
        flags,
        keyword_flags,
        tags,
        add_to_minion,
        add_to_skill,
        add_to_aura,
        new_aura,
        apply_to_enemy,
        func_body: None,
    }
}

// ---------------------------------------------------------------------------
// modTagList parser
// ---------------------------------------------------------------------------

fn parse_mod_tag_list(lines: &[&str]) -> Result<Vec<ModTagEntry>, String> {
    let start =
        find_table_start(lines, "local modTagList = {").ok_or("Could not find modTagList")?;
    let end = find_matching_brace(lines, start)?;

    let mut entries = Vec::new();
    let mut i = start + 1;
    while i < end {
        let raw_line = lines[i];
        let line = strip_lua_line_comment(raw_line).trim().to_string();
        if line.is_empty() || line.starts_with("--") {
            i += 1;
            continue;
        }

        if let Some((key, rest)) = extract_bracket_key(&line) {
            if let Some(value_part) = strip_equals(rest) {
                let (value, next_i) = collect_value(lines, i, value_part, end);
                let entry = parse_mod_tag_value(key, &value);
                entries.push(entry);
                i = next_i;
                continue;
            }
        }
        i += 1;
    }

    Ok(entries)
}

fn parse_mod_tag_value(key: &str, value: &str) -> ModTagEntry {
    let trimmed = value.trim();

    if trimmed.starts_with("function") {
        return ModTagEntry {
            pattern: LuaPattern(key.to_string()),
            tags: vec![],
            func_body: Some(trimmed.to_string()),
        };
    }

    let inner = trim_outer_braces(trimmed);
    let tags = extract_all_tags(&inner);

    ModTagEntry {
        pattern: LuaPattern(key.to_string()),
        tags,
        func_body: None,
    }
}

// ---------------------------------------------------------------------------
// specialModList parser
// ---------------------------------------------------------------------------

fn parse_special_mod_list(lines: &[&str]) -> Result<Vec<SpecialModEntry>, String> {
    let start = find_table_start(lines, "local specialModList = {")
        .ok_or("Could not find specialModList")?;
    let end = find_matching_brace(lines, start)?;

    let mut entries = Vec::new();
    let mut i = start + 1;
    while i < end {
        let raw_line = lines[i];
        let line = strip_lua_line_comment(raw_line).trim().to_string();
        if line.is_empty() || line.starts_with("--") {
            i += 1;
            continue;
        }

        if let Some((key, rest)) = extract_bracket_key(&line) {
            if let Some(value_part) = strip_equals(rest) {
                let line_number = i + 1; // 1-based
                let (value, next_i) = collect_value(lines, i, value_part, end);
                let trimmed_value = value.trim_start();
                let is_function = trimmed_value.starts_with("function(")
                    || trimmed_value.starts_with("function (");
                let template =
                    crate::templates::classify_special_mod(&value, is_function, line_number);
                entries.push(SpecialModEntry {
                    pattern: LuaPattern(key.to_string()),
                    template,
                    line_number,
                });
                i = next_i;
                continue;
            }
        }
        i += 1;
    }

    Ok(entries)
}

// ---------------------------------------------------------------------------
// Keystone additions
// ---------------------------------------------------------------------------

fn add_keystones(special_mods: &mut Vec<SpecialModEntry>) {
    let keystones = [
        "Acrobatics",
        "Ancestral Bond",
        "Arrow Dancing",
        "Arsenal of Vengeance",
        "Avatar of Fire",
        "Blood Magic",
        "Bloodsoaked Blade",
        "Call to Arms",
        "Chaos Inoculation",
        "Conduit",
        "Corrupted Soul",
        "Crimson Dance",
        "Divine Flesh",
        "Divine Shield",
        "Doomsday",
        "Eldritch Battery",
        "Elemental Equilibrium",
        "Elemental Overload",
        "Eternal Youth",
        "Ghost Dance",
        "Ghost Reaver",
        "Glancing Blows",
        "Hex Master",
        "Imbalanced Guard",
        "Immortal Ambition",
        "Inner Conviction",
        "Iron Grip",
        "Iron Reflexes",
        "Iron Will",
        "Lethe Shade",
        "Magebane",
        "Mind Over Matter",
        "Minion Instability",
        "Mortal Conviction",
        "Necromantic Aegis",
        "Pain Attunement",
        "Perfect Agony",
        "Phase Acrobatics",
        "Point Blank",
        "Power of Purpose",
        "Precise Technique",
        "Resolute Technique",
        "Runebinder",
        "Solipsism",
        "Supreme Decadence",
        "Supreme Ego",
        "The Agnostic",
        "The Impaler",
        "Transcendence",
        "Unwavering Stance",
        "Vaal Pact",
        "Versatile Combatant",
        "Wicked Ward",
        "Wind Dancer",
        "Zealot's Oath",
    ];

    for name in &keystones {
        special_mods.push(make_keystone_entry(name));
    }
}

fn add_cluster_keystones(special_mods: &mut Vec<SpecialModEntry>) {
    let cluster_keystones = [
        "Disciple of Kitava",
        "Lone Messenger",
        "Nature's Patience",
        "Secrets of Suffering",
        "Kineticism",
        "Veteran's Awareness",
        "Hollow Palm Technique",
        "Pitfighter",
    ];

    for name in &cluster_keystones {
        special_mods.push(make_keystone_entry(name));
    }
}

fn make_keystone_entry(name: &str) -> SpecialModEntry {
    SpecialModEntry {
        pattern: LuaPattern(name.to_lowercase()),
        template: SpecialModTemplate::StaticMods(vec![LuaModCall {
            name: "Keystone".into(),
            mod_type: "LIST".into(),
            // Wrap in quotes so lua_value_to_rust() recognises this as a string
            // literal and emits ModValue::String("...") rather than the TODO fallback.
            value: format!("\"{}\"", name),
            flags: None,
            keyword_flags: None,
            tags: vec![],
            dynamic_name: false,
        }]),
        line_number: 0, // synthetic
    }
}

// ---------------------------------------------------------------------------
// Shared helpers for parsing Lua table values
// ---------------------------------------------------------------------------

/// Trim outer `{` and `}` from a value string.
fn trim_outer_braces(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        trimmed[1..trimmed.len() - 1].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

/// Extract the raw value(s) for a named field like `flags = ...` from a table interior.
/// Returns the raw expression(s) as strings.
fn extract_field_values(inner: &str, field: &str) -> Vec<String> {
    let pattern = format!("{} = ", field);
    if let Some(pos) = inner.find(&pattern) {
        let after = &inner[pos + pattern.len()..];
        // Read until comma or end of string, respecting braces and parens
        let value = read_until_field_end(after);
        vec![value.trim().to_string()]
    } else {
        vec![]
    }
}

/// Read a field value until we hit a comma at depth 0 or end of input.
fn read_until_field_end(s: &str) -> &str {
    let mut depth = 0i32;
    for (i, ch) in s.char_indices() {
        match ch {
            '{' | '(' => depth += 1,
            '}' | ')' => {
                if depth == 0 {
                    return &s[..i];
                }
                depth -= 1;
            }
            ',' if depth == 0 => return &s[..i],
            _ => {}
        }
    }
    s
}

/// Extract all tag entries from a Lua table interior.
/// Looks for `tag = { ... }` and `tagList = { ... }`.
fn extract_all_tags(inner: &str) -> Vec<LuaTag> {
    let mut tags = Vec::new();

    // Look for `tag = {`
    if let Some(pos) = find_field_start(inner, "tag") {
        let after = &inner[pos..];
        if let Some(brace_pos) = after.find('{') {
            let abs = pos + brace_pos;
            if let Some(brace_end) = find_matching_brace_in_str(inner, abs) {
                let tag_src = &inner[abs..=brace_end];
                if let Some(tag) = parse_lua_tag(tag_src) {
                    tags.push(tag);
                }
            }
        }
    }

    // Look for `tagList = {`
    if let Some(pos) = find_field_start(inner, "tagList") {
        let after = &inner[pos..];
        if let Some(brace_pos) = after.find('{') {
            let abs = pos + brace_pos;
            if let Some(outer_end) = find_matching_brace_in_str(inner, abs) {
                // The tagList is { { ... }, { ... } }
                let list_content = &inner[abs + 1..outer_end];
                let mut j = 0;
                while j < list_content.len() {
                    if list_content.as_bytes()[j] == b'{' {
                        if let Some(inner_end) = find_matching_brace_in_str(list_content, j) {
                            let tag_src = &list_content[j..=inner_end];
                            if let Some(tag) = parse_lua_tag(tag_src) {
                                tags.push(tag);
                            }
                            j = inner_end + 1;
                            continue;
                        }
                    }
                    j += 1;
                }
            }
        }
    }

    tags
}

/// Find the start position of a field assignment like `tag = ` in a string,
/// being careful not to match `tagList` when looking for `tag`.
fn find_field_start(s: &str, field: &str) -> Option<usize> {
    let mut search_from = 0;
    loop {
        if let Some(pos) = s[search_from..].find(field) {
            let abs_pos = search_from + pos;
            let after_field = abs_pos + field.len();

            // Check that the character after the field name is `=` or whitespace then `=`
            let rest = s[after_field..].trim_start();
            if !rest.starts_with('=') {
                search_from = after_field;
                continue;
            }

            // Check that this isn't a substring of a longer identifier
            // (e.g., "tagList" when searching for "tag")
            if after_field < s.len() && s.as_bytes()[after_field].is_ascii_alphanumeric() {
                search_from = after_field;
                continue;
            }

            return Some(abs_pos);
        } else {
            return None;
        }
    }
}

/// Find matching `}` for a `{` at position `start` in a string.
fn find_matching_brace_in_str(s: &str, start: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;
    for (i, ch) in s[start..].char_indices() {
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                return Some(start + i);
            }
        }
    }
    None
}

/// Parse a single Lua tag like `{ type = "Condition", var = "LowLife" }`.
fn parse_lua_tag(src: &str) -> Option<LuaTag> {
    let inner = trim_outer_braces(src);
    if inner.is_empty() {
        return None;
    }

    let mut tag_type = String::new();
    let mut fields = Vec::new();

    // Extract type field
    if let Some(type_pos) = inner.find("type = \"") {
        let after = &inner[type_pos + 8..];
        if let Some(end) = after.find('"') {
            tag_type = after[..end].to_string();
        }
    }

    // Extract other key = value pairs
    // Simple approach: find all `key = value` patterns
    let pairs = extract_kv_pairs(&inner);
    for (k, v) in pairs {
        if k != "type" {
            fields.push((k, v));
        }
    }

    if tag_type.is_empty() && fields.is_empty() {
        return None;
    }

    Some(LuaTag { tag_type, fields })
}

/// Extract key = value pairs from a Lua table interior (shallow).
fn extract_kv_pairs(s: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut pos = 0;
    let bytes = s.as_bytes();

    while pos < bytes.len() {
        // Skip whitespace and commas
        while pos < bytes.len()
            && (bytes[pos] == b' '
                || bytes[pos] == b','
                || bytes[pos] == b'\t'
                || bytes[pos] == b'\n'
                || bytes[pos] == b'\r')
        {
            pos += 1;
        }
        if pos >= bytes.len() {
            break;
        }

        // Try to read an identifier
        let key_start = pos;
        while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_') {
            pos += 1;
        }
        if pos == key_start {
            // Not an identifier - skip this char
            pos += 1;
            continue;
        }
        let key = &s[key_start..pos];

        // Skip whitespace
        while pos < bytes.len() && bytes[pos] == b' ' {
            pos += 1;
        }

        // Expect `=`
        if pos >= bytes.len() || bytes[pos] != b'=' {
            continue;
        }
        pos += 1;

        // Skip whitespace
        while pos < bytes.len() && bytes[pos] == b' ' {
            pos += 1;
        }

        // Read value
        let val_start = pos;
        if pos < bytes.len() && bytes[pos] == b'{' {
            // Nested table - find matching brace
            if let Some(end) = find_matching_brace_in_str(s, pos) {
                pos = end + 1;
            } else {
                pos += 1;
            }
        } else if pos < bytes.len() && bytes[pos] == b'"' {
            // String value
            pos += 1;
            while pos < bytes.len() && bytes[pos] != b'"' {
                pos += 1;
            }
            if pos < bytes.len() {
                pos += 1; // skip closing quote
            }
        } else {
            // Bare value (number, boolean, identifier, etc.)
            while pos < bytes.len()
                && bytes[pos] != b','
                && bytes[pos] != b'}'
                && bytes[pos] != b'\n'
            {
                pos += 1;
            }
        }

        let value = s[val_start..pos].trim().to_string();
        pairs.push((key.to_string(), value));
    }

    pairs
}

/// Extract a LuaTag from a position in a string that starts with `tag`.
fn extract_lua_tag_from(s: &str) -> Option<LuaTag> {
    // Find the `{` after `tag =`
    if let Some(brace_pos) = s.find('{') {
        if let Some(brace_end) = find_matching_brace_in_str(s, brace_pos) {
            let tag_src = &s[brace_pos..=brace_end];
            return parse_lua_tag(tag_src);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_mod_parser_lua() -> String {
        std::fs::read_to_string("../../third-party/PathOfBuilding/src/Modules/ModParser.lua")
            .expect("ModParser.lua not found — ensure git submodule is checked out")
    }

    #[test]
    fn parses_form_list() {
        let source = load_mod_parser_lua();
        let parsed = parse_mod_parser_lua(&source).unwrap();
        assert!(
            parsed.forms.len() >= 80,
            "Expected ~85 forms, got {}",
            parsed.forms.len()
        );
        let first = &parsed.forms[0];
        assert_eq!(first.form, FormType::Inc);
        assert!(first.pattern.0.contains("%d+"));
        assert!(first.pattern.0.contains("increased"));
        assert!(parsed.forms.iter().any(|f| f.form == FormType::Flag));
        assert!(parsed.forms.iter().any(|f| f.form == FormType::Doubled));
    }

    #[test]
    fn parses_mod_name_list() {
        let source = load_mod_parser_lua();
        let parsed = parse_mod_parser_lua(&source).unwrap();
        assert!(
            parsed.mod_names.len() >= 650,
            "Expected ~687, got {}",
            parsed.mod_names.len()
        );
        let strength = parsed.mod_names.iter().find(|e| e.key == "strength");
        assert!(strength.is_some());
        assert_eq!(strength.unwrap().names, vec!["Str"]);
        let attrs = parsed.mod_names.iter().find(|e| e.key == "attributes");
        assert!(attrs.is_some());
        assert_eq!(attrs.unwrap().names.len(), 4);
    }

    #[test]
    fn parses_mod_flag_list() {
        let source = load_mod_parser_lua();
        let parsed = parse_mod_parser_lua(&source).unwrap();
        assert!(
            parsed.mod_flags.len() >= 170,
            "Expected ~181, got {}",
            parsed.mod_flags.len()
        );
    }

    #[test]
    fn parses_pre_flag_list() {
        let source = load_mod_parser_lua();
        let parsed = parse_mod_parser_lua(&source).unwrap();
        assert!(
            parsed.pre_flags.len() >= 180,
            "Expected ~188, got {}",
            parsed.pre_flags.len()
        );
    }

    #[test]
    fn parses_mod_tag_list() {
        let source = load_mod_parser_lua();
        let parsed = parse_mod_parser_lua(&source).unwrap();
        assert!(
            parsed.mod_tags.len() >= 600,
            "Expected ~637, got {}",
            parsed.mod_tags.len()
        );
    }

    #[test]
    fn parses_special_mod_list() {
        let source = load_mod_parser_lua();
        let parsed = parse_mod_parser_lua(&source).unwrap();
        // ~2,030 static + 56 keystones + 8 cluster keystones = ~2,094
        assert!(
            parsed.special_mods.len() >= 2000,
            "Expected ~2,094+, got {}",
            parsed.special_mods.len()
        );
    }
}
