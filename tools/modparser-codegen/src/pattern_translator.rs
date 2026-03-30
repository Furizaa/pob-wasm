//! Translates Lua pattern syntax to Rust regex syntax.

/// Characters that are special in Rust regex and need escaping when they appear
/// as literal characters (outside of character classes).
fn is_regex_meta(c: char) -> bool {
    matches!(
        c,
        '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '\\' | '|' | '^' | '$'
    )
}

/// Translate a `%X` Lua escape sequence to its Rust regex equivalent.
/// `in_class` indicates whether we're inside a `[...]` character class.
fn translate_lua_escape(c: char, in_class: bool) -> Result<String, String> {
    match c {
        // Character classes
        'd' => Ok(r"\d".into()),
        'D' => Ok(r"\D".into()),
        'w' => Ok(r"\w".into()),
        'W' => Ok(r"\W".into()),
        's' => Ok(r"\s".into()),
        'S' => Ok(r"\S".into()),
        'a' => {
            if in_class {
                Ok("a-zA-Z".into())
            } else {
                Ok("[a-zA-Z]".into())
            }
        }
        'l' => {
            if in_class {
                Ok("a-z".into())
            } else {
                Ok("[a-z]".into())
            }
        }
        'u' => {
            if in_class {
                Ok("A-Z".into())
            } else {
                Ok("[A-Z]".into())
            }
        }
        'p' => {
            if in_class {
                Ok("[:punct:]".into())
            } else {
                Ok("[[:punct:]]".into())
            }
        }
        // Literal percent
        '%' => Ok("%".into()),
        // Literal characters that are regex-special: escape them for regex
        // Inside a character class, some chars don't need escaping
        '-' => {
            // Hyphen: always escape as \- (safe in and out of classes)
            Ok(r"\-".into())
        }
        '.' => {
            if in_class {
                // Inside [...], a literal dot doesn't need escaping
                Ok(".".into())
            } else {
                Ok(r"\.".into())
            }
        }
        '+' => {
            if in_class {
                // Inside [...], + is literal
                Ok("+".into())
            } else {
                Ok(r"\+".into())
            }
        }
        '*' => {
            if in_class {
                Ok("*".into())
            } else {
                Ok(r"\*".into())
            }
        }
        '?' => {
            if in_class {
                Ok("?".into())
            } else {
                Ok(r"\?".into())
            }
        }
        '[' => {
            if in_class {
                Ok(r"\[".into())
            } else {
                Ok(r"\[".into())
            }
        }
        ']' => Ok(r"\]".into()),
        '(' => {
            if in_class {
                Ok("(".into())
            } else {
                Ok(r"\(".into())
            }
        }
        ')' => {
            if in_class {
                Ok(")".into())
            } else {
                Ok(r"\)".into())
            }
        }
        '^' => {
            if in_class {
                Ok(r"\^".into())
            } else {
                Ok(r"\^".into())
            }
        }
        '$' => {
            if in_class {
                Ok("$".into())
            } else {
                Ok(r"\$".into())
            }
        }
        // Any other char after % is treated as a literal escape
        other => {
            if is_regex_meta(other) {
                Ok(format!("\\{other}"))
            } else {
                // Not regex-special, emit literally
                Ok(other.to_string())
            }
        }
    }
}

