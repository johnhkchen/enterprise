package dev.b28.enterprise.territory;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/**
 * The town grader (the studio's `js/terrain/grade.mjs`, thread 0013, ported; change one, change both): plans a town
 * site's landform before any plot is laid (John: "the houses are sitting on weird hills"), in four passes:
 * <ol>
 * <li>streets: each street's long profile, the ground along it smoothed and held under a grade limit, met at its ends
 * by the forum and the streets graded before it; level across its width;</li>
 * <li>frontages: a plot takes its street frontage's level (so its door's half steps work); a plot holding a building
 * already standing keeps that building's ground;</li>
 * <li>between: the land inside the outer edge that's neither street nor plot: the planned levels spread over it as a
 * smooth correction to the natural ground, then cut into terraces (3 a step) with no riser over 4 and never two side by
 * side, so every edge is one earthworks tile;</li>
 * <li>edge: a blend band past the outer edge, where the correction fades to nothing: the town meets wild land on a
 * slope, never a cliff.</li>
 * </ol>
 * Pure: grids in, grids out, no world. Columns are indexed {@code z * X + x} in the caller's frame.
 */
public final class Grading {
    private Grading() {}

    public static final short NONE = Short.MIN_VALUE;
    public static final byte WILD = 0, BLEND = 1, FREE = 2, STREET = 3, PLOT = 4, FIXED = 5, WATER = 6, FORUM = 7, NATURAL = 8;
    static final int[][] DIRS = {{0, -1}, {0, 1}, {1, 0}, {-1, 0}};
    /** The ways down, as {@link #DIRS}. */
    public static final String[] DIR_NAMES = {"north", "south", "east", "west"};

    /** A street: its kind ("avenue" first, then "lane"), its width, its centreline's points {x, z}. */
    public record Street(String kind, int width, int[][] line) {}

    /** A plot: its corner, size, and the way its door faces (onto its street: 0 north, 1 south, 2 east, 3 west). */
    public record Plot(int x, int z, int sx, int sz, int door) {}

    /**
     * A step: its high column, the way down ({@link #DIRS}; for an inner corner a diagonal, 4 south-east, 5 south-west,
     * 6 north-west, 7 north-east), the rise, its kind, and its corner ("outer", "inner" or null).
     */
    public record Edge(int x, int z, int dir, int rise, String kind, String corner) {}

    /**
     * What the grade planned: each column's height and role, where a street crosses water, the steps, a report (with the
     * plots still more than 4 off a neighbour, and the columns left natural).
     */
    public record Grade(short[] target, byte[] role, byte[] bridge, List<Edge> edges, int cut, int fill, int conflicts,
            int plotsOver, int natural) {}

    public static Grade grade(int X, int Z, short[] top, boolean[] water, short[] fixed, List<Street> streets,
            List<Plot> plots, int forumX, int forumZ, int forumReach, int forumLevel, int[] outer, int blend) {
        return grade(X, Z, top, water, fixed, streets, plots, forumX, forumZ, forumReach, forumLevel, outer, blend, 3, 4,
                1 / 3.0, 3, 4, 0.4);
    }

