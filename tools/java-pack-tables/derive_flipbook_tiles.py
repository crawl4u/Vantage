#!/usr/bin/env python3
"""Lists the terrain atlas tile each Bedrock block texture fills, for animation patches.

Bedrock animates a block texture through `textures/flipbook_textures.json`, which names the atlas
tile (a `terrain_texture.json` key), the index of the texture within that tile and, for tiles
with several UV variants, the variant. Reads the pinned bedrock-samples pack and writes
`flipbook-tiles.tsv`: texture path (relative to textures/, no extension), atlas tile, atlas index
and variant (`-` when the tile has none), with `vanilla` when Bedrock animates it itself. Run:
derive_flipbook_tiles.py <bedrock-samples resource_pack> <out.tsv>.
"""

import json
import os
import re
import sys


def jsonc(path):
    with open(path, encoding="utf8") as handle:
        return json.loads(re.sub(r"(?m)^\s*//.*$", "", handle.read()))


def main(root, out_path):
    terrain = jsonc(os.path.join(root, "textures/terrain_texture.json"))["texture_data"]
    flipbooks = jsonc(os.path.join(root, "textures/flipbook_textures.json"))
    vanilla = {entry["flipbook_texture"] for entry in flipbooks}
    rows = {}
    for tile, entry in terrain.items():
        textures = entry.get("textures")
        if textures is None:
            continue
        if not isinstance(textures, list):
            textures = [textures]
        for index, texture in enumerate(textures):
            path = texture.get("path") if isinstance(texture, dict) else texture
            if not isinstance(path, str) or not path.startswith("textures/blocks/"):
                continue
            if not any(os.path.isfile(os.path.join(root, path + ext)) for ext in (".png", ".tga")):
                continue
            # Several tiles can share a picture (dirt is also the grass block's bottom): the tile
            # named after the file is the one the texture belongs to.
            if path not in rows or tile == os.path.basename(path):
                rows[path] = (tile, index, "-")
    for entry in flipbooks:
        path = entry["flipbook_texture"]
        if not any(os.path.isfile(os.path.join(root, path + ext)) for ext in (".png", ".tga")):
            continue
        rows[path] = (
            entry["atlas_tile"],
            entry.get("atlas_index", 0),
            entry.get("atlas_tile_variant", "-"),
        )
    with open(out_path, "w", newline="\n") as out:
        out.write(
            "# Bedrock block texture (relative to textures/, no extension), its terrain atlas tile,\n"
            "# atlas index, tile variant ('-' for none) and 'vanilla' when Bedrock animates it. Derived\n"
            "# by tools/java-pack-tables/derive_flipbook_tiles.py from the pinned bedrock-samples pack.\n"
        )
        for path in sorted(rows):
            tile, index, variant = rows[path]
            kind = "vanilla" if path in vanilla else "static"
            out.write(
                f"{path[len('textures/'):]}\t{tile}\t{index}\t{variant}\t{kind}\n"
            )
    print(f"{len(rows)} block textures, {len(vanilla)} animated by Bedrock")


if __name__ == "__main__":
    main(*sys.argv[1:3])
