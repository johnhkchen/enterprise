// node --test apps/asset-studio/tools/check-asset.test.mjs
// A 5 x 5 cottage written as a real structure file, then broken one rule at a time.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { gzipSync } from "node:zlib";
import { check } from "./check-asset.mjs";

// a tiny NBT writer, enough for a structure file, so this test runs in either repo
const t = { int: (v) => ({ id: 3, v }), string: (v) => ({ id: 8, v }), compound: (v) => ({ id: 10, v }), list: (et, v) => ({ id: 9, et, v }) };
const ET = { int: 3, compound: 10 };
function enc(tag) {
  const b = [], u16 = (n) => { const x = Buffer.alloc(2); x.writeUInt16BE(n); b.push(x); }, i32 = (n) => { const x = Buffer.alloc(4); x.writeInt32BE(n); b.push(x); };
  const str = (s) => { const x = Buffer.from(s, "utf8"); u16(x.length); b.push(x); };
  const payload = (id, v) => {
    if (id === 3) i32(v);
    else if (id === 8) str(v);
    else if (id === 10) { for (const [k, c] of Object.entries(v)) { b.push(Buffer.from([c.id])); str(k); payload(c.id, c.v ?? c); } b.push(Buffer.from([0])); }
  };
  const any = (c) => { if (c.id === 9) { b.push(Buffer.from([ET[c.et]])); i32(c.v.length); for (const e of c.v) ET[c.et] === 10 ? compound(e) : payload(ET[c.et], e.v ?? e); } else payload(c.id, c.v); };
  const compound = (o) => { for (const [k, c] of Object.entries(o)) { b.push(Buffer.from([c.id])); str(k); any(c); } b.push(Buffer.from([0])); };
  b.push(Buffer.from([10])); str(""); compound(tag.v);
  return gzipSync(Buffer.concat(b));
}
const write = enc;

const dir = mkdtempSync(join(tmpdir(), "check-asset-"));

