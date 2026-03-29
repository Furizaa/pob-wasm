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
                message: format!(
                    "expected {} bytes for rows, file has {}",
                    rows_end + 8,
                    bytes.len()
                ),
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
        Ok(Self {
            row_count,
            row_size,
            rows,
            var_data,
        })
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
        let offset =
            u64::from_le_bytes(self.rows[base + 8..base + 16].try_into().unwrap()) as usize;
        (0..count)
            .map(|i| {
                let pos = offset + i * 8;
                u64::from_le_bytes(self.var_data[pos..pos + 8].try_into().unwrap())
            })
            .collect()
    }
}

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
