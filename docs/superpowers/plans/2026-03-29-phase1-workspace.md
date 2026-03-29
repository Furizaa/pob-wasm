# Phase 1: Workspace Scaffolding — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a Cargo workspace with three crate skeletons (`pob-calc`, `pob-wasm`, `data-extractor`), add the WASM toolchain, and verify everything compiles.

**Architecture:** A Cargo workspace at the repo root owns all crates. `pob-calc` is a pure library with no WASM dependencies. `pob-wasm` is a `cdylib` crate that depends on `pob-calc` and `wasm-bindgen`. `data-extractor` is a native binary that depends on `pob-calc`.

**Tech Stack:** Rust 1.82 stable, `wasm-bindgen 0.2`, `wasm32-unknown-unknown` target, `wasm-pack 0.13`

---

## File Map

```
Cargo.toml                          ← workspace root
crates/
  pob-calc/
    Cargo.toml
    src/
      lib.rs
  pob-wasm/
    Cargo.toml
    src/
      lib.rs
  data-extractor/
    Cargo.toml
    src/
      main.rs
```

---

### Task 1: Cargo workspace root

**Files:**
- Create: `Cargo.toml`

- [ ] **Step 1: Create the workspace `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
    "crates/pob-calc",
    "crates/pob-wasm",
    "crates/data-extractor",
]

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
```

- [ ] **Step 2: Verify no existing `Cargo.toml` at root conflicts**

Run: `ls Cargo.toml 2>/dev/null && echo EXISTS || echo OK`
Expected: `OK` (file did not exist before)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add Cargo workspace root"
```

---

### Task 2: `pob-calc` crate skeleton

**Files:**
- Create: `crates/pob-calc/Cargo.toml`
- Create: `crates/pob-calc/src/lib.rs`

- [ ] **Step 1: Create directory structure**

```bash
mkdir -p crates/pob-calc/src
```

- [ ] **Step 2: Create `crates/pob-calc/Cargo.toml`**

```toml
[package]
name = "pob-calc"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
```

- [ ] **Step 3: Create `crates/pob-calc/src/lib.rs`**

```rust
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
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p pob-calc
```

Expected output contains: `test tests::version_is_semver ... ok`

- [ ] **Step 5: Commit**

```bash
git add crates/pob-calc/
git commit -m "chore: add pob-calc crate skeleton"
```

---

### Task 3: `data-extractor` crate skeleton

**Files:**
- Create: `crates/data-extractor/Cargo.toml`
- Create: `crates/data-extractor/src/main.rs`

- [ ] **Step 1: Create directory structure**

```bash
mkdir -p crates/data-extractor/src
```

- [ ] **Step 2: Create `crates/data-extractor/Cargo.toml`**

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
```

- [ ] **Step 3: Create `crates/data-extractor/src/main.rs`**

```rust
fn main() {
    println!("data-extractor v{}", env!("CARGO_PKG_VERSION"));
}
```

- [ ] **Step 4: Build to verify it compiles**

```bash
cargo build -p data-extractor
```

Expected: `Finished` with no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/data-extractor/
git commit -m "chore: add data-extractor crate skeleton"
```

---

### Task 4: `pob-wasm` crate skeleton + WASM toolchain

**Files:**
- Create: `crates/pob-wasm/Cargo.toml`
- Create: `crates/pob-wasm/src/lib.rs`

- [ ] **Step 1: Install the WASM target and wasm-pack**

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack --version "^0.13"
```

Expected: both complete without error. Verify with:
```bash
rustup target list --installed | grep wasm32
wasm-pack --version
```
Expected: `wasm32-unknown-unknown` in target list; `wasm-pack 0.13.x`

- [ ] **Step 2: Create directory structure**

```bash
mkdir -p crates/pob-wasm/src
```

- [ ] **Step 3: Create `crates/pob-wasm/Cargo.toml`**

```toml
[package]
name = "pob-wasm"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
pob-calc = { path = "../pob-calc" }
wasm-bindgen = "0.2"
serde = { workspace = true }
serde_json = { workspace = true }

[dev-dependencies]
wasm-bindgen-test = "0.3"
```

- [ ] **Step 4: Create `crates/pob-wasm/src/lib.rs`**

```rust
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
```

- [ ] **Step 5: Build the WASM package to verify it compiles**

```bash
wasm-pack build crates/pob-wasm --target web --dev
```

Expected: `[INFO]: Your wasm pkg is ready to publish at crates/pob-wasm/pkg/`

- [ ] **Step 6: Run native unit tests**

```bash
cargo test -p pob-wasm
```

Expected: `test tests::version_roundtrip ... ok`

- [ ] **Step 7: Add generated `pkg/` to `.gitignore`**

Append to the root `.gitignore` (create it if it doesn't exist):

```
/crates/pob-wasm/pkg/
/target/
```

- [ ] **Step 8: Commit**

```bash
git add crates/pob-wasm/ .gitignore
git commit -m "chore: add pob-wasm crate skeleton with wasm-bindgen"
```

---

### Task 5: CI smoke check — build all crates

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create `.github/workflows/ci.yml`**

```bash
mkdir -p .github/workflows
```

```yaml
name: CI

on:
  push:
    branches: ["main"]
  pull_request:

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          submodules: recursive

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown

      - name: Install wasm-pack
        run: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

      - name: Run native tests
        run: cargo test --workspace --exclude pob-wasm

      - name: Build WASM package
        run: wasm-pack build crates/pob-wasm --target web --dev
```

- [ ] **Step 2: Verify CI file is valid YAML**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))" && echo OK
```

Expected: `OK`

- [ ] **Step 3: Commit**

```bash
git add .github/
git commit -m "chore: add CI workflow for native tests and WASM build"
```

---

**Phase 1 complete.** The workspace compiles natively and to WASM. Proceed to Phase 2 (data extractor).
