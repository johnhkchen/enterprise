// The Dalaran module set (dalaran/platform_hall): the same slots, sizes and ids as Lordaeron's timber_relief shell, so
// the solver and plan.json carry over, in Dalaran's own construction: red lacquered columns (stripped mangrove) on a
// stone plinth with a stepped lip, white plaster (calcite) between a dark beam and sill, lattice windows (dark oak
// trapdoors), bracket sets under the eave on every column, a waist eave (a violet tile skirt) between storeys, and a
// concave hipped roof of violet tile (purpur) with upturned corners, one module per body size. Stair cores and fit-out
// are Lordaeron's (the plan layer is shared). Authored as Lordaeron's: front face, z 0 the wall line, z 1 the relief.
import { Grid } from "../lordaeron-ref/grid.mjs";
import { allModules as lordaeronModules, H } from "../lordaeron-ref/modules.mjs";

export { H };
export const EAVE = 2;                                   // the roof's overhang past the relief
const B = {
  col: "stripped_mangrove_log[axis=y]", plinth: "polished_andesite", beamX: "dark_oak_log[axis=x]", infill: "calcite",
  lip: "andesite_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]",
  skirt: "purpur_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]",
  bracket: "dark_oak_stairs[facing=north,half=top,shape=straight,waterlogged=false]",
  lattice: "dark_oak_trapdoor[facing=south,half=bottom,open=true,powered=false,waterlogged=false]",
  sill: "dark_oak_trapdoor[facing=south,half=top,open=false,powered=false,waterlogged=false]",       // a ledge: nothing spawns on it
  potsill: "dark_oak_stairs[facing=north,half=top,shape=straight,waterlogged=false]", counter: "mangrove_slab[type=top,waterlogged=false]",
  door: (half, hinge) => `dark_oak_door[facing=south,half=${half},hinge=${hinge},open=false,powered=false]`,
  lantern: "lantern[hanging=true,waterlogged=false]", lamp: "lantern[hanging=false,waterlogged=false]",
};
// the bottom layer of a storey: on the ground the plinth with its lip, above it the beam under a tile skirt
// the column: red lacquer on the ground storey, dark timber framing the plaster above
const col = (st) => (st === "ground" ? B.col : "dark_oak_log[axis=y]");
const base = (st) => (st === "ground" ? [B.plinth, B.lip] : [B.beamX, B.skirt]);

function columns(w, fill, party = false) {
  const g = new Grid([w, H, party ? 1 : 2]);
  for (let x = 0; x < w; x++) for (let y = 0; y < H; y++) {
    const [wall, relief] = fill(x, y);
    g.set(x, y, 0, wall);
    if (!party) g.set(x, y, 1, relief ?? "air");
  }
  return g;
}
// a bay's wall: the base, plaster, the beam at the top (layer H-1)
const bayWall = (st, y, mid) => (y === 0 ? base(st)[0] : y === H - 1 ? B.beamX : mid);
// a lattice window: one column of the bay, two high, the rest white plaster (so the wall reads white, the lattice dark)
const lattice = (y, x = 0) => (x === 0 && y >= 2 && y <= H - 2 ? B.lattice : B.infill);

