const ORDER = ["north", "east", "south", "west"];
const turn = (dir, q) => (ORDER.includes(dir) ? ORDER[(ORDER.indexOf(dir) + q) % 4] : dir);
/** A block state turned q quarter turns clockwise (seen from above): facing, axis, sign rotation, side connections. */
export function rotateState(state, q) {
  if (!state || !(q % 4)) return state;
  const out = {};
  for (const [key, v] of Object.entries(state)) {
    if (ORDER.includes(key)) out[turn(key, q)] = v;                              // wire, pane, fence, wall sides
    else if (key === "facing" || key === "horizontal_facing") out[key] = turn(v, q);
    else if (key === "axis" && q % 2) out[key] = v === "x" ? "z" : v === "z" ? "x" : v;
    else if (key === "rotation") out[key] = String((+v + 4 * q) % 16);
    else out[key] = v;
  }
  return out;
}
