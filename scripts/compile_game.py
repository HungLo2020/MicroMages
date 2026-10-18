"""Compile the fixed, verified PC game into Rust basic blocks at build time.

No instruction decoder or interpreter is linked into the resulting game.
Address labels preserve provenance and the original computed control transfers.
instructions.json contains standard 6502 mnemonic/address-mode facts (cross-checked
against py65's published opcode table), not implementation code.
"""
import hashlib
import json
from pathlib import Path
import sys

assets, out = Path(sys.argv[1]), Path(sys.argv[2])
manifest = json.loads((assets / "manifest.json").read_text())
for name, digest in manifest["files"].items():
    if hashlib.sha256((assets / name).read_bytes()).hexdigest() != digest:
        raise SystemExit(f"Asset SHA-256 mismatch: {name}")
expected = manifest["source_executable_sha256"]
prg = (assets / "game-data.bin").read_bytes()
tiles = (assets / "tiles.bin").read_bytes()
if len(prg) != 0x8000 or len(tiles) != 0x2000:
    raise SystemExit("Unexpected program or tile data size")
out.mkdir(parents=True, exist_ok=True)
(out / "game-data.bin").write_bytes(prg)
(out / "tiles.bin").write_bytes(tiles)
ops = {int(k): v for k, v in json.loads(Path("scripts/instructions.json").read_text()).items()}
ops[4] = ["FRAME", "imm"]
lengths = dict(imp=1, acc=1, imm=2, zpg=2, zpx=2, zpy=2, inx=2, iny=2,
               rel=2, abs=3, abx=3, aby=3, ind=3)
branches = dict(BPL="!s.n", BMI="s.n", BVC="!s.v", BVS="s.v", BCC="!s.c", BCS="s.c", BNE="!s.z", BEQ="s.z")

def byte(a):
    return prg[a - 0x8000]

def word(a):
    return byte(a) | byte(a + 1) << 8

roots = {0x8f72, 0xbf86, 0xf47d, 0x8fc0}
for low, high, n in [(0xb35f, 0xb366, 7), (0x8dd2, 0x8de2, 16), (0xb42d, 0xb433, 6)]:
    for i in range(n):
        target = ((byte(low + i) | byte(high + i) << 8) + 1) & 0xffff
        if target >= 0x8000:
            roots.add(target)
extra = Path("scripts/game_roots.json")
if extra.exists():
    roots.update(int(x, 0) if isinstance(x, str) else x for x in json.loads(extra.read_text()))

decoded = {}
pending = list(roots)
leaders = set(roots)
while pending:
    addr = pending.pop()
    while addr not in decoded and 0x8000 <= addr <= 0xfffd:
        opcode = byte(addr)
        if opcode not in ops:
            break
        name, mode = ops[opcode]
        size = lengths[mode]
        if addr + size > 0x10000:
            break
        arg = word(addr + 1) if size == 3 else byte(addr + 1) if size == 2 else 0
        nxt = (addr + size) & 0xffff
        decoded[addr] = (name, mode, arg, nxt)
        targets = []
        terminal = False
        if name in branches:
            target = (nxt + (arg if arg < 128 else arg - 256)) & 0xffff
            targets = [nxt, target]
            terminal = True
        elif name == "JSR":
            targets = [arg, nxt]
            terminal = True
        elif name == "JMP":
            targets = [arg] if mode == "abs" else []
            terminal = True
        elif name == "FRAME":
            targets = [nxt]
            terminal = True
        elif name in ("RTS", "RTI", "BRK"):
            terminal = True
        if terminal:
            for t in targets:
                leaders.add(t)
                if t not in decoded:
                    pending.append(t)
            break
        addr = nxt

def address(mode, arg):
    a = f"0x{arg:04x}u16"
    return {
        "zpg": a, "abs": a,
        "zpx": f"(0x{arg:02x}u8.wrapping_add(s.x)) as u16",
        "zpy": f"(0x{arg:02x}u8.wrapping_add(s.y)) as u16",
        "abx": f"{a}.wrapping_add(s.x as u16)",
        "aby": f"{a}.wrapping_add(s.y as u16)",
        "inx": f"s.pointer(0x{arg:02x}u8.wrapping_add(s.x))",
        "iny": f"s.pointer(0x{arg:02x}).wrapping_add(s.y as u16)",
    }[mode]

