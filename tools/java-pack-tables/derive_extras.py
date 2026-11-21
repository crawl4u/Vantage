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

import difflib
import glob
import hashlib
import json
import os
import re
import struct
import sys
import zipfile
import zlib

# A texture matched only by path or file name must still look like its Bedrock namesake: when
# its brightness pattern correlates with Bedrock's below this, the two editions lay the picture out
# differently (fish, dolphins, baby animals), and copying it would draw a garbled mob, so the
# mob keeps Bedrock's own texture instead.
MIN_SIMILARITY = 0.6

# For a texture drawn on a known Bedrock model, the share of that model's box faces that must
# show the same shapes in the Java picture; below it the two editions lay the texture out
# differently.
MIN_AGREEMENT = 0.6

# Textures the translator turns to Bedrock's orientation itself (see `flipped_box_faces` in
# extras.rs), so their raw pixels correlate poorly with Bedrock's and are kept regardless.
REORIENTED = ("entity/chest/",)

# Rule 4 (same size, different name): a Java entity texture takes the Bedrock one that shares at
# least this share of its drawn pixels, or that share with a similar file name, so the editions'
# differently spelled variants (`cat_british_shorthair` and `britishshorthair`) meet.
FUZZY_SAME_ART = 0.9
FUZZY_NAMED = 0.6
FUZZY_NAME_RATIO = 0.7

# Textures extras.rs maps by an explicit alias, which the table must not repeat.
ALIASED = ("entity/bell/bell_body", "entity/shulker/shulker", "entity/shulker/shulker_light_gray")

