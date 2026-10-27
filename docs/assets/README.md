# Assets: how buildings get made for Enterprise

This folder is the contract between Enterprise (this repo: the mod, its sim, its town plans) and the asset studio
(an outside pipeline that turns real terrain and real city layouts into finished Minecraft builds). It says what we
send them, what they send back, where it lands in the repo, and how we check it. It's an early contract: expect it to
change as we learn what works. Change it by PR like anything else.

- `pipeline-brief.md`: the studio's own brief (services, prices, how they work).
- `pipeline-samples/`: their sample renders, for reference (a city skyline, a downtown at dusk, Notre-Dame and a river
  quay, the National Mall, Mont-Saint-Michel, the Capitol and its axis).
- `request-NNN-*.md`: our requests to them, numbered. `request-001-first-kingdoms.md` is the first.
- This README: the contract.

## The idea in one paragraph

The sim grows towns (`sim/`, the core) and the mod draws them in the world (`core/`, the binding). The town's shape is
already planned procedurally: a 13 x 13 forum round the bell, an inner and an outer wall (rectangles, square to the
compass, gates where four avenues cross them), lanes every 18 blocks inside the inner wall, wards between the lanes,
homes fronting the lanes, a ring of gardens between the walls, open fields along farm tracks outside, pastures beyond,
and caravan roads planned over the real ground between towns. Today every building is a few hard-coded blocks. The
studio is good at exactly the part we're weakest at: making a place look like a real place. So we send them our plans
as GIS data they already know how to read, they design the kingdoms' architecture and the rules for filling our plans,
they deliver kits of structures plus those rules as data, and our code places them as the sim grows each town.

The aim is character. Villagers shouldn't live in wood boxes: a town should look lived in, show what its people do for
a living, and change as it grows.

## Who owns what

**We own what makes a building work for villagers and for the sim.** That covers:
- where beds, workstations, doors and the bell go, and how many beds a home holds;
- which blocks the game treats as job sites or homes, and which to avoid;
- the paths villagers must be able to walk, with the headroom they need;
- the light levels that stop mobs spawning;
- how the census counts a town and how villagers claim beds and jobs;
- the plan's slots and their sizes.

We state these in this README and in every mass we send. They aren't up for negotiation in a design. If one gets in
the way of a good design, tell us and we'll change the rule.

**The studio owns how it looks.** That covers architecture, materials and palettes, massing beyond our envelope, roofs,
ornament, interiors beyond the required blocks, gardens, street dressing, landscaping, how a ward or a square is
composed, and how a town reads at each tier. We review for function and for the brief, but we don't art-direct
block by block.

## What flows which way

```
 Enterprise ──► studio                          studio ──► Enterprise
 ───────────────────────                        ─────────────────────────
 plans as GeoJSON (per town)                    concept art (per kingdom and tier), approved first
 terrain tiles (height, surface, water)         style sheets (each kingdom's architectural language)
 the town's sim state (kingdom, tier, people)   modular parts (.nbt) + assembly rules (build any size)
 masses: simple shapes with the required        detailed builds from our masses (.nbt + .json), the
   blocks fixed (see "Masses")                    required blocks exactly where the mass had them
 our surveyor's map render                      layouts as GeoJSON (what goes on which plot)
 comments on each round                         composition rules as JSON (how to fill any plot)
                                                previews (PNG/JPEG)
```

## Where things live in the repo

```
core/src/main/resources/data/enterprise/
  structure/<kingdom>/<kind>/<name>.nbt        shipped structures (in the mod jar)
  structure/<kingdom>/<kind>/<name>.json       the metadata for each, side by side (same stem)
  structure/<kingdom>/part/<set>/<part>.nbt    modular parts (facade bays, corners, roof modules...), each with a .json
  town_rules/<kingdom>.json                    shipped composition rules for a kingdom
  town_kits/<kingdom>/<set>.json               shipped assembly rules: how a set's parts make a building of any size
  town_layouts/<kingdom>/<kind>/<name>.json    shipped pre-composed layouts (a ward block, a forum, a quay run)

assets-src/                                    working material, not shipped
  concept/<kingdom>/<tier>/                    concept art: town views, street scenes, a landmark (approved first)
  masses/<batch>/<name>.json                   masses we send to be detailed (see "Masses")
  gis/<world>/<town-id>-<slug>/                exports we send (see "GIS export" below)
  design/<world>/<town-id>-<slug>/             designs they return for a real town (layouts, renders)
  kingdoms/<kingdom>/style.md                  the kingdom's style sheet (theirs, reviewed by us)
  kingdoms/<kingdom>/palettes.json             its palettes, named
  <kingdom>/<kind>/<name>/                     per structure: previews, revision notes, any source files
    front.jpg  top.jpg  [cutaway.jpg]  notes.md

docs/assets/                                   this contract, requests, the studio's brief and samples
```

