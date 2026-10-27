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

let LIB = null;
const mod = (id) => { LIB ||= new Map(allModules().map((m) => [m.id, m])); const m = LIB.get(id); if (!m) throw new Error(`no module ${id}`); return m; };

export function compose(L) {
  const [W, D] = L.body, S = L.storeys, party = L.party || [false, false];
  const rw = party[0] ? 0 : 1, re = party[1] ? 0 : 1, roofBase = 2 + H * S;
  const span = L.roof === "gable_x" ? D + 2 : W + rw + re, layers = roofLayers(span);
  const g = new Grid([W + rw + re, roofBase + layers + 1, D + 2]);
  const X = (x) => x + rw, Z = (z) => z + 1;
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
  for (const p of L.plan) paste(p.module, p.at[0], 2 + H * p.storey + (p.dy || 0), p.at[1], p.q || 0);
  // roof slices over the relief
  if (L.roof === "gable_x") {
    for (let x = -rw; x < W + re; x++) {
      const kind = x < 0 || x >= W ? "verge" : (x === 0 && !party[0]) || (x === W - 1 && !party[1]) ? "gable" : "bay";
      paste(`roof_${kind}_${span}`, x, roofBase, -1);
    }
  } else {
    for (let z = -1; z <= D; z++) {
      const kind = z < 0 || z >= D ? "verge" : z === 0 || z === D - 1 ? "gable" : "bay";
      g.paste(mod(`roof_${kind}_${span}`).grid, 0, roofBase, Z(z), { q: 1 });
    }
  }
  // the chimney stack over its breast, one above the ridge
  if (L.chimney) {
    const [cx, cz] = L.chimney, top = roofBase + layers - 1 + 1;
    for (let y = roofBase; y <= top; y++) party[0] && cx === 0 ? paste("chimney_stack_party", 0, y, cz) : g.paste(mod("chimney_stack").grid, X(cx) - 1, y, Z(cz), { q: 1 });
  }
  return g;
}

/** The reference layout for a house: what the solver should be able to find. p as house3's params. */
export function layoutOf(p) {
  const W = p.W, D = p.D, S = p.storeys, party = p.party || [false, false];
  const core = p.core || (D >= 7 && W >= 10 ? "straight" : "turned"), nb = (W - 1) / 3;
  const doorBay = p.doorBay ?? Math.floor((nb - 1) / 2), mirrorDoor = p.door === "left";
  const shell = [], plan = [];
  const coreCells = new Set();
  if (core === "straight") for (let z = 1; z <= 5; z++) { coreCells.add(`${W - 2},${z}`); coreCells.add(`${W - 3},${z}`); }
  else for (let x = W - 4; x <= W - 2; x++) for (let z = 1; z <= 3; z++) coreCells.add(`${x},${z}`);
  const behindStair = (xs, zs) => xs.some((x) => zs.some((z) => coreCells.has(`${x},${z}`)));
  const chimneyZ = 3, westChimney = !party[0] && D >= 7;
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
          module = face === "west" && westChimney && i === chimneyZ ? `chimney_${st}` : isParty ? `post_party_${st}` : `post_${st}`;
        } else if ((i % 3) === 1) {
          const bay = Math.floor(i / 3);
          // the room behind this bay: two interior cells along the face
          const cells = { front: [[i, D - 2], [i + 1, D - 2]], back: [[W - 1 - i, 1], [W - 2 - i, 1]], west: [[1, i], [1, i + 1]], east: [[W - 2, D - 1 - i], [W - 2, D - 2 - i]] }[face];
          const stair = cells.some(([x, z]) => coreCells.has(`${x},${z}`));
          if (isParty) module = `bay_party_${st}`;
          else if (face === "front" && s === 0 && bay === doorBay) module = "bay_door_ground";
          else if (stair) module = `bay_landing_${st}`;
          else if (face === "front" && s === 0 && p.shop) module = "bay_shop_ground";
          else if (s === 0 && (face === "west" || face === "east")) module = `bay_landing_${st}`;       // a single window on a ground side
          else if (face === "front" && (bay + s) % 2 === 0) module = `bay_window_box_${st}`;
          else module = `bay_window_${st}`;
          shell.push({ face, i, module, storey: s, mirror: module === "bay_door_ground" && mirrorDoor });
          continue;
        } else continue;                                                                      // the bay's second column
        shell.push({ face, i, module, storey: s });
      }
    }
  }
  // cores
  for (let s = 0; s < S; s++) {
    const part = s === 0 ? "bottom" : s === S - 1 ? "top" : "mid";
    if (core === "straight") plan.push({ storey: s, module: `core_straight_${part}`, at: [W - 3, 1] });
    else plan.push({ storey: s, module: `core_turned_${part}`, at: [W - 4, 1] });
  }
  // fit-out
  const beds = [], workstations = [];
  const backHearth = party[0] || D < 7;
  plan.push({ storey: 0, module: "hearth", at: [1, backHearth ? 1 : chimneyZ], dy: 1 });
  const ws = backHearth ? [2, 1] : [1, 1];
  if (p.trade) { plan.push({ storey: 0, module: "workstation", at: ws, dy: 1 }); workstations.push({ storey: 0, at: ws }); }
  else plan.push({ storey: 0, module: "shelf", at: ws, dy: 1 });
  const slots = core === "straight" ? [[1, 1], [3, 1], [1, 4], [3, 4], [5, 1], [5, 4]] : [[1, 1], [2, 1]];
  for (let s = 1; s < S; s++) {
    let n = 0;
    for (const [x, z] of slots) {
      if (n >= (p.beds || (core === "straight" ? 3 : 2))) break;
      if (x >= W - 1 || z + 1 >= D - 1 || coreCells.has(`${x},${z}`) || coreCells.has(`${x},${z + 1}`)) continue;
      plan.push({ storey: s, module: "bed", at: [x, z], dy: 1 }); beds.push({ storey: s, head: [x, z] }); n++;
    }
  }
  for (let s = 0; s < S; s++) {
    plan.push({ storey: s, module: "lantern", at: [2, D - 2], dy: H - 1 });
    plan.push({ storey: s, module: "lantern", at: core === "straight" ? [W - 3, 3] : [W - 3, 2], dy: H - 1 });
    if (W >= 10) { plan.push({ storey: s, module: "lantern", at: [5, 3], dy: H - 1 }); plan.push({ storey: s, module: "lantern", at: [2, 2], dy: H - 1 }); }
  }
  const chimney = westChimney ? [0, chimneyZ] : [0, 1];
  return { body: [W, D], storeys: S, party, roof: p.roof, core, shell, plan, chimney, beds, workstations, doorBay, door: p.door || "right" };
}

/** Metadata (beds, workstations, entrance) for a composed layout, in template coordinates. */
export function metaOf(L) {
  const rw = (L.party || [])[0] ? 0 : 1, X = (x) => x + rw, Z = (z) => z + 1;
  const doorX = L.doorBay * 3 + 1 + (L.door === "left" ? 0 : 1);
  return {
    entrance: { at: [X(doorX), 3, Z(L.body[1] - 1)], facing: "south", width: 1 },
    beds: L.beds.map((b) => ({ head: [X(b.head[0]), 3 + H * b.storey, Z(b.head[1])], foot: [X(b.head[0]), 3 + H * b.storey, Z(b.head[1] + 1)] })),
    workstations: L.workstations.map((w) => ({ at: [X(w.at[0]), 3 + H * w.storey, Z(w.at[1])], block: "any" })),
  };
}
