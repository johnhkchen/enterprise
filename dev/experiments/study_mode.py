"""Which rules a server runs, as the studies ask it: `society=core` (the core's) or the Java rules'.

Most Java-server studies retire in favour of `sim/host` (John, 2026-10-18; docs/binding-plan.md "Studies"): the ones
that set up and read what is still the Java rules' (`merchant_ramp.py`, `farm_wages.py`, `caravan_network.py`,
`standalone_town.py`) call `java_only` first and exit saying so under `society=core`. `network_study.py` is the one kept
check and runs in both modes.
"""

import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]


def server_is_core():
    """Does the server answer `/enterprise mint` as the core (its words: "The core's mint")? ENTERPRISE_LANE picks a lane."""
    done = subprocess.run([sys.executable, str(REPO / "scripts" / "rcon.py"), "enterprise mint"], env=dict(os.environ),
                          capture_output=True, text=True)
    return "The core's mint" in done.stdout


def java_only(script):
    """Exits with "Java mode only (retired for sim/host)" if the server runs the core's rules."""
    if server_is_core():
        sys.exit(f"{script}: Java mode only (retired for sim/host); under society=core use `just study` (sim/host) or "
                 f"network_study.py, the kept in-game check")
