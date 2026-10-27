package dev.b28.enterprise.territory;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.BitSet;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Random;
import java.util.Set;
import java.util.function.BiPredicate;

/**
 * Wave-function collapse over a 3D grid of cells (John: "play around with some wave function collapse style procedural
 * generation of buildings"): each cell takes one module of a set, and two modules may stand side by side only where
 * their facing sockets meet ({@link #fits}). The studio designs the modules and their sockets (`docs/assets` thread 0004:
 * facade bays, posts, roof modules, room tiles); this finds an arrangement on a grid of any size that keeps every rule,
 * the least-decided cell first, choosing among what's left by weight, deterministic for a seed. Plain data in, plain
 * data out: no world.
 *
 * <p>Sockets are labels; {@code a|b} means either. Two faces meet if a label of one equals a label of the other, or a
 * set's {@code pairs} joins them. Outside the grid, a face meets the set's {@code edge} label for that side (any label
 * if none is given).
 */
public final class Wfc {
    private Wfc() {}

    /** The six sides of a cell: west (-x), east (+x), down (-y), up (+y), north (-z), south (+z). */
    public enum Side {
        WEST(-1, 0, 0), EAST(1, 0, 0), DOWN(0, -1, 0), UP(0, 1, 0), NORTH(0, 0, -1), SOUTH(0, 0, 1);

        final int dx, dy, dz;

        Side(int dx, int dy, int dz) {
            this.dx = dx;
            this.dy = dy;
            this.dz = dz;
        }

        public Side opposite() {
            return values()[ordinal() ^ 1];
        }
    }

    /** A module: its id, its socket on each side (absent: any), and how often it's chosen among those allowed. */
    public record Module(String id, Map<Side, String> sockets, double weight) {
        String socket(Side side) {
            return sockets.getOrDefault(side, "*");
        }
    }

    /** A set: its modules, the socket labels it joins besides equal ones, and what lies beyond the grid on each side. */
    public record Rules(List<Module> modules, Set<List<String>> pairs, Map<Side, String> edge) {}

    /** Do two faces meet: a label of one equals or is paired with a label of the other ({@code *}: any)? */
    static boolean fits(String a, String b, Set<List<String>> pairs) {
        for (String x : a.split("\\|")) {
            for (String y : b.split("\\|")) {
                if (x.equals("*") || y.equals("*") || x.equals(y) || pairs.contains(List.of(x, y))
                        || pairs.contains(List.of(y, x))) {
                    return true;
                }
            }
        }
        return false;
    }

    /**
     * An arrangement of the rules' modules on an {@code nx} x {@code ny} x {@code nz} grid: each cell's module index
     * (cell {@code (x, y, z)} at {@code (y * nz + z) * nx + x}). {@code allowed} says which modules a cell may take (a
     * door only on the ground floor's front, a room tile by its storey); {@code fixed} pins cells to a module (the
     * door, the stair core). Tries up to {@code tries} seeds from {@code seed} on a contradiction; empty if none works.
     */
    public static Optional<int[]> solve(Rules rules, int nx, int ny, int nz, BiPredicate<int[], Module> allowed,
            Map<Integer, Integer> fixed, long seed, int tries) {
        int m = rules.modules().size();
        // Which modules may stand beside each, on each side.
        BitSet[][] compat = new BitSet[6][m];
        for (Side side : Side.values()) {
            for (int a = 0; a < m; a++) {
                BitSet ok = new BitSet(m);
                for (int b = 0; b < m; b++) {
                    if (fits(rules.modules().get(a).socket(side), rules.modules().get(b).socket(side.opposite()),
                            rules.pairs())) {
                        ok.set(b);
                    }
                }
                compat[side.ordinal()][a] = ok;
            }
        }
        for (int t = 0; t < tries; t++) {
            Optional<int[]> out = attempt(rules, compat, nx, ny, nz, allowed, fixed, new Random(seed + 7919L * t));
            if (out.isPresent()) {
                return out;
            }
        }
        return Optional.empty();
    }