const SHELL = {
  post: (st) => columns(1, (x, y) => [y === 0 ? base(st)[0] : col(st), y === 0 ? base(st)[1] : y === H - 1 ? B.bracket : "air"]),
  chimney: (st) => columns(1, (x, y) => [y === 0 ? base(st)[0] : y === H - 1 ? B.beamX : B.infill, y === 0 ? base(st)[1] : "air"]),
  bay_plain: (st) => columns(2, (x, y) => [bayWall(st, y, B.infill), y === 0 ? base(st)[1] : "air"]),
  bay_window: (st) => columns(2, (x, y) => [bayWall(st, y, lattice(y, x)), y === 0 ? base(st)[1] : y === 1 && x === 0 ? B.sill : "air"]),
  bay_window_box: (st) => columns(2, (x, y) => [bayWall(st, y, lattice(y, x)),
    y === 0 ? base(st)[1] : y === 1 ? (x === 0 ? B.potsill : B.sill) : y === 2 && x === 0 ? "potted_azalea_bush" : "air"]),
  bay_landing: (st) => columns(2, (x, y) => [bayWall(st, y, x === 0 ? lattice(y) : B.infill), y === 0 ? base(st)[1] : y === 1 && x === 0 ? B.sill : "air"]),
  bay_door: () => columns(2, (x, y) => {                       // the door on the right, a lattice sidelight on the left
    if (y === 0) return [B.plinth, B.lip];
    if (x === 0) return [y === H - 1 ? B.beamX : lattice(y), y === 1 ? B.sill : "air"];
    return [y === 1 ? B.door("lower", "left") : y === 2 ? B.door("upper", "left") : y === H - 1 ? B.beamX : B.infill,
      y === 3 ? B.bracket : y === 4 ? B.lamp : "air"];         // a bracket over the door, a lamp on it
  }),
  bay_arcade: () => columns(2, (x, y) => [y === 0 ? B.plinth : y === 3 ? (x === 0 ? "dark_oak_stairs[facing=west,half=top,shape=straight,waterlogged=false]" : B.lantern) : y === 4 ? B.beamX : "air",
    y === 0 ? B.lip : "air"]),
  bay_shop: () => columns(2, (x, y) => [y === 0 ? B.plinth : y === H - 1 ? B.beamX : y === 2 ? "glass_pane[east=true,north=false,south=false,waterlogged=false,west=true]" : y === 3 ? B.lattice : B.infill,
    y === 0 ? B.lip : y === 1 ? B.counter : "air"]),
  // the corner, 2 x 2: x 0 the side's relief, x 1 the wall corner; z 0 the wall row, z 1 the front relief. Columns at the
  // wall corner and proud on the side; the base steps down on both faces and at the diagonal (nothing full to stand on)
  corner: (st) => { const g = new Grid([2, H, 2]); for (let y = 0; y < H; y++) {
    const ground = st === "ground";
    g.set(1, y, 0, y === 0 ? base(st)[0] : col(st));
    g.set(0, y, 0, y === 0 ? (ground ? B.lip.replace("north", "east") : B.skirt.replace("north", "east")) : col(st));
    g.set(1, y, 1, y === 0 ? (ground ? "andesite_slab[type=bottom,waterlogged=false]" : "purpur_slab[type=bottom,waterlogged=false]") : y === H - 1 ? B.bracket.replace("north", "south") : "air");
    g.set(0, y, 1, y === 0 ? base(st)[1] : "air");
  } return g; },
  corner_party: (st) => { const g = new Grid([1, H, 2]); for (let y = 0; y < H; y++) { g.set(0, y, 0, y === 0 ? base(st)[0] : col(st)); g.set(0, y, 1, y === 0 ? base(st)[1] : y === H - 1 ? B.bracket : "air"); } return g; },
  post_party: (st) => columns(1, (x, y) => [y === 0 ? base(st)[0] : col(st)], true),
  bay_party: (st) => columns(2, (x, y) => [bayWall(st, y, B.infill)], true),
  chimney_party: (st) => columns(1, (x, y) => [bayWall(st, y, B.infill)], true),
};

/** The concave hipped roof over a body's relief, one piece: sx x sz the relief's outline plus EAVE all round. Ring k
 *  steps in k from the eave; its height in half blocks is RISE[k] (shallow off the eave, then a block a ring). */
const TILE = (f, half = "bottom", shape = "straight") => `purpur_stairs[facing=${f},half=${half},shape=${shape},waterlogged=false]`;
const RISE = [0, 1, 2, 3, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22];
export const hipLayers = (sx, sz) => (RISE[Math.floor((Math.min(sx, sz) - 1) / 2)] >> 1) + 3;
export function hipRoof(sx, sz) {
  const g = new Grid([sx, hipLayers(sx, sz), sz]);
  for (let k = 0; k <= sx - 1 - k && k <= sz - 1 - k; k++) {
    const hh = RISE[k], y = hh >> 1, half = hh % 2 === 1, ax = k, bx = sx - 1 - k, az = k, bz = sz - 1 - k;
    const set = (x, z, f, shape = "straight") => g.set(x, y, z, half ? "purpur_slab[type=top,waterlogged=false]" : TILE(f, "bottom", shape));
    if (ax === bx || az === bz) {
      const yr = y + (half ? 1 : 0);
      for (let x = ax; x <= bx; x++) for (let z = az; z <= bz; z++) { for (let yy = 0; yy < yr; yy++) if (!g.get(x, yy, z)) g.set(x, yy, z, "purpur_block"); g.set(x, yr, z, "purpur_slab[type=bottom,waterlogged=false]"); }
      g.set(ax, yr, az, "purpur_pillar[axis=y]"); g.set(bx, yr, bz, "purpur_pillar[axis=y]");
      g.set(ax, yr + 1, az, "end_rod[facing=up]"); g.set(bx, yr + 1, bz, "end_rod[facing=up]");
      break;
    }
    for (let x = ax + 1; x < bx; x++) { set(x, az, "south"); set(x, bz, "north"); }
    for (let z = az + 1; z < bz; z++) { set(ax, z, "east"); set(bx, z, "west"); }
    for (const [cx, cz, f] of [[ax, az, "south"], [bx, az, "west"], [ax, bz, "east"], [bx, bz, "north"]]) set(cx, cz, f, "outer_left");
    if (k === 0) for (const [cx, cz, f] of [[ax, az, "south"], [bx, az, "west"], [ax, bz, "east"], [bx, bz, "north"]]) g.set(cx, y + 1, cz, TILE(f, "top", "outer_left"));
    // the ceiling under the roof, closed: the first ring inside the eave carries the attic floor
    for (let x = ax + 1; x < bx; x++) for (let z = az + 1; z < bz; z++) for (let yy = 0; yy <= y; yy++) if (!g.get(x, yy, z)) g.set(x, yy, z, k + 1 >= EAVE ? "purpur_block" : "dark_oak_planks");
  }
  // under the eaves: nothing; the attic air above the ceiling (layer 0 over the body) stays solid, decked
  return g;
}