Why this shape:

- NeoForge 1.21.1 loads structure templates from `data/<namespace>/structure/**.nbt` (singular `structure`, nested
  folders allowed). `structure/lordaeron/home/cottage_5x5_a.nbt` is the template
  `enterprise:lordaeron/home/cottage_5x5_a`. The game's own loader ignores the `.json` beside it; our code reads it
  with a reload listener of its own (code to add, see "Code on our side").
- The `.nbt` and its `.json` sit together so one PR diff shows both, and nothing can ship without its metadata.
- Previews and notes stay out of the jar (`assets-src/`). Keep previews small (JPEG, about 1280 px wide, a few hundred
  KB): GitHub is slow from the dev machine, and we aren't using Git LFS yet.
- `data/enterprise/structure/floor.nbt` is a game-test arena, not an asset. Leave it where it is.

## Names and ids

- **Kingdoms** (folder names): `stormwind`, `lordaeron`, `gilneas`, `kul_tiras`, `stromgarde`, `dalaran`, `alterac`,
  and `common` for things every kingdom shares (props, plain fences, field gear).
- **Kinds** (folder names): `home`, `stall`, `hall`, `landmark`, `forum`, `wall`, `dock`, `bridge`, `road`, `farm`,
  `pasture`, `mine`, `prop`, `yard` (a trade's yard), and `part` (modular parts, in sets). Layouts use `ward`, `forum`,
  `quay`, `avenue`.
- **Names**: lower snake case, `<what>_<w>x<d>_<letter>`, width along the street (x) then depth (z), as authored
  facing south. `cottage_5x5_a`, `townhouse_7x7_b`, `granary_7x7_a`, `gatehouse_11x7_a`.
- **Palette variants** add `__<palette>`: `cottage_5x5_a__brick`. The base has no suffix. A palette name is one of the
  kingdom's named palettes (`palettes.json`).
- **Never reuse an id for a different shape.** If a footprint, a door, a bed or a workstation moves, it's a new name
  (next letter). Looks-only changes keep the id and bump `rev`.

## Structure files

- **Minecraft 1.21.1, NeoForge**, vanilla structure format, gzipped NBT, `DataVersion` 3955. Nothing from a newer
  version: the game can't load a template saved by a newer one.
- **Vanilla blocks only.** Where the mod needs one of its own blocks (the Merchant's Desk), or a block that depends on
  the town (a workstation chosen by trade), leave the cell as air and declare a marker in the JSON. No structure blocks,
  jigsaw blocks, command blocks or spawners in shipped files.
- **No entities**, except item frames, paintings and armour stands, and only if the JSON lists them. No villagers or
  animals: the sim owns every villager and animal and spawns them itself.
- **Orientation.** Author everything with its front facing **south (+z)**: the street is on the south side. Our code
  turns it to face its real street (as `Growth.buildHome` does today). Wall pieces: the wall runs east-west (along x),
  the town inside is to the south. Corner pieces: authored as the town's north-west corner (walls leave it east and
  south). Mark in the JSON whether a piece may also be mirrored.
- **Origin.** The template's (0, 0, 0) is its north-west corner (lowest x, lowest z) at its lowest layer.
- **Ground row.** The JSON's `ground` is the layer that sits at the top block of the ground (it replaces that block: a
  floor, paving, a plinth's top). Layers below it are foundation, buried where the ground is high and showing as a
  plinth where it falls away. Give homes and halls 2 foundation layers, walls and towers 4. The placer fills under the
  lowest layer with the ground's own block (`Growth.level` today) and can run declared posts down to solid ground (piers,
  stilts: `pilings` in the JSON).
- **Air and void.** Air in the template clears what was there (rooms, doorways, headroom). `structure_void` leaves
  the world as it was (round irregular footprints, a garden's open edges). The footprint is the template's full x/z
  size; pad smaller builds to their slot with `structure_void` so corners line up.
- **Light.** Every interior cell a mob could stand in must be lit to 8 or more, and the template's outdoor walkable
  ground too: towns run unloaded and come back loaded, and a dark cottage spawns a zombie by the bed.
- **Paths for villagers.** Villagers walk the plan's lanes and avenues to their doors. Doors are wooden doors (villagers
  open those), two blocks high, with nothing between a door and the street but walkable ground. Inside, every bed and
  every declared workstation must be reachable from a door by a villager (no trapdoors, no one-high gaps, no carpet on
  a step that makes it 1.5 high).

