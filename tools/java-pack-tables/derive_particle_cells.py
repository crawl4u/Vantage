#!/usr/bin/env python3
"""Derives which Java particle sprite fills each cell of Bedrock's particle atlas.

Bedrock draws most particles from one atlas, `textures/particle/particles.png`, in 8-pixel
cells; Java Edition 1.20+ keeps one file per particle frame under `textures/particle/`. A cell
takes the Java sprite whose pixels equal the cell's in the pinned bedrock-samples pack. Where
several sprites are equal (a frame shared by two particles), the sprite of the family most of
the row's other cells name is chosen, then the one whose frame number is the cell's column. Cells no Java sprite matches are left out, so a pack's
translation never blanks them. Run: derive_particle_cells.py <client.jar> <bedrock-samples
resource_pack> <out.tsv>.
"""

import os
import sys
import zipfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import derive_extras as extras  # noqa: E402

CELL = 8
PREFIX = "assets/minecraft/textures/particle/"


def family(name):
    return name.rsplit("_", 1)[0]


def main(jar_path, bedrock_root, out_path):
    width, height, pixels = extras.decode(
        open(os.path.join(bedrock_root, "textures/particle/particles.png"), "rb").read()
    )
    assert width == height == 128, "the sample atlas is 128 by 128"
    cells = {}
    for row in range(height // CELL):
        for col in range(width // CELL):
            block = b"".join(
                pixels[((row * CELL + y) * width + col * CELL) * 4 :][: CELL * 4]
                for y in range(CELL)
            )
            if any(block[3::4]):
                cells[(col, row)] = block
    sprites = {}
    with zipfile.ZipFile(jar_path) as jar:
        for name in jar.namelist():
            if name.startswith(PREFIX) and name.endswith(".png"):
                image = extras.decode(jar.read(name))
                if image and image[0] == image[1] == CELL:
                    sprites[name[len(PREFIX) : -4]] = image[2]
    candidates = {
        cell: sorted(name for name, block in sprites.items() if block == pixels_)
        for cell, pixels_ in cells.items()
    }
    candidates = {cell: names for cell, names in candidates.items() if names}
    chosen = {}
    for (col, row), names in candidates.items():
        row_names = [n for (c, r), ns in candidates.items() if r == row and c != col for n in ns]
        chosen[(col, row)] = max(
            names, key=lambda n: (
                sum(family(n) == family(o) for o in row_names),
                n.rsplit("_", 1)[-1] == str(col % 8),
                n,
            ),
        )
    with open(out_path, "w", newline="\n") as out:
        out.write(
            "# Java particle sprite (relative to assets/minecraft/textures/particle, no extension)\n"
            "# then the column and row of the 8-pixel cell it fills in Bedrock's particle atlas.\n"
            "# Derived by tools/java-pack-tables/derive_particle_cells.py by exact pixel match.\n"
        )
        for (col, row), name in sorted(chosen.items(), key=lambda x: (x[0][1], x[0][0])):
            out.write(f"{name}\t{col}\t{row}\n")
    print(f"{len(chosen)} of {len(cells)} atlas cells matched")


if __name__ == "__main__":
    main(*sys.argv[1:4])
