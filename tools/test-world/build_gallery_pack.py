#!/usr/bin/env python3
"""Builds a Bedrock behavior pack that lays out a visual testing world.

The pack holds three functions and one structure:

* `gallery_setup` loads the chunks, flattens the area, moves the player to the start and
  schedules the other two once the area is loaded.
* `gallery_blocks` places one of every registry block on a grid, a gap apart so connecting
  shapes (fences, panes, walls) stay separate.
* `gallery_items` loads the structure holding one item frame per registry item on a wall.

Usage:
    build_gallery_pack.py BLOCKS.txt ITEMS.txt OUT_PACK_DIR

BLOCKS.txt and ITEMS.txt list one identifier per line, as `cargo run -p assets --example
dump_ids` writes them. Run `/function gallery_setup` in the world once the pack is applied.
"""

import json
import struct
import sys
from pathlib import Path

PACK_UUID = "6f0c3b7e-5a41-4d0e-9b7a-2c1d8e3a4f50"
MODULE_UUID = "a1b2c3d4-0e0f-4a1b-8c2d-3e4f5a6b7c8d"

# Superflat: bedrock at -64, grass top at -61, so blocks stand on y = -60.
FLOOR_Y = -60
BLOCK_COLUMNS = 40
BLOCK_STEP_X = 2
BLOCK_STEP_Z = 3
BLOCKS_Z0 = 10
WALL_COLUMNS = 60
WALL_ROWS = 28
WALL_Z = 140

# Registry entries that are not placeable world blocks.
SKIP_BLOCKS = {
    "cinnabar:reserved",
    "minecraft:air",
    "minecraft:allow",
    "minecraft:deny",
    "minecraft:border_block",
    "minecraft:structure_void",
    "minecraft:moving_block",
    "minecraft:info_update",
    "minecraft:info_update2",
    "minecraft:unknown",
}


def read_ids(path):
    return [line.strip() for line in Path(path).read_text().splitlines() if line.strip()]


