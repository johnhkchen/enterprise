# Alpha smoke run (A4), 2026-10-23

docs/alpha-plan.md section 5, run headless on main at 3d27fcd plus this commit's study fixes.

## Passed

1. **The jar outside the dev run.** NeoForge 21.1.252 installed from the cached installer into a scratch folder, with
   Create 6.0.10, Numismatics 1.1.0 and `enterprise-0.1.0-alpha.1.jar`: `society = core`, "The core is running (232 ms
   to start)", and `/enterprise core version` answered. The jar-in-jar (binding, Chicory) loads outside a dev run.
2. **Version and mint** on the dev server, no mode set: `enterprise 0.1.0-alpha.1 · core enterprise-core 0.1.0 · format 1
   · schema 41c307f6fa0993a8 · society core`; "The core's mint: 16.00 spurs per emerald".
3. **10 days of the test market**, the ledger after each, about 10 s a game day.
4. **The 30-day network study** ran to the end with `--stop-when any.treasury<5` unmet.
5. **The log**: no "The core trapped", "refused a look", "The core is paused", "didn't start", or exception.
6. **Cash books**: nothing unexplained. A town's 64 starting emeralds show as 58: sold to the mint for spurs at its
   first count, and the spurs counted at today's rate.
7. **No negatives** in any ledger, except mood, which may be.
8. **Restart**: the ledger read the same after a restart, but for one chest slot used in the seconds the server ran.
9. **`/enterprise core report`** wrote its folder.
10. **No Java rule ran**: `world/data` holds only `enterprise_names.dat` and `enterprise_terrain.dat`.

## Findings

1. **No caravan left in 30 days (the big one). Fixed in the study, not the core.** Each hub route said, all 30 days,
   "no trip: waiting for a fuller load: 120 times, 0 items". The core was right: the far towns wanted only comforts
   (22 books, worth 3.75 emeralds) and the road costs 6 to 24 emeralds of food. A town wants a good only if its
   switched-on lockers buy it (the wire never sets `Trades.uses`; only the fast host's study does). And the core
   knows a locker only where a *loaded* villager has claimed its workstation with a chest touching it
   (`LockerAdapter`). The study switched trades on 300 ticks after building each town, before any villager had
   claimed one (0 switched on), then unloaded the towns: 3 of 4 never had a locker placed. Java mode never needed
   the placement, so only core mode broke. Proved twice: the day-30 save loaded in the Rust core and run 10 days
   with lockers counted and switched on sent 8 wagons (7,500 items; the hub 64 to 332 emeralds); and live, the
   towns loaded for a minute, `autoswitch`, then unloaded, sent 6 wagons in 8 days. Fix: the study (core mode)
   waits 1,200 ticks, then switches trades on, before it unloads the towns. The 30-day study then ran 19 hub trips
   and sold about 15,000 items; S1-S3 end with moods of +1.2 to +1.4 (were -0.75). `docs/alpha.md` tells players a
   locker needs a chest by its workstation.
   S1 (Meadowworth) on day 30:
   ```
   Meadowworth · day 30 · mood -0.75 · 6 people
   == Goods ==   bread 1728, feather 15, beef 7, chicken 7, porkchop 7, leather 3
   == People ==  6 hands; the Merchant may manage 2 lockers; 0 lockers managed
   == Wagons ==  #0 Haywick → Meadowworth: home: no trip: waiting for a fuller load, 0 trips, 0 sold
   ```
2. **Towns' moods fall** with no trade: S1-S4 to -0.8, T to -0.6 by day 30 (no comforts arrive).
3. **A headless town needs its whole census area loaded** (48 blocks round its bell) or the core never counts its
   people: 0 hands, no Merchant, no lockers. With a player near, view distance covers it. Fixed for the tools:
   CLAUDE.md and the plan's forceload, and the study loads it during setup.
4. **The network study had never run under the core** (fixed here): it set its desks by command in chunks already
   loaded, which the core doesn't see until they load again, so every town was missing ("The core keeps no town at
   1002, -60, 1002") and it stopped on day 1. It now founds each town with `enterprise core stock`.
5. **S4 has no route** at 4,000 blocks: "beyond a leg's food, or no way there". As designed.

## The study, by day

| day | town | treasury | mood | villagers | hexes |
|---|---|---|---|---|---|
| 1 | S1 | 58.0 | -0.15 | 6.0 | 2.0 |
| 1 | S2 | 58.0 | -0.15 | 6.0 | 2.0 |
| 1 | S3 | 58.0 | -0.15 | 6.0 | 2.0 |
| 1 | S4 | 58.0 | -0.15 | 6.0 | 2.0 |
| 1 | T | 64.0 | -0.13 | 5.0 | 1.0 |
| 10 | S1 | 58.0 | -0.68 | 6.0 | 5.0 |
| 10 | S2 | 58.0 | -0.68 | 6.0 | 5.0 |
| 10 | S3 | 58.0 | -0.68 | 6.0 | 5.0 |
| 10 | S4 | 58.0 | -0.68 | 6.0 | 5.0 |
| 10 | T | 64.0 | -0.56 | 5.0 | 4.0 |
| 20 | S1 | 58.0 | -0.74 | 6.0 | 6.0 |
| 20 | S2 | 58.0 | -0.74 | 6.0 | 6.0 |
| 20 | S3 | 58.0 | -0.74 | 6.0 | 6.0 |
| 20 | S4 | 58.0 | -0.74 | 6.0 | 6.0 |
| 20 | T | 64.0 | -0.62 | 5.0 | 6.0 |
| 30 | S1 | 58.0 | -0.75 | 6.0 | 8.0 |
| 30 | S2 | 58.0 | -0.75 | 6.0 | 8.0 |
| 30 | S3 | 58.0 | -0.75 | 6.0 | 8.0 |
| 30 | S4 | 58.0 | -0.75 | 6.0 | 8.0 |
| 30 | T | 64.0 | -0.62 | 5.0 | 7.0 |
