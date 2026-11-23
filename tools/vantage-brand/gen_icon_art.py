#!/usr/bin/env python3
"""Converts a directory of icon SVGs into `crates/vantage-gui/src/icon_art.rs`.

Each SVG is rasterised at 512 by 512 over its own view box (so every icon lands on the 24 by 24 icon grid), its
alpha mask is traced with marching squares at a fixed precision and the outlines are simplified. The result is
solid shapes, each an outline followed by the holes cut out of it. Nothing is drawn by hand.

Usage: gen_icon_art.py <svg directory> <output .rs> [names to skip, comma separated]
Needs ImageMagick (`magick`) with an SVG renderer on the path.
"""
import math
import os
import re
import subprocess
import sys

SIZE = 512
EPS = 0.45
ISO = 0.5


def render(svg):
    """The alpha plane of `svg` rendered over its view box, as rows of bytes."""
    raw = subprocess.run(
        ["magick", "-background", "none", "-density", "2400", svg, "-resize", f"{SIZE}x{SIZE}!", "-alpha", "extract",
         "-depth", "8", "gray:-"],
        capture_output=True, check=True).stdout
    assert len(raw) == SIZE * SIZE, (svg, len(raw))
    return [raw[y * SIZE:(y + 1) * SIZE] for y in range(SIZE)]


def trace(px):
    def val(x, y):
        if x < 0 or y < 0 or x >= SIZE or y >= SIZE:
            return 0.0
        return px[y][x] / 255.0

    def lerp(a, b):
        return (ISO - a) / (b - a) if b != a else 0.5

    segs = []
    for y in range(-1, SIZE):
        for x in range(-1, SIZE):
            tl, tr, br, bl = val(x, y), val(x + 1, y), val(x + 1, y + 1), val(x, y + 1)
            case = (tl > ISO) * 8 + (tr > ISO) * 4 + (br > ISO) * 2 + (bl > ISO)
            if case in (0, 15):
                continue
            top = (x + lerp(tl, tr), y)
            right = (x + 1, y + lerp(tr, br))
            bottom = (x + lerp(bl, br), y + 1)
            left = (x, y + lerp(tl, bl))
            segs.extend({
                1: [(left, bottom)], 2: [(bottom, right)], 3: [(left, right)], 4: [(top, right)],
                5: [(left, top), (bottom, right)], 6: [(top, bottom)], 7: [(left, top)], 8: [(left, top)],
                9: [(top, bottom)], 10: [(top, right), (left, bottom)], 11: [(top, right)], 12: [(left, right)],
                13: [(bottom, right)], 14: [(left, bottom)],
            }[case])

    def key(p):
        return (round(p[0] * 1000), round(p[1] * 1000))

    adj = {}
    for a, b in segs:
        adj.setdefault(key(a), []).append((a, b))
        adj.setdefault(key(b), []).append((b, a))
    used, loops = set(), []
    for a, b in segs:
        sk = (key(a), key(b))
        if sk in used or (sk[1], sk[0]) in used:
            continue
        loop, nxt = [a], b
        used.add(sk)
        while True:
            loop.append(nxt)
            k = key(nxt)
            if k == key(loop[0]):
                break
            step = None
            for p, q in adj.get(k, []):
                e = (key(p), key(q))
                if e in used or (e[1], e[0]) in used:
                    continue
                step = (p, q, e)
                break
            if step is None:
                break
            used.add(step[2])
            nxt = step[1]
        if len(loop) > 8:
            loops.append(loop[:-1] if key(loop[-1]) == key(loop[0]) else loop)
    return loops


