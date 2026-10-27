// Compose a house from module placements (a layout), and the reference layouts for the golden houses.
//
// A layout: { body: [W, D], storeys, party: [west, east], roof: gable_x|gable_z, core: straight|turned,
//   shell: [{ face, i, module, storey }],            face front|back|west|east, i = position along the face from the
//                                                    outside-left (corners at i = 0 and L-1 belong to front and back)
//   plan:  [{ storey, module, at: [x, z] }],         cores and fit-outs, body coordinates, x/z of the module's min corner
// }
// The composer turns each shell module onto its face, stacks storeys (layer 0 of a storey is its floor/sill), lays
// the foundation, the floors, the room air, the roof slices (gable ends on the walls, verges past them) and the stack.
import { Grid } from "./grid.mjs";
import { allModules, H, roofLayers } from "./modules.mjs";
import { readFileSync, existsSync } from "node:fs";

// the plan layer as data (town_kits/lordaeron/plan.json): body classes -> core, partitions, furniture per storey
const PLAN_FILE = new URL("../../../core/src/main/resources/data/enterprise/town_kits/lordaeron/plan.json", import.meta.url);
const PLAN = existsSync(PLAN_FILE) ? JSON.parse(readFileSync(PLAN_FILE, "utf8")) : null;
export const planBody = (W, D) => PLAN && Object.entries(PLAN.bodies).find(([, b]) => b.when.W === W && b.when.D === D);
const expr = (e, v) => (typeof e === "number" ? e : Function(...Object.keys(v), `"use strict"; return (${e});`)(...Object.values(v)));

// module libraries by set: Lordaeron's by default; another kingdom's shell family registers its own (same ids)
const LIBS = new Map(), SOURCES = new Map([["lordaeron", allModules]]);
export const registerSet = (name, modulesFn) => { SOURCES.set(name, modulesFn); LIBS.delete(name); };
let SET = "lordaeron";
const mod = (id) => {
  if (!LIBS.has(SET)) LIBS.set(SET, new Map(SOURCES.get(SET)().map((m) => [m.id, m])));
  const m = LIBS.get(SET).get(id); if (!m) throw new Error(`no module ${id} in ${SET}`); return m;
};

