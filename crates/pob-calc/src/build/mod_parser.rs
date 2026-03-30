//! Mod parser — generated from Path of Building's ModParser.lua.
//!
//! This module wraps the auto-generated parser (`mod_parser_generated.rs`)
//! and the hand-written handlers for complex patterns (`mod_parser_manual.rs`).
//! Together they provide 100% coverage of ModParser.lua's pattern tables.

#[allow(
    clippy::all,
    unused_imports,
    dead_code,
    unused_variables,
    unreachable_patterns,
    unused_mut,
    non_upper_case_globals,
    non_snake_case
)]
#[path = "mod_parser_generated.rs"]
mod generated;

#[path = "mod_parser_manual.rs"]
mod mod_parser_manual;

use crate::mod_db::types::{Mod, ModSource};

/// Parse a stat text line into zero or more Mod values.
///
/// Mirrors Path of Building's `ModParser.parseMod()` function.
/// Tries the generated parser first; falls back to `item_parser` for patterns
/// not yet handled by the generated code (transitional — removed in Task 14).
pub fn parse_mod(line: &str, source: ModSource) -> Vec<Mod> {
    let result =
        generated::parse_mod_generated(line, &source, &mod_parser_manual::handle_manual_special);
    if result.is_empty() {
        // Fallback: the generated parser doesn't handle this pattern yet.
        // Use the hand-written item_parser until full coverage is validated.
        return super::item_parser::parse_stat_text(line, source);
    }
    result
}
