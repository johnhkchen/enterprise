#!/usr/bin/env node
// Check a delivery against Enterprise's asset contract before it leaves the studio: the same list their C14 will run,
// so they never see a build that breaks a town. One structure at a time, or a folder of them.
//
//   node tools/check-asset.mjs <name>.nbt [--meta <name>.json] [--mass <mass>.json] [--json]
//   node tools/check-asset.mjs <dir>                      every .nbt under it, each with the .json beside it
//
// Checks: DataVersion 3955; vanilla blocks or the mod's decorative twins (enterprise:decor_<vanilla id>); size = the JSON's footprint; no blocks from after 1.21.1 or forbidden ones; every bed,
// bell, job site and other point of interest declared, and nothing declared that isn't there; beds whole; chests and
// barrels touching a job site only as declared lockers; entities only frames, paintings and armour stands, listed;
// block light 8+ on every walkable cell a mob could spawn on (a full solid top: not bottom slabs, stairs, glass, leaves); a villager's walk from the door to every bed and workstation, up stairs and slab steps
// between floors (never ladders, trapdoors or full-block steps); from a mass,
// every fixed block at its cell and state. A part ({"kind": "part"}) skips the building rules (beds, light, walk). Light and walking are close approximations of the game's rules, so a pass
// here is a strong sign, and the game test on their side is the final word.
//
// Self-contained (Node 18+, no dependencies) so both sides can vendor the same file and judge a build the same way.
import { readFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { join, basename } from "node:path";
import { gunzipSync, inflateSync } from "node:zlib";

// a minimal NBT reader (Java edition, big-endian): compounds -> objects, lists -> arrays, longs -> BigInt
function readNbt(buf) {
  if (buf[0] === 0x1f && buf[1] === 0x8b) buf = gunzipSync(buf); else if (buf[0] === 0x78) buf = inflateSync(buf);
  let o = 0;
  const str = () => { const n = buf.readUInt16BE(o); o += 2; const s = buf.toString("utf8", o, o + n); o += n; return s; };
  const arr = (n, f) => Array.from({ length: n }, f);
  const read = (t) => {
    switch (t) {
      case 1: return buf.readInt8(o++);
      case 2: o += 2; return buf.readInt16BE(o - 2);
      case 3: o += 4; return buf.readInt32BE(o - 4);
      case 4: o += 8; return buf.readBigInt64BE(o - 8);
      case 5: o += 4; return buf.readFloatBE(o - 4);
      case 6: o += 8; return buf.readDoubleBE(o - 8);
      case 7: { const n = read(3); const v = [...buf.subarray(o, o + n)]; o += n; return v; }
      case 8: return str();
      case 9: { const et = buf[o++], n = read(3); return arr(n, () => read(et)); }
      case 10: { const c = {}; for (let k = buf[o++]; k !== 0; k = buf[o++]) { const name = str(); c[name] = read(k); } return c; }
      case 11: { const n = read(3); return arr(n, () => read(3)); }
      case 12: { const n = read(3); return arr(n, () => read(4)); }
      default: throw new Error(`nbt: tag ${t}`);
    }
  };
  if (buf[o++] !== 10) throw new Error("nbt: root is not a compound");
  str();
  return read(10);
}
// decorative twins in the mod: look like job sites, but no villager can claim them
const TWINS = /^enterprise:decor_(barrel|beehive|cauldron|composter|brewing_stand|smithing_table|fletching_table|cartography_table|lectern|smoker|blast_furnace|loom|stonecutter|grindstone)$/;
const shape = (n) => n && n.replace(/^enterprise:decor_/, "");          // a twin is shaped like its vanilla block

const DATA_VERSION = 3955;
const JOB = /^(barrel|blast_furnace|brewing_stand|cartography_table|(water_|lava_|powder_snow_)?cauldron|composter|fletching_table|grindstone|lectern|loom|smithing_table|smoker|stonecutter)$/;
const OTHER_POI = /^(beehive|bee_nest|lightning_rod|lodestone)$/;
const CONTAINER = /^(chest|trapped_chest|barrel)$/;
const FORBIDDEN = /^(structure_block|jigsaw|(chain_|repeating_)?command_block|spawner|trial_spawner|vault)$/;
// added after 1.21.1 (1.21.2 onward): a template saved with these won't load on the server
const NEWER = /^(pale_oak_.*|stripped_pale_oak_.*|pale_moss_.*|pale_hanging_moss|creaking_heart|resin_.*|chiseled_resin_bricks|(open|closed)_eyeblossom|potted_(open|closed)_eyeblossom|leaf_litter|wildflowers|bush|firefly_bush|cactus_flower|(short|tall)_dry_grass|dried_ghast|test_block|test_instance_block|.*_shelf|copper_(chest|golem_statue|bars|chain|lantern|torch)|.*_copper_(chest|golem_statue|bars|chain|lantern|torch))$/;
const ENTITY_OK = /^minecraft:(item_frame|glow_item_frame|painting|armor_stand)$/;
const LIGHT = { torch: 14, wall_torch: 14, lantern: 15, sea_lantern: 15, glowstone: 15, jack_o_lantern: 15, shroomlight: 15,
  soul_torch: 10, soul_wall_torch: 10, soul_lantern: 10, end_rod: 14, campfire: 15, soul_campfire: 10, lava: 15, fire: 15,
  ochre_froglight: 15, verdant_froglight: 15, pearlescent_froglight: 15, beacon: 15, conduit: 15, redstone_lamp: 15,
  crying_obsidian: 10, magma_block: 3, glow_lichen: 7, sculk_catalyst: 6, amethyst_cluster: 5, candle: 3, respawn_anchor: 15,
  cave_vines: 14, cave_vines_plant: 14, furnace: 13, smoker: 13, blast_furnace: 13, copper_bulb: 15, enchanting_table: 7 };
// blocks light passes through and villagers can stand inside (no collision), by name pattern
const OPEN = /(^air$|^cave_air$|^structure_void$|_sapling$|^(short_|tall_)?grass$|^fern$|^large_fern$|flower|tulip|^dandelion$|^poppy$|orchid|allium|bluet|daisy|^cornflower$|lily_of|rose|^peony$|^lilac$|^sunflower$|torch$|_sign$|_button$|_pressure_plate$|^redstone_wire$|^rail$|_rail$|^ladder$|^vine$|^tripwire|_carpet$|^moss_carpet$|^snow$|^light$|_banner$|^lever$|^cobweb$|^sugar_cane$|^wheat$|^carrots$|^potatoes$|^beetroots$|_stem$|^sweet_berry_bush$|^dead_bush$|^seagrass$|^water$|^lava$|^fire$|_mushroom$|^glow_lichen$|^hanging_roots$|^spore_blossom$|^nether_sprouts$)/;
// potted plants are flower pots (a block), whatever grows in them
const isOpen = (n) => !/^potted_|flower_pot$/.test(n || "") && OPEN.test(n || "");
// blocks light passes through but villagers can't walk into
const SEE_THROUGH = /(glass|_pane$|_bars$|^chain$|_fence$|_fence_gate$|_wall$|_door$|_trapdoor$|_leaves$|_slab$|_stairs$|lantern$|^bell$|_bed$|^flower_pot$|^potted_|^decorated_pot$|^candle|_candle$|^end_rod$|^lightning_rod$|^campfire$|^soul_campfire$|^anvil$|^chipped_anvil$|^damaged_anvil$|^brewing_stand$|^cauldron$|_cauldron$|^hopper$|^scaffolding$|^cake$|_head$|_skull$|^grindstone$|^lectern$|^stonecutter$|^enchanting_table$|^composter$|^chest$|^trapped_chest$|^ender_chest$|^daylight_detector$|^bamboo$|^big_dripleaf|^small_dripleaf|^pointed_dripstone$|^amethyst_cluster$|_amethyst_bud$|^conduit$|^beacon$|^turtle_egg$|^sea_pickle$|^kelp|^cave_vines|^twisting_vines|^weeping_vines|^azalea$|^flowering_azalea$|^honey_block$|^slime_block$|^ice$|^frosted_ice$|^cactus$|^iron_bars$)/;

const bare = (n) => n.replace(/^minecraft:/, "");
const key = (x, y, z) => `${x},${y},${z}`;
const propsOf = (s) => Object.fromEntries((s.match(/\[(.*)\]/)?.[1] || "").split(",").filter(Boolean).map((kv) => kv.split("=")));
const nameOf = (s) => bare(s.replace(/\[.*$/, ""));

function load(file) {
  const root = readNbt(readFileSync(file));
  const palette = root.palette || root.palettes?.[0] || [];
  const states = palette.map((p) => ({ name: bare(p.Name), props: p.Properties || {} }));
  const cells = new Map();
  for (const b of root.blocks || []) cells.set(key(...b.pos), states[b.state]);
  return { size: root.size, dataVersion: root.DataVersion, cells, entities: root.entities || [] };
}

export function check(nbtFile, meta, mass) {
  const errs = [], warns = [], s = load(nbtFile), [W, H, D] = s.size;
  // a part ({"kind": "part"} or {"slot": "part"}) is checked as a piece, not a building: version, blocks, points of
  // interest and lockers, but no beds, entrances, light or walk
  const part = meta?.kind === "part" || meta?.slot === "part";
  if (part) meta = { ...meta, beds: (meta.beds || []), workstations: meta.workstations || [], lockers: meta.lockers || [], markers: meta.markers || [], entities: meta.entities || [] };
  const at = (x, y, z) => s.cells.get(key(x, y, z));                // undefined = no block stored (acts as void)
  const name = (x, y, z) => at(x, y, z)?.name;

  if (s.dataVersion !== DATA_VERSION) errs.push(`DataVersion ${s.dataVersion}, not ${DATA_VERSION}`);
  // footprint: [W, D] (the whole structure stands on the ground), or { at, size } with an overhang (eaves past the ground
  // footprint, thread 0010): the ground part must lie inside the structure, the overhang round it
  if (Array.isArray(meta?.footprint) && (meta.footprint[0] !== W || meta.footprint[1] !== D)) errs.push(`size ${W} x ${D}, the JSON's footprint is ${meta.footprint.join(" x ")}`);
  if (meta?.footprint && !Array.isArray(meta.footprint)) {
    const { at = [0, 0], size = [] } = meta.footprint;
    if (at[0] < 0 || at[1] < 0 || at[0] + size[0] > W || at[1] + size[1] > D) errs.push(`footprint ${size.join(" x ")} at ${at.join(", ")} doesn't lie inside the ${W} x ${D} structure`);
  }

  // every block: forbidden, newer, points of interest
  const beds = [], jobs = [], bells = [], others = [], containers = [];
  for (const [k, b] of s.cells) {
    const p = k.split(",").map(Number);
    if (FORBIDDEN.test(b.name)) errs.push(`forbidden block ${b.name} at ${k}`);
    if (b.name.includes(":") && !TWINS.test(b.name)) errs.push(`${b.name} at ${k} is not vanilla or a known decorative twin`);
    if (NEWER.test(b.name)) errs.push(`${b.name} at ${k} is newer than 1.21.1`);
    if (b.name.endsWith("_bed")) beds.push({ p, b });
    else if (JOB.test(b.name)) jobs.push({ p, b });
    else if (b.name === "bell") bells.push(p);
    else if (OTHER_POI.test(b.name)) others.push({ p, b });
    if (CONTAINER.test(b.name)) containers.push({ p, b });
  }
  const same = (a, c) => a && c && a[0] === c[0] && a[1] === c[1] && a[2] === c[2];
  const listed = (arr, p) => (arr || []).some((x) => same(x.at || x, p));

  // beds: whole, and declared head + foot
  const heads = beds.filter((x) => x.b.props.part === "head");
  for (const { p, b } of heads) {
    const d = { north: [0, 0, 1], south: [0, 0, -1], east: [-1, 0, 0], west: [1, 0, 0] }[b.props.facing] || [0, 0, 0];
    const foot = [p[0] + d[0], p[1], p[2] + d[2]];
    if (at(...foot)?.name !== b.name || at(...foot)?.props.part !== "foot") errs.push(`bed at ${p} has no foot at ${foot}`);
    if (meta && !part && !(meta.beds || []).some((x) => same(x.head, p))) errs.push(`bed at ${p} not declared`);
  }
  for (const x of meta?.beds || []) if (!heads.some((h) => same(h.p, x.head))) errs.push(`declared bed head ${x.head} has no bed`);

  // job sites and other points of interest
  for (const { p, b } of jobs) if (meta && !listed(meta.workstations, p)) errs.push(`job site ${b.name} at ${p} not declared (villagers will claim it)`);
  for (const w of meta?.workstations || []) {
    const b = name(...w.at);
    if (w.block === "any") { if (b && b !== "air" && b !== "structure_void") warns.push(`workstation marker ${w.at} should be air for the placer, is ${b}`); }
    else if (b !== bare(w.block)) errs.push(`declared workstation ${w.block} at ${w.at} is ${b || "empty"}`);
  }
  for (const p of bells) if (meta && !(meta.markers || []).some((m) => m.what === "bell" && same(m.at, p))) errs.push(`bell at ${p}: bells are only ever markers`);
  for (const { p, b } of others) errs.push(`${b.name} at ${p} is a point of interest; avoid it`);

  // lockers: a container touching a job site on any face
  const N6 = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
  const jobCells = new Set([...jobs.map((j) => key(...j.p)), ...(meta?.workstations || []).map((w) => key(...w.at))]);
  for (const { p, b } of containers) for (const d of N6) {
    const q = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
    if (jobCells.has(key(...q)) && !(meta?.lockers || []).some((l) => same(l.at, p) && same(l.for, q))) errs.push(`${b.name} at ${p} touches the job site at ${q}: a locker, not declared`);
  }

  // entities
  const ents = s.entities.map((e) => e.nbt?.id || "?");
  for (const id of ents) if (!ENTITY_OK.test(id)) errs.push(`entity ${id} not allowed`);
  if (ents.length !== (meta?.entities || []).length) (ents.length ? errs : warns).push(`${ents.length} entities in the file, ${(meta?.entities || []).length} listed`);

  // light: block light flood fill from emitters through open and see-through cells (falls by 1 a step)
  const passLight = (n) => n === undefined || isOpen(shape(n)) || SEE_THROUGH.test(shape(n));
  const light = new Map(), queue = [];
  for (const [k, b] of s.cells) {
    let lv = LIGHT[b.name] ?? (b.name.endsWith("candle") ? 3 * (+b.props.candles || 1) : 0);
    if (b.props.lit === "false") lv = 0;
    if (lv) { light.set(k, lv); queue.push([...k.split(",").map(Number), lv]); }
  }
  while (queue.length) {
    const [x, y, z, lv] = queue.shift();
    if (lv <= 1) continue;
    for (const d of N6) {
      const q = [x + d[0], y + d[1], z + d[2]];
      if (q[0] < 0 || q[1] < 0 || q[2] < 0 || q[0] >= W || q[1] >= H || q[2] >= D) continue;
      if (!passLight(name(...q))) continue;
      const kq = key(...q);
      if ((light.get(kq) || 0) >= lv - 1) continue;
      light.set(kq, lv - 1); queue.push([...q, lv - 1]);
    }
  }

  // walkable: a cell a mob or villager can stand in: open itself and above, on something solid (stored, not open)
  const open = (x, y, z) => { const n = shape(name(x, y, z)); return n !== undefined && (isOpen(n) || /_door$|_fence_gate$/.test(n) && at(x, y, z).props.open === "true" || /_door$/.test(n)); };
  const standable = (x, y, z) => { const n = shape(name(x, y - 1, z)); return y > 0 && n !== undefined && !isOpen(n) && !/_fence$|_wall$|_fence_gate$|_door$/.test(n); };
  // a mob spawns only on a full, solid top: not on bottom slabs or stairs, glass, leaves and the like (top slabs and
  // upside-down stairs do count)
  const spawnable = (x, y, z) => {
    const b = at(x, y - 1, z), n = shape(b?.name);
    if (!n || isOpen(n)) return false;
    if (/_slab$/.test(n)) return b.props.type === "top" || b.props.type === "double";
    if (/_stairs$/.test(n)) return b.props.half === "top";
    return !SEE_THROUGH.test(n);
  };
  const walk = (x, y, z) => open(x, y, z) && (y + 1 >= H || open(x, y + 1, z) || name(x, y + 1, z) === undefined) && standable(x, y, z);
  // the contract's rule: interior cells (something above them) and the outdoor ground (the layer over `ground`).
  // Open-sky cells higher up (roofs, wall-walks) can spawn mobs at night too: those are a warning.
  const covered = (x, y, z) => { for (let yy = y + 2; yy < H; yy++) { const n = shape(name(x, yy, z)); if (n !== undefined && !isOpen(n)) return true; } return false; };
  const groundY = (meta?.ground ?? 0) + 1;
  const dark = [], roofs = [];
  for (let x = 0; x < W; x++) for (let y = 1; y < H; y++) for (let z = 0; z < D; z++) {
    const n = name(x, y, z);
    if (n === undefined || n === "structure_void" || /_door$/.test(n)) continue;
    if (!walk(x, y, z) || !spawnable(x, y, z) || (light.get(key(x, y, z)) || 0) >= 8) continue;
    (covered(x, y, z) || y <= groundY ? dark : roofs).push([x, y, z]);
  }
  if (part) dark.length = roofs.length = 0;
  if (dark.length) errs.push(`${dark.length} walkable cell${dark.length > 1 ? "s" : ""} under light 8, e.g. ${dark.slice(0, 5).map((p) => `(${p})`).join(" ")}`);
  if (roofs.length) warns.push(`${roofs.length} open-sky cell${roofs.length > 1 ? "s" : ""} above the ground (roofs, ledges) a mob could spawn on at night, e.g. ${roofs.slice(0, 3).map((p) => `(${p})`).join(" ")}`);

  // reach: from each entrance, a villager's walk between floors: up only by half steps (onto a stair, or onto a bottom
  // slab, or off a bottom slab), down any drop of 3 or less; ladders, trapdoors and full-block steps break the route
  // a door cell is passable only as a doorway: the lower half here, the upper half over it, something to stand on
  const doorway = (x, y, z) => {
    const lo = at(x, y, z), hi = at(x, y + 1, z), below = shape(name(x, y - 1, z));
    return !!lo && /_door$/.test(lo.name) && lo.props.half === "lower" && !!hi && /_door$/.test(hi.name) && hi.props.half === "upper" && !!below && !isOpen(below);
  };
  const halfStep = (x, y, z, p) => {
    const to = at(p[0], p[1] - 1, p[2]), from = at(x, y - 1, z);
    const tn = shape(to?.name) || "", fn = shape(from?.name) || "";
    if (/_stairs$/.test(tn)) return true;
    if (/_slab$/.test(tn) && to.props.type === "bottom") return true;
    return /_slab$/.test(fn) && from.props.type === "bottom";
  };
  if (meta && !part) {
    const starts = (meta.entrances || []).map((e) => e.at);
    if (!starts.length) warns.push("no entrances declared");
    const seen = new Set(), q = [];
    for (const p of starts) { seen.add(key(...p)); q.push(p); }
    while (q.length) {
      const [x, y, z] = q.shift();
      for (const [dx, dz] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) for (const dy of [0, 1, -1, -2, -3]) {
        const p = [x + dx, y + dy, z + dz];
        if (p[0] < 0 || p[2] < 0 || p[0] >= W || p[2] >= D || p[1] < 0 || p[1] >= H) continue;
        if (dy === 1 && !open(x, y + 2, z) && name(x, y + 2, z) !== undefined) continue;   // headroom to step up
        if (dy === 1 && !halfStep(x, y, z, p)) continue;                                    // a full block is a jump: no
        if (!walk(...p) && !doorway(...p)) continue;
        const k = key(...p);
        if (!seen.has(k)) { seen.add(k); q.push(p); }
        break;
      }
    }
    const reachable = (p) => N6.some((d) => [0, -1].some((dy) => seen.has(key(p[0] + d[0], p[1] + d[1] + dy, p[2] + d[2]))));
    for (const bd of meta.beds || []) if (!reachable(bd.head) && !reachable(bd.foot)) errs.push(`bed at ${bd.head} not reachable from the door`);
    for (const w of meta.workstations || []) if (!reachable(w.at)) errs.push(`workstation at ${w.at} not reachable from the door`);
    for (const e of meta.entrances || []) if (e.facing !== "south") warns.push(`entrance at ${e.at} faces ${e.facing}; fronts face south`);
  }

  // mass: every fixed block exactly
  if (mass) {
    for (const f of mass.fixed || []) {
      const b = at(...f.at), want = nameOf(f.block), wp = propsOf(f.block);
      if (!b || b.name !== want || Object.entries(wp).some(([k, v]) => b.props[k] !== v)) errs.push(`fixed ${f.role} ${f.block} at ${f.at} is ${b ? b.name + JSON.stringify(b.props) : "empty"}`);
    }
    if (mass.footprint && (mass.footprint[0] !== W || mass.footprint[1] !== D)) errs.push(`size ${W} x ${D}, the mass is ${mass.footprint.join(" x ")}`);
  }

  return { file: nbtFile, size: s.size, blocks: s.cells.size, ok: !errs.length, errors: errs, warnings: warns,
    counts: { beds: heads.length, jobSites: jobs.length, containers: containers.length, entities: ents.length, unlit: dark.length, openSky: roofs.length } };
}

// ---- cli
if (import.meta.url === `file://${process.argv[1]}`) {
  const argv = process.argv.slice(2), opt = (k) => (argv.includes(k) ? argv[argv.indexOf(k) + 1] : null);
  const target = argv.find((a) => !a.startsWith("--") && a !== opt("--meta") && a !== opt("--mass"));
  if (!target) { console.log(readFileSync(new URL(import.meta.url), "utf8").split("\n").slice(1, 15).map((l) => l.slice(3)).join("\n")); process.exit(1); }
  const files = statSync(target).isDirectory()
    ? (function walk(d) { return readdirSync(d).flatMap((f) => { const p = join(d, f); return statSync(p).isDirectory() ? walk(p) : p.endsWith(".nbt") ? [p] : []; }); })(target)
    : [target];
  const readJson = (p) => (p && existsSync(p) ? JSON.parse(readFileSync(p, "utf8")) : null);
  // A palette variant's metadata ({id, base, palette}) is its base's (the base beside it), with its own id and palette.
  const metaOf = (p) => {
    const m = readJson(p);
    if (!m || !m.base || m.footprint) return m;
    const base = readJson(p.replace(/[^/]*$/, m.base.split("/").pop() + ".json"));
    return base ? { ...base, id: m.id, palette: m.palette } : m;
  };
  const results = files.map((f) => check(f, metaOf(opt("--meta") || f.replace(/\.nbt$/, ".json")), readJson(opt("--mass"))));
  if (argv.includes("--json")) console.log(JSON.stringify(results, null, 2));
  else for (const r of results) {
    console.log(`${r.ok ? "PASS" : "FAIL"}  ${basename(r.file)}  ${r.size.join("x")}, ${r.blocks} blocks, ${r.counts.beds} beds, ${r.counts.jobSites} job sites`);
    for (const e of r.errors) console.log(`  error: ${e}`);
    for (const w of r.warnings) console.log(`  warn:  ${w}`);
  }
  process.exit(results.every((r) => r.ok) ? 0 : 1);
}
