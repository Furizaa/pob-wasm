use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt};
use oozextract::Extractor;

use crate::error::ExtractError;

// ---------------------------------------------------------------------------
// Path hash (MurmurHash64A, used since PoE patch 3.21.2)
//
// Reference: https://github.com/poe-tool-dev/ggpk.discussion/wiki/Bundle-scheme
// The path is lowercased; no ++ salt. Seed = 0x1337b33f.
// ---------------------------------------------------------------------------
pub fn filepath_hash(path: &str) -> u64 {
    murmur_hash64a(path.to_lowercase().as_bytes(), 0x1337b33f)
}

fn murmur_hash64a(data: &[u8], seed: u64) -> u64 {
    const M: u64 = 0xc6a4a7935bd1e995;
    const R: u32 = 47;

    let len = data.len();
    let mut h: u64 = seed ^ ((len as u64).wrapping_mul(M));

    let chunks = len / 8;
    for i in 0..chunks {
        let offset = i * 8;
        let mut k = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
        k = k.wrapping_mul(M);
        k ^= k >> R;
        k = k.wrapping_mul(M);
        h ^= k;
        h = h.wrapping_mul(M);
    }

    let tail = &data[chunks * 8..];
    if !tail.is_empty() {
        let mut k: u64 = 0;
        for (i, &byte) in tail.iter().enumerate() {
            k ^= (byte as u64) << (i * 8);
        }
        h ^= k;
        h = h.wrapping_mul(M);
    }

    h ^= h >> R;
    h = h.wrapping_mul(M);
    h ^= h >> R;
    h
}

// ---------------------------------------------------------------------------
// PoE multi-chunk bundle decompressor
// ---------------------------------------------------------------------------
fn decompress_bundle(src: &[u8]) -> Result<Vec<u8>, ExtractError> {
    let mut c = Cursor::new(src);

    let _uc_field = c.read_u32::<LittleEndian>().map_err(io_err)?;
    let _c_total = c.read_u32::<LittleEndian>().map_err(io_err)?;
    let _head_size = c.read_u32::<LittleEndian>().map_err(io_err)?;
    let _first_enc = c.read_u32::<LittleEndian>().map_err(io_err)?;
    let _unk1 = c.read_u32::<LittleEndian>().map_err(io_err)?;
    let uncompressed_size = c.read_u64::<LittleEndian>().map_err(io_err)? as usize;
    let _c_total2 = c.read_u64::<LittleEndian>().map_err(io_err)?;
    let chunk_count = c.read_u32::<LittleEndian>().map_err(io_err)? as usize;
    let chunk_unpacked_size = c.read_u32::<LittleEndian>().map_err(io_err)? as usize;
    let _unk2 = c.read_u32::<LittleEndian>().map_err(io_err)?;
    let _unk3 = c.read_u32::<LittleEndian>().map_err(io_err)?;
    let _unk4 = c.read_u32::<LittleEndian>().map_err(io_err)?;
    let _unk5 = c.read_u32::<LittleEndian>().map_err(io_err)?;

    let mut chunk_sizes: Vec<usize> = Vec::with_capacity(chunk_count);
    for _ in 0..chunk_count {
        chunk_sizes.push(c.read_u32::<LittleEndian>().map_err(io_err)? as usize);
    }

    let mut output = vec![0u8; uncompressed_size];
    let mut chunk_data_offset = c.position() as usize;
    let mut bytes_written = 0usize;

    for i in 0..chunk_count {
        let chunk_src = src
            .get(chunk_data_offset..chunk_data_offset + chunk_sizes[i])
            .ok_or_else(|| ExtractError::Dat64Parse {
                file: "bundle".into(),
                message: format!("chunk {i} out of bounds"),
            })?;

        let chunk_out_size = chunk_unpacked_size.min(uncompressed_size - bytes_written);
        let chunk_dst = output
            .get_mut(bytes_written..bytes_written + chunk_out_size)
            .ok_or_else(|| ExtractError::Dat64Parse {
                file: "bundle".into(),
                message: format!("output slice {i} out of bounds"),
            })?;

        Extractor::new()
            .read_from_slice(chunk_src, chunk_dst)
            .map_err(|e| ExtractError::Dat64Parse {
                file: "bundle".into(),
                message: format!("chunk {i} ooz decompress: {e}"),
            })?;

        bytes_written += chunk_out_size;
        chunk_data_offset += chunk_sizes[i];
    }

    Ok(output)
}

