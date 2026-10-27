# Interiors: a common language

John: "each house built should have a sane interior design, both in terms of room layout, walkway length to room
volume ratios, and so on. Past designs often neglected the usability of rooms to squeeze in an interior." These are
the words we use for interior space, how each is measured, and the thresholds a house is held to.

The tool that measures them is `scripts/assets/floorplan.py`. It's pure Python, run on any structure:

```
python3 scripts/assets/floorplan.py house.nbt --out plans/
```

It writes `plans/<name>/plan.png`: the storeys side by side from above, ground floor first. Use `--json` for every
number. The studio and we both run it, and a disagreement about a house is a disagreement about a number on it.

## The words

- **Floor:** a cell inside the house where a villager can stand. It has something solid to stand on (not a bed, a
  chest or other furniture) and two clear blocks over it. "Inside" means the house's air that outside air can't reach
  without passing a door, a window or a wall.
- **Storey:** the floor at one height, counted from the feet. A stair's steps belong to no storey.
- **Landing:** floor joined to a stair's top or foot with no beds or work in it: designed circulation, counted as
  circulation whole (it should be 2 wide or more).
- **Room:** floor joined on one storey, bounded by walls and split at doors. An open arch joins two spaces into one
  room.
- **Use:** what a room is for, judged by what's in it and beside it:
  - `chamber`: beds;
  - `work`: a workstation;
  - `hall`: the street door opens into it;
  - `store`: chests or casks;
  - `stair`: all half steps;
  - `passage`: 4 cells or fewer, or one wide;
  - `room`: none of these.
- **Route:** the shortest walk from the street door to where a villager goes: a bed (from beside its head or its foot),
  a workstation, a chest, the hearth. Shelves, casks and decor are used floor (not idle) but no destination. Routes go
  level, and up or down a block only onto or off a stair or slab.
- **Circulation:** the floor on any route, judged over the storeys of two rooms or more (a one-room storey's routes
  are its use, not corridors).
- **Idle floor:** floor neither on a route nor beside anything used. It's space that does nothing: no bed, chest,
  table or hearth beside it, and nobody walks over it.
- **Made bed:** a bed with free floor along a long side, so it can be got into and made. Two beds pushed together
  against a wall aren't made beds.
- **Through room:** a chamber that a route to someone else's bed passes through. A stair that opens into a bedroom
  makes every bed above it a route through that bedroom.
- **Walk:** each bed's route over the storeys it crosses (a storey's worth being the root of a storey's floor), the mean
  over the beds: how far you walk for the size of the house, a third-floor bed allowed three storeys' worth. About 1
  is direct; over 2 is a maze.
- **Least width:** how narrow a room gets, measured as each cell's shorter run of floor across the room, at the
  tenth percentile of its cells.
- **Clear height:** air over the floor, the room's median.
- **Low cells:** floor with under 3 of clear height.
- **Floor a bed:** chamber floor divided by beds.

## The thresholds (v0.2: tuned with the studio, thread 0008)

| Measure | Target | Why |
|---|---|---|
| Every bed and workstation reached | all | the census counts only what a villager can walk to |
| Made beds | all | a bed in a corner behind another is a bed nobody can use |
| Through rooms | 0 | a stair opens onto a landing or the hall, never a bedroom |
| Circulation | 15% to 45% of floor (landings whole) | below that the plan is cramped; above it, a corridor house |
| Idle floor | under 45% | an empty box isn't a room: furnish it, shrink it, or give it a use |
| Walk | under 2.0 | routes direct for the house's size |
| Floor a bed | 4 to 8.5 | under 4 is a dormitory; over 8 is two beds in a barn |
| Least width, rooms | 3 or more (passages 2) | a 1-wide room is a corridor |
| Clear height | 3 or more (median) | a storey of 5 gives 4; under 3 reads as a crawlspace |
| Windows | 1 or more a chamber, about 1 per 8 floor | light, and a reason a room is on the outside wall |
| Hearth | in the hall, with its chimney | where a house lives |
| Storeys by width | a 7-wide body is one storey; two or more from 10 wide | a stair core in a 5 x 5 interior leaves no room 3 wide (thread 0008) |
| The stair's foot | a clear 2-wide approach from the street door | the "spine": nothing shelved or stacked across it |

## How to build to them (systems, not single houses)

The thresholds should be properties of the systems that make houses, so every house they make passes. They shouldn't
be checks we run once on a hand-built one. Concretely, for the plan layer of the module set:

- **Landing tiles.** A stair core's top opens onto a `landing` tile, and chambers open off the landing through a
  door. That fixes through rooms by construction.
- **Chamber tiles sized by beds.** A chamber tile is 2 beds, each with a free long side, plus a chest and a window,
  about 10 to 12 floor. Bigger floors split into more chambers, not bigger empty ones.
- **Furnishing by use.** Each room tile carries its furniture set:
  - hall: hearth, table, benches;
  - chamber: beds, a chest, a lantern;
  - work: the workstation and its stock.

  The furniture is placed by the tile, so idle floor stays low whatever the house size.
- **Circulation as its own tile.** `passage` and `landing` are tiles with a width of at least 2, so routes are
  designed, not left over.

`floorplan.py --check` holds a house to these and exits non-zero on a miss (a `quality.json` "interior" section
overrides them). It runs on every house the solver makes. The studio's validator can gain an `--interior` mode with these as
warnings first, errors once we've agreed the numbers.