    private static Optional<int[]> attempt(Rules rules, BitSet[][] compat, int nx, int ny, int nz,
            BiPredicate<int[], Module> allowed, Map<Integer, Integer> fixed, Random random) {
        int m = rules.modules().size(), n = nx * ny * nz;
        BitSet[] domain = new BitSet[n];
        ArrayDeque<Integer> queue = new ArrayDeque<>();
        for (int c = 0; c < n; c++) {
            int[] at = at(c, nx, nz);
            BitSet d = new BitSet(m);
            for (int k = 0; k < m; k++) {
                Module mod = rules.modules().get(k);
                if (!allowed.test(at, mod)) {
                    continue;
                }
                // At the grid's edge, only what meets what lies beyond it.
                boolean ok = true;
                for (Side side : Side.values()) {
                    int x = at[0] + side.dx, y = at[1] + side.dy, z = at[2] + side.dz;
                    if ((x < 0 || y < 0 || z < 0 || x >= nx || y >= ny || z >= nz)
                            && !fits(mod.socket(side), rules.edge().getOrDefault(side, "*"), rules.pairs())) {
                        ok = false;
                        break;
                    }
                }
                if (ok) {
                    d.set(k);
                }
            }
            Integer pin = fixed.get(c);
            if (pin != null) {
                boolean had = d.get(pin);
                d.clear();
                if (had) {
                    d.set(pin);
                }
            }
            if (d.isEmpty()) {
                return Optional.empty();
            }
            domain[c] = d;
            queue.add(c);
        }
        if (!propagate(domain, compat, queue, nx, ny, nz)) {
            return Optional.empty();
        }
        while (true) {
            // The least-decided cell (fewest modules left), ties broken by the seed.
            int best = -1, fewest = Integer.MAX_VALUE;
            double tie = 0;
            for (int c = 0; c < n; c++) {
                int k = domain[c].cardinality();
                if (k > 1) {
                    double r = random.nextDouble();
                    if (k < fewest || k == fewest && r < tie) {
                        best = c;
                        fewest = k;
                        tie = r;
                    }
                }
            }
            if (best < 0) {
                break;
            }
            // One of its modules, by weight.
            double total = 0;
            for (int k = domain[best].nextSetBit(0); k >= 0; k = domain[best].nextSetBit(k + 1)) {
                total += Math.max(0, rules.modules().get(k).weight());
            }
            double pick = random.nextDouble() * total;
            int chosen = domain[best].nextSetBit(0);
            for (int k = domain[best].nextSetBit(0); k >= 0; k = domain[best].nextSetBit(k + 1)) {
                pick -= Math.max(0, rules.modules().get(k).weight());
                if (pick <= 0) {
                    chosen = k;
                    break;
                }
            }
            domain[best].clear();
            domain[best].set(chosen);
            queue.add(best);
            if (!propagate(domain, compat, queue, nx, ny, nz)) {
                return Optional.empty();
            }
        }
        int[] out = new int[n];
        for (int c = 0; c < n; c++) {
            out[c] = domain[c].nextSetBit(0);
        }
        return Optional.of(out);
    }

    /** Each cell's modules narrowed to those some module of each neighbour's can stand beside; false on a cell left with none. */
    private static boolean propagate(BitSet[] domain, BitSet[][] compat, ArrayDeque<Integer> queue, int nx, int ny, int nz) {
        Set<Integer> queued = new HashSet<>(queue);
        while (!queue.isEmpty()) {
            int c = queue.poll();
            queued.remove(c);
            int[] at = at(c, nx, nz);
            for (Side side : Side.values()) {
                int x = at[0] + side.dx, y = at[1] + side.dy, z = at[2] + side.dz;
                if (x < 0 || y < 0 || z < 0 || x >= nx || y >= ny || z >= nz) {
                    continue;
                }
                int nb = (y * nz + z) * nx + x;
                BitSet reach = new BitSet();
                for (int k = domain[c].nextSetBit(0); k >= 0; k = domain[c].nextSetBit(k + 1)) {
                    reach.or(compat[side.ordinal()][k]);
                }
                BitSet before = (BitSet) domain[nb].clone();
                domain[nb].and(reach);
                if (domain[nb].isEmpty()) {
                    return false;
                }
                if (!domain[nb].equals(before) && queued.add(nb)) {
                    queue.add(nb);
                }
            }
        }
        return true;
    }

    /** A cell's (x, y, z). */
    public static int[] at(int cell, int nx, int nz) {
        return new int[] {cell % nx, cell / (nx * nz), cell / nx % nz};
    }

    /** A cell's index. */
    public static int cell(int x, int y, int z, int nx, int nz) {
        return (y * nz + z) * nx + x;
    }

    /** Does every pair of neighbouring cells in an arrangement meet by the rules? (For tests and the studio's sheets.) */
    public static boolean valid(Rules rules, int[] grid, int nx, int ny, int nz) {
        for (int c = 0; c < grid.length; c++) {
            int[] at = at(c, nx, nz);
            for (Side side : new Side[] {Side.EAST, Side.UP, Side.SOUTH}) {
                int x = at[0] + side.dx, y = at[1] + side.dy, z = at[2] + side.dz;
                if (x < nx && y < ny && z < nz) {
                    Module a = rules.modules().get(grid[c]), b = rules.modules().get(grid[cell(x, y, z, nx, nz)]);
                    if (!fits(a.socket(side), b.socket(side.opposite()), rules.pairs())) {
                        return false;
                    }
                }
            }
        }
        return true;
    }

    /** The modules' ids by cell, for a sheet. */
    public static List<String> ids(Rules rules, int[] grid) {
        List<String> out = new ArrayList<>();
        for (int k : grid) {
            out.add(rules.modules().get(k).id());
        }
        return out;
    }
}
