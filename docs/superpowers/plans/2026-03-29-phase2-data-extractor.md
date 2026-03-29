# Phase 2: Data Extractor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `data-extractor` CLI binary that reads `Content.ggpk`, extracts the `.dat64` game tables and stat description text files, and writes structured JSON files to `data/` that `pob-calc` will consume.

**Architecture:** The `ggpk` crate (v1.2.2) handles GGPK file reading via memory-mapped I/O. The extractor parses `.dat64` binary format in-house (it is not complex — a fixed header, a row section, and a string section). Each data type (gems, passives, bases, mods, misc) gets its own transformer module producing a typed Rust struct that serializes to JSON via `serde_json`. The passive tree is extracted from the game's `Data/PassiveSkills.dat` and `Data/PassiveSkillTreeuiExtraImages.json`, **not** from POB's shipped `TreeData/` files.

**Tech Stack:** Rust 1.82 stable, `ggpk 1.2.2`, `serde/serde_json`, `thiserror`, `clap 4`

**Prerequisites:** Phase 1 complete. A `Content.ggpk` file available locally (path passed via CLI argument — not committed to the repo).

**Reference:** `third-party/PathOfBuilding/src/Export/Scripts/` for field names and output schemas. `third-party/PathOfBuilding/src/Export/spec.lua` for `.dat64` field type definitions.

---

## File Map

```
crates/data-extractor/
  Cargo.toml                         ← add clap, ggpk dependencies
  src/
    main.rs                          ← CLI entry point (clap), orchestrates extraction
    error.rs                         ← ExtractError enum
    ggpk_reader.rs                   ← thin wrapper: open GGPK, read file bytes by path
    dat64.rs                         ← dat64 binary parser (generic row/field reader)
    transform/
      mod.rs                         ← re-exports all transformers
      misc.rs                        ← GameConstants, DefaultMonsterStats → misc.json
      gems.rs                        ← ActiveSkills + GrantedEffects → gems.json
      skills.rs                      ← GrantedEffectsPerLevel → skills/<id>.json
      bases.rs                       ← BaseItemTypes + weapon/armour types → bases.json
      mods.rs                        ← Mods.dat → mods.json
      tree.rs                        ← PassiveSkills.dat + tree layout → tree/<ver>.json
data/
  .gitkeep                           ← placeholder so directory is tracked
  misc.json                          ← written by extractor
  gems.json
  bases.json
  mods.json
  tree/
    .gitkeep
scripts/
  extract.sh                         ← wrapper: calls data-extractor with correct args
```

---

### Task 1: Extend `data-extractor` dependencies and CLI skeleton

**Files:**
- Modify: `crates/data-extractor/Cargo.toml`
- Modify: `crates/data-extractor/src/main.rs`
- Create: `crates/data-extractor/src/error.rs`

- [ ] **Step 1: Update `crates/data-extractor/Cargo.toml`**

```toml
[package]
name = "data-extractor"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "data-extractor"
path = "src/main.rs"

[dependencies]
pob-calc = { path = "../pob-calc" }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
ggpk = "1.2.2"
clap = { version = "4", features = ["derive"] }
```

- [ ] **Step 2: Create `crates/data-extractor/src/error.rs`**

```rust
#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("GGPK I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found in GGPK: {0}")]
    FileNotFound(String),

    #[error("dat64 parse error in {file}: {message}")]
    Dat64Parse { file: String, message: String },

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}
```

- [ ] **Step 3: Rewrite `crates/data-extractor/src/main.rs`**

```rust
mod error;
mod ggpk_reader;
mod dat64;
mod transform;

use clap::Parser;
use std::path::{Path, PathBuf};

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
```

- [ ] **Step 4: Build to verify it compiles**

```bash
cargo build -p data-extractor
```

Expected: `Finished` with no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/data-extractor/src/error.rs crates/data-extractor/src/main.rs crates/data-extractor/Cargo.toml
git commit -m "feat(extractor): add CLI skeleton with clap and error type"
```

---

### Task 2: GGPK reader wrapper

**Files:**
- Create: `crates/data-extractor/src/ggpk_reader.rs`

The `ggpk` crate provides `GGPK::from_file(path)`, `ggpk.get_file(path)` returning a `GGPKFile`, and `ggpk.list_files()`. This task wraps it with convenient file-reading helpers.

- [ ] **Step 1: Create `crates/data-extractor/src/ggpk_reader.rs`**

```rust
use std::path::Path;
use crate::error::ExtractError;

pub struct GgpkReader {
    inner: ggpk::GGPK,
}

