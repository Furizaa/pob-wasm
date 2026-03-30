//! Hand-written handlers for specialModList entries that codegen can't template.
//!
//! Each handler is identified by a string ID (emitted by the codegen manifest).
//! See mod_parser_manual_manifest.rs for the full list of entries with their
//! Lua source as comments for reference.

use crate::mod_db::types::*;

/// Handle a manually-implemented special mod pattern.
///
/// `id` is the handler ID assigned by the codegen manifest.
/// `caps` contains the regex captures from the pattern match.
/// `source` is the ModSource for the resulting mods.
///
/// Returns the mods for this pattern, or empty vec if the handler is not yet implemented.
pub fn handle_manual_special(id: &str, _caps: &regex::Captures, _source: &ModSource) -> Vec<Mod> {
    match id {
        // TODO(phase3-task13): implement all handlers from manifest
        // See mod_parser_manual_manifest.rs for the full list
        _ => {
            // Unimplemented manual handler — return empty for now
            vec![]
        }
    }
}