    public static Grade grade(int X, int Z, short[] top, boolean[] water, short[] fixed, List<Street> streets,
            List<Plot> plots, int forumX, int forumZ, int forumReach, int forumLevel, int[] outer, int blend, int step,
            int maxRise, double gradeLimit, int near, int keepAway, double keepFall) {
        int N = X * Z;
        byte[] role = new byte[N], bridge = new byte[N];
        short[] target = new short[N];
        Arrays.fill(target, NONE);
        int[] owner = new int[N], plotOf = new int[N];
        Arrays.fill(owner, -1);
        Arrays.fill(plotOf, -1);
        int ox0 = outer[0], oz0 = outer[1], ox1 = outer[2], oz1 = outer[3];
        // Roles: water and standing buildings first, then the forum, streets, plots; the rest inside is free land.
        for (int z = 0; z < Z; z++) {
            for (int x = 0; x < X; x++) {
                int i = z * X + x, d = Math.max(Math.max(ox0 - x, x - ox1), Math.max(Math.max(oz0 - z, z - oz1), 0));
                role[i] = d == 0 ? FREE : d <= blend ? BLEND : WILD;
                if (water[i]) {
                    role[i] = WATER;
                    target[i] = top[i];
                } else if (fixed[i] != NONE) {
                    role[i] = FIXED;
                    target[i] = fixed[i];
                }
            }
        }
        for (int z = forumZ - forumReach; z <= forumZ + forumReach; z++) {
            for (int x = forumX - forumReach; x <= forumX + forumReach; x++) {
                int i = at(X, Z, x, z);
                if (claim(role, i, FORUM)) {
                    target[i] = (short) forumLevel;
                }
            }
        }

        // 1. Streets: avenues first (they meet the forum), then lanes (they meet the avenues).
        List<Integer> order = new ArrayList<>();
        for (int s = 0; s < streets.size(); s++) {
            order.add(s);
        }
        order.sort((a, b) -> Integer.compare(streets.get(a).kind().equals("avenue") ? 0 : 1, streets.get(b).kind().equals("avenue") ? 0 : 1));
        for (int si : order) {
            Street s = streets.get(si);
            double half = (s.width() - 1) / 2.0;
            List<int[]> samples = new ArrayList<>();
            for (int k = 0; k + 1 < s.line().length; k++) {
                int ax = s.line()[k][0], az = s.line()[k][1], bx = s.line()[k + 1][0], bz = s.line()[k + 1][1];
                int L = Math.max(Math.abs(bx - ax), Math.abs(bz - az));
                for (int t = 0; t <= L; t++) {
                    if (k == 0 || t > 0) {
                        samples.add(new int[] {ax + (int) Math.round((bx - ax) * (double) t / Math.max(1, L)),
                                az + (int) Math.round((bz - az) * (double) t / Math.max(1, L)), bz != az ? 1 : 0});
                    }
                }
            }
            int n = samples.size();
            if (n == 0) {
                continue;
            }
            List<int[]> cells = new ArrayList<>();
            for (int[] sm : samples) {
                List<Integer> c = new ArrayList<>();
                for (int o = -(int) Math.floor(half); o <= (int) Math.ceil(half); o++) {
                    int i = sm[2] == 1 ? at(X, Z, sm[0] + o, sm[1]) : at(X, Z, sm[0], sm[1] + o);
                    if (i >= 0) {
                        c.add(i);
                    }
                }
                cells.add(c.stream().mapToInt(Integer::intValue).toArray());
            }
            double[] nat = new double[n];
            Double[] pinned = new Double[n];
            for (int k = 0; k < n; k++) {
                nat[k] = median(Arrays.stream(cells.get(k)).filter(i -> !water[i]).map(i -> top[i]).toArray());
                int[] fixedHere = Arrays.stream(cells.get(k)).filter(i -> role[i] == STREET || role[i] == FORUM).map(i -> target[i]).toArray();
                pinned[k] = fixedHere.length > 0 ? median(fixedHere) : null;
            }
            // Smooth the natural profile (a 9-sample mean), then hold it under the grade from every pinned sample both ways.
            double[] p = new double[n];
            for (int k = 0; k < n; k++) {
                double a = 0;
                int w = 0;
                for (int j = Math.max(0, k - 4); j <= Math.min(n - 1, k + 4); j++) {
                    if (!Double.isNaN(nat[j])) {
                        a += nat[j];
                        w++;
                    }
                }
                p[k] = w > 0 ? a / w : Double.NaN;
            }
            Double firstPin = Arrays.stream(pinned).filter(v -> v != null).findFirst().orElse((double) forumLevel);
            for (int k = 0; k < n; k++) {
                if (Double.isNaN(p[k])) {
                    p[k] = k > 0 ? p[k - 1] : firstPin;
                }
            }
            double[] h = new double[n];
            for (int k = 0; k < n; k++) {
                h[k] = pinned[k] != null ? pinned[k] : p[k];
            }
            for (int pass = 0; pass < 4; pass++) {
                for (int k = 1; k < n; k++) {
                    if (pinned[k] == null) {
                        h[k] = Math.min(h[k - 1] + gradeLimit, Math.max(h[k - 1] - gradeLimit, h[k]));
                    }
                }
                for (int k = n - 2; k >= 0; k--) {
                    if (pinned[k] == null) {
                        h[k] = Math.min(h[k + 1] + gradeLimit, Math.max(h[k + 1] - gradeLimit, h[k]));
                    }
                }
            }
            for (int k = 0; k < n; k++) {
                // Over water the street is a bridge: its deck at least 2 over the water.
                int wetTop = Integer.MIN_VALUE;
                for (int i : cells.get(k)) {
                    if (water[i]) {
                        wetTop = Math.max(wetTop, top[i]);
                    }
                }
                short level = (short) Math.round(wetTop != Integer.MIN_VALUE ? Math.max(h[k], wetTop + 2) : h[k]);
                for (int i : cells.get(k)) {
                    if (role[i] == WATER) {
                        role[i] = STREET;
                        bridge[i] = (byte) (samples.get(k)[2] == 1 ? 1 : 2);
                    }
                    if (claim(role, i, STREET) || role[i] == STREET && owner[i] == -1) {
                        target[i] = level;
                        owner[i] = si;
                    }
                }
            }
        }

        // 2. Frontages: plots holding a standing building first (they keep its ground), then the rest.
        int[][] out = {{0, -1}, {0, 1}, {1, 0}, {-1, 0}};
        List<Integer> plotOrder = new ArrayList<>();
        for (int k = 0; k < plots.size(); k++) {
            plotOrder.add(k);
        }
        plotOrder.sort((a, b) -> Integer.compare(holds(X, Z, role, plots.get(b)), holds(X, Z, role, plots.get(a))));
        // Each plot laid: {plot, level, frontage, built}.
        List<int[]> plotLevels = new ArrayList<>();
        for (int pi : plotOrder) {
            Plot pl = plots.get(pi);
            int dx = out[pl.door()][0], dz = out[pl.door()][1];
            List<Integer> built = new ArrayList<>();
            for (int z = pl.z(); z < pl.z() + pl.sz(); z++) {
                for (int x = pl.x(); x < pl.x() + pl.sx(); x++) {
                    int i = at(X, Z, x, z);
                    if (i >= 0 && role[i] == FIXED) {
                        built.add((int) target[i]);
                    }
                }
            }
            Integer lv = null;
            if (!built.isEmpty()) {
                lv = (int) Math.round(median(built.stream().mapToInt(Integer::intValue).toArray()));
            } else {
                // Walk out from the door side's middle to the street it fronts.
                int mx = pl.x() + (dx == 0 ? pl.sx() >> 1 : dx > 0 ? pl.sx() - 1 : 0);
                int mz = pl.z() + (dz == 0 ? pl.sz() >> 1 : dz > 0 ? pl.sz() - 1 : 0);
                for (int k = 1; k <= 8 && lv == null; k++) {
                    int i = at(X, Z, mx + dx * k, mz + dz * k);
                    if (i >= 0 && (role[i] == STREET || role[i] == FORUM)) {
                        lv = (int) target[i];
                    }
                }
                if (lv == null) {
                    List<Integer> v = new ArrayList<>();
                    for (int z = pl.z(); z < pl.z() + pl.sz(); z++) {
                        for (int x = pl.x(); x < pl.x() + pl.sx(); x++) {
                            int i = at(X, Z, x, z);
                            if (i >= 0) {
                                v.add((int) top[i]);
                            }
                        }
                    }
                    if (v.isEmpty()) {
                        continue;
                    }
                    lv = (int) Math.round(median(v.stream().mapToInt(Integer::intValue).toArray()));
                }
            }
            plotLevels.add(new int[] {pi, lv, lv, built.isEmpty() ? 0 : 1});
            for (int z = pl.z(); z < pl.z() + pl.sz(); z++) {
                for (int x = pl.x(); x < pl.x() + pl.sx(); x++) {
                    int i = at(X, Z, x, z);
                    if (claim(role, i, PLOT)) {
                        target[i] = (short) (int) lv;
                        plotOf[i] = pi;
                    }
                }
            }
        }
        // A plot may sit 1 off its frontage (its door's approach takes the half step) where that closes a step over
        // maxRise against a street, the forum, a building or another plot; what stays over is counted.
        java.util.Map<Integer, List<Integer>> cellsOf = new java.util.HashMap<>();
        for (int[] e : plotLevels) {
            cellsOf.put(e[0], new ArrayList<>());
        }
        for (int i = 0; i < N; i++) {
            if (plotOf[i] >= 0) {
                cellsOf.get(plotOf[i]).add(i);
            }
        }
        for (int round = 0; round < 3; round++) {
            for (int[] e : plotLevels) {
                if (e[3] == 1) {
                    continue;
                }
                int best = e[2], bestOver = Integer.MAX_VALUE;
                for (int lv : new int[] {e[2], e[2] - 1, e[2] + 1}) {
                    int over = overOf(X, Z, role, target, plotOf, cellsOf.get(e[0]), e[0], lv, maxRise);
                    if (over < bestOver) {
                        best = lv;
                        bestOver = over;
                    }
                }
                if (best != e[1]) {
                    e[1] = best;
                    for (int i : cellsOf.get(e[0])) {
                        target[i] = (short) best;
                    }
                }
            }
        }
        int plotsOver = 0;
        for (int[] e : plotLevels) {
            if (overOf(X, Z, role, target, plotOf, cellsOf.get(e[0]), e[0], e[1], maxRise) > 0) {
                plotsOver++;
            }
        }

        // 3 and 4. The land between and the blend: the correction (planned minus natural) known on every planned column,
        // spread by relaxation over free and blend columns, held at zero past the band; free land terraced.
        float[] corr = new float[N];
        boolean[] known = new boolean[N];
        for (int i = 0; i < N; i++) {
            if (role[i] == WILD || role[i] == WATER) {
                known[i] = true;
            } else if (role[i] != FREE && role[i] != BLEND) {
                known[i] = true;
                corr[i] = target[i] - top[i];
            }
        }
        for (int it = 0; it < 600; it++) {
            for (int z = 0; z < Z; z++) {
                for (int x = 0; x < X; x++) {
                    int i = z * X + x;
                    if (known[i]) {
                        continue;
                    }
                    float a = 0;
                    int w = 0;
                    for (int[] d : DIRS) {
                        int j = at(X, Z, x + d[0], z + d[1]);
                        if (j >= 0) {
                            a += corr[j];
                            w++;
                        }
                    }
                    corr[i] += 1.85f * (a / w - corr[i]); // over-relaxed Gauss-Seidel
                }
            }
        }
        // The free land follows the natural ground's lie, not its every bump: a 7 x 7 mean, twice (water left out).
        float[] lie = new float[N];
        for (int i = 0; i < N; i++) {
            lie[i] = top[i];
        }
        for (int pass = 0; pass < 2; pass++) {
            float[] nx = new float[N];
            for (int z = 0; z < Z; z++) {
                for (int x = 0; x < X; x++) {
                    float a = 0;
                    int w = 0;
                    for (int ddz = -3; ddz <= 3; ddz++) {
                        for (int ddx = -3; ddx <= 3; ddx++) {
                            int j = at(X, Z, x + ddx, z + ddz);
                            if (j >= 0 && !water[j]) {
                                a += lie[j];
                                w++;
                            }
                        }
                    }
                    nx[z * X + x] = w > 0 ? a / w : lie[z * X + x];
                }
            }
            lie = nx;
        }
        // Land the town doesn't use keeps its slope: free land more than keepAway from any street, plot, the forum or a
        // building, on ground that falls keepFall or more a column, is left natural (the correction fading out over 8),
        // like the edge band, rather than terraced.
        int[] dUsed = new int[N];
        Arrays.fill(dUsed, 1 << 20);
        int[] queue = new int[N];
        int qn = 0;
        for (int i = 0; i < N; i++) {
            if (role[i] == STREET || role[i] == PLOT || role[i] == FORUM || role[i] == FIXED) {
                dUsed[i] = 0;
                queue[qn++] = i;
            }
        }
        for (int h = 0; h < qn; h++) {
            int i = queue[h], x = i % X, z = i / X;
            for (int[] d : DIRS) {
                int j = at(X, Z, x + d[0], z + d[1]);
                if (j >= 0 && dUsed[j] > dUsed[i] + 1) {
                    dUsed[j] = dUsed[i] + 1;
                    queue[qn++] = j;
                }
            }
        }
        int natural = 0;
        for (int z = 0; z < Z; z++) {
            for (int x = 0; x < X; x++) {
                int i = z * X + x;
                if (role[i] == FREE && dUsed[i] > keepAway && fall(X, Z, lie, x, z) >= keepFall) {
                    role[i] = NATURAL;
                    natural++;
                }
            }
        }
        for (int i = 0; i < N; i++) {
            if (role[i] == NATURAL) {
                target[i] = (short) Math.round(top[i] + corr[i] * Math.min(1, Math.max(0, 1 - (dUsed[i] - keepAway) / 8.0)));
            } else if (role[i] == FREE) {
                target[i] = (short) (forumLevel + step * Math.round((lie[i] + corr[i] - forumLevel) / step));
            } else if (role[i] == BLEND) {
                target[i] = (short) Math.round(top[i] + corr[i]);
            } else if (role[i] == WILD) {
                target[i] = top[i];
            }
        }
        // No riser over maxRise and never two side by side: every 3 x 3 window of the town spans at most maxRise; only
        // free columns move (toward the window's middle), planned ones hold.
        for (int it = 0; it < 200; it++) {
            int moved = 0;
            for (int z = 0; z < Z; z++) {
                for (int x = 0; x < X; x++) {
                    int i = z * X + x;
                    if (role[i] != FREE) {
                        continue;
                    }
                    double lo = Double.NEGATIVE_INFINITY, hi = Double.POSITIVE_INFINITY;
                    for (int ddz = -1; ddz <= 1; ddz++) {
                        for (int ddx = -1; ddx <= 1; ddx++) {
                            int j = at(X, Z, x + ddx, z + ddz);
                            if (j < 0 || j == i || role[j] == WILD || role[j] == BLEND) {
                                continue;
                            }
                            lo = Math.max(lo, target[j] - maxRise);
                            hi = Math.min(hi, target[j] + maxRise);
                        }
                    }
                    long v = lo > hi ? Math.round((lo + hi) / 2) : Math.round(Math.min(hi, Math.max(lo, target[i])));
                    if (v != target[i]) {
                        target[i] = (short) v;
                        moved++;
                    }
                }
            }
            if (moved == 0) {
                break;
            }
        }
        // The blend band and land left natural rise and fall a block a column at most among themselves (natural slopes,
        // no tiles), and meet the town within maxRise.
        for (int it = 0; it < 40; it++) {
            for (int z = 0; z < Z; z++) {
                for (int x = 0; x < X; x++) {
                    int i = z * X + x;
                    if (role[i] != BLEND && role[i] != NATURAL) {
                        continue;
                    }
                    double lo = Double.NEGATIVE_INFINITY, hi = Double.POSITIVE_INFINITY;
                    for (int[] d : DIRS) {
                        int j = at(X, Z, x + d[0], z + d[1]);
                        if (j < 0 || role[j] == WATER) {
                            continue;
                        }
                        int tol = role[j] == BLEND || role[j] == NATURAL || role[j] == WILD ? 1 : maxRise;
                        lo = Math.max(lo, target[j] - tol);
                        hi = Math.min(hi, target[j] + tol);
                    }
                    if (lo <= hi) {
                        target[i] = (short) Math.round(Math.min(hi, Math.max(lo, target[i])));
                    }
                }
            }
        }

        // Edges: the high column of every step, its way down, its rise and its tile family.
        boolean[] nearPlanned = new boolean[N];
        for (int z = 0; z < Z; z++) {
            for (int x = 0; x < X; x++) {
                byte r = role[z * X + x];
                if (r != STREET && r != FORUM && r != FIXED) {
                    continue;
                }
                for (int ddz = -near; ddz <= near; ddz++) {
                    for (int ddx = -near; ddx <= near; ddx++) {
                        int j = at(X, Z, x + ddx, z + ddz);
                        if (j >= 0) {
                            nearPlanned[j] = true;
                        }
                    }
                }
            }
        }
        List<Edge> edges = new ArrayList<>();
        int conflicts = 0;
        for (int z = 0; z < Z; z++) {
            for (int x = 0; x < X; x++) {
                int i = z * X + x;
                if (role[i] == WILD || role[i] == WATER || role[i] == FIXED || bridge[i] != 0) {
                    continue;
                }
                for (int d = 0; d < 4; d++) {
                    int j = at(X, Z, x + DIRS[d][0], z + DIRS[d][1]);
                    if (j < 0 || role[j] == WILD || role[j] == FIXED || bridge[j] != 0) {
                        continue;
                    }
                    int rise = target[i] - (role[j] == WATER ? top[j] : target[j]);
                    if (rise <= 0) {
                        continue;
                    }
                    String kind = role[j] == WATER ? "quay" : role[i] == BLEND || role[j] == BLEND
                            || role[i] == NATURAL && role[j] == NATURAL ? "slope"
                            : role[i] == STREET && role[j] == STREET && rise == 1 ? "street_step"
                            : nearPlanned[i] || nearPlanned[j] || role[i] == PLOT || role[j] == PLOT ? "wall" : "cut_face";
                    if (kind.equals("slope") && rise <= 1) {
                        continue;
                    }
                    edges.add(new Edge(x, z, d, rise, kind, null));
                    if (rise > maxRise && !kind.equals("slope")) {
                        conflicts++;
                    }
                }
            }
        }
        // Corners: where two steps meet at a column (an outer corner) the corner tile may be laid for both; where a
        // column stands high with only its diagonal low (an inner corner, between two walls meeting), it gets a corner
        // edge of its own, or the wall shows a gap there.
        java.util.Map<Integer, List<Integer>> stepsAt = new java.util.HashMap<>();
        for (int k = 0; k < edges.size(); k++) {
            Edge e = edges.get(k);
            stepsAt.computeIfAbsent(e.z() * X + e.x(), q -> new ArrayList<>()).add(k);
        }
        for (List<Integer> list : stepsAt.values()) {
            boolean ns = list.stream().anyMatch(k -> edges.get(k).dir() < 2), ew = list.stream().anyMatch(k -> edges.get(k).dir() >= 2);
            if (list.size() > 1 && ns && ew) {
                for (int k : list) {
                    Edge e = edges.get(k);
                    edges.set(k, new Edge(e.x(), e.z(), e.dir(), e.rise(), e.kind(), "outer"));
                }
            }
        }
        int[][] diagonals = {{1, 1, 4}, {-1, 1, 5}, {-1, -1, 6}, {1, -1, 7}};
        for (int z = 0; z < Z; z++) {
            for (int x = 0; x < X; x++) {
                int i = z * X + x;
                if (stepsAt.containsKey(i) || role[i] == WILD || role[i] == BLEND || role[i] == WATER || role[i] == FIXED
                        || role[i] == NATURAL || bridge[i] != 0) {
                    continue;
                }
                for (int[] dg : diagonals) {
                    int j = at(X, Z, x + dg[0], z + dg[1]), a = at(X, Z, x + dg[0], z), b = at(X, Z, x, z + dg[1]);
                    if (j < 0 || a < 0 || b < 0) {
                        continue;
                    }
                    int wantA = dg[1] > 0 ? 1 : 0, wantB = dg[0] > 0 ? 2 : 3;
                    Edge ea = stepsAt.getOrDefault(a, List.of()).stream().map(edges::get).filter(e -> e.dir() == wantA).findFirst().orElse(null);
                    Edge eb = stepsAt.getOrDefault(b, List.of()).stream().map(edges::get).filter(e -> e.dir() == wantB).findFirst().orElse(null);
                    if (ea == null || eb == null || target[a] != target[i] || target[b] != target[i] || target[j] >= target[i]) {
                        continue;
                    }
                    String kind = ea.kind().equals("wall") || eb.kind().equals("wall") ? "wall" : ea.kind();
                    if (kind.equals("slope") || kind.equals("street_step")) {
                        continue;
                    }
                    edges.add(new Edge(x, z, dg[2], target[i] - target[j], kind, "inner"));
                    break;
                }
            }
        }
        int cut = 0, fill = 0;
        for (int i = 0; i < N; i++) {
            if (role[i] != WILD && role[i] != WATER && role[i] != FIXED) {
                int d = target[i] - top[i];
                if (d > 0) {
                    fill += d;
                } else {
                    cut -= d;
                }
            }
        }
        return new Grade(target, role, bridge, edges, cut, fill, conflicts, plotsOver, natural);
    }

