# Network study, 2026-10-14

`dev/experiments/network_study.py 40 OUT.csv --inject 20`; data in `network-study-2026-10-14.csv`.

Setup: a hub warehouse (18 shulkers of goods, finite) with caravan routes to S1, S2, S3 (300, 600, 1,200 blocks) and S4
(4,000, refused for food). Each town has a farm, four locker villagers, a Merchant and 64 emeralds. Toll town T sits on
the hub-S3 route. On day 20 a spring opens behind the hub: a node whose warehouse refills every day with 9 shulkers of
goods, with routes to S1, S2 and S3 (its S3 route also crosses T). Everything is unloaded; rounds and caravans load what
they need. 40 days take about a minute of tick warp.

## Days 1-19: a finite hub

- The hub sells about 435 items a day across its three routes and brings home about 220 spurs a day.
- It runs out of what S1 trades in by day 10 (S1's deliveries stop at 3,580 items); S2 and S3 follow. Trade then decays.
  **Steady state without production: drain, then stop.**
- Towns gain 0.8-2.1 emeralds a day while supplied, and drift down afterwards (farm wages on produce nobody buys).
- No crop sells, so every farm's labour share sits at the 25% floor.

## Days 20-40: the spring opens

- Spring routes sell about 775 items a day and bring home about 330 spurs a day.
- The hub is out-competed: its S1 caravan sells nothing (the spring, refilled daily and closer to some towns, keeps
  S1 at par) and runs at a loss on road food.
- Towns trade continuously (about 50 trades a day each) and gain 4.9-5.9 emeralds a day each, **with no sign of
  levelling off**: nothing in a town spends its money except wages.
- The mint rate eases from 15.7 to 14.8 spurs per emerald as converted emeralds build up.

## Garrisons and tolls

- Tolls hold at 5% (an unguarded town no longer cuts its rate for lack of income).
- No town ever hires a guard. T's crossing traffic rose from about 45 to 164 spurs a day after the spring opened; one
  more guard on T's 9 hexes would unlock 164 x 5% / 9 = 0.9 spurs a day against a guard's cost of about 10.3. **Guards
  are uneconomic by about 10x** while a toll needs every hex of a town guarded.

## Fixes made during the study

- An unguarded town (which collects nothing) kept cutting its toll and reached 0%, after which no guard could ever look
  worth hiring. It now keeps its rate.
- Caravans with no cargo and no surplus to buy no longer leave. (Caravans with cargo a town is already full of still do;
  see below.)

## What the data points to

1. **Guard chokepoints, not whole territories.** Collect the toll when the hexes a route actually crosses are guarded.
   A guard on T's road hex would then collect 5% of all 164 spurs, about 8 a day, close to its cost; a busier road or a
   higher toll pays for it. This is the "garrison on a hex levies tolls" design in the decisions doc.
2. **A sink for town money.** Towns with steady supply grow their money without limit. Spending it on hexes (the
   decisions doc's "earn hexes with money"), improvements and garrisons would give town money an equilibrium.
3. **Skip doomed trips.** Record a destination's stock when a caravan visits, and don't send cargo it was already full
   of last time.
