#!/usr/bin/env python3
"""Create or update the "Enterprise (dev)" Prism Launcher instance.

The pack in pack/ (packwiz) is the source of truth for which mods ship. This script
mirrors it into a Prism instance and drops in the locally built core jar, so the
instance always plays the current pack plus the current build. The testbench datapack
(dev/testbench) is copied into the instance's "testing" world.

    python3 scripts/prism_sync.py             # full sync: instance files, pack mods, core jar, testbench
    python3 scripts/prism_sync.py --prelaunch # core jar and testbench only (Prism pre-launch hook)
"""

import argparse
import hashlib
import json
import shutil
import sys
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
PACK = REPO / "pack"
CORE_LIBS = REPO / "core" / "build" / "libs"
PRISM = Path.home() / "Library" / "Application Support" / "PrismLauncher"
INSTANCE_NAME = "Enterprise (dev)"
INSTANCE = PRISM / "instances" / "Enterprise-dev"
GAME = INSTANCE / ".minecraft"
MODS = GAME / "mods"
# Pack-managed jars from the last sync, so jars dropped from the pack get removed.
MANIFEST = MODS / ".enterprise-managed.json"
CORE_PREFIX = "enterprise-"
TESTBENCH = REPO / "dev" / "testbench"
TEST_WORLD = GAME / "saves" / "testing"
HASH_FORMATS = {"sha1", "sha256", "sha512"}


def load_toml(path: Path) -> dict:
    # tomllib is Python 3.11+; the --prelaunch hook runs under Prism's older system python3.
    import tomllib

    return tomllib.loads(path.read_text())


def write_instance_files(pack: dict) -> None:
    INSTANCE.mkdir(parents=True, exist_ok=True)
    versions = pack["versions"]
    mmc_pack = {
        "components": [
            {"uid": "org.lwjgl3", "version": "3.3.3", "dependencyOnly": True},
            {"uid": "net.minecraft", "version": versions["minecraft"], "important": True},
            {"uid": "net.neoforged", "version": versions["neoforge"]},
        ],
        "formatVersion": 1,
    }
    (INSTANCE / "mmc-pack.json").write_text(json.dumps(mmc_pack, indent=4) + "\n")

    cfg = INSTANCE / "instance.cfg"
    if cfg.exists():
        return  # Prism owns this file after creation (play time, UI state).
    java = PRISM / "java" / "java-runtime-delta" / "bin" / "java"
    # A no-argument wrapper: Prism's settings file mangles quoted arguments.
    pre_launch = REPO / "scripts" / "prism-prelaunch"
    cfg.write_text(
        "\n".join(
            [
                "[General]",
                "ConfigVersion=1.3",
                "InstanceType=OneSix",
                f"name={INSTANCE_NAME}",
                "iconKey=default",
                "OverrideMemory=true",
                "MinMemAlloc=1024",
                "MaxMemAlloc=6144",
                "OverrideJavaLocation=true",
                f"JavaPath={java}",
                "OverrideCommands=true",
                f"PreLaunchCommand={pre_launch}",
                'notes="Dev instance for the Enterprise pack. Mods come from pack/ in the repo '
                '(python3 scripts/prism_sync.py); the core jar is copied in on every launch."',
                "",
            ]
        )
    )


def sync_pack_mods() -> None:
    MODS.mkdir(parents=True, exist_ok=True)
    wanted = {}
    for meta_path in sorted((PACK / "mods").glob("*.pw.toml")):
        meta = load_toml(meta_path)
        if meta.get("side", "both") == "server":
            continue
        wanted[meta["filename"]] = meta

    previous = set(json.loads(MANIFEST.read_text())) if MANIFEST.exists() else set()
    for stale in previous - wanted.keys():
        (MODS / stale).unlink(missing_ok=True)
        print(f"removed {stale}")

    for filename, meta in wanted.items():
        target = MODS / filename
        download = meta["download"]
        if download.get("hash-format") not in HASH_FORMATS:
            sys.exit(f"{filename}: unsupported hash format {download.get('hash-format')}")
        if target.exists() and _hash(target, download["hash-format"]) == download["hash"]:
            continue
        print(f"downloading {filename}")
        tmp = target.with_suffix(".part")
        with urllib.request.urlopen(download["url"]) as resp, tmp.open("wb") as out:
            shutil.copyfileobj(resp, out)
        if _hash(tmp, download["hash-format"]) != download["hash"]:
            tmp.unlink()
            sys.exit(f"{filename}: hash mismatch")
        tmp.replace(target)

    MANIFEST.write_text(json.dumps(sorted(wanted)) + "\n")


def _hash(path: Path, fmt: str) -> str:
    h = hashlib.new(fmt)
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def copy_core_jar() -> None:
    jars = [
        p for p in CORE_LIBS.glob(f"{CORE_PREFIX}*.jar")
        if not p.name.endswith(("-sources.jar", "-javadoc.jar"))
    ] if CORE_LIBS.exists() else []
    if not jars:
        print("no core jar found; run `just build` first", file=sys.stderr)
        return
    newest = max(jars, key=lambda p: p.stat().st_mtime)
    MODS.mkdir(parents=True, exist_ok=True)
    for old in MODS.glob(f"{CORE_PREFIX}*.jar"):
        old.unlink()
    shutil.copy2(newest, MODS / newest.name)
    print(f"core jar: {newest.name}")


def copy_testbench() -> None:
    if not TEST_WORLD.exists():
        return  # Create a world named "testing" in the instance to get the testbench.
    target = TEST_WORLD / "datapacks" / "enterprise-testbench"
    shutil.rmtree(target, ignore_errors=True)
    shutil.copytree(TESTBENCH, target)
    print("testbench: copied into saves/testing (run /reload if the world is open)")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--prelaunch", action="store_true", help="only copy the core jar and testbench")
    args = parser.parse_args()

    if not args.prelaunch:
        pack = load_toml(PACK / "pack.toml")
        write_instance_files(pack)
        sync_pack_mods()
    copy_core_jar()
    copy_testbench()


if __name__ == "__main__":
    main()