    /** How far a plot at {@code lv} stands more than maxRise off its neighbours (streets, the forum, buildings, plots), summed. */
    private static int overOf(int X, int Z, byte[] role, short[] target, int[] plotOf, List<Integer> cells, int plot, int lv,
            int maxRise) {
        int over = 0;
        for (int i : cells) {
            int x = i % X, z = i / X;
            for (int[] d : DIRS) {
                int j = at(X, Z, x + d[0], z + d[1]);
                if (j < 0 || plotOf[j] == plot) {
                    continue;
                }
                if (role[j] == STREET || role[j] == FORUM || role[j] == FIXED || role[j] == PLOT) {
                    over += Math.max(0, Math.abs(lv - target[j]) - maxRise);
                }
            }
        }
        return over;
    }

    /** How steeply the land's lie falls at a column, over 4 either way. */
    private static double fall(int X, int Z, float[] lie, int x, int z) {
        int a = at(X, Z, x - 2, z), b = at(X, Z, x + 2, z), c = at(X, Z, x, z - 2), d = at(X, Z, x, z + 2);
        double gx = a >= 0 && b >= 0 ? Math.abs(lie[a] - lie[b]) / 4 : 0, gz = c >= 0 && d >= 0 ? Math.abs(lie[c] - lie[d]) / 4 : 0;
        return Math.max(gx, gz);
    }

    private static int at(int X, int Z, int x, int z) {
        return x < 0 || z < 0 || x >= X || z >= Z ? -1 : z * X + x;
    }

    private static boolean claim(byte[] role, int i, byte r) {
        if (i >= 0 && (role[i] == FREE || role[i] == BLEND)) {
            role[i] = r;
            return true;
        }
        return false;
    }

    private static int holds(int X, int Z, byte[] role, Plot p) {
        int k = 0;
        for (int z = p.z(); z < p.z() + p.sz(); z++) {
            for (int x = p.x(); x < p.x() + p.sx(); x++) {
                int i = at(X, Z, x, z);
                if (i >= 0 && role[i] == FIXED) {
                    k++;
                }
            }
        }
        return k;
    }

    /** The upper median, as the studio's ({@code s[s.length >> 1]}); NaN if empty. */
    private static double median(int[] a) {
        if (a.length == 0) {
            return Double.NaN;
        }
        int[] s = a.clone();
        Arrays.sort(s);
        return s[s.length >> 1];
    }
}
