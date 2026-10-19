#!/usr/bin/env python3
"""Recover editable graphics and level data from the archived PC game regions.

This one-time migration tool is not part of the game build. Decoder provenance:
level pointers D4F9; map expansion EFC3-F0E3; metatiles F148; palettes 9FC8;
spawn stream F23F; animation records F4CE. Addresses refer to source PRG mapped
at 8000. Outputs contain expanded data, never instructions or packed streams.
Requires Pillow. Supply the preserved source regions explicitly.
"""

import argparse
import hashlib
import json
import re
from pathlib import Path
from PIL import Image

COLORS = [
    0x585858,
    0x00237C,
    0x0D1099,
    0x300092,
    0x4F006C,
    0x600035,
    0x5C0500,
    0x461800,
    0x272D00,
    0x093E00,
    0x004500,
    0x004106,
    0x003545,
    0,
    0,
    0,
    0xA1A1A1,
    0x0B53D7,
    0x3337FE,
    0x6621F7,
    0x9515BE,
    0xAC166E,
    0xA62721,
    0x864300,
    0x596200,
    0x2D7A00,
    0x0C8500,
    0x007F2A,
    0x006D85,
    0,
    0,
    0,
    0xFFFFFF,
    0x51A5FE,
    0x8084FE,
    0xBC6AFE,
    0xF15BFE,
    0xFE5EC4,
    0xFE7269,
    0xE19321,
    0xADB600,
    0x79D300,
    0x51DF21,
    0x3AD974,
    0x39C3DF,
    0x424242,
    0,
    0,
    0xFFFFFF,
    0xB5D9FE,
    0xCACAFE,
    0xE3BEFE,
    0xF9B8FE,
    0xFEBAE7,
    0xFEC3BC,
    0xF4D199,
    0xDEE086,
    0xC6EC87,
    0xB2F29D,
    0xA7F0C3,
    0xA8E7F0,
    0xACACAC,
    0,
    0,
]
ANIMATION_NAMES = {
    0x9849: "pickup_gem",
    0x9684: "pickup_large_gem",
    0x9627: "pickup_feather",
    0x96BF: "pickup_swap",
    0x9852: "pickup_fairy",

    0x9767: "player_float_spent",
    0x9775: "player_float",
    0x97C7: "player_portal",
    0x9721: "player_death",
    0x9749: "player_look_down",
    0x9728: "player_ghost",
    0x979C: "player_ghost_float",
    0x97A7: "player_idle",
    0x9786: "player_run",
    0x9797: "player_jump",
    0x97AC: "player_fall",
    0x9744: "player_shoot_horizontal",
    0x97B1: "player_shoot_up",
    0x971C: "player_shoot_down",
    0x976C: "player_wall_slide",
    0x9762: "player_fast_fall",
    0x9717: "player_ladder_idle",
    0x97B6: "player_ladder_move",
    0x96D8: "turret_idle",
    0x980D: "summoning_boss_idle",
    0x9845: "turret_fire",
    0x9705: "lightning_middle",
    0x96EB: "lightning_end",
    0x96F4: "lightning_warning",
    0x96C8: "turret_minion_walk",
    0x9833: "goat_idle",
    0x9659: "willowisp_hurt",
    0x966D: "willowisp_idle",
    0x95C3: "troll_attack",
    0x95D4: "minion_fly",
    0x9635: "snake_attack",
    0x96E3: "sorcerer_boss_wounded",
    0x96DF: "sorcerer_boss_transformed",
    0x9630: "snake_idle",
    0x97F7: "walker_walk",
    0x97E8: "walker_attack",
    0x96DB: "sorcerer_boss_idle",
    0x95E9: "diagonal_flyer_awaken",
    0x9679: "bomb_walk",
    0x986A: "wall_spike",
    0x960C: "hopper_rise",
    0x9601: "chaser_fly",
    0x9618: "hopper_grounded",
    0x9830: "goat_attack",
}
MUSIC = [
    "mysterious",
    "heroic",
    "spooky",
    "dreamy",
    "evil",
    "intro",
    "fanfare",
    "boss",
    "march",
]


