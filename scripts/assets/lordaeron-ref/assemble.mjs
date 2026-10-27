// The reference assembler for town_kits/lordaeron/timber.json: reads the rules and the part files and builds a home
// (or a terrace) for given params. Deterministic: the same params give the same blocks.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { Grid, splitState, joinState } from "./grid.mjs";

export function loadKit(bundle) {
  const data = join(bundle, "core/src/main/resources/data/enterprise");
  const rules = JSON.parse(readFileSync(join(data, "town_kits/lordaeron/timber.json"), "utf8"));
  const palettes = JSON.parse(readFileSync(join(bundle, "assets-src/kingdoms/lordaeron/palettes.json"), "utf8"));
  const cache = new Map();
  const part = (name) => {
    if (!cache.has(name)) cache.set(name, Grid.load(join(data, "structure/lordaeron/part/timber", `${name}.nbt`)));
    return cache.get(name);
  };
  return { rules, palettes, part };
}

// integer expressions over the house's variables (the rules' mini-language)
export function evalExpr(expr, vars) {
  if (typeof expr === "number") return expr;
  if (!/^[\w\s+\-*/()<>=!&|.]*$/.test(expr)) throw new Error(`bad expression ${expr}`);
  const names = Object.keys(vars);
  const v = Function(...names, `"use strict"; return (${expr});`)(...names.map((n) => vars[n]));
  return typeof v === "number" ? Math.trunc(v) : v;
}

/** palette: base block -> the palette's block for the same role (stairs, slabs, doors keep their states) */
export function paletteMap(palettes, name) {
  const base = palettes.palettes[palettes.base], pal = { ...base, ...(palettes.palettes[name] || {}) };
  const byId = new Map(Object.entries(base).filter(([r]) => r !== "note").map(([role, id]) => [splitState(id).name, role]));
  return (s) => {
    const st = splitState(s), role = byId.get(st.name);
    if (!role || pal[role] === base[role]) return s;
    const to = splitState(pal[role]);
    const shaped = /_(stairs|slab|door|trapdoor|pane)$/.test(to.name) && /_(stairs|slab|door|trapdoor|pane)$/.test(st.name);
    return joinState({ name: to.name, props: shaped ? st.props : st.name.endsWith("_log") && to.name.endsWith("_log") ? st.props : to.props });
  };
}

const REPEAT = ["panel", "panel", "stud"];

// the column kind at each position of a wall
function frontBack(L, face, storey, front) {
  const c = (L - 1) / 2, out = [], jambs = face === "front" && storey === "ground" && L >= 7;
  for (let i = 0; i < L; i++) {
    const k = Math.abs(i - c);
    if (i === 0 || i === L - 1) { out.push("post_corner"); continue; }
    if (k === 0) { out.push(face === "front" && storey === "ground" ? "door" : face === "back" && storey === "ground" ? "window" : "stud"); continue; }
    if (jambs && k === 1) { out.push("stud"); continue; }                                   // door jambs on wider fronts
    const kk = jambs ? k - 1 : k, kind = REPEAT[(kk - 1) % 3], j = Math.floor((kk - 1) / 3);
    if (kind === "stud") { out.push("stud"); continue; }
    if (face === "front" && storey === "ground" && front === "shop" && j === 0) out.push("shop");
    else if (face === "back" && storey === "ground") out.push("panel");
    else out.push(j % 2 === 0 ? "window" : "panel");
  }
  return out;
}
function side(L) {
  const out = [];
  for (let i = 0; i < L; i++) {
    if (i === 0 || i === L - 1) { out.push("post_corner"); continue; }
    const d = Math.min(i, L - 1 - i);
    out.push(L % 2 && i === (L - 1) / 2 ? "window" : REPEAT[(d - 1) % 3]);
  }
  return out;
}