### Villager rules (ours; the part that breaks a town if it's wrong)

The sim counts a town by its villagers' points of interest: beds (homes), workstations (jobs) and the bell (the
meeting place). A town is three beds and three workstations round a bell. Some blocks are points of interest whether
or not you meant them to be:

| Block | What the game makes of it |
|---|---|
| any bed | a home a villager will claim |
| bell | the town's meeting place |
| barrel, blast furnace, brewing stand, cartography table, cauldron (any), composter, fletching table, grindstone, lectern, loom, smithing table, smoker, stonecutter | a job site a jobless villager will claim |
| beehive, bee nest, lightning rod, lodestone | points of interest too: avoid |

And: **a chest, trapped chest or barrel touching a job site on any face is a locker** (`locker/Lockers.java`), a
shop the town's Merchant runs. So:

- Put a bed, a job-site block or a bell only where the JSON declares it, with its role.
- Never put a chest or barrel against a job site unless the JSON declares that pair as a locker.
- For décor use blocks that aren't points of interest: bookshelves and chiseled bookshelves, chests (away from job
  sites), decorated pots, crafting tables, furnaces, anvils, hay, candles, flower pots, banners.
- A home has **2 beds** for now (the core assumes 2 a home until the census counts them: `sim/core/src/land.rs`
  `unlaid_beds`). Homes with more beds come with a code change; ask for them in the JSON (`beds`) and we'll wire it.
- The bell is never in a structure except a forum centrepiece, and there it's a marker: the town's bell already exists
  and its position is the town's key in the save. The centrepiece is placed round it.

## Slots: what fits where

Footprints are in blocks, width (along the street) x depth. "Today" means our code can site it now, once the shared
placer exists; "needs code" is flagged with the change. The plan's numbers come from `territory/TownPlans.java`,
`Forums.java`, `Growth.java`, `Buildings.java`, `Jobs.java`, `Pastures.java`, `FieldMaps.java`, `Fields.java`,
`Docks.java`, `Works.java`.

**The plan's grid.** Inside the inner wall, lanes run every 18 blocks north-south and east-west from the bell's lines.
Each lane takes 4 blocks (2 of lane and a doorstep each side), so the ground between two lanes is **14 blocks**; next
to an avenue (3 wide, 5 kept with its edges) it's **13**. The wards are therefore cells of 14 x 14 (13 x 14 beside an
avenue, 13 x 13 at the cross), with part-cells against the wall. Homes front the lane on each side of a cell, back to
back, so a home can be at most **7 deep** (6 where it fronts an avenue). Along the street, homes stand one block apart.
The forum's 13 x 13 and a block round it are kept clear.

| Slot | Footprint | Today | Notes |
|---|---|---|---|
| Home on a lane | **5 x 5**, 2 beds | yes (`Growth.HOME_SIZE`) | door on the south face; a 1-block gap each side; up to 4 above ground checked clear (taller is fine, the placer clears headroom) |
| Home, other sizes | 7 x 5, 7 x 7, 9 x 6, 9 x 7, 11 x 7 | needs code | `TownPlans.frontages` takes one square size; make it width x depth (C3) |
| Row of attached homes (terrace) | widths 4 to 7, depth up to 7, no gap | needs code | a ward side filled end to end (C3) |
| Ward block | 14 x 14, 13 x 14, 13 x 13, part-cells | needs code | a whole cell composed at once (C8) |
| Landmark lot | merged cells: 14 x 32, 32 x 32, 32 x 50 | needs code | lanes between merged cells are given up (C8, C9) |
| Market stall | **3 x 3** | yes (`Jobs.STALL_SIZE`) | a workstation marker at the centre (1, 1, 1), a locker chest touching it; at least 6 from the bell |
| Hall | as its kind below | yes (`Buildings.java`, `sim/core/src/works.rs`) | square; door at the south face's middle; sited on the nearest open ground 8 to 32 from the bell |
| Forum centrepiece | 5 x 5 or 7 x 7, round the bell | needs code | the bell marker at its centre; walkable all round (C6) |
| Forum dressing | the 13 x 13 ring | needs code | lamps, benches, trees, stalls round the centrepiece (C6) |
| Inner wall pieces | thickness 1 to 7 (odd), centred on the wall's line | needs code | see "Wall kits" (C5) |
| Farmstead | 9 x 9 (one field cell) or 9 x 19 | needs code | at a holding's track end, in place of its first field (C10) |
| Pasture shelter | 5 x 3, 5 x 5 | needs code | in a corner inside the fence of pastures 12 x 10 and up (C10) |
| Pier / quay | pier 1 x 4 today | needs code | quay segments that follow a shore (C11) |
| Bridge | deck 5 wide (3 of deck, a rail each side) | needs code | today laid at hex angles over water; kits need straight runs (C12) |
| Mine head | 3 x 3 shaft, 5 x 5 headframe | needs code | `Mines.java` digs the 3 x 3 (C10) |

