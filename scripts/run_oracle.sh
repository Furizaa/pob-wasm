#!/bin/bash
# run_oracle.sh: Wrapper to run gen_oracle.lua from the correct directory.
# Usage: ./scripts/run_oracle.sh <path-to-build.xml>
# Output: JSON to stdout

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
POB_SRC="$REPO_ROOT/third-party/PathOfBuilding/src"
XML_PATH="$1"

if [ -z "$XML_PATH" ]; then
    echo "Usage: $0 <build.xml>" >&2
    exit 1
fi

# Make xml_path absolute
if [[ "$XML_PATH" != /* ]]; then
    XML_PATH="$(pwd)/$XML_PATH"
fi

# Run from POB src directory
cd "$POB_SRC"
luajit "$SCRIPT_DIR/gen_oracle.lua" "$XML_PATH"
