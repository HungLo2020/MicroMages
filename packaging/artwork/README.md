# Micro Mages launcher artwork

Official artwork downloaded from [Micro Mages on Steam](https://store.steampowered.com/app/1065020/Micro_Mages/).
Artwork belongs to Morphcat Games; source files and attribution are preserved.
`sources.json` records the exact CDN URLs, retrieval date, source SHA-256 hashes,
and the transformations applied. These are adaptations of the original artwork,
not newly licensed or generated illustrations.

- `cover.png`: opaque RGB PNG, 900×1200, exact 3:4, under 2 MB. The complete Steam
  600×900 cover is resized proportionally to 800×1200 and centered in a 50-pixel
  black side frame. The title, characters and publisher credit remain intact.
- `icon.png`: transparent RGBA PNG, 512×512. Steam's 48×48 purple-mage icon is
  enlarged 10× with nearest-neighbor sampling and centered with 16-pixel padding.
- `source/`: unmodified downloaded cover and multi-resolution Windows icon.
  These files are retained for reproducibility but are not included in releases.

Run `python3 scripts/prepare_artwork.py` to rebuild the PNGs locally with Pillow.
Builds do not download images. `package_linux.py` ships the PNGs and provenance at
`app/basalt/`. Basalt discovers the installed images automatically; the catalog
does not request any separate artwork download. Artwork updates ship with the
game release. The URLs in `sources.json` document attribution and provenance only.