**Halls today** (kind id, footprint, height the sim prices them at): `square` 9 x 9 x 1 (the Masons' Square: an open
plaza, flower beds for a master Farmer), `granary` 7 x 7 x 4, `root_cellar` 5 x 5 x 3, `counting_house` 7 x 7 x 4,
`temple` 7 x 7 x 4, `library` 7 x 7 x 4, `smithy` 7 x 7 x 4, `watchtower` 3 x 3 x 7, `stone_docks` (relays the pier
in stone), `caravanserai` 9 x 9 x 4. The height is what the sim charges for (blocks = size x size x height); a
designed hall may be taller. A bigger footprint changes the sim's costs and needs the size changed in both
`Buildings.java` and `works.rs` (C4); within the footprint, anything goes.

**Heights.** No hard limit from the plan. As a guide so towns read well from the road: hamlet homes 5 to 8, town homes
8 to 12, city rows 12 to 18, halls 8 to 16, wall curtains 4 (palisade) to 12, towers and gatehouses to 24, landmarks to
48 and spires beyond. One template is at most 48 x 48 x 48 (the structure block's limit); bigger landmarks come in
parts with an assembly in the JSON.

### Wall kits

The inner wall is a rectangle round the village (`TownPlans.measure`): each side is 33 blocks or more (live towns:
65 to 95), and its length is whatever the village needed, so pieces must tile any length. A gate, 3 wide, sits where
each avenue crosses, on the bell's line, which is rarely the side's middle. Ground 3 blocks either side of the wall's
line is kept clear, so a wall piece is at most 7 thick. The outer wall is the same shape, twice the reach (120 at
most), reserved for later.

A wall kit is:

- `corner`: 7 x 7, centred on the corner column.
- `gatehouse`: 11 wide (the 3-wide passage in the middle, on the avenue's line) x 7 thick.
- `curtain_4`, `curtain_8`: straight runs 4 and 8 long.
- `tower`: an interval tower 7 long.
- `filler_1`, `filler_2`, `filler_3`: so any side tiles exactly.
- `step_up`, `step_down` (optional): a 4-long run that meets a neighbour one to three blocks higher or lower, for
  slopes. Each piece is placed at its own ground height; the seams must look built, not broken.

Tiers: a palisade kit (hamlet, village), a stone curtain kit (town), a city kit (taller, wall-walk, parapets). Projecting
bastions, a barbican in front of a gate, or a gate wider than 3 need the plan to reserve the ground (C5).

## Concept art comes first

Before any build for a kingdom or a tier, the studio paints it. For each kingdom at each tier asked for, deliver:
- a town view from the road coming in;
- a street scene inside the inner wall;
- the forum;
- one landmark.

These go in `assets-src/concept/<kingdom>/<tier>/` with a page of notes on materials, roofs, planting and the signs of
trade. John approves the set (comments on the PR) before builds start. After that, the concept art is the reference
every build and every review is checked against.

## Masses: simple shapes in, detailed builds out

Our plans make lots of buildings no catalogue would have: odd plot sizes, part-cells against a wall, a hall the sim
wants at a size nobody drew. For those we send a **mass**. A mass is a few boxes that give the envelope and the roof
line, with every block villagers need already fixed in place. The studio returns a detailed build that fills the
envelope and keeps every fixed block exactly where the mass put it. So any shape our code makes can get real detail
without a hand-made catalogue entry.

A mass is JSON (`assets-src/masses/<batch>/<name>.json`). The same shapes as a `.nbt` are optional: colour-coded
wool, one colour a tag, with the fixed blocks placed for real.

```json
{
  "mass": "hearthglen-ward-3-plot-2",
  "kingdom": "lordaeron", "tier": "town", "use": "home", "trades": ["minecraft:armorer"],
  "footprint": [7, 7], "ground": 0, "facing": "south",
  "boxes": [
    {"min": [0, 1, 2], "max": [6, 8, 6], "tag": "body", "storeys": 2},
    {"min": [0, 9, 2], "max": [6, 12, 6], "tag": "roof", "roof": "gable", "ridge": "x"},
    {"min": [0, 0, 0], "max": [6, 0, 1], "tag": "yard"}
  ],
  "fixed": [
    {"at": [3, 1, 6], "block": "minecraft:oak_door[facing=south,half=lower]", "role": "door"},
    {"at": [5, 1, 3], "block": "minecraft:blast_furnace", "role": "workstation"},
    {"at": [1, 5, 3], "block": "minecraft:white_bed[facing=north,part=head]", "role": "bed"},
    {"at": [1, 5, 4], "block": "minecraft:white_bed[facing=north,part=foot]", "role": "bed"},
    {"at": [3, 5, 3], "block": "minecraft:white_bed[facing=north,part=head]", "role": "bed"},
    {"at": [3, 5, 4], "block": "minecraft:white_bed[facing=north,part=foot]", "role": "bed"}
  ],
  "walk": [[3, 1, 5], [4, 1, 4], [2, 1, 4], [2, 5, 4]],
  "may_exceed": {"eaves": 1, "jetty": 1, "chimney": 3}
}
```

- The example is a 7 x 7 armorer's house on a lane: two storeys of 4 (ground at layer 0, the upper floor's beds on
  layer 5), a gable roof, a yard at the back (north; the street is south, past the door).
- `boxes` tags: `body` (enclosed, with `storeys`), `roof` (`gable`, `hip`, `flat`, `tower`; `ridge` axis), `tower`,
  `porch`, `yard` (open ground inside the plot: gardens, the trade's yard), `frontage` (a doorstep strip inside the
  plot, kept walkable), `void` (leave as it is). The street side is always the south edge.
- `fixed`: exact blocks with their states, by role (`door`, `bed`, `workstation`, `locker`, `bell`, `desk`, `light`).
  The build has these blocks at these positions and states, and adds no other point of interest.
- `walk`: cells that must stay walkable with 2 blocks of headroom (the way from the door to each bed and workstation).
  It's a guide; the rule is that every fixed block is reachable from the door.
- `may_exceed`: how far ornament may go past the envelope (eaves, jettied upper floors, chimneys). Nothing may go past
  the footprint at ground level.
- The studio returns `<name>.nbt` and the usual `.json`. Our check (C14) compares the result's fixed blocks with the
  mass's, cell by cell.

Masses can come from our plans directly (C15): the GIS export can write one for every free frontage, ward cell and
hall the sim has asked for. A batch of masses then becomes a batch of builds.

## Modular kits: parts that snap together

Hand-made buildings don't scale to seven kingdoms, five tiers and every plot size. So most of the catalogue should be
**parts on a grid** plus **assembly rules**, and our code builds houses of any width, depth and height from them (C16).
Variants come from parameters, not one-off files.

**The grid.** Widths and depths are odd, so a door or a gate can sit in the middle and a building can be centred on a
line:
- **Facades:** 1-block corner posts with 2-block bays between, so a front is 2n + 1 wide (5, 7, 9, 11, 13).
- **Depths:** 5 or 7 (the most a lane-fronting home can be), or 9 to 13 for halls.
- **Storeys:** 4 blocks each (the floor and 3 of headroom), with the ground storey on the mass's ground row.
- **Roofs:** one module a bay along the ridge, a gable or hip end for each depth.
- **Walls:** the wall kit's 4-long modules (see "Wall kits").

**A set** (`structure/<kingdom>/part/<set>/`) is one coherent family, e.g. Lordaeron timber-frame. Typical parts:
- **Walls of the building:** corner posts (ground, upper, jettied upper); bays (plain, window, shop front, door,
  carriage arch, each for the ground storey and the upper storeys).
- **Roof:** gable and hip ends at 5 and 7 deep, ridge and valley modules, dormers, chimneys, a lantern or cupola.
- **Additions:** porches, outside stairs, lean-tos, bay windows, signs and hanging lanterns.
- **Interior:** fit-outs that hold the required blocks in fixed places (a bed alcove with 2 beds, a hearth, a
  workstation corner).

Each part has a `.json` like any structure, plus its `sockets`: which faces join which (a bay's left and right edges
take a post or another bay; its top takes a storey or a roof module).

**The assembly rules** (`town_kits/<kingdom>/<set>.json`) say how to build from parameters:

```json
{
  "set": "lordaeron/timber",
  "params": {"bays": [2, 6], "depth": [5, 7], "storeys": [1, 3], "roof": ["gable", "hip"], "palette": ["daub", "brick", "stone"],
             "front": ["plain", "shop", "jettied"], "trade": "any"},
  "ground": {"door_bay": "middle", "shop_bay_if": "trade", "corner": "post_ground"},
  "upper": {"bay": ["window", "window", "plain"], "corner": "post_upper", "jetty_if": "front == jettied"},
  "roof": {"end": "gable_{depth}", "ridge": "x", "dormer_every": 2, "chimney": "back-left"},
  "fitout": {"beds": "bed_alcove_2", "workstation": "trade_corner"},
  "pick": "by seed: town id, slot"
}
```

They may be richer than this; the studio decides how. What we need is that our code can run them, deterministically,
for any parameter values in range, and that the result keeps the villager rules. Part sets that pass let our code
build homes, halls and terraces for any plot the plan makes. Masses then cover only what the sets can't.

## Character: signs of trade, gardens, lived-in streets

Every kit should show who lives where and what they do, at every tier:
- **Homes:** window boxes, a kitchen garden, a bench by the door, washing lines, a woodpile, a cat's ledge.
- **Trade yards** (`yard`, 3 x 3 to 7 x 7 beside a home or stall, chosen by the trade of the villager who lives there):
  - a smith's yard with an anvil and quenching trough (armorer, toolsmith, weaponsmith)
  - a farmer's bake oven, since our Farmers bake bread
  - a butcher's smokehouse
  - a shepherd's dye vats and fleeces drying
  - a leatherworker's tanning racks
  - a fisherman's net racks and boats drawn up
  - a fletcher's targets
  - a mason's stone yard
  - a cleric's herb garden
  - a librarian's book barrow
  - a cartographer's map table under an awning
- **Streets** at every tier: a hamlet's beaten paths and hedges, a village's stalls and well, a town's planted avenues
  and lamps, a city's paved squares, statues and fountains.

Mind the villager rules here: tanning vats are not cauldrons, and a bake oven is not a smoker. Use blocks that aren't
job sites unless the JSON declares one.

## Metadata: one JSON per structure

```json
{
  "id": "enterprise:lordaeron/home/cottage_5x5_a",
  "rev": 1,
  "kingdom": "lordaeron",
  "kind": "home",
  "slot": "home",
  "tiers": ["hamlet", "village"],
  "style": "tudor timber-frame, thatch",
  "palette": "base",
  "variants": ["brick", "stone"],
  "footprint": [5, 5],
  "height": 8,
  "ground": 2,
  "mirror": true,
  "entrances": [{"at": [2, 3, 4], "facing": "south", "width": 1}],
  "beds": [{"head": [1, 3, 2], "foot": [1, 3, 3]}, {"head": [3, 3, 2], "foot": [3, 3, 3]}],
  "workstations": [],
  "lockers": [],
  "markers": [],
  "pilings": [],
  "entities": [],
  "notes": "Two beds either side of the way in, a chest at the back."
}
```

- `slot`: one of the slot names in the table above (`home`, `stall`, `hall:<kind>`, `forum:centre`, `wall:<piece>`,
  `farmstead`, `pasture:shelter`, `dock:<piece>`, `bridge:<piece>`, `landmark:<lot>`, `prop`).
- Positions are in the template's own coordinates, as authored (facing south), before any turning. In the example,
  layers 0 and 1 are foundation, layer 2 (`ground`) is the floor, so the door and the beds stand on layer 3.
- `workstations`: `{"at": [x, y, z], "block": "minecraft:lectern"}`, or `"block": "any"` for a stall where the trade
  decides.
- `lockers`: `{"at": [x, y, z], "for": [x, y, z]}`: a container and the job site it touches.
- `markers`: `{"at": [x, y, z], "what": "desk" | "bell" | "workstation" | "flower" | "lamp" | "banner" | ...}`: cells the
  placer fills.
- `pilings`: `{"at": [x, z], "block": "minecraft:spruce_log"}`: columns run down from the lowest layer to solid ground.
- `assembly` (landmarks in parts): `[{"part": "<id>", "at": [x, y, z]}]`.
- A palette variant's JSON may be just `{"id": "...__brick", "base": "...", "palette": "brick", "rev": 1}`: everything
  else is the base's.

## Previews

For each structure (palette variants: one front view is enough), in `assets-src/<kingdom>/<kind>/<name>/`:

- `front.jpg`: three-quarter view from the south-east, daylight.
- `top.jpg`: straight down, one pixel a block or more, north up, the footprint's edge marked.
- `cutaway.jpg` for homes and halls: roof off, so beds and workstations show.
- `notes.md`: one line a revision (date, rev, what changed, why).

Layouts and town designs also get a render from a viewpoint we'd stand at in game (a road coming in, the forum).

## GIS export: our plans, as data they can read

One folder a town: `assets-src/gis/<world>/<town-id>-<slug>/`. Written by a dev script (code to add, C13).

**Coordinates.** Minecraft blocks, one unit a block. GeoJSON coordinates are `[x, -z]`, so north is up in any GIS
viewer (in Minecraft +z is south). Every feature also carries its exact block coordinates in `properties.mc` where
they matter. The raster world files (`.pgw`) use the same convention.

- `town.json`: the town's sim state on the day exported: id, name, kingdom, tier, day; the bell (x, y, z); villagers
  and their trades and levels; beds; mood and which comforts are met; treasury; halls built and works under way; its
  trade routes (partner towns, what goes each way, trips); its land's biomes; the focus the sim gave it (growing,
  building, trading).
