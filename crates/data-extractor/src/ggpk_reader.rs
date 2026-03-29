use crate::error::ExtractError;
use std::path::Path;

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
    /// The GGPK library stores files with a leading slash, so we normalise here.
    pub fn read_bytes(&self, path: &str) -> Result<Vec<u8>, ExtractError> {
        // Normalise: ensure leading slash to match GGPK's internal path format
        let ggpk_path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        };

        // get_file panics on missing files; use catch_unwind to convert to an error
        let result = std::panic::catch_unwind(|| self.inner.get_file(&ggpk_path).bytes().to_vec());

        result.map_err(|_| ExtractError::FileNotFound(path.to_string()))
    }

    /// Read a file as a UTF-8 string (for .json, .txt, .ot files).
    pub fn read_text(&self, path: &str) -> Result<String, ExtractError> {
        let bytes = self.read_bytes(path)?;
        // Game text files may be UTF-16LE; try UTF-8 first, then UTF-16LE
        match String::from_utf8(bytes.clone()) {
            Ok(s) => Ok(s),
            Err(_) => {
                // UTF-16LE: pairs of bytes, strip BOM (FF FE) if present
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
