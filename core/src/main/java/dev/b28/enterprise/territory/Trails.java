package dev.b28.enterprise.territory;

import dev.b28.enterprise.hex.Hex;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Desire paths: the trails the core's wagons wore ({@code Output.Trail}, {@code map.rs}), laid in the world as dirt
 * path. A trail runs through its hex from its edge with the hex the road came from, by the hex's centre, to its edge
 * with the hex it goes on to, as wide as the core says (two blocks, wider as it's worn more: laid again each time);
 * where the ground is soil (grass, dirt, podzol and the like) it's trodden to path, a tree in its way felled (its trunk
 * in the way cut down: its leaves, with no trunk, fall on their own), what grows on it and the leaves over it cleared,
 * and a lone block's bump or dip on the way levelled. A field in its way is cut through, its crops cleared and a hedge
 * planted on each side. Sand, stone, water and anything built are left as they are.
 *
 * <p>Saved with the world: the trails not yet laid (their ground not loaded), laid as it is.
 */
public final class Trails extends SavedData {
    private static final String NAME = "enterprise_trails";
    /** Blocks over a trail's ground cleared of leaves: a wagon's height and its load. */
    static final int HEADROOM = 4;

    /**
     * A trail to lay: its hex, the hexes before and after it on the road (the same hex at a road's end), its width, and
     * the whole road's hexes end to end (empty: a trail saved before roads were laid whole, laid hex by hex).
     */
    public record Trail(Hex hex, Hex from, Hex to, int width, List<Hex> path, boolean llama) {
        public Trail(Hex hex, Hex from, Hex to, int width) {
            this(hex, from, to, width, List.of(), false);
        }
    }

    private final List<Trail> waiting = new ArrayList<>();

