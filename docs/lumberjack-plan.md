# The Lumberjack

The mod's first trade of its own (John, 2026-11-01: "a big push on our first added profession: lumberjack"). Settled
with John the same day:

- **Woodlots, felled visibly.** A town lays woodlots, as its farmland, and its Lumberjacks fell the grown ones and
  replant them. The core keeps each lot's growth (compute on touch: a lot is a number, when its trees stand grown); the
  game's blocks are its view.
- **Wood feeds charcoal, builds and crates, and furnishings.**
- **Logs are the Lumberjack's only.** Hands without a trade gather sticks, apples and string in the woods, no logs, once
  the host has the trade.
- **Placeholder art now, the asset studio's later.** A chopping block and the villager's clothes from the game's own
  textures; the studio's, for John's approval, replace them.

## Landings

1. **Core** (built): the trade, woodlots and felling, charcoal, the job draw. Below.
2. **The mod's trade** (built): the profession and its workstation block (`enterprise:chopping_block`, its point of
   interest, a stump of the game's oak textures), the villager's look (the Merchant's cloak in red plaid), its plain
   stall (a chopping block and a chest; no kingdom kit has one yet), the host's trade list and its recipes: charcoal,
   and a chest of two logs, a home's furniture (`comforts/furnishings`), which gives a town short of furnishings a
   reason to draw one (logs alone are worth too little: seed 1717's 30 days drew none for its woods). Logs stop coming
   from idle hands in the game here.
3. **Woodlots in the world** (built): the core tells each lot's growth with the town's land (`Answer::Land`'s
   `woodlots`); the host (`territory/Woodlots`) lays each new lot on the nearest free 7 x 7 of the land's own ground
   in a ring past the pastures (32 deep; 24 blocks out for a town with no plan) and shows it: grown, four trees (the
   land's own where they stood); felled, logs and leaves gone and four saplings. It loads nothing for them (John: "We
   don't need to be constantly loading such chunks"): a lot is laid, felled or grown when its ground is loaded anyway,
   by a player near it, and caught up then. Headless studies load none of that ring, so their lots stay unlaid.
4. **Its trades and its wood** (built; John, 2026-11-02: "Crates can be made out of emeralds, 6 per emerald at a
   lumberjack as a trade. You can also trade 10-20 lumber into 1 emerald with the lumberjack, and they'll grow the
   type of lumber they like"): a Lumberjack sells crates (`enterprise:crate`, a good now) six to the emerald and buys
   logs, the wood its town's Lumberjacks like ten to the emerald and any other twenty (`Lockers::like_wood`). The wood
   is rolled with the town's dice at its first felling from the logs the host has (`kit::Kits::wood`), its woodlots
   fell it, and the host grows its tree (`Answer::Land`'s `wood`; `Woodlots.wood`: oak where a single sapling grows
   none). Halls already take timber.
5. **Crates made at home, traded between towns** (built; John: "replace that mechanic with local carpenters and
   local regional crate trade"): no crate is bought abroad. A town's Merchant buys its crates of its Lumberjack, as
   many as it keeps empty and as many again where a neighbour bids for them (`rules/lockers.rs`); a town with no
   Lumberjack bids for them at up to twice their worth (`crate_ceiling`, `rules/market.rs`); short of crates, goods
   ride loose.

## Timber (John: "gate building creation behind lumberjacks")

A home takes 16 logs and a hall 32 where the host has the trade; a town waiting on timber signs a Lumberjack on first
(retraining its least-trained hand if none is idle), keeps 48 logs from its workshop, and fells on to 64. Every land
but water holds a woodlot or more.

## The core (landing 1)

- `roster::LUMBERJACK`, in the mod's trades after the squad's (not a squad's: it stays home).
- `kit::Kits::woodlots`: each lot by when it stands grown. A town lays as many as its Lumberjacks keep felling while
  they regrow (`lots_felled_per_level_day` × levels × `woodlot_grow_days`, rounded up), as far as its land holds
  (`kit::woodland`: 8 lots a hex of woods or taiga, standing grown, then 3 a hex of open ground, planted as saplings).
- Felling (`rules/kit.rs`, daily, with the hunting and fishing): half a lot a day a level, 16 logs a lot (a Novice's 8
  logs a day, where a hand gathering the woods brought in 6), while the warehouse has room for the logs; a felled lot
  is replanted and grown again 5 days after, of the wood its Lumberjacks like (landing 4).
- Charcoal (`workshop::MOD_CONVERSIONS`): a Lumberjack burns 3 logs into 2 charcoal, as the game's furnace does, up to
  `workshop::FUEL_PAR` (16) of fuel with the town's coal counted; every recipe fired with coal has a twin fired with
  charcoal (`services::CHARCOAL_TWINS`: smelting and smoking).
- The job draw (`jobs::woodland_marbles`): a town whose trades use wood (`jobs::USE_WOOD`: a Librarian's shelves, a
  Mason's halls, the smiths' fires, a Butcher's smokehouse), holding fewer than 16 logs and with warehouse room, puts in
  four days of a Novice's felling of its woods unfelled.
- Its offers: it buys logs (landing 4), saplings and iron, and sells crates, logs and charcoal (`lockers.rs`).

Dials: `lots_felled_per_level_day` (0.5), `logs_per_lot` (16), `woodlot_grow_days` (5).
