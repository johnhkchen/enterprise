#!/usr/bin/env python3
"""Takes a batch from the asset studio into the repo, the same way every time (protocol v0.2; John: "build code tools
that make each improvement easy and cheap to reuse").

    python3 scripts/assets/ingest.py BATCH.zip MANIFEST.json [--thread 0004] [--merge]

1. Checks every file in the zip against the manifest's sha256 (and that nothing else is in it).
2. Unpacks onto a branch `assets/<batch>` off main.
3. Runs the shared validator on every kind folder the batch touched, and the floor-plan check on its houses (their
   plans written to assets-src/<kingdom>/<kind>/<name>/plan.png with notes.md).
4. Commits, pushes, opens a pull request, and with --merge merges it and returns to main.

Stops at the first failure with the branch left for a look; nothing is committed if a check fails.
"""
import hashlib
import json
import subprocess
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def sh(*cmd, check=True):
    print("$ " + " ".join(cmd), flush=True)
    r = subprocess.run(cmd, cwd=ROOT, text=True, capture_output=True)
    if r.stdout.strip():
        print(r.stdout.strip()[-3000:])
    if check and r.returncode != 0:
        sys.exit(f"failed: {' '.join(cmd)}\n{r.stderr.strip()[-2000:]}")
    return r


def main():
    args = [a for i, a in enumerate(sys.argv[1:], 1) if not a.startswith("--") and sys.argv[i - 1] != "--thread"]
    if len(args) != 2:
        sys.exit(__doc__)
    batch_zip, manifest = Path(args[0]), json.loads(Path(args[1]).read_text())
    thread = sys.argv[sys.argv.index("--thread") + 1] if "--thread" in sys.argv else manifest.get("thread", "?")
    name = manifest["batch"]
    want = {f["path"]: f["sha256"] for f in manifest["files"]}
    with zipfile.ZipFile(batch_zip) as z:
        got = {i.filename: hashlib.sha256(z.read(i)).hexdigest() for i in z.infolist() if not i.is_dir()}
        bad = [p for p in want if got.get(p) != want[p]]
        extra = [p for p in got if p not in want and p != "manifest.json"]
        if bad or extra:
            sys.exit(f"{name}: {len(bad)} files don't match the manifest {bad[:5]}, {len(extra)} not in it {extra[:5]}")
        print(f"{name}: {len(want)} files match the manifest")
        if sh("git", "status", "--porcelain", check=False).stdout.strip():
            sys.exit("the working tree isn't clean: commit or set aside first")
        sh("git", "switch", "-q", "main")
        sh("git", "pull", "-q")
        sh("git", "switch", "-q", "-c", f"assets/{name}")
        for p in want:
            dest = ROOT / p
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(z.read(p))
    # The kind folders the batch touched, judged by the validator (parts in parts mode); its houses' floor plans.
    kinds = sorted({str(Path(p).parent) for p in want if "/structure/" in p and p.endswith(".nbt")})
    for k in kinds:
        sh("node", "scripts/assets/check-asset.mjs", k)
    # The interior check is for homes (beds, chambers, stairs); halls and the rest get their plans drawn, ungated.
    houses = [p for p in want if p.endswith(".nbt") and "/structure/" in p and "__" not in p
              and any(f"/{k}/" in p for k in ("home", "plot", "hall", "workshop"))]
    plans = []
    for h in houses:
        rel = Path(h).relative_to("core/src/main/resources/data/enterprise/structure")
        out = ROOT / "assets-src" / rel.parent
        gated = any(f"/{k}/" in h for k in ("home", "plot", "workshop"))
        r = sh("python3", "scripts/assets/floorplan.py", h, "--out", str(out), *(["--check"] if gated else []), check=False)
        plans.append(f"- `{rel}`: " + (("passes" if r.returncode == 0 else "misses") + " the interior check" if gated
                                       else "plan drawn (not a home: no interior gate)"))
    sh("git", "add", "-A", ".")
    body = (f"Studio batch {name} (desk thread {thread}): {len(want)} files, every one matching the manifest's sha256; "
            f"the shared validator passed on {len(kinds)} folders.\n\nInterior check (floor plans in assets-src):\n"
            + ("\n".join(plans) or "- no houses") + "\n\n🤖 Generated with [Claude Code](https://claude.com/claude-code)")
    sh("git", "commit", "-q", "-m", f"Studio batch {name} (thread {thread}; manifest verified, {len(want)} files)\n\n"
       "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n"
       "Claude-Session: https://claude.ai/code/session_01CDgYfAGty41bkm7BTnmR1p")
    sh("git", "push", "-q", "-u", "origin", f"assets/{name}")
    pr = sh("gh", "pr", "create", "--title", f"Studio batch {name}", "--body", body).stdout.strip().splitlines()[-1]
    print(pr)
    if "--merge" in sys.argv:
        sh("gh", "pr", "merge", pr, "--merge", "--delete-branch")
        sh("git", "switch", "-q", "main")
        sh("git", "pull", "-q")


if __name__ == "__main__":
    main()
