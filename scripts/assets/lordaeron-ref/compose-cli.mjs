#!/usr/bin/env node
// The reference composer for town_kits/lordaeron/timber_relief.json, from your repo root:
//   node scripts/assets/lordaeron-ref/compose-cli.mjs <golden .json> out.nbt     re-compose a golden house from its layouts (base palette)
//   node scripts/assets/lordaeron-ref/compose-cli.mjs layout.json out.nbt         compose any layout (the format in the set file)
import { readFileSync } from "node:fs";
import { compose } from "./compose.mjs";
import { Grid } from "./grid.mjs";
const [inp, out] = process.argv.slice(2), j = JSON.parse(readFileSync(inp, "utf8"));
const parts = j.golden ? j.golden.layouts : [{ at: [0, 0, 0], layout: j }];
const grids = parts.map((p) => ({ at: p.at, g: compose(p.layout) }));
const size = [Math.max(...grids.map((p) => p.at[0] + p.g.size[0])), Math.max(...grids.map((p) => p.g.size[1])), Math.max(...grids.map((p) => p.g.size[2]))];
const g = new Grid(size);
for (const p of grids) g.paste(p.g, ...p.at);
g.save(out);
console.log(JSON.stringify({ out, size }));