impl GgpkReader {
    pub fn open(path: &Path) -> Result<Self, ExtractError> {
        let inner = ggpk::GGPK::from_file(path)?;
        Ok(Self { inner })
    }

    /// Read raw bytes of a file inside the GGPK by its virtual path.
    /// `path` uses forward slashes, e.g. "Data/ActiveSkills.dat64"
    pub fn read_bytes(&self, path: &str) -> Result<Vec<u8>, ExtractError> {
        let file = self.inner.get_file(path);
        if file.data.is_empty() {
            // ggpk returns an empty GGPKFile for missing paths
            return Err(ExtractError::FileNotFound(path.to_string()));
        }
        Ok(file.data.to_vec())
    }

    /// Read a file as a UTF-8 string (for .json, .txt, .ot files).
    pub fn read_text(&self, path: &str) -> Result<String, ExtractError> {
        let bytes = self.read_bytes(path)?;
        // Game text files may be UTF-16LE; try UTF-8 first, then UTF-16LE
        match String::from_utf8(bytes.clone()) {
            Ok(s) => Ok(s),
            Err(_) => {
                // UTF-16LE: pairs of bytes, strip BOM (FF FE) if present
                let start = if bytes.starts_with(&[0xFF, 0xFE]) { 2 } else { 0 };
                let u16_iter = bytes[start..].chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]));
                String::from_utf16(&u16_iter.collect::<Vec<_>>())
                    .map_err(|_| ExtractError::FileNotFound(path.to_string()))
            }
        }
    }
}
```

- [ ] **Step 2: Add the stub transformer modules so it compiles**

Create `crates/data-extractor/src/transform/mod.rs`:

```rust
pub mod misc;
pub mod gems;
pub mod bases;
pub mod mods;
pub mod tree;
```

Create `crates/data-extractor/src/transform/misc.rs` (and repeat for `gems.rs`, `bases.rs`, `mods.rs`, `tree.rs`):

```rust
use std::path::Path;
use crate::{error::ExtractError, ggpk_reader::GgpkReader};

pub fn extract(_reader: &GgpkReader, _output: &Path) -> Result<(), ExtractError> {
    Ok(())
}
```

- [ ] **Step 3: Add `ggpk_reader` and `transform` to `main.rs` module list (already done in Task 1 Step 3)**

- [ ] **Step 4: Build to verify**

```bash
cargo build -p data-extractor
```

Expected: `Finished` with no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/data-extractor/src/ggpk_reader.rs crates/data-extractor/src/transform/
git commit -m "feat(extractor): add GGPK reader wrapper and transform stubs"
```

---

### Task 3: `dat64` binary parser

**Files:**
- Create: `crates/data-extractor/src/dat64.rs`
- Create: `crates/data-extractor/src/transform/mod.rs` (already exists — no change needed)

**Background:** The `.dat64` format has:
- 4 bytes: row count (u32 LE)
- row count × row_size bytes: fixed-width row data
- 8 bytes: `0xBBBBBBBBBBBBBBBB` sentinel marking start of variable-length data
- remaining bytes: variable-length string/array data (strings are UTF-16LE null-terminated)

Fields within rows are either fixed-width (Int=4 bytes, Bool=1 byte, Float=4 bytes, Long=8 bytes) or offsets into the variable section (String, Array — 8 bytes each: 4-byte count + 4-byte offset, or for String just an 8-byte offset).

For this project we only need a subset of fields from each table. The approach is: define the field offsets we care about manually, read them directly from the row bytes. This avoids implementing a full schema-driven parser.

- [ ] **Step 1: Create `crates/data-extractor/src/dat64.rs`**

