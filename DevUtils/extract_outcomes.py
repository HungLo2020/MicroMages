#!/usr/bin/env python3
"""Archive original outcome artwork as editable PNG tiles and JSON timelines.

Research input is RGB frames from the separately retained original reference.
No program bytes are shipped. Dynamic score digits and menu selection are drawn
by Rust; participant choreography is captured for all four player counts.
"""
import argparse
import json
from pathlib import Path
from PIL import Image


def extract(source, output):
    output.mkdir(parents=True, exist_ok=True)
    artwork = [bytes(192)]
    index = {artwork[0]: 0}
    scenes = [('game-over', source/'mm-gameover-p4', [1950], False),
              ('normal-ending', source/'mm-ending', [1950], False)]
    scenes += [(f'hard-ending-{n}', source/f'mm-hard-ending-p{n}', range(1651, 3801), False) for n in range(1,5)]
    for name, folder, frames, looping in scenes:
        tracks = [[] for _ in range(960)]
        for tick, frame in enumerate(frames):
            image = Image.open(folder/f'{frame:04}.ppm').convert('RGB')
            for cell in range(960):
                x, y = cell%32*8, cell//32*8
                raw = image.crop((x,y,x+8,y+8)).tobytes()
                tile = index.get(raw)
                if tile is None:
                    tile = len(artwork)
                    index[raw] = tile
                    artwork.append(raw)
                if not tracks[cell] or tracks[cell][-1][1] != tile:
                    tracks[cell].append([tick,tile])
        (output/f'{name}.json').write_text(json.dumps(dict(width=256,height=240,tile_size=8,duration=len(frames),looping=looping,tracks=tracks),separators=(',',':'))+'\n')
    atlas = Image.new('RGB',(512, ((len(artwork)+63)//64)*8))
    for tile, raw in enumerate(artwork):
        atlas.paste(Image.frombytes('RGB',(8,8),raw),(tile%64*8,tile//64*8))
    atlas.save(output/'outcomes.png',optimize=True)
    print(f'{len(artwork)} unique tiles; {atlas.size} PNG')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('frames',type=Path)
    parser.add_argument('--output',type=Path,default=Path('assets/presentation/outcomes'))
    args = parser.parse_args()
    extract(args.frames,args.output)
