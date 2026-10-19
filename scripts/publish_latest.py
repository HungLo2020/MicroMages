#!/usr/bin/env python3
"""Replace the single mutable latest release after the workflow's validation job.

Upload under non-matching temporary names first, keeping the previous download
available if an upload fails. GitHub has no multi-asset atomic replacement API;
publication serializes the short rename/tag/metadata step and rolls names back
on failure. Release immutability must be disabled for this rolling-release model.
"""
import json
import os
import shutil
import subprocess
import tempfile
import time
from pathlib import Path


def api(path, method="GET", body=None, missing_ok=False):
    command = ["gh", "api", f"repos/{os.environ['GH_REPO']}/{path}", "--method", method]
    if body is not None:
        command += ["--input", "-"]
    result = subprocess.run(command, input=json.dumps(body) if body is not None else None,
                            capture_output=True, text=True)
    if result.returncode:
        if missing_ok and "HTTP 404" in result.stderr:
            return None
        raise RuntimeError(result.stderr.strip())
    return json.loads(result.stdout) if result.stdout.strip() else None


def promote_assets(old_assets, new_assets, update_release, request=api):
    """Restore old download names if a promotion fails; never delete first."""
    renamed_old, renamed_new = [], []
    try:
        for old in old_assets:
            request(f"releases/assets/{old['id']}", "PATCH", {"name": f"{old['name']}.{old['id']}.previous"})
            renamed_old.append(old)
        for new, name in new_assets:
            request(f"releases/assets/{new['id']}", "PATCH", {"name": name})
            renamed_new.append(new)
        update_release()
    except Exception:
        for new in reversed(renamed_new):
            request(f"releases/assets/{new['id']}", "PATCH", {"name": new['name']})
        for old in reversed(renamed_old):
            request(f"releases/assets/{old['id']}", "PATCH", {"name": old['name']})
        raise
    for old in renamed_old:
        request(f"releases/assets/{old['id']}", "DELETE")


def main():
    commit = os.environ["RELEASE_COMMIT"]
    if api("commits/main")["sha"] != commit:
        print("A newer commit is on main; this run will not replace latest.")
        return
    if api("immutable-releases")["enabled"]:
        raise RuntimeError("Repository release immutability must be disabled for rolling latest")
    files = [Path("dist/MicroMages-latest-linux-x64.zip"),
             Path("dist/MicroMages-latest-linux-x64.zip.sha256")]
    for path in files:
        if not path.is_file():
            raise ValueError(f"Missing release artifact {path}")
    release = api("releases/tags/latest", missing_ok=True)
    if release is None:
        release = api("releases", "POST", {"tag_name": "latest", "target_commitish": commit,
                      "name": "Latest", "draft": True, "prerelease": False})
    if release.get("immutable"):
        raise RuntimeError("The latest release is immutable; rolling releases require mutable releases")
    old_tag = api("git/ref/tags/latest", missing_ok=True)
    suffix = f".{commit[:12]}.{time.time_ns()}.uploading"
    with tempfile.TemporaryDirectory() as directory:
        staged = []
        for path in files:
            copy = Path(directory) / (path.name + suffix)
            shutil.copyfile(path, copy)
            staged.append(copy)
        subprocess.run(["gh", "release", "upload", "latest", *map(str, staged)], check=True)
        assets = api(f"releases/{release['id']}/assets?per_page=100")
        by_name = {asset["name"]: asset for asset in assets}
        new_assets = [(by_name[path.name + suffix], path.name) for path in files]
        old_assets = [by_name[path.name] for path in files if path.name in by_name]
        if api("commits/main")["sha"] != commit:
            for asset, _ in new_assets:
                api(f"releases/assets/{asset['id']}", "DELETE")
            print("Newer main detected after upload; kept previous latest.")
            return
        def publish():
            try:
                if old_tag is None:
                    api("git/refs", "POST", {"ref": "refs/tags/latest", "sha": commit})
                else:
                    api("git/refs/tags/latest", "PATCH", {"sha": commit, "force": True})
                api(f"releases/{release['id']}", "PATCH", {
                    "name": "Latest", "body": f"Rolling Linux x64 build of `{commit}`.\n\n"
                    "Basalt standard 1. Extract the ZIP and run app/bin/micromages. "
                    "Updates replace app/ and preserve data/. Ubuntu 22.04 or newer compatible desktop Linux.\n",
                    "target_commitish": commit, "draft": False, "prerelease": False, "make_latest": "true",
                })
            except Exception:
                if old_tag is not None:
                    api("git/refs/tags/latest", "PATCH", {"sha": old_tag["object"]["sha"], "force": True})
                raise
        promote_assets(old_assets, new_assets, publish)
    print(f"Published latest from {commit}")


if __name__ == "__main__":
    main()
