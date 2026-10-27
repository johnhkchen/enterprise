package dev.b28.enterprise.territory;

import dev.b28.enterprise.hex.Hex;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.TicketType;
import net.minecraft.tags.BlockTags;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.entity.ai.village.poi.PoiRecord;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Each town's field map (core mode; John: "plots of farmland surrounding the old town square, something like a
 * voronoi"), in rings round its plan's walls ({@link TownPlans}): gardens between the inner wall and the outer, grain
 * outside the outer, pastures beyond ({@link Pastures}). It grows in phases (John: "some at founding, more as town
 * grows. Farms form along branch paths that connect to trade paths"): a farm track leaves one of its ring's trade paths
 * (an avenue, its road on out through the outer gate, or a caravan road) across it, routed over the land as a road is
 * ({@link Trails#between}), and holdings lie along it either side, each the ground nearer its seed than any other
 * holding's (a Voronoi parcel: straight edges at odd angles where they meet). A holding is farmed in strips running
 * away from its track, square to the compass, a field wide with a ditch down the middle and a balk between: the open
 * field. A field is one strip's field-long stretch, those by the track first; when a ring's holdings are full the town
 * opens its next track, nearest the bell first.
 *
 * <p>Saved with the world: each town's tracks and holdings, and the fields taken.
 */
public final class FieldMaps extends SavedData {
    private static final String NAME = "enterprise_field_maps";

    /** A ring of the town's farmland: its gardens between its walls, its grain outside them. */
    public enum Ring { GARDEN, GRAIN }

    /** How deep the grain ring runs beyond the outer wall's strip. */
    public static final int GRAIN_DEPTH = 40;
    /** How far apart tracks leave a trade path; holdings' seeds along a track, and how far off it. */
    static final int SPACING = 40;
    static final int SEED = 24;
    static final int OFFSET = 13;
    /** A track's longest run and its width; the least run worth a track. */
    static final int TRACK = 44;
    static final int TRACK_WIDTH = 2;
    static final int SHORTEST = 16;
    /** A strip: a field wide, a balk between two, so many fields long, begun so far from its track. */
    static final int WIDE = Fields.SIZE;
    static final int BALK = 1;
    static final int LENGTHS = 4;
    static final int SETBACK = 3;
    /** Strips each way from a holding's middle one. */
    static final int STRIPS = 3;
    /** Tracks opened in each ring when the town's plan is made. */
    static final int FOUNDING = 3;
    /** How far a holding reaches from its seed at most. */
    static final int HOLDING = 40;
    /** How far a field's ground may rise and fall, to be filled level. */
    static final int LEVELLED = 2;

    /** A farm track: its ring, the trade path it leaves (an index into {@link #tradePaths}) and its line as laid. */
    public record Track(Ring ring, int path, double[][] line) {}

    /**
     * A holding: its seed, its track (an index), whether its strips run north-south, and which way along them is away
     * from its track (+1 or -1).
     */
    public record Parcel(int x, int z, int track, boolean northSouth, int away) {}

    /** A town's map. */
    static final class TownMap {
        final List<Track> tracks = new ArrayList<>();
        final List<Parcel> parcels = new ArrayList<>();
        final Set<Long> taken = new HashSet<>();
        /** Slots dressed with the kingdom's crop-free field tiles, till a field is planted there. */
        final Map<Long, Integer> dressed = new HashMap<>();
        /** Slots judged for dressing and found unfit (off the map, or ground too rough): not looked at again. */
        final Set<Long> undressable = new HashSet<>();
        /** Its farmsteads' lots ({x0, z0, x1, z1}), by holding. */
        final Map<Integer, int[]> farmsteads = new java.util.TreeMap<>();
        /** The grain ring's roads on out through the outer gates laid (avenue indices). */
        final Set<Integer> roads = new HashSet<>();
        boolean founded;
    }

    private final Map<Long, TownMap> maps = new LinkedHashMap<>();
    /** The way each field found lies (packed corner: whether its strips run north-south), till it's planted. */
    private final Map<Long, Boolean> sited = new HashMap<>();

    private static final TicketType<ChunkPos> SURVEYING =
            TicketType.create("enterprise_fields", Comparator.comparingLong(ChunkPos::toLong), 1200);

    public static FieldMaps get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(FieldMaps::new, FieldMaps::load, null), NAME);
    }

    /** Each town's tracks and holdings, for the map. */
    public Map<Long, List<Track>> tracks() {
        Map<Long, List<Track>> out = new LinkedHashMap<>();
        maps.forEach((town, m) -> out.put(town, List.copyOf(m.tracks)));
        return out;
    }

    /** The ring a crop is grown in first: roots, greens and berries in the gardens, grain and cane in the open fields. */
    public static Ring ringOf(Item crop) {
        return crop == Items.WHEAT || crop == Items.SUGAR_CANE || crop == Items.MELON_SLICE || crop == Items.PUMPKIN
                ? Ring.GRAIN : Ring.GARDEN;
    }

    /** Is the column in the ring's band of the plan: off both walls' strips, between them or beyond the outer? */
    static boolean in(TownPlans.Plan p, Ring ring, double x, double z) {
        int bx = (int) Math.floor(x), bz = (int) Math.floor(z);
        int margin = TownPlans.STRIP + 1;
        TownPlans.Rect inner = p.inner(), outer = p.outer();
        return switch (ring) {
            case GARDEN -> !inner.contains(bx, bz) && inner.fromEdge(bx, bz) > margin && outer.contains(bx, bz)
                    && outer.fromEdge(bx, bz) > margin;
            case GRAIN -> !outer.contains(bx, bz) && outer.fromEdge(bx, bz) > margin
                    && outer.fromEdge(bx, bz) <= margin + GRAIN_DEPTH;
        };
    }

    /**
     * The ring's trade paths, as lines (block x, z): its four avenues between the walls; beyond them, the avenues' roads
     * on out through the outer gates across the grain ring, then the caravan roads planned through it.
     */
    static List<double[][]> tradePaths(TownPlans.Plan p, Ring ring) {
        double bx = p.bell().getX() + 0.5, bz = p.bell().getZ() + 0.5;
        TownPlans.Rect i = p.inner(), o = p.outer();
        int m = TownPlans.STRIP + 1;
        List<double[][]> out = new ArrayList<>();
        if (ring == Ring.GARDEN) {
            out.add(new double[][] {{bx, i.z0() - m + 0.5}, {bx, o.z0() + m + 0.5}});
            out.add(new double[][] {{bx, i.z1() + m + 0.5}, {bx, o.z1() - m + 0.5}});
            out.add(new double[][] {{i.x0() - m + 0.5, bz}, {o.x0() + m + 0.5, bz}});
            out.add(new double[][] {{i.x1() + m + 0.5, bz}, {o.x1() - m + 0.5, bz}});
            return out;
        }
        int far = m + GRAIN_DEPTH;
        out.add(new double[][] {{bx, o.z0() + 0.5}, {bx, o.z0() - far + 0.5}});
        out.add(new double[][] {{bx, o.z1() + 0.5}, {bx, o.z1() + far + 0.5}});
        out.add(new double[][] {{o.x0() + 0.5, bz}, {o.x0() - far + 0.5, bz}});
        out.add(new double[][] {{o.x1() + 0.5, bz}, {o.x1() + far + 0.5, bz}});
        // The caravan roads' stretches through the ring, in the order planned.
        List<double[][]> roads = new ArrayList<>(Trails.PLANNED.values());
        roads.sort(Comparator.comparingDouble((double[][] r) -> r[0][0]).thenComparingDouble(r -> r[0][1]));
        for (double[][] road : roads) {
            List<double[]> run = new ArrayList<>();
            for (double[] pt : resample(road, 2)) {
                if (in(p, ring, pt[0], pt[1])) {
                    run.add(pt);
                } else {
                    if (run.size() * 2 >= SPACING) {
                        out.add(run.toArray(new double[0][]));
                    }
                    run = new ArrayList<>();
                }
            }
            if (run.size() * 2 >= SPACING) {
                out.add(run.toArray(new double[0][]));
            }
        }
        return out;
    }

    /** A line's points every {@code step} blocks along it, its ends kept. */
    static List<double[]> resample(double[][] line, double step) {
        List<double[]> out = new ArrayList<>();
        if (line.length == 0) {
            return out;
        }
        out.add(line[0]);
        double carry = 0;
        for (int k = 0; k + 1 < line.length; k++) {
            double[] a = line[k], b = line[k + 1];
            double len = Math.hypot(b[0] - a[0], b[1] - a[1]);
            double s = step - carry;
            while (s <= len) {
                out.add(new double[] {a[0] + (b[0] - a[0]) * s / len, a[1] + (b[1] - a[1]) * s / len});
                s += step;
            }
            carry = len - (s - step);
        }
        double[] last = line[line.length - 1];
        if (Math.hypot(last[0] - out.get(out.size() - 1)[0], last[1] - out.get(out.size() - 1)[1]) > 0.5) {
            out.add(last);
        }
        return out;
    }

    /** How far the place is from a line. */
    static double from(double[][] line, double x, double z) {
        double best = Double.MAX_VALUE;
        for (int k = 0; k + 1 < line.length; k++) {
            double ax = line[k][0], az = line[k][1], dx = line[k + 1][0] - ax, dz = line[k + 1][1] - az;
            double len2 = dx * dx + dz * dz;
            double t = len2 == 0 ? 0 : Math.max(0, Math.min(1, ((x - ax) * dx + (z - az) * dz) / len2));
            best = Math.min(best, Math.hypot(x - ax - t * dx, z - az - t * dz));
        }
        return line.length == 1 ? Math.hypot(x - line[0][0], z - line[0][1]) : best;
    }

    /** Does the segment from {@code a} to {@code b} cross the line? */
    static boolean crosses(double[] a, double[] b, double[][] line) {
        for (int k = 0; k + 1 < line.length; k++) {
            double[] c = line[k], d = line[k + 1];
            double d1 = side(c, d, a), d2 = side(c, d, b), d3 = side(a, b, c), d4 = side(a, b, d);
            if (d1 * d2 < 0 && d3 * d4 < 0) {
                return true;
            }
        }
        return false;
    }

    private static double side(double[] p, double[] q, double[] r) {
        return (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    }

    /** A track that might be opened: where it leaves its trade path (an index), the way it runs, and its start's reach. */
    record Branch(int path, double[] from, double[] way, double reach) {}

    /** The ring's tracks that might still be opened, nearest the bell first. */
    static List<Branch> branches(TownPlans.Plan p, Ring ring, TownMap m) {
        double bx = p.bell().getX() + 0.5, bz = p.bell().getZ() + 0.5;
        List<double[][]> paths = tradePaths(p, ring);
        List<Branch> out = new ArrayList<>();
        for (int k = 0; k < paths.size(); k++) {
            List<double[]> pts = resample(paths.get(k), 1);
            for (int s = SPACING / 2; s < pts.size() - 1; s += SPACING) {
                double[] a = pts.get(Math.max(0, s - 2)), b = pts.get(Math.min(pts.size() - 1, s + 2));
                double len = Math.hypot(b[0] - a[0], b[1] - a[1]);
                if (len == 0) {
                    continue;
                }
                double tx = (b[0] - a[0]) / len, tz = (b[1] - a[1]) / len;
                double[] at = pts.get(s);
                for (double[] way : new double[][] {{-tz, tx}, {tz, -tx}}) {
                    out.add(new Branch(k, at, way, Math.hypot(at[0] - bx, at[1] - bz)));
                }
            }
        }
        // Not where a track already runs: none starting near this one's start, none it would start beside.
        out.removeIf(b -> m.tracks.stream().anyMatch(t -> t.ring() == ring
                && from(t.line(), b.from()[0] + b.way()[0] * SEED / 2.0, b.from()[1] + b.way()[1] * SEED / 2.0) < SEED));
        out.sort(Comparator.comparingDouble(Branch::reach).thenComparingDouble(b -> b.from()[0])
                .thenComparingDouble(b -> b.from()[1]).thenComparingDouble(b -> b.way()[0]));
        return out;
    }

    /** Where a branch's track would end: as far as {@link #TRACK} along its way while in its ring. */
    static double[] end(TownPlans.Plan p, Ring ring, Branch b) {
        double[] end = null;
        for (int n = 3; n <= TRACK; n++) {
            double x = b.from()[0] + b.way()[0] * n, z = b.from()[1] + b.way()[1] * n;
            if (!in(p, ring, x, z)) {
                break;
            }
            end = new double[] {x, z};
        }
        return end != null && Math.hypot(end[0] - b.from()[0], end[1] - b.from()[1]) >= SHORTEST ? end : null;
    }

    /** A value in [-1, 1) fixed by the numbers. */
    static double jitter(long a, long b, long c) {
        long h = a * 0x9E3779B97F4A7C15L ^ b * 0xC2B2AE3D27D4EB4FL ^ c * 0x165667B19E3779F9L;
        h ^= h >>> 31;
        h *= 0xBF58476D1CE4E5B9L;
        h ^= h >>> 29;
        return ((h >>> 11) / (double) (1L << 53)) * 2 - 1;
    }

    /** The holdings along a track: a seed every {@link #SEED} along it, either side, a little off true. */
    static List<Parcel> parcels(TownPlans.Plan p, Ring ring, int index, double[][] line, long town) {
        List<double[]> pts = resample(line, 1);
        List<Parcel> out = new ArrayList<>();
        for (int s = SEED / 2; s < pts.size(); s += SEED) {
            double[] a = pts.get(Math.max(0, s - 2)), b = pts.get(Math.min(pts.size() - 1, s + 2));
            double len = Math.hypot(b[0] - a[0], b[1] - a[1]);
            if (len == 0) {
                continue;
            }
            double tx = (b[0] - a[0]) / len, tz = (b[1] - a[1]) / len;
            double[] q = pts.get(s);
            for (int side : new int[] {-1, 1}) {
                double along = 6 * jitter(town, Math.round(q[0]), Math.round(q[1]) * 2 + side);
                double across = OFFSET + 4 * jitter(town, Math.round(q[1]), Math.round(q[0]) * 2 - side);
                double x = q[0] + tx * along - tz * side * across, z = q[1] + tz * along + tx * side * across;
                if (!in(p, ring, x, z)) {
                    continue;
                }
                // Strips run away from the track, square to the compass: north-south off a track running east-west.
                boolean northSouth = Math.abs(tx) >= Math.abs(tz);
                int away = northSouth ? (z >= q[1] ? 1 : -1) : (x >= q[0] ? 1 : -1);
                out.add(new Parcel((int) Math.floor(x), (int) Math.floor(z), index, northSouth, away));
            }
        }
        return out;
    }

    /** A field's place: its corner (x, z) and whether its strip runs north-south. */
    record Slot(int x, int z, boolean northSouth) {}

    /** A holding's fields in the order they're farmed: nearest its track first, its middle strip's first. */
    static List<Slot> slots(Parcel parcel, double[][] track) {
        List<double[]> pts = resample(track, 1);
        List<int[]> order = new ArrayList<>();
        for (int n = 0; n < LENGTHS; n++) {
            for (int k = 0; k <= STRIPS; k++) {
                order.add(new int[] {n, k});
                if (k > 0) {
                    order.add(new int[] {n, -k});
                }
            }
        }
        List<Slot> out = new ArrayList<>();
        for (int[] nk : order) {
            int n = nk[0], k = nk[1];
            // Across: the strip's corner; along: from the track beside it (its edge nearest the holding), or the seed's.
            int across = (parcel.northSouth() ? parcel.x() : parcel.z()) - WIDE / 2 + k * (WIDE + BALK);
            double mid = across + WIDE / 2.0;
            double edge = Double.NaN;
            for (double[] pt : pts) {
                double a = parcel.northSouth() ? pt[0] : pt[1], b = parcel.northSouth() ? pt[1] : pt[0];
                if (Math.abs(a - mid) <= WIDE / 2.0 + 1) {
                    edge = Double.isNaN(edge) ? b : parcel.away() > 0 ? Math.max(edge, b) : Math.min(edge, b);
                }
            }
            if (Double.isNaN(edge)) {
                edge = (parcel.northSouth() ? parcel.z() : parcel.x()) - parcel.away() * OFFSET;
            }
            int near = (int) Math.floor(edge) + parcel.away() * (SETBACK + 1);
            int along = parcel.away() > 0 ? near + n * WIDE : near - WIDE + 1 - n * WIDE;
            out.add(parcel.northSouth() ? new Slot(across, along, true) : new Slot(along, across, false));
        }
        return out;
    }

    /**
     * May a field go in this slot of the holding, by the map alone: every part of it nearer the holding's seed than any
     * other's, in its ring, off every track and trade path, and off the town plans' walls, avenues and lanes?
     */
    static boolean open(ServerLevel level, TownPlans.Plan p, Ring ring, TownMap m, int parcel, Slot s,
            List<double[][]> paths) {
        if (m.taken.contains(BlockPos.asLong(s.x(), 0, s.z()))) {
            return false;
        }
        Parcel own = m.parcels.get(parcel);
        // On its seed's side of every road and every other track: a holding is bounded by them.
        double[] seed = {own.x() + 0.5, own.z() + 0.5}, mid = {s.x() + WIDE / 2.0, s.z() + WIDE / 2.0};
        for (double[][] path : paths) {
            if (crosses(seed, mid, path)) {
                return false;
            }
        }
        for (int t = 0; t < m.tracks.size(); t++) {
            if (t != own.track() && crosses(seed, mid, m.tracks.get(t).line())) {
                return false;
            }
        }
        for (int dx = 0; dx < WIDE; dx += 2) {
            for (int dz = 0; dz < WIDE; dz += 2) {
                double x = s.x() + dx + 0.5, z = s.z() + dz + 0.5;
                if (!in(p, ring, x, z)) {
                    return false;
                }
                double mine = Math.hypot(x - own.x() - 0.5, z - own.z() - 0.5);
                if (mine > HOLDING) {
                    return false;
                }
                for (int i = 0; i < m.parcels.size(); i++) {
                    Parcel o = m.parcels.get(i);
                    double theirs = Math.hypot(x - o.x() - 0.5, z - o.z() - 0.5);
                    if (i != parcel && (theirs < mine || theirs == mine && i < parcel)) {
                        return false;
                    }
                }
                for (Track t : m.tracks) {
                    if (from(t.line(), x, z) < TRACK_WIDTH / 2.0 + 1.5) {
                        return false;
                    }
                }
                for (double[][] path : paths) {
                    if (from(path, x, z) < TownPlans.GATE / 2.0 + 2) {
                        return false;
                    }
                }
            }
        }
        return TownPlans.clear(level, s.x(), s.z(), WIDE, WIDE, true);
    }

    /**
     * A track's fields as its town's map would have them were it the map's only track, in the order farmed: each a
     * corner (x, z) and 1 if its strip runs north-south. The ground isn't looked at.
     */
    public static List<int[]> fieldsAlong(ServerLevel level, TownPlans.Plan plan, Ring ring, double[][] track, long town) {
        TownMap m = new TownMap();
        m.tracks.add(new Track(ring, -1, track));
        m.parcels.addAll(parcels(plan, ring, 0, track, town));
        List<double[][]> paths = tradePaths(plan, ring);
        List<int[]> out = new ArrayList<>();
        for (int i = 0; i < m.parcels.size(); i++) {
            for (Slot s : slots(m.parcels.get(i), track)) {
                if (open(level, plan, ring, m, i, s, paths)) {
                    out.add(new int[] {s.x(), s.z(), s.northSouth() ? 1 : 0});
                }
            }
        }
        return out;
    }

    /**
     * The ground's top under a field's footprint, if it can be farmed: loaded, the land's own soil or sand (no rock,
     * water, path or field) under any trees, within {@link #LEVELLED} of level and {@link Growth#FIELD_RISE} of the
     * bell's height. Else {@link Integer#MIN_VALUE}. Changes nothing.
     */
    static int ground(ServerLevel level, int x0, int z0, int bellY) {
        int top = Integer.MIN_VALUE, bottom = Integer.MAX_VALUE;
        for (int dx = 0; dx < WIDE; dx++) {
            for (int dz = 0; dz < WIDE; dz++) {
                int y = under(level, x0 + dx, z0 + dz);
                if (y == Integer.MIN_VALUE) {
                    return Integer.MIN_VALUE;
                }
                BlockState state = level.getBlockState(new BlockPos(x0 + dx, y, z0 + dz));
                if (!dev.b28.enterprise.settlement.Grounds.natural(state) || state.is(Blocks.FARMLAND)
                        || state.is(Blocks.DIRT_PATH) || state.is(BlockTags.BASE_STONE_OVERWORLD)
                        || !state.getFluidState().isEmpty()) {
                    return Integer.MIN_VALUE;
                }
                top = Math.max(top, y);
                bottom = Math.min(bottom, y);
            }
        }
        return top - bottom <= LEVELLED && Math.abs(top - bellY) <= Growth.FIELD_RISE ? top : Integer.MIN_VALUE;
    }

    /** The ground's height at a column under any tree on it; {@link Integer#MIN_VALUE} if it isn't loaded. */
    static int under(ServerLevel level, int x, int z) {
        if (!level.isLoaded(new BlockPos(x, 0, z))) {
            return Integer.MIN_VALUE;
        }
        int y = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z) - 1;
        while (level.getBlockState(new BlockPos(x, y, z)).is(BlockTags.LOGS) && y > level.getMinBuildHeight()) {
            y--;
        }
        return y;
    }

    /**
     * Where the town's next field of {@code crop} goes on its map, if it has a plan: the first open slot of its ring's
     * holdings (the other ring's if its own has no more tracks to open) whose ground can be farmed, in a hex it holds
     * ({@code held}; any if empty) and clear of its people's yards; the ring's next track opened when its holdings are
     * full. Empty while the ground is loading, or for a town with no plan (its fields go where {@link Growth#farmSite}
     * finds).
     */
    public Optional<BlockPos> site(ServerLevel level, long town, BlockPos bell, Item crop, Set<Hex> held) {
        TownPlans.Plan plan = TownPlans.get(level.getServer()).of(town);
        if (plan == null) {
            return Optional.empty();
        }
        TownMap m = maps.computeIfAbsent(town, k -> new TownMap());
        Ring first = ringOf(crop);
        List<BlockPos> people = level.getPoiManager()
                .getInRange(type -> true, bell, Growth.FIELD_FARTHEST + 8, PoiManager.Occupancy.ANY).map(PoiRecord::getPos)
                .toList();
        for (Ring ring : first == Ring.GARDEN ? List.of(Ring.GARDEN, Ring.GRAIN) : List.of(Ring.GRAIN, Ring.GARDEN)) {
            for (int tries = 0; tries < 4; tries++) {
                Optional<BlockPos> found = firstSlot(level, plan, ring, m, bell, held, people);
                if (found.isPresent()) {
                    return found;
                }
                int opened = openTrack(level, town, plan, ring, m);
                if (opened < 0) {
                    return Optional.empty(); // its ground loading: asked again next look
                }
                if (opened == 0) {
                    break; // no more tracks in this ring
                }
            }
        }
        return Optional.empty();
    }

    private Optional<BlockPos> firstSlot(ServerLevel level, TownPlans.Plan plan, Ring ring, TownMap m, BlockPos bell,
            Set<Hex> held, List<BlockPos> people) {
        List<double[][]> paths = tradePaths(plan, ring);
        for (int i = 0; i < m.parcels.size(); i++) {
            Parcel parcel = m.parcels.get(i);
            Track track = m.tracks.get(parcel.track());
            if (track.ring() != ring) {
                continue;
            }
            List<Slot> all = slots(parcel, track.line());
            for (int j = 0; j < all.size(); j++) {
                Slot s = all.get(j);
                if (ring == Ring.GRAIN && LOT.contains(j)) {
                    continue; // the holding's farmstead's
                }
                if (!open(level, plan, ring, m, i, s, paths)) {
                    continue;
                }
                if (!held.isEmpty() && !held.contains(Hex.at(s.x() + WIDE / 2.0, s.z() + WIDE / 2.0))) {
                    continue;
                }
                BlockPos corner = new BlockPos(s.x(), bell.getY(), s.z());
                if (!Growth.zoned(bell, corner, WIDE, people)) {
                    continue;
                }
                // A dressed slot's ground is its dressing's (a pond, hay on it): the height it was laid at.
                Integer dressedY = m.dressed.get(BlockPos.asLong(s.x(), 0, s.z()));
                int y = dressedY != null ? dressedY : ground(level, s.x(), s.z(), bell.getY());
                if (y != Integer.MIN_VALUE) {
                    sited.put(BlockPos.asLong(s.x(), 0, s.z()), s.northSouth());
                    return Optional.of(new BlockPos(s.x(), y, s.z()));
                }
            }
        }
        return Optional.empty();
    }

    /**
     * Opens the ring's next track, nearest the bell first, laid over the land with its road out through the gate if
     * it's the first off it: 1 if opened, 0 if the ring has none left, -1 if its ground is loading (held loaded).
     */
    int openTrack(ServerLevel level, long town, TownPlans.Plan plan, Ring ring, TownMap m) {
        List<double[][]> paths = tradePaths(plan, ring);
        for (Branch b : branches(plan, ring, m)) {
            double[] end = end(plan, ring, b);
            if (end == null) {
                continue;
            }
            // Its land and its holdings' loaded first.
            Set<ChunkPos> chunks = new LinkedHashSet<>();
            int r = OFFSET + LENGTHS * WIDE;
            for (double[] pt : resample(new double[][] {b.from(), end}, 8)) {
                for (int dx = -r; dx <= r; dx += 16) {
                    for (int dz = -r; dz <= r; dz += 16) {
                        chunks.add(new ChunkPos(BlockPos.containing(pt[0] + dx, 0, pt[1] + dz)));
                    }
                }
            }
            if (!hold(level, chunks)) {
                return -1;
            }
            double[][] line = Trails.between(level, b.from(), end, false, 1.2, 16);
            if (ring == Ring.GRAIN && b.path() < 4 && m.roads.add(b.path())) {
                Trails.layLine(level, paths.get(b.path()), TownPlans.GATE);
            }
            int laid = Trails.layLine(level, line, TRACK_WIDTH, true);
            int index = m.tracks.size();
            m.tracks.add(new Track(ring, b.path(), line));
            List<Parcel> parcels = parcels(plan, ring, index, line, town);
            m.parcels.addAll(parcels);
            setDirty();
            dev.b28.enterprise.Enterprise.LOGGER.info("Town {} opens a {} track at {},{} ({} blocks, {} holdings)", town,
                    ring == Ring.GARDEN ? "garden" : "field", (int) b.from()[0], (int) b.from()[1], laid, parcels.size());
            return 1;
        }
        return 0;
    }

    /** Whether all the chunks are loaded; those that aren't are held loaded a while, to be found loaded next look. */
    public static boolean hold(ServerLevel level, java.util.Collection<ChunkPos> chunks) {
        boolean loaded = true;
        for (ChunkPos c : chunks) {
            if (!level.hasChunk(c.x, c.z)) {
                level.getChunkSource().addRegionTicket(SURVEYING, c, 0, c);
                loaded = false;
            }
        }
        return loaded;
    }

    /** The town's first tracks, opened with its plan (John: "some at founding"): {@link #FOUNDING} in each ring. */
    public void found(ServerLevel level, long town) {
        TownPlans.Plan plan = TownPlans.get(level.getServer()).of(town);
        TownMap m = maps.computeIfAbsent(town, k -> new TownMap());
        if (plan == null || m.founded) {
            return;
        }
        for (Ring ring : Ring.values()) {
            long have = m.tracks.stream().filter(t -> t.ring() == ring).count();
            for (long n = have; n < FOUNDING; n++) {
                int opened = openTrack(level, town, plan, ring, m);
                if (opened < 0) {
                    return; // its ground loading: founded on a later look
                }
                if (opened == 0) {
                    break;
                }
            }
        }
        m.founded = true;
        setDirty();
    }

    /**
     * A field on the map planted: the ground under it cleared of trees and plants, its strip's field laid as the map
     * has it (its ditch along the strip), and the slot taken. A field the map didn't site is planted as ever.
     */
    public void till(ServerLevel level, long town, BlockPos corner, Item crop) {
        Boolean northSouth = sited.remove(BlockPos.asLong(corner.getX(), 0, corner.getZ()));
        if (northSouth == null) {
            Fields.plant(level, corner, crop);
            return;
        }
        TownMap map = maps.computeIfAbsent(town, k -> new TownMap());
        boolean wasDressed = map.dressed.remove(BlockPos.asLong(corner.getX(), 0, corner.getZ())) != null;
        for (int dx = -1; dx <= WIDE; dx++) {
            for (int dz = -1; dz <= WIDE; dz++) {
                for (int up = 1; up <= 12; up++) {
                    BlockPos at = corner.offset(dx, up, dz);
                    BlockState state = level.getBlockState(at);
                    boolean inside = dx >= 0 && dz >= 0 && dx < WIDE && dz < WIDE;
                    if (state.is(BlockTags.LOGS) && inside || state.is(BlockTags.LEAVES)
                            || !state.isAir() && state.canBeReplaced() && state.getFluidState().isEmpty()
                            || wasDressed && inside && !state.isAir()) {
                        level.setBlock(at, Blocks.AIR.defaultBlockState(), 3);
                    }
                }
            }
        }
        // The kingdom's field for the crop where it has one (the studio's: a border, a scarecrow, mixed growth), its own
        // water; the open field's ditch where it hasn't.
        String kingdom = Kingdoms.get(level.getServer()).of(town);
        List<FieldTiles.Tile> tiles = FieldTiles.tiles(level, kingdom);
        Optional<FieldTiles.Tile> tile = FieldTiles.field(tiles, crop, corner.asLong());
        if (tile.isPresent()) {
            Growth.level(level, corner, WIDE);
            FieldTiles.lay(level, tile.get(), corner.getX(), corner.getY(), corner.getZ(), northSouth ? 0 : 1);
        } else {
            Fields.plant(level, corner, crop, northSouth);
        }
        balk(level, tiles, corner.getX(), corner.getY(), corner.getZ(), northSouth, town, map);
        map.taken.add(BlockPos.asLong(corner.getX(), 0, corner.getZ()));
        setDirty();
    }

    /**
     * A grain holding's farmstead lot (John: "no sensible farmhouses"): its first two strips' first two fields, by its
     * track (indices into {@link #slots}' order): 19 x 18, the farmhouse facing the track, its fields round it.
     */
    static final Set<Integer> LOT = Set.of(0, 1, 2 * STRIPS + 1, 2 * STRIPS + 2);

    /**
     * Where the town's next farmhouse ({@code width} x {@code depth} as authored) goes: the first grain holding without a
     * farmstead whose lot is on the map, loaded and level enough, its plot (the house's side yards, its front yard toward
     * the track) in the lot's middle, or the lot itself for a farmstead kit its size (19 x 18). Empty if none.
     */
    public Optional<Plots.Site> farmstead(ServerLevel level, long town, int width, int depth) {
        TownPlans.Plan plan = TownPlans.get(level.getServer()).of(town);
        TownMap m = maps.get(town);
        if (plan == null || m == null) {
            return Optional.empty();
        }
        // A farmstead kit takes the lot whole (the studio's: house, barn, yard, garden); a house alone, a plot in it.
        boolean whole = width == 2 * WIDE + BALK && depth == 2 * WIDE;
        int w = whole ? width : Plots.width(width), d = whole ? depth : Math.min(2 * WIDE, depth + TownPlans.FRONT_YARD + 4);
        if (w > 2 * WIDE + BALK) {
            return Optional.empty();
        }
        List<double[][]> paths = tradePaths(plan, Ring.GRAIN);
        for (int i = 0; i < m.parcels.size(); i++) {
            Parcel parcel = m.parcels.get(i);
            Track track = m.tracks.get(parcel.track());
            if (track.ring() != Ring.GRAIN || m.farmsteads.containsKey(i)) {
                continue;
            }
            List<Slot> all = slots(parcel, track.line());
            int x0 = Integer.MAX_VALUE, z0 = Integer.MAX_VALUE, x1 = Integer.MIN_VALUE, z1 = Integer.MIN_VALUE;
            boolean fits = true;
            for (int j : LOT) {
                Slot s = all.get(j);
                if (!level.isLoaded(new BlockPos(s.x(), 0, s.z())) || !level.isLoaded(new BlockPos(s.x() + WIDE, 0, s.z() + WIDE))
                        || !open(level, plan, Ring.GRAIN, m, i, s, paths)) {
                    fits = false;
                    break;
                }
                x0 = Math.min(x0, s.x());
                z0 = Math.min(z0, s.z());
                x1 = Math.max(x1, s.x() + WIDE - 1);
                z1 = Math.max(z1, s.z() + WIDE - 1);
            }
            if (!fits) {
                continue;
            }
            // Its street the track's side: north of fields running south from it, and so on.
            net.minecraft.core.Direction door = parcel.northSouth()
                    ? (parcel.away() > 0 ? net.minecraft.core.Direction.NORTH : net.minecraft.core.Direction.SOUTH)
                    : (parcel.away() > 0 ? net.minecraft.core.Direction.WEST : net.minecraft.core.Direction.EAST);
            int lotX = x1 - x0 + 1, lotZ = z1 - z0 + 1;
            int px, pz;
            switch (door) {
                case NORTH -> { px = x0 + (lotX - w) / 2; pz = z0; }
                case SOUTH -> { px = x0 + (lotX - w) / 2; pz = z1 - d + 1; }
                case WEST -> { px = x0; pz = z0 + (lotZ - w) / 2; }
                default -> { px = x1 - d + 1; pz = z0 + (lotZ - w) / 2; }
            }
            int[] rect = Plots.world(px, pz, w, d, door, 0, 0, w, d);
            // A lot is levelled whole when it's laid (and its edges retained): a little more roll than a house's plot.
            int y = Plots.ground(level, rect[0], rect[1], rect[2], rect[3], LOT_ROLL);
            if (y == Integer.MIN_VALUE || Math.abs(y - plan.bell().getY()) > Plots.RISE) {
                continue;
            }
            pending.put(town, new int[] {i, x0, z0, x1, z1});
            return Optional.of(new Plots.Site(new BlockPos(px, y, pz), door, w, d));
        }
        return Optional.empty();
    }

    /** How far a farmstead lot's ground may rise and fall across it. */
    static final int LOT_ROLL = 6;

    /** The lot last offered each town, till its farmhouse is laid: {holding, x0, z0, x1, z1}. */
    private final Map<Long, int[]> pending = new HashMap<>();

    /** The town's farmhouse laid on the lot it was offered: the lot its farmstead's, from now on counted with the town. */
    public void settle(long town) {
        int[] p = pending.remove(town);
        TownMap m = maps.get(town);
        if (p != null && m != null) {
            m.farmsteads.put(p[0], new int[] {p[1], p[2], p[3], p[4]});
            setDirty();
        }
    }

    /** How many farmsteads the town has. */
    public int farmsteads(long town) {
        TownMap m = maps.get(town);
        return m == null ? 0 : m.farmsteads.size();
    }

    /**
     * The farmstead lots ({x0, z0, x1, z1}) of the town whose bell is at {@code bell}: where its census counts beds and
     * people beyond its reach (John: farmers live on their land, and count as the town's).
     */
    public List<int[]> farmsteadsOf(MinecraftServer server, BlockPos bell) {
        List<int[]> out = new ArrayList<>();
        TownPlans.get(server).plans().forEach((town, p) -> {
            TownMap m = maps.get(town);
            if (m != null && p.bell().getX() == bell.getX() && p.bell().getZ() == bell.getZ()) {
                out.addAll(m.farmsteads.values());
            }
        });
        return out;
    }

    /** Slots a town dresses a look at most: the land's dress laid a little at a time. */
    static final int DRESS_EACH = 6;

    /**
     * Dresses some of the town's unsown slots (John: "farmland that can cover more area and look more irregular"): each
     * open slot of its holdings that the land allows laid with one of its kingdom's crop-free field tiles (fallow, a
     * meadow, an orchard; nothing a census counts), its balk edged with its holding's edge, till a field is planted there.
     * Up to {@link #DRESS_EACH} a call; how many were dressed.
     */
    public int dress(ServerLevel level, long town) {
        TownPlans.Plan plan = TownPlans.get(level.getServer()).of(town);
        TownMap m = maps.get(town);
        if (plan == null || m == null || !m.founded) {
            return 0;
        }
        List<FieldTiles.Tile> tiles = FieldTiles.tiles(level, Kingdoms.get(level.getServer()).of(town));
        if (tiles.isEmpty()) {
            return 0;
        }
        int done = 0;
        for (Ring ring : Ring.values()) {
            List<double[][]> paths = tradePaths(plan, ring);
            for (int i = 0; i < m.parcels.size() && done < DRESS_EACH; i++) {
                Parcel parcel = m.parcels.get(i);
                Track track = m.tracks.get(parcel.track());
                if (track.ring() != ring) {
                    continue;
                }
                List<Slot> all = slots(parcel, track.line());
                for (int j = 0; j < all.size(); j++) {
                    Slot s = all.get(j);
                    if (done >= DRESS_EACH) {
                        break;
                    }
                    if (ring == Ring.GRAIN && LOT.contains(j)) {
                        continue; // the holding's farmstead's
                    }
                    long key = BlockPos.asLong(s.x(), 0, s.z());
                    if (m.dressed.containsKey(key) || m.undressable.contains(key) || m.taken.contains(key)
                            || sited.containsKey(key)) {
                        continue;
                    }
                    if (!level.isLoaded(new BlockPos(s.x(), 0, s.z())) || !level.isLoaded(new BlockPos(s.x() + WIDE, 0, s.z() + WIDE))) {
                        continue; // looked at when it's loaded
                    }
                    int y = open(level, plan, ring, m, i, s, paths) ? ground(level, s.x(), s.z(), plan.bell().getY())
                            : Integer.MIN_VALUE;
                    Optional<FieldTiles.Tile> tile = FieldTiles.dressing(tiles, key ^ town);
                    if (y == Integer.MIN_VALUE || tile.isEmpty()) {
                        m.undressable.add(key);
                        setDirty();
                        continue;
                    }
                    BlockPos corner = new BlockPos(s.x(), y, s.z());
                    for (int dx = 0; dx < WIDE; dx++) {
                        for (int dz = 0; dz < WIDE; dz++) {
                            for (int up = 1; up <= 12; up++) {
                                BlockPos at = corner.offset(dx, up, dz);
                                BlockState state = level.getBlockState(at);
                                if (state.is(BlockTags.LOGS) || state.is(BlockTags.LEAVES)
                                        || !state.isAir() && state.canBeReplaced() && state.getFluidState().isEmpty()) {
                                    level.setBlock(at, Blocks.AIR.defaultBlockState(), 2);
                                }
                            }
                        }
                    }
                    Growth.level(level, corner, WIDE);
                    FieldTiles.lay(level, tile.get(), s.x(), y, s.z(), s.northSouth() ? 0 : 1);
                    balk(level, tiles, s.x(), y, s.z(), s.northSouth(), town, m);
                    m.dressed.put(key, y);
                    setDirty();
                    done++;
                }
            }
        }
        return done;
    }

    /**
     * The balk on a slot's far side across its strip edged with its holding's edge (one kind a holding, so a farm reads
     * as one): north-south strips' balk east of the slot, east-west strips' south of it.
     */
    private void balk(ServerLevel level, List<FieldTiles.Tile> tiles, int x, int y, int z, boolean northSouth, long town,
            TownMap m) {
        int parcel = -1;
        for (int i = 0; i < m.parcels.size(); i++) {
            Parcel p = m.parcels.get(i);
            if (parcel < 0 || Math.hypot(p.x() - x, p.z() - z) < Math.hypot(m.parcels.get(parcel).x() - x,
                    m.parcels.get(parcel).z() - z)) {
                parcel = i;
            }
        }
        Optional<FieldTiles.Tile> edge = FieldTiles.edge(tiles, town * 131 + parcel);
        if (edge.isEmpty()) {
            return;
        }
        int ex = northSouth ? x + WIDE : x, ez = northSouth ? z : z + WIDE;
        for (Track t : m.tracks) {
            if (from(t.line(), ex + 0.5, ez + 0.5) < TRACK_WIDTH / 2.0 + 1
                    || from(t.line(), ex + (northSouth ? 0.5 : WIDE - 0.5), ez + (northSouth ? WIDE - 0.5 : 0.5)) < TRACK_WIDTH / 2.0 + 1) {
                return; // a track runs there
            }
        }
        FieldTiles.lay(level, edge.get(), ex, y, ez, northSouth ? 1 : 0);
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        CompoundTag towns = new CompoundTag();
        maps.forEach((town, m) -> {
            CompoundTag e = new CompoundTag();
            ListTag tracks = new ListTag();
            for (Track t : m.tracks) {
                CompoundTag tt = new CompoundTag();
                tt.putString("ring", t.ring().name());
                tt.putInt("path", t.path());
                int[] line = new int[2 * t.line().length];
                for (int i = 0; i < t.line().length; i++) {
                    line[2 * i] = (int) Math.round(t.line()[i][0] * 4);
                    line[2 * i + 1] = (int) Math.round(t.line()[i][1] * 4);
                }
                tt.putIntArray("line", line);
                tracks.add(tt);
            }
            e.put("tracks", tracks);
            int[] parcels = new int[5 * m.parcels.size()];
            for (int i = 0; i < m.parcels.size(); i++) {
                Parcel p = m.parcels.get(i);
                System.arraycopy(new int[] {p.x(), p.z(), p.track(), p.northSouth() ? 1 : 0, p.away()}, 0, parcels, 5 * i, 5);
            }
            e.putIntArray("parcels", parcels);
            e.putLongArray("taken", m.taken.stream().mapToLong(Long::longValue).sorted().toArray());
            long[] dressedAt = m.dressed.keySet().stream().mapToLong(Long::longValue).sorted().toArray();
            e.putLongArray("dressed", dressedAt);
            e.putIntArray("dressedY", java.util.Arrays.stream(dressedAt).mapToInt(k -> m.dressed.get(k)).toArray());
            e.putLongArray("undressable", m.undressable.stream().mapToLong(Long::longValue).sorted().toArray());
            e.putIntArray("roads", m.roads.stream().mapToInt(Integer::intValue).sorted().toArray());
            e.putBoolean("founded", m.founded);
            int[] lots = new int[5 * m.farmsteads.size()];
            int at = 0;
            for (var f : m.farmsteads.entrySet()) {
                System.arraycopy(new int[] {f.getKey(), f.getValue()[0], f.getValue()[1], f.getValue()[2], f.getValue()[3]}, 0,
                        lots, at, 5);
                at += 5;
            }
            e.putIntArray("farmsteads", lots);
            towns.put(Long.toString(town), e);
        });
        tag.put("towns", towns);
        return tag;
    }

    public static FieldMaps load(CompoundTag tag, HolderLookup.Provider registries) {
        FieldMaps out = new FieldMaps();
        CompoundTag towns = tag.getCompound("towns");
        for (String key : towns.getAllKeys()) {
            CompoundTag e = towns.getCompound(key);
            TownMap m = new TownMap();
            for (Tag t : e.getList("tracks", Tag.TAG_COMPOUND)) {
                CompoundTag tt = (CompoundTag) t;
                int[] line = tt.getIntArray("line");
                double[][] pts = new double[line.length / 2][];
                for (int i = 0; i < pts.length; i++) {
                    pts[i] = new double[] {line[2 * i] / 4.0, line[2 * i + 1] / 4.0};
                }
                m.tracks.add(new Track(Ring.valueOf(tt.getString("ring")), tt.getInt("path"), pts));
            }
            int[] parcels = e.getIntArray("parcels");
            for (int i = 0; i + 4 < parcels.length; i += 5) {
                m.parcels.add(new Parcel(parcels[i], parcels[i + 1], parcels[i + 2], parcels[i + 3] == 1, parcels[i + 4]));
            }
            for (long t : e.getLongArray("taken")) {
                m.taken.add(t);
            }
            long[] dressedAt = e.getLongArray("dressed");
            int[] dressedY = e.getIntArray("dressedY");
            for (int i = 0; i < dressedAt.length && i < dressedY.length; i++) {
                m.dressed.put(dressedAt[i], dressedY[i]);
            }
            for (long t : e.getLongArray("undressable")) {
                m.undressable.add(t);
            }
            for (int r : e.getIntArray("roads")) {
                m.roads.add(r);
            }
            m.founded = e.getBoolean("founded");
            int[] lots = e.getIntArray("farmsteads");
            for (int i = 0; i + 4 < lots.length; i += 5) {
                m.farmsteads.put(lots[i], new int[] {lots[i + 1], lots[i + 2], lots[i + 3], lots[i + 4]});
            }
            out.maps.put(Long.parseLong(key), m);
        }
        return out;
    }
}
