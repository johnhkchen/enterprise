#!/usr/bin/env node
// The reference assembler for town_kits/lordaeron/timber.json, from your repo root:
//   node scripts/assets/lordaeron-ref/cli.mjs '{"width":5,"depth":5,"storeys":1,"roof":"gable_z","palette":"daub","front":"plain"}' out.nbt
//   node scripts/assets/lordaeron-ref/cli.mjs '{"units":[5,5,5],"depth":5,"storeys":2,"palette":"brick","front":"plain"}' out.nbt   (a terrace)
// It reads the rules and the part files from the repo (core/src/main/resources/data/enterprise/...) and the palettes
// from assets-src/kingdoms/lordaeron/palettes.json, and writes the assembled structure. Diff your C16's output against it.
import { loadKit, assembleHome, assembleTerrace, paletteMap } from "./assemble.mjs";
import { Grid } from "./grid.mjs";
const [params, out] = process.argv.slice(2);
if (!params || !out) { console.error("cli.mjs '<params json>' <out.nbt>"); process.exit(1); }
const p = JSON.parse(params), kit = loadKit(process.cwd());
const r = p.units ? assembleTerrace(kit, p) : assembleHome(kit, p), map = paletteMap(kit.palettes, p.palette || "stone");
const g = new Grid(r.grid.size);
for (const [x, y, z, s] of r.grid.each()) g.set(x, y, z, map(s));
g.save(out);
console.log(JSON.stringify({ out, size: g.size, beds: r.beds, workstations: r.workstations, entrances: r.entrances || [r.entrance] }));
