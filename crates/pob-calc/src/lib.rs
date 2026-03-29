//! pob-calc: Path of Building calculation engine port.
//!
//! This crate has zero WebAssembly dependencies and compiles to native
//! binaries for testing. The pob-wasm crate wraps it for WASM targets.

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver() {
        let v = version();
        assert!(v.contains('.'), "expected semver, got {v}");
    }
}
