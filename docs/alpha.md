# Enterprise alpha 0.1.0-alpha.1

Towns that trade. Villagers keep a market's books, send caravans to the next town, and buy and sell with your coin.
This alpha runs on the new engine (the "core"). Some things aren't in it yet, and it says so when you try them.

**Start a new world.** An alpha world may not open in the next alpha: a newer build will tell you it can't read it and
leave it as it was.

## Try

1. Put a Merchant's Desk by a bell and some beds, and let a villager take the job.
2. Put emeralds and goods in the desk's chests.
3. Give each villager a chest touching its workstation (its locker), then switch its trades on in the Merchant's
   Lockers tab. A caravan only carries what a town's lockers buy (and comforts it's short of).
4. Build a second desk 300 blocks away and send a caravan from the Caravans tab.
5. Put a Numismatics vendor by the desk. After a game day, read the Ledger tab: the town's money, goods, people,
   wagons, papers and the mint.

No Merchant at a desk? `/enterprise ledger <desk>` prints the same lines.

## Not in the alpha yet

- The commands that say "not in the alpha yet" (each says what to use instead).
- Crafted crates: they work as plain storage, but packed goods aren't counted. Keep goods loose in the desk's chests.
- Piers, pack llamas, and settlers starting new villages.
- Metals sold at the port, and renaming a market.

## What to expect

- Emeralds above the town's reserve go to the mint.
- Lockers empty into the desk's chests.
- A villager's own trades can differ from what the Lockers tab shows.

## Judge these while you play

- Lockers emptied into the desk at each look.
- Every emerald above a reserve of 0 sold to the mint at a desk's first count.
- Stopping a route winds it down.
- The census never taking a lost profession.

## Something wrong?

1. Run `/enterprise core report`. It saves a folder and tells you where.
2. Zip that folder with `logs/latest.log` and send both.

`/enterprise core version` says which build you're on.
