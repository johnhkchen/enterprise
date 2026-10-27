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
 * fields and pastures outside the inner wall ({@link #clear}).
 *
 * <p>Saved with the world: each town's plan, and whether its avenues are laid.
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

    /** A town's plan: its bell, its walls, and whether its avenues are laid. */
    public record Plan(BlockPos bell, Rect inner, Rect outer, boolean avenues) {}

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
            plan = new Plan(plan.bell(), plan.inner(), plan.outer(), true);
            plans.put(town, plan);
            setDirty();
            dev.b28.enterprise.Enterprise.LOGGER.info("Town {} lays its avenues to its gates ({} blocks)", town, laid);
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
        return new Plan(bell, inner, outer, false);
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
                    if (avenueNS || avenueEW) {
                        return false;
                    }
                }
            }
        }
        return true;
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
                        new Rect(o[0], o[1], o[2], o[3]), e.getBoolean("avenues")));
            }
        }
        return out;
    }
}