export function compose(L) {
  SET = L.set || "lordaeron";
  const [W, D] = L.body, S = L.storeys, party = L.party || [false, false];
  const rw = party[0] ? 0 : 1, re = party[1] ? 0 : 1, roofBase = 2 + H * S;
  // a hipped roof is one module over the whole relief outline, its eaves overhanging by `margin` all round
  const E = L.roof === "hip" ? L.margin || 0 : 0;
  const span = L.roof === "gable_x" ? D + 2 : W + rw + re, layers = L.roof === "hip" ? mod(`roof_hip_${W}x${D}`).grid.size[1] : roofLayers(span);
  const g = new Grid([W + rw + re + 2 * E, roofBase + layers + 1, D + 2 + 2 * E]);
  const X = (x) => x + rw + E, Z = (z) => z + 1 + E;
  const paste = (id, x, y, z, q = 0, mirror = false) => g.paste(mod(id).grid, X(x), y, Z(z), { q, mirror });

  // foundation, floors, room air
  for (let x = -rw; x < W + re; x++) for (let z = -1; z <= D; z++) paste("foundation", x, 0, z);
  for (let s = 0; s < S; s++) {
    const gy = 2 + H * s;
    for (let x = 1; x < W - 1; x++) for (let z = 1; z < D - 1; z++) { paste("floor", x, gy, z); paste("room_air", x, gy + 1, z); }
  }
  // shell: face anchors for a module of width w at position i (outside-left), relief outside the body line
  const anchor = { front: (i, w) => [i, D - 1, 0], back: (i, w) => [W - i - w, -1, 2], east: (i, w) => [W - 1, D - i - w, 3], west: (i, w) => [-1, i, 1] };
  for (const s of L.shell) {
    const m = mod(s.module), w = m.grid.size[0], party1 = m.grid.size[2] === 1, gy = 2 + H * s.storey;
    let [x, z, q] = anchor[s.face](s.i, w);
    if (party1) { if (s.face === "west") x = 0; if (s.face === "back") z = 0; }               // one layer: on the wall line
    if (m.id.startsWith("corner_party")) {                                                  // 1 x 2: wall + front relief
      if (s.face === "front") [x, z, q] = [s.i, D - 1, 0]; else [x, z, q] = [W - 1 - s.i, -1, 2];
    } else if (m.id.startsWith("corner")) {
      // the 2 x 2 corner, its wall cell on the body's corner: front-left (i = 0 on front) is the authored one
      const c = { "front:0": [-1, D - 1, 0], [`front:${W - 1}`]: [W - 1, D - 1, 3], "back:0": [W - 1, -1, 2], [`back:${W - 1}`]: [-1, -1, 1] }[`${s.face}:${s.i}`];
      [x, z, q] = c;
    }
    paste(s.module, x, gy, z, q, !!s.mirror);
  }
  // plan: cores and fit-outs (cores are placed after the floors, so their holes win)
  for (const p of L.plan) {
    if (p.block) g.set(X(p.at[0]), 2 + H * p.storey + (p.dy || 0), Z(p.at[1]), p.block);              // an exact block (a declared vanilla workstation)
    else paste(p.module, p.at[0], 2 + H * p.storey + (p.dy || 0), p.at[1], p.q || 0);
  }
  // roof slices over the relief (or the one hipped roof, eaves and all)
  if (L.roof === "hip") g.paste(mod(`roof_hip_${W}x${D}`).grid, X(-rw) - E, roofBase, Z(-1) - E);
  else if (L.roof === "gable_x") {
    for (let x = -rw; x < W + re; x++) {
      const kind = x < 0 || x >= W ? "verge" : (x === 0 && !party[0]) || (x === W - 1 && !party[1]) ? "gable" : "bay";
      paste(`roof_${kind}_${span}`, x, roofBase, -1, 0, x >= W);
    }
  } else {
    for (let z = -1; z <= D; z++) {
      const kind = z < 0 || z >= D ? "verge" : z === 0 || z === D - 1 ? "gable" : "bay";
      g.paste(mod(`roof_${kind}_${span}`).grid, E, roofBase, Z(z), { q: 1, mirror: z >= D });            // the far verge mirrored: its boards face the gable
    }
  }
  // the chimney stack over its breast, one above the ridge
  if (L.chimney && L.roof !== "hip") {
    const [cx, cz] = L.chimney, top = roofBase + layers - 1 + 1;
    for (let y = roofBase; y <= top; y++) party[0] && cx === 0 ? paste("chimney_stack_party", 0, y, cz) : g.paste(mod("chimney_stack").grid, X(cx) - 1, y, Z(cz), { q: 1 });
  }
  return g;
}