```rust
use crate::error::ExtractError;

/// A parsed dat64 file.
pub struct Dat64 {
    pub row_count: usize,
    row_size: usize,
    rows: Vec<u8>,
    var_data: Vec<u8>,
}

impl Dat64 {
    /// Parse raw bytes from a .dat64 file.
    /// `row_size` must be determined by the caller from the table schema.
    pub fn parse(bytes: Vec<u8>, row_size: usize, file_name: &str) -> Result<Self, ExtractError> {
        if bytes.len() < 4 {
            return Err(ExtractError::Dat64Parse {
                file: file_name.to_string(),
                message: "file too short for row count".to_string(),
            });
        }
        let row_count = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let rows_end = 4 + row_count * row_size;
        if bytes.len() < rows_end + 8 {
            return Err(ExtractError::Dat64Parse {
                file: file_name.to_string(),
                message: format!("expected {} bytes for rows, file has {}", rows_end + 8, bytes.len()),
            });
        }
        // Validate sentinel
        let sentinel = &bytes[rows_end..rows_end + 8];
        if sentinel != &[0xBB; 8] {
            return Err(ExtractError::Dat64Parse {
                file: file_name.to_string(),
                message: format!("missing 0xBB sentinel at offset {rows_end}"),
            });
        }
        let var_data = bytes[rows_end + 8..].to_vec();
        let rows = bytes[4..rows_end].to_vec();
        Ok(Self { row_count, row_size, rows, var_data })
    }

    /// Read a u32 field at `byte_offset` within row `row_index`.
    pub fn read_u32(&self, row_index: usize, byte_offset: usize) -> u32 {
        let base = row_index * self.row_size + byte_offset;
        u32::from_le_bytes(self.rows[base..base + 4].try_into().unwrap())
    }

    /// Read a u64 field (used for row keys / foreign keys in dat64).
    pub fn read_u64(&self, row_index: usize, byte_offset: usize) -> u64 {
        let base = row_index * self.row_size + byte_offset;
        u64::from_le_bytes(self.rows[base..base + 8].try_into().unwrap())
    }

    /// Read a bool field (1 byte).
    pub fn read_bool(&self, row_index: usize, byte_offset: usize) -> bool {
        self.rows[row_index * self.row_size + byte_offset] != 0
    }

    /// Read a float (f32) field.
    pub fn read_f32(&self, row_index: usize, byte_offset: usize) -> f32 {
        let base = row_index * self.row_size + byte_offset;
        f32::from_le_bytes(self.rows[base..base + 4].try_into().unwrap())
    }

    /// Read a UTF-16LE string from the variable section.
    /// The field at `byte_offset` is an 8-byte offset into the var section.
    pub fn read_string(&self, row_index: usize, byte_offset: usize) -> String {
        let base = row_index * self.row_size + byte_offset;
        let offset = u64::from_le_bytes(self.rows[base..base + 8].try_into().unwrap()) as usize;
        self.read_var_string(offset)
    }

    fn read_var_string(&self, offset: usize) -> String {
        let data = &self.var_data;
        if offset >= data.len() {
            return String::new();
        }
        // UTF-16LE null-terminated
        let mut chars = Vec::new();
        let mut i = offset;
        while i + 1 < data.len() {
            let c = u16::from_le_bytes([data[i], data[i + 1]]);
            if c == 0 {
                break;
            }
            chars.push(c);
            i += 2;
        }
        String::from_utf16_lossy(&chars).to_string()
    }

    /// Read an array of u64 row-key references.
    /// The field at `byte_offset` is a 16-byte struct: 8-byte count + 8-byte offset.
    pub fn read_key_array(&self, row_index: usize, byte_offset: usize) -> Vec<u64> {
        let base = row_index * self.row_size + byte_offset;
        let count = u64::from_le_bytes(self.rows[base..base + 8].try_into().unwrap()) as usize;
        let offset = u64::from_le_bytes(self.rows[base + 8..base + 16].try_into().unwrap()) as usize;
        (0..count)
            .map(|i| {
                let pos = offset + i * 8;
                u64::from_le_bytes(self.var_data[pos..pos + 8].try_into().unwrap())
            })
            .collect()
    }
}
```

- [ ] **Step 2: Add unit tests for the parser**

