mod error;
mod ggpk_reader;
mod dat64;
mod transform;

use clap::Parser;
use std::path::PathBuf;

/// Extract game data from Content.ggpk and write JSON files to an output directory.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Path to Content.ggpk (or the game's Steam install directory)
    ggpk: PathBuf,

    /// Output directory for JSON data files (will be created if it does not exist)
    #[arg(short, long, default_value = "data")]
    output: PathBuf,
}

fn main() -> Result<(), error::ExtractError> {
    let args = Args::parse();
    std::fs::create_dir_all(&args.output)?;
    std::fs::create_dir_all(args.output.join("tree"))?;

    let reader = ggpk_reader::GgpkReader::open(&args.ggpk)?;

    println!("Extracting misc data...");
    transform::misc::extract(&reader, &args.output)?;

    println!("Extracting gem data...");
    transform::gems::extract(&reader, &args.output)?;

    println!("Extracting base item data...");
    transform::bases::extract(&reader, &args.output)?;

    println!("Extracting mod data...");
    transform::mods::extract(&reader, &args.output)?;

    println!("Extracting passive tree data...");
    transform::tree::extract(&reader, &args.output)?;

    println!("Done. Output written to {}", args.output.display());
    Ok(())
}