# Entity folders rule 4 leaves alone: their pictures are composed or laid out per edition.
FUZZY_EXCLUDED = ("entity/banner/", "entity/shield/", "entity/player/", "entity/equipment/",
                  "entity/villager/", "entity/zombie_villager/")

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
    # Palette and grey images may pack several pixels per byte; the rest are 8 bits a channel.
    packed = depth in (1, 2, 4) and color in (0, 3)
    if (depth != 8 and not packed) or interlace != 0 or color not in (0, 2, 3, 4, 6):
        return None
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[color]
    stride = (width * depth + 7) // 8 if packed else width * channels
    step = 1 if packed else channels
    raw = zlib.decompress(idat)
    rows, previous = [], bytearray(stride)
    for y in range(height):
        base = y * (stride + 1)
        filt, line = raw[base], bytearray(raw[base + 1 : base + 1 + stride])
        for x in range(stride):
            a = line[x - step] if x >= step else 0
            b = previous[x]
            c = previous[x - step] if x >= step else 0
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
    if packed:
        # Expand to one byte per pixel so the loop below reads palette indices or grey levels.
        mask = (1 << depth) - 1
        scale = 255 // mask
        expanded = []
        for line in rows:
            pixels = bytearray(width)
            for x in range(width):
                pixels[x] = (line[x * depth // 8] >> (8 - depth - (x * depth) % 8)) & mask
            expanded.append(pixels if color == 3 else bytearray(v * scale for v in pixels))
        rows = expanded
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


def read_geometries(bedrock_root):
    """Bedrock model box layouts, and the model ids each entity texture is drawn on.

    Returns (geometries, textures): `geometries` maps a model id to (width, height, boxes) with
    each box (u, v, width, height, depth) in texels, and `textures` maps a texture path
    (relative to textures/) to the model ids its entities use.
    """

    def parse(path):
        try:
            with open(path, encoding="utf-8") as handle:
                return json.loads(re.sub(r"(?m)^\s*//.*$", "", handle.read()))
        except (OSError, ValueError):
            return {}

    geometries = {}
    for path in glob.glob(os.path.join(bedrock_root, "models", "entity", "*.geo.json")):
        for model in parse(path).get("minecraft:geometry", []):
            description = model["description"]
            boxes = [
                (int(cube["uv"][0]), int(cube["uv"][1]), *[int(round(side)) for side in cube["size"]])
                for bone in model.get("bones", [])
                for cube in bone.get("cubes", [])
                if isinstance(cube.get("uv"), list) and "size" in cube
            ]
            geometries[description["identifier"]] = (
                description.get("texture_width", 64),
                description.get("texture_height", 64),
                boxes,
            )
    textures = {}
    for path in glob.glob(os.path.join(bedrock_root, "entity", "*.entity.json")):
        description = parse(path).get("minecraft:client_entity", {}).get("description", {})
        models = set(description.get("geometry", {}).values())
        for texture in description.get("textures", {}).values():
            textures.setdefault(texture.replace("textures/", "", 1), set()).update(models)
    return geometries, textures


def box_faces(u, v, w, h, d):
    """The six face rectangles (x, y, width, height) of a Bedrock box at (u, v)."""
    return [
        (u, v + d, d, h),
        (u + d, v + d, w, h),
        (u + d + w, v + d, d, h),
        (u + d + w + d, v + d, w, h),
        (u + d, v, w, d),
        (u + d + w, v, w, d),
    ]


def region(image, rect):
    width, _, rgba = image
    x, y, w, h = rect
    return [rgba[((y + j) * width + x + i) * 4 : ((y + j) * width + x + i) * 4 + 4] for j in range(h) for i in range(w)]


def pixels(image):
    """The image's pixels as 4-byte values, row by row."""
    width, height, rgba = image
    return [rgba[i : i + 4] for i in range(0, width * height * 4, 4)]


def alike(a, b):
    """Whether Java's pixels `a` and Bedrock's `b` (same-size lists) show the same shapes.

    Judged over the pixels Java draws, since Bedrock's TGA mob textures are opaque everywhere:
    Bedrock must draw them too, and their brightness must follow the same pattern.
    """
    drawn = [(p, q) for p, q in zip(a, b) if p[3] > 0]
    if not drawn:
        return True
    if sum(1 for _, q in drawn if q[3] > 0) < 0.9 * len(drawn):
        return False
    xs = [(p[0] + p[1] + p[2]) / 3 for p, _ in drawn]
    ys = [(q[0] + q[1] + q[2]) / 3 for _, q in drawn]
    mx, my = sum(xs) / len(xs), sum(ys) / len(ys)
    vx = sum((x - mx) ** 2 for x in xs)
    vy = sum((y - my) ** 2 for y in ys)
    if vx < 1e-6 or vy < 1e-6:
        return vx / len(xs) < 40 and vy / len(xs) < 40
    return sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / (vx * vy) ** 0.5 >= 0.8


def layout_agreement(java_image, bedrock_image, boxes):
    """The share of the model's box faces whose picture shows the same shapes in both images."""
    width, height, _ = java_image
    faces = [
        rect
        for box in boxes
        for rect in box_faces(*box)
        if rect[2] > 0 and rect[3] > 0 and rect[0] + rect[2] <= width and rect[1] + rect[3] <= height
    ]
    if not faces:
        return None
    agree = sum(1 for rect in faces if alike(region(java_image, rect), region(bedrock_image, rect)))
    return agree / len(faces)


def similarity(java_image, bedrock_image):
    """How alike two same-size pictures are in layout, from 0 to 1, whatever their colours.

    The correlation of their pixel brightness over the pixels either one draws: a recoloured
    copy (a dyed shulker box) keeps its shapes and scores high, while a picture laid out
    differently scores near zero.
    """
    (width, height, a), (_, _, b) = java_image, bedrock_image
    xs, ys = [], []
    for i in range(0, width * height * 4, 4):
        if a[i + 3] or b[i + 3]:
            xs.append((a[i] + a[i + 1] + a[i + 2]) / 3 if a[i + 3] else -1.0)
            ys.append((b[i] + b[i + 1] + b[i + 2]) / 3 if b[i + 3] else -1.0)
    n = len(xs)
    if n == 0:
        return 1.0
    mean_x, mean_y = sum(xs) / n, sum(ys) / n
    cov = sum((x - mean_x) * (y - mean_y) for x, y in zip(xs, ys))
    var_x = sum((x - mean_x) ** 2 for x in xs)
    var_y = sum((y - mean_y) ** 2 for y in ys)
    if var_x == 0 or var_y == 0:
        return 1.0 if var_x == var_y else 0.0
    return cov / (var_x * var_y) ** 0.5


# The editions re-encode some art a few steps apart (Bedrock's spider is two levels darker), so
# pixels count as equal when every channel is within this many levels.
CHANNEL_TOLERANCE = 6


def opaque_equality(java_image, bedrock_image):
    """The share of pixels either picture draws that are equal (within tolerance) in both."""
    (width, height, a), (_, _, b) = java_image, bedrock_image
    drawn = equal = 0
    for i in range(0, width * height * 4, 4):
        if a[i + 3] or b[i + 3]:
            drawn += 1
            if all(abs(a[i + c] - b[i + c]) <= CHANNEL_TOLERANCE for c in range(4)):
                equal += 1
    return equal / drawn if drawn else 1.0


def decode_tga(data):
    """Returns (width, height, rgba bytes) for an uncompressed or run-length true-colour TGA."""
    if len(data) < 18:
        return None
    id_length, colour_map, kind = data[0], data[1], data[2]
    width, height = struct.unpack("<HH", data[12:16])
    depth, descriptor = data[16], data[17]
    if colour_map or kind not in (2, 10) or depth not in (24, 32) or not width or not height:
        return None
    bytes_per = depth // 8
    pos = 18 + id_length
    pixels = bytearray()
    total = width * height
    if kind == 2:
        pixels = data[pos : pos + total * bytes_per]
    else:
        count = 0
        while count < total and pos < len(data):
            header = data[pos]
            pos += 1
            run = (header & 127) + 1
            if header & 128:
                pixels += data[pos : pos + bytes_per] * run
                pos += bytes_per
            else:
                pixels += data[pos : pos + run * bytes_per]
                pos += run * bytes_per
            count += run
    if len(pixels) < total * bytes_per:
        return None
    out = bytearray(total * 4)
    for i in range(total):
        b, g, r = pixels[i * bytes_per : i * bytes_per + 3]
        a = pixels[i * bytes_per + 3] if bytes_per == 4 else 255
        out[i * 4 : i * 4 + 4] = bytes((r, g, b, a))
    if not descriptor & 0x20:  # bottom-up rows
        rows = [out[y * width * 4 : (y + 1) * width * 4] for y in range(height)]
        out = bytearray(b"".join(reversed(rows)))
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
            if file.endswith((".png", ".tga")) and not file.endswith("_mers.tga"):
                path = os.path.join(folder, file)
                relative = os.path.relpath(path, textures).replace(os.sep, "/")[:-4]
                if relative in bedrock:
                    continue
                with open(path, "rb") as handle:
                    raw = handle.read()
                image = decode_tga(raw) if file.endswith(".tga") else decode(raw)
                if image:
                    bedrock[relative] = image
    by_pixels = {}
    for path, image in bedrock.items():
        by_pixels.setdefault(digest(image), []).append(path)
    stems = {}
    for path in bedrock:
        if path.startswith("entity/"):
            stems.setdefault(os.path.basename(path), []).append(path)
    geometries, model_ids = read_geometries(bedrock_root)
    rows, taken, incompatible = [], {}, []
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
        if (
            target
            and path.startswith("entity/")
            and not path.startswith(REORIENTED)
            and digest(image) != digest(bedrock[target])
        ):
            agreement = None
            for model in model_ids.get(target, ()):
                tw, th, boxes = geometries.get(model, (0, 0, []))
                if (tw, th) == image[:2]:
                    score = layout_agreement(image, bedrock[target], boxes)
                    if score is not None and (agreement is None or score > agreement):
                        agreement = score
            if agreement is None:
                agreement = 1.0 if alike(pixels(image), pixels(bedrock[target])) else 0.0
            if agreement < MIN_AGREEMENT:
                incompatible.append(path)
                target = None
        if target:
            rows.append((path, target, image[0], image[1]))
            taken.setdefault(target, []).append(path)
    # Rule 4, after every earlier rule so each Bedrock texture keeps its first source.
    mapped = {path for path, _, _, _ in rows} | set(incompatible)
    used = set(taken)
    by_size = {}
    for path, image in bedrock.items():
        if path.startswith("entity/") and path not in used:
            by_size.setdefault(image[:2], []).append(path)
    for path in sorted(java):
        if path in mapped or path in ALIASED or not path.startswith("entity/") or path.startswith(FUZZY_EXCLUDED):
            continue
        image = java[path]
        stem = os.path.basename(path)
        scored = []
        for candidate in by_size.get(image[:2], []):
            if candidate in used:
                continue
            ratio = difflib.SequenceMatcher(None, stem, os.path.basename(candidate)).ratio()
            scored.append((round(opaque_equality(image, bedrock[candidate]), 2), ratio, candidate))
        if not scored:
            continue
        scored.sort(reverse=True)
        _, ratio, target = scored[0]
        score = opaque_equality(image, bedrock[target])
        if not (score >= FUZZY_SAME_ART or (score >= FUZZY_NAMED and ratio >= FUZZY_NAME_RATIO)):
            continue
        agreement = None
        for model in model_ids.get(target, ()):
            tw, th, boxes = geometries.get(model, (0, 0, []))
            if (tw, th) == image[:2]:
                found = layout_agreement(image, bedrock[target], boxes)
                if found is not None and (agreement is None or found > agreement):
                    agreement = found
        if agreement is not None and agreement < MIN_AGREEMENT:
            incompatible.append(path)
            continue
        rows.append((path, target, image[0], image[1]))
        taken.setdefault(target, []).append(path)
        used.add(target)
    with open(out_path, "w", newline="\n") as out:
        out.write(
            "# Java Edition texture path -> Bedrock texture path, both without extension, then the\n"
            "# vanilla width and height. Java paths are relative to assets/minecraft/textures; Bedrock\n"
            "# paths to textures/. Derived by tools/java-pack-tables/derive_extras.py by exact pixel\n"
            "# match against the pinned Mojang/bedrock-samples pack, else same path and size.\n"
        )
        for java_path, bedrock_path, width, height in rows:
            out.write(f"{java_path}\t{bedrock_path}\t{width}\t{height}\n")
    # The translator also matches by file name at run time, so the refused paths are listed for it.
    listed = os.path.join(os.path.dirname(out_path), "extra-incompatible.txt")
    with open(listed, "w", newline="\n") as out:
        out.write(
            "# Java textures laid out differently from their Bedrock namesakes, which the translator\n"
            "# must not copy over them. Derived by tools/java-pack-tables/derive_extras.py.\n"
        )
        for path in incompatible:
            out.write(path + "\n")
    shared = sum(1 for sources in taken.values() if len(sources) > 1)
    print(f"{len(rows)} of {len(java)} Java textures mapped; {shared} Bedrock textures have several sources")


if __name__ == "__main__":
    main(*sys.argv[1:4])