ENTITY_NAMES = {
    1: "inhaling_boss",
    2: "summoning_boss",
    3: "flying_boss",
    4: "checkpoint",
    5: "turret",
    6: "final_boss",
    7: "goat",
    8: "sorcerer_boss",
    9: "willowisp",
    10: "bomb",
    11: "troll",
    12: "snake",
    14: "walker",
    18: "diagonal_flyer",
    19: "chaser",
    20: "hopper",
    24: "leaper",
    25: "wall_spike",
    26: "horizontal_wind",
    27: "lightning",
    28: "level_exit",
    29: "spiderweb",
    31: "wind_current",
    33: "player_start",
    36: "summoning_platform",
    40: "flame",
    46: "portal_blue",
    47: "segmented_boss",
    48: "portal_green",
    49: "boss_entrance",
    51: "portal_pink",
}


class Source:
    def __init__(self, data):
        assert len(data) == 32768
        self.data = data

    def byte(self, address):
        assert 0x8000 <= address < 0x10000, hex(address)
        return self.data[address - 0x8000]

    def word(self, address):
        return self.byte(address) + 256 * self.byte(address + 1)


class Bits:
    def __init__(self, source, address):
        self.source, self.position = source, address * 8

    def read(self, count):
        value = 0
        for _ in range(count):
            value = value * 2 + (
                (self.source.byte(self.position // 8) >> (7 - self.position % 8)) & 1
            )
            self.position += 1
        return value


def palette(source, address):
    bits = Bits(source, address)
    return [15 if i % 4 == 0 else bits.read(6) for i in range(16)]


def theme(source, index, hard):
    b = source.byte
    a = source.word(0xA937 + index * 2)
    tile_count, block_count = b(a), b(a + 1)
    a += 2
    tiles = [
        [b(source.word(0xF2F4 + part * 2) + i) for i in range(76)]
        + [b(a + part * tile_count + i) for i in range(tile_count)]
        for part in range(4)
    ]
    a += 4 * tile_count
    colors = [(b(a + i // 4) >> (2 * (i % 4))) & 3 for i in range(76 + tile_count)]
    a += (76 + tile_count + 3) // 4
    blocks = [
        [b(source.word(0xF2FC + part * 2) + i) for i in range(36)]
        + [b(a + part * block_count + i) for i in range(block_count)]
        for part in range(4)
    ]
    a += 4 * block_count
    if hard:
        a += 9
    bg = palette(source, a)
    # Mode-specific palette substitutions in EED3.
    if index == 0 and hard:
        colors = [1 if i in (3, 4) else i for i in colors]
    if index == 2:
        start, end = (0, 80) if hard else (85, 124)
        for i in range(start, min(end, len(colors))):
            if colors[i] == 2:
                colors[i] = 0
    return tiles, blocks, colors, bg + palette(source, 0xB786)


def mirror(source, tile, tower):
    b = source.byte
    if 30 <= tile < b(0xA933 + tower):
        return tile ^ 1
    if tile == 17:
        return 0
    for i in reversed(range(6)):
        if tile == b(0xA927 + i):
            return b(0xA92D + i)
        if tile == b(0xA92D + i):
            return b(0xA927 + i)
    return tile


def level(source, index, hard=False):
    b = source.byte
    a = source.word(0xD4F9 + index * 2)
    tower = min(index // 3, 3)
    floor = index % 3
    if index >= 12:
        tower, floor = 3, 3
    count = 32 if index == 13 else 128
    tiles, blocks, colors, palettes = theme(source, tower, hard)
    if index == 13:
        palettes = palette(source, 0xAD7A) + palette(source, 0xB786)
    layout = [b(a + 1 + i) for i in range(count)]
    override_address = a + 1 + count
    override = []
    for i in range(b(a)):
        val = b(override_address + i)
        override += [(val >> 4) + 47 if val >> 4 else None] + [None] * (val & 15)
    rows = []
    for group in range(count // 4):
        codes = layout[group * 4 : group * 4 + 4]
        rotate = sum((v & 1) << (3 - i) for i, v in enumerate(codes))
        # Original emits bottom half first, then the top half, scrolling upward.
        for half in (1, 0):
            halfrow = []
            for i, code in enumerate(codes):
                block = code >> 1
                if (
                    hard
                    and group * 4 + 3 - i < len(override)
                    and override[group * 4 + 3 - i] is not None
                ):
                    block = override[group * 4 + 3 - i]
                halfrow.extend([blocks[half * 2 + 1][block], blocks[half * 2][block]])
            halfrow.reverse()
            row = [0] * 16
            for i, v in enumerate(halfrow):
                row[(rotate + i) % 16] = v
            for i in range(8):
                row[(rotate + 8 + i) % 16] = mirror(
                    source, row[(rotate + 7 - i) % 16], tower
                )
            rows.append(row)
    rows.reverse()
    width, height = 32, len(rows) * 2
    terrain = []
    for metatile_id in range(len(colors)):
        pictures = [part[metatile_id] for part in tiles]
        materials = [
            b(0xA93F + picture) if picture < 24 else (1 if picture < 86 else 0)
            for picture in pictures
        ]
        # Hidden hard-mode block faces do not change their collision geometry.
        if hard and tower == 1:
            for half in range(2):
                if 18 <= pictures[half * 2] < 24:
                    pictures[half * 2 : half * 2 + 2] = [16, 16]
        terrain.append(
            dict(
                id=metatile_id,
                tiles=pictures,
                collision=materials,
                palette=colors[metatile_id],
            )
        )
    out_tiles = []
    out_colors = []
    out_collision = []
    for row in rows:
        for half in range(2):
            for tile in row:
                for side in range(2):
                    picture = tiles[half * 2 + side][tile]
                    collision = (
                        b(0xA93F + picture)
                        if picture < 24
                        else (1 if picture < 86 else 0)
                    )
                    # Hard tower two hides these block faces visually; collision
                    # remains derived from the original metatile (F194-F1AB).
                    if hard and tower == 1 and 18 <= tiles[half * 2][tile] < 24:
                        picture = 16
                    out_tiles.append(picture)
                    out_colors.append(colors[tile])
                    out_collision.append(collision)
    stream = Bits(source, a + 1 + count + b(a))
    row = 0
    spawns = []
    for _ in range(1024):
        command = stream.read(2)
        if command == 3:
            break
        if command == 0:
            row += stream.read(3) + 1
            continue
        flip = stream.read(2) if command == 1 else 0
        x = stream.read(5) * 8 if command == 1 else stream.read(4) * 16 + 8
        kind = b(0xD4DA + stream.read(5 if command == 1 else 4))
        offset = 4 if kind == 33 else b(0xB774 + b(0xB78E + kind))
        spawns.append(
            dict(
                kind=kind,
                entity=ENTITY_NAMES[kind],
                x=x,
                y=height * 8 - row * 8 + offset,
                flip_x=bool(flip & 1),
                flip_y=bool(flip & 2),
            )
        )
    else:
        raise ValueError("Unterminated spawn stream")
    return dict(
        id=index,
        hard=hard,
        name=f"Tower {tower+1}-{floor+1}",
        tower=tower,
        floor=floor,
        music=MUSIC[b(0xD4CC + index)],
        boss=any(spawn["entity"].endswith("_boss") for spawn in spawns),
        width=width,
        height=height,
        tiles=out_tiles,
        metatiles=[metatile for row in rows for metatile in row],
        terrain=terrain,
        palettes=out_colors,
        collision=out_collision,
        colors=palettes,
        spawns=spawns,
        player_start=next(s for s in spawns if s["kind"] == 33),
    )


def animations(source, disassembly):
    """Decode frame records, optional offsets and wand attachments, and loop targets."""
    b = source.byte
    pointers = set(ANIMATION_NAMES)
    for line in disassembly.splitlines():
        if line.endswith(": BRK"):
            address = int(line[:4], 16)
            if address + 2 < 0x10000:
                pointers.add(source.word(address + 1))
    for address in range(0x8000, 0xFFF8):
        if (
            b(address) == 0xA9
            and b(address + 2) == 0xA0
            and b(address + 4) in (0x20, 0x4C)
            and source.word(address + 5) in (0xF495, 0xD3F8)
        ):
            pointers.add(b(address + 1) + 256 * b(address + 3))
    result = []
    for address in sorted(pointers):
        position = 0
        frames = []
        seen = {}
        loop = None
        try:
            while position not in seen and len(frames) < 128:
                seen[position] = len(frames)
                header = b(address + position)
                if header >= 128:
                    position += header - 256
                    if position < 0:
                        raise ValueError()
                    continue
                duration = header >> 2
                if duration == 0:
                    raise ValueError()
                frame = dict(
                    duration=duration,
                    sprite=b(address + position + 1),
                    offset_x=0,
                    offset_y=0,
                )
                position += 2
                if header & 1:
                    offset = b(address + position)
                    position += 1
                    frame.update(offset_x=(offset & 15) - 8, offset_y=(offset >> 4) - 8)
                if header & 2:
                    hit = b(address + position)
                    offset = b(address + position + 1)
                    position += 2
                    frame["attachment"] = dict(
                        tile=256 + hit,
                        offset_x=(offset & 15) - 8,
                        offset_y=(offset >> 4) - 8,
                    )
                frames.append(frame)
            if position in seen:
                loop = seen[position]
            if not frames or len(frames) >= 128:
                continue
            result.append(
                dict(
                    id=ANIMATION_NAMES.get(
                        address, f"catalog_animation_{len(result):03}"
                    ),
                    source_address=f"{address:04x}",
                    frames=frames,
                    loop_start=loop,
                )
            )
        except (AssertionError, ValueError):
            continue
    return dict(animations=result)


def sprites(source):
    b = source.byte
    definitions = []
    sprite_addresses = sorted(
        {b(0xB7E2 + k) + 256 * b(0xB80F + k) for k in range(1, 45) if b(0xB78E + k)}
    )
    signed = lambda x: x if x < 128 else x - 256
    for kind in range(1, 0x47):
        if kind in (0x25, 0x26, 0x27):
            continue
        size = b(0xB78E + kind)
        if kind > 44:
            continue  # Higher types use direct drawing routines or invisible triggers.
        # All frame offsets are encoded as one byte by the animation records.
        frames = []
        frame_ids = range(256)
        if size:
            address = b(0xB7E2 + kind) + 256 * b(0xB80F + kind)
            layout = b(0xEBA0 + size) + 256 * b(0xEBA9 + size)
            end = next((a for a in sprite_addresses if a > address), address + 256)
            frame_ids = range(0, min(256, end - address - 1), b(layout))
        for frame in frame_ids:
            parts = []
            if size == 0:
                if frame:
                    parts = [
                        dict(
                            tile=256 + ((frame & 63) + b(0xB7E2 + kind)) % 256,
                            x=-4,
                            y=-4,
                            flip_x=bool(frame & 64),
                            flip_y=bool(frame & 128),
                        )
                    ]
            else:
                address = b(0xB7E2 + kind) + 256 * b(0xB80F + kind)
                layout = b(0xEBA0 + size) + 256 * b(0xEBA9 + size)
                base = b(address)
                for part in range(b(layout)):
                    value = b(address + ((frame + part + 1) & 255))
                    if value & 63:
                        parts.append(
                            dict(
                                tile=256 + ((value & 63) + base) % 256,
                                x=signed(b(layout + 1 + part * 2)),
                                y=signed(b(layout + 2 + part * 2)),
                                flip_x=bool(value & 64),
                                flip_y=bool(value & 128),
                            )
                        )
            frames.append(dict(id=frame, parts=parts))
        definitions.append(
            dict(kind=kind, default_palette=b(0xB894 + kind) & 3, frames=frames)
        )
    # Segmented boss renders directly through C76C instead of the metasprite table.
    definitions.append(
        dict(
            kind=47,
            default_palette=b(0xB894 + 47) & 3,
            frames=[
                dict(
                    id=i,
                    parts=[
                        dict(
                            tile=256 + b(0xB6EB + i),
                            x=-4,
                            y=-4,
                            flip_x=False,
                            flip_y=False,
                        )
                    ],
                )
                for i in range(4)
            ],
        )
    )
    # Explicit art selected by direct-drawing routines, not metasprite aliases.
    # C75B/C792 select vertical/horizontal spells; F6DB freeze; 8F40 spear;
    # 9944 selects the goat bubble's four-frame cycle.
    direct_tiles = {
        50: [b(0xC352 + i) for i in range(3)],
        54: [0x18, 0x19, 0x1A, 0x1B],
        58: [b(0xC792), b(0xC75B)],
        53: [b(0xC793), b(0xC75C)],
        57: [0xE7],
        56: [0xDD],
        59: [b(0x9944 + i) for i in range(4)],
    }
    for kind, tile_ids in direct_tiles.items():
        definitions.append(
            dict(
                kind=kind,
                default_palette=b(0xB894 + kind) & 3,
                default_frame=0,
                frames=[
                    dict(
                        id=i,
                        parts=[
                            dict(
                                tile=256 + tile, x=-4, y=-4, flip_x=False, flip_y=False
                            )
                        ],
                    )
                    for i, tile in enumerate(tile_ids)
                ],
            )
        )
    # These source entities only trigger gameplay; their art is in terrain.
    for kind in (46, 48, 49, 51):
        definitions.append(
            dict(
                kind=kind,
                default_palette=0,
                default_frame=0,
                frames=[dict(id=0, parts=[])],
            )
        )
    defaults = {4: 0, 22: 2, 28: 0, 33: 1, 40: 1, 41: 1, 47: 0}
    for definition in definitions:
        if definition["kind"] in defaults:
            definition["default_frame"] = defaults[definition["kind"]]
    return dict(sprites=sorted(definitions, key=lambda definition: definition["kind"]))


def write_json(path, value):
    text = json.dumps(value, indent=2)

    def compact_numbers(match):
        numbers = re.findall(r"\d+", match.group())
        indent = match.group().split("\n")[-1].split("]")[0]
        rows = [", ".join(numbers[i : i + 32]) for i in range(0, len(numbers), 32)]
        if len(rows) == 1:
            return "[" + rows[0] + "]"
        return (
            "[\n"
            + ",\n".join(indent + "  " + row for row in rows)
            + "\n"
            + indent
            + "]"
        )

    text = re.sub(r"\[\n(?:\s+\d+,?\n)+\s*\]", compact_numbers, text)
    path.write_text(text + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--program", type=Path, required=True)
    parser.add_argument("--tiles", type=Path, required=True)
    parser.add_argument(
        "--disassembly", type=Path, default=Path("analysis/game-disassembly.txt")
    )
    parser.add_argument("--output", type=Path, default=Path("assets"))
    args = parser.parse_args()
    source = Source(args.program.read_bytes())
    chr_data = args.tiles.read_bytes()
    assert len(chr_data) == 8192
    graphics = args.output / "graphics"
    graphics.mkdir(parents=True, exist_ok=True)
    levels = args.output / "levels"
    levels.mkdir(parents=True, exist_ok=True)
    image = Image.new("RGBA", (128, 256))
    for tile in range(512):
        for y in range(8):
            for x in range(8):
                value = ((chr_data[tile * 16 + y] >> (7 - x)) & 1) | (
                    ((chr_data[tile * 16 + y + 8] >> (7 - x)) & 1) << 1
                )
                image.putpixel(
                    ((tile % 16) * 8 + x, (tile // 16) * 8 + y),
                    (value * 85, value * 85, value * 85, 255 if value else 0),
                )
    image.save(graphics / "tiles.png")
    write_json(graphics / "palette.json", dict(colors=COLORS))
    write_json(graphics / "sprites.json", sprites(source))
    write_json(
        graphics / "animations.json", animations(source, args.disassembly.read_text())
    )
    write_json(
        levels / "levels.json",
        dict(
            levels=[
                level(source, i, hard)
                for hard in (False, True)
                for i in range(14 if hard else 13)
            ]
        ),
    )
    write_json(
        args.output / "manifest.json",
        {
            "source_executable_sha256": "b7bf279c1b7ce6714aca5634719d300ea60d2a0b8e073918c17a3702ee0a6e9c",
            "source_program_sha256": hashlib.sha256(source.data).hexdigest(),
            "source_tiles_sha256": hashlib.sha256(chr_data).hexdigest(),
            "files": {
                str(path.relative_to(args.output)): hashlib.sha256(
                    path.read_bytes()
                ).hexdigest()
                for path in sorted(args.output.rglob("*"))
                if path.is_file()
                and path.name != "manifest.json"
                and path.suffix != ".bin"
            },
        },
    )
    print("Extracted 512 tiles and 27 level/mode records.")


if __name__ == "__main__":
    main()