fn io_err(e: std::io::Error) -> ExtractError {
    ExtractError::Io(e)
}

// ---------------------------------------------------------------------------
// Path string decoder for the bundle index path section
// ---------------------------------------------------------------------------
fn build_paths(bytes: &[u8]) -> Vec<String> {
    let mut c = Cursor::new(bytes);
    let mut generation_phase = false;
    let mut table: Vec<String> = vec![];
    let mut files: Vec<String> = vec![];

    while c.position() + 4 <= bytes.len() as u64 {
        let index = match c.read_u32::<LittleEndian>() {
            Ok(v) => v as usize,
            Err(_) => break,
        };
        if index == 0 {
            generation_phase = !generation_phase;
            if generation_phase {
                table.clear();
            }
        } else {
            let mut text_bytes = Vec::new();
            loop {
                match c.read_u8() {
                    Ok(0) | Err(_) => break,
                    Ok(b) => text_bytes.push(b),
                }
            }
            let mut text = String::from_utf8(text_bytes).unwrap_or_default();
            if index <= table.len() {
                text = format!("{}{}", table[index - 1], text);
            }
            if generation_phase {
                table.push(text);
            } else {
                files.push(text);
            }
        }
    }
    files
}

// ---------------------------------------------------------------------------
// Bundle index
// ---------------------------------------------------------------------------
struct IndexEntry {
    bundle_name: String,
    offset: u32,
    size: u32,
}

struct BundleIndex {
    files: HashMap<u64, IndexEntry>,
    /// All decoded path strings (for diagnostics / discovery)
    pub paths: Vec<String>,
}

impl BundleIndex {
    fn parse(index_bytes: Vec<u8>) -> Result<Self, ExtractError> {
        let data = decompress_bundle(&index_bytes)?;
        let mut c = Cursor::new(&data[..]);

        let bundle_count = c.read_u32::<LittleEndian>().map_err(io_err)? as usize;
        let mut bundle_names: Vec<String> = Vec::with_capacity(bundle_count);
        for _ in 0..bundle_count {
            let name_len = c.read_u32::<LittleEndian>().map_err(io_err)? as usize;
            let mut name_bytes = vec![0u8; name_len];
            use std::io::Read;
            c.read_exact(&mut name_bytes).map_err(io_err)?;
            let _uncompressed_size = c.read_u32::<LittleEndian>().map_err(io_err)?;
            bundle_names.push(String::from_utf8(name_bytes).unwrap_or_default());
        }

        let file_count = c.read_u32::<LittleEndian>().map_err(io_err)? as usize;
        let mut files: HashMap<u64, IndexEntry> = HashMap::with_capacity(file_count);
        for _ in 0..file_count {
            let hash = c.read_u64::<LittleEndian>().map_err(io_err)?;
            let bundle_idx = c.read_u32::<LittleEndian>().map_err(io_err)? as usize;
            let offset = c.read_u32::<LittleEndian>().map_err(io_err)?;
            let size = c.read_u32::<LittleEndian>().map_err(io_err)?;
            let bundle_name = bundle_names.get(bundle_idx).cloned().unwrap_or_default();
            files.insert(
                hash,
                IndexEntry {
                    bundle_name,
                    offset,
                    size,
                },
            );
        }

        // Skip path remap table
        let path_rep_count = c.read_u32::<LittleEndian>().map_err(io_err)? as usize;
        c.set_position(c.position() + 20 * path_rep_count as u64);

        // Decode path strings section
        let remaining_start = c.position() as usize;
        let remaining = &data[remaining_start..];
        let paths = if !remaining.is_empty() {
            decompress_bundle(remaining)
                .map(|pd| build_paths(&pd))
                .unwrap_or_default()
        } else {
            vec![]
        };

        Ok(BundleIndex { files, paths })
    }