/** One home. p: { width, depth, storeys, roof, palette, front, trade } -> { grid, meta } */
export function assembleHome(kit, p, { into, ox = 0, roofEnds = [true, true], skipRoof = false } = {}) {
  const { rules, part } = kit, W = p.width, D = p.depth, S = p.storeys, front = p.front || "plain";
  if (W % 2 === 0 || W < 5 || W > 13 || ![5, 7].includes(D)) throw new Error(`width ${W} x depth ${D} out of range`);
  if (p.roof === "gable_z" && ![5, 7].includes(W)) throw new Error("gable_z needs width 5 or 7");
  if (front === "jettied" && (D !== 7 || S !== 2)) throw new Error("jettied needs depth 7 and 2 storeys");
  const SH = kit.storey || 4, pitch = p.pitch || 1;
  const roofBase = 2 + SH * S, span = p.roof === "gable_z" ? W : D, layers = ((span - 1) / 2) * pitch + 1;
  const g = into || new Grid([W, roofBase + layers + 1, D]);
  const put = (name, x, y, z, q = 0) => g.paste(part(name), ox + x, y, z, { q });
  const c = (W - 1) / 2, beds = [], workstations = [], late = [];

  for (let x = 0; x < W; x++) for (let z = 0; z < D; z++) put("foundation", x, 0, z);
  for (let s = 0; s < S; s++) {
    const st = s === 0 ? "ground" : "upper", gy = 2 + SH * s;
    const zw = st === "ground" && front === "jettied" ? D - 2 : D - 1, zf = zw - 1;
    for (let x = 1; x < W - 1; x++) for (let z = 1; z < zw; z++) put("floor", x, gy, z);
    // walls: front, back, then the sides between the corners
    frontBack(W, "front", st, front).forEach((k, x) => put(`${k}_${st}`, x, gy, zw, 0));
    frontBack(W, "back", st, front).forEach((k, x) => put(`${k}_${st}`, x, gy, 0, 2));
    const sides = side(zw + 1);
    for (let z = 1; z < zw; z++) { put(`${sides[z]}_${st}`, W - 1, gy, z, 3); put(`${sides[z]}_${st}`, 0, gy, z, 1); }
    // chimney in the west wall at the front room's row (the ground storey's row, carried up)
    const zfGround = (front === "jettied" ? D - 2 : D - 1) - 1;
    put(`chimney_${st}`, 0, gy, zfGround, 1);
    // room air
    for (let x = 1; x < W - 1; x++) for (let z = 1; z < zw; z++) for (let y = gy + 1; y < gy + SH; y++) g.set(ox + x, y, z, "air");
    // the street strip under a jetty
    if (st === "ground" && front === "jettied") for (let x = 0; x < W; x++) { g.set(ox + x, gy, D - 1, "cobblestone"); for (let y = gy + 1; y < gy + SH; y++) g.set(ox + x, y, D - 1, "air"); }
    // fit-out
    const vars = { W, D, c, g: gy, h: SH, zw, zf, storeys: S, trade: p.trade ? 1 : 0 };
    for (const f of rules.fitout[st]) {
      if (f.if && !evalExpr(f.if, vars)) continue;
      const [x, y, z] = f.at.map((e) => evalExpr(e, vars));
      if (f.part === "workstation") {
        const blk = p.trade === "any" ? "air" : p.trade;
        g.set(ox + x, y, z, blk); workstations.push({ at: [ox + x, y, z], block: p.trade === "any" ? "any" : `minecraft:${blk.replace(/^minecraft:|\[.*$/g, "")}` });
        continue;
      }
      if (f.part === "ladder") { late.push([x, y, z]); continue; }   // after the floor above, so it climbs through it
      put(f.part, x, y, z);
      if (f.part === "bed") beds.push({ head: [ox + x, y, z], foot: [ox + x, y, z + 1] });
      if (f.part === "lantern" && W >= 9) { put("lantern", c - 4, y, z); put("lantern", c + 4, y, z); }
    }
  }
  for (const [x, y, z] of late) put("ladder", x, y, z);
  // jetty: brackets and a lantern under the overhang
  if (front === "jettied") {
    frontBack(W, "front", "upper", front).forEach((k, x) => { if (k === "post_corner" || k === "stud") put("jetty_bracket", x, 1 + SH, D - 1); });
    put("lantern", c, 1 + SH, D - 1);
  }
  // roof
  if (!skipRoof) roof(kit, g, { W, D, roof: p.roof, roofBase, ox, ends: roofEnds });
  // chimney stack, one above the ridge
  const zfGround = (front === "jettied" ? D - 2 : D - 1) - 1;
  if (!skipRoof) for (let y = roofBase; y <= roofBase + layers; y++) put("chimney_stack", 0, y, zfGround);

  const ground = 2, zDoor = front === "jettied" ? D - 2 : D - 1;
  return { grid: g, beds, workstations, entrance: { at: [ox + c, ground + 1, zDoor], facing: "south", width: 1 }, height: roofBase + layers + 1,
    roofBase, layers, chimney: [ox, zfGround] };
}

