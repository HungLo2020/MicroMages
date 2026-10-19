#!/usr/bin/env python3
"""Validate Basalt's shipped artwork contract; does not download or alter images."""
from pathlib import Path
from PIL import Image


def verify_artwork(directory):
    directory = Path(directory)
    cover = directory / "cover.png"
    if cover.stat().st_size >= 2_000_000:
        raise ValueError("Cover must be under 2 MB")
    with Image.open(cover) as image:
        image.load()
        width, height = image.size
        if image.format != "PNG" or width < 600 or height < 800 or width * 4 != height * 3:
            raise ValueError("Cover must be a PNG, at least 600×800 and exactly 3:4")
        if image.convert("RGBA").getchannel("A").getextrema() != (255, 255):
            raise ValueError("Cover must be opaque")
    with Image.open(directory / "icon.png") as image:
        image.load()
        if image.format != "PNG" or image.width != image.height or image.width < 256:
            raise ValueError("Icon must be a square PNG at least 256×256")
        if image.mode != "RGBA" or image.getchannel("A").getextrema() != (0, 255):
            raise ValueError("Icon must contain a visible emblem and transparent padding")


if __name__ == "__main__":
    verify_artwork(Path(__file__).resolve().parents[1] / "packaging/artwork")
