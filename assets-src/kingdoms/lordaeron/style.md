# Lordaeron: style sheet

The old northern kingdom of farms, abbeys and market towns: Tudor and late-medieval England, read through Warcraft's
warmth. Reference: the approved concept set (`assets-src/concept/lordaeron/`). This page is what every Lordaeron
build is checked against; the module set (`town_kits/lordaeron/timber_relief.json`) builds to it by default.

## The look in one breath

Dark oak frames on white daub, a stone or brick plinth, steep roofs of thatch, red tile or slate, brick chimneys from
the hearth through the roof, flower boxes, lanterns at doors, hedged gardens. Nothing flat: the frame stands proud of
the daub, every floor has a girt band, every window a sill.

## Walls

- **Frame:** corner posts proud as logs (`dark_oak_log`); inner posts as dark boards on the daub
  (`dark_oak_trapdoor`, open); a half-slab girt band at every floor (`dark_oak_slab`, bottom) that turns the corners.
- **Infill:** white daub (`calcite`) upstairs; ground storey stone (`stone_bricks`), daub or brick by palette.
- **Rhythm:** posts 1, bays 2, storeys 5 (a floor and 4 of room). A front is 3n + 1 wide (7, 10, 13).
- **Relief is required.** The frame layer stands one block outside the wall line. Flat walls fail the relief census
  (`js/relief.mjs`: relief 0.3 and detail 0.2 on every face).

## Openings

- **Windows:** a pair per bay over a room, glass panes on a thin trapdoor ledge; on the street front every other bay
  has a stair sill with a potted flower. Beside a stair, a single landing window.
- **Doors:** a wooden door and a sidelight in one bay, under a stair hood with a lantern standing on it. Villagers
  open wooden doors; never trapdoors on a route.
- **Shop fronts:** two-pane glazing over a slab counter.

## Roofs

- **Pitch 2** (two up for one across): steeper than vanilla's 45 degrees, as the concept art shows.
- **Covering by palette:** thatch (`hay_block`, stepped), red tile (`granite` stairs), slate (`deepslate_tile`).
- **Ends:** gable ends on the end walls (daub with a king post and tie beam), verge slices one beyond.
- **Chimneys:** brick, from the hearth up the west wall as a breast, one block above the ridge; never a cap or a
  crown.

## Palettes (`palettes.json`)

| Palette | Ground | Upper | Roof | Chimney | Use |
|---|---|---|---|---|---|
| `daub` | daub | daub | thatch | brick | cottages, hamlets, villages |
| `stone` | stone | daub | red tile | brick | corner houses, the forum, halls |
| `brick` | brick | brick | slate | stone | the town's better houses, halls |

## Plans (with the client's interior measures)

- **Every bed a made bed**, against a wall, a free long side, a chest beside.
- **No through rooms:** the stair arrives on a landing or in the only chamber.
- **A short spine** from the street door to the stair; what villagers walk to stands beside it.
- **7 wide:** one chamber floor. **10 wide:** a chamber and a 2 x 2 landing behind an L partition. **13 wide:** the
  straight stair, two chambers.
- **Furniture by use:** hall (hearth, table, benches, chest, shelf); chamber (2 beds, chest, table, shelf); work (the
  trade's workstation and its stock). Decorative job-site lookalikes are the mod's twins.

## Grounds

- **Front yards** read as gardens: picket or hedge, flowers, a bench, a lamp; or a trade's yard on the street (smith,
  baker, dyer).
- **Back yards:** kitchen garden, washing line, woodpile, well, hens, a tree.
- **Light:** a lamp at most 5 columns apart, so no grass sits dark.

## Planting

Oak and lime on avenues, orchards (oak with flowering azalea) on the abbey hill, box hedges (oak leaves) round
gardens, flower boxes under windows; wheat and carrots in kitchen gardens.

## Tiers

| Tier | Reads as |
|---|---|
| Hamlet | thatched cottages (7 wide, one chamber floor), a palisade marked out |
| Village | stalls round the forum, a chapel, halls in daub, timber palisade with a gatehouse |
| Town | two- and three-storey houses (10 wide), shop fronts, brick and slate, stone wall with towers |
| City | 13-wide houses with two chambers a floor, terraces, guildhalls, the abbey grown Perpendicular |
