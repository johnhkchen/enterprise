# Asset request: the first kingdoms

From Enterprise, a Minecraft 1.21.1 (NeoForge) modpack where villager towns grow, trade by caravan and wall themselves
in. Request 001. The contract this request relies on is in `docs/assets/README.md`: who owns what, file layout,
naming, the villager rules, masses, modular parts, metadata, the GIS export, and how we review. Read that first.

- **What you need, and what it's for:** towns with character. See "Character first" and "What we're asking for".
- **Style or setting:** seven human kingdoms, each a WoW kingdom crossed with a real culture. See "The kingdoms".
- **Minecraft version and loader:** 1.21.1, NeoForge, vanilla `.nbt` structures (DataVersion 3955), vanilla blocks
  only.
- **How the mod will place or use the structures:** procedurally, as the sim grows each town, on a plan our code
  already lays out. Mostly assembled from your parts by your rules, the rest detailed from our masses. See "How our
  towns are laid out today" and "What can be placed today".
- **Anything else:** a pilot town, a small first batch, the catalogue with priorities and budget, questions for you.

## Character first

Today our villagers live in wood boxes: a 5 x 5 cottage of planks and cobblestone with a slab roof, the same one every
time. We want towns with serious character and detail:
- **Homes that look lived in:** kitchen gardens, window boxes, a woodpile by the door, washing between houses, a
  bench where someone sits of an evening.
- **Signs of trade by profession:** you can tell where the smith lives from the anvil and quenching trough in the
  yard. The same goes for:
  - the farmer's bake oven (our Farmers bake the town's bread);
  - the butcher's smokehouse;
  - the shepherd's dye vats and fleeces drying;
  - the leatherworker's tanning racks;
  - the fisherman's nets and boats drawn up;
  - the mason's stone yard;
  - the cleric's herb garden.
- **Streets dressed at every tier:** a hamlet's beaten paths and hedges, a village's well and market stalls, a town's
  planted avenues and lamps, a city's paved squares, statues and fountains.
- **A town that changes as it grows,** so a player coming back after twenty days sees it has prospered.

**The split:** we know what villager life needs, and the README states it. That covers beds, workstations, doors,
the blocks the game treats as homes and job sites, walkable paths and headroom, light against mobs, and how the
census counts a town. You own everything about how it looks.

## What we know you can do

John sent us your samples (`docs/assets/pipeline-samples/sample-1..6.jpg`):
- a whole city seen from a forested hill;
- a downtown of distinct towers at dusk;
- Notre-Dame with its flying buttresses over a stone river quay with lamps;
- the National Mall's lawns and tree-lined walks;
- Mont-Saint-Michel on its island with the causeway and polders;
- the Capitol with its reflecting pool down a planned axis.

That's the bar. You take real terrain and real city plans in, can paint concept art and build from it, and can turn
simple masses into detailed builds. Please don't read this request as an order for cottages.

## What we're asking for

1. **Concept art first.** For each kingdom and tier: a town view from the road coming in, a street scene, the forum,
   one landmark, and a page of notes. John approves it before any build, and it's the reference every review checks
   against.
2. **Design the kingdoms.** Each kingdom's architectural language (a style sheet, named palettes, its trees and
   planting) and the composition rules: how a ward fills, how a forum dresses, how a road is planted, how a town reads
   at each tier.
3. **Modular part sets, not one-off buildings.** Parts that snap together on a grid:
   - facade bays and corner posts;
   - roof modules, gable ends, dormers and chimneys;
   - porches and outside stairs;
   - wall and tower segments;
   - trade yards;
   - street furniture.

   Add assembly rules our code runs to build homes, halls and terraces of any width, depth and height from them.
   Variants come from parameters (palette, roof, storeys, width in bays, shop front or not), not from extra files.
   The README has the grid and the formats.
4. **Detail our masses.** Where a plot is an odd shape or a size no set covers, our code sends a mass. That's a few
   boxes for the envelope and the roof line, with the door, beds and workstation already fixed. You return a detailed
   build that keeps exactly those blocks where they are. The README specifies the mass format.
5. **Design for our plans.** We export each town's plan as GIS data you already read: GeoJSON, height and surface
   rasters, and the town's state in the sim (kingdom, size, trades, wealth). You return layouts that fit its plots,
   and the rules that would do the same for any town.
6. **Own the jobs you're better at, and hand us the rules.** Laying out a ward, dressing a square, landscaping an
   avenue, running a quay along a shore: you design them and give us data or rules that reproduce them on any town the
   sim grows. Our code places; you decide what good looks like.

