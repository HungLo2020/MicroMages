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
from verify_artwork import verify_artwork

ROOT = Path(__file__).resolve().parents[1]
# Platform libraries belong to the host, including ALSA's matching plugins/config.
HOST_LIBRARIES = {
    "libasound.so.2", "libudev.so.1", "libgcc_s.so.1", "libstdc++.so.6",
    "ld-linux-x86-64.so.2", "libc.so.6", "libm.so.6", "libpthread.so.0", "libdl.so.2", "librt.so.1",
}


def package(binary, output, revision):
    verify_artwork(ROOT / "packaging/artwork")
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        raise ValueError("Package on Linux x86_64")
    dynamic = subprocess.check_output(["readelf", "-d", str(binary)], text=True)
    if "(RPATH)" in dynamic or "(RUNPATH)" in dynamic:
        raise ValueError("Release binaries must use host libraries without an embedded library search path")
    with tempfile.TemporaryDirectory(prefix="micromages-package-") as directory:
        root = Path(directory)
        app = root / "app"
        (app / "bin").mkdir(parents=True)
        (app / "licenses").mkdir()
        executable = app / "bin/micromages"
        shutil.copy2(binary, executable)
        executable.chmod(0o755)
        shutil.copytree(ROOT / "assets", app / "assets")
        shutil.copy2(ROOT / "README.md", app / "README.md")
        (app / "basalt").mkdir()
        for name in ["cover.png", "icon.png", "sources.json"]:
            shutil.copy2(ROOT / "packaging/artwork" / name, app / "basalt" / name)
        for line in subprocess.check_output(["ldd", str(binary)], text=True).splitlines():
            if "not found" in line:
                raise ValueError(f"Unresolved host dependency: {line}")
        needed = {line.split("[")[1].split("]")[0] for line in dynamic.splitlines() if "(NEEDED)" in line}
        if not needed <= HOST_LIBRARIES:
            raise ValueError(f"Unclassified direct host dependencies: {needed - HOST_LIBRARIES}")
        for notice in (ROOT / "packaging/licenses").glob("*"):
            shutil.copy2(notice, app / "licenses" / notice.name)
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
