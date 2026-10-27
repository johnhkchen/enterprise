# What a fully built town needs: the catalogue

From the economy as the sim runs it today: every mechanic that has, or should have, a building or a site. The file is `docs/assets/catalogue.json` in our repo, and `scripts/assets/factory.py` reads it. It's the checklist each faction is measured against: `factory.py status <faction>` lists what a faction has at each tier, and `factory.py new <faction> --like lordaeron` scaffolds a new faction with a request for everything it lacks, citing the reference faction's builds as examples.

**How to read it.**
- `holds` is ours: the functional rules (beds, workstations, lockers, what must never be inside). The look is yours.
- **Priority:** 1 = needed for a working village, 2 = a full town, 3 = city and capital.
- **from:** the smallest tier that needs it.
- **code:** what our side must build before it can be placed. "today" means it can be placed now.

**The rule that breaks towns.** These blocks are job sites a villager takes:
- barrel, cauldron, composter, lectern, smoker, blast furnace, loom, stonecutter, grindstone, brewing stand, and the smithing, fletching and cartography tables.

A chest touching one of them becomes a shop. So every decorative one must be its `enterprise:decor_*` twin.

Several of today's hall interiors break this rule: the counting house, temple, library, smithy and caravanserai all have job-site blocks as decoration. Your designed halls fix that.

## Homes

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `home` | a household: the census counts its beds; built when every bed is taken and mood holds | 2 beds today (more once C3 reads `beds`); a door to the street; stairs, never ladders, to upper beds; no job site | 5x5 today; 7x7, 9x7, 11x9 bodies with relief (+2) under test (thread 0003) | hamlet | today/C3 | 1 |
| `home:terrace` | a row of households sharing walls, the town's density | 2-3 beds a unit; one door a unit; no job site | units 7 wide x 7 deep, rows filling a ward's 14 | village | C3 | 1 |
| `home:burgage` | a narrow deep plot: house on the street, kitchen garden behind (G2 grid) | 3-8 beds over 2-3 storeys; a ground-floor hall and hearth; may hold its trade's workstation | 9x13, 11x13 plots | village | C3, G2 grid | 1 |

