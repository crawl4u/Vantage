#!/usr/bin/env python3
"""Lists the pinned bedrock-samples entity, environment, misc, map and painting textures with their
sizes.

Writes `bedrock-textures.tsv`: each Bedrock texture (relative to textures/, no extension) and its
width and height. A Java pack whose texture has an older name than the Java Edition jar's is
matched against it by path or file name and size. Run:
derive_bedrock_textures.py <bedrock-samples resource_pack> <out.tsv>.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import derive_extras as extras  # noqa: E402


def main(bedrock_root, out_path):
    textures = os.path.join(bedrock_root, "textures")
    rows = []
    for top in ("entity", "environment", "misc", "map", "painting"):
        for folder, _, files in os.walk(os.path.join(textures, top)):
            for file in files:
                if file.endswith(".png"):
                    path = os.path.join(folder, file)
                    relative = os.path.relpath(path, textures).replace(os.sep, "/")[:-4]
                    with open(path, "rb") as handle:
                        image = extras.decode(handle.read())
                    if image:
                        rows.append((relative, image[0], image[1]))
    with open(out_path, "w", newline="\n") as out:
        out.write(
            "# Bedrock entity texture (relative to textures/, no extension), then its width and\n"
            "# height, from the pinned Mojang/bedrock-samples pack. Derived by\n"
            "# tools/java-pack-tables/derive_bedrock_entities.py.\n"
        )
        for relative, width, height in sorted(rows):
            out.write(f"{relative}\t{width}\t{height}\n")
    print(f"{len(rows)} textures")


if __name__ == "__main__":
    main(*sys.argv[1:3])
