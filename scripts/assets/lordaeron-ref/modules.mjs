// The Lordaeron timber module set (WFC): every piece a house is composed from, with its detail built in.
// Base palette (stone); h = 5 (a sill or floor layer, then 4 of room), pitch 2.
//
// Shell modules are authored for the FRONT face: local x runs left to right as seen from outside, y up (0 = the sill or
// floor layer), z 0 = the wall on the body's line, z 1 = the relief layer outside it. The composer turns them onto
// the other faces. Party modules are one layer (z 0 only).
import { Grid } from "./grid.mjs";

export const H = 5, PITCH = 2;
const B = {
  post: "dark_oak_log[axis=y]", beamX: "stripped_dark_oak_log[axis=x]", beamZ: "stripped_dark_oak_log[axis=z]",
  plinth: "cobblestone", floor: "spruce_planks", infill: "calcite", infill_ground: "stone_bricks", roof: "granite", chimney: "bricks",
  rstair: (f) => `granite_stairs[facing=${f},half=bottom,shape=straight,waterlogged=false]`,
  slab: "granite_slab[type=bottom,waterlogged=false]",
  sill: "dark_oak_stairs[facing=north,half=top,shape=straight,waterlogged=false]",
  board: "dark_oak_trapdoor[facing=south,half=bottom,open=true,powered=false,waterlogged=false]",
  band: "dark_oak_slab[type=bottom,waterlogged=false]",
  ledge: "spruce_trapdoor[facing=south,half=top,open=false,powered=false,waterlogged=false]",
  counter: "spruce_slab[type=top,waterlogged=false]",
  pane: "glass_pane[east=true,north=false,south=false,waterlogged=false,west=true]",
  door: (half, hinge) => `spruce_door[facing=south,half=${half},hinge=${hinge},open=false,powered=false]`,
  step: (f, shape = "straight") => `spruce_stairs[facing=${f},half=bottom,shape=${shape},waterlogged=false]`,
  lantern: "lantern[hanging=true,waterlogged=false]",
};
const infillOf = (st) => (st === "ground" ? B.infill_ground : B.infill);

// a shell column: wall[y] and relief[y] for y = 0..H-1 (relief null = a party module, one layer)
function columns(w, fill, party = false) {
  const g = new Grid([w, H, party ? 1 : 2]);
  for (let x = 0; x < w; x++) for (let y = 0; y < H; y++) {
    const [wall, relief] = fill(x, y);
    g.set(x, y, 0, wall);
    if (!party) g.set(x, y, 1, relief ?? "air");
  }
  return g;
}
const window = (st, y) => (y >= 2 && y <= H - 2 ? B.pane : infillOf(st));

const SHELL = {
  post: (st) => columns(1, (x, y) => [y === 0 ? B.beamX : B.post, y === 0 ? B.band : B.board]),
  chimney: () => columns(1, () => [B.chimney, B.chimney]),
  bay_plain: (st) => columns(2, (x, y) => [y === 0 ? B.beamX : infillOf(st), y === 0 ? B.band : "air"]),
  bay_window: (st) => columns(2, (x, y) => [y === 0 ? B.beamX : window(st, y), y === 0 ? B.band : y === 1 ? B.ledge : "air"]),
  bay_window_box: (st) => columns(2, (x, y) => [y === 0 ? B.beamX : window(st, y),
    y === 0 ? B.band : y === 1 ? (x === 0 ? B.sill : B.ledge) : y === 2 && x === 0 ? "potted_red_tulip" : "air"]),
  bay_landing: (st) => columns(2, (x, y) => [y === 0 ? B.beamX : x === 0 ? window(st, y) : infillOf(st), y === 0 ? B.band : y === 1 && x === 0 ? B.ledge : "air"]),
  bay_door: () => columns(2, (x, y) => {                       // the door on the right, a sidelight on the left
    if (y === 0) return [B.beamX, B.band];
    if (x === 0) return [window("ground", y), y === 1 ? B.ledge : "air"];
    return [y === 1 ? B.door("lower", "left") : y === 2 ? B.door("upper", "left") : y === 3 ? B.beamX : B.infill_ground,
      y === 3 ? B.sill : y === 4 ? "lantern[hanging=false,waterlogged=false]" : "air"];                    // the hood, a lamp on it
  }),
  // an open arcade bay (market halls): paving through, 3 clear under curved braces from the posts, the girt above
  bay_arcade: () => columns(2, (x, y) => [y === 0 ? "stone_bricks" : y === 3 ? (x === 0 ? "dark_oak_stairs[facing=west,half=top,shape=straight,waterlogged=false]" : B.lantern) : y === 4 ? B.beamX : "air",   // a brace from the post, a lantern hung from the girt
    y === 0 ? B.band : "air"]),
  bay_shop: () => columns(2, (x, y) => [y === 0 ? B.beamX : y >= 2 && y <= H - 2 ? B.pane : B.infill_ground, y === 0 ? B.band : y === 1 ? B.counter : "air"]),
  // a corner, 2 x 2 in plan: local x 0 = the side's relief, 1 = the wall corner; z 0 = wall row, 1 = front relief.
  // Posts stand proud on both faces; the girt band turns the outside corner.
  corner: () => { const g = new Grid([2, H, 2]); for (let y = 0; y < H; y++) { g.set(1, y, 0, B.post); g.set(1, y, 1, B.post); g.set(0, y, 0, B.post); g.set(0, y, 1, y === 0 ? B.band : "air"); } return g; },
  // where a front meets a party wall: the wall's corner post, proud on the front only
  corner_party: () => { const g = new Grid([1, H, 2]); for (let y = 0; y < H; y++) { g.set(0, y, 0, B.post); g.set(0, y, 1, B.post); } return g; },
  post_party: () => columns(1, (x, y) => [B.post], true),
  bay_party: (st) => columns(2, (x, y) => [y === 0 ? B.beamX : infillOf(st)], true),
  chimney_party: () => columns(1, () => [B.chimney], true),
};

