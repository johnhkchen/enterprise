package dev.b28.enterprise.territory;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.entity.ai.village.poi.PoiRecord;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Each town's plan (core mode; John: "a reserved area for walls ... and also a second ring wall, taking inspiration
 * from ancient Chinese cities"), fixed at its first survey: an inner wall round the village as it stands and an outer
 * one round the city it may grow into, both rectangles square to the compass (John: "town walls shouldn't be perfect
 * circles"), gates where the four avenues from its forum pass through them. The inner wall is the town's wall work;
 * the outer is kept clear for later. Homes, fields and pastures are sited off the walls' strips and the avenues, and
 * fields and pastures outside the inner wall ({@link #clear}). Inside the inner wall, lanes every {@link Plan#ward} blocks
 * parallel to the avenues (John: "lanes"), wards between them, and new homes fronting a lane or an avenue
 * ({@link #frontages}).
 *
 * <p>Saved with the world: each town's plan, and whether its avenues and lanes are laid.
 */
public final class TownPlans extends SavedData {
    private static final String NAME = "enterprise_town_plans";
    /** How far round the bell the village is measured: its beds, workstations and bell. */
    static final int VILLAGE = 64;
    /** The inner wall's margin round the village, and its least reach from the bell each way. */
    static final int MARGIN = 5;
    static final int LEAST = 16;
    /** The outer wall's reach: so many times the inner's, at most {@link #MOST} each way. */
    static final int OUTER = 2;
    static final int MOST = 120;
    /** Ground kept clear each side of a wall's line, and of an avenue's. */
    public static final int STRIP = 3;
    /** A gate's width, and an avenue's. */
    public static final int GATE = 3;
    /**
     * How far apart a new town's lanes are inside its inner wall (John: "mid density ... villagers live in multiple
     * floors"; the studio's houses, thread 0004): wards of 27 x 27 between streets, plots 13 deep back to back (a house
     * 9 deep with its relief and a garden behind, or 13 deep), plots 9, 12 or 15 wide along the street. A lane's width:
     * four, a street a cart and its neighbours' doorsteps share. Each plan keeps its own ({@link Plan#ward}): a town
     * planned with 14-block wards keeps them.
     */
    static final int WARD = 39;
    /** A plot's yards on a ward with room for them (John: "front yard, back yard leeway ... interchangeable doodad
     * regions"): its front yard's depth between the street and the house, its back yard's behind. */
    public static final int FRONT_YARD = 3;
    public static final int BACK_YARD = 4;
    /** The ward of a plan saved before plans kept their own: lanes every 20, wards of 14 x 14. */
    static final int FIRST_WARD = 20;
    public static final int LANE = 4;

    /** A rectangle, its corners inclusive. */
    public record Rect(int x0, int z0, int x1, int z1) {
        boolean contains(int x, int z) {
            return x >= x0 && x <= x1 && z >= z0 && z <= z1;
        }

        /** How far the place is from the rectangle's edge (inside or out). */
        int fromEdge(int x, int z) {
            int dx = x < x0 ? x0 - x : x > x1 ? x - x1 : Math.min(x - x0, x1 - x);
            int dz = z < z0 ? z0 - z : z > z1 ? z - z1 : Math.min(z - z0, z1 - z);
            boolean in = contains(x, z);
            return in ? Math.min(dx, dz) : Math.max(x < x0 || x > x1 ? dx : 0, z < z0 || z > z1 ? dz : 0);
        }
    }

    /** A town's plan: its bell, its walls, and whether its avenues and its lanes are laid. */
    public record Plan(BlockPos bell, Rect inner, Rect outer, boolean avenues, boolean lanes, int ward) {
        public Plan(BlockPos bell, Rect inner, Rect outer, boolean avenues, boolean lanes) {
            this(bell, inner, outer, avenues, lanes, WARD);
        }

        /** The ground between two streets, along and across: the widest house it fronts and twice the deepest (back to back). */
        public int cell() {
            return ward - LANE - 2;
        }

        /** The deepest house a ward holds, two back to back (with a block between where the cell is odd). */
        public int deepest() {
            return cell() / 2;
        }

        /** Does the ward have room for plots with yards (a front yard, the house, a back yard: 16 deep)? */
        public boolean yards() {
            return deepest() >= FRONT_YARD + 9 + BACK_YARD;
        }
    }

    private final Map<Long, Plan> plans = new LinkedHashMap<>();

    public static TownPlans get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(TownPlans::new, TownPlans::load, null), NAME);
    }

    public Map<Long, Plan> plans() {
        return Map.copyOf(plans);
    }

    public Plan of(long town) {
        return plans.get(town);
    }

    /**
     * The town's plan fixed round {@code bell} if it has none (its village measured now), and its avenues laid once their
     * ground is loaded. Returns the plan.
     */
    public Plan ensure(ServerLevel level, long town, BlockPos bell) {
        Plan plan = plans.get(town);
        if (plan == null) {
            plan = measure(level, bell);
            plans.put(town, plan);
            setDirty();
            dev.b28.enterprise.Enterprise.LOGGER.info("Town {} plans its walls: inner {} to {}, outer {} to {}", town,
                    plan.inner().x0() + "," + plan.inner().z0(), plan.inner().x1() + "," + plan.inner().z1(),
                    plan.outer().x0() + "," + plan.outer().z0(), plan.outer().x1() + "," + plan.outer().z1());
        }
        if (!plan.avenues() && loaded(level, plan.outer())) {
            int laid = avenues(level, plan);
            plan = new Plan(plan.bell(), plan.inner(), plan.outer(), true, plan.lanes(), plan.ward());
            plans.put(town, plan);
            setDirty();
            dev.b28.enterprise.Enterprise.LOGGER.info("Town {} lays its avenues to its gates ({} blocks)", town, laid);
        }
        if (!plan.lanes() && loaded(level, plan.inner())) {
            int laid = 0;
            for (double[][] lane : lanes(plan)) {
                laid += Trails.layLine(level, lane, LANE);
            }
            plan = new Plan(plan.bell(), plan.inner(), plan.outer(), plan.avenues(), true, plan.ward());
            plans.put(town, plan);
            setDirty();
            dev.b28.enterprise.Enterprise.LOGGER.info("Town {} lays {} lanes inside its wall ({} blocks)", town,
                    lanes(plan).size(), laid);
        }
        return plan;
    }

    /** The village round the bell (its beds, workstations, bell) and the walls it calls for. */
    static Plan measure(ServerLevel level, BlockPos bell) {
        int x0 = bell.getX() - LEAST, x1 = bell.getX() + LEAST, z0 = bell.getZ() - LEAST, z1 = bell.getZ() + LEAST;
        for (PoiRecord p : level.getPoiManager().getInRange(type -> true, bell, VILLAGE, PoiManager.Occupancy.ANY).toList()) {
            BlockPos at = p.getPos();
            x0 = Math.min(x0, at.getX() - MARGIN);
            x1 = Math.max(x1, at.getX() + MARGIN);
            z0 = Math.min(z0, at.getZ() - MARGIN);
            z1 = Math.max(z1, at.getZ() + MARGIN);
        }
        Rect inner = new Rect(x0, z0, x1, z1);
        int cx = bell.getX(), cz = bell.getZ();
        Rect outer = new Rect(cx - Math.min(MOST, OUTER * (cx - x0)), cz - Math.min(MOST, OUTER * (cz - z0)),
                cx + Math.min(MOST, OUTER * (x1 - cx)), cz + Math.min(MOST, OUTER * (z1 - cz)));
        return new Plan(bell, inner, outer, false, false);
    }

    private static boolean loaded(ServerLevel level, Rect r) {
        for (int x = r.x0(); x <= r.x1() + 15; x += 16) {
            for (int z : new int[] {r.z0(), (r.z0() + r.z1()) / 2, r.z1()}) {
                if (!level.isLoaded(new BlockPos(Math.min(x, r.x1()), 0, z))) {
                    return false;
                }
            }
        }
        return level.isLoaded(new BlockPos(r.x0(), 0, (r.z0() + r.z1()) / 2))
                && level.isLoaded(new BlockPos(r.x1(), 0, (r.z0() + r.z1()) / 2));
    }

    /** The four avenues: from the forum's edge north, south, east and west to the outer wall's line, a street wide. */
    static int avenues(ServerLevel level, Plan plan) {
        BlockPos b = plan.bell();
        double x = b.getX() + 0.5, z = b.getZ() + 0.5, from = Forums.REACH + 1;
        Rect o = plan.outer();
        int laid = 0;
        laid += Trails.layLine(level, new double[][] {{x, z - from}, {x, o.z0() + 0.5}}, GATE);
        laid += Trails.layLine(level, new double[][] {{x, z + from}, {x, o.z1() + 0.5}}, GATE);
        laid += Trails.layLine(level, new double[][] {{x - from, z}, {o.x0() + 0.5, z}}, GATE);
        laid += Trails.layLine(level, new double[][] {{x + from, z}, {o.x1() + 0.5, z}}, GATE);
        return laid;
    }

    /** The ground inside the inner wall the town's lanes and homes may take: off the wall's strip. */
    static Rect within(Plan plan) {
        Rect r = plan.inner();
        int in = STRIP + 1;
        return new Rect(r.x0() + in, r.z0() + in, r.x1() - in, r.z1() - in);
    }

    /** The lanes' lines (block x, z) north-south ({@code true}) or east-west, every {@link Plan#ward} blocks from the bell's. */
    static List<Integer> laneLines(Plan plan, boolean northSouth) {
        Rect w = within(plan);
        int from = northSouth ? plan.bell().getX() : plan.bell().getZ();
        int lo = northSouth ? w.x0() : w.z0(), hi = northSouth ? w.x1() : w.z1();
        List<Integer> out = new ArrayList<>();
        for (int at = from - plan.ward(); at - LANE >= lo; at -= plan.ward()) {
            out.add(at);
        }
        for (int at = from + plan.ward(); at + LANE <= hi; at += plan.ward()) {
            out.add(at);
        }
        return out;
    }

    /** The lanes inside the inner wall, wall strip to wall strip, parallel to the avenues. */
    public static List<double[][]> lanes(Plan plan) {
        Rect w = within(plan);
        List<double[][]> out = new ArrayList<>();
        for (int x : laneLines(plan, true)) {
            out.add(new double[][] {{x + 0.5, w.z0() + 0.5}, {x + 0.5, w.z1() + 0.5}});
        }
        for (int z : laneLines(plan, false)) {
            out.add(new double[][] {{w.x0() + 0.5, z + 0.5}, {w.x1() + 0.5, z + 0.5}});
        }
        return out;
    }

    /** A home's place fronting a lane or an avenue: its corner (x, z) and the way its door faces, onto the street. */
    public record Frontage(int x, int z, net.minecraft.core.Direction door, int sx, int sz) {
        Frontage(int x, int z, net.minecraft.core.Direction door) {
            this(x, z, door, 0, 0);
        }
    }

    /**
     * Where new homes go inside the inner wall: a row each side of every lane and avenue, a home every
     * {@code HOME_SIZE + 1} blocks along it, a doorstep between its door and the street; nearest the bell first.
     */
    public static List<Frontage> frontages(Plan plan, int size) {
        return frontages(plan, size, size);
    }

    /**
     * As {@link #frontages(Plan, int)}, for homes {@code width} along their street and {@code depth} back from it: each
     * frontage's corner and its footprint in the world ({@code sx} x {@code sz}, turned to its street).
     */
    public static List<Frontage> frontages(Plan plan, int width, int depth) {
        return frontages(plan, width, depth, 1);
    }

    /**
     * A town's plots {@code width} along the street, a ward's half deep, side by side: the house on each with its side
     * yards inside its width, its front and back yards inside its depth ({@link Plan#yards}).
     */
    public static List<Frontage> plots(Plan plan, int width) {
        return frontages(plan, width, plan.deepest(), 0);
    }

    /** As {@link #frontages(Plan, int, int)}, {@code gap} blocks between neighbours along the street. */
    public static List<Frontage> frontages(Plan plan, int width, int depth, int gap) {
        Rect w = within(plan);
        int bx = plan.bell().getX(), bz = plan.bell().getZ();
        List<Frontage> out = new ArrayList<>();
        // Each street: its line, the blocks it takes either side of it (to the west or north, and to the east or south).
        List<int[]> ns = new ArrayList<>(), ew = new ArrayList<>();
        ns.add(new int[] {bx, GATE / 2, GATE / 2});
        ew.add(new int[] {bz, GATE / 2, GATE / 2});
        laneLines(plan, true).forEach(x -> ns.add(new int[] {x, LANE / 2, LANE - LANE / 2 - 1}));
        laneLines(plan, false).forEach(z -> ew.add(new int[] {z, LANE / 2, LANE - LANE / 2 - 1}));
        // Along each street, each ward's stretch of it (between the cross streets) filled from its start: a home never
        // straddles a cross street.
        for (int[] s : ns) {
            for (int[] run : runs(w.z0(), w.z1(), ew)) {
                for (int z = run[0]; z + width - 1 <= run[1]; z += width + gap) {
                    out.add(new Frontage(s[0] - s[1] - 2 - depth + 1, z, net.minecraft.core.Direction.EAST, depth, width));
                    out.add(new Frontage(s[0] + s[2] + 2, z, net.minecraft.core.Direction.WEST, depth, width));
                }
            }
        }
        for (int[] s : ew) {
            for (int[] run : runs(w.x0(), w.x1(), ns)) {
                for (int x = run[0]; x + width - 1 <= run[1]; x += width + gap) {
                    out.add(new Frontage(x, s[0] - s[1] - 2 - depth + 1, net.minecraft.core.Direction.SOUTH, width, depth));
                    out.add(new Frontage(x, s[0] + s[2] + 2, net.minecraft.core.Direction.NORTH, width, depth));
                }
            }
        }
        out.removeIf(f -> f.x() < w.x0() || f.z() < w.z0() || f.x() + f.sx() - 1 > w.x1() || f.z() + f.sz() - 1 > w.z1());
        // Off every street (a row along a lane crosses the avenues and the other lanes), and off the forum.
        out.removeIf(f -> {
            for (int dx = 0; dx < f.sx(); dx++) {
                for (int dz = 0; dz < f.sz(); dz++) {
                    int x = f.x() + dx, z = f.z() + dz;
                    if (Math.abs(x - bx) <= GATE / 2 + 1 || Math.abs(z - bz) <= GATE / 2 + 1 || onLane(plan, x, z)) {
                        return true;
                    }
                }
            }
            return false;
        });
        int f0 = Forums.REACH + 1;
        out.removeIf(f -> f.x() <= bx + f0 && f.x() + f.sx() - 1 >= bx - f0 && f.z() <= bz + f0 && f.z() + f.sz() - 1 >= bz - f0);
        out.sort(Comparator.comparingDouble((Frontage f) -> Math.hypot(f.x() + f.sx() / 2.0 - bx, f.z() + f.sz() / 2.0 - bz))
                .thenComparingInt(Frontage::x).thenComparingInt(Frontage::z));
        return out;
    }

    /**
     * The stretches from {@code lo} to {@code hi} between the streets across them (each {line, blocks before, blocks
     * after}), off each street and the block kept either side of it.
     */
    static List<int[]> runs(int lo, int hi, List<int[]> streets) {
        List<int[]> out = new ArrayList<>();
        Integer start = null;
        for (int v = lo; v <= hi + 1; v++) {
            boolean kept = v > hi;
            for (int[] st : streets) {
                kept |= v >= st[0] - st[1] - 1 && v <= st[0] + st[2] + 1;
            }
            if (!kept && start == null) {
                start = v;
            } else if (kept && start != null) {
                out.add(new int[] {start, v - 1});
                start = null;
            }
        }
        return out;
    }

    /** The plan whose bell this is, if any. */
    public static java.util.Optional<Plan> at(ServerLevel level, BlockPos bell) {
        if (level.getServer() == null) {
            return java.util.Optional.empty();
        }
        return get(level.getServer()).plans.values().stream().filter(p -> p.bell().equals(bell)).findFirst();
    }

    /** The way a home with its corner here faces: onto its street if it fronts one in any town's plan, else south. */
    public static net.minecraft.core.Direction doorOf(ServerLevel level, BlockPos corner, int size) {
        return doorOf(level, corner, size, size);
    }

    /** As {@link #doorOf(ServerLevel, BlockPos, int)}, for a home {@code width} along its street, {@code depth} back. */
    public static net.minecraft.core.Direction doorOf(ServerLevel level, BlockPos corner, int width, int depth) {
        if (level.getServer() != null) {
            for (Plan p : get(level.getServer()).plans.values()) {
                if (p.inner().contains(corner.getX(), corner.getZ())) {
                    for (Frontage f : frontages(p, width, depth)) {
                        if (f.x() == corner.getX() && f.z() == corner.getZ()) {
                            return f.door();
                        }
                    }
                }
            }
        }
        return net.minecraft.core.Direction.SOUTH;
    }

    /**
     * The inner wall's columns, in order round from the east: its rectangle's edge, less a gate {@link #GATE} wide where
     * each avenue passes (the bell's line north-south and east-west). Only x and z count.
     */
    public static List<BlockPos> wall(Plan plan) {
        Rect r = plan.inner();
        int bx = plan.bell().getX(), bz = plan.bell().getZ();
        List<BlockPos> columns = new ArrayList<>();
        for (int x = r.x0(); x <= r.x1(); x++) {
            for (int z = r.z0(); z <= r.z1(); z++) {
                boolean edge = x == r.x0() || x == r.x1() || z == r.z0() || z == r.z1();
                boolean gate = (z == r.z0() || z == r.z1()) && Math.abs(x - bx) <= GATE / 2
                        || (x == r.x0() || x == r.x1()) && Math.abs(z - bz) <= GATE / 2;
                if (edge && !gate) {
                    columns.add(new BlockPos(x, 0, z));
                }
            }
        }
        columns.sort(Comparator.comparingDouble(
                c -> (Math.atan2(c.getZ() - bz, c.getX() - bx) + 2 * Math.PI) % (2 * Math.PI)));
        return columns;
    }

    /**
     * May a {@code sx} x {@code sz} footprint with its corner here go on the plans' ground: off every wall's strip
     * ({@link #STRIP} each side of its line) and every avenue's, and, for a field or a pasture ({@code farm}), outside
     * every inner wall?
     */
    public static boolean clear(ServerLevel level, int x0, int z0, int sx, int sz, boolean farm) {
        if (level.getServer() == null) {
            return true;
        }
        for (Plan p : get(level.getServer()).plans.values()) {
            Rect fp = new Rect(x0, z0, x0 + sx - 1, z0 + sz - 1);
            if (!near(fp, p.outer(), STRIP + 8)) {
                continue; // nowhere near this town
            }
            for (int x = fp.x0(); x <= fp.x1(); x++) {
                for (int z = fp.z0(); z <= fp.z1(); z++) {
                    if (p.inner().fromEdge(x, z) <= STRIP || p.outer().fromEdge(x, z) <= STRIP) {
                        return false;
                    }
                    if (farm && p.inner().contains(x, z)) {
                        return false;
                    }
                    boolean avenueNS = Math.abs(x - p.bell().getX()) <= GATE / 2 + STRIP - 2
                            && p.outer().contains(x, z);
                    boolean avenueEW = Math.abs(z - p.bell().getZ()) <= GATE / 2 + STRIP - 2
                            && p.outer().contains(x, z);
                    if (avenueNS || avenueEW || onLane(p, x, z)) {
                        return false;
                    }
                }
            }
        }
        return true;
    }

    /** Is the column on one of the plan's lanes, or the block either side of it? */
    public static boolean onLane(Plan p, int x, int z) {
        if (!p.lanes() || !within(p).contains(x, z)) {
            return false;
        }
        for (int lx : laneLines(p, true)) {
            if (x >= lx - LANE / 2 - 1 && x <= lx + LANE - LANE / 2) {
                return true;
            }
        }
        for (int lz : laneLines(p, false)) {
            if (z >= lz - LANE / 2 - 1 && z <= lz + LANE - LANE / 2) {
                return true;
            }
        }
        return false;
    }

    /** How far beyond its inner wall a town's fields and pastures may go. */
    static final int FARMS_BEYOND = 40;

    /**
     * How far from {@code bell} a field or pasture may go: {@code least}, or past the inner wall of a town planned round
     * that bell by {@link #FARMS_BEYOND}, where they must lie outside it.
     */
    public static int farmReach(ServerLevel level, BlockPos bell, int least) {
        if (level.getServer() == null) {
            return least;
        }
        for (Plan p : get(level.getServer()).plans.values()) {
            if (p.bell().equals(bell)) {
                Rect r = p.inner();
                int far = Math.max(Math.max(bell.getX() - r.x0(), r.x1() - bell.getX()),
                        Math.max(bell.getZ() - r.z0(), r.z1() - bell.getZ()));
                return Math.max(least, far + FARMS_BEYOND);
            }
        }
        return least;
    }

    /** How far beyond the outer wall's line a town's pastures begin: past its grain's nearer half (John: "pastures beyond"). */
    static final int PASTURES_BEYOND = STRIP + 1 + FieldMaps.GRAIN_DEPTH / 2;
    /** How deep the pastures' ring runs past where it begins. */
    static final int PASTURE_DEPTH = 56;

    /**
     * How far from {@code bell} a pasture may go: {@code least}, or for a town planned round that bell, out past its
     * outer wall through its pastures' ring.
     */
    public static int pastureReach(ServerLevel level, BlockPos bell, int least) {
        return at(level, bell).map(p -> {
            Rect r = p.outer();
            int far = Math.max(Math.max(bell.getX() - r.x0(), r.x1() - bell.getX()),
                    Math.max(bell.getZ() - r.z0(), r.z1() - bell.getZ()));
            return Math.max(least, far + PASTURES_BEYOND + PASTURE_DEPTH);
        }).orElse(least);
    }

    /**
     * May a pasture of {@code sx} x {@code sz} with its corner here go on the ground of the town planned round
     * {@code bell}: in its pastures' ring, beyond its grain? (Any ground, for a town with no plan.)
     */
    public static boolean pastureRing(ServerLevel level, BlockPos bell, int x0, int z0, int sx, int sz) {
        return at(level, bell).map(p -> {
            Rect o = p.outer();
            int gapX = Math.max(0, Math.max(o.x0() - (x0 + sx - 1), x0 - o.x1()));
            int gapZ = Math.max(0, Math.max(o.z0() - (z0 + sz - 1), z0 - o.z1()));
            int gap = Math.max(gapX, gapZ);
            return gap > PASTURES_BEYOND && gap <= PASTURES_BEYOND + PASTURE_DEPTH;
        }).orElse(true);
    }

    private static boolean near(Rect a, Rect b, int by) {
        return a.x1() >= b.x0() - by && a.x0() <= b.x1() + by && a.z1() >= b.z0() - by && a.z0() <= b.z1() + by;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        CompoundTag towns = new CompoundTag();
        plans.forEach((town, p) -> {
            CompoundTag e = new CompoundTag();
            e.putLong("bell", p.bell().asLong());
            e.putIntArray("inner", new int[] {p.inner().x0(), p.inner().z0(), p.inner().x1(), p.inner().z1()});
            e.putIntArray("outer", new int[] {p.outer().x0(), p.outer().z0(), p.outer().x1(), p.outer().z1()});
            e.putBoolean("avenues", p.avenues());
            e.putBoolean("lanes", p.lanes());
            e.putInt("ward", p.ward());
            towns.put(Long.toString(town), e);
        });
        tag.put("towns", towns);
        return tag;
    }

    public static TownPlans load(CompoundTag tag, HolderLookup.Provider registries) {
        TownPlans out = new TownPlans();
        CompoundTag towns = tag.getCompound("towns");
        for (String key : towns.getAllKeys()) {
            CompoundTag e = towns.getCompound(key);
            int[] i = e.getIntArray("inner"), o = e.getIntArray("outer");
            if (i.length == 4 && o.length == 4) {
                out.plans.put(Long.parseLong(key), new Plan(BlockPos.of(e.getLong("bell")), new Rect(i[0], i[1], i[2], i[3]),
                        new Rect(o[0], o[1], o[2], o[3]), e.getBoolean("avenues"), e.getBoolean("lanes"),
                        e.contains("ward") ? e.getInt("ward") : FIRST_WARD));
            }
        }
        return out;
    }
}
