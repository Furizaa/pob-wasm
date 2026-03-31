//! Mod parser — generated from Path of Building's ModParser.lua.
//!
//! This module wraps the auto-generated parser (`mod_parser_generated.rs`)
//! and the hand-written handlers for complex patterns (`mod_parser_manual.rs`).
//! Together they provide 100% coverage of ModParser.lua's pattern tables.

#[allow(
    clippy::all,
    unused_imports,
    unused_assignments,
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
pub(crate) mod mod_parser_manual;

use crate::mod_db::types::{Mod, ModSource};

// Verify all manual handlers are implemented
const _: () = assert!(
    mod_parser_manual::IMPLEMENTED_MANUAL_COUNT == 25,
    "Not all manual handlers implemented"
);

/// Parse a stat text line into zero or more Mod values.
///
/// Mirrors Path of Building's `ModParser.parseMod()` function.
/// 100% coverage of ModParser.lua's pattern tables — templated entries
/// handled by generated code, remaining entries by hand-written handlers.
pub fn parse_mod(line: &str, source: ModSource) -> Vec<Mod> {
    generated::parse_mod_generated(line, &source, &mod_parser_manual::handle_manual_special)
}
