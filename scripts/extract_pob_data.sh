#!/bin/bash
set -euo pipefail

OUTPUT_DIR="${1:-data}"
POB_SRC="third-party/PathOfBuilding/src"

if [ ! -d "$POB_SRC/Data" ]; then
    echo "Error: $POB_SRC/Data not found."
    echo "Make sure the PathOfBuilding submodule is initialized:"
    echo "  git submodule update --init"
    exit 1
fi

cargo build -p pob-data-extractor --release
./target/release/pob-data-extractor "$POB_SRC" --output "$OUTPUT_DIR"

echo ""
echo "Data files written to $OUTPUT_DIR/"
ls -lh "$OUTPUT_DIR"/gems.json "$OUTPUT_DIR"/bases.json "$OUTPUT_DIR"/uniques.json "$OUTPUT_DIR"/mods.json 2>/dev/null || true