// the roof over a W x D body at (ox, oz); eaves > 0 carries it that far past the walls (verge slices outside the gables)
export function roof(kit, g, { W, D, roof: kind, roofBase, ox = 0, oz = 0, ends = [true, true], eaves = 0 }) {
  const { part } = kit, e = eaves;
  if (kind === "gable_z") {
    for (let z = -e; z < D + e; z++) {
      const out = z < 0 || z >= D, end = (z === 0 && ends[0]) || (z === D - 1 && ends[1]);
      g.paste(part(`${out ? "roof_verge" : end ? "gable_end" : "roof_bay"}_${W + 2 * e}`), ox - e, roofBase, oz + z, { q: 1 });
    }
  } else {
    for (let x = -e; x < W + e; x++) {
      const out = x < 0 || x >= W, end = (x === 0 && ends[0]) || (x === W - 1 && ends[1]);
      g.paste(part(`${out ? "roof_verge" : end ? "gable_end" : "roof_bay"}_${D + 2 * e}`), ox + x, roofBase, oz - e);
    }
  }
}

/**
 * A home on a plot bigger than its body: the body set back (inset.side, inset.front, inset.back), the roof carried
 * out over the setback as eaves, the front strip dressed (a path to the door, window boxes, a bench, a woodpile, a lamp
 * post that lights it) and the side and back strips hedged. p.plot = [width, depth].
 */