## Trades: stalls and workshops

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `stall:farmer` | the farmer's market stall: hired at once, trades from its locker | exactly one composter (declared workstation); a chest touching it (declared locker); everything else decor twins | 3x3 today; 5x3 market bays | hamlet | today | 1 |
| `workshop:farmer` | the farmer's house-and-workshop: home above, trade below, its yard on the street (signs of trade) | one composter declared; optional declared locker; 2-4 beds above; yard dressing from decor twins | 7x7 to 11x9 bodies | village | C3, workstation from metadata | 2 |
| `stall:librarian` | the librarian's market stall: hired at once, trades from its locker | exactly one lectern (declared workstation); a chest touching it (declared locker); everything else decor twins | 3x3 today; 5x3 market bays | hamlet | today | 1 |
| `workshop:librarian` | the librarian's house-and-workshop: home above, trade below, its yard on the street (signs of trade) | one lectern declared; optional declared locker; 2-4 beds above; yard dressing from decor twins | 7x7 to 11x9 bodies | village | C3, workstation from metadata | 2 |
| `stall:cartographer` | the cartographer's market stall: hired at once, trades from its locker | exactly one cartography_table (declared workstation); a chest touching it (declared locker); everything else decor twins | 3x3 today; 5x3 market bays | hamlet | today | 1 |
| `workshop:cartographer` | the cartographer's house-and-workshop: home above, trade below, its yard on the street (signs of trade) | one cartography_table declared; optional declared locker; 2-4 beds above; yard dressing from decor twins | 7x7 to 11x9 bodies | village | C3, workstation from metadata | 2 |
| `stall:fletcher` | the fletcher's market stall: hired at once, trades from its locker | exactly one fletching_table (declared workstation); a chest touching it (declared locker); everything else decor twins | 3x3 today; 5x3 market bays | hamlet | today | 1 |
| `workshop:fletcher` | the fletcher's house-and-workshop: home above, trade below, its yard on the street (signs of trade) | one fletching_table declared; optional declared locker; 2-4 beds above; yard dressing from decor twins | 7x7 to 11x9 bodies | village | C3, workstation from metadata | 2 |
| `stall:fisherman` | the fisherman's market stall: hired at once, trades from its locker | exactly one barrel (declared workstation); a chest touching it (declared locker); everything else decor twins | 3x3 today; 5x3 market bays | hamlet | today | 1 |
| `workshop:fisherman` | the fisherman's house-and-workshop: home above, trade below, its yard on the street (signs of trade) | one barrel declared; optional declared locker; 2-4 beds above; yard dressing from decor twins | 7x7 to 11x9 bodies | village | C3, workstation from metadata | 2 |
| `stall:shepherd` | the shepherd's market stall: hired at once, trades from its locker | exactly one loom (declared workstation); a chest touching it (declared locker); everything else decor twins | 3x3 today; 5x3 market bays | hamlet | today | 1 |
| `workshop:shepherd` | the shepherd's house-and-workshop: home above, trade below, its yard on the street (signs of trade) | one loom declared; optional declared locker; 2-4 beds above; yard dressing from decor twins | 7x7 to 11x9 bodies | village | C3, workstation from metadata | 2 |
| `stall:cleric` | the cleric's market stall: hired at once, trades from its locker | exactly one brewing_stand (declared workstation); a chest touching it (declared locker); everything else decor twins | 3x3 today; 5x3 market bays | hamlet | today | 1 |
| `workshop:cleric` | the cleric's house-and-workshop: home above, trade below, its yard on the street (signs of trade) | one brewing_stand declared; optional declared locker; 2-4 beds above; yard dressing from decor twins | 7x7 to 11x9 bodies | village | C3, workstation from metadata | 2 |
| `stall:butcher` | the butcher's market stall: hired at once, trades from its locker | exactly one smoker (declared workstation); a chest touching it (declared locker); everything else decor twins | 3x3 today; 5x3 market bays | hamlet | today | 1 |
| `workshop:butcher` | the butcher's house-and-workshop: home above, trade below, its yard on the street (signs of trade) | one smoker declared; optional declared locker; 2-4 beds above; yard dressing from decor twins | 7x7 to 11x9 bodies | village | C3, workstation from metadata | 2 |
| `stall:leatherworker` | the leatherworker's market stall: hired at once, trades from its locker | exactly one cauldron (declared workstation); a chest touching it (declared locker); everything else decor twins | 3x3 today; 5x3 market bays | hamlet | today | 1 |
| `workshop:leatherworker` | the leatherworker's house-and-workshop: home above, trade below, its yard on the street (signs of trade) | one cauldron declared; optional declared locker; 2-4 beds above; yard dressing from decor twins | 7x7 to 11x9 bodies | village | C3, workstation from metadata | 2 |
| `stall:mason` | the mason's market stall: hired at once, trades from its locker | exactly one stonecutter (declared workstation); a chest touching it (declared locker); everything else decor twins | 3x3 today; 5x3 market bays | hamlet | today | 1 |
| `workshop:mason` | the mason's house-and-workshop: home above, trade below, its yard on the street (signs of trade) | one stonecutter declared; optional declared locker; 2-4 beds above; yard dressing from decor twins | 7x7 to 11x9 bodies | village | C3, workstation from metadata | 2 |
| `stall:armorer` | the armorer's market stall: hired at once, trades from its locker | exactly one blast_furnace (declared workstation); a chest touching it (declared locker); everything else decor twins | 3x3 today; 5x3 market bays | hamlet | today | 1 |
| `workshop:armorer` | the armorer's house-and-workshop: home above, trade below, its yard on the street (signs of trade) | one blast_furnace declared; optional declared locker; 2-4 beds above; yard dressing from decor twins | 7x7 to 11x9 bodies | village | C3, workstation from metadata | 2 |
| `stall:toolsmith` | the toolsmith's market stall: hired at once, trades from its locker | exactly one smithing_table (declared workstation); a chest touching it (declared locker); everything else decor twins | 3x3 today; 5x3 market bays | hamlet | today | 1 |
| `workshop:toolsmith` | the toolsmith's house-and-workshop: home above, trade below, its yard on the street (signs of trade) | one smithing_table declared; optional declared locker; 2-4 beds above; yard dressing from decor twins | 7x7 to 11x9 bodies | village | C3, workstation from metadata | 2 |
| `stall:weaponsmith` | the weaponsmith's market stall: hired at once, trades from its locker | exactly one grindstone (declared workstation); a chest touching it (declared locker); everything else decor twins | 3x3 today; 5x3 market bays | hamlet | today | 1 |
| `workshop:weaponsmith` | the weaponsmith's house-and-workshop: home above, trade below, its yard on the street (signs of trade) | one grindstone declared; optional declared locker; 2-4 beds above; yard dressing from decor twins | 7x7 to 11x9 bodies | village | C3, workstation from metadata | 2 |

