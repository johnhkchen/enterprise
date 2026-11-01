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
2. **The mod's trade:** the profession and its workstation block (`enterprise:chopping_block`, its point of interest),
   the villager's look, its stall in the kingdoms' kits, the host's trade list, recipes (charcoal) and offers. Logs stop
   coming from idle hands in the game here.
3. **Woodlots in the world:** the host lays a town's lots (a ring past its fields, as `FieldMaps` lays farmland: the
   woods' own trees where it has them, saplings on open ground) and shows each lot as the core has it, felled and
   replanted or grown, when its chunk is touched.
4. **Wood's goods:** wooden furnishings, and crates and halls built of planks and logs.

## The core (landing 1)

- `roster::LUMBERJACK`, in the mod's trades after the squad's (not a squad's: it stays home).
- `kit::Kits::woodlots`: each lot by when it stands grown. A town lays as many as its Lumberjacks keep felling while
  they regrow (`lots_felled_per_level_day` × levels × `woodlot_grow_days`, rounded up), as far as its land holds
  (`kit::woodland`: 8 lots a hex of woods or taiga, standing grown, then 3 a hex of open ground, planted as saplings).
- Felling (`rules/kit.rs`, daily, with the hunting and fishing): half a lot a day a level, 16 logs a lot (a Novice's 8
  logs a day, where a hand gathering the woods brought in 6), while the warehouse has room for the logs; a felled lot
  is replanted and grown again 5 days after. Oak, spruce on taiga.
- Charcoal (`workshop::MOD_CONVERSIONS`): a Lumberjack burns 3 logs into 2 charcoal, as the game's furnace does, up to
  `workshop::FUEL_PAR` (16) of fuel with the town's coal counted; every recipe fired with coal has a twin fired with
  charcoal (`services::CHARCOAL_TWINS`: smelting and smoking).
- The job draw (`jobs::woodland_marbles`): a town whose trades use wood (`jobs::USE_WOOD`: a Librarian's shelves, a
  Mason's halls, the smiths' fires, a Butcher's smokehouse), holding fewer than 16 logs and with warehouse room, puts in
  four days of a Novice's felling of its woods unfelled.
- Its offers: it buys saplings and iron, and sells logs and charcoal (`lockers.rs`).

Dials: `lots_felled_per_level_day` (0.5), `logs_per_lot` (16), `woodlot_grow_days` (5).