export function assemblePlot(kit, p) {
  const [Wp, Dp] = p.plot, ins = { side: 1, front: 1, back: 1, ...(p.inset || {}) };
  const W = Wp - 2 * ins.side, D = Dp - ins.front - ins.back;
  const body = assembleHome(kit, { ...p, width: W, depth: D }, { skipRoof: true });
  const g = new Grid([Wp, body.height + 1, Dp]), ox = ins.side, oz = ins.back;
  // ground: dirt under, grass on top, then the body
  for (let x = 0; x < Wp; x++) for (let z = 0; z < Dp; z++) { g.set(x, 0, z, "dirt"); g.set(x, 1, z, "dirt"); g.set(x, 2, z, "grass_block[snowy=false]"); }
  g.paste(body.grid, ox, 0, oz);
  const e = Math.min(ins.side, ins.front, ins.back, 1);
  roof(kit, g, { W, D, roof: p.roof, roofBase: body.roofBase, ox, oz, eaves: e });
  const top = body.roofBase + body.layers + (p.roof === "gable_z" ? 0 : 0);
  for (let y = body.roofBase; y <= top; y++) g.set(ox + body.chimney[0], y, oz + body.chimney[1], "bricks");
  // hedges in the side and back strips, 2 high (no ground there a mob could stand on)
  const hedge = (x, z) => { g.set(x, 3, z, "oak_leaves[distance=1,persistent=true,waterlogged=false]"); g.set(x, 4, z, "oak_leaves[distance=1,persistent=true,waterlogged=false]"); };
  for (let z = 0; z < oz + D; z++) for (let x = 0; x < Wp; x++) if (x < ox || x >= ox + W || z < oz) hedge(x, z);
  // the front strip
  const c = ox + (W - 1) / 2, zs = oz + D;                       // the door's x; the first row in front of the body
  for (let z = zs; z < Dp; z++) g.set(c, 2, z, "dirt_path");
  const fronts = [];
  for (let x = ox + 1; x < ox + W - 1; x++) {
    const wall = g.get(x, 3 + 1, zs - 1) || "";
    if (/glass_pane/.test(wall) && x !== c) fronts.push(x);
  }
  for (const x of fronts) { g.set(x, 3, zs, "spruce_trapdoor[facing=south,half=top,open=false,powered=false,waterlogged=false]"); g.set(x, 4, zs, "potted_red_tulip"); }
  const free = [...Array(Wp).keys()].filter((x) => x !== c && !fronts.includes(x) && x > 0 && x < Wp - 1);
  if (free.length) g.set(free[0], 3, zs, "oak_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]");     // a bench
  if (free.length > 1) { const x = free.at(-1); g.set(x, 3, zs, "oak_log[axis=x]"); g.set(x, 4, zs, "oak_log[axis=x]"); } // a woodpile
  const lx = Math.min(c + 1, Wp - 1), lz = Dp - 1;                // a lamp post by the path, at the street edge
  if (!fronts.includes(lx) || lz !== zs) { g.set(lx, 3, lz, "spruce_fence[east=false,north=false,south=false,waterlogged=false,west=false]"); g.set(lx, 4, lz, "lantern[hanging=false,waterlogged=false]"); }
  for (let z = zs; z < Dp; z++) for (let x = 0; x < Wp; x++) if (x !== c && !g.get(x, 3, z)) {
    const k = (x * 7 + z * 3) % 5;                                // flowers, deterministic by cell
    if (k === 0) g.set(x, 3, z, "poppy"); else if (k === 2) g.set(x, 3, z, "oxeye_daisy"); else if (k === 4) g.set(x, 3, z, "cornflower");
  }
  const sh = (r) => r.map(([x, y, z]) => [x + ox, y, z + oz]);
  return { grid: g, height: body.height + 1, entrance: { at: [c, 3, zs - 1], facing: "south", width: 1 },
    beds: body.beds.map((b) => ({ head: sh([b.head])[0], foot: sh([b.foot])[0] })),
    workstations: body.workstations.map((w) => ({ ...w, at: sh([w.at])[0] })), body: [W, D] };
}

/** A kit built in memory at a given scale (for experiments), instead of from the part files. */
export function memoryKit(bundle, allParts, { storey = 4, pitch = 1, spans = [5, 7, 9, 11, 13, 15] } = {}) {
  const k = loadKit(bundle), parts = new Map(allParts({ storey, pitch, spans }).map((x) => [x.name, x.grid]));
  return { ...k, storey, part: (n) => { if (!parts.has(n)) throw new Error(`no part ${n}`); return parts.get(n); } };
}

/** A terrace: units side by side sharing party walls, one gable_x roof with gable ends on every party wall. */
export function assembleTerrace(kit, p) {
  const widths = p.units, W = widths.reduce((a, w) => a + w - 1, 1), D = p.depth, S = p.storeys;
  const roofBase = 2 + 4 * S, layers = (D - 1) / 2 + 1, g = new Grid([W, roofBase + layers + 1, D]);
  const beds = [], workstations = [], entrances = [];
  let ox = 0;
  for (const w of widths) {
    const r = assembleHome(kit, { ...p, width: w, roof: "gable_x" }, { into: g, ox });
    beds.push(...r.beds); workstations.push(...r.workstations); entrances.push(r.entrance);
    ox += w - 1;
  }
  return { grid: g, beds, workstations, entrances, height: roofBase + layers + 1 };
}
