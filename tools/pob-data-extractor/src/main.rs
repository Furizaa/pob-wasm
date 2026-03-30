mod extract_bases;
mod extract_gems;
mod extract_mods;
mod extract_uniques;
mod lua_env;
mod types;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "pob-data-extractor")]
#[command(about = "Extract game data from Path of Building Lua sources into JSON")]
struct Cli {
    /// Path to the PathOfBuilding/src directory
    pob_src: String,

    /// Output directory for generated JSON files
    #[arg(short, long, default_value = "data")]
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    std::fs::create_dir_all(&cli.output)?;

    println!("Extracting gem data...");
    extract_gems::extract(&cli.pob_src, &cli.output)?;

    println!("Extracting base item data...");
    extract_bases::extract(&cli.pob_src, &cli.output)?;

    println!("Extracting unique item data...");
    extract_uniques::extract(&cli.pob_src, &cli.output)?;

    println!("Extracting mod data...");
    extract_mods::extract(&cli.pob_src, &cli.output)?;

    println!("Done. Output written to {}", cli.output.display());
    Ok(())
}
