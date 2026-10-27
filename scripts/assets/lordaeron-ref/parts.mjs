// The Lordaeron timber-frame part set, authored in the base palette (stone), front facing south (+z).
// Each part is a small Grid; `json` carries what the contract asks of a part (sockets, size, roles).
import { Grid } from "./grid.mjs";

// the base palette's blocks by role (palettes.json "stone"): every role a different block, so a palette can be applied
// by replacing blocks
export const BASE = {
  post: "dark_oak_log[axis=y]", beamX: "stripped_dark_oak_log[axis=x]", brace: "dark_oak_stairs",
  infill: "calcite", infill_ground: "stone_bricks", plinth: "cobblestone", floor: "spruce_planks",
  roof: "granite", roof_stair: "granite_stairs", roof_slab: "granite_slab", chimney: "bricks",
  window: "glass_pane[east=true,west=true]", door: "spruce_door",
};
export const STOREY = 4;                                         // today's storey: the floor or sill layer, then 3 of headroom
let H = STOREY;                                                  // the storey height the parts are being made at

// ---- wall columns: 1 wide, H high, 1 thick; layer 0 is the sill or girt, the wall face is the south side.
// Windows fill layers 2..H-2 (one pane at H=4, two at 5, three at 6); a door is 2 high with a lintel over it.
function column(fill) { const g = new Grid([1, H, 1]); fill((y, s) => g.set(0, y, 0, s)); return g; }
const sill = (put) => put(0, BASE.beamX);
const walls = (storey) => (storey === "ground" ? BASE.infill_ground : BASE.infill);

const COLUMNS = {
  post_corner: (st) => column((put) => { for (let y = 0; y < H; y++) put(y, BASE.post); }),
  stud: (st) => column((put) => { sill(put); for (let y = 1; y < H; y++) put(y, BASE.post); }),
  panel: (st) => column((put) => { sill(put); for (let y = 1; y < H; y++) put(y, walls(st)); }),
  window: (st) => column((put) => { sill(put); for (let y = 1; y < H; y++) put(y, y >= 2 && y <= H - 2 ? BASE.window : walls(st)); }),
  door: (st) => column((put) => { sill(put); put(1, `${BASE.door}[facing=south,half=lower,hinge=left,open=false,powered=false]`);
    put(2, `${BASE.door}[facing=south,half=upper,hinge=left,open=false,powered=false]`); put(3, BASE.beamX); for (let y = 4; y < H; y++) put(y, walls(st)); }),
  shop: (st) => column((put) => { sill(put); for (let y = 1; y < H; y++) put(y, y >= 2 ? BASE.window : walls(st)); }),
  chimney: () => column((put) => { for (let y = 0; y < H; y++) put(y, BASE.chimney); }),
};
const COLUMN_STOREYS = { post_corner: ["ground", "upper"], stud: ["ground", "upper"], panel: ["ground", "upper"],
  window: ["ground", "upper"], door: ["ground"], shop: ["ground"], chimney: ["ground", "upper"] };

// ---- roof slices: 1 wide along the ridge (x), spanning `span` across it (z); layer 0 sits on the walls' top.
// Pitch 1 is 45 degrees (a stair a step); pitch 2 is steep (a full block under each stair: 2 up for 1 across).
export const roofLayers = (span, pitch = 1) => ((span - 1) / 2) * pitch + 1;
let P = 1;
function roofSlice(span, gable, verge = false) {
  const m = (span - 1) / 2, top = m * P, g = new Grid([1, top + 1, span]);
  for (let k = 0; k < m; k++) for (let t = 0; t < P; t++) {
    const y = k * P + t, stair = t === P - 1;
    g.set(0, y, k, stair ? `${BASE.roof_stair}[facing=south,half=bottom,shape=straight,waterlogged=false]` : BASE.roof);
    g.set(0, y, span - 1 - k, stair ? `${BASE.roof_stair}[facing=north,half=bottom,shape=straight,waterlogged=false]` : BASE.roof);
    for (let z = k + 1; z < span - 1 - k && !verge; z++) {
      if (y === 0) g.set(0, 0, z, gable ? `stripped_dark_oak_log[axis=z]` : BASE.floor);   // tie beam, or the ceiling
      else if (gable) g.set(0, y, z, z === m ? BASE.post : BASE.infill);                   // king post and daub
      else g.set(0, y, z, span > 5 || P > 1 ? BASE.floor : "air");                         // a big attic is decked solid: no dark space a mob could spawn in
    }
  }
  g.set(0, top, m, `${BASE.roof_slab}[type=bottom,waterlogged=false]`);
  return g;
}