Add to `crates/data-extractor/src/dat64.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_dat64(row_count: u32, row_bytes: &[u8], var_bytes: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&row_count.to_le_bytes());
        buf.extend_from_slice(row_bytes);
        buf.extend_from_slice(&[0xBB; 8]);
        buf.extend_from_slice(var_bytes);
        buf
    }

    #[test]
    fn reads_u32_field() {
        // 1 row, row_size=4, row contains u32 value 42
        let bytes = make_dat64(1, &42u32.to_le_bytes(), &[]);
        let dat = Dat64::parse(bytes, 4, "test.dat64").unwrap();
        assert_eq!(dat.read_u32(0, 0), 42);
    }

    #[test]
    fn reads_bool_field() {
        let bytes = make_dat64(1, &[1u8, 0, 0, 0], &[]);
        let dat = Dat64::parse(bytes, 4, "test.dat64").unwrap();
        assert!(dat.read_bool(0, 0));
        assert!(!dat.read_bool(0, 1));
    }

    #[test]
    fn reads_string_field() {
        // Row contains 8-byte offset = 0; var section contains "Hi" in UTF-16LE + null
        let offset: u64 = 0;
        let row = offset.to_le_bytes();
        // "Hi" in UTF-16LE: H=0x48,0x00  i=0x69,0x00  null=0x00,0x00
        let var = [0x48u8, 0x00, 0x69, 0x00, 0x00, 0x00];
        let bytes = make_dat64(1, &row, &var);
        let dat = Dat64::parse(bytes, 8, "test.dat64").unwrap();
        assert_eq!(dat.read_string(0, 0), "Hi");
    }

    #[test]
    fn rejects_missing_sentinel() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&42u32.to_le_bytes());
        // No sentinel
        assert!(Dat64::parse(bytes, 4, "bad.dat64").is_err());
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p data-extractor dat64
```

Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/data-extractor/src/dat64.rs
git commit -m "feat(extractor): implement dat64 binary parser with tests"
```

---

### Task 4: `misc.json` transformer — GameConstants and monster stat tables

**Files:**
- Modify: `crates/data-extractor/src/transform/misc.rs`

**Reference:** `third-party/PathOfBuilding/src/Export/Scripts/miscdata.lua`

The `misc.json` output contains:
- `gameConstants`: map of `Id → Value/Divisor` from `GameConstants.dat`
- `characterConstants`: key/value pairs from `Metadata/Characters/Character.ot`
- `monsterLifeTable`, `monsterDamageTable`, etc.: arrays from `DefaultMonsterStats.dat`

**GameConstants.dat64 row layout** (derived from POB's spec.lua):
- offset 0: Id (string, 8 bytes)
- offset 8: Value (i32, 4 bytes)
- offset 12: Divisor (i32, 4 bytes)
- row_size = 16

**DefaultMonsterStats.dat64 row layout:**
- offset 0: MonsterLife (i32, 4 bytes)
- offset 4: Evasion (i32, 4 bytes)
- offset 8: Accuracy (i32, 4 bytes)
- offset 12: Damage (i32, 4 bytes)
- offset 16: MinionLife (i32, 4 bytes)
- offset 20: MinionDamage (i32, 4 bytes)
- offset 24: AilmentThreshold (i32, 4 bytes)
- offset 28: MonsterPhysConversionMulti (f32, 4 bytes)
- row_size = 32

- [ ] **Step 1: Write the failing test first**

Add to the bottom of `crates/data-extractor/src/transform/misc.rs`:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn misc_json_schema() {
        // This test verifies the output JSON has the expected top-level keys.
        // It runs against a real GGPK only when GGPK_PATH env var is set;
        // otherwise it is skipped.
        let Some(ggpk_path) = std::env::var("GGPK_PATH").ok() else {
            eprintln!("GGPK_PATH not set, skipping integration test");
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let reader = crate::ggpk_reader::GgpkReader::open(std::path::Path::new(&ggpk_path)).unwrap();
        super::extract(&reader, tmp.path()).unwrap();
        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join("misc.json")).unwrap()
        ).unwrap();
        assert!(json.get("gameConstants").is_some(), "missing gameConstants");
        assert!(json.get("monsterLifeTable").is_some(), "missing monsterLifeTable");
        assert!(json.get("characterConstants").is_some(), "missing characterConstants");
    }
}
```

Add `tempfile = "3"` to `[dev-dependencies]` in `crates/data-extractor/Cargo.toml`.

- [ ] **Step 2: Run test to confirm it skips gracefully (no GGPK available in CI)**

```bash
cargo test -p data-extractor transform::misc
```

Expected output contains: `GGPK_PATH not set, skipping integration test`

- [ ] **Step 3: Implement `crates/data-extractor/src/transform/misc.rs`**