// layers: 0-1 foundation, 2 floor, 3-5 walls, 6 roof. Door on the south face (z = 4) at x = 2.
function toFile(cells, size) {
  const palette = [...new Set(cells.values())], blocks = [];
  for (const [k, s] of cells) blocks.push({ pos: t.list("int", k.split(",").map(Number).map(t.int)), state: t.int(palette.indexOf(s)) });
  const pal = palette.map((s) => {
    const [, n, p] = s.match(/^([^[]+)(?:\[(.*)\])?$/);
    const o = { Name: t.string(n.includes(":") ? n : `minecraft:${n}`) };
    if (p) o.Properties = t.compound(Object.fromEntries(p.split(",").map((kv) => kv.split("=")).map(([k, v]) => [k, t.string(v)])));
    return o;
  });
  const root = t.compound({ DataVersion: t.int(3955), size: t.list("int", size.map(t.int)), palette: t.list("compound", pal),
    blocks: t.list("compound", blocks.map((b) => ({ pos: b.pos, state: b.state }))), entities: t.list("compound", []) });
  const f = join(dir, `c${Math.random().toString(36).slice(2)}.nbt`);
  writeFileSync(f, write(root));
  return f;
}

// layers: 0-1 foundation, 2 floor, 3-5 walls, 6 roof. Door on the south face (z = 4) at x = 2.
function cottage(edit = () => {}) {
  const cells = new Map(), put = (x, y, z, s) => cells.set(`${x},${y},${z}`, s);
  for (let x = 0; x < 5; x++) for (let z = 0; z < 5; z++) {
    put(x, 0, z, "cobblestone"); put(x, 1, z, "cobblestone"); put(x, 2, z, "oak_planks"); put(x, 6, z, "oak_slab[type=bottom]");
    for (let y = 3; y <= 5; y++) put(x, y, z, x === 0 || x === 4 || z === 0 || z === 4 ? "oak_planks" : "air");
  }
  put(2, 3, 4, "oak_door[facing=south,half=lower,hinge=left,open=false]"); put(2, 4, 4, "oak_door[facing=south,half=upper,hinge=left,open=false]");
  put(1, 3, 1, "white_bed[facing=north,part=head]"); put(1, 3, 2, "white_bed[facing=north,part=foot]");
  put(3, 3, 1, "white_bed[facing=north,part=head]"); put(3, 3, 2, "white_bed[facing=north,part=foot]");
  put(2, 5, 2, "lantern[hanging=true]");
  edit(put, cells);
  return toFile(cells, [5, 7, 5]);
}

// two storeys of 5 (floors at y 2 and 7), 7 x 5; a stair run along the back wall rising west from x 5 to x 1, the top
// step in the upper floor's layer; a bed on each floor. `steps` swaps the stairs for something else.
function house2(steps = "stairs") {
  const cells = new Map(), put = (x, y, z, s) => cells.set(`${x},${y},${z}`, s);
  for (let x = 0; x < 7; x++) for (let z = 0; z < 5; z++) {
    put(x, 0, z, "cobblestone"); put(x, 1, z, "cobblestone"); put(x, 2, z, "oak_planks"); put(x, 7, z, "oak_planks"); put(x, 12, z, "oak_planks");
    for (const y of [3, 4, 5, 6, 8, 9, 10, 11]) put(x, y, z, x === 0 || x === 6 || z === 0 || z === 4 ? "oak_planks" : "air");
  }
  put(3, 3, 4, "oak_door[facing=south,half=lower,hinge=left,open=false]"); put(3, 4, 4, "oak_door[facing=south,half=upper,hinge=left,open=false]");
  for (const x of [2, 3, 4]) put(x, 7, 1, "air");                                         // the stairwell in the upper floor
  [[5, 3], [4, 4], [3, 5], [2, 6], [1, 7]].forEach(([x, y]) => put(x, y, 1,
    steps === "stairs" ? "oak_stairs[facing=west,half=bottom,shape=straight]" : steps === "blocks" ? "oak_planks" : "air"));
  if (steps === "ladder") { for (let y = 3; y <= 7; y++) put(5, y, 1, "ladder[facing=west]"); put(5, 7, 1, "ladder[facing=west]"); for (const x of [1, 2, 3, 4]) put(x, 7, 1, "oak_planks"); }
  put(1, 3, 2, "white_bed[facing=north,part=head]"); put(1, 3, 3, "white_bed[facing=north,part=foot]");
  put(5, 8, 2, "white_bed[facing=north,part=head]"); put(5, 8, 3, "white_bed[facing=north,part=foot]");
  put(3, 6, 3, "lantern[hanging=true]"); put(3, 11, 3, "lantern[hanging=true]");
  return toFile(cells, [7, 13, 5]);
}
const meta2 = () => ({ footprint: [7, 5], entrances: [{ at: [3, 3, 4], facing: "south" }],
  beds: [{ head: [1, 3, 2], foot: [1, 3, 3] }, { head: [5, 8, 2], foot: [5, 8, 3] }], workstations: [], lockers: [], markers: [], entities: [] });

const meta = () => ({ footprint: [5, 5], entrances: [{ at: [2, 3, 4], facing: "south" }],
  beds: [{ head: [1, 3, 1], foot: [1, 3, 2] }, { head: [3, 3, 1], foot: [3, 3, 2] }], workstations: [], lockers: [], markers: [], entities: [] });

test("a good cottage passes", () => {
  const r = check(cottage(), meta());
  assert.deepEqual(r.errors, []);
  assert.equal(r.counts.beds, 2);
});

test("an undeclared job site fails", () => {
  const r = check(cottage((put) => put(3, 3, 3, "smithing_table")), meta());
  assert.match(r.errors.join("\n"), /job site smithing_table at 3,3,3 not declared/);
});

test("a chest against a declared job site is an undeclared locker", () => {
  const m = meta(); m.workstations = [{ at: [3, 3, 3], block: "minecraft:smithing_table" }];
  const r = check(cottage((put) => { put(3, 3, 3, "smithing_table"); put(3, 4, 3, "chest[facing=south]"); }), m);
  assert.match(r.errors.join("\n"), /chest at 3,4,3 touches the job site at 3,3,3/);
  m.lockers = [{ at: [3, 4, 3], for: [3, 3, 3] }];
  assert.ok(!check(cottage((put) => { put(3, 3, 3, "smithing_table"); put(3, 4, 3, "chest[facing=south]"); }), m).errors.some((e) => /locker/.test(e)));
});

test("a dark room fails", () => {
  const r = check(cottage((put) => put(2, 5, 2, "air")), meta());
  assert.match(r.errors.join("\n"), /walkable cells? under light 8/);
});

test("a walled-off bed fails the walk", () => {
  const r = check(cottage((put) => { for (let x = 1; x <= 3; x++) for (let y = 3; y <= 4; y++) put(x, y, 3, "oak_planks"); }), meta());
  assert.match(r.errors.join("\n"), /bed at 1,3,1 not reachable/);
});

test("blocks from after 1.21.1 fail", () => {
  const r = check(cottage((put) => put(0, 3, 0, "pale_oak_planks")), meta());
  assert.match(r.errors.join("\n"), /pale_oak_planks .* newer than 1.21.1/);
});

test("a mass's fixed block out of place fails", () => {
  const mass = { footprint: [5, 5], fixed: [{ at: [2, 3, 4], block: "minecraft:oak_door[facing=south,half=lower]", role: "door" },
    { at: [1, 3, 1], block: "minecraft:white_bed[facing=east,part=head]", role: "bed" }] };
  const r = check(cottage(), meta(), mass);
  assert.equal(r.errors.length, 1);
  assert.match(r.errors[0], /fixed bed .* at 1,3,1/);
});

test("a decorative twin is not a job site, and a chest against it is not a locker", () => {
  const r = check(cottage((put) => { put(3, 3, 3, "enterprise:decor_barrel[facing=up]"); put(3, 4, 3, "chest[facing=south]"); }), meta());
  assert.deepEqual(r.errors, []);
  assert.equal(r.counts.jobSites, 0);
});

test("stairs carry the walk to an upstairs bed", () => {
  assert.deepEqual(check(house2("stairs"), meta2()).errors, []);
});

test("a ladder or full-block steps don't", () => {
  for (const steps of ["ladder", "blocks"]) assert.match(check(house2(steps), meta2()).errors.join("\n"), /bed at 5,8,2 not reachable/, steps);
});

test("a potted plant is a block, not a cell to stand in", () => {
  const r = check(cottage((put) => { put(2, 5, 2, "air"); put(3, 3, 3, "potted_red_tulip"); put(2, 4, 2, "lantern[hanging=false]"); }), meta());
  assert.ok(!r.errors.some((e) => /3,4,3/.test(e)), r.errors.join("\n"));
});

test("a sunk door (a 1-high doorway) breaks the walk", () => {
  // the cottage's door moved down a block: its lower half in the floor layer, its upper half where the lower was
  const r = check(cottage((put) => { put(2, 2, 4, "oak_door[facing=south,half=lower,hinge=left,open=false]"); put(2, 3, 4, "oak_door[facing=south,half=upper,hinge=left,open=false]"); put(2, 4, 4, "oak_planks"); }),
    { ...meta(), entrances: [{ at: [2, 2, 4], facing: "south" }] });
  assert.match(r.errors.join("\n"), /not reachable/);
});