We'll start small and end to end (one kingdom, one real town: "First batch"), then go big ("The catalogue").

## How our towns are laid out today

All of this is live code (`core/src/main/java/dev/b28/enterprise/territory/`); sizes are exact.

- **A town** is a bell, beds and workstations within 48 blocks: a vanilla village, or one our settlers found. The
  sim counts its villagers and grows it:
  - homes when every bed is taken;
  - children when a bed is free;
  - halls when it can afford them;
  - walls when it's content.
- **The forum** (`Forums.java`): a 13 x 13 paved square round the bell, laid on the land's own ground at the town's
  first survey. The four avenues leave from its edges.
- **The town plan** (`TownPlans.java`, modelled on ancient Chinese walled cities at John's choice):
  - **Inner wall:** a rectangle round the village (its beds, workstations and bell plus 5 blocks, at least 16 each way
    from the bell). Live towns' inner walls run 65 to 95 blocks a side.
  - **Outer wall:** twice the inner wall's reach from the bell (120 at most). Both walls are square to the compass.
  - **Avenues:** four, 3 wide, running from the forum north, south, east and west to the outer wall, through 3-wide
    gates in both walls. They follow the bell's lines, so a gate is rarely in the middle of its side.
  - **Kept clear:** 3 blocks either side of each wall's line, and along each avenue.
- **Lanes and wards:** inside the inner wall, lanes 2 wide every 18 blocks, parallel to the avenues. The ground
  between them makes 14 x 14 cells (13 beside an avenue), with part-cells against the wall.
- **Homes** (`Growth.java`): 5 x 5 cottages with 2 beds, fronting a lane or avenue, door turned to the street, one
  block apart, nearest the bell first. A town without a plan puts them on the nearest flat open ground 8 to 32 from the
  bell.
- **Halls** (`Buildings.java`, priced in the sim's `works.rs`): square, door south, on the nearest open ground 8 to
  32 from the bell:

  | Hall | Footprint |
  |---|---|
  | Masons' Square | 9 x 9 |
  | granary | 7 x 7 |
  | root cellar | 5 x 5 |
  | counting house | 7 x 7 |
  | temple | 7 x 7 |
  | library | 7 x 7 |
  | smithy | 7 x 7 |
  | watchtower | 3 x 3, 7 high |
  | stone docks | a stone pier |
  | caravanserai | 9 x 9 |
- **Stalls** (`Jobs.java`): a 3 x 3 plot with a trade's workstation and a chest against it. The chest makes it a shop
  the Merchant runs.
- **The wall** (`Works.java`): today a 3-high run of logs, sandstone or cobblestone on the inner wall's line. It goes
  up a quarter at a time as the town's masons are paid, with gaps where the ground is a path or a field.
- **Farmland** (`FieldMaps.java`, `Fields.java`):
  - gardens between the two walls, open fields beyond the outer wall;
  - laid along farm tracks that branch off the avenues and roads;
  - each track has holdings either side: Voronoi parcels round seeds every 24 blocks;
  - holdings are farmed in strips square to the compass: fields 9 x 9 with a ditch down the middle, a 1-block balk
    between strips, up to 4 fields long, starting 3 back from the track.
- **Pastures** (`Pastures.java`): the largest rectangle of open grass (18 x 14 down to 7 x 7) in a ring beyond the
  grain, fenced in birch, with a gate toward the bell.
- **Docks** (`Docks.java`): a 4-block plank pier into water within 32 of the bell, with a lantern at its end.
- **Roads** (`Trails.java`): planned over the real ground between towns (wagons climb 1 in 12 at most, llama trains
  1 in 3), trodden to path, and decked in planks where they cross water. Bridges over wide water (`Works.java`) are a
  5-wide stone or timber deck.
- **Names** for now are WoW's human settlements (Goldshire, Lakeshire, Hearthglen, Southshore...). There's no kingdom
  or tier in the code yet; we'll add both (C2 in the README).

**Hearthglen** (town 0 of our seed-8675309 study world), from the current run:
- bell at (-309, 78, -321);
- inner wall x -364 to -269, z -359 to -300 (96 x 60);
- outer wall x -419 to -229, z -397 to -279 (191 x 119).

The bell is 55 blocks from the west wall and 40 from the east, 38 from the north and 21 from the south. The plan is
lopsided because the village was.

## What can be placed today, and what needs our code

Every structure needs our placer first (C1: load, turn, level, fill markers). That's one piece of work on our side.
The part-set assembler (C16) and masses from our plans (C15) are the next two. Beyond those:

| Fits today's siting | Needs our code first |
|---|---|
| Homes 5 x 5, 2 beds (assembled or whole) | Homes of other sizes, terraces, corner houses (C3); more than 2 beds (C3 + the core) |
| Stalls 3 x 3, trade yards beside them | Stalls round the forum (C6) |
| Halls at today's footprints (within them, any shape, any height) | Bigger halls (C4), landmarks and their lots (C8, C9) |
| Detailed builds from masses of those sizes | Forum centrepiece and dressing (C6) |
| | Wall kits on the inner wall; the outer wall at all (C5) |
| | Ward blocks laid out whole (C8); upgrading a building to its next tier (C7) |
| | Farmsteads, field barns, mills, pasture shelters, mine heads (C10) |
| | Quays along a shore, harbours (C11); bridges from a kit (C12) |
| | Roadside waystations and toll houses (the sim has toll posts; showing them in the world is new) |

The first batch keeps mostly to the left column. It adds the wall kit and the forum's well house because C5 and C6
are small. Everything on the right is in the catalogue, and we'll write the code as the kits arrive.

## The kingdoms

A first set of seven. Each is a WoW human kingdom paired with a real culture that fits its character, so you have
both a mood and real buildings to work from. The pairings are proposals: push back where you see a better one.

| Kingdom | Paired with | Why |
|---|---|---|
| **Stormwind** (Elwynn, Westfall, Redridge, Duskwood) | Norman and early Gothic England and northern France: Caen-white stone, blue slate, Romanesque keeps, a Notre-Dame-scale cathedral; timber-and-plaster villages (Goldshire) | Stormwind is the white-stone, blue-roofed crown city with a cathedral at its heart; Elwynn is English countryside. Your Notre-Dame and its quay are the right register for its capital. |
| **Lordaeron** (Hearthglen, Andorhal, Brill, Tarren Mill, Southshore) | Tudor and late-medieval England: timber-framed wattle and daub, close studding, jettied upper floors, thatch and red tile, Perpendicular churches, brick palaces (Hampton Court), tithe barns, windmills | Lordaeron is the old northern kingdom of farms, abbeys and market towns. Hearthglen, our pilot, is a hilltop abbey town. |
| **Gilneas** | Scottish burghs and Elizabethan England: harled stone, crow-stepped gables, tall tenements on a Royal Mile, tolbooths with clock towers, slate, a great curtain wall | Gilneas is grey, walled off (the Greymane Wall) and rain-soaked; Edinburgh's Old Town is its natural cousin. |
| **Kul Tiras** | Hanseatic and Dutch ports: brick Gothic churches (Lübeck's St Mary's), stepped and bell gables, warehouses with hoist beams (Bryggen, the Speicherstadt), crane gates (Gdańsk), gate towers (the Holstentor), quays and lighthouses | The sea-trading island nation. Ours is a trade sim, and the Hanse is the archetype of merchant towns. |
| **Stromgarde** (Arathi) | Roman and Adriatic: a castrum's cardo and decumanus, a forum with a basilica and colonnades, terracotta roofs, Diocletian's Palace at Split, campaniles, aqueducts | Stromgarde is the heir of Arathor, the first human empire, now in ruins. Our plan's forum and four avenues are a Roman camp already. |
| **Dalaran** | Tang and Song China: Chang'an's walled wards and markets, gate towers on platforms, drum and bell towers, pagodas, academies, courtyard houses, paifang arches over avenues, arched timber bridges (the Qingming scroll) | Our town plan is modelled on Chinese walled cities, so one kingdom should wear it natively. Dalaran is the city of scholars and orderly magistrates; its violet becomes glazed violet roof tiles. |
| **Alterac** | Alpine and Carpathian: stone-based timber chalets, steep snowy roofs, fortified churches (Biertan), castles on crags (Hohenwerfen, Bran), covered timber bridges (Lucerne) | Alterac is the mountain kingdom of cold passes, and our sim makes passes the places worth fighting over. |

Each kingdom also needs:
- 2 to 4 named palettes, e.g. Lordaeron's "daub" (white plaster and oak), "brick" and "stone";
- trees and planting that belong to it: oak and lime avenues in Lordaeron, cypress in Stromgarde, willow and ginkgo in
  Dalaran, larch and spruce in Alterac;
- a style sheet (`assets-src/kingdoms/<kingdom>/style.md`) we can read in five minutes.

**Growth tiers.** A town should visibly upgrade as the sim grows it. A proposal (tier bands are ours to tune):

| Tier | Villagers | How it should read |
|---|---|---|
| Hamlet | under 8 | cottages, a paved forum with a well, a palisade marked out but not built |
| Village | 8 to 23 | stalls round the forum, a chapel, a timber palisade with a gatehouse, gardens between the walls |
| Town | 24 to 47 | two-storey homes with trade yards, halls, a stone inner wall with towers, avenues planted |
| City | 48 to 95 | terraces filling the wards, a cathedral or great temple, guildhalls, the outer wall begun, quays |
| Capital | 96 and up, or the kingdom's seat | a keep or palace on the main axis, the outer wall whole, a grand dressed forum |

Today our towns reach about 20 to 30 villagers (mood caps growth until a pack adds more comforts), so hamlet to town is
what players will see first.

## The pilot: Hearthglen, end to end

One real town, start to finish, before anything is built in bulk.

1. **We export** Hearthglen (town 0, seed-8675309 study world) into `assets-src/gis/seed-8675309/0-hearthglen/`,
   using our GIS export (C13, still to write). The export holds:
   - its plan, terrain and sim state;
   - our surveyor's map;
   - masses for its halls and free plots (C15).

   We take it from a saved checkpoint, not a running world.
2. **You paint** Lordaeron at the village and town tiers: Hearthglen from the road, a street in one of its wards, its
   forum, and its abbey. John approves it.
3. **You design** Lordaeron's language (style sheet, palettes) and its first part set. Then you design Hearthglen at
   both tiers:
   - the forum dressed;
   - the inner wall built from a kit;
   - its wards filled, from the part set where it fits and from our masses where it doesn't;
   - one avenue planted out to the gate;
   - the farmsteads at the head of its first farm tracks.

   You return `layout.geojson`, renders from the road and the forum, and the rules that would do the same for any
   Lordaeron town (`town_rules/lordaeron.json` and `town_kits/lordaeron/<set>.json`, extended as you need).
4. **We place** what we can in a fork of the same world: assembled homes, stalls and yards, halls from masses, the
   wall kit, the well house. We render the result and put your render and ours side by side in the PR. Where ours
   falls short, that's the next code we write.
5. **Two revision rounds** at most on the pilot. Then we size the catalogue from what we learned.

## First batch (P0): Lordaeron, enough to test end to end

| # | What | Size | Count | Service |
|---|---|---|---|---|
| 1 | Concept art, village and town tiers: town view, street scene, forum, the abbey | | 8 images + notes | renders / signature |
| 2 | Lordaeron style sheet and palettes | | 1 | signature (design) |
| 3 | Lordaeron timber-frame part set and its assembly rules: corner posts, 2-block bays (plain, window, door, shop front; ground and upper), jettied upper posts, gable ends for depths 5 and 7, roof bay modules, dormer, 2 chimneys, porch, a 2-bed alcove fit-out | grid as README | about 25 parts + 1 rules file | signature (the set), bulk (the parts) |
| 4 | Homes assembled from the set at today's 5 x 5 slot, 3 palettes, and renders of the same set at 7 x 7 and as a terrace to prove it scales | 5 x 5 | 6 parameter sets, rendered | from #3 (near free) |
| 5 | Trade yards: smith, farmer's bake oven, shepherd's dye vats, fisherman's nets | 3 x 3 to 5 x 5 | 4 | bulk / signature-lite |
| 6 | Market stalls | 3 x 3, workstation marker at the centre, a locker chest touching it | 4 | bulk |
| 7 | Detailed builds from our masses: granary, temple (a chapel), smithy, counting house, the forum's well house round the bell, and one odd part-cell plot against the wall | today's footprints; 7 x 7; about 14 x 9 | 6 masses | signature-lite, one each |
| 8 | Inner wall kit, stone tier: corner 7 x 7, gatehouse 11 x 7, curtain 4 and 8, tower 7, fillers 1/2/3, step up and down | as README | about 10 pieces | 2 signature (gatehouse, corner), rest bulk |
| 9 | Hearthglen design: layout, renders at two tiers, `town_rules/lordaeron.json` | | 1 | layouts + renders |

Rough cost from your price table:

| Item | Estimate |
|---|---|
| Concept art | $20–60 (your estimate; renders run 3–80 min) |
| Style sheet | $5–15 |
| Part set and rules | $15–40 |
| Yards | $8–20 |
| Stalls | $1–8 |
| Masses | 6 x $2–8 = $12–48 |
| Wall kit | $15–35 |
| Hearthglen layout and renders | $10–40 |
| Two revision rounds | $20–50 |
| **Total** | **about $110–320** |

Tell us if your estimate differs.

Acceptance for the batch:
- John has approved the concept art, and the builds match it.
- Every piece passes the README's acceptance list.
- Our assembler builds the 5 x 5 homes from the part set at every parameter value in range.
- The six detailed builds keep every fixed block of their masses.
- Villagers claim the beds and workstations in a game test and can walk to them from the street.
- The wall kit tiles all four of Hearthglen's sides exactly, with the gates on the avenues.
- John looks at the two renders side by side and says it reads as Lordaeron, with character.

## The catalogue (after the pilot)

Per kingdom unless marked. "Code" refers to the README's list. Costs are rough, from your table, per kingdom. Most of
the catalogue is part sets and rules, so the count of distinct buildings in game is far larger than the count of
files.

### P1: Lordaeron to the town tier, and two more languages to prove the system

- Lordaeron: the palisade wall tier, a second part set (stone and brick, for halls and the city), all 13 trade yards,
  and forum dressing.
- Stormwind and Dalaran: each to first-batch level, concept art included.

That lets us see three languages on one map before committing to seven. **About $350–900** for the three.

### P2: every kingdom, hamlet to city

| Kit | What | Size | Count | Code | Est. |
|---|---|---|---|---|---|
| Concept art | town view, street, forum, landmark at each tier | | 4 a tier | | $20–60 a tier |
| Part sets | 2 to 4 sets a kingdom: vernacular homes (hamlet, village), town houses and shops, city terraces and tenements, halls (stone, brick or timber as fits) | the README grid | about 25–40 parts a set | C16 | $15–40 a set |
| Trade yards | one per vanilla trade: armorer, butcher, cartographer, cleric, farmer, fisherman, fletcher, leatherworker, librarian, mason, shepherd, toolsmith, weaponsmith; plus the Merchant's counting stall | 3 x 3 to 7 x 7 | 14 | today | $15–40 |
| Masses detailed | whatever our plans throw up that the sets can't: part-cells, odd halls, corner plots | any | per town, on demand | C15 | $2–8 each |
| Ward blocks | composition rules and a few example layouts: a close, a courtyard block (siheyuan for Dalaran), a market block, a garden block; how part-cells fill | 14 x 14, 13 x 14, 13 x 13 | 4–6 layouts + rules | C8 | layouts |
| Stalls | stalls, market hall bays, a covered market (city) | 3 x 3, 5 x 3; a 14 x 14 cell | 8 + 1 | today / C6, C8 | $5–20 |
| Halls | the 10 kinds, from the hall set at today's size and a city size | today's; a 14 x 14 cell | rules + masses | C4, C16 | $20–80 |
| Forum | a centrepiece a tier (well, market cross, fountain, statue, bell or drum tower), and a street-furniture set: lamps, benches, trees in planters, notice board, stocks, horse trough, market awnings | 5 x 5 to 9 x 9; props 1 x 1 to 3 x 3 | 4 + about 15 | C6 | $20–60 |
| Walls | palisade, stone and city kits for the inner wall; the outer wall's kit (bigger towers, a grand gate); barbicans and bastions | as README | 3 x 10 + 12 | C5 | $40–120 |
| Farms | farmsteads at the head of a holding (in place of its first field), field barns, a windmill, a watermill on a stream, dovecote, hay ricks; hedgerow and orchard rules | 9 x 9, 9 x 19 | 4 + 3 + 2 + 3 | C10 | $15–50 |
| Pastures | shelters and byres in a pasture's corner, a shepherd's hut, a dipping pen | 5 x 3, 5 x 5 | 4 | C10 | $3–10 |
| Docks | piers, quay segments that follow a shore (straight, convex, concave, stairs to the water), crane, fish market, boathouse | 1 x 4 to 3 x 12; quay runs 8 long | 12 | C11 | $20–50 |
| Landscaping | rules for planting and lighting avenues, lanes, a gate's approach, a square's trees, a town's edge (hedges, orchards, a green) | rules + props | 1 set | C6, C8 | layouts |

About **$250–700 a kingdom**, so **$1,500–4,000** for the other six. We'd go one or two kingdoms at a time.

### P2: landmarks (signature, each kingdom)

These are multi-part where they pass 48 blocks. They stand on merged ward cells (14 x 32, 32 x 32, 32 x 50) or at the
end of an avenue; the Capitol's axis is the model. All need C8 and C9.

- **Stormwind**: a cathedral at Notre-Dame's scale with a square in front; a keep (the White Tower); a harbour
  lighthouse; the Lion's Pride inn for Goldshire.
- **Lordaeron**: an abbey on a hill (Hearthglen; Glastonbury, Ely); a brick palace round courts (Hampton Court) for the
  capital; a guildhall (Lavenham); a great tithe barn; Tarren Mill's windmill.
- **Gilneas**: a crown-spired kirk (St Giles'); a royal palace (Holyrood); a tolbooth with a clock tower; a stretch of
  the Greymane Wall with its great gate.
- **Kul Tiras**: a brick Gothic church (St Mary's, Lübeck); Proudmoore's keep on the harbour; a crane gate; a
  warehouse row on a quay; a shipyard with a slip.
- **Stromgarde**: a basilica and colonnaded forum; a palace-fortress (Diocletian's); a temple (the Maison Carrée); a
  triumphal-arch gatehouse; an aqueduct kit that runs along a road.
- **Dalaran**: a palace hall on a terrace (Daming Palace); a pagoda (the Giant Wild Goose); an academy and examination
  hall; drum and bell towers for the forum; a magistrate's yamen for every town; paifang arches for avenues; an arched
  timber bridge.
- **Alterac**: a castle on a crag; a fortified church with its ring wall; a covered bridge; a pass gate with a toll
  house.

About **$40–90 a kingdom** plus revisions; **$300–650** for seven.

### P3: the rest of the world

- **Capital tier** for each kingdom: the composed axis, the outer wall whole, the dressed grand forum. Layouts.
- **Bridges** from a kit: abutments, piers, 5-wide spans 4 and 8 long, an arched span, bridgehead towers (C12).
- **On the roads**: waystations, inns, toll houses where the sim posts guards, milestones, wayside shrines. Waystations
  are new sim work; toll posts exist.
- **Mine heads**: a headframe over our 3 x 3 shaft, spoil heaps, an ore yard (C10).
- **Terrain dressing**: a town's edge blended into the land; quarries where the masons get stone; river banks in towns.
- **Upgrades** (C7): the assembly rules' answer to "the town grew". The same slot gets more storeys, a shop front,
  brick for daub, so a street changes in place rather than being torn down.

## Anything else we should know

- **The villager rules are ours, and they're the part that can break a town.** Beds, job-site blocks, bells and chests
  touching job sites all mean something to the game and our sim. The README lists them; please read that section
  closely and declare every one in the JSON. A trade yard's look must come from blocks that aren't job sites: a
  cauldron is a leatherworker's workstation and a smoker is a butcher's.
- **Determinism.** The same world must build the same town every time (our studies fork from checkpoints and compare
  exactly). Assembly rules must choose by the data we give them (the town, the slot, a seed), never at random.
- **Performance.** Towns are built while their chunks are loaded; the rest of the time they're numbers in the sim. A
  ward block of a few thousand blocks is fine. A landmark of tens of thousands we'll place over several ticks.
- **No entities but décor**, and no villagers or animals: the sim spawns those.
- **Where things go** in the repo, how to name them, the JSON, masses, parts, previews and the PR workflow: all in the
  README.
- **What we'd like back first:**
  - your plan and estimate for the pilot and the first batch;
  - one concept sketch of a Lordaeron street;
  - anything in this request or the README that makes your job harder than it needs to be.

Questions for you:

1. Is GeoJSON in `[x, -z]` block coordinates, plus 16-bit PNG height tiles with world files, the easiest input for
   you? Or do you prefer region files, a GeoTIFF, or something else?
2. Masses: JSON boxes, or a colour-coded `.nbt`, or both? What else would you want in a mass to do your best detailing?
3. Part sets: does a 2-block bay between 1-block posts with 4-block storeys suit you, or would another grid give
   better buildings? What should a socket carry for your parts to join cleanly?
4. Ward blocks: whole `.nbt` files, or layouts of parts plus rules? We prefer parts plus rules so towns don't repeat;
   tell us which gives better quality.
5. Wall kits on slopes: step pieces, or one piece with a deep plinth that we bury?
6. What do you need from us to design the growth tiers so a street upgrades in place?