```rust
use std::path::Path;
use serde::Serialize;
use crate::{error::ExtractError, ggpk_reader::GgpkReader, dat64::Dat64};

#[derive(Serialize)]
struct MiscData {
    game_constants: std::collections::HashMap<String, f64>,
    character_constants: std::collections::HashMap<String, f64>,
    monster_life_table: Vec<i32>,
    monster_damage_table: Vec<i32>,
    monster_evasion_table: Vec<i32>,
    monster_accuracy_table: Vec<i32>,
    monster_ally_life_table: Vec<i32>,
    monster_ally_damage_table: Vec<i32>,
    monster_ailment_threshold_table: Vec<i32>,
    monster_phys_conversion_multi_table: Vec<f32>,
}

pub fn extract(reader: &GgpkReader, output: &Path) -> Result<(), ExtractError> {
    let game_constants = extract_game_constants(reader)?;
    let character_constants = extract_character_constants(reader)?;
    let (
        monster_life_table, monster_damage_table, monster_evasion_table,
        monster_accuracy_table, monster_ally_life_table, monster_ally_damage_table,
        monster_ailment_threshold_table, monster_phys_conversion_multi_table,
    ) = extract_monster_stats(reader)?;

    let data = MiscData {
        game_constants,
        character_constants,
        monster_life_table,
        monster_damage_table,
        monster_evasion_table,
        monster_accuracy_table,
        monster_ally_life_table,
        monster_ally_damage_table,
        monster_ailment_threshold_table,
        monster_phys_conversion_multi_table,
    };

    let json = serde_json::to_string_pretty(&data)?;
    std::fs::write(output.join("misc.json"), json)?;
    Ok(())
}

fn extract_game_constants(reader: &GgpkReader) -> Result<std::collections::HashMap<String, f64>, ExtractError> {
    // GameConstants.dat64: Id(str,8) Value(i32,4) Divisor(i32,4) = row_size 16
    let bytes = reader.read_bytes("Data/GameConstants.dat64")?;
    let dat = Dat64::parse(bytes, 16, "GameConstants.dat64")?;
    let mut map = std::collections::HashMap::new();
    for i in 0..dat.row_count {
        let id = dat.read_string(i, 0);
        let value = dat.read_u32(i, 8) as i32;
        let divisor = dat.read_u32(i, 12) as i32;
        if divisor != 0 {
            map.insert(id, value as f64 / divisor as f64);
        } else {
            map.insert(id, value as f64);
        }
    }
    Ok(map)
}

fn extract_character_constants(reader: &GgpkReader) -> Result<std::collections::HashMap<String, f64>, ExtractError> {
    // Character.ot is a text file with key = value lines inside Stats{} and Pathfinding{} blocks
    let text = reader.read_text("Metadata/Characters/Character.ot")?;
    let mut map = std::collections::HashMap::new();
    let mut in_block = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Stats") || trimmed.starts_with("Pathfinding") {
            in_block = true;
        } else if trimmed == "}" {
            in_block = false;
        } else if in_block {
            if let Some((key, val)) = trimmed.split_once('=') {
                let key = key.trim().to_string();
                let val = val.trim().trim_end_matches(';').trim();
                if let Ok(n) = val.parse::<f64>() {
                    map.insert(key, n);
                }
            }
        }
    }
    Ok(map)
}

fn extract_monster_stats(reader: &GgpkReader) -> Result<(Vec<i32>, Vec<i32>, Vec<i32>, Vec<i32>, Vec<i32>, Vec<i32>, Vec<i32>, Vec<f32>), ExtractError> {
    // DefaultMonsterStats.dat64 row_size = 32 (see plan header for layout)
    let bytes = reader.read_bytes("Data/DefaultMonsterStats.dat64")?;
    let dat = Dat64::parse(bytes, 32, "DefaultMonsterStats.dat64")?;
    let mut life = Vec::new();
    let mut damage = Vec::new();
    let mut evasion = Vec::new();
    let mut accuracy = Vec::new();
    let mut ally_life = Vec::new();
    let mut ally_damage = Vec::new();
    let mut ailment = Vec::new();
    let mut phys_conv = Vec::new();
    for i in 0..dat.row_count {
        life.push(dat.read_u32(i, 0) as i32);
        evasion.push(dat.read_u32(i, 4) as i32);
        accuracy.push(dat.read_u32(i, 8) as i32);
        damage.push(dat.read_u32(i, 12) as i32);
        ally_life.push(dat.read_u32(i, 16) as i32);
        ally_damage.push(dat.read_u32(i, 20) as i32);
        ailment.push(dat.read_u32(i, 24) as i32);
        phys_conv.push(dat.read_f32(i, 28));
    }
    Ok((life, damage, evasion, accuracy, ally_life, ally_damage, ailment, phys_conv))
}

#[cfg(test)]
mod tests {
    #[test]
    fn misc_json_schema() {
        let Some(ggpk_path) = std::env::var("GGPK_PATH").ok() else {
            eprintln!("GGPK_PATH not set, skipping integration test");
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let reader = crate::ggpk_reader::GgpkReader::open(std::path::Path::new(&ggpk_path)).unwrap();
        super::extract(&reader, tmp.path()).unwrap();
        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join("misc.json")).unwrap()
        ).unwrap();
        assert!(json.get("gameConstants").is_some(), "missing gameConstants");
        assert!(json.get("monsterLifeTable").is_some(), "missing monsterLifeTable");
        assert!(json.get("characterConstants").is_some(), "missing characterConstants");
    }
}
```