// roof slices, 1 wide along the ridge, spanning `span` across it (local z), pitch 2
export const roofLayers = (span) => Math.floor((span - 1) / 2) * PITCH + 1;
function roofSlice(span, kind) {
  const m = Math.floor((span - 1) / 2), g = new Grid([1, roofLayers(span), span]);
  for (let k = 0; k < m; k++) for (let t = 0; t < PITCH; t++) {
    const y = k * PITCH + t, last = t === PITCH - 1;
    g.set(0, y, k, last ? B.rstair("south") : B.roof); g.set(0, y, span - 1 - k, last ? B.rstair("north") : B.roof);
    if (kind === "verge") {                                    // boards on the gable behind: a king post and a collar, proud of the daub
      for (let i = k + 1; i < span - 1 - k; i++) {
        const centre = Math.abs(i - (span - 1) / 2) < 1, collar = y === PITCH && i > k + 1 && i < span - 2 - k;
        if (y > 0 && (centre || collar)) g.set(0, y, i, "dark_oak_trapdoor[facing=west,half=bottom,open=true,powered=false,waterlogged=false]");
      }
      continue;
    }
    for (let i = k + 1; i < span - 1 - k; i++) {
      const centre = Math.abs(i - (span - 1) / 2) < 1;
      g.set(0, y, i, y === 0 ? (kind === "gable" ? B.beamZ : B.floor) : kind === "gable" ? (centre ? B.post : B.infill) : B.floor);
    }
  }
  for (let i = m; i < span - m; i++) g.set(0, m * PITCH, i, B.slab);
  return g;
}

// stair cores, one storey each: local x across, z 0 = the core's north row; layer 0 is this storey's floor
function core(type, part) {
  const below = part !== "bottom", run = part !== "top";
  if (type === "straight") {                                   // local x 0 = corridor, 1 = stair; z 0..4 = body z 1..5
    const g = new Grid([2, H, 5]);
    for (let z = 0; z < 5; z++) for (let x = 0; x < 2; x++) { g.set(x, 0, z, B.floor); for (let y = 1; y < H; y++) g.set(x, y, z, "air"); }
    if (below) { for (const z of [1, 2, 3]) g.set(1, 0, z, "air"); g.set(1, 0, 0, B.step("north")); }   // the holes, and the top step of the run below
    if (run) for (let k = 0; k < 4; k++) g.set(1, 1 + k, 4 - k, B.step("north"));                       // 4 steps; the 5th is the next storey's
    return g;
  }
  const g = new Grid([3, H, 3]);                               // turned, hugging the walls: up the east side northward,
  for (let z = 0; z < 3; z++) for (let x = 0; x < 3; x++) { g.set(x, 0, z, B.floor); for (let y = 1; y < H; y++) g.set(x, y, z, "air"); }   // round the corner, west along the back
  if (below) { g.set(2, 0, 1, "air"); g.set(2, 0, 0, "air"); g.set(1, 0, 0, "air"); g.set(0, 0, 0, B.step("west")); }
  if (run) { g.set(2, 1, 2, B.step("north")); g.set(2, 2, 1, B.step("north")); g.set(2, 3, 0, B.step("west", "outer_right")); g.set(1, 4, 0, B.step("west")); }
  return g;
}

