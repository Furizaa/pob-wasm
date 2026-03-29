use wasm_bindgen::prelude::*;

/// Returns the engine version string. Used to verify the WASM module loads.
#[wasm_bindgen]
pub fn version() -> String {
    pob_calc::version().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_roundtrip() {
        assert!(!version().is_empty());
    }
}
