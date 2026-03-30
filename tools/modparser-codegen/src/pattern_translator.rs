//! Translates Lua pattern syntax to Rust regex syntax.

/// Translate a Lua pattern string to a Rust regex string.
/// Returns the regex string and the number of capture groups.
pub fn lua_pattern_to_regex(lua_pattern: &str) -> Result<(String, usize), String> {
    todo!("Implement in Task 3")
}
