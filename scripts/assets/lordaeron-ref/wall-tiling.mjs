// Tile one side of a town wall with the stone kit: corners at both ends (7 each, centred on the corner column, so 3
// beyond it), a gatehouse (11, its passage centred) on each avenue, interval towers (7) about every `towerEvery`
// blocks, curtains of 8 and 4 between, and fillers of 1-3 so the side closes exactly. Deterministic: the same side
// gives the same pieces.
//   side: length L (corner column to corner column), gates: positions along it of each avenue's centre line
//   -> [{ piece, at }] with `at` the piece's west end along the side (corner pieces at -3 and L-4)
export function tileSide(L, gates = [], { towerEvery = 24 } = {}) {
  const out = [{ piece: "corner_7x7", at: -3 }, { piece: "corner_7x7", at: L - 4 }];
  const fixed = gates.map((c) => ({ piece: "gatehouse_11x7", at: c - 5, len: 11 })).sort((a, b) => a.at - b.at);
  out.push(...fixed.map(({ piece, at }) => ({ piece, at })));
  // the runs between the fixed pieces
  const stops = [{ end: 4 }, ...fixed.map((f) => ({ start: f.at, end: f.at + f.len })), { start: L - 4 }];
  for (let i = 0; i + 1 < stops.length; i++) {
    let x = stops[i].end, gap = stops[i + 1].start - x;
    if (gap < 0) throw new Error(`pieces overlap at ${x}: move a gate`);
    // towers in long runs, centred in equal stretches
    const towers = Math.max(0, Math.floor((gap + 7) / (towerEvery + 7)));
    const stretch = (gap - towers * 7) / (towers + 1);
    for (let t = 0; t <= towers; t++) {
      let run = Math.round(stretch * (t + 1)) - Math.round(stretch * t);
      while (run >= 8) { out.push({ piece: "curtain_8", at: x }); x += 8; run -= 8; }
      while (run >= 4) { out.push({ piece: "curtain_4", at: x }); x += 4; run -= 4; }
      if (run > 0) { out.push({ piece: `filler_${run}`, at: x }); x += run; }
      if (t < towers) { out.push({ piece: "tower_7x7", at: x }); x += 7; }
    }
    if (x !== stops[i + 1].start) throw new Error(`run ended at ${x}, not ${stops[i + 1].start}`);
  }
  return out.sort((a, b) => a.at - b.at);
}