- [ ] **Step 4: Add `tempfile` dev-dependency**

In `crates/data-extractor/Cargo.toml` add under `[dev-dependencies]`:
```toml
tempfile = "3"
```

- [ ] **Step 5: Build**

```bash
cargo build -p data-extractor
```

Expected: `Finished` with no errors.

- [ ] **Step 6: Commit**

```bash
git add crates/data-extractor/src/transform/misc.rs crates/data-extractor/Cargo.toml
git commit -m "feat(extractor): implement misc.json transformer (GameConstants, monster stats)"
```

---

### Task 5: `gems.json` transformer

**Files:**
- Modify: `crates/data-extractor/src/transform/gems.rs`

**Reference:** `third-party/PathOfBuilding/src/Export/Scripts/skills.lua` and `skillGemList.lua`.

The `gems.json` output is a map of gem ID → gem metadata:
```json
{
  "Fireball": {
    "id": "Fireball",
    "display_name": "Fireball",
    "is_support": false,
    "skill_types": [1, 3, 7],
    "granted_effect_id": "Fireball"
  }
}
```

**ActiveSkills.dat64 row layout** (key fields only, row_size = 72):
- offset 0: Id (str, 8 bytes)
- offset 8: DisplayedName (str, 8 bytes)
- offset 16: Description (str, 8 bytes)
- offset 24: SkillTotemLifeMultiplier (f32, 4 bytes)
- offset 28: Types (key array, 16 bytes) — references ActiveSkillType.dat
- offset 44: IsSupport (bool, 1 byte)

**GrantedEffects.dat64** (row_size = 40):
- offset 0: Id (str, 8 bytes)
- offset 8: IsSupport (bool, 1 byte)

**SkillGems.dat64** (row_size = 64):
- offset 0: BaseItemTypesKey (u64, 8 bytes) → row index into BaseItemTypes.dat
- offset 8: GrantedEffectsKey (u64, 8 bytes) → row index into GrantedEffects.dat
- offset 16: SupportGemLetter (str, 8 bytes)

**Note:** Row offsets above are approximations based on POB's spec.lua field ordering. You may need to adjust them by running the extractor against a real GGPK and inspecting output. The test below will catch misalignment.

- [ ] **Step 1: Write the failing test**

Replace `crates/data-extractor/src/transform/gems.rs` with:

```rust
use std::path::Path;
use serde::Serialize;
use crate::{error::ExtractError, ggpk_reader::GgpkReader};

#[derive(Serialize)]
pub struct GemData {
    pub id: String,
    pub display_name: String,
    pub is_support: bool,
    pub skill_types: Vec<u32>,
}

pub fn extract(_reader: &GgpkReader, _output: &Path) -> Result<(), ExtractError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn gems_json_schema() {
        let Some(ggpk_path) = std::env::var("GGPK_PATH").ok() else {
            eprintln!("GGPK_PATH not set, skipping integration test");
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let reader = crate::ggpk_reader::GgpkReader::open(std::path::Path::new(&ggpk_path)).unwrap();
        super::extract(&reader, tmp.path()).unwrap();
        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join("gems.json")).unwrap()
        ).unwrap();
        let obj = json.as_object().expect("gems.json must be an object");
        assert!(!obj.is_empty(), "gems.json must not be empty");
        // Spot-check: Fireball should exist as an active skill
        let fireball = obj.get("Fireball").expect("Fireball gem not found");
        assert_eq!(fireball["is_support"], false);
        assert!(!fireball["skill_types"].as_array().unwrap().is_empty());
    }
}
```

- [ ] **Step 2: Run test to confirm stub fails correctly**

```bash
GGPK_PATH=/path/to/Content.ggpk cargo test -p data-extractor transform::gems 2>&1 | tail -5
```

Expected: test fails because `gems.json` doesn't exist yet (or is skipped if no GGPK).

- [ ] **Step 3: Implement the transformer**

Replace the `extract` function body in `crates/data-extractor/src/transform/gems.rs`:

