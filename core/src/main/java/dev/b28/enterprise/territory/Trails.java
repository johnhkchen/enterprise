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
 * and a lone block's bump or dip on the way levelled. Sand, stone, water and anything built are left as they are.
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
    public record Trail(Hex hex, Hex from, Hex to, int width, List<Hex> path) {
        public Trail(Hex hex, Hex from, Hex to, int width) {
            this(hex, from, to, width, List.of());
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
            double[][] stretch = within(taut(t.path(), bells), t.hex());
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
     * stays within the road's own hexes (the ground the core planned it over), turning only where they turn. Greedy
     * string pulling over the hexes' centres, the line checked every two blocks.
     */
    public static double[][] taut(List<Hex> path, java.util.Map<Long, BlockPos> bells) {
        java.util.Set<Hex> corridor = new java.util.HashSet<>(path);
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

    /** The stretch of a polyline within {@code hex}: its points a block apart there, from where it enters to where it leaves. */
    static double[][] within(double[][] line, Hex hex) {
        List<double[]> out = new ArrayList<>();
        for (int i = 0; i + 1 < line.length; i++) {
            double[] a = line[i], b = line[i + 1];
            double length = Math.hypot(b[0] - a[0], b[1] - a[1]);
            int steps = Math.max(1, (int) Math.ceil(length));
            for (int k = 0; k <= steps; k++) {
                double x = a[0] + (b[0] - a[0]) * k / steps, z = a[1] + (b[1] - a[1]) * k / steps;
                if (Hex.at(x, z).equals(hex)) {
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
        if (t.path().size() >= 2 && line.length >= 2) {
            double[][] way = wind(level, line[0], line[line.length - 1], new java.util.HashSet<>(t.path()), t.hex());
            if (way != null) {
                line = way;
            }
        }
        return layLine(level, line, forum ? Math.max(t.width(), Forums.ROAD_WIDTH) : t.width());
    }

    /** What a climb of a block costs a wagon, a block's step (squared in the climb: a gentle grade is cheap, a steep dear). */
    static final double CLIMB = 3;
    /** A climb of more than this in a step is a cliff to go round, at {@link #CLIFF}. */
    static final int STEEP = 1;
    static final double CLIFF = 40;
    /** A ford's cost a block, a felled tree's, and an old path's share of a step's (a road keeps to the way worn). */
    static final double FORD = 30;
    static final double FELL = 1.5;
    static final double WORN = 0.7;
    /** The meander on the flat: a gentle cost, fixed by the ground's place, that bends a road a little. */
    static final double MEANDER = 0.35;

    /**
     * A wagon's way across {@code hex} from {@code a} to {@code b} (where the road's taut line enters and leaves it):
     * the cheapest over its ground's columns, a step costing its length, more as it climbs (gentle grades cheap, a
     * cliff dear, so it goes round a hill), a ford, a tree to fell; less on a path already worn; and a gentle meander
     * on the flat (John: "a little more wagon-optimized ... low elevation change, gentle slopes. A bit of winding").
     * Over the road's own hexes near {@code hex}, its ground loaded. Null if there's no way (the caller lays it taut).
     */
    static double[][] wind(ServerLevel level, double[] a, double[] b, java.util.Set<Hex> corridor, Hex hex) {
        int reach = (int) Math.ceil(Hex.SIZE) + 6;
        int x0 = (int) Math.floor(hex.centerX()) - reach, z0 = (int) Math.floor(hex.centerZ()) - reach;
        int span = 2 * reach + 1;
        int[] height = new int[span * span];
        double[] extra = new double[span * span];
        for (int i = 0; i < span; i++) {
            for (int j = 0; j < span; j++) {
                int x = x0 + i, z = z0 + j, k = i * span + j;
                if (!corridor.contains(Hex.at(x + 0.5, z + 0.5)) || !level.isLoaded(new BlockPos(x, 0, z))) {
                    height[k] = Integer.MIN_VALUE;
                    continue;
                }
                int y = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z) - 1;
                height[k] = y;
                BlockState top = level.getBlockState(new BlockPos(x, y, z));
                double e = 0;
                if (!top.getFluidState().isEmpty()) {
                    e += FORD;
                } else if (top.is(net.minecraft.tags.BlockTags.LOGS)) {
                    e += FELL;
                } else if (!soil(top) && !dev.b28.enterprise.settlement.Grounds.natural(top)) {
                    e += CLIFF; // something built: around it
                }
                if (level.getBlockState(new BlockPos(x, y, z)).is(Blocks.DIRT_PATH)) {
                    e -= 1 - WORN;
                }
                e += MEANDER * noise(x, z);
                extra[k] = e;
            }
        }
        int sa = cell(a, x0, z0, span), sb = cell(b, x0, z0, span);
        if (sa < 0 || sb < 0 || height[sa] == Integer.MIN_VALUE || height[sb] == Integer.MIN_VALUE) {
            return null;
        }
        double[] cost = new double[span * span];
        java.util.Arrays.fill(cost, Double.MAX_VALUE);
        int[] came = new int[span * span];
        java.util.PriorityQueue<double[]> open = new java.util.PriorityQueue<>(java.util.Comparator.comparingDouble(o -> o[0]));
        cost[sa] = 0;
        open.add(new double[] {dist(sa, sb, span), sa});
        int[][] steps = {{1, 0}, {-1, 0}, {0, 1}, {0, -1}, {1, 1}, {1, -1}, {-1, 1}, {-1, -1}};
        while (!open.isEmpty()) {
            double[] o = open.poll();
            int k = (int) o[1];
            if (k == sb) {
                break;
            }
            if (o[0] - dist(k, sb, span) > cost[k] + 1e-9) {
                continue;
            }
            int i = k / span, j = k % span;
            for (int[] s : steps) {
                int ni = i + s[0], nj = j + s[1];
                if (ni < 0 || nj < 0 || ni >= span || nj >= span) {
                    continue;
                }
                int n = ni * span + nj;
                if (height[n] == Integer.MIN_VALUE) {
                    continue;
                }
                double step = s[0] != 0 && s[1] != 0 ? Math.sqrt(2) : 1;
                int climb = Math.abs(height[n] - height[k]);
                double c = step * (1 + Math.max(-0.6, extra[n])) + CLIMB * climb * climb + (climb > STEEP ? CLIFF : 0);
                if (cost[k] + c < cost[n]) {
                    cost[n] = cost[k] + c;
                    came[n] = k;
                    open.add(new double[] {cost[n] + dist(n, sb, span), n});
                }
            }
        }
        if (cost[sb] == Double.MAX_VALUE) {
            return null;
        }
        List<double[]> way = new ArrayList<>();
        for (int k = sb; ; k = came[k]) {
            way.add(new double[] {x0 + k / span + 0.5, z0 + k % span + 0.5});
            if (k == sa) {
                break;
            }
        }
        java.util.Collections.reverse(way);
        return way.toArray(new double[0][]);
    }

    private static int cell(double[] p, int x0, int z0, int span) {
        int i = (int) Math.floor(p[0]) - x0, j = (int) Math.floor(p[1]) - z0;
        return i < 0 || j < 0 || i >= span || j >= span ? -1 : i * span + j;
    }

    private static double dist(int a, int b, int span) {
        return Math.hypot(a / span - b / span, a % span - b % span);
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
        int laid = 0;
        for (int i = 0; i < cols.size(); i++) {
            int y = top[i];
            BlockPos c = cols.get(i);
            BlockState ground = level.getBlockState(c.atY(y));
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
                        List.copyOf(path)));
            }
        }
        return trails;
    }
}
