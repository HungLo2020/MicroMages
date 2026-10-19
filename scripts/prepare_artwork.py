#!/usr/bin/env python3
"""Resize the checked-in official Steam artwork for Basalt, without network access."""
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1] / "packaging/artwork"
# Ubuntu 22.04's Pillow predates the Resampling enum; the filters are identical.
RESAMPLING = getattr(Image, "Resampling", Image)


def prepare(destination=ROOT):
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    # Preserve the complete 2:3 illustration, proportions, title and credit.
    # A restrained black frame changes the canvas to Basalt's exact 3:4 ratio.
    with Image.open(ROOT / "source/steam-cover.jpg") as source:
        cover = Image.new("RGB", (900, 1200), (0, 0, 0))
        cover.paste(source.convert("RGB").resize((800, 1200), RESAMPLING.LANCZOS), (50, 0))
        cover.save(destination / "cover.png", optimize=True)
    # The original largest ICO frame is pixel art. Integer enlargement retains
    # its pixels; transparent padding makes a square 512px launcher emblem.
    with Image.open(ROOT / "source/steam-icon.ico") as source:
        icon = Image.new("RGBA", (512, 512), (0, 0, 0, 0))
        emblem = source.convert("RGBA").resize((480, 480), RESAMPLING.NEAREST)
        icon.paste(emblem, (16, 16))
        icon.save(destination / "icon.png", optimize=True)


if __name__ == "__main__":
    prepare()