    fn get(&self, path: &str) -> Option<&IndexEntry> {
        self.files.get(&filepath_hash(path))
    }

    /// Find all paths in the index matching a substring (case-insensitive).
    pub fn find_paths(&self, needle: &str) -> Vec<&str> {
        let needle_lc = needle.to_lowercase();
        self.paths
            .iter()
            .filter(|p| p.to_lowercase().contains(&needle_lc))
            .map(|s| s.as_str())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Public GgpkReader
// ---------------------------------------------------------------------------
pub struct GgpkReader {
    ggpk: ggpk::GGPK,
    index: BundleIndex,
}

impl GgpkReader {
    pub fn open(path: &Path) -> Result<Self, ExtractError> {
        let ggpk = ggpk::GGPK::from_file(path)?;
        let index_bytes = read_ggpk_file(&ggpk, "Bundles2/_.index.bin")?;
        let index = BundleIndex::parse(index_bytes)?;
        Ok(Self { ggpk, index })
    }

    /// Find paths in the index matching a substring (for diagnostics).
    pub fn find_paths(&self, needle: &str) -> Vec<&str> {
        self.index.find_paths(needle)
    }

    /// Read raw bytes of a file by its virtual path (e.g. "Data/ActiveSkills.dat64").
    /// Paths are case-insensitive (lowercased before hashing).
    pub fn read_bytes(&self, path: &str) -> Result<Vec<u8>, ExtractError> {
        let entry = self
            .index
            .get(path)
            .ok_or_else(|| ExtractError::FileNotFound(path.to_string()))?;

        let bundle_path = format!("Bundles2/{}.bundle.bin", entry.bundle_name);
        let bundle_bytes = read_ggpk_file(&self.ggpk, &bundle_path)?;
        let unpacked = decompress_bundle(&bundle_bytes)?;

        let start = entry.offset as usize;
        let end = start + entry.size as usize;
        unpacked
            .get(start..end)
            .map(|s| s.to_vec())
            .ok_or_else(|| ExtractError::Dat64Parse {
                file: bundle_path.clone(),
                message: format!(
                    "slice {start}..{end} out of bounds (bundle size {})",
                    unpacked.len()
                ),
            })
    }

    /// Read a file as a UTF-8 string (for .json, .txt, .ot files).
    pub fn read_text(&self, path: &str) -> Result<String, ExtractError> {
        let bytes = self.read_bytes(path)?;
        match String::from_utf8(bytes.clone()) {
            Ok(s) => Ok(s),
            Err(_) => {
                let start = if bytes.starts_with(&[0xFF, 0xFE]) {
                    2
                } else {
                    0
                };
                let u16_iter = bytes[start..]
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]));
                String::from_utf16(&u16_iter.collect::<Vec<_>>())
                    .map_err(|_| ExtractError::FileNotFound(path.to_string()))
            }
        }
    }
}

/// Read a file from a GGPK using the ggpk crate.
/// Paths use NO leading slash (e.g. "Bundles2/_.index.bin").
fn read_ggpk_file(ggpk: &ggpk::GGPK, path: &str) -> Result<Vec<u8>, ExtractError> {
    let ggpk_path = path.trim_start_matches('/').to_string();
    let result = std::panic::catch_unwind(|| ggpk.get_file(&ggpk_path).bytes().to_vec());
    result.map_err(|_| ExtractError::FileNotFound(path.to_string()))
}
