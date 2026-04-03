#!/usr/bin/env python3
"""
Extract tattoo passive data from PoB's Data/TattooPassives.lua and write tattoos.json.

Usage:
  python3 scripts/extract_tattoo_passives.py [--input PATH] [--output PATH]

Outputs a JSON file of the form:
{
  "nodes": {
    "Acrobatics": {
      "dn": "Acrobatics",
      "is_tattoo": true,
      "override_type": "KeystoneTattoo",
      "is_keystone": true,
      "is_notable": false,
      "is_mastery": false,
      "stats": ["stat1", "stat2"]
    },
    ...
  }
}

The `stats` field holds the `sd` array (stat description lines), which are used
to replace the original passive node's stats when a tattoo override is applied.
"""

import re
import json
import argparse
import os
import sys


def find_nodes_section(content: str):
    """
    Find the start and end positions of the top-level ["nodes"] = { ... } block.
    Returns (block_content_start, block_content_end) — indices into content.
    """
    # Find '["nodes"] ='
    nodes_key = content.find('["nodes"] = {')
    if nodes_key < 0:
        return None, None
    brace_open = content.index('{', nodes_key)
    depth = 1
    i = brace_open + 1
    while i < len(content) and depth > 0:
        c = content[i]
        if c == '{':
            depth += 1
        elif c == '}':
            depth -= 1
        i += 1
    # brace_open+1 .. i-1 is the content inside the nodes block
    return brace_open + 1, i - 1


def scan_top_level_entries(text: str):
    """
    Scan `text` for top-level key-value entries of the form:
        ["KeyName"] = { ... },
    Yields (name, block_text) pairs where block_text is the content
    between the braces (exclusive).

    This scans only TOP-LEVEL entries — depth-aware so nested { } are skipped.
    """
    i = 0
    n = len(text)
    while i < n:
        # Find next ["..."] = { pattern at depth 0
        m = re.search(r'\["([^"]+)"\]\s*=\s*\{', text[i:])
        if not m:
            break
        name = m.group(1)
        block_open = i + m.end()  # position just after the opening {

        # Scan for matching closing }
        depth = 1
        j = block_open
        while j < n and depth > 0:
            c = text[j]
            if c == '{':
                depth += 1
            elif c == '}':
                depth -= 1
            j += 1
        # text[block_open:j-1] is the block content
        block = text[block_open:j - 1]
        yield name, block
        i = j  # continue after this block


def extract_string_field(block: str, key: str) -> str:
    """Extract a quoted string value for a Lua key like ["key"] = "value"."""
    # Only match at depth 0 in block — look for the key followed by = "..."
    pattern = re.compile(r'\["' + re.escape(key) + r'"\]\s*=\s*"([^"]*)"')
    # Find the first occurrence; to avoid matching inside nested blocks we
    # look for the key only in the first depth-0 portion.
    # Since keys are flat strings (no nested quotes), this works fine.
    m = pattern.search(block)
    return m.group(1) if m else ""


def extract_bool_field(block: str, key: str) -> bool:
    """Extract a boolean for a Lua key like ["key"] = true/false."""
    pattern = re.compile(r'\["' + re.escape(key) + r'"\]\s*=\s*(true|false)')
    m = pattern.search(block)
    return m.group(1) == "true" if m else False


def extract_sd_field(block: str) -> list:
    """
    Extract the sd stat descriptions array from a node block.
    The block contains: ["sd"] = { [1] = "...", [2] = "...", ... }
    """
    # Find ["sd"] = {
    sd_key_pos = block.find('["sd"]')
    if sd_key_pos < 0:
        return []
    brace_pos = block.index('{', sd_key_pos)

    # Find matching close brace
    depth = 1
    j = brace_pos + 1
    while j < len(block) and depth > 0:
        c = block[j]
        if c == '{':
            depth += 1
        elif c == '}':
            depth -= 1
        j += 1
    sd_block = block[brace_pos + 1:j - 1]

    # Extract string values: [N] = "..."
    strings = re.findall(r'\[\d+\]\s*=\s*"([^"]*)"', sd_block)
    return strings


def parse_node(name: str, block: str) -> dict:
    """Parse one tattoo node block."""
    dn = extract_string_field(block, "dn") or name
    is_tattoo = extract_bool_field(block, "isTattoo")
    override_type = extract_string_field(block, "overrideType")
    is_keystone = extract_bool_field(block, "ks")
    is_notable = extract_bool_field(block, "not")
    is_mastery = extract_bool_field(block, "m")
    stats = extract_sd_field(block)
    active_effect_image = extract_string_field(block, "activeEffectImage")
    icon = extract_string_field(block, "icon")

    return {
        "dn": dn,
        "is_tattoo": is_tattoo,
        "override_type": override_type,
        "is_keystone": is_keystone,
        "is_notable": is_notable,
        "is_mastery": is_mastery,
        "stats": stats,
        "active_effect_image": active_effect_image,
        "icon": icon,
    }


def extract_tattoo_nodes(content: str) -> dict:
    """Extract all tattoo nodes from TattooPassives.lua content."""
    start, end = find_nodes_section(content)
    if start is None:
        print("Warning: could not find [\"nodes\"] section", file=sys.stderr)
        return {}

    nodes_text = content[start:end]
    nodes = {}
    for name, block in scan_top_level_entries(nodes_text):
        node = parse_node(name, block)
        nodes[name] = node

    return nodes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        '--input',
        default='third-party/PathOfBuilding/src/Data/TattooPassives.lua',
        help='Path to TattooPassives.lua'
    )
    parser.add_argument(
        '--output',
        default='data/tattoos.json',
        help='Output JSON file path'
    )
    args = parser.parse_args()

    print(f"Reading tattoo data from: {args.input}")

    with open(args.input, 'r', encoding='utf-8') as f:
        content = f.read()

    nodes = extract_tattoo_nodes(content)
    print(f"Found {len(nodes)} tattoo nodes")

    # Sanity check: every node should have a dn and be_tattoo=true
    bad = [(k, v) for k, v in nodes.items() if not v.get('dn') or not v.get('is_tattoo')]
    if bad:
        print(f"Warning: {len(bad)} nodes with missing dn or is_tattoo=false")

    output = {"nodes": nodes}

    os.makedirs(os.path.dirname(args.output) or '.', exist_ok=True)
    with open(args.output, 'w', encoding='utf-8') as f:
        json.dump(output, f, indent=2, ensure_ascii=False)

    print(f"Written to: {args.output}")


if __name__ == '__main__':
    main()