## Markets

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `desk` | the Merchant's Desk and its warehouse: the town's stock is the chests touching the desk | a marker for the desk (mod block); many chests touching it (warehouses jammed with one); room for a Blaze Banker and up to 3 vendors touching it; no job site | 9x9 to 13x13 market hall at the forum | hamlet | C1 markers | 1 |

## Halls (the sim builds these in order: square, granary, root cellar, counting house, temple, library, smithy, watchtower, stone docks, caravanserai)

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `hall:square` | Masons' Square: +0.5 mood, flower beds with a master Farmer | open paving, lamps at the corners; must stay open ground | 9x9x1 today (taller is free; wider needs C4) | hamlet | today | 1 |
| `hall:granary` | bulk wheat as hay, up to 36,864 | hay inside; no chest touching a job site | 7x7x4 today (taller is free; wider needs C4) | village | today | 1 |
| `hall:root_cellar` | roots in store, up to 27,648 | decor barrels (never vanilla barrels) | 5x5x3 today (taller is free; wider needs C4) | village | today | 1 |
| `hall:counting_house` | the market's paperwork: halves stationery, doubles discounting | decor lectern and casks unless declared; desks, shelves | 7x7x4 today (taller is free; wider needs C4) | village | today | 1 |
| `hall:temple` | blessings count double, children twice as often | decor brewing stand unless declared; candles, an altar | 7x7x4 today (taller is free; wider needs C4) | village | today | 1 |
| `hall:library` | +3 culture a day | bookshelves; decor lectern unless declared | 7x7x4 today (taller is free; wider needs C4) | village | today | 1 |
| `hall:smithy` | mine yields half again; kit lasts longer | decor smithing table and blast furnace unless declared; anvil, quench | 7x7x4 today (taller is free; wider needs C4) | village | today | 1 |
| `hall:watchtower` | half the guards count as armed | stairs to a lit top; no ladders on a villager's way | 3x3x7 today (taller is free; wider needs C4) | village | today | 1 |
| `hall:stone_docks` | doubles the catch | on the shore within 32 of the bell; a quay to the water | 3 wide, out over water today (taller is free; wider needs C4) | village | today | 1 |
| `hall:caravanserai` | +1 culture a day; the town is known 800 blocks round | animal stalls, stores, a court; beds only if we agree (they count as homes) | 9x9x4 today (taller is free; wider needs C4) | village | today | 1 |

## Civic

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `forum:centre` | the bell's setting: the town's meeting place | the bell as a marker (it exists already); lamps; never a bed or job site | fits the 13x13 forum round the bell | hamlet | C6 | 1 |
| `forum:dressing` | the square dressed: stalls round it, trees, benches, a well | stall slots left as markers; decor twins | the forum ring | hamlet | C6 | 2 |
| `prop` | street furniture and planting: lamps, benches, wells, troughs, signs, trees, hedges | no POI blocks | 1x1 to 3x3 | hamlet | C1 | 2 |