def block_commands(blocks):
    placeable = [b for b in blocks if b not in SKIP_BLOCKS and not b.startswith("minecraft:reserved")]
    lines = []
    for index, block in enumerate(placeable):
        x = (index % BLOCK_COLUMNS) * BLOCK_STEP_X
        z = BLOCKS_Z0 + (index // BLOCK_COLUMNS) * BLOCK_STEP_Z
        lines.append(f"setblock {x} {FLOOR_Y} {z} {block}")
    rows = (len(placeable) + BLOCK_COLUMNS - 1) // BLOCK_COLUMNS
    return lines, rows


# --- Little-endian NBT, as .mcstructure files use -------------------------------------------

END, BYTE, SHORT, INT, LONG, FLOAT, DOUBLE, BYTE_ARRAY, STRING, LIST, COMPOUND, INT_ARRAY = range(12)


def _string(text):
    data = text.encode("utf-8")
    return struct.pack("<H", len(data)) + data


class Tag:
    def __init__(self, kind, value):
        self.kind, self.value = kind, value


def byte(v): return Tag(BYTE, v)
def short(v): return Tag(SHORT, v)
def integer(v): return Tag(INT, v)
def real(v): return Tag(FLOAT, v)
def text(v): return Tag(STRING, v)
def compound(**items): return Tag(COMPOUND, items)
def compound_of(items): return Tag(COMPOUND, items)
def listing(kind, values): return Tag(LIST, (kind, values))


def _payload(tag):
    k, v = tag.kind, tag.value
    if k == BYTE: return struct.pack("<b", v)
    if k == SHORT: return struct.pack("<h", v)
    if k == INT: return struct.pack("<i", v)
    if k == FLOAT: return struct.pack("<f", v)
    if k == STRING: return _string(v)
    if k == LIST:
        kind, values = v
        return struct.pack("<Bi", kind, len(values)) + b"".join(_payload(x) for x in values)
    if k == COMPOUND:
        out = b""
        for name, child in v.items():
            out += struct.pack("<B", child.kind) + _string(name) + _payload(child)
        return out + b"\x00"
    raise ValueError(k)


def write_nbt(root_name, root):
    return struct.pack("<B", COMPOUND) + _string(root_name) + _payload(root)


def item_frame_entity(x, y, z, item):
    return compound(
        id=text("ItemFrame"),
        isMovable=byte(1),
        x=integer(x), y=integer(y), z=integer(z),
        ItemDropChance=real(1.0),
        ItemRotation=real(0.0),
        Item=compound(Count=byte(1), Damage=short(0), Name=text(item), WasPickedUp=byte(0)),
    )


def items_structure(items):
    """One wall block with an item frame in front of it per item, in rows from the top."""
    columns, rows = WALL_COLUMNS, WALL_ROWS
    size = (columns, rows, 2)  # x, y, z: the wall at z = 1, the frames at z = 0
    palette = [
        compound(name=text("minecraft:gray_concrete"), states=compound(), version=integer(18153475)),
        compound(
            name=text("minecraft:item_frame"),
            states=compound(
                facing_direction=integer(2),
                item_frame_map_bit=byte(0),
                item_frame_photo_bit=byte(0),
            ),
            version=integer(18153475),
        ),
    ]
    cells = size[0] * size[1] * size[2]
    indices = [-1] * cells
    block_data = {}

    def at(x, y, z):
        return (x * size[1] + y) * size[2] + z

    for x in range(columns):
        for y in range(rows):
            indices[at(x, y, 1)] = 0
    for n, item in enumerate(items[: columns * rows]):
        x, row = n % columns, n // columns
        y = rows - 1 - row
        cell = at(x, y, 0)
        indices[cell] = 1
        block_data[str(cell)] = compound(
            block_entity_data=item_frame_entity(x, y, 0, item),
        )
    structure = compound(
        block_indices=listing(LIST, [
            listing(INT, [integer(i) for i in indices]),
            listing(INT, [integer(-1) for _ in indices]),
        ]),
        entities=listing(COMPOUND, []),
        palette=compound(
            default=compound(
                block_palette=listing(COMPOUND, palette),
                block_position_data=compound_of(block_data),
            )
        ),
    )
    root = compound(
        format_version=integer(1),
        size=listing(INT, [integer(v) for v in size]),
        structure=structure,
        structure_world_origin=listing(INT, [integer(0), integer(0), integer(0)]),
    )
    return write_nbt("", root), len(items), columns * rows


def main():
    blocks_path, items_path, out = sys.argv[1:4]
    out = Path(out)
    blocks, items = read_ids(blocks_path), read_ids(items_path)
    block_lines, block_rows = block_commands(blocks)

    (out / "functions").mkdir(parents=True, exist_ok=True)
    (out / "structures" / "gallery").mkdir(parents=True, exist_ok=True)

    manifest = {
        "format_version": 2,
        "header": {
            "name": "Cinnabar test gallery",
            "description": "Every block on a grid and every item in a frame, for visual testing.",
            "uuid": PACK_UUID,
            "version": [1, 0, 0],
            "min_engine_version": [1, 21, 0],
        },
        "modules": [{"type": "data", "uuid": MODULE_UUID, "version": [1, 0, 0]}],
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2))

    far_x = BLOCK_COLUMNS * BLOCK_STEP_X
    last_z = BLOCKS_Z0 + block_rows * BLOCK_STEP_Z
    setup = [
        f"tickingarea add 0 {FLOOR_Y} 0 {far_x} {FLOOR_Y} {last_z} gallery_blocks_area",
        f"tickingarea add 0 {FLOOR_Y} {WALL_Z - 2} {WALL_COLUMNS} {FLOOR_Y} {WALL_Z + 2} gallery_items_area",
        "schedule on_area_loaded add tickingarea gallery_blocks_area gallery_blocks",
        "schedule on_area_loaded add tickingarea gallery_items_area gallery_items",
        f"tp @s 0 {FLOOR_Y} 0 facing 0 {FLOOR_Y} {BLOCKS_Z0}",
    ]
    (out / "functions" / "gallery_setup.mcfunction").write_text("\n".join(setup) + "\n")
    (out / "functions" / "gallery_blocks.mcfunction").write_text("\n".join(block_lines) + "\n")
    (out / "functions" / "gallery_items.mcfunction").write_text(
        f"structure load gallery:items 0 {FLOOR_Y} {WALL_Z}\n"
    )

    data, placed, capacity = items_structure(items)
    (out / "structures" / "gallery" / "items.mcstructure").write_bytes(data)
    print(f"{len(block_lines)} block commands in {block_rows} rows; {placed}/{len(items)} items "
          f"in frames (capacity {capacity})")


if __name__ == "__main__":
    main()
