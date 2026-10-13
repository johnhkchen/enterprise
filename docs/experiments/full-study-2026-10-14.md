# Full study, 40 days (2026-10-14)

`python3 dev/experiments/network_study.py 40 docs/experiments/full-study-2026-10-14.csv --comforts --river-bridge --inject 20`

Everything on: the hub (one shulker of comforts, finite trade goods) and its four routes; a river through the row of
hexes south of the toll town T with no crossing at all; on day 20 a spring opens that refills nine shulkers of trade
goods, a shulker of comforts and a stack of books every day, with routes to S1, S2 and S3. S4 is out of range.
Day-by-day rows for every town and route are in the CSV.

## Where it settles (final run)

| | S1 | S2 | S3 | T (toll town) |
|---|---|---|---|---|
| Money, day 40 (emeralds) | 57 | 95 | 30 | 132 |
| Mood | +4.0 | +4.1 | +4.1 | -0.6 |
| Comforts met | all four | all four | all four | none |
| Villagers (from 5) | 8 | 7 | 7 | 5 |
| Homes / children | 4 / 3 | 3 / 2 | 3 / 2 | 0 / 0 |
| Hexes | 14 | 12 | 12 | 8 |
| Works | wall | wall | wall | two bridges |

- **Town money is bounded.** Content towns moved between about 20 and 95 emeralds for 40 days; in the earlier study,
  before sinks, they gained about 5 a day without end. Walls (24 emeralds each), cottages (8), children (3) and about
  2.5 emeralds a day of comforts take it.
- **Growth is slow and steady.** A town alternates a home (every bed taken) with a child (a bed free), about 4 days
  each, so it adds a villager about every 8 days when it can pay.
- **T is a toll baron.** It bridged its river hex on day 8 (19 emeralds; its estimate: repaid in under 2 days), the
  only crossing for the roads to S2 and S3, and guarded it at 25%. When the spring's roads arrived it bridged the next
  hex west too (repaid in 14 days), since those roads cross there, and guarded both. It collects about 100 spurs a
  day and is the richest town, but no caravan serves it, so it has no comforts and doesn't grow.

## What the study found and fixed

1. **Caravans brought comforts every other trip.** A town's stocktake is taken right after a caravan sells, when its
   shelves are fullest. Wants now allow for what its villagers will have used by the next caravan's arrival.
2. **A poverty trap.** S1 spent its working capital on comforts, a wall and homes, then couldn't pay for the paper its
   lockers sell, so it earned nothing: 2 emeralds and no trades from day 15. Towns now keep back their trading
   capital (2 days of what their lockers sell) before spending on comforts, homes, children or works.
3. **Trading capital counted the wrong thing.** Counted from what lockers could take, it tied up all of T's money for
   goods no caravan brings, and T never built its bridge. It's now what the lockers actually sell, averaged.
4. **Caravans ate the treats.** Cookies, pies and honey were road food, so long routes' towns never kept any. Comforts
   are no longer eaten as food.

## Still open

- A rich town with no route (T) can't spend on comforts. Towns should commission their own caravans, or hire passing
  ones.
- The census sometimes counts too few villagers (S1 counted 3 or 4 of 6 on some days); the incomplete-census guard
  only catches counts under half.
- Raids, and walls' defence against them.