const one = (s) => { const g = new Grid([1, 1, 1]); g.set(0, 0, 0, s); return g; };
const FITOUT = {
  floor: () => one(B.floor), foundation: () => { const g = new Grid([1, 2, 1]); g.set(0, 0, 0, B.plinth); g.set(0, 1, 0, B.plinth); return g; },
  room_air: () => { const g = new Grid([1, H - 1, 1]); for (let y = 0; y < H - 1; y++) g.set(0, y, 0, "air"); return g; },
  bed: () => { const g = new Grid([1, 1, 2]); g.set(0, 0, 0, "white_bed[facing=north,occupied=false,part=head]"); g.set(0, 0, 1, "white_bed[facing=north,occupied=false,part=foot]"); return g; },
  hearth: () => one("furnace[facing=east,lit=false]"), chest: () => one("chest[facing=west,type=single,waterlogged=false]"),
  shelf: () => one("bookshelf"), table: () => one("crafting_table"), lantern: () => one(B.lantern), workstation: () => one("air"),
  // room furniture and partitions (the plan layer's furniture sets)
  bench: () => one("spruce_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]"),           // the sitter faces south; turn with q
  table: () => { const g = new Grid([1, 2, 1]); g.set(0, 0, 0, "spruce_fence[east=false,north=false,south=false,waterlogged=false,west=false]"); g.set(0, 1, 0, "spruce_pressure_plate[powered=false]"); return g; },
  cask: () => one("enterprise:decor_barrel[facing=up,open=false]"),
  pot: () => one("potted_fern"),
  wall_inner: () => { const g = new Grid([1, H - 1, 1]); for (let y = 0; y < H - 1; y++) g.set(0, y, 0, y === H - 2 ? "stripped_spruce_log[axis=z]" : "spruce_planks"); return g; },
  door_inner: () => { const g = new Grid([1, H - 1, 1]); g.set(0, 0, 0, "spruce_door[facing=east,half=lower,hinge=left,open=false,powered=false]"); g.set(0, 1, 0, "spruce_door[facing=east,half=upper,hinge=left,open=false,powered=false]"); for (let y = 2; y < H - 1; y++) g.set(0, y, 0, y === H - 2 ? "stripped_spruce_log[axis=z]" : "spruce_planks"); return g; },
  // hall furniture (decorative lookalikes of job sites are the mod's twins)
  hay: () => one("hay_block[axis=y]"),
  desk: () => one("enterprise:decor_lectern[facing=east]"),
  altar: () => { const g = new Grid([1, 2, 1]); g.set(0, 0, 0, "chiseled_stone_bricks"); g.set(0, 1, 0, "candle[candles=4,lit=true,waterlogged=false]"); return g; },
  candles: () => one("candle[candles=3,lit=true,waterlogged=false]"),
  anvil: () => one("anvil[facing=north]"),
  smith_table: () => one("enterprise:decor_smithing_table"),
  forge: () => one("enterprise:decor_blast_furnace[facing=east]"),
  quench: () => one("enterprise:decor_cauldron[contents=water]"),
  rack: () => one("iron_bars[east=false,north=false,south=false,waterlogged=false,west=false]"),
  stall_rail: () => one("spruce_fence[east=false,north=true,south=true,waterlogged=false,west=false]"),
  goods_hay: () => one("hay_block[axis=y]"), goods_pumpkin: () => one("carved_pumpkin[facing=south]"), goods_wool: () => one("white_wool"),
  goods_blue: () => one("light_blue_wool"), goods_books: () => one("chiseled_bookshelf[facing=south,slot_0_occupied=true,slot_1_occupied=true,slot_2_occupied=false,slot_3_occupied=true,slot_4_occupied=false,slot_5_occupied=true]"),
  goods_kelp: () => one("dried_kelp_block"), goods_pickle: () => one("sea_pickle[pickles=3,waterlogged=false]"), goods_stone: () => one("chiseled_stone_bricks"),
  goods_iron: () => one("iron_block"), goods_bars: () => one("iron_bars[east=false,north=false,south=false,waterlogged=false,west=false]"), goods_candle: () => one("candle[candles=3,lit=true,waterlogged=false]"),
  goods_leather: () => one("brown_wool"), goods_target: () => one("target[power=0]"), goods_pot: () => one("decorated_pot[cracked=false,facing=south,waterlogged=false]"),
  goods_chain: () => one("chain[axis=y,waterlogged=false]"), goods_meat: () => one("red_terracotta"),
  goods_bread: () => one("hay_block[axis=x]"), goods_cake: () => one("cake[bites=0]"), goods_red: () => one("red_wool"), goods_honey: () => one("honeycomb_block"),
  oven: () => one("furnace[facing=south,lit=true]"), bar: () => one("stripped_spruce_log[axis=x]"),
  door_inner_z: () => { const g = new Grid([1, H - 1, 1]); g.set(0, 0, 0, "spruce_door[facing=south,half=lower,hinge=left,open=false,powered=false]"); g.set(0, 1, 0, "spruce_door[facing=south,half=upper,hinge=left,open=false,powered=false]"); for (let y = 2; y < H - 1; y++) g.set(0, y, 0, y === H - 2 ? "stripped_spruce_log[axis=x]" : "spruce_planks"); return g; },
  chimney_stack: () => { const g = new Grid([1, 1, 2]); g.set(0, 0, 0, B.chimney); g.set(0, 0, 1, B.chimney); return g; },
  chimney_stack_party: () => one(B.chimney),
};

