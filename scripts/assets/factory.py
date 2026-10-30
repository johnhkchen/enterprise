#!/usr/bin/env python3
"""The workflow factory for factions' building sets (John: "once a major kit is made, make it so new factions can have
similar work done by leveraging the source controlled aspect of our workflow factory").

    python3 scripts/assets/factory.py status [FACTION]          what each faction has of the catalogue, and what's missing
    python3 scripts/assets/factory.py new FACTION --name "Name" --like REFERENCE [--inspiration "..."]
    python3 scripts/assets/factory.py request FACTION [--tier village]   a request to the studio for what it lacks

The catalogue (`docs/assets/catalogue.json`) is everything a fully built town needs, by slot: what each is for in the
economy, what it must hold (beds, a workstation, a locker), its footprints and tiers, and the code it needs. A faction
is a kingdom's folder of structures (`core/src/main/resources/data/enterprise/structure/<faction>/`), its style
(`assets-src/kingdoms/<faction>/`) and its rules (`town_kits/`, `town_rules/`). A new faction starts from a reference
faction's finished work (`--like lordaeron`): its style sheet as the template to rewrite, and a request listing every
catalogue slot with the reference's structure for it as the example, so the studio repeats a proven set in a new
language rather than starting over. Work for a faction goes on a branch `faction/<faction>` and lands by pull request.
"""
import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CATALOGUE = ROOT / "docs/assets/catalogue.json"
STRUCTURES = ROOT / "core/src/main/resources/data/enterprise/structure"
KINGDOMS = ROOT / "assets-src/kingdoms"
REQUESTS = ROOT / "docs/assets"
TIERS = ["hamlet", "village", "town", "city", "capital"]


def catalogue():
    return json.loads(CATALOGUE.read_text())


def kits(faction):
    """A faction's structures' metadata, by slot."""
    out = {}
    for meta in sorted((STRUCTURES / faction).glob("**/*.json")):
        try:
            m = json.loads(meta.read_text())
        except ValueError:
            continue
        # A part with no slot fills its kind's (the kit races' homes, forums, halls: thread 0015).
        slot = m.get("slot") or m.get("kind")
        if slot:
            out.setdefault(slot, []).append(m)
    return out


def factions():
    found = {p.name for p in STRUCTURES.iterdir() if p.is_dir() and p.name != "test"} if STRUCTURES.exists() else set()
    found |= {p.name for p in KINGDOMS.iterdir() if p.is_dir()} if KINGDOMS.exists() else set()
    return sorted(found)


def covers(entry, have, tier):
    """Has the faction a structure for the entry's slot at this tier (any tier if the structure names none)?"""
    # A stall for any trade (`stall:any`, the trade's job block at its marker) is every trade's stall.
    any_stall = have.get("stall:any", []) if entry["slot"].startswith("stall:") else []
    for m in have.get(entry["slot"], []) + any_stall:
        # A building from its first tier on: a town's forge still stands when it's a city.
        if not m.get("tiers") or TIERS.index(tier) >= min(TIERS.index(t) for t in m["tiers"] if t in TIERS):
            return True
    return False


def needed(entry, tier):
    return TIERS.index(entry.get("from", "hamlet")) <= TIERS.index(tier)


def status(args):
    cat = catalogue()
    for faction in [args.faction] if args.faction else factions():
        have = kits(faction)
        print(f"== {faction}")
        for tier in TIERS:
            want = [e for e in cat["slots"] if needed(e, tier)]
            done = [e for e in want if covers(e, have, tier)]
            missing = [e["slot"] for e in want if not covers(e, have, tier)]
            print(f"  {tier:8} {len(done):3} of {len(want):3}" + (f"   missing: {', '.join(missing[:12])}"
                                                                    + (" ..." if len(missing) > 12 else "") if missing else ""))


def new(args):
    faction = args.faction
    style = KINGDOMS / faction
    if style.exists():
        sys.exit(f"{style} exists")
    style.mkdir(parents=True)
    ref = KINGDOMS / args.like / "style.md"
    template = ref.read_text() if ref.exists() else "# Style sheet\n"
    (style / "faction.json").write_text(json.dumps({
        "id": faction, "name": args.name, "like": args.like, "inspiration": args.inspiration or "",
        "status": "requested"}, indent=2) + "\n")
    (style / "style.md").write_text(
        f"# {args.name}: style sheet (to be written by the studio)\n\n"
        f"Inspiration: {args.inspiration or '(to agree)'}. Reference faction: {args.like}; its sheet follows as the shape "
        f"this one should take, not its content.\n\n---\n\n" + template)
    print(f"wrote {style}; branch: git switch -c faction/{faction}")
    request(argparse.Namespace(faction=faction, tier=args.tier, like=args.like))


def request(args):
    cat = catalogue()
    faction = args.faction
    meta = KINGDOMS / faction / "faction.json"
    info = json.loads(meta.read_text()) if meta.exists() else {"name": faction, "like": getattr(args, "like", None) or "lordaeron"}
    like = getattr(args, "like", None) or info.get("like")
    have, ref = kits(faction), kits(like) if like else {}
    lines = [f"# Asset request: {info.get('name', faction)} ({faction}), to the {args.tier} tier", "",
             f"Generated by `scripts/assets/factory.py request {faction}` from `docs/assets/catalogue.json`.",
             f"Inspiration: {info.get('inspiration') or '(to agree)'}. Reference faction: {like or '(none)'}: "
             "repeat its proven set in this faction's language; the contract (`docs/assets/README.md`) holds as ever.",
             "", "| Slot | For | Must hold | Footprints | Reference |", "|---|---|---|---|---|"]
    for e in cat["slots"]:
        if not needed(e, args.tier) or covers(e, have, args.tier):
            continue
        refs = ", ".join(m["id"] for m in ref.get(e["slot"], [])[:2]) or "-"
        lines.append(f"| `{e['slot']}` | {e['for']} | {e.get('holds', '-')} | {e.get('footprints', '-')} | {refs} |")
    out = REQUESTS / f"request-{faction}-{args.tier}.md"
    out.write_text("\n".join(lines) + "\n")
    print(f"wrote {out}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("status")
    s.add_argument("faction", nargs="?")
    n = sub.add_parser("new")
    n.add_argument("faction")
    n.add_argument("--name", required=True)
    n.add_argument("--like", default="lordaeron")
    n.add_argument("--inspiration")
    n.add_argument("--tier", default="village", choices=TIERS)
    r = sub.add_parser("request")
    r.add_argument("faction")
    r.add_argument("--tier", default="village", choices=TIERS)
    a = ap.parse_args()
    {"status": status, "new": new, "request": request}[a.cmd](a)


if __name__ == "__main__":
    main()