- `plan.geojson`: features with `properties.kind`:
  - `forum` (polygon, 13 x 13), `bell` (point)
  - `wall_inner`, `wall_outer` (polygon), `wall_strip` (the 7-wide band each keeps clear), `gate` (point: width,
    facing, which wall)
  - `avenue` (line, width 3), `lane` (line, width 2), `ward_cell` (polygon: the ground between lanes; its size and the
    streets on each side)
  - `frontage` (polygon: a home slot the code would use, door facing), `building` (what stands now: homes, halls by
    kind, stalls by trade, the desk; with beds and workstations as points)
  - `ring` (polygons: the gardens between the walls, the grain ring, the pastures' ring)
  - `farm_track` (line: ring, the trade path it leaves), `holding` (polygon: the Voronoi parcel, its seed, its strips'
    direction), `field` (polygon 9 x 9: crop, taken or free), `pasture` (polygon: grass, animals)
  - `road` (line: the caravan road planned over the land, its carrier wagon or llama, trips run), `bridge` (polygon),
    `dock` (line)
  - `hex` (polygon: owner, water share; for context)
- `terrain/`: over the outer wall, the pastures' ring and 64 blocks round:
  - `height.png` (16-bit greyscale: the ground's top y, trees left out, + 64) with `height.pgw`
  - `surface.png` (8-bit indexed: the top block) with `surface.json` (index to block id)
  - `water.png` (mask), `trees.png` (mask), `biome.png` with `biome.json`
- `map.png`: our surveyor's map of the same area (`dev/experiments/world_render.py`), for a quick look.
- `region/` (on request): the world's own region files for the area, if they want the blocks themselves.

## What they send back for a town design

In `assets-src/design/<world>/<town-id>-<slug>/`:

- `layout.geojson`: features with `properties.asset` (a structure id), `properties.corner` ([x, y, z], block
  coordinates of the north-west corner at ground), `properties.facing` (the way its front faces), `properties.tier`
  (from which tier it stands), and `properties.slot` (the plan feature it fills: a frontage, a ward cell, a wall
  side). Landscaping (trees, hedges, lamps, paving) as features too, with an asset or a block.
- Renders of the design at each tier asked for.
- The rules that would reproduce it on any town (below), so the design isn't a one-off.

## Composition rules (data our code runs)

Where the studio is better at a job than our code (filling a ward, dressing a square, planting an avenue, running a
quay along a shore), they own the design and give us rules as data. `town_rules/<kingdom>.json`, version 0, to grow
as needed (they may add fields; tell us in the PR):

```json
{
  "kingdom": "lordaeron",
  "rev": 1,
  "tiers": {
    "hamlet":  {"villagers": [0, 7]},
    "village": {"villagers": [8, 23]},
    "town":    {"villagers": [24, 47]},
    "city":    {"villagers": [48, 95]},
    "capital": {"villagers": [96, null], "or": "the kingdom's seat"}
  },
  "homes": {"village": [{"tag": "home/cottage", "weight": 3}, {"tag": "home/longhouse", "weight": 1}]},
  "ward": {"town": {"layouts": ["lordaeron/ward/close_14x14_a"], "fill": "frontage-first", "yard": "prop/garden"}},
  "forum": {"village": {"centre": ["lordaeron/forum/market_cross_5x5_a"], "ring": [{"asset": "common/prop/lamp_a", "every": 4}]}},
  "avenue": {"town": {"tree": "minecraft:oak", "every": 6, "offset": 3, "lamp": "common/prop/lamp_a"}},
  "wall": {"village": "lordaeron/wall/palisade", "town": "lordaeron/wall/stone"},
  "quay": {"town": {"run": ["lordaeron/dock/quay_8_a"], "every": 24, "stairs": "lordaeron/dock/quay_stairs_a"}}
}
```

Tags (`home/cottage`) are matched against `kind/name` prefixes. Tier bands are a project dial; the studio proposes
what reads right, we tune it against how fast the sim grows towns.

## Code on our side (none of it written yet)

The kits and rules do nothing until we place them. What each needs is flagged by number in the request:

- **C1 Placer** (`territory/Kits.java`): load a template and its JSON; pick a variant fixed by the town and the slot
  (same world, same town, same choice); turn it to face its street; level the ground, fill foundations, run pilings,
  clear headroom (trees) over its full height; fill markers (the desk, a stall's workstation by trade); report the
  beds' heads to the census as `Growth.buildHome` does; spread a large placement over ticks. Replaces the hard-coded
  `Growth.buildHome`, `Buildings.build`, `Jobs.buildStall` as each kind's kit arrives, with the old code as fallback.
- **C2 Kingdom and tier**: a town's kingdom (settlers inherit their home town's; first towns by region) and its tier
  (from its people). Name lists per kingdom (`settlement/Names.java` and `sim/core/src/settlers.rs`, in step).
- **C3 Frontages by width and depth**, terraces.
- **C4 Hall sizes**: bigger halls change their price in the core (`works.rs`) and `Buildings.java` together.
- **C5 Walls from kits**: the wall laid piece by piece as the sim's share of it grows, tier by tier; the outer wall
  as a work of its own; ground reserved for bastions and barbicans.
- **C6 Forum dressing**: a centrepiece round the bell, the ring dressed, stalls round the square.
- **C7 Upgrades**: a building replaced by its next tier's in the same slot, never losing beds or job sites the census
  counted (the save is the truth; blocks are its view).
- **C8 Ward blocks and landmark lots**: wards as cells the code fills from a layout; merged cells for landmarks.
- **C9 Landmarks**: a work the sim can ask for (a cathedral, a keep, a palace) and the host can site.
- **C10 Farm and pasture buildings, mine heads**: farmsteads at holdings' track ends, shelters in pastures,
  headframes.
- **C11 Quays and docks** that follow a shore.
- **C12 Bridges in straight runs** from a kit, and bridgeheads.
- **C13 GIS export**: `dev/experiments/gis_export.py WORLD [TRACE.db] --town N OUT/`, pure Python like
  `world_render.py` (it already reads region files, `data/enterprise_town_plans.dat`,
  `data/enterprise_field_maps.dat` and `enterprise-roads.json`); the sim state from the run's trace
  (`enterprise-sim --trace`). Run on a stopped server's world or a checkpoint, never a live one.
- **C14 Checks**: `scripts/assets_check.py` (reads every shipped `.nbt` and `.json`: DataVersion, size within its slot,
  no forbidden blocks, every point of interest declared, front facing south, beds whole, previews present; a detailed
  build against its mass, fixed block by fixed block) in `just check`; and a game test that places each slot kind's
  kit on a test arena and checks villagers claim its beds and workstations and can walk to them from the street.
- **C15 Masses from plans**: the GIS export writes a mass for each free frontage, ward cell and hall the sim asked
  for, with the required blocks fixed where our rules put them.
- **C16 Kit assembler** (`territory/Assembly.java`): runs a set's assembly rules for given parameters and slot,
  snapping parts by their sockets, then hands the result to the placer (C1). Deterministic by town and slot.

## Workflow

1. We write a request (`docs/assets/request-NNN-*.md`) and, for town designs, the GIS export; for detailing, masses.
2. They reply with a plan, an estimate and a test image (in the request's PR thread, or a reply file beside it).
3. For a new kingdom or tier: concept art first, on its own PR, approved by John before any build.
4. They deliver on a branch `assets/NNN-<slug>` and open a PR to `main`: `.nbt` + `.json` under
   `core/src/main/resources/data/enterprise/`, previews and notes under `assets-src/`. One PR a batch.
5. The PR must pass `scripts/assets_check.py` (C14, once written; until then we check by hand).
6. We place the batch in a headless test world (`just server-setup`, a fork from a study checkpoint) and render it.
   We comment on the PR, one comment a structure, with what to change.
7. A revision round is new commits on the same branch: bump `rev`, add a line to `notes.md`. Looks-only changes keep
   the id; shape changes take a new name.
8. We merge when it passes and reads right in game. Shipped ids are stable from then on.

## Acceptance, for every structure

- Loads in 1.21.1 (DataVersion 3955), vanilla blocks only, fits its slot exactly (padded with void), front facing south.
- Every bed, job site, bell and locker declared in the JSON, and no others.
- Lit inside and on its walkable ground; doors villagers can use; every bed and workstation reachable from the street.
- Sits on sloped ground without floating or burying its door (foundation layers present).
- Reads as its kingdom and tier at a glance, against the approved concept art and beside the others in the kit
  (renders side by side in the PR).
- From a mass: every fixed block at its position and state, nothing past the envelope but what `may_exceed` allows.
- A part set: assembles at every parameter value in range with no gaps or clashes (our assembler's test renders).
- Previews and `notes.md` present.
