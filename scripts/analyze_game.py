#!/usr/bin/env python3
"""Static 6502 control-flow evidence; py65 is used only as a disassembler."""
from pathlib import Path
import json
from py65.devices.mpu6502 import MPU
from py65.disassembler import Disassembler
rom=Path('analysis/game-prg.bin').read_bytes()
m=MPU(); m.memory[0x8000:]=rom
d=Disassembler(m)
seeds={0x8f72,0xbf86,0xf47d}
# Proven split-address RTS dispatch: high table pushed before low table.
for lo,hi,count in [(0xb35f,0xb366,6),(0x8dd2,0x8de2,16),(0xb42d,0xb433,5),(0x9db0,0x9db7,7),(0xf879,0xf87e,5),(0xcadf,0xcaeb,12)]:
 seeds.update(((m.memory[hi+i]<<8)|m.memory[lo+i])+1 for i in range(count))
# Callback registration: immediate A=low/Y=high before JSR/JMP bf4e or 9287.
for off in range(len(rom)-7):
 if rom[off]==0xa9 and rom[off+2]==0xa0 and rom[off+4] in (0x20,0x4c) and (rom[off+5]|rom[off+6]<<8) in (0xbf4e,0x9287):
  seeds.add((rom[off+1]|rom[off+3]<<8)+1)
# Entity types 1..0x46 have init/update/destroy callback pairs.
for i in range(1,0x47):
 if i in (0x25,0x26,0x27): continue
 p=m.memory[0xb836+i]|m.memory[0xb8da+i]<<8
 for j in (0,2,4):
  t=(m.memory[p+j]<<8|m.memory[p+j+1])+1
  if 0x8000<=t<0xfffa:seeds.add(t)
seeds={s for s in seeds if 0x8000<=s<0xfffa}
known={}; pending=list(seeds); indirect=[]; invalid=[]
while pending:
 a=pending.pop()
 while 0x8000<=a<0xfffa and a not in known:
  n,s=d.instruction_at(a); op=m.memory[a]
  if op==4:n,s=2,'YIELD_RENDER'
  if s=='???':invalid.append(a);break
  known[a]=(n,s)
  if op==0x20:pending.append(m.memory[a+1]|m.memory[a+2]<<8)
  if op==0x4c:a=m.memory[a+1]|m.memory[a+2]<<8;continue
  if op==0x6c:indirect.append(a);break
  if op in (0x60,0x40,0):break
  if op in (0x10,0x30,0x50,0x70,0x90,0xb0,0xd0,0xf0):
   off=m.memory[a+1];pending.append(a+2+(off if off<128 else off-256))
  a+=n
Path('analysis/game-disassembly.txt').write_text('\n'.join(f'{a:04x}: {s}' for a,(n,s) in sorted(known.items()))+'\n')
report={'instructions':len(known),'seeds':[hex(s) for s in sorted(seeds)],'indirect_jumps':[hex(a) for a in indirect],'unknown_opcodes':[hex(a) for a in invalid],'rts_sites':[hex(a) for a,(n,s) in sorted(known.items()) if s=='RTS']}
Path('analysis/game-flow.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
