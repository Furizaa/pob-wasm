#!/bin/bash
# generate_all_oracles.sh: Generate .expected.json for ALL oracle builds.
# Usage: ./scripts/generate_all_oracles.sh
# Requires: luajit, PathOfBuilding submodule initialized

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ORACLE_DIR="$REPO_ROOT/crates/pob-calc/tests/oracle"

if ! command -v luajit &> /dev/null; then
    echo "ERROR: luajit not found. Install with: brew install luajit (macOS) or apt install luajit (Linux)" >&2
    exit 1
fi

if [ ! -f "$REPO_ROOT/third-party/PathOfBuilding/src/HeadlessWrapper.lua" ]; then
    echo "ERROR: PathOfBuilding submodule not initialized. Run: git submodule update --init --recursive" >&2
    exit 1
fi

PASS=0
FAIL=0
TOTAL=0

for xml in "$ORACLE_DIR"/*.xml; do
    name="$(basename "$xml" .xml)"
    expected="$ORACLE_DIR/${name}.expected.json"
    TOTAL=$((TOTAL + 1))

    echo -n "Generating: ${name} ... "

    if output=$("$SCRIPT_DIR/run_oracle.sh" "$xml" 2>/tmp/gen_oracle_stderr.txt); then
        if echo "$output" | python3 -m json.tool > /dev/null 2>&1; then
            echo "$output" > "$expected"
            echo "OK"
            PASS=$((PASS + 1))
        else
            echo "FAIL (invalid JSON)"
            FAIL=$((FAIL + 1))
        fi
    else
        echo "FAIL"
        cat /tmp/gen_oracle_stderr.txt >&2
        FAIL=$((FAIL + 1))
    fi
done

echo ""
echo "Results: ${PASS}/${TOTAL} passed, ${FAIL} failed"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