    public static Trails get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Trails::new, Trails::load, null), NAME);
    }

    public void add(Trail trail) {
        waiting.add(trail);
        setDirty();
    }

    public List<Trail> waiting() {
        return List.copyOf(waiting);
    }

    public void laid(Trail trail) {
        if (waiting.remove(trail)) {
            setDirty();
        }
    }

    /** The line a trail treads, in blocks: from its edge with {@code from}, by its centre, to its edge with {@code to}. */
    public static double[][] line(Trail t) {
        return line(t, java.util.Map.of());
    }

    /**
     * The line a trail treads in its hex: its stretch of the whole road pulled taut ({@link #taut}), from the bell of
     * the town at each end (in {@code bells}, by town) or its hex's centre; a trail with no road, edge to centre to edge.
     */
    public static double[][] line(Trail t, java.util.Map<Long, BlockPos> bells) {
        if (t.path().size() >= 2) {
            double[][] road = roadOf(t);
            double[][] stretch = within(road != null ? road : taut(t.path(), bells), t.hex(), t.path());
            if (stretch.length >= 2) {
                return stretch;
            }
        }
        double cx = t.hex().centerX(), cz = t.hex().centerZ();
        double ax = (cx + t.from().centerX()) / 2, az = (cz + t.from().centerZ()) / 2;
        double bx = (cx + t.to().centerX()) / 2, bz = (cz + t.to().centerZ()) / 2;
        return new double[][] {{ax, az}, {cx, cz}, {bx, bz}};
    }

    /**
     * A road's line pulled taut through its hexes (John: "roads should follow actual optimal paths not hex-optimized
     * paths"): from its first hex's town bell (or centre) to its last's, each stretch as straight as it can go while it
     * stays within the road's hexes and those beside them ({@link #band}: held to its own, it kept their zigzag),
     * turning only where they turn. Greedy string pulling over the hexes' centres, the line checked every two blocks.
     */
    public static double[][] taut(List<Hex> path, java.util.Map<Long, BlockPos> bells) {
        java.util.Set<Hex> corridor = band(path);
        List<double[]> points = new ArrayList<>();
        for (int i = 0; i < path.size(); i++) {
            Hex h = path.get(i);
            double[] p = {h.centerX(), h.centerZ()};
            if (i == 0 || i == path.size() - 1) {
                for (BlockPos bell : bells.values()) {
                    if (Hex.at(bell).equals(h)) {
                        p = new double[] {bell.getX() + 0.5, bell.getZ() + 0.5};
                    }
                }
            }
            points.add(p);
        }
        List<double[]> out = new ArrayList<>();
        out.add(points.get(0));
        int at = 0;
        while (at < points.size() - 1) {
            int next = at + 1;
            for (int j = points.size() - 1; j > at + 1; j--) {
                if (inside(points.get(at), points.get(j), corridor)) {
                    next = j;
                    break;
                }
            }
            out.add(points.get(next));
            at = next;
        }
        return out.toArray(new double[0][]);
    }

    /**
     * Is the trail's road planned ({@link #road}), or one laid hex by hex (none to plan)? A road is planned once the land
     * of its band is generated and loaded ({@link #landOf}; John: "the plan can't really take place until the terrain is
     * generated"), at the look that finds it so.
     */
    public static boolean planned(Trail t) {
        return t.path().size() < 2 || PLANNED.containsKey(Ends.of(t));
    }

    /**
     * The trail's road planned over its land, which must be loaded ({@link #landOf}); the roads planned written to the
     * world's {@code enterprise-roads.json} for the surveyor's map (`dev/experiments/world_render.py`).
     */
    public static void plan(ServerLevel level, Trail t, java.util.Map<Long, BlockPos> bells) {
        if (t.path().size() >= 2 && !PLANNED.containsKey(Ends.of(t))) {
            Ends ends = Ends.of(t);
            double[][] road = road(level, t.path(), bells, t.llama());
            if (!t.path().get(0).equals(ends.a())) {
                road = reversed(road);
            }
            PLANNED.put(ends, road);
            StringBuilder json = new StringBuilder("[");
            for (var e : PLANNED.entrySet()) {
                json.append(json.length() > 1 ? "," : "").append("{\"path\":[");
                json.append("[" + e.getKey().a().q() + "," + e.getKey().a().r() + "],[" + e.getKey().b().q() + ","
                        + e.getKey().b().r() + "]");
                json.append("],\"line\":[");
                json.append(String.join(",", java.util.Arrays.stream(e.getValue())
                        .map(p -> "[" + Math.round(p[0] * 10) / 10.0 + "," + Math.round(p[1] * 10) / 10.0 + "]").toList()));
                json.append("]}");
            }
            json.append("]");
            try {
                java.nio.file.Files.writeString(level.getServer().getWorldPath(net.minecraft.world.level.storage.LevelResource.ROOT)
                        .resolve("enterprise-roads.json"), json);
            } catch (java.io.IOException e) {
                dev.b28.enterprise.Enterprise.LOGGER.warn("The roads weren't written for the map: {}", e.getMessage());
            }
            dev.b28.enterprise.Enterprise.LOGGER.info("A road planned over its land from hex {} to {} ({} points, {})",
                    t.path().get(0), t.path().get(t.path().size() - 1), road.length, t.llama() ? "llamas" : "wagons");
        }
    }

    /** How far a road may stray from the straight way: its ground's sum of distances to the ends, as a share of theirs. */
    static final double STRAY = 1.35;
    static final double STRAY_BLOCKS = 64;

    /** Its ends: the bells of the towns at the road's ends, or their hexes' centres. */
    static double[][] ends(List<Hex> path, java.util.Map<Long, BlockPos> bells) {
        double[][] taut = taut(path, bells);
        return new double[][] {taut[0], taut[taut.length - 1]};
    }

    /**
     * Is the place within a road's reach: the ellipse round its ends ({@link #STRAY}), whatever hexes the core routed it
     * over (John: "planning shouldn't assume the band is the most optimal")?
     */
    static boolean reach(double[] a, double[] b, double x, double z) {
        return Math.hypot(x - a[0], z - a[1]) + Math.hypot(x - b[0], z - b[1])
                <= STRAY * Math.hypot(b[0] - a[0], b[1] - a[1]) + STRAY_BLOCKS;
    }

    /** The chunks of a road's reach, to be loaded (made, where never yet) before it's planned. */
    public static List<ChunkPos> landOf(Trail t, java.util.Map<Long, BlockPos> bells) {
        double[][] e = ends(t.path(), bells);
        double[] a = e[0], b = e[1];
        double r = (STRAY * Math.hypot(b[0] - a[0], b[1] - a[1]) + STRAY_BLOCKS) / 2;
        java.util.Set<ChunkPos> out = new java.util.LinkedHashSet<>();
        double mx = (a[0] + b[0]) / 2, mz = (a[1] + b[1]) / 2;
        for (double x = mx - r; x <= mx + r + 16; x += 16) {
            for (double z = mz - r; z <= mz + r + 16; z += 16) {
                if (reach(a, b, x, z)) {
                    out.add(new ChunkPos(BlockPos.containing(x, 0, z)));
                }
            }
        }
        return List.copyOf(out);
    }

    /** Roads planned over the land, by their hexes: the same plan every time, so planned again after a restart. */
    static final java.util.Map<Ends, double[][]> PLANNED = new java.util.concurrent.ConcurrentHashMap<>();

    /**
     * A road's key: its two end hexes in a fixed order. One road serves both ways between two towns, whatever carries
     * on it, and the same road stays when the core routes them over other hexes (seed 8675309's first run of planned
     * roads laid fifteen for five pairs of towns, each direction and each new routing its own: the stray paths).
     */
    record Ends(Hex a, Hex b) {
        static Ends of(Trail t) {
            Hex first = t.path().get(0), last = t.path().get(t.path().size() - 1);
            boolean flip = first.q() > last.q() || first.q() == last.q() && first.r() > last.r();
            return flip ? new Ends(last, first) : new Ends(first, last);
        }
    }

    /** The trail's road as planned, from its path's first hex to its last; null if not planned yet. */
    static double[][] roadOf(Trail t) {
        if (t.path().size() < 2) {
            return null;
        }
        Ends ends = Ends.of(t);
        double[][] road = PLANNED.get(ends);
        return road == null || t.path().get(0).equals(ends.a()) ? road : reversed(road);
    }

    static double[][] reversed(double[][] line) {
        double[][] out = new double[line.length][];
        for (int i = 0; i < line.length; i++) {
            out[i] = line[line.length - 1 - i];
        }
        return out;
    }
    /** The road planner's grid, in blocks. */
    static final int GRID = 4;
    /**
     * How dear a grade is to a wagon (squared), the grade past which it's a cliff to go round and the cliff's toll, and
     * a ford's share of a step (a step costs 1 + this): a wagon wants a gentle road, 1 rise in 12 run its steepest
     * (John: "even flatter. Maybe 1 rise per 12 run").
     */
    static final double GRADE = 100;
    static final double SHEER = 1.0 / 12;
    static final double SHEER_TOLL = 60;
    static final double WET = 25;
    /** A llama train's: steeper ground it takes, 1 rise in 3 run its steepest (John), water as a wagon does. */
    static final double LLAMA_GRADE = 12;
    static final double LLAMA_SHEER = 1.0 / 3;

    /**
     * A road planned over the land itself (John: "paths follow terrain, based on what makes wagon or llama travel
     * feasible"), from bell to bell anywhere within its reach ({@link #reach}: not held to the hexes the core routed it
     * over), its land generated and loaded: the cheapest way for its
     * carrier on a {@link #GRID}-block grid of the ground's heights (under its trees), a step costing its length, more as
     * it climbs (the grade squared: gentle slopes cheap, steep ones dear, past the carrier's steepest a cliff to go
     * round), several times more through water, and a little meander on the flat. Then pulled straight wherever that
     * costs no more, and its corners rounded, so it runs as a road does: along a valley, round a hill, over the saddle.
     * The taut line if there's no way.
     */
    public static double[][] road(ServerLevel level, List<Hex> path, java.util.Map<Long, BlockPos> bells, boolean llama) {
        double[][] ends = ends(path, bells);
        return between(level, ends[0], ends[1], llama, STRAY, STRAY_BLOCKS);
    }

    /**
     * As {@link #road}, from {@code a} to {@code b} (block x, z) within the ellipse round them {@code stray} times their
     * distance plus {@code strayBlocks} across: a farm track off a trade road as much as a road between towns.
     */
    public static double[][] between(ServerLevel level, double[] a, double[] b, boolean llama, double stray,
            double strayBlocks) {
        double[][] ends = {a, b};
        double r = (stray * Math.hypot(b[0] - a[0], b[1] - a[1]) + strayBlocks) / 2;
        double minX = (a[0] + b[0]) / 2 - r, maxX = (a[0] + b[0]) / 2 + r;
        double minZ = (a[1] + b[1]) / 2 - r, maxZ = (a[1] + b[1]) / 2 + r;
        int x0 = (int) Math.floor(minX), z0 = (int) Math.floor(minZ);
        int nx = (int) Math.ceil((maxX - minX) / GRID) + 1, nz = (int) Math.ceil((maxZ - minZ) / GRID) + 1;
        Land land = new Land(level, llama);
        double[] h = new double[nx * nz];
        boolean[] ok = new boolean[nx * nz];
        for (int i = 0; i < nx; i++) {
            for (int j = 0; j < nz; j++) {
                int x = x0 + i * GRID, z = z0 + j * GRID;
                ok[i * nz + j] = Math.hypot(x + 0.5 - a[0], z + 0.5 - a[1]) + Math.hypot(x + 0.5 - b[0], z + 0.5 - b[1])
                        <= stray * Math.hypot(b[0] - a[0], b[1] - a[1]) + strayBlocks;
                if (ok[i * nz + j]) {
                    h[i * nz + j] = land.height(x, z);
                }
            }
        }
        // Smoothed over the twelve blocks round each point: whole blocks on a four-block grid make every climb at least
        // 1 in 4, and a gentle slope reads as steps.
        double[] even = h.clone();
        for (int i = 0; i < nx; i++) {
            for (int j = 0; j < nz; j++) {
                if (!ok[i * nz + j]) {
                    continue;
                }
                double sum = 0;
                int n = 0;
                for (int di = -1; di <= 1; di++) {
                    for (int dj = -1; dj <= 1; dj++) {
                        int a2 = i + di, b2 = j + dj;
                        if (a2 >= 0 && b2 >= 0 && a2 < nx && b2 < nz && ok[a2 * nz + b2]) {
                            sum += h[a2 * nz + b2];
                            n++;
                        }
                    }
                }
                even[i * nz + j] = sum / n;
            }
        }
        h = even;
        int sa = Math.clamp(Math.round((float) ((a[0] - x0) / GRID)), 0, nx - 1) * nz
                + Math.clamp(Math.round((float) ((a[1] - z0) / GRID)), 0, nz - 1);
        int sb = Math.clamp(Math.round((float) ((b[0] - x0) / GRID)), 0, nx - 1) * nz
                + Math.clamp(Math.round((float) ((b[1] - z0) / GRID)), 0, nz - 1);
        ok[sa] = ok[sb] = true;
        h[sa] = land.height(x0 + sa / nz * GRID, z0 + sa % nz * GRID);
        h[sb] = land.height(x0 + sb / nz * GRID, z0 + sb % nz * GRID);
        double[] cost = new double[nx * nz];
        java.util.Arrays.fill(cost, Double.MAX_VALUE);
        int[] came = new int[nx * nz];
        java.util.PriorityQueue<double[]> open = new java.util.PriorityQueue<>(java.util.Comparator.comparingDouble(o -> o[0]));
        cost[sa] = 0;
        open.add(new double[] {0, sa});
        int[][] steps = {{1, 0}, {-1, 0}, {0, 1}, {0, -1}, {1, 1}, {1, -1}, {-1, 1}, {-1, -1}};
        while (!open.isEmpty()) {
            double[] o = open.poll();
            int k = (int) o[1];
            if (k == sb) {
                break;
            }
            if (o[0] - GRID * Math.hypot(k / nz - sb / nz, k % nz - sb % nz) > cost[k] + 1e-9) {
                continue;
            }
            int i = k / nz, j = k % nz;
            for (int[] s : steps) {
                int ni = i + s[0], nj = j + s[1];
                if (ni < 0 || nj < 0 || ni >= nx || nj >= nz || !ok[ni * nz + nj]) {
                    continue;
                }
                int n = ni * nz + nj;
                double c = land.step(x0 + i * GRID, z0 + j * GRID, h[k], x0 + ni * GRID, z0 + nj * GRID, h[n]);
                if (cost[k] + c < cost[n]) {
                    cost[n] = cost[k] + c;
                    came[n] = k;
                    open.add(new double[] {cost[n] + GRID * Math.hypot(ni - sb / nz, nj - sb % nz), n});
                }
            }
        }
        if (cost[sb] == Double.MAX_VALUE) {
            return ends;
        }
        List<double[]> way = new ArrayList<>();
        List<Double> spent = new ArrayList<>();
        for (int k = sb; ; k = came[k]) {
            way.add(new double[] {x0 + k / nz * GRID + 0.5, z0 + k % nz * GRID + 0.5});
            spent.add(cost[k]);
            if (k == sa) {
                break;
            }
        }
        java.util.Collections.reverse(way);
        java.util.Collections.reverse(spent);
        way.set(0, a);
        way.set(way.size() - 1, b);
        // Pulled straight wherever the straight way costs no more (a twentieth's grace) than the planned one.
        List<double[]> straight = new ArrayList<>();
        straight.add(way.get(0));
        int at = 0;
        while (at < way.size() - 1) {
            int next = at + 1;
            for (int j = Math.min(way.size() - 1, at + 64); j > at + 1; j--) {
                double planned = spent.get(j) - spent.get(at);
                if (land.line(way.get(at), way.get(j)) <= planned * 1.05) {
                    next = j;
                    break;
                }
            }
            straight.add(way.get(next));
            at = next;
        }
        // Its corners rounded (Chaikin's cutting, twice).
        List<double[]> round = straight;
        for (int pass = 0; pass < 2; pass++) {
            List<double[]> cut = new ArrayList<>();
            cut.add(round.get(0));
            for (int i = 0; i + 1 < round.size(); i++) {
                double[] p = round.get(i), q = round.get(i + 1);
                cut.add(new double[] {0.75 * p[0] + 0.25 * q[0], 0.75 * p[1] + 0.25 * q[1]});
                cut.add(new double[] {0.25 * p[0] + 0.75 * q[0], 0.25 * p[1] + 0.75 * q[1]});
            }
            cut.add(round.get(round.size() - 1));
            round = cut;
        }
        return round.toArray(new double[0][]);
    }

    /**
     * The land as it was generated, a column at a time (kept): its ground's height under any tree, and whether water
     * lies over it. Its chunks must be loaded; one that isn't reads as water (a road keeps off ground it can't see).
     */
    static final class Land {
        private final ServerLevel level;
        private final double grade, sheer;
        private final java.util.Map<Long, int[]> columns = new java.util.HashMap<>();

        Land(ServerLevel level, boolean llama) {
            this.level = level;
            this.grade = llama ? LLAMA_GRADE : GRADE;
            this.sheer = llama ? LLAMA_SHEER : SHEER;
        }

        private int[] column(int x, int z) {
            return columns.computeIfAbsent(BlockPos.asLong(x, 0, z), k -> {
                if (!level.isLoaded(new BlockPos(x, 0, z))) {
                    return new int[] {level.getSeaLevel(), 1};
                }
                int y = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z) - 1;
                BlockState top = level.getBlockState(new BlockPos(x, y, z));
                while (top.is(net.minecraft.tags.BlockTags.LOGS) && y > level.getMinBuildHeight()) {
                    top = level.getBlockState(new BlockPos(x, --y, z));
                }
                return new int[] {y, top.getFluidState().isEmpty() ? 0 : 1};
            });
        }

        double height(int x, int z) {
            return column(x, z)[0];
        }

        /** The ground's height averaged over the twelve blocks round the place (see {@link #road}'s grid). */
        double even(int x, int z) {
            double sum = 0;
            for (int dx = -GRID; dx <= GRID; dx += GRID) {
                for (int dz = -GRID; dz <= GRID; dz += GRID) {
                    sum += height(x + dx, z + dz);
                }
            }
            return sum / 9;
        }

        boolean wet(int x, int z) {
            return column(x, z)[1] == 1;
        }

        /** What a wagon's step from one column to the next costs. */
        double step(int x1, int z1, double h1, int x2, int z2, double h2) {
            double d = Math.hypot(x2 - x1, z2 - z1);
            double slope = Math.abs(h2 - h1) / d;
            return d * (1 + this.grade * slope * slope + 0.15 * noise(x2, z2)) + (slope > sheer ? SHEER_TOLL : 0)
                    + (wet(x2, z2) ? WET * d : 0);
        }

        /** What a straight way from {@code a} to {@code b} costs, a {@link #GRID}'s step at a time. */
        double line(double[] a, double[] b) {
            double length = Math.hypot(b[0] - a[0], b[1] - a[1]);
            int n = Math.max(1, (int) Math.ceil(length / GRID));
            double total = 0;
            int px = (int) Math.floor(a[0]), pz = (int) Math.floor(a[1]);
            double ph = even(px, pz);
            for (int k = 1; k <= n; k++) {
                int x = (int) Math.floor(a[0] + (b[0] - a[0]) * k / n), z = (int) Math.floor(a[1] + (b[1] - a[1]) * k / n);
                double hh = even(x, z);
                total += step(px, pz, ph, x, z, hh);
                px = x;
                pz = z;
                ph = hh;
            }
            return total;
        }
    }

    /** A road's hexes and those beside them: the ground its line may take. */
    public static java.util.Set<Hex> band(List<Hex> path) {
        java.util.Set<Hex> band = new java.util.HashSet<>(path);
        for (Hex h : path) {
            band.addAll(h.neighbors());
        }
        return band;
    }

    /** Does the straight line from {@code a} to {@code b} stay within {@code corridor}'s hexes (checked every 2 blocks)? */
    static boolean inside(double[] a, double[] b, java.util.Set<Hex> corridor) {
        double length = Math.hypot(b[0] - a[0], b[1] - a[1]);
        int steps = Math.max(1, (int) Math.ceil(length / 2));
        for (int k = 0; k <= steps; k++) {
            double x = a[0] + (b[0] - a[0]) * k / steps, z = a[1] + (b[1] - a[1]) * k / steps;
            if (!corridor.contains(Hex.at(x, z))) {
                return false;
            }
        }
        return true;
    }

    /**
     * The stretch of a road's line that {@code hex} lays: its points a block apart whose nearest of the road's hexes it
     * is, taken in order (a road planned over its land runs where it will, not through the core's hexes alone).
     */
    static double[][] within(double[][] line, Hex hex, List<Hex> path) {
        List<double[]> out = new ArrayList<>();
        int owner = 0;
        for (int i = 0; i + 1 < line.length; i++) {
            double[] a = line[i], b = line[i + 1];
            double length = Math.hypot(b[0] - a[0], b[1] - a[1]);
            int steps = Math.max(1, (int) Math.ceil(length));
            for (int k = 0; k <= steps; k++) {
                double x = a[0] + (b[0] - a[0]) * k / steps, z = a[1] + (b[1] - a[1]) * k / steps;
                // The road's hex nearest the place, never one it has passed.
                int best = owner;
                double near = Double.MAX_VALUE;
                for (int n = owner; n < path.size(); n++) {
                    double d = Math.hypot(x - path.get(n).centerX(), z - path.get(n).centerZ());
                    if (d < near) {
                        near = d;
                        best = n;
                    }
                }
                owner = best;
                if (path.get(owner).equals(hex)) {
                    out.add(new double[] {x, z});
                }
            }
        }
        return out.toArray(new double[0][]);
    }

    /** The chunks a trail's line crosses (to be loaded before it's laid). */
    public static List<ChunkPos> chunks(Trail t, java.util.Map<Long, BlockPos> bells) {
        List<ChunkPos> out = new ArrayList<>();
        for (BlockPos p : columns(line(t, bells), t.width())) {
            ChunkPos c = new ChunkPos(p);
            if (!out.contains(c)) {
                out.add(c);
            }
        }
        return out;
    }

    /** The columns along a polyline, in order, each once: a block's step at a time, {@code width} wide about it. */
    public static List<BlockPos> columns(double[][] points, int width) {
        List<BlockPos> out = new ArrayList<>();
        java.util.Set<Long> seen = new java.util.HashSet<>();
        for (int i = 0; i + 1 < points.length; i++) {
            double x0 = points[i][0], z0 = points[i][1], x1 = points[i + 1][0], z1 = points[i + 1][1];
            double length = Math.hypot(x1 - x0, z1 - z0);
            int steps = Math.max(1, (int) Math.ceil(length * 2));
            // The side the second block lies to: a step across the line.
            double sx = length == 0 ? 0 : -(z1 - z0) / length, sz = length == 0 ? 0 : (x1 - x0) / length;
            for (int k = 0; k <= steps; k++) {
                double x = x0 + (x1 - x0) * k / steps, z = z0 + (z1 - z0) * k / steps;
                for (int w = -(width / 2); w < width - width / 2; w++) {
                    BlockPos p = BlockPos.containing(x + sx * w, 0, z + sz * w);
                    if (seen.add(p.asLong())) {
                        out.add(p);
                    }
                }
            }
        }
        return out;
    }

    /** Laid in the world: a trail's line trodden to path where it's soil. Its chunks must be loaded. */
    public static int lay(ServerLevel level, Trail t, java.util.Map<Long, BlockPos> bells) {
        // Through a town's forum hex, its main street: three wide at the least.
        boolean forum = bells.values().stream().anyMatch(b -> Hex.at(b).equals(t.hex()));
        double[][] line = line(t, bells);
        return layLine(level, line, forum ? Math.max(t.width(), Forums.ROAD_WIDTH) : t.width(), t.path().size() >= 2);
    }

    /** A gentle, smooth value in [0, 1) fixed by the place (a few blocks' worth of wavelength). */
    static double noise(int x, int z) {
        double s = Math.sin(x * 0.11 + z * 0.07) + Math.sin(x * 0.05 - z * 0.13 + 1.7);
        return (s + 2) / 4;
    }

    /**
     * The columns along {@code points} (block x, z), {@code width} wide, trodden to path where their top is soil: a
     * trunk standing there felled, what grows there and the leaves over it cleared, a lone block's bump cut and dip
     * filled to the ground either side. Returns the columns laid.
     */
    public static int layLine(ServerLevel level, double[][] points, int width) {
        return layLine(level, points, width, false);
    }

    /**
     * As {@link #layLine(ServerLevel, double[][], int)}; with {@code bridges}, a road planned across water is decked over
     * it in planks at the surface, so it goes on to the far bank rather than ending at the water's edge.
     */
    public static int layLine(ServerLevel level, double[][] points, int width, boolean bridges) {
        List<BlockPos> cols = columns(points, width);
        int[] top = new int[cols.size()];
        for (int i = 0; i < cols.size(); i++) {
            BlockPos c = cols.get(i);
            int y = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, c.getX(), c.getZ()) - 1;
            while (level.getBlockState(c.atY(y)).is(net.minecraft.tags.BlockTags.LOGS)) {
                level.setBlock(c.atY(y), Blocks.AIR.defaultBlockState(), 3); // a trunk in the way, felled
                y--;
            }
            top[i] = y;
        }
        // Which columns are water (a road's crossings, below).
        boolean[] wet = new boolean[cols.size()];
        for (int i = 0; i < cols.size(); i++) {
            wet[i] = !level.getBlockState(cols.get(i).atY(top[i])).getFluidState().isEmpty();
        }
        int laid = 0;
        // Each run of water the road crosses, by its length (the studio's crossings for roads, thread 0014): a causeway
        // for a ford, a timber trestle to 40, a ferry's stage on each bank past that; never planks across a lake.
        if (bridges) {
            for (int i = 0; i < cols.size(); ) {
                if (!wet[i]) {
                    i++;
                    continue;
                }
                int j = i;
                while (j < cols.size() && wet[j]) {
                    j++;
                }
                laid += cross(level, cols, top, i, j, width);
                i = j;
            }
        }
        // A field in the way is cut through: its crops on the way cleared, and a hedge planted along each side (below).
        java.util.Set<Long> road = new java.util.HashSet<>();
        List<BlockPos> cut = new ArrayList<>();
        for (int i = 0; i < cols.size(); i++) {
            road.add(cols.get(i).asLong());
        }
        for (int i = 0; i < cols.size(); i++) {
            int y = top[i];
            BlockPos c = cols.get(i);
            BlockState ground = level.getBlockState(c.atY(y));
            if (wet[i]) {
                continue; // crossed above, or left as water
            }
            if (sown(level, c.atY(y))) {
                reap(level, c.atY(y));
                level.setBlock(c.atY(y), Blocks.DIRT.defaultBlockState(), 3);
                ground = level.getBlockState(c.atY(y));
                cut.add(c.atY(y));
            }
            if (!soil(ground)) {
                continue;
            }
            // A lone block's bump or dip along the way (the columns a step before and after, WIDTH apart, both level).
            int before = i >= width ? top[i - width] : y, after = i + width < cols.size() ? top[i + width] : y;
            if (before == after && y == before + 1 && soil(level.getBlockState(c.atY(y - 1)))) {
                level.setBlock(c.atY(y), Blocks.AIR.defaultBlockState(), 3);
                y -= 1;
            } else if (before == after && y == before - 1 && level.getBlockState(c.atY(y + 1)).canBeReplaced()
                    && level.getFluidState(c.atY(y + 1)).isEmpty()) {
                level.setBlock(c.atY(y), Blocks.DIRT.defaultBlockState(), 3);
                y += 1;
            }
            BlockState above = level.getBlockState(c.atY(y + 1));
            if (!above.isAir() && above.canBeReplaced() && level.getFluidState(c.atY(y + 1)).isEmpty()) {
                level.setBlock(c.atY(y + 1), Blocks.AIR.defaultBlockState(), 3);
                BlockState higher = level.getBlockState(c.atY(y + 2)); // a tall plant's top half
                if (!higher.isAir() && higher.canBeReplaced() && level.getFluidState(c.atY(y + 2)).isEmpty()) {
                    level.setBlock(c.atY(y + 2), Blocks.AIR.defaultBlockState(), 3);
                }
            }
            for (int h = y + 1; h <= y + HEADROOM; h++) {
                if (level.getBlockState(c.atY(h)).is(net.minecraft.tags.BlockTags.LEAVES)) {
                    level.setBlock(c.atY(h), Blocks.AIR.defaultBlockState(), 3);
                }
            }
            level.setBlock(c.atY(y), Blocks.DIRT_PATH.defaultBlockState(), 3);
            laid++;
        }
        for (BlockPos c : cut) {
            for (int[] d : new int[][] {{1, 0}, {-1, 0}, {0, 1}, {0, -1}}) {
                BlockPos side = c.offset(d[0], 0, d[1]);
                if (!road.contains(BlockPos.asLong(side.getX(), 0, side.getZ())) && sown(level, side)) {
                    reap(level, side);
                    level.setBlock(side, Blocks.GRASS_BLOCK.defaultBlockState(), 3);
                    level.setBlock(side.above(), Furlongs.hedge(Zoning.mix(side.getX(), side.getZ())), 3);
                    laid++;
                }
            }
        }
        return laid;
    }

    /** A road's ground or a hedge planted beside it: never sown over (a field laid or dressed after the road). */
    static boolean kept(ServerLevel level, BlockPos ground) {
        BlockState over = level.getBlockState(ground.above());
        return level.getBlockState(ground).is(Blocks.DIRT_PATH)
                || over.is(net.minecraft.tags.BlockTags.LEAVES) && over.getValue(net.minecraft.world.level.block.LeavesBlock.PERSISTENT);
    }

    /** A field's ground: farmland, or grass under a sown bush or cane. */
    static boolean sown(ServerLevel level, BlockPos ground) {
        BlockState state = level.getBlockState(ground), over = level.getBlockState(ground.above());
        return state.is(Blocks.FARMLAND) || state.is(Blocks.GRASS_BLOCK)
                && (over.is(Blocks.SWEET_BERRY_BUSH) || over.is(Blocks.SUGAR_CANE));
    }

    /** What grows on a field's ground cleared: a crop, a bush, a stand of cane. */
    static void reap(ServerLevel level, BlockPos ground) {
        for (BlockPos up = ground.above(); ; up = up.above()) {
            BlockState s = level.getBlockState(up);
            if (!s.is(net.minecraft.tags.BlockTags.CROPS) && !s.is(Blocks.SWEET_BERRY_BUSH) && !s.is(Blocks.SUGAR_CANE)
                    && !s.is(Blocks.MELON_STEM) && !s.is(Blocks.PUMPKIN_STEM) && !s.is(Blocks.ATTACHED_MELON_STEM)
                    && !s.is(Blocks.ATTACHED_PUMPKIN_STEM)) {
                return;
            }
            level.setBlock(up, Blocks.AIR.defaultBlockState(), 2);
        }
    }

    /** The longest water a road fords on a causeway, and the longest it bridges, in blocks along it. */
    static final int FORD = 3, TRESTLE = 40;

    /**
     * A road's crossing of the run of water {@code cols[from..to)}: its length along the road the run's columns over its
     * width. A causeway of fieldstone flush with the water for a ford; a timber trestle two over the water to 40 (spruce
     * posts down to the bed every 4 along, a rail on its edge columns, a lantern on every 8th post); past that, a ferry's
     * stage (planks on posts, a lantern) at each bank. The columns laid.
     */
    static int cross(ServerLevel level, List<BlockPos> cols, int[] top, int from, int to, int width) {
        int span = Math.max(1, (to - from + width - 1) / Math.max(1, width)), water = Integer.MIN_VALUE, laid = 0;
        for (int k = from; k < to; k++) {
            water = Math.max(water, top[k]);
        }
        java.util.Set<Long> deck = new java.util.HashSet<>();
        if (span <= FORD) {
            for (int k = from; k < to; k++) {
                BlockPos c = cols.get(k);
                for (int y = water; y > level.getMinBuildHeight(); y--) {
                    BlockState b = level.getBlockState(c.atY(y));
                    if (b.getFluidState().isEmpty() && !b.isAir() && !b.canBeReplaced()) {
                        break;
                    }
                    level.setBlock(c.atY(y), y == water ? Blocks.DIRT_PATH.defaultBlockState()
                            : (y + c.getX() + c.getZ()) % 3 == 0 ? Blocks.MOSSY_COBBLESTONE.defaultBlockState()
                            : Blocks.COBBLESTONE.defaultBlockState(), 3);
                }
                laid++;
            }
            return laid;
        }
        if (span > TRESTLE) {
            // A ferry's stage at each bank: the first and last few columns over the water decked on posts.
            for (int k : new int[] {from, to - 1}) {
                for (int d = 0; d < Math.min(3 * Math.max(1, width), to - from); d++) {
                    int at = k == from ? from + d : to - 1 - d;
                    BlockPos c = cols.get(at);
                    level.setBlock(c.atY(water + 1), Blocks.SPRUCE_PLANKS.defaultBlockState(), 3);
                    if (d % 3 == 0) {
                        level.setBlock(c.atY(water), Blocks.SPRUCE_LOG.defaultBlockState(), 3);
                    }
                    laid++;
                }
                BlockPos post = cols.get(k).atY(water + 2);
                level.setBlock(post, Blocks.SPRUCE_FENCE.defaultBlockState(), 3);
                level.setBlock(post.above(), Blocks.LANTERN.defaultBlockState(), 3);
            }
            return laid;
        }
        // A trestle: the deck two over the water, posts to the bed every 4 along, a rail on the deck's edges.
        int d = water + 2;
        for (int k = from; k < to; k++) {
            deck.add(cols.get(k).asLong());
        }
        for (int k = from; k < to; k++) {
            BlockPos c = cols.get(k);
            for (int y = d + 1; y <= d + 3; y++) {
                level.setBlock(c.atY(y), Blocks.AIR.defaultBlockState(), 3);
            }
            level.setBlock(c.atY(d), Blocks.SPRUCE_PLANKS.defaultBlockState(), 3);
            int along = (k - from) / Math.max(1, width);
            boolean edge = false;
            for (var dir : net.minecraft.core.Direction.Plane.HORIZONTAL) {
                BlockPos n = c.relative(dir);
                edge |= !deck.contains(n.asLong()) && !level.getFluidState(n.atY(water)).isEmpty();
            }
            if (along % 4 == 0 && edge) {
                for (int y = d - 1; y > level.getMinBuildHeight(); y--) {
                    BlockState b = level.getBlockState(c.atY(y));
                    if (b.getFluidState().isEmpty() && !b.isAir() && !b.canBeReplaced()) {
                        break;
                    }
                    level.setBlock(c.atY(y), Blocks.SPRUCE_LOG.defaultBlockState(), 3);
                }
            }
            // A passing place every 16 on a long trestle (the studio's, thread 0014: wagons meet on long crossings): a bay
            // 2 wide off the deck's edge, its outer rail lit, the rail open to it.
            boolean bay = false;
            if (edge && span > 20 && along % 16 == 8) {
                for (var dir : net.minecraft.core.Direction.Plane.HORIZONTAL) {
                    BlockPos n = c.relative(dir);
                    if (deck.contains(n.asLong()) || level.getFluidState(n.atY(water)).isEmpty()) {
                        continue;
                    }
                    for (int w = 1; w <= 2; w++) {
                        BlockPos b = c.relative(dir, w);
                        level.setBlock(b.atY(d), Blocks.SPRUCE_PLANKS.defaultBlockState(), 3);
                        level.setBlock(b.atY(d + 1), Blocks.AIR.defaultBlockState(), 3);
                    }
                    BlockPos rail = c.relative(dir, 3);
                    level.setBlock(rail.atY(d), Blocks.SPRUCE_PLANKS.defaultBlockState(), 3);
                    level.setBlock(rail.atY(d + 1), Blocks.SPRUCE_FENCE.defaultBlockState(), 3);
                    level.setBlock(rail.atY(d + 2), Blocks.LANTERN.defaultBlockState(), 3);
                    for (int y = d - 1; y > level.getMinBuildHeight(); y--) {
                        BlockState under = level.getBlockState(rail.atY(y));
                        if (under.getFluidState().isEmpty() && !under.isAir() && !under.canBeReplaced()) {
                            break;
                        }
                        level.setBlock(rail.atY(y), Blocks.SPRUCE_LOG.defaultBlockState(), 3);
                    }
                    bay = true;
                }
            }
            if (edge && !bay) {
                level.setBlock(c.atY(d + 1), Blocks.SPRUCE_FENCE.defaultBlockState(), 3);
                if (along % 8 == 0) {
                    level.setBlock(c.atY(d + 2), Blocks.LANTERN.defaultBlockState(), 3);
                }
            }
            laid++;
        }
        return laid;
    }

    /** Ground a wagon's wheels tread to path: the soil a world grows grass on. */
    static boolean soil(BlockState state) {
        return state.is(Blocks.GRASS_BLOCK) || state.is(Blocks.DIRT) || state.is(Blocks.COARSE_DIRT)
                || state.is(Blocks.PODZOL) || state.is(Blocks.MYCELIUM) || state.is(Blocks.ROOTED_DIRT)
                || state.is(Blocks.DIRT_PATH);
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        for (Trail t : waiting) {
            CompoundTag e = new CompoundTag();
            e.putIntArray("hexes", new int[] {t.hex().q(), t.hex().r(), t.from().q(), t.from().r(), t.to().q(), t.to().r()});
            e.putInt("width", t.width());
            int[] road = new int[2 * t.path().size()];
            for (int i = 0; i < t.path().size(); i++) {
                road[2 * i] = t.path().get(i).q();
                road[2 * i + 1] = t.path().get(i).r();
            }
            e.putIntArray("path", road);
            e.putBoolean("llama", t.llama());
            list.add(e);
        }
        tag.put("waiting", list);
        return tag;
    }

    public static Trails load(CompoundTag tag, HolderLookup.Provider registries) {
        Trails trails = new Trails();
        for (Tag t : tag.getList("waiting", Tag.TAG_COMPOUND)) {
            int[] h = ((CompoundTag) t).getIntArray("hexes");
            int width = ((CompoundTag) t).contains("width") ? ((CompoundTag) t).getInt("width") : 2;
            int[] road = ((CompoundTag) t).getIntArray("path");
            List<Hex> path = new ArrayList<>();
            for (int i = 0; i + 1 < road.length; i += 2) {
                path.add(new Hex(road[i], road[i + 1]));
            }
            if (h.length == 6) {
                trails.waiting.add(new Trail(new Hex(h[0], h[1]), new Hex(h[2], h[3]), new Hex(h[4], h[5]), width,
                        List.copyOf(path), ((CompoundTag) t).getBoolean("llama")));
            }
        }
        return trails;
    }
}
