#!/usr/bin/env python3
"""Build the Basalt ZIP from a Linux x64 release binary (no compilation)."""
import argparse
import hashlib
import json
import platform
import shutil
import subprocess
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BUNDLED = {
    "libasound.so.2": "libasound2*/copyright",
    "libudev.so.1": "libudev1/copyright",
    "libgcc_s.so.1": "libgcc-s1/copyright",
}
SYSTEM = {"libc.so.6", "libm.so.6", "libpthread.so.0", "libdl.so.2", "librt.so.1"}


def package(binary, output, revision):
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        raise ValueError("Package on Linux x86_64")
    dynamic = subprocess.check_output(["readelf", "-d", str(binary)], text=True)
    if "$ORIGIN/../lib" not in dynamic:
        raise ValueError("Build with RUSTFLAGS='-C link-arg=-Wl,-rpath,$ORIGIN/../lib'")
    with tempfile.TemporaryDirectory(prefix="micromages-package-") as directory:
        root = Path(directory)
        app = root / "app"
        (app / "bin").mkdir(parents=True)
        (app / "lib").mkdir()
        (app / "licenses").mkdir()
        executable = app / "bin/micromages"
        shutil.copy2(binary, executable)
        executable.chmod(0o755)
        shutil.copytree(ROOT / "assets", app / "assets")
        shutil.copy2(ROOT / "README.md", app / "README.md")
        shutil.copy2(ROOT / "packaging/basalt.json", app / "basalt.json")
        found = set()
        for line in subprocess.check_output(["ldd", str(binary)], text=True).splitlines():
            parts = line.split()
            if "not found" in line:
                raise ValueError(f"Unresolved dependency: {line}")
            if len(parts) < 3 or parts[1] != "=>":
                continue  # kernel vDSO and the system ELF loader
            name, source = parts[0], Path(parts[2])
            if name in SYSTEM:
                continue
            if name not in BUNDLED:
                raise ValueError(f"Unclassified runtime dependency: {name}")
            shutil.copy2(source, app / "lib" / name)
            found.add(name)
            notices = list(Path('/usr/share/doc').glob(BUNDLED[name]))
            if not notices:
                raise ValueError(f"Missing distribution license notice for {name}")
            shutil.copy2(notices[0], app / "licenses" / f"{name}.copyright")
        if found != set(BUNDLED):
            raise ValueError(f"Expected runtime libraries {set(BUNDLED)}, found {found}")
        (app / "build-info.json").write_text(json.dumps({
            "commit": revision, "platform": "linux-x64", "basalt_standard": 1,
        }, indent=2) + "\n")
        output.parent.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
            for path in sorted(app.rglob("*")):
                if path.is_file():
                    archive.write(path, path.relative_to(root))
    checksum = hashlib.sha256(output.read_bytes()).hexdigest()
    output.with_suffix(output.suffix + ".sha256").write_text(f"{checksum}  {output.name}\n")
    print(f"Packaged {output} ({output.stat().st_size} bytes)")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/micro-mages-re")
    parser.add_argument("--output", type=Path, default=ROOT / "dist/MicroMages-latest-linux-x64.zip")
    parser.add_argument("--revision", required=True)
    args = parser.parse_args()
    package(args.binary.resolve(), args.output.resolve(), args.revision)
