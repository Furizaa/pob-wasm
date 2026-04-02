#!/usr/bin/env python3
"""
Extract mastery effect data from PoB's tree.lua files and write mastery_effects.json.

Usage:
  python3 scripts/extract_mastery_effects.py [--tree-dir PATH] [--output PATH]

The tree.lua file format is PoB's Lua table format used in TreeData/3_*/tree.lua.
By default, uses the latest available tree (highest version number in alphabetical order
that doesn't have a suffix like "_ruthless", "_alternate").

Outputs a JSON file of the form:
{
  "effects": {
    "<effect_id>": ["stat string", ...],
    ...
  }
}
"""

import re
import json
import argparse
import os
import sys


def extract_effects_from_lua(content: str) -> dict:
    """
    Extract effect_id -> stats mapping from PoB tree.lua content.
    Mirrors PassiveTree.lua:497-506:
        if node.masteryEffects then
            for _, effect in pairs(node.masteryEffects) do
                if not self.masteryEffects[effect.effect] then
                    self.masteryEffects[effect.effect] = { id = effect.effect, sd = effect.stats }
                end
            end
        end
    """
    # Pattern: ["effect"]= N, \n ["stats"]= { "stat1", "stat2" }
    effect_pattern = re.compile(
        r'\["effect"\]=\s*(\d+),\s*\n\s*\["stats"\]=\s*\{(.*?)\}',
        re.DOTALL
    )

    mastery_effects = {}
    for m in effect_pattern.finditer(content):
        effect_id = int(m.group(1))
        stats_raw = m.group(2)
        # Extract quoted strings from the stats array
        stats = re.findall(r'"([^"]+)"', stats_raw)
        if effect_id not in mastery_effects:
            mastery_effects[effect_id] = stats

    return mastery_effects


def version_key(name: str):
    """Parse a version string like '3_27' into a tuple (3, 27) for numeric sorting."""
    parts = name.split('_')
    try:
        return tuple(int(p) for p in parts)
    except ValueError:
        return (0, 0)


def find_latest_tree_lua(pob_tree_data_dir: str) -> str:
    """Find the latest standard (non-ruthless, non-alternate) tree.lua file."""
    # Get all subdirectory names
    try:
        entries = os.listdir(pob_tree_data_dir)
    except FileNotFoundError:
        sys.exit(f"Error: TreeData directory not found: {pob_tree_data_dir}")

    # Filter to standard version directories only (e.g. "3_27", not "3_27_ruthless")
    # Standard: exactly two numeric components separated by underscore.
    version_dirs = [
        e for e in entries
        if re.match(r'^\d+_\d+$', e)
        and os.path.isdir(os.path.join(pob_tree_data_dir, e))
        and os.path.exists(os.path.join(pob_tree_data_dir, e, 'tree.lua'))
    ]

    if not version_dirs:
        sys.exit(f"Error: No standard tree.lua files found in {pob_tree_data_dir}")

    # Sort numerically and pick the latest
    version_dirs.sort(key=version_key)
    latest = version_dirs[-1]
    return os.path.join(pob_tree_data_dir, latest, 'tree.lua')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        '--tree-dir',
        default='third-party/PathOfBuilding/src/TreeData',
        help='Path to PoB TreeData directory'
    )
    parser.add_argument(
        '--output',
        default='data/mastery_effects.json',
        help='Output JSON file path'
    )
    parser.add_argument(
        '--tree-lua',
        help='Explicit path to tree.lua (overrides --tree-dir)'
    )
    args = parser.parse_args()

    if args.tree_lua:
        tree_lua_path = args.tree_lua
    else:
        tree_lua_path = find_latest_tree_lua(args.tree_dir)

    print(f"Reading mastery effects from: {tree_lua_path}")

    with open(tree_lua_path, 'r', encoding='utf-8') as f:
        content = f.read()

    effects = extract_effects_from_lua(content)
    print(f"Found {len(effects)} unique mastery effects")

    output = {"effects": {str(k): v for k, v in sorted(effects.items())}}

    os.makedirs(os.path.dirname(args.output) or '.', exist_ok=True)
    with open(args.output, 'w', encoding='utf-8') as f:
        json.dump(output, f, indent=2, ensure_ascii=False)

    print(f"Written to: {args.output}")


if __name__ == '__main__':
    main()