/// Translate a Lua pattern string to a Rust regex string.
/// Returns the regex string and the number of capture groups.
pub fn lua_pattern_to_regex(lua_pattern: &str) -> Result<(String, usize), String> {
    let chars: Vec<char> = lua_pattern.chars().collect();
    let len = chars.len();
    let mut out = String::with_capacity(len * 2);
    let mut captures: usize = 0;
    let mut i = 0;
    let mut in_class = false; // inside [...]

    while i < len {
        let c = chars[i];

        if c == '%' {
            // Lua escape sequence
            i += 1;
            if i >= len {
                return Err("Pattern ends with lone '%'".into());
            }
            let next = chars[i];
            out.push_str(&translate_lua_escape(next, in_class)?);
            i += 1;
        } else if c == '[' && !in_class {
            // Start of character class
            in_class = true;
            out.push('[');
            i += 1;
            // Handle negation: [^...] in Lua
            if i < len && chars[i] == '^' {
                out.push('^');
                i += 1;
            }
        } else if c == ']' && in_class {
            // End of character class
            in_class = false;
            out.push(']');
            i += 1;
        } else if c == '(' && !in_class {
            // Start of capture group — check for special (.-)
            captures += 1;
            if i + 3 < len && chars[i + 1] == '.' && chars[i + 2] == '-' && chars[i + 3] == ')' {
                // Non-greedy capture: Lua's (.-) → regex (.*?)
                out.push_str("(.*?)");
                i += 4;
            } else {
                out.push('(');
                i += 1;
            }
        } else if c == ')' && !in_class {
            out.push(')');
            i += 1;
        } else {
            // Regular character — pass through
            // Note: in Lua patterns, most chars are literal. In regex, we only
            // need to escape them if they're regex-special AND not inside a class.
            if !in_class && is_regex_meta(c) {
                // Characters like ^, $, ?, +, *, . are also used in Lua patterns
                // with the same meaning as regex, so pass them through directly.
                match c {
                    '^' | '$' | '?' | '+' | '*' | '.' => {
                        out.push(c);
                    }
                    // Other regex metas that shouldn't appear bare in Lua patterns
                    // but escape them just in case
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                }
            } else {
                out.push(c);
            }
            i += 1;
        }
    }

    Ok((out, captures))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_digit_class() {
        let (regex, caps) = lua_pattern_to_regex("^(%d+)%% increased").unwrap();
        assert_eq!(regex, r"^(\d+)% increased");
        assert_eq!(caps, 1);
    }

    #[test]
    fn translates_signed_number() {
        let (regex, caps) = lua_pattern_to_regex("^([%+%-][%d%.]+)%%?").unwrap();
        assert_eq!(regex, r"^([+\-][\d.]+)%?");
        assert_eq!(caps, 1);
    }

    #[test]
    fn translates_non_greedy_capture() {
        let (regex, caps) = lua_pattern_to_regex("regenerate (.-)%% of").unwrap();
        assert_eq!(regex, r"regenerate (.*?)% of");
        assert_eq!(caps, 1);
    }

    #[test]
    fn translates_letter_class() {
        let (regex, caps) = lua_pattern_to_regex("(%a+) damage").unwrap();
        assert_eq!(regex, r"([a-zA-Z]+) damage");
        assert_eq!(caps, 1);
    }

    #[test]
    fn translates_literal_percent() {
        let (regex, _) = lua_pattern_to_regex("%%").unwrap();
        assert_eq!(regex, "%");
    }

    #[test]
    fn translates_literal_hyphen() {
        let (regex, _) = lua_pattern_to_regex("non%-curse").unwrap();
        assert_eq!(regex, r"non\-curse");
    }

    #[test]
    fn translates_literal_dot() {
        let (regex, _) = lua_pattern_to_regex("%.").unwrap();
        assert_eq!(regex, r"\.");
    }

    #[test]
    fn translates_char_class_with_lua_escapes() {
        let (regex, _) = lua_pattern_to_regex("[%d%.]+").unwrap();
        assert_eq!(regex, r"[\d.]+");
    }

    #[test]
    fn translates_optional_char() {
        let (regex, _) = lua_pattern_to_regex("hits?").unwrap();
        assert_eq!(regex, "hits?");
    }

    #[test]
    fn translates_word_class() {
        let (regex, _) = lua_pattern_to_regex("%w+").unwrap();
        assert_eq!(regex, r"\w+");
    }

    #[test]
    fn translates_lowercase_class() {
        let (regex, _) = lua_pattern_to_regex("%l").unwrap();
        assert_eq!(regex, "[a-z]");
    }

    #[test]
    fn translates_complex_pattern() {
        let (regex, caps) =
            lua_pattern_to_regex("^(%d+)%% of physical damage converted to (%a+) damage").unwrap();
        assert_eq!(
            regex,
            r"^(\d+)% of physical damage converted to ([a-zA-Z]+) damage"
        );
        assert_eq!(caps, 2);
    }

    #[test]
    fn preserves_char_class_brackets() {
        let (regex, _) = lua_pattern_to_regex("[hd][ae][va][el]").unwrap();
        assert_eq!(regex, "[hd][ae][va][el]");
    }

    #[test]
    fn translates_escaped_plus() {
        let (regex, _) = lua_pattern_to_regex("%+").unwrap();
        assert_eq!(regex, r"\+");
    }

    #[test]
    fn translates_uppercase_class() {
        let (regex, _) = lua_pattern_to_regex("%u").unwrap();
        assert_eq!(regex, "[A-Z]");
    }

    #[test]
    fn translates_space_class() {
        let (regex, _) = lua_pattern_to_regex("%s+").unwrap();
        assert_eq!(regex, r"\s+");
    }

    #[test]
    fn translates_non_space_class() {
        let (regex, _) = lua_pattern_to_regex("%S").unwrap();
        assert_eq!(regex, r"\S");
    }

    #[test]
    fn translates_dollar_anchor() {
        let (regex, _) = lua_pattern_to_regex("^test$").unwrap();
        assert_eq!(regex, "^test$");
    }

    // Bulk validation against real patterns
    #[test]
    fn all_form_list_patterns_translate_and_compile() {
        let source =
            std::fs::read_to_string("../../third-party/PathOfBuilding/src/Modules/ModParser.lua")
                .unwrap();
        let parsed = crate::lua_parser::parse_mod_parser_lua(&source).unwrap();
        for form in &parsed.forms {
            let result = lua_pattern_to_regex(&form.pattern.0);
            assert!(
                result.is_ok(),
                "Failed to translate '{}': {:?}",
                form.pattern.0,
                result.err()
            );
            let (regex_str, _) = result.unwrap();
            assert!(
                regex::Regex::new(&regex_str).is_ok(),
                "Invalid regex '{}' from '{}'",
                regex_str,
                form.pattern.0
            );
        }
    }

    #[test]
    fn all_special_mod_patterns_translate_and_compile() {
        let source =
            std::fs::read_to_string("../../third-party/PathOfBuilding/src/Modules/ModParser.lua")
                .unwrap();
        let parsed = crate::lua_parser::parse_mod_parser_lua(&source).unwrap();
        let mut failures = 0;
        for entry in &parsed.special_mods {
            let result = lua_pattern_to_regex(&entry.pattern.0);
            if let Ok((regex_str, _)) = &result {
                if regex::Regex::new(regex_str).is_err() {
                    eprintln!("Invalid regex '{}' from '{}'", regex_str, entry.pattern.0);
                    failures += 1;
                }
            } else {
                eprintln!(
                    "Failed to translate '{}': {:?}",
                    entry.pattern.0,
                    result.err()
                );
                failures += 1;
            }
        }
        assert!(
            failures == 0,
            "{failures} patterns failed to translate/compile"
        );
    }
}