/** Every module: { id, grid, meta } (meta: the WFC fields: layer, where, sockets, behind, weight, mirror, size) */
export function allModules({ spans = [9, 12, 15] } = {}) {
  const out = [], add = (id, grid, meta) => out.push({ id, grid, meta: { id, size: grid.size, ...meta } });
  const shellMeta = (st, extra) => ({ layer: "shell", where: { storey: st }, mirror: true, ...extra });
  for (const st of ["ground", "upper"]) {
    add(`post_${st}`, SHELL.post(st), shellMeta(st, { slot: "post", sockets: { left: "bay|corner", right: "bay|corner", down: "floor_line", up: "floor_line|eave" }, weight: 1 }));
    add(`bay_plain_${st}`, SHELL.bay_plain(st), shellMeta(st, { slot: "bay", behind: ["store", "hall", "chamber"], weight: 1 }));
    add(`bay_window_${st}`, SHELL.bay_window(st), shellMeta(st, { slot: "bay", behind: ["chamber", "hall"], weight: 4 }));
    add(`bay_window_box_${st}`, SHELL.bay_window_box(st), shellMeta(st, { slot: "bay", behind: ["chamber", "hall"], where: { storey: st, face: ["front"] }, weight: 2 }));
    add(`bay_landing_${st}`, SHELL.bay_landing(st), shellMeta(st, { slot: "bay", behind: ["stair"], weight: 3 }));
    add(`corner_${st}`, SHELL.corner(), shellMeta(st, { slot: "corner", weight: 1 }));
    add(`corner_party_${st}`, SHELL.corner_party(), shellMeta(st, { slot: "corner", relief: "party", weight: 1 }));
    add(`post_party_${st}`, SHELL.post_party(), shellMeta(st, { slot: "post", relief: "party", weight: 1 }));
    add(`bay_party_${st}`, SHELL.bay_party(st), shellMeta(st, { slot: "bay", relief: "party", weight: 1 }));
    add(`chimney_${st}`, SHELL.chimney(), shellMeta(st, { slot: "post", behind: ["hearth"], where: { storey: st, face: ["side"] }, weight: 1 }));
    add(`chimney_party_${st}`, SHELL.chimney_party(), shellMeta(st, { slot: "post", relief: "party", behind: ["hearth"], weight: 1 }));
  }
  add("bay_door_ground", SHELL.bay_door(), shellMeta("ground", { slot: "bay", where: { storey: "ground", face: ["front"] }, limit: "exactly 1 per front", weight: 1 }));
  add("bay_arcade_ground", SHELL.bay_arcade(), shellMeta("ground", { slot: "bay", behind: ["market"], weight: 1, note: "open arcade: market halls" }));
  add("bay_shop_ground", SHELL.bay_shop(), shellMeta("ground", { slot: "bay", behind: ["shop", "work", "hall"], where: { storey: "ground", face: ["front"] }, weight: 2 }));
  for (const span of spans) for (const kind of ["bay", "gable", "verge"]) add(`roof_${kind}_${span}`, roofSlice(span, kind), { layer: "roof", span, pitch: PITCH, kind, mirror: true });
  for (const type of ["straight", "turned"]) for (const part of ["bottom", "mid", "top"]) add(`core_${type}_${part}`, core(type, part), { layer: "plan", tile: type === "straight" ? "stair2x5" : "stair3x3", part, stacks: true });
  for (const [id, f] of Object.entries(FITOUT)) add(id, f(), { layer: "fitout" });
  return out;
}