/** Every module: Dalaran's shell and roofs, Lordaeron's cores and fit-out. Body sizes: the plan's bodies. */
export function allModules({ bodies = [[7, 7], [10, 7], [13, 7], [10, 10]] } = {}) {
  const out = [], add = (id, grid, meta) => out.push({ id, grid, meta: { id, size: grid.size, ...meta } });
  const shellMeta = (st, extra) => ({ layer: "shell", where: { storey: st }, mirror: true, ...extra });
  for (const st of ["ground", "upper"]) {
    add(`post_${st}`, SHELL.post(st), shellMeta(st, { slot: "post", weight: 1 }));
    add(`bay_plain_${st}`, SHELL.bay_plain(st), shellMeta(st, { slot: "bay", behind: ["store", "hall", "chamber"], weight: 1 }));
    add(`bay_window_${st}`, SHELL.bay_window(st), shellMeta(st, { slot: "bay", behind: ["chamber", "hall"], weight: 4 }));
    add(`bay_window_box_${st}`, SHELL.bay_window_box(st), shellMeta(st, { slot: "bay", behind: ["chamber", "hall"], where: { storey: st, face: ["front"] }, weight: 2 }));
    add(`bay_landing_${st}`, SHELL.bay_landing(st), shellMeta(st, { slot: "bay", behind: ["stair"], weight: 3 }));
    add(`corner_${st}`, SHELL.corner(st), shellMeta(st, { slot: "corner", weight: 1 }));
    add(`corner_party_${st}`, SHELL.corner_party(st), shellMeta(st, { slot: "corner", relief: "party", weight: 1 }));
    add(`post_party_${st}`, SHELL.post_party(st), shellMeta(st, { slot: "post", relief: "party", weight: 1 }));
    add(`bay_party_${st}`, SHELL.bay_party(st), shellMeta(st, { slot: "bay", relief: "party", weight: 1 }));
    add(`chimney_${st}`, SHELL.chimney(st), shellMeta(st, { slot: "post", behind: ["hearth"], where: { storey: st, face: ["side"] }, weight: 1 }));
    add(`chimney_party_${st}`, SHELL.chimney_party(st), shellMeta(st, { slot: "post", relief: "party", behind: ["hearth"], weight: 1 }));
  }
  add("bay_door_ground", SHELL.bay_door(), shellMeta("ground", { slot: "bay", where: { storey: "ground", face: ["front"] }, limit: "exactly 1 per front", weight: 1 }));
  add("bay_arcade_ground", SHELL.bay_arcade(), shellMeta("ground", { slot: "bay", behind: ["market"], weight: 1 }));
  add("bay_shop_ground", SHELL.bay_shop(), shellMeta("ground", { slot: "bay", behind: ["shop", "work", "hall"], where: { storey: "ground", face: ["front"] }, weight: 2 }));
  // roofs: one per body, over the relief outline (W + 2) x (D + 2) and the eave all round
  for (const [W, D] of bodies) { const sx = W + 2 + 2 * EAVE, sz = D + 2 + 2 * EAVE; add(`roof_hip_${W}x${D}`, hipRoof(sx, sz), { layer: "roof", kind: "hip", body: [W, D], eave: EAVE }); }
  for (const m of lordaeronModules()) if (m.meta.layer === "plan" || m.meta.layer === "fitout") out.push(m);
  return out;
}