## Walls

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `wall:curtain` | the town wall's curtain: raised a quarter at a time, mood and culture | walkable gate passages 3 wide on the avenues; stairs to wall walks | as the wall kit (README: corner 7x7, gatehouse 11x7, curtains on the grid) | hamlet | C5 | 1 |
| `wall:tower` | the town wall's tower: raised a quarter at a time, mood and culture | walkable gate passages 3 wide on the avenues; stairs to wall walks | as the wall kit (README: corner 7x7, gatehouse 11x7, curtains on the grid) | hamlet | C5 | 1 |
| `wall:corner` | the town wall's corner: raised a quarter at a time, mood and culture | walkable gate passages 3 wide on the avenues; stairs to wall walks | as the wall kit (README: corner 7x7, gatehouse 11x7, curtains on the grid) | hamlet | C5 | 1 |
| `wall:gatehouse` | the town wall's gatehouse: raised a quarter at a time, mood and culture | walkable gate passages 3 wide on the avenues; stairs to wall walks | as the wall kit (README: corner 7x7, gatehouse 11x7, curtains on the grid) | hamlet | C5 | 1 |
| `wall:filler` | the town wall's filler: raised a quarter at a time, mood and culture | walkable gate passages 3 wide on the avenues; stairs to wall walks | as the wall kit (README: corner 7x7, gatehouse 11x7, curtains on the grid) | hamlet | C5 | 1 |
| `wall:step` | the town wall's step: raised a quarter at a time, mood and culture | walkable gate passages 3 wide on the avenues; stairs to wall walks | as the wall kit (README: corner 7x7, gatehouse 11x7, curtains on the grid) | hamlet | C5 | 1 |
| `wall:palisade` | a hamlet's or village's wall | log palisade; timber gate | as the stone kit | hamlet | C5 | 2 |

## Farming

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `farmstead` | a holding's farm buildings at the head of its first field: barn, yard | no bed unless declared; a composter only if declared for a Farmer | 9x9, 9x13 | village | C10 | 2 |
| `barn` | a field barn among the strips | hay; no POI | 5x7 to 7x9 | village | C10 | 3 |
| `mill` | a windmill or watermill: grain to flour (bread) | no POI; a watermill on a stream | 7x7 to 9x9 | town | C10 | 3 |

## Animals

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `pasture:shelter` | a pasture's shelter and trough | inside a birch-fenced pasture; open to the animals | 5x3, 5x5 | hamlet | C10 | 2 |
| `stable` | the Merchant's llama train and wagons' teams | stalls for 2-6 llamas; hay; a hitching rail; no bed | 9x7, 11x9 | village | C10, llama entities | 2 |

## Caravans, roads and post

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `wagon_yard` | where wagons load and leave: by the desk | a loading dock against the warehouse; open ground for a wagon | 9x9 to 13x9 | village | planned | 2 |
| `waystation` | a roadside stop on a caravan road between towns | a trough, a shelter; no bed unless agreed | 7x7 | village | planned | 3 |
| `toll_house` | the guard's post where a road is tolled (the core picks the hexes) | a gate across the road 3 wide; a guard's room | 7x5 astride a 3-wide road | village | planned | 2 |
| `post_office` | riders carry contracts, bills and letters | a counter; pigeonholes; a stable stall | 7x7 | town | planned | 3 |
| `bridge:span` | a river bridge for wagons (and toll bridges) | a 3-wide deck with rails; spans of 4 to 16 | straight runs (C12) | hamlet | C12 | 2 |
| `bridge:head` | a bridge's ends: a gate or toll | the deck's 3 wide passes through | 7x7 | village | C12 | 3 |

