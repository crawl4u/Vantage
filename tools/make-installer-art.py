"""Renders the Windows setup's sidebar and badge from the Vantage logo pack.

Usage: make-installer-art.py <logo-set-dir> <out-dir>
Writes sidebar.png (440x800, shown at 220x400) and badge.png (128x128).
"""
import sys
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter

BACKGROUND = (16, 14, 42)
TOP = (46, 33, 128)
BOTTOM = (13, 11, 36)
GLOW = (110, 80, 235)


def trim(image: Image.Image) -> Image.Image:
    box = image.getchannel("A").getbbox()
    return image.crop(box) if box else image


def gradient(size: tuple[int, int]) -> Image.Image:
    width, height = size
    column = Image.new("RGB", (1, height))
    for y in range(height):
        t = y / (height - 1)
        column.putpixel((0, y), tuple(round(TOP[i] + (BOTTOM[i] - TOP[i]) * t) for i in range(3)))
    return column.resize(size).convert("RGBA")


def sidebar(logo_dir: Path) -> Image.Image:
    size = (440, 800)
    canvas = gradient(size)
    # A soft violet glow behind the mark.
    glow = Image.new("RGBA", size, (0, 0, 0, 0))
    ImageDraw.Draw(glow).ellipse((50, 190, 390, 530), fill=GLOW + (150,))
    canvas.alpha_composite(glow.filter(ImageFilter.GaussianBlur(70)))
    mark = trim(Image.open(logo_dir / "mark-transparent.png").convert("RGBA"))
    mark = mark.resize((250, round(250 * mark.height / mark.width)), Image.LANCZOS)
    canvas.alpha_composite(mark, ((size[0] - mark.width) // 2, 250))
    # The wordmark is the lockup without its mark: everything right of the mark.
    lockup = Image.open(logo_dir / "banner-lockup-transparent.png").convert("RGBA")
    alpha = lockup.getchannel("A")
    columns = [alpha.crop((x, 0, x + 1, lockup.height)).getbbox() is not None for x in range(lockup.width)]
    gap = next(x for x in range(600, lockup.width) if not any(columns[x:x + 40]))
    start = next(x for x in range(gap, lockup.width) if columns[x])
    word = trim(lockup.crop((start, 0, lockup.width, lockup.height)))
    word = word.resize((320, round(320 * word.height / word.width)), Image.LANCZOS)
    canvas.alpha_composite(word, ((size[0] - word.width) // 2, 560))
    # A hairline on the edge that meets the content.
    ImageDraw.Draw(canvas).line([(size[0] - 1, 0), (size[0] - 1, size[1])], fill=(120, 100, 235, 90), width=2)
    return canvas


def main() -> None:
    logo_dir, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    sidebar(logo_dir).save(out / "sidebar.png")
    icon = Image.open(logo_dir / "icon-rounded-128.png").convert("RGBA")
    icon.save(out / "badge.png", optimize=True)


main()
