# Oracle Parity Testing in CI — Notes for Later

**Context:** Deferred after Phase 5. The oracle test infrastructure exists (`scripts/gen_oracle.lua`, `scripts/run_oracle.sh`, `crates/pob-calc/tests/oracle.rs`) but is not wired into CI for real parity comparison.

---

## Current state (end of Phase 4)

Three oracle test functions exist in `crates/pob-calc/tests/oracle.rs`:

| Test | Always runs | Requires `DATA_DIR` | Requires expected JSON |
|---|---|---|---|
| `oracle_melee_str_parses` | yes | no | no |
| `oracle_melee_str_calculate_returns_result` | yes (skips if no `DATA_DIR`) | yes | no |
| `oracle_melee_str_life_matches_pob` | yes (skips if no `DATA_DIR`) | yes | yes |

The parity check (`life_matches_pob`) only runs when `DATA_DIR` is set. No CI job sets it.

## What's missing

Three distinct problems to solve:

1. **Generating expected JSON** — requires running POB's Lua engine against real game data. `scripts/run_oracle.sh` works locally but needs the POB environment (see below).

2. **Committing expected JSON** — `melee_str.expected.json` is a placeholder (`{"output":{"Life":1118,...}}`), not real POB output. Needs to be regenerated once the POB environment is available.

3. **Running parity comparison in CI** — `DATA_DIR` must point to real game data, and the expected JSON must be committed and up-to-date.

## POB environment requirements for oracle generation

POB's `HeadlessWrapper.lua` was designed for a Windows runtime with bundled C extensions. On macOS/Linux with plain LuaJIT:

| Problem | Fix in `gen_oracle.lua` |
|---|---|
| `GetVirtualScreenSize` is nil | Pre-define returning `1920, 1080` |
| `lua-utf8` C extension missing | Pure-Lua stub in `package.preload` |
| `sha1` path | `runtime/lua/?/init.lua` in `package.path` |
| Relative `dofile` paths | Must `cd` to `third-party/PathOfBuilding/src/` — handled by `run_oracle.sh` |

For CI, POB provides a Docker image that bundles all required native extensions:
`ghcr.io/pathofbuildingcommunity/pathofbuilding-tests:latest`

## Decision needed before implementing

**How often should parity checks run?**

- **On every PR** — catches regressions immediately; requires committing expected JSON and pulling Docker image on every CI run (~1-2 min overhead per job)
- **Nightly / on-demand** — cheaper; parity drift is caught eventually but not per-commit

Recommendation: on every PR, using committed expected JSON (generated once, re-generated when POB updates). The Docker image pull is acceptable overhead.

## Suggested implementation (when ready)

1. Generate real expected JSON files using the POB Docker container:
   ```bash
   docker run --rm -v $(pwd):/repo ghcr.io/pathofbuildingcommunity/pathofbuilding-tests:latest \
     sh -c "cd /repo/third-party/PathOfBuilding/src && luajit /repo/scripts/gen_oracle.lua \
       /repo/crates/pob-calc/tests/oracle/melee_str.xml"
   ```

2. Add a CI job to `.github/workflows/ci.yml` using the POB Docker container:
   ```yaml
   oracle:
     runs-on: ubuntu-latest
     container: ghcr.io/pathofbuildingcommunity/pathofbuilding-tests:latest
     steps:
       - uses: actions/checkout@v4
         with:
           submodules: recursive
       - uses: dtolnay/rust-toolchain@stable
       - name: Run oracle parity tests
         run: DATA_DIR=data cargo test -p pob-calc oracle
         # DATA_DIR triggers the parity assertions; expected JSON must be committed
   ```

3. Set `DATA_DIR` to point to the committed `data/` directory. This requires Phase 2 (data extractor) to be complete and `data/*.json` to be committed.

4. Add a regeneration workflow (separate, manual trigger) that runs `run_oracle.sh` for each oracle build and commits updated expected JSON files.

## Related files

- `scripts/gen_oracle.lua` — oracle generation script
- `scripts/run_oracle.sh` — shell wrapper (handles `cd` to POB src dir)
- `crates/pob-calc/tests/oracle.rs` — Rust oracle tests
- `crates/pob-calc/tests/oracle/melee_str.xml` — oracle build XML
- `crates/pob-calc/tests/oracle/melee_str.expected.json` — placeholder (needs real output)
- `docs/superpowers/specs/2026-03-29-pob-wasm-design.md` §6 — testing strategy
