#!/usr/bin/env python3
"""Derives the Java -> Bedrock texture table for everything but blocks, items and the GUI.

Reads a Java Edition client jar and the pinned bedrock-samples resource pack, both from the
machine running it (neither is committed), and writes `extra-names.tsv`, a table of Java texture
paths (relative to assets/minecraft/textures, without extension) and the Bedrock paths (relative
to textures/) they become, with the vanilla size of each. A Java texture maps when

  1. a Bedrock sample has exactly its pixels (the same artwork, wherever Bedrock keeps it), or
  2. a Bedrock sample sits at the same path with the same size.

  3. an entity texture of the same size has the same file name as exactly one Bedrock entity
     texture no other Java texture claims, in a folder this script does not know to differ
     (banners and shields share names with patterns of another layout, skins are the player's own,
     and profession overlays are not Bedrock's villager skins).

A pixel match wins; an ambiguous pixel match (one Java texture equal to several Bedrock ones) is
left to the same-path rule, then the file-name rule. Run: derive_extras.py <client.jar> <bedrock-samples resource_pack>
<out.tsv>.
"""

import hashlib
import os
import struct
import sys
import zipfile
import zlib

# Entity paths the file-name rule never matches: their layouts differ from Bedrock's namesakes.
RENAME_EXCLUDED = ("entity/banner/", "entity/shield/", "entity/player/", "/profession/")

FOLDERS = ("entity", "painting", "environment", "misc", "map", "colormap", "particle", "trims")


def paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    return a if pa <= pb and pa <= pc else (b if pb <= pc else c)


def decode(data):
    """Returns (width, height, rgba bytes) for a non-interlaced 8-bit PNG, else None."""
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        return None
    pos, header, palette, trns, idat = 8, None, None, None, b""
    while pos < len(data):
        length, kind = struct.unpack(">I4s", data[pos : pos + 8])
        body = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", body)
        elif kind == b"PLTE":
            palette = body
        elif kind == b"tRNS":
            trns = body
        elif kind == b"IDAT":
            idat += body
    if header is None:
        return None
    width, height, depth, color, _, _, interlace = header
    if depth != 8 or interlace != 0 or color not in (0, 2, 3, 4, 6):
        return None
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[color]
    stride = width * channels
    raw = zlib.decompress(idat)
    rows, previous = [], bytearray(stride)
    for y in range(height):
        base = y * (stride + 1)
        filt, line = raw[base], bytearray(raw[base + 1 : base + 1 + stride])
        for x in range(stride):
            a = line[x - channels] if x >= channels else 0
            b = previous[x]
            c = previous[x - channels] if x >= channels else 0
            if filt == 1:
                line[x] = (line[x] + a) & 255
            elif filt == 2:
                line[x] = (line[x] + b) & 255
            elif filt == 3:
                line[x] = (line[x] + ((a + b) >> 1)) & 255
            elif filt == 4:
                line[x] = (line[x] + paeth(a, b, c)) & 255
        rows.append(line)
        previous = line
    out = bytearray()
    for line in rows:
        for x in range(width):
            if color == 6:
                out += line[x * 4 : x * 4 + 4]
            elif color == 2:
                out += line[x * 3 : x * 3 + 3] + b"\xff"
            elif color == 0:
                out += bytes([line[x]] * 3) + b"\xff"
            elif color == 4:
                out += bytes([line[x * 2]] * 3) + bytes([line[x * 2 + 1]])
            else:
                index = line[x]
                alpha = trns[index] if trns and index < len(trns) else 255
                out += palette[index * 3 : index * 3 + 3] + bytes([alpha])
    return width, height, bytes(out)


def digest(image):
    width, height, rgba = image
    return hashlib.sha1(struct.pack(">II", width, height) + rgba).hexdigest()


def main(jar_path, bedrock_root, out_path):
    prefix = "assets/minecraft/textures/"
    java = {}
    with zipfile.ZipFile(jar_path) as jar:
        for name in jar.namelist():
            if name.startswith(prefix) and name.endswith(".png"):
                relative = name[len(prefix) : -4]
                if relative.split("/")[0] in FOLDERS:
                    image = decode(jar.read(name))
                    if image:
                        java[relative] = image
    bedrock = {}
    textures = os.path.join(bedrock_root, "textures")
    for folder, _, files in os.walk(textures):
        for file in files:
            if file.endswith(".png"):
                path = os.path.join(folder, file)
                relative = os.path.relpath(path, textures).replace(os.sep, "/")[:-4]
                with open(path, "rb") as handle:
                    image = decode(handle.read())
                if image:
                    bedrock[relative] = image
    by_pixels = {}
    for path, image in bedrock.items():
        by_pixels.setdefault(digest(image), []).append(path)
    stems = {}
    for path in bedrock:
        if path.startswith("entity/"):
            stems.setdefault(os.path.basename(path), []).append(path)
    rows, taken = [], {}
    claimed = {}
    for path in java:
        if path.startswith("entity/") and not any(part in path for part in RENAME_EXCLUDED):
            for candidate in stems.get(os.path.basename(path), []):
                if bedrock[candidate][:2] == java[path][:2]:
                    claimed.setdefault(candidate, []).append(path)
    for path in sorted(java):
        image = java[path]
        matches = by_pixels.get(digest(image), [])
        target = None
        if len(matches) == 1:
            target = matches[0]
        elif path in bedrock and bedrock[path][:2] == image[:2]:
            target = path
        else:
            named = [
                candidate
                for candidate in stems.get(os.path.basename(path), [])
                if claimed.get(candidate) == [path]
            ]
            if path.startswith("entity/") and len(named) == 1:
                target = named[0]
        if target:
            rows.append((path, target, image[0], image[1]))
            taken.setdefault(target, []).append(path)
    with open(out_path, "w", newline="\n") as out:
        out.write(
            "# Java Edition texture path -> Bedrock texture path, both without extension, then the\n"
            "# vanilla width and height. Java paths are relative to assets/minecraft/textures; Bedrock\n"
            "# paths to textures/. Derived by tools/java-pack-tables/derive_extras.py by exact pixel\n"
            "# match against the pinned Mojang/bedrock-samples pack, else same path and size.\n"
        )
        for java_path, bedrock_path, width, height in rows:
            out.write(f"{java_path}\t{bedrock_path}\t{width}\t{height}\n")
    shared = sum(1 for sources in taken.values() if len(sources) > 1)
    print(f"{len(rows)} of {len(java)} Java textures mapped; {shared} Bedrock textures have several sources")


if __name__ == "__main__":
    main(*sys.argv[1:4])