/** The reference layout for a house: what the solver should be able to find. p as house3's params. */
export function layoutOf(p) {
  p = { ...p };
  const W = p.W, D = p.D, S = p.storeys, party = p.party || [false, false];
  const PB = !p.fitout && planBody(W, D);
  if (PB && (S < PB[1].storeys[0] || S > PB[1].storeys[1])) throw new Error(`a ${W} x ${D} body (${PB[0]}) takes ${PB[1].storeys.join(" to ")} storeys, not ${S}`);
  if (PB && !PB[1].core) p = { ...p, noCore: true };
  const core = (PB && PB[1].core?.type) || p.core || (D >= 7 && W >= 13 ? "straight" : "turned"), nb = (W - 1) / 3;
  const doorBay = p.doorBay ?? Math.floor((nb - 1) / 2), mirrorDoor = p.door === "left";
  const shell = [], plan = [];
  const coreCells = new Set();
  if (p.noCore) {} else if (core === "straight") for (let z = 1; z <= 5; z++) { coreCells.add(`${W - 2},${z}`); coreCells.add(`${W - 3},${z}`); }
  else for (let x = W - 4; x <= W - 2; x++) for (let z = 1; z <= 3; z++) coreCells.add(`${x},${z}`);
  const behindStair = (xs, zs) => xs.some((x) => zs.some((z) => coreCells.has(`${x},${z}`)));
  const chimneyZ = D >= 10 ? 6 : 3, westChimney = !party[0] && D >= 7;
  // a corner house (p.corner = "east" | "west"): that side faces a street too, and is treated as a second front
  const street = (f) => f === "front" || f === p.corner;
  for (let s = 0; s < S; s++) {
    const st = s === 0 ? "ground" : "upper";
    for (const face of ["front", "back", "west", "east"]) {
      const Lf = face === "front" || face === "back" ? W : D, isParty = (face === "west" && party[0]) || (face === "east" && party[1]);
      for (let i = 0; i < Lf; i++) {
        const corner = i === 0 || i === Lf - 1;
        if (corner && (face === "west" || face === "east")) continue;
        let module;
        if (corner) {
          const partySide = (face === "front" && ((i === 0 && party[0]) || (i === Lf - 1 && party[1]))) || (face === "back" && ((i === 0 && party[1]) || (i === Lf - 1 && party[0])));
          module = partySide ? `corner_party_${st}` : `corner_${st}`;
        } else if (i % 3 === 0) {
          module = face === "west" && i === chimneyZ && D >= 7 ? (isParty ? `chimney_party_${st}` : `chimney_${st}`) : isParty ? `post_party_${st}` : `post_${st}`;
        } else if ((i % 3) === 1) {
          const bay = Math.floor(i / 3);
          // the room behind this bay: two interior cells along the face
          const cells = { front: [[i, D - 2], [i + 1, D - 2]], back: [[W - 1 - i, 1], [W - 2 - i, 1]], west: [[1, i], [1, i + 1]], east: [[W - 2, D - 1 - i], [W - 2, D - 2 - i]] }[face];
          const stair = cells.some(([x, z]) => coreCells.has(`${x},${z}`));
          if (isParty) module = `bay_party_${st}`;
          else if (p.arcade && s === 0 && (p.arcade !== "front" || face === "front")) module = "bay_arcade_ground";   // arcade "front": the street face only
          else if (face === "front" && s === 0 && bay === doorBay) module = "bay_door_ground";
          else if (stair) module = `bay_landing_${st}`;
          else if (street(face) && s === 0 && p.shop) module = "bay_shop_ground";
          else if (s === 0 && (face === "west" || face === "east") && face !== p.corner && !p.noCore) module = `bay_landing_${st}`;   // a single window on a ground side (a hall's sides get pairs)
          else if ((street(face) || (face === "back" && S === 1)) && (bay + s) % 2 === 0) module = `bay_window_box_${st}`;
          else module = `bay_window_${st}`;
          shell.push({ face, i, module, storey: s, mirror: module === "bay_door_ground" && mirrorDoor });
          continue;
        } else continue;                                                                      // the bay's second column
        shell.push({ face, i, module, storey: s });
      }
    }
  }
  // cores
  for (let s = 0; s < S && !p.noCore; s++) {
    const part = s === 0 ? "bottom" : s === S - 1 ? "top" : "mid";
    if (core === "straight") plan.push({ storey: s, module: `core_straight_${part}`, at: [W - 3, 1] });
    else plan.push({ storey: s, module: `core_turned_${part}`, at: [W - 4, 1] });
  }
  // fit-out: the plan layer's rooms and their furniture sets, by body width (rules, not one-off houses):
  //   every bed a made bed (a free long side), chambers off a landing (never through a bedroom), furniture by use,
  //   circulation kept clear from the street door to the stair. A 7-wide body holds one chamber floor (2 storeys at
  //   most); a 10-wide body splits each upper floor into a chamber and a 2-wide landing by the stair.
  const beds = [], workstations = [], backHearth = D < 7;
  const put = (storey, module, at, dy = 1, q = 0) => plan.push({ storey, module, at, dy, q });
  const bed = (storey, head, q = 0) => { put(storey, "bed", head, 1, q); beds.push({ storey, head, foot: q === 0 ? [head[0], head[1] + 1] : [head[0] - 1, head[1]] }); };
  const top = H - 1;
  const work = (at) => {
    if (!p.trade) return put(0, "shelf", at);
    plan.push({ storey: 0, module: "workstation", at, dy: 1, ...(p.trade !== "any" ? { block: p.trade } : {}) });   // "any": an air marker the placer fills
    workstations.push({ storey: 0, at });
  };
  // The furnishing rule (tuned with the shared floor-plan tool): a short spine from the street door to the stair is
  // kept clear; everything a villager walks to (hearth, chest, cask, shelf, workstation) stands beside the spine or
  // against a wall near it, so routes stay short; corners away from the spine take what nobody walks to (a table and
  // benches, plants), so no floor sits idle. Upper floors: made beds on the west wall with a chest between.
  if (p.fitout) p.fitout({ put, bed, W, D, S, top, work, doorX: doorBay * 3 + 1 + (p.door === "left" ? 0 : 1) });
  else if (PB) {
    for (let s2 = 0; s2 < S; s2++) {
      const list = PB[1].storey[String(s2)] || (s2 > 0 ? PB[1].storey.upper : null) || [];
      const v = { W, D, cx: W - 4, storeys: S, storey: s2, beds: p.beds || 2, trade: p.trade ? 1 : 0 };
      for (const e of list) {
        if (e.if != null && !expr(e.if, v)) continue;
        const at = e.at.map((c) => expr(c, v)), dy = e.dy ?? 1;
        if (e.role === "bed") bed(s2, at, e.q || 0);
        else if (e.role === "work") { if (p.trade) work(at); else if (e.else) put(s2, e.else, at, dy); }
        else put(s2, e.module, at, dy, e.q || 0);
      }
    }
  }
  else if (W < 10) {
    // 7 wide: a one-room cottage (the plan-layer rule: multi-storey starts at 10 wide). A 5 x 5 interior can't hold two
    // rooms each 3 wide behind a partition, so hall and chamber share the room: the hearth on the west wall with a
    // table and benches, two made beds against the back wall, a chest between them, the door's way kept clear.
    if (S > 1) throw new Error("a 7-wide body is single-storey (multi-storey starts at 10 wide)");
    bed(0, [1, 1]); bed(0, [W - 2, 1]); put(0, "chest", [3, 1]);
    put(0, "hearth", [1, chimneyZ]); put(0, "table", [3, 3]); put(0, "bench", [3, 2]); put(0, "bench", [4, 3], 1, 1);
    work([W - 2, 4]); put(0, "cask", [W - 2, 3]); put(0, "pot", [1, D - 2]);
    put(0, "lantern", [2, 3], top); put(0, "lantern", [4, 2], top);
  } else if (core === "turned") {
    // 10 wide: the hall below; above, a chamber to the west and a 2 x 2 landing at the stair's top behind an L partition
    const cx = W - 4;                                                    // the core's west column (its top step)
    // the hall; a clear 2-wide approach from the door along the front to the stair's foot (W-3..W-2, z 4..5)
    put(0, "hearth", [1, chimneyZ]); put(0, "table", [2, 3]); put(0, "bench", [2, 2]); put(0, "bench", [2, 4], 1, 2);
    work([4, D - 2]); put(0, "chest", [3, 4]); put(0, "cask", [cx, 4]); put(0, "shelf", [1, 1]); put(0, "shelf", [3, 1]); put(0, "pot", [1, D - 2]);
    put(0, "lantern", [2, 3], top); put(0, "lantern", [5, 3], top); put(0, "lantern", [cx + 1, 3], top);
    for (let s = 1; s < S; s++) {
      // the landing is the 2 x 2 inside the stair's corner (cx..cx+1, z 2..3): a partition west of it (door at z 2) and
      // south of it (z 4, from cx east); the next flight is entered from its side, so the front strip is the chamber's
      for (let z = 1; z <= 3; z++) put(s, z === 2 ? "door_inner" : "wall_inner", [cx - 1, z]);
      for (let x = cx; x <= W - 2; x++) put(s, "wall_inner", [x, 4]);
      if ((p.beds || 2) > 2) throw new Error("a 10-wide chamber floor holds 2 made beds (floor a bed 4 to 8.5; the partition door must stay clear)");
      bed(s, [1, 1]); bed(s, [1, 4]);
      put(s, "chest", [1, 3]); put(s, "table", [cx - 2, 1]); put(s, "bench", [cx - 3, 1], 1, 1); put(s, "chest", [W - 2, D - 2]);   // the table against the partition, so the front strip joins the chamber
      put(s, "lantern", [2, 3], top); put(s, "lantern", [cx, 2], top); put(s, "lantern", [W - 3, D - 2], top);
    }
  } else {
    // hall: door (5,5), spine along z 5 to the stair's foot (W-3, 5)
    put(0, "hearth", [1, chimneyZ]); put(0, "bench", [2, 2]); put(0, "table", [2, 3]); put(0, "bench", [2, 4], 1, 2);
    work([4, D - 2]); put(0, "chest", [6, 4]); put(0, "cask", [7, 4]); put(0, "shelf", [1, 1]);
    put(0, "table", [4, 2]); put(0, "bench", [4, 1], 1, 2); put(0, "bench", [5, 2], 1, 1); put(0, "pot", [1, D - 2]);
    put(0, "lantern", [2, 3], top); put(0, "lantern", [5, 3], top); put(0, "lantern", [W - 3, 2], top);
    // upper floors: a chamber x 1..W-6 behind a partition with a door, a 2-wide landing by the stair
    const wx = W - 5;
    for (let s = 1; s < S; s++) {
      for (let z = 1; z < D - 1; z++) put(s, z === 3 ? "door_inner" : "wall_inner", [wx, z]);
      bed(s, [1, 1]); if ((p.beds || 2) >= 2) bed(s, [1, 4]); if ((p.beds || 2) >= 3) bed(s, [wx - 1, 1]);
      put(s, "chest", [1, 3]); put(s, "table", [wx - 1, D - 3]); put(s, "bench", [wx - 1, D - 2], 1, 2);
      put(s, "lantern", [2, 3], top); put(s, "lantern", [W - 3, 3], top);
    }
  }
  const chimney = D >= 7 ? [0, chimneyZ] : [0, 1];
  return { body: [W, D], storeys: S, party, roof: p.roof, core, shell, plan, chimney, beds, workstations, doorBay, door: p.door || "right" };
}

/** Metadata (beds, workstations, entrance) for a composed layout, in template coordinates. */
export function metaOf(L) {
  const E = L.roof === "hip" ? L.margin || 0 : 0;
  const rw = (L.party || [])[0] ? 0 : 1, X = (x) => x + rw + E, Z = (z) => z + 1 + E;
  const doorX = L.doorBay * 3 + 1 + (L.door === "left" ? 0 : 1);
  return {
    entrance: { at: [X(doorX), 3, Z(L.body[1] - 1)], facing: "south", width: 1 },
    beds: L.beds.map((b) => ({ head: [X(b.head[0]), 3 + H * b.storey, Z(b.head[1])], foot: [X(b.foot[0]), 3 + H * b.storey, Z(b.foot[1])] })),
    workstations: L.workstations.map((w) => ({ at: [X(w.at[0]), 3 + H * w.storey, Z(w.at[1])], block: "any" })),
  };
}
