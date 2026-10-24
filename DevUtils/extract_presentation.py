#!/usr/bin/env python3
"""Convert observed original intro frames into editable PNG art and JSON timelines.

Frames are RGB PPM images named 0001.ppm onward, captured by the archived
reference implementation; this offline tool never ships executable game data.
Identical 8x8 artwork is stored once. Each screen cell has explicit [tick,tile]
keyframes, so artists can change artwork independently from timing.
"""
import argparse
import json
from pathlib import Path
from PIL import Image

def extract(source, output):
    output.mkdir(parents=True, exist_ok=True)
    artwork = [bytes(8*8*3)]
    index = {artwork[0]: 0}
    for name, start, stop in [('intro', 0, 948), ('title', 948, 1460)]:
        tracks = [[] for _ in range(960)]
        for tick, frame in enumerate(range(start, stop)):
            image = Image.new('RGB', (256,240)) if frame == 0 else Image.open(source/f'{frame:04}.ppm').convert('RGB')
            for cell in range(960):
                x,y = cell%32*8, cell//32*8
                raw = image.crop((x,y,x+8,y+8)).tobytes()
                tile = index.get(raw)
                if tile is None:
                    tile = len(artwork); index[raw] = tile; artwork.append(raw)
                if not tracks[cell] or tracks[cell][-1][1] != tile:
                    tracks[cell].append([tick,tile])
        (output/f'{name}.json').write_text(json.dumps({'width':256,'height':240,'tile_size':8,'duration':stop-start,'looping':name=='title','tracks':tracks},separators=(',',':'))+'\n')
    atlas = Image.new('RGB',(512, ((len(artwork)+63)//64)*8))
    for tile, raw in enumerate(artwork):
        atlas.paste(Image.frombytes('RGB',(8,8),raw),(tile%64*8,tile//64*8))
    atlas.save(output/'cinematic.png',optimize=True)
    print(f'{len(artwork)} unique artwork tiles, {atlas.size} PNG')

if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('frames',type=Path)
    parser.add_argument('--output',type=Path,default=Path('assets/presentation'))
    args=parser.parse_args();extract(args.frames,args.output)