```rust
use std::path::Path;
use std::collections::HashMap;
use serde::Serialize;
use crate::{error::ExtractError, ggpk_reader::GgpkReader, dat64::Dat64};

#[derive(Serialize)]
pub struct GemData {
    pub id: String,
    pub display_name: String,
    pub is_support: bool,
    pub skill_types: Vec<u32>,
}

pub fn extract(reader: &GgpkReader, output: &Path) -> Result<(), ExtractError> {
    // ActiveSkillType.dat64: just need row indices for type IDs
    // row_size = 8 (Id string only)
    let ast_bytes = reader.read_bytes("Data/ActiveSkillType.dat64")?;
    let ast = Dat64::parse(ast_bytes, 8, "ActiveSkillType.dat64")?;
    let mut type_ids: Vec<u32> = Vec::new();
    for i in 0..ast.row_count {
        // Store the 1-based type number (POB uses _rowIndex+1)
        type_ids.push((i + 1) as u32);
    }

    // ActiveSkills.dat64: row_size = 72 (see plan header)
    let as_bytes = reader.read_bytes("Data/ActiveSkills.dat64")?;
    let active_skills = Dat64::parse(as_bytes, 72, "ActiveSkills.dat64")?;

    let mut gems: HashMap<String, GemData> = HashMap::new();
    for i in 0..active_skills.row_count {
        let id = active_skills.read_string(i, 0);
        if id.is_empty() {
            continue;
        }
        let display_name = active_skills.read_string(i, 8);
        let is_support = active_skills.read_bool(i, 44);
        // Types is a key-array at offset 28 (16 bytes: count + offset)
        let type_row_indices = active_skills.read_key_array(i, 28);
        let skill_types: Vec<u32> = type_row_indices.iter()
            .filter_map(|&idx| type_ids.get(idx as usize).copied())
            .collect();

        gems.insert(id.clone(), GemData { id, display_name, is_support, skill_types });
    }

    let json = serde_json::to_string_pretty(&gems)?;
    std::fs::write(output.join("gems.json"), json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn gems_json_schema() {
        let Some(ggpk_path) = std::env::var("GGPK_PATH").ok() else {
            eprintln!("GGPK_PATH not set, skipping integration test");
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let reader = crate::ggpk_reader::GgpkReader::open(std::path::Path::new(&ggpk_path)).unwrap();
        super::extract(&reader, tmp.path()).unwrap();
        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join("gems.json")).unwrap()
        ).unwrap();
        let obj = json.as_object().expect("gems.json must be an object");
        assert!(!obj.is_empty(), "gems.json must not be empty");
        let fireball = obj.get("Fireball").expect("Fireball gem not found");
        assert_eq!(fireball["is_support"], false);
        assert!(!fireball["skill_types"].as_array().unwrap().is_empty());
    }
}
```

- [ ] **Step 4: Build**

```bash
cargo build -p data-extractor
```

- [ ] **Step 5: Commit**

```bash
git add crates/data-extractor/src/transform/gems.rs
git commit -m "feat(extractor): implement gems.json transformer"
```

---

### Task 6: `bases.json`, `mods.json`, `tree/<ver>.json` transformers

These three transformers follow the exact same pattern as Task 5. They are described here with their schemas. Each should have:
1. A struct with `#[derive(Serialize)]`
2. A test gated on `GGPK_PATH` env var with a spot-check assertion
3. Implementation reading the relevant `.dat64` files

**Files:**
- Modify: `crates/data-extractor/src/transform/bases.rs`
- Modify: `crates/data-extractor/src/transform/mods.rs`
- Modify: `crates/data-extractor/src/transform/tree.rs`

#### 6a: `bases.json`

**Reference:** `third-party/PathOfBuilding/src/Export/Scripts/bases.lua`

Output struct:
```rust
#[derive(Serialize)]
pub struct BaseItem {
    pub id: String,
    pub name: String,
    pub item_class: String,   // e.g. "Body Armour", "Sword"
    pub base_str: u32,        // implicit armour/str requirement
    pub base_dex: u32,
    pub base_int: u32,
    pub drop_level: u32,
}
```

**BaseItemTypes.dat64 row layout** (row_size = 96):
- offset 0: Id (str, 8)
- offset 8: Name (str, 8)
- offset 16: ItemClassesKey (u64, 8) → ItemClasses.dat row index
- offset 24: DropLevel (u32, 4)

**ItemClasses.dat64 row layout** (row_size = 32):
- offset 0: Id (str, 8)
- offset 8: Name (str, 8)

Output file: `data/bases.json` — array of `BaseItem`.

Spot-check assertion: `"Short Bow"` exists in the output array with `item_class == "Bow"`.

#### 6b: `mods.json`

**Reference:** `third-party/PathOfBuilding/src/Export/Scripts/mods.lua`

Output struct:
```rust
#[derive(Serialize)]
pub struct ModEntry {
    pub id: String,
    pub name: String,          // display text (stat description)
    pub mod_type: String,      // "Prefix" | "Suffix" | "Corrupted" | "Unique" | ...
    pub domain: u32,           // 1=item, 2=flask, 5=skill, etc.
    pub generation_type: u32,
}
```