// ---- small parts the assembler places by rule
const one = (s, size = [1, 1, 1]) => { const g = new Grid(size); g.set(0, 0, 0, s); return g; };
const SMALL = {
  foundation: () => { const g = new Grid([1, 2, 1]); g.set(0, 0, 0, BASE.plinth); g.set(0, 1, 0, BASE.plinth); return g; },
  floor: () => one(BASE.floor),
  chimney_stack: () => one(BASE.chimney),
  jetty_bracket: () => one(`${BASE.brace}[facing=north,half=top,shape=straight,waterlogged=false]`),
  bed: () => { const g = new Grid([1, 1, 2]); g.set(0, 0, 0, "white_bed[facing=north,occupied=false,part=head]"); g.set(0, 0, 1, "white_bed[facing=north,occupied=false,part=foot]"); return g; },
  hearth: () => one("furnace[facing=east,lit=false]"),
  chest: () => one("chest[facing=west,type=single,waterlogged=false]"),
  lantern: () => one("lantern[hanging=true,waterlogged=false]"),
  ladder: () => { const g = new Grid([1, H, 1]); for (let y = 0; y < H; y++) g.set(0, y, 0, "ladder[facing=west,waterlogged=false]"); return g; },
  shelf: () => one("bookshelf"),
  table: () => one("crafting_table"),
  pot: () => one("potted_red_tulip"),
};

const SOCKETS = {
  column: { left: "column", right: "column", top: "column|roof", bottom: "column|foundation", face: "south (the street, after turning)" },
  roof: { along: "roof_bay|gable_end (x)", bottom: "the top storey's columns", span: "z" },
  small: { placed: "by rule (timber.json fitout)" },
};

/** Every part: { name, grid, json }. Options set the scale: storey height, roof pitch, the roof spans to make. */
export function allParts({ storey = STOREY, pitch = 1, spans = [5, 7] } = {}) {
  H = storey; P = pitch;
  const out = [];
  for (const [kind, sts] of Object.entries(COLUMN_STOREYS)) for (const st of sts) {
    const name = `${kind}_${st}`;
    out.push({ name, grid: COLUMNS[kind](st), json: { kind: "column", column: kind, storey: st, storey_height: H, sockets: SOCKETS.column } });
  }
  for (const span of spans) {
    out.push({ name: `roof_bay_${span}`, grid: roofSlice(span, false), json: { kind: "roof", roof: "bay", span, pitch: P, layers: roofLayers(span, P), sockets: SOCKETS.roof } });
    out.push({ name: `roof_verge_${span}`, grid: roofSlice(span, false, true), json: { kind: "roof", roof: "verge", span, pitch: P, layers: roofLayers(span, P), sockets: SOCKETS.roof, note: "the roof's edge carried past a gable: covering only" } });
    out.push({ name: `gable_end_${span}`, grid: roofSlice(span, true), json: { kind: "roof", roof: "gable_end", span, pitch: P, layers: roofLayers(span, P), sockets: SOCKETS.roof } });
  }
  for (const [name, f] of Object.entries(SMALL)) out.push({ name, grid: f(), json: { kind: "fitout", sockets: SOCKETS.small } });
  H = STOREY; P = 1;
  return out;
}
