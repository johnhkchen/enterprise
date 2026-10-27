// A sparse block grid that reads and writes vanilla structure files (DataVersion 3955, Minecraft 1.21.1).
// States are strings: "minecraft:spruce_door[facing=south,half=lower]". Cells left unset are not stored, so they
// act as structure_void (the world is left as it was).
import { readFileSync, writeFileSync } from "node:fs";
import { parse, simplify, write, t } from "./nbt.mjs";
import { rotateState } from "./rotate.mjs";

export const DATA_VERSION = 3955;
const key = (x, y, z) => `${x},${y},${z}`;

export function splitState(s) {
  const m = s.match(/^([^[]+)(?:\[(.*)\])?$/);
  const name = m[1].includes(":") ? m[1] : `minecraft:${m[1]}`;
  return { name, props: Object.fromEntries((m[2] || "").split(",").filter(Boolean).map((kv) => kv.split("="))) };
}
export const joinState = ({ name, props }) => {
  const p = Object.entries(props || {}).sort(([a], [b]) => a.localeCompare(b)).map(([k, v]) => `${k}=${v}`).join(",");
  return p ? `${name}[${p}]` : name;
};
/** A state turned q quarter turns clockwise seen from above. */
export const turnState = (s, q) => { const st = splitState(s); return joinState({ name: st.name, props: rotateState(st.props, q) }); };
/** A state mirrored east-west (x flips). */
export function mirrorState(s) {
  const st = splitState(s), p = { ...st.props };
  if (p.facing === "east") p.facing = "west"; else if (p.facing === "west") p.facing = "east";
  if (p.hinge) p.hinge = p.hinge === "left" ? "right" : "left";
  if (p.shape) p.shape = p.shape.replace("left", "@").replace("right", "left").replace("@", "right");
  if ("east" in p || "west" in p) [p.east, p.west] = [p.west, p.east];
  return joinState({ name: st.name, props: p });
}

export class Grid {
  constructor(size = [0, 0, 0]) { this.size = [...size]; this.cells = new Map(); this.entities = []; }
  set(x, y, z, s) { if (s == null) this.cells.delete(key(x, y, z)); else this.cells.set(key(x, y, z), s.includes(":") ? s : `minecraft:${s}`); return this; }
  get(x, y, z) { return this.cells.get(key(x, y, z)); }
  fill(x0, y0, z0, x1, y1, z1, s) { for (let x = x0; x <= x1; x++) for (let y = y0; y <= y1; y++) for (let z = z0; z <= z1; z++) this.set(x, y, z, s); return this; }
  *each() { for (const [k, s] of this.cells) { const [x, y, z] = k.split(",").map(Number); yield [x, y, z, s]; } }
  /** Paste another grid at (ox, oy, oz), turned q quarter turns about its own footprint and optionally mirrored first. */
  paste(g, ox, oy, oz, { q = 0, mirror = false, map = (s) => s } = {}) {
    const [w, , d] = g.size;
    for (let [x, y, z, s] of g.each()) {
      if (mirror) { x = w - 1 - x; s = mirrorState(s); }
      let [px, pz] = [x, z];
      for (let i = 0; i < ((q % 4) + 4) % 4; i++) [px, pz] = [(i % 2 ? w : d) - 1 - pz, px];   // clockwise about the footprint
      this.set(ox + px, oy + y, oz + pz, map(turnState(s, q)));
    }
    return this;
  }
  toNbt() {
    const states = [...new Set(this.cells.values())].sort(), idx = new Map(states.map((s, i) => [s, i]));
    const palette = states.map((s) => {
      const { name, props } = splitState(s), o = { Name: t.string(name) };
      if (Object.keys(props).length) o.Properties = t.compound(Object.fromEntries(Object.entries(props).map(([k, v]) => [k, t.string(String(v))])));
      return o;
    });
    const blocks = [...this.cells].sort(([a], [b]) => a.localeCompare(b, undefined, { numeric: true }))
      .map(([k, s]) => ({ pos: t.list("int", k.split(",").map(Number)), state: t.int(idx.get(s)) }));
    return write(t.compound({ DataVersion: t.int(DATA_VERSION), size: t.list("int", this.size),
      palette: t.list("compound", palette), blocks: t.list("compound", blocks), entities: t.list("compound", []) }));
  }
  save(file) { writeFileSync(file, this.toNbt()); return this; }
  static load(file) {
    const r = simplify(parse(readFileSync(file)).tag), g = new Grid(r.size);
    const pal = (r.palette || r.palettes[0]).map((p) => joinState({ name: p.Name, props: p.Properties || {} }));
    for (const b of r.blocks) g.cells.set(key(...b.pos), pal[b.state]);
    return g;
  }
}