**Mods.dat64 row layout** (row_size = 104):
- offset 0: Id (str, 8)
- offset 8: Name (str, 8)
- offset 16: GenerationType (u32, 4)
- offset 20: Domain (u32, 4)

Output file: `data/mods.json` — map of `Id → ModEntry`.

Spot-check assertion: result map is not empty; a known prefix mod like `"IncreasedLife1"` or any mod with `domain == 1` exists.

#### 6c: `tree/<ver>.json`

**Reference:** `third-party/PathOfBuilding/src/Export/Tree/tree.lua` for the schema (the GGPK export produces the same structure).

The passive tree is stored as a JSON file inside the GGPK at `Data/PassiveSkillTreeuiExtraImages.json` (layout file) and individual passive node data comes from `PassiveSkills.dat64`.

For the tree JSON, read the raw tree data file from the GGPK:
- Path: `Data/PassiveTree.json` (for PoE 1 current league)
- This is already a JSON file — read it with `reader.read_text()`, parse it, and write it out to `data/tree/poe1_current.json`

```rust
pub fn extract(reader: &GgpkReader, output: &Path) -> Result<(), ExtractError> {
    // PoE 1 current tree
    let tree_json = reader.read_text("Data/PassiveTree.json")?;
    // Validate it parses as JSON
    let _: serde_json::Value = serde_json::from_str(&tree_json)?;
    std::fs::write(output.join("tree").join("poe1_current.json"), tree_json)?;
    Ok(())
}
```

Spot-check assertion: `data/tree/poe1_current.json` is valid JSON with a `"nodes"` key.

- [ ] **Step 1: Implement all three transformers following the pattern above**

For each transformer (`bases.rs`, `mods.rs`, `tree.rs`):
1. Write the `#[derive(Serialize)]` struct
2. Write the GGPK-gated test with spot-check assertion
3. Implement the `extract` function

- [ ] **Step 2: Build**

```bash
cargo build -p data-extractor
```

- [ ] **Step 3: Run unit tests (should skip gracefully without GGPK)**

```bash
cargo test -p data-extractor
```

Expected: all tests either pass or print `"GGPK_PATH not set, skipping integration test"`.

- [ ] **Step 4: Commit**

```bash
git add crates/data-extractor/src/transform/bases.rs \
        crates/data-extractor/src/transform/mods.rs \
        crates/data-extractor/src/transform/tree.rs
git commit -m "feat(extractor): implement bases, mods, and tree transformers"
```

---

### Task 7: `data/` placeholder, `extract.sh` wrapper, and CI note

**Files:**
- Create: `data/.gitkeep`
- Create: `data/tree/.gitkeep`
- Create: `scripts/extract.sh`

- [ ] **Step 1: Create output directory placeholders**

```bash
mkdir -p data/tree
touch data/.gitkeep data/tree/.gitkeep
```

- [ ] **Step 2: Create `scripts/extract.sh`**

```bash
#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 /path/to/Content.ggpk"
  exit 1
fi

GGPK_PATH="$1"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT_DIR="$REPO_ROOT/data"

echo "Building data-extractor..."
cargo build -p data-extractor --release

echo "Extracting data from $GGPK_PATH..."
"$REPO_ROOT/target/release/data-extractor" "$GGPK_PATH" --output "$OUTPUT_DIR"

echo "Extraction complete. Review changes in $OUTPUT_DIR before committing."
```

```bash
chmod +x scripts/extract.sh
```

- [ ] **Step 3: Add `data/*.json` and `data/tree/*.json` to `.gitignore` with an override to allow `.gitkeep`**

Append to `.gitignore`:
```
# Generated data files (not committed; run scripts/extract.sh to regenerate)
# data/*.json and data/tree/*.json are regenerated from Content.ggpk
# Uncomment the lines below if you want to commit extracted data:
# !data/*.json
# !data/tree/*.json
```

**Note for the team:** After running `scripts/extract.sh` successfully against a real GGPK, the generated JSON files should be committed. Uncomment the `!data/*.json` lines in `.gitignore` and `git add data/` to include them.

- [ ] **Step 4: Commit**

```bash
git add data/.gitkeep data/tree/.gitkeep scripts/extract.sh .gitignore
git commit -m "chore: add data output directory, extract.sh wrapper"
```

---

**Phase 2 complete.** Run `scripts/extract.sh /path/to/Content.ggpk` with a real GGPK to populate `data/`. Review the output JSON files, then commit them. Proceed to Phase 3 (core types).