def emit(addr, name, mode, arg, nxt):
    lines = [f"// ${addr:04x}: {name} {mode} ${arg:04x}"]
    if name in branches:
        target = (nxt + (arg if arg < 128 else arg - 256)) & 0xffff
        lines.append(f"return Ok(if {branches[name]} {{ 0x{target:04x} }} else {{ 0x{nxt:04x} }});")
    elif name == "JSR":
        lines += [f"s.push_word(0x{nxt - 1:04x});", f"s.on_transfer(0x{arg:04x});", f"return Ok(0x{arg:04x});"]
    elif name == "JMP":
        if mode == "abs":
            lines += [f"s.on_transfer(0x{arg:04x});", f"return Ok(0x{arg:04x});"]
        else:
            lines += [f"let lo = s.read(0x{arg:04x}) as u16;", f"let hi = s.read(0x{(arg & 0xff00) | ((arg + 1) & 0xff):04x}) as u16;", "let target = lo | hi << 8; s.on_transfer(target); return Ok(target);"]
    elif name == "RTS":
        lines += ["return Ok(s.pop_word().wrapping_add(1));"]
    elif name == "RTI":
        lines += ["let flags = s.pop(); s.set_flags(flags); return Ok(s.pop_word());"]
    elif name == "FRAME":
        lines += [f"s.frame_ready = true; return Ok(0x{nxt:04x});"]
    elif name == "BRK":
        # The game uses BRK as an animation-pointer call. Its IRQ routine consumes
        # the saved return location and the two inline data bytes, then RTSes.
        lines += [f"s.push_word(0x{(addr+2)&0xffff:04x});", "s.push(s.flags() | 0x30); s.interrupt_mask = true; return Ok(0xf47d);"]
    elif name in ("LDA", "LDX", "LDY", "ORA", "AND", "EOR", "ADC", "SBC", "CMP", "CPX", "CPY", "BIT"):
        if mode == "imm":
            lines.append(f"let value = 0x{arg:02x}u8;")
        else:
            lines += [f"let addr = {address(mode, arg)};", "let value = s.read(addr);"]
        if name.startswith("LD"):
            r = name[-1].lower()
            lines += [f"s.{r} = value; s.nz(value);"]
        elif name in ("ORA", "AND", "EOR"):
            op = dict(ORA="|", AND="&", EOR="^")[name]
            lines += [f"s.a {op}= value; s.nz(s.a);"]
        elif name == "ADC": lines += ["s.add(value);"]
        elif name == "SBC": lines += ["s.add(!value);"]
        elif name in ("CMP", "CPX", "CPY"):
            r = dict(CMP="a", CPX="x", CPY="y")[name]
            lines += [f"s.c = s.{r} >= value; s.nz(s.{r}.wrapping_sub(value));"]
        else: lines += ["s.z = s.a & value == 0; s.n = value & 0x80 != 0; s.v = value & 0x40 != 0;"]
    elif name in ("STA", "STX", "STY"):
        lines += [f"let addr = {address(mode,arg)};", f"s.write(addr, s.{name[-1].lower()});"]
    elif name in ("INC", "DEC", "ASL", "LSR", "ROL", "ROR"):
        if mode == "acc": lines += ["let value = s.a;"]
        else: lines += [f"let addr = {address(mode,arg)};", "let value = s.read(addr);"]
        operation = {
            "INC": "let value = value.wrapping_add(1);",
            "DEC": "let value = value.wrapping_sub(1);",
            "ASL": "s.c = value & 128 != 0; let value = value << 1;",
            "LSR": "s.c = value & 1 != 0; let value = value >> 1;",
            "ROL": "let carry = s.c as u8; s.c = value & 128 != 0; let value = (value << 1) | carry;",
            "ROR": "let carry = (s.c as u8) << 7; s.c = value & 1 != 0; let value = (value >> 1) | carry;",
        }[name]
        lines += [operation, "s.nz(value);", "s.a = value;" if mode == "acc" else "s.write(addr, value);"]
    elif name in ("TAX", "TAY", "TXA", "TYA", "TSX", "TXS"):
        fr, to = name[1].lower(), name[2].lower()
        fr = "sp" if fr == "s" else fr
        to = "sp" if to == "s" else to
        lines += [f"s.{to} = s.{fr};"]
        if to != "sp": lines += [f"s.nz(s.{to});"]
    elif name in ("INX", "INY", "DEX", "DEY"):
        r = name[-1].lower(); op = "add" if name.startswith("IN") else "sub"
        lines += [f"s.{r} = s.{r}.wrapping_{op}(1); s.nz(s.{r});"]
    elif name == "PHA": lines += ["s.push(s.a);"]
    elif name == "PLA": lines += ["s.a = s.pop(); s.nz(s.a);"]
    elif name == "PHP": lines += ["s.push(s.flags() | 0x30);"]
    elif name == "PLP": lines += ["let flags = s.pop(); s.set_flags(flags);"]
    elif name in ("CLC", "SEC"): lines += [f"s.c = {'true' if name == 'SEC' else 'false'};"]
    elif name == "CLV": lines += ["s.v = false;"]
    elif name in ("CLI", "SEI"): lines += [f"s.interrupt_mask = {'true' if name == 'SEI' else 'false'};"]
    elif name in ("CLD", "SED"): lines += [f"s.decimal = {'true' if name == 'SED' else 'false'};"]
    elif name == "NOP": pass
    else: raise ValueError((addr, name, mode))
    # Per-instruction temporary scope prevents accidental reuse of lowered locals.
    return ["{", *lines, "}"]