## Sea

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `dock:pier` | a fisherman's pier: cod landed daily | out over water from the shore | 3 wide, 4-12 long | hamlet | C11 | 2 |
| `dock:quay` | a stone quay along a shore: stone docks, the capital's port | follows the shoreline; bollards, cranes as decor | runs along a shore | town | C11 | 2 |
| `customs_house` | realm trade over the sea: the crown's customs | a counter; stores; by the quay | 9x9 | city | planned | 3 |

## Money

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `mint` | the capital's mint: the spur's rate | strongroom; decor; no POI | 11x11 | capital | planned | 3 |
| `bank` | discounting and lending against contracts | counters; a vault | 9x9 to 11x11 | city | planned | 3 |
| `exchange` | rent and forward crop deeds traded at hubs | a hall with counters | 11x9 | city | planned | 3 |

## Mining and materials

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `mine:head` | a mine shaft's head: the Toolsmith's iron and coal | the shaft's 1x1 or 2x2 opening with stairs or a winch (no ladders on a villager's way) | 5x5 headframe | village | C10 | 2 |
| `quarry` | the masons' stone, 16 blocks a day per hex | an open cut with ramps | free | village | planned | 3 |
| `woodyard` | hands' gathering: logs | a saw pit, log stacks | 7x7 | village | planned | 3 |

## Comforts (today they come straight from the warehouse, with no building)

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `comfort:treats` | where the town's treats come from: the bakery or confectioner | a shop front; decor twins; beds above only if agreed | 7x7 | village | planned (comforts come straight from the warehouse today) | 2 |
| `comfort:reading` | where the town's reading come from: the reading room or bookseller | a shop front; decor twins; beds above only if agreed | 7x7 | village | planned (comforts come straight from the warehouse today) | 2 |
| `comfort:clothing` | where the town's clothing come from: the tailor | a shop front; decor twins; beds above only if agreed | 7x7 | village | planned (comforts come straight from the warehouse today) | 2 |
| `comfort:furnishings` | where the town's furnishings come from: the chandler and upholsterer | a shop front; decor twins; beds above only if agreed | 7x7 | village | planned (comforts come straight from the warehouse today) | 2 |
| `comfort:leisure` | where the town's leisure come from: the tavern and tobacconist | a shop front; decor twins; beds above only if agreed | 9x9 to 11x9 | village | planned (comforts come straight from the warehouse today) | 2 |

## War

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `barracks` | recruits wait for a caravan to the front; a depot today | beds (they count: agree a number); chests inside | 11x9 to 13x11 | town | planned | 3 |
| `armoury` | war goods held: swords, chestplates, bows | chests inside (a depot); no POI | 9x7 | town | planned | 3 |

## Settlers

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `founding_kit` | a new town's first buildings, as settlers lay them: bell, desk and chest, a Farmer's stall, two cottages | passes the census: a bell, 3+ beds, 3+ workstations within 48 | composed round a 13x13 forum | hamlet | planned | 2 |

## Landmarks

| Slot | For | Holds | Footprints | From | Code | P |
|---|---|---|---|---|---|---|
| `landmark:abbey` | a town's or city's great church | parts within lots of merged wards | 32x32, 32x50 lots | town | C8, C9 | 2 |
| `landmark:keep` | a lord's keep or castle | parts | 32x32 | city | C8, C9 | 3 |
| `landmark:palace` | a capital's palace or yamen | parts | 32x50 | capital | C8, C9 | 3 |

## What I'd ask for next, after the Lordaeron houses
1. **Priority 1, Lordaeron, village tier:**
   - the 13 trade stalls as one stall set (a frame, an awning and goods per trade, its workstation and locker declared);
   - the desk's market hall;
   - the 10 halls redesigned with twins;
   - the forum centrepiece;
   - the stone wall kit.

   Most can share the timber set's parts.
2. **Workshops, one per trade:** house above, trade below, with a yard showing the trade. This is where a street gets its character.
3. **The comfort shops** (bakery, tailor, reading room, chandler, tavern) and the farm, animal and road buildings. These need small code on our side first; I'll land it as you go.

