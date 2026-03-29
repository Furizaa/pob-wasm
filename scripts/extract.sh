#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 /path/to/Content.ggpk"
  exit 1
fi

GGPK_PATH="$1"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT_DIR="$REPO_ROOT/data"

echo "Building data-extractor..."
cargo build -p data-extractor --release

echo "Extracting data from $GGPK_PATH..."
"$REPO_ROOT/target/release/data-extractor" "$GGPK_PATH" --output "$OUTPUT_DIR"

echo "Extraction complete. Review changes in $OUTPUT_DIR before committing."