def simplify(points, eps):
    """Douglas-Peucker on a closed loop."""
    n = len(points)
    closed = points + [points[0]]

    def rec(lo, hi):
        if hi <= lo + 1:
            return [closed[lo]]
        (x1, y1), (x2, y2) = closed[lo], closed[hi]
        dx, dy = x2 - x1, y2 - y1
        norm = math.hypot(dx, dy) or 1e-9
        worst, at = 0.0, lo
        for i in range(lo + 1, hi):
            d = abs(dy * closed[i][0] - dx * closed[i][1] + x2 * y1 - y2 * x1) / norm
            if d > worst:
                worst, at = d, i
        return rec(lo, at) + rec(at, hi) if worst > eps else [closed[lo]]

    far = max(range(n), key=lambda i: (points[i][0] - points[0][0]) ** 2 + (points[i][1] - points[0][1]) ** 2)
    return rec(0, far) + rec(far, n)


def inside(p, loop):
    c, j = False, len(loop) - 1
    for i in range(len(loop)):
        if (loop[i][1] > p[1]) != (loop[j][1] > p[1]) and p[0] < (loop[j][0] - loop[i][0]) * (p[1] - loop[i][1]) / (loop[j][1] - loop[i][1]) + loop[i][0]:
            c = not c
        j = i
    return c


def groups_of(loops):
    loops = [simplify(l, EPS) for l in loops]
    loops = [l for l in loops if len(l) >= 3]
    depth = [sum(1 for j, m in enumerate(loops) if j != i and inside(l[0], m)) for i, l in enumerate(loops)]
    groups = []
    for i, l in enumerate(loops):
        if depth[i] % 2 == 0:
            holes = [loops[j] for j in range(len(loops)) if depth[j] == depth[i] + 1 and inside(loops[j][0], l)]
            groups.append([l] + holes)
    return groups


def camel(name):
    return "".join(part.capitalize() for part in re.split(r"[-_]", name))


def main():
    src, out = sys.argv[1], sys.argv[2]
    skip = set(sys.argv[3].split(",")) if len(sys.argv) > 3 else set()
    names = sorted(f[:-4] for f in os.listdir(src) if f.endswith(".svg") and not f.startswith("minecraft__"))
    names = [n for n in names if n not in skip]
    scale = 24.0 / SIZE
    body, variants, arms = [], [], []
    for name in names:
        groups = groups_of(trace(render(os.path.join(src, name + ".svg"))))
        ident = camel(name)
        variants.append(f"    {ident},")
        rows = []
        for grp in groups:
            rings = ", ".join("&[" + ", ".join(f"[{p[0] * scale:.2f}, {p[1] * scale:.2f}]" for p in ring) + "]" for ring in grp)
            rows.append(f"        &[{rings}],")
        body.append(f"const {ident.upper()}: &[&[&[[f32; 2]]]] = &[\n" + "\n".join(rows) + "\n];")
        arms.append(f"            Art::{ident} => {ident.upper()},")
        print(name, len(groups), sum(len(r) for g in groups for r in g))
    text = (
        "//! OneClient-style icons as vector shapes on the 24 by 24 icon grid, converted by\n"
        "//! `tools/vantage-brand/gen_icon_art.py` from supplied artwork. Generated; do not edit by hand.\n\n"
        "/// A shape: its outline first, then the holes cut out of it.\n"
        "pub type Shape = &'static [&'static [[f32; 2]]];\n\n"
        "/// An icon converted from artwork.\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub enum Art {\n"
        + "\n".join(variants)
        + "\n}\n\nimpl Art {\n    /// Every icon, for tests and previews.\n    pub const ALL: &'static [Art] = &[\n"
        + "\n".join(f"        Art::{camel(n)}," for n in names)
        + "\n    ];\n\n    /// The icon's solid shapes.\n    #[must_use]\n    pub fn shapes(self) -> &'static [Shape] {\n        match self {\n"
        + "\n".join(arms).replace("=> ", "=> ")
        + "\n        }\n    }\n}\n\n#[rustfmt::skip]\nmod data {\n    use super::Shape;\n"
        + "\n".join("    pub " + b.replace("&[&[&[[f32; 2]]]]", "&[Shape]").replace("\n", "\n    ") for b in body)
        + "\n}\nuse data::*;\n"
    )
    open(out, "w", newline="\n").write(text)


if __name__ == "__main__":
    main()