# Split at every known control-flow destination; long straight-line regions are
# ordinary Rust statements, not an instruction dispatch loop.
blocks = {}
for start in sorted(leaders):
    if start not in decoded: continue
    addr, lines = start, []
    while True:
        name, mode, arg, nxt = decoded[addr]
        lines.extend(emit(addr, name, mode, arg, nxt))
        if name in branches or name in ("JMP", "JSR", "RTS", "RTI", "FRAME", "BRK"):
            break
        if nxt in leaders or nxt not in decoded:
            lines.append(f"return Ok(0x{nxt:04x});")
            break
        addr = nxt
    blocks[start] = lines

generated = ["// Generated from the verified repository game data at build time. Do not edit.",
             "#[allow(unreachable_code, unused_variables, clippy::needless_return)]",
             "pub fn dispatch(s: &mut super::Game, block: u16) -> Result<u16, String> {", "match block {"]
# Shard to keep LLVM compile time bounded.
for page in sorted({a >> 8 for a in blocks}):
    generated.append(f"0x{page:02x}00..=0x{page:02x}ff => page_{page:02x}(s, block),")
generated.extend(['_ => Err(format!("Untranslated game block ${block:04x}")),', "}", "}"])
for page in sorted({a >> 8 for a in blocks}):
    generated.extend(["#[allow(unreachable_code, unused_variables, clippy::needless_return)]",
                      f"fn page_{page:02x}(s: &mut super::Game, block: u16) -> Result<u16, String> {{", "match block {"])
    for addr, lines in blocks.items():
        if addr >> 8 == page:
            generated.extend([f"0x{addr:04x} => {{", *lines, "}"])
    generated.extend(['_ => Err(format!("Untranslated game block ${block:04x}")),', "}", "}"])
(out / "native_game.rs").write_text("\n".join(generated) + "\n")
(out / "translation.json").write_text(json.dumps({"source_sha256":expected,"instructions":len(decoded),"blocks":len(blocks),"roots":sorted(roots)},indent=2))
print(f"Compiled {len(decoded)} instructions into {len(blocks)} native Rust blocks",file=sys.stderr)
