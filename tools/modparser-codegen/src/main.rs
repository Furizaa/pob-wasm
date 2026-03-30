//! ModParser code generator — reads ModParser.lua, emits Rust source.

mod emitter;
mod lua_parser;
mod pattern_translator;
mod templates;
mod types;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "modparser-codegen")]
#[command(about = "Generate Rust mod parser from PoB's ModParser.lua")]
struct Cli {
    /// Path to ModParser.lua
    #[arg(
        long,
        default_value = "third-party/PathOfBuilding/src/Modules/ModParser.lua"
    )]
    input: PathBuf,

    /// Output path for mod_parser_generated.rs
    #[arg(
        long,
        default_value = "crates/pob-calc/src/build/mod_parser_generated.rs"
    )]
    output: PathBuf,

    /// Output path for mod_parser_manual_manifest.rs (stubs for manual handlers)
    #[arg(
        long,
        default_value = "crates/pob-calc/src/build/mod_parser_manual_manifest.rs"
    )]
    manifest: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    let source = std::fs::read_to_string(&cli.input)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", cli.input.display()));

    let parsed = lua_parser::parse_mod_parser_lua(&source)
        .unwrap_or_else(|e| panic!("Failed to parse: {e}"));

    eprintln!(
        "Parsed: {} forms, {} mod_names, {} mod_flags, {} pre_flags, {} mod_tags, {} special_mods",
        parsed.forms.len(),
        parsed.mod_names.len(),
        parsed.mod_flags.len(),
        parsed.pre_flags.len(),
        parsed.mod_tags.len(),
        parsed.special_mods.len()
    );

    let generated =
        emitter::emit_generated(&parsed).unwrap_or_else(|e| panic!("Failed to emit: {e}"));

    std::fs::write(&cli.output, &generated)
        .unwrap_or_else(|e| panic!("Failed to write {}: {e}", cli.output.display()));

    let manifest = emitter::emit_manual_manifest(&parsed)
        .unwrap_or_else(|e| panic!("Failed to emit manifest: {e}"));

    std::fs::write(&cli.manifest, &manifest)
        .unwrap_or_else(|e| panic!("Failed to write {}: {e}", cli.manifest.display()));

    let total = parsed.special_mods.len();
    let manual = parsed
        .special_mods
        .iter()
        .filter(|e| {
            matches!(
                e.template,
                crate::types::SpecialModTemplate::ManualRequired { .. }
            )
        })
        .count();
    let templated = total - manual;
    eprintln!(
        "specialModList: {templated}/{total} templated ({:.1}%), {manual} manual",
        templated as f64 / total as f64 * 100.0
    );

    eprintln!("Generated: {}", cli.output.display());
    eprintln!("Manifest:  {}", cli.manifest.display());
}
