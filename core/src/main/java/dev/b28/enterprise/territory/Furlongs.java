package dev.b28.enterprise.territory;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.BlockTags;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.CropBlock;
import net.minecraft.world.level.block.LeavesBlock;
import net.minecraft.world.level.block.state.BlockState;

/**
 * A town's farmland: the rings' land in fields that fill it, as a painted countryside has it (John's reference: big
 * fields of one crop in rows, hedgerows with trees round each, orchards in blocks; "polygons rather than squares").
 * Every block of a ring belongs to the ring's holding whose seed is nearest (each holding a Voronoi cell, its edges
 * straight lines at odd angles where holdings meet), and a holding is cut by straight lines of its own into fields about
 * {@link #WIDE} by {@link #LONG}, each its piece of the cell, clipped by the ring, the tracks and roads, the suburbs and the
 * farmsteads' lots.
 *
 * <p>Every field is sown (John: "sow everything, count it": the census counts every crop) with one crop in rows down the
 * holding's way, a water channel every ninth row (each crop within 4 of it, the game's reach): what its land suits, as
 * crops grow in the world. Wheat and potatoes take hills on contour terraces (cereals were grown up hillsides in strips,
 * potatoes are an upland crop); carrots and beetroot want deep level soil and cane stands in water, so they keep to the
 * flat; the steepest land is orchard or pasture. A hedgerow (with a tree now and then) runs round each field, open where
 * the field meets its track so its farmer can get in; a pasture is fenced. When the town plants a crop it re-sows the
 * nearest field that suits it.
 */
public final class Furlongs {
    private Furlongs() {}

    /** A field's width across the holding's way and length along it; the hedgerow between two. */
    static final int WIDE = 25, LONG = 41, HEDGE = 1;
    static final int PITCH = WIDE + HEDGE, RUN = LONG + HEDGE;
    /** A holding's reach from its seed, to look for its cell in. */
    static final int REACH = 2 * FieldMaps.HOLDING;
    /** The fewest blocks a field is worth: less, and it's the hedge's. */
    static final int LEAST = 40;
    /** The water channels' spacing across a field: each crop within 4 of one. */
    static final int CHANNEL = 9;

    /** A field: its holding, its place in the holding's grid, its bounds, what's on it, its relief and its blocks. */
    static final class Field {
        final int holding, k, n, x0, z0, x1, z1;
        String state;
        int relief;
        int area;
        /** On the high ground (walls, not hedges); its crop bands' height. */
        boolean high;
        int band = 3;

        Field(int holding, int k, int n, int x0, int z0, int x1, int z1, String state, int relief) {
            this.holding = holding;
            this.k = k;
            this.n = n;
            this.x0 = x0;
            this.z0 = z0;
            this.x1 = x1;
            this.z1 = z1;
            this.state = state;
            this.relief = relief;
        }

        boolean sown() {
            return state.startsWith("crop:");
        }
    }

    /** A holding's grid's offsets (its own, so neighbours' lines don't meet). */
    static int[] offsets(long town, int holding) {
        long h = Zoning.mix(town, holding * 7919L + 13);
        return new int[] {(int) Math.floorMod(h, (long) PITCH), (int) Math.floorMod(h >>> 20, (long) RUN)};
    }

    /** Not a holding's: off the rings, a lot, the suburbs. */
    static final int NONE = -1;
    /** Not a holding's: a track or a road (a field's open side, its way in). */
    static final int ROAD = -2;

    /** Whose land the block is: the holding, {@link #ROAD} or {@link #NONE}. */
    static int owner(TownPlans.Plan plan, FieldMaps.TownMap m, List<double[][]>[] paths, Map<Integer, int[]> lots, int x, int z) {
        double cx = x + 0.5, cz = z + 0.5;
        FieldMaps.Ring ring = FieldMaps.in(plan, FieldMaps.Ring.GARDEN, cx, cz) ? FieldMaps.Ring.GARDEN
                : FieldMaps.in(plan, FieldMaps.Ring.GRAIN, cx, cz) ? FieldMaps.Ring.GRAIN : null;
        if (ring == null) {
            return NONE;
        }
        for (FieldMaps.Track t : m.tracks) {
            if (FieldMaps.from(t.line(), cx, cz) < FieldMaps.TRACK_WIDTH / 2.0 + 1) {
                return ROAD;
            }
        }
        for (double[][] path : paths[ring.ordinal()]) {
            if (FieldMaps.from(path, cx, cz) < TownPlans.GATE / 2.0 + 2) {
                return ROAD;
            }
        }
        // The garden ring's suburbs along the avenues are the town's plots, not fields.
        if (ring == FieldMaps.Ring.GARDEN) {
            int band = TownPlans.GATE / 2 + 2 + plan.deepest() + 1;
            if (Math.abs(x - plan.bell().getX()) <= band || Math.abs(z - plan.bell().getZ()) <= band) {
                return NONE;
            }
        }
        for (int[] lot : lots.values()) {
            if (x >= lot[0] - 1 && x <= lot[2] + 1 && z >= lot[1] - 1 && z <= lot[3] + 1) {
                return NONE;
            }
        }
        int best = NONE;
        double bestD = Double.MAX_VALUE;
        for (int i = 0; i < m.parcels.size(); i++) {
            FieldMaps.Parcel p = m.parcels.get(i);
            if (m.tracks.get(p.track()).ring() != ring) {
                continue;
            }
            double d = Math.hypot(cx - p.x() - 0.5, cz - p.z() - 0.5);
            if (d < bestD) {
                bestD = d;
                best = i;
            }
        }
        return best;
    }

    /** The holding's field a block falls in ({k, n, across, along}: its place in the field), or null on a hedge line. */
    static int[] place(FieldMaps.Parcel p, int[] off, int x, int z) {
        int across = p.northSouth() ? x : z, along = p.northSouth() ? z : x;
        int a = Math.floorMod(across - off[0], PITCH), b = Math.floorMod(along - off[1], RUN);
        if (a >= WIDE || b >= LONG) {
            return null;
        }
        return new int[] {Math.floorDiv(across - off[0], PITCH), Math.floorDiv(along - off[1], RUN), a, b};
    }

    /** A field's blocks: those in its bounds its holding owns, in its place of the grid. */
    static List<int[]> cells(TownPlans.Plan plan, FieldMaps.TownMap m, List<double[][]>[] paths, Map<Integer, int[]> lots, long town,
            Field f) {
        FieldMaps.Parcel p = m.parcels.get(f.holding);
        int[] off = offsets(town, f.holding);
        List<int[]> out = new ArrayList<>();
        for (int x = f.x0; x <= f.x1; x++) {
            for (int z = f.z0; z <= f.z1; z++) {
                int[] kn = place(p, off, x, z);
                if (kn != null && kn[0] == f.k && kn[1] == f.n && owner(plan, m, paths, lots, x, z) == f.holding) {
                    out.add(new int[] {x, z});
                }
            }
        }
        return out;
    }

    @SuppressWarnings("unchecked")
    static List<double[][]>[] paths(TownPlans.Plan plan) {
        return new List[] {FieldMaps.tradePaths(plan, FieldMaps.Ring.GARDEN), FieldMaps.tradePaths(plan, FieldMaps.Ring.GRAIN)};
    }

    /**
     * Lays out and sows a holding's fields: its cell found block by block, cut into its fields, each sown with what its
     * land suits (or an orchard or a pasture), its hedgerows planted. False if its ground isn't loaded yet.
     */
    static boolean dress(ServerLevel level, TownPlans.Plan plan, FieldMaps.TownMap m, Map<Integer, int[]> lots, long town,
            int holding) {
        FieldMaps.Parcel p = m.parcels.get(holding);
        for (int[] c : new int[][] {{-1, -1}, {1, 1}, {-1, 1}, {1, -1}}) {
            if (!level.isLoaded(new BlockPos(p.x() + c[0] * REACH, 0, p.z() + c[1] * REACH))) {
                return false;
            }
        }
        List<double[][]>[] paths = paths(plan);
        int[] off = offsets(town, holding);
        FieldMaps.Ring ring = m.tracks.get(p.track()).ring();
        // Who owns each block round the seed (a block's margin over, for the hedges), found once.
        int x0 = p.x() - REACH - 1, z0 = p.z() - REACH - 1, size = 2 * REACH + 3;
        int[][] own = new int[size][size];
        for (int i = 0; i < size; i++) {
            for (int j = 0; j < size; j++) {
                own[i][j] = owner(plan, m, paths, lots, x0 + i, z0 + j);
            }
        }
        // The holding's blocks by field ({x, z, across, along}); its hedge lines.
        Map<Long, List<int[]>> fields = new java.util.LinkedHashMap<>();
        Map<Long, Long> fieldOf = new HashMap<>();
        Set<Long> hedge = new HashSet<>();
        for (int i = 1; i < size - 1; i++) {
            for (int j = 1; j < size - 1; j++) {
                if (own[i][j] != holding) {
                    continue;
                }
                int x = x0 + i, z = z0 + j;
                int[] kn = place(p, off, x, z);
                if (kn == null) {
                    hedge.add(BlockPos.asLong(x, 0, z));
                    continue;
                }
                long key = ((long) kn[0] << 32) ^ (kn[1] & 0xffffffffL);
                fields.computeIfAbsent(key, q -> new ArrayList<>()).add(new int[] {x, z, kn[2], kn[3]});
                fieldOf.put(BlockPos.asLong(x, 0, z), key);
            }
        }
        Set<Long> fenced = new HashSet<>();
        for (var e : fields.entrySet()) {
            List<int[]> cells = e.getValue();
            if (cells.size() < LEAST) {
                cells.forEach(c -> hedge.add(BlockPos.asLong(c[0], 0, c[1]))); // too small a piece to farm: the hedge's
                continue;
            }
            int bx0 = Integer.MAX_VALUE, bz0 = Integer.MAX_VALUE, bx1 = Integer.MIN_VALUE, bz1 = Integer.MIN_VALUE;
            int lo = Integer.MAX_VALUE, hi = Integer.MIN_VALUE;
            boolean wet = false;
            for (int[] c : cells) {
                bx0 = Math.min(bx0, c[0]);
                bz0 = Math.min(bz0, c[1]);
                bx1 = Math.max(bx1, c[0]);
                bz1 = Math.max(bz1, c[1]);
                int y = Earthworks.height(level, c[0], c[1]);
                if (y != Integer.MIN_VALUE) {
                    lo = Math.min(lo, y);
                    hi = Math.max(hi, y);
                    wet |= !level.getBlockState(new BlockPos(c[0], y, c[1])).getFluidState().isEmpty();
                }
            }
            int relief = hi >= lo ? hi - lo : 0;
            long h = Zoning.mix(town * 31 + holding, e.getKey());
            Field f = new Field(holding, (int) (e.getKey() >> 32), e.getKey().intValue(), bx0, bz0, bx1, bz1,
                    use(ring, relief, wet, h), relief);
            f.area = cells.size();
            m.fields.add(f);
            // Its edge: where it meets another field, another holding, or the end of the land, a hedgerow (a pasture's a
            // fence); open where it meets a track or road, its way in.
            Set<Long> edge = new HashSet<>();
            for (int[] c : cells) {
                int i = c[0] - x0, j = c[1] - z0;
                for (int[] q : new int[][] {{i + 1, j}, {i - 1, j}, {i, j + 1}, {i, j - 1}}) {
                    int o = own[q[0]][q[1]];
                    Long theirs = fieldOf.get(BlockPos.asLong(x0 + q[0], 0, z0 + q[1]));
                    if (o == NONE || o >= 0 && o != holding && holding < o
                            || o == holding && theirs != null && !theirs.equals(e.getKey()) && e.getKey() < theirs) {
                        edge.add(BlockPos.asLong(c[0], 0, c[1]));
                        break;
                    }
                }
            }
            if (f.state.equals("pasture")) {
                fenced.addAll(edge);
            } else {
                hedge.addAll(edge);
            }
            List<int[]> inside = cells.stream().filter(c -> !edge.contains(BlockPos.asLong(c[0], 0, c[1]))).toList();
            switch (f.state) {
                case "orchard" -> orchard(level, inside, h);
                case "pasture" -> pasture(level, inside);
                default -> sow(level, f, inside, crop(f.state));
            }
        }
        for (long k : hedge) {
            hedgerow(level, BlockPos.getX(k), BlockPos.getZ(k), Zoning.mix(BlockPos.getX(k) * 31L, BlockPos.getZ(k)), false);
        }
        for (long k : fenced) {
            hedgerow(level, BlockPos.getX(k), BlockPos.getZ(k), 1, true);
        }
        return true;
    }

    /**
     * What a field is used for, by its land: the steepest orchard or pasture; a hill's field wheat or potatoes (now and
     * then an orchard); the flat any crop, the gardens between the walls most often vegetables, a wet field cane.
     */
    static String use(FieldMaps.Ring ring, int relief, boolean wet, long h) {
        int r = (int) Math.floorMod(h, 12L);
        if (relief > HILL) {
            return r < 7 ? "orchard" : "pasture";
        }
        if (relief > FLAT) {
            return r < 6 ? "crop:wheat" : r < 10 ? "crop:potato" : "orchard";
        }
        if (wet && r < 6) {
            return "crop:sugar_cane";
        }
        if (ring == FieldMaps.Ring.GARDEN) {
            return switch (r) {
                case 0, 1, 2 -> "crop:carrot";
                case 3, 4, 5 -> "crop:potato";
                case 6, 7, 8 -> "crop:beetroot";
                case 9 -> "crop:sweet_berries";
                default -> "crop:wheat";
            };
        }
        return r < 6 ? "crop:wheat" : r < 8 ? "crop:potato" : r < 10 ? "crop:carrot" : r < 11 ? "crop:beetroot" : "orchard";
    }

    static Item crop(String state) {
        return switch (state.substring(state.indexOf(':') + 1)) {
            case "carrot" -> Items.CARROT;
            case "potato" -> Items.POTATO;
            case "beetroot" -> Items.BEETROOT;
            case "sugar_cane" -> Items.SUGAR_CANE;
            case "sweet_berries" -> Items.SWEET_BERRIES;
            default -> Items.WHEAT;
        };
    }

    /** A hedgerow's block on the land's own ground (a tree in it now and then), or a fence round a pasture. */
    static void hedgerow(ServerLevel level, int x, int z, long h, boolean fence) {
        int y = Earthworks.height(level, x, z);
        if (y == Integer.MIN_VALUE || !level.getBlockState(new BlockPos(x, y, z)).getFluidState().isEmpty()) {
            return;
        }
        fell(level, x, y, z);
        BlockPos ground = new BlockPos(x, y, z), over = ground.above();
        if (dev.b28.enterprise.settlement.Grounds.natural(level.getBlockState(ground))) {
            level.setBlock(ground, Blocks.GRASS_BLOCK.defaultBlockState(), 2);
        }
        if (fence) {
            level.setBlock(over, Blocks.SPRUCE_FENCE.defaultBlockState(), 3);
            return;
        }
        if (Math.floorMod(h, 13L) == 0) {
            tree(level, over, h);
        } else {
            level.setBlock(over, hedge(), 2);
            if (Math.floorMod(h, 3L) == 0) {
                level.setBlock(over.above(), hedge(), 2);
            }
        }
    }

    private static BlockState hedge() {
        return hedge(0);
    }

    /**
     * A hedge's leaves: azalea, which no biome tints (oak leaves take the savanna's olive and read as scrub: the studio,
     * r60), now and then in flower.
     */
    static BlockState hedge(long h) {
        return (Math.floorMod(h >>> 5, 10L) == 0 ? Blocks.FLOWERING_AZALEA_LEAVES : Blocks.AZALEA_LEAVES).defaultBlockState()
                .setValue(LeavesBlock.PERSISTENT, true);
    }

    /** A small tree standing at {@code at}: oak or birch by the hash. */
    static void tree(ServerLevel level, BlockPos at, long h) {
        // Lordaeron's trees: oak mostly, birch now and then (the studio's layout: 85 to 15).
        boolean oak = Math.floorMod(h >>> 8, 100L) < 85;
        BlockState log = oak ? Blocks.OAK_LOG.defaultBlockState() : Blocks.BIRCH_LOG.defaultBlockState();
        // An oak's crown in azalea leaves (untinted green: the biome's olive would read as savanna); a birch's its own.
        BlockState leaf = oak ? hedge(h ^ 0x5bd1e995L) : Blocks.BIRCH_LEAVES.defaultBlockState().setValue(LeavesBlock.PERSISTENT, true);
        int trunk = 3 + (int) Math.floorMod(h >>> 12, 2L);
        for (int up = 0; up < trunk; up++) {
            level.setBlock(at.above(up), log, 2);
        }
        BlockPos crown = at.above(trunk);
        for (int dx = -2; dx <= 2; dx++) {
            for (int dz = -2; dz <= 2; dz++) {
                for (int dy = -1; dy <= 1; dy++) {
                    if (Math.abs(dx) + Math.abs(dz) + Math.max(0, dy) * 2 > 3 || dx == 0 && dz == 0 && dy < 1) {
                        continue;
                    }
                    BlockPos q = crown.offset(dx, dy, dz);
                    if (level.getBlockState(q).isAir()) {
                        level.setBlock(q, leaf, 2);
                    }
                }
            }
        }
        level.setBlock(crown, leaf, 2);
    }

    /** An orchard: grass, a tree every five blocks in rows. */
    static void orchard(ServerLevel level, List<int[]> cells, long h) {
        for (int[] c : cells) {
            BlockPos ground = grass(level, c[0], c[1]);
            if (ground == null) {
                continue;
            }
            if (c[2] % 5 == 2 && c[3] % 5 == 2) {
                tree(level, ground.above(), h ^ c[2] * 131L ^ c[3]);
            } else if (Math.floorMod(Zoning.mix(c[0], c[1]), 4L) == 0) {
                level.setBlock(ground.above(), Blocks.SHORT_GRASS.defaultBlockState(), 2);
            }
        }
    }

    /** A pasture: grass (its edge fenced). */
    static void pasture(ServerLevel level, List<int[]> cells) {
        for (int[] c : cells) {
            BlockPos ground = grass(level, c[0], c[1]);
            if (ground != null && Math.floorMod(Zoning.mix(c[0], c[1]), 3L) == 0) {
                level.setBlock(ground.above(), Blocks.SHORT_GRASS.defaultBlockState(), 2);
            }
        }
    }

    /** The land's own ground at a block grassed and cleared over; null if it's water or not loaded. */
    private static BlockPos grass(ServerLevel level, int x, int z) {
        int y = Earthworks.height(level, x, z);
        if (y == Integer.MIN_VALUE || !level.getBlockState(new BlockPos(x, y, z)).getFluidState().isEmpty()) {
            return null;
        }
        fell(level, x, y, z);
        BlockPos ground = new BlockPos(x, y, z);
        if (dev.b28.enterprise.settlement.Grounds.natural(level.getBlockState(ground))) {
            level.setBlock(ground, Blocks.GRASS_BLOCK.defaultBlockState(), 2);
        }
        return ground;
    }

    /** The trees and plants over a block cleared. */
    static void fell(ServerLevel level, int x, int y, int z) {
        for (int up = 1; up <= 14; up++) {
            BlockPos at = new BlockPos(x, y + up, z);
            BlockState s = level.getBlockState(at);
            if (s.is(BlockTags.LOGS) || s.is(BlockTags.LEAVES) && !s.getValue(LeavesBlock.PERSISTENT)
                    || !s.isAir() && s.canBeReplaced() && s.getFluidState().isEmpty()) {
                level.setBlock(at, Blocks.AIR.defaultBlockState(), 2);
            }
        }
    }

    /** The most a flat crop's field may rise and fall; a hill crop's. */
    static final int FLAT = 2, HILL = 8;

    /**
     * Does the crop grow on a hillside, as it does in the world? Wheat (and the cereals) were grown up hillsides in strips
     * and on terraces; potatoes are an upland crop, grown on ridges; berry bushes take a slope as vines and orchards do.
     * Carrots and beetroot want deep, loose, level soil, and cane stands in water: the flat.
     */
    static boolean onHills(Item crop) {
        return crop == Items.WHEAT || crop == Items.POTATO || crop == Items.SWEET_BERRIES;
    }

    /**
     * The field a crop the town plants goes in: one of its ring's fields that suits the crop and isn't sown with it,
     * nearest the bell (a flat crop on the flat; a hill crop on a hill's field first, leaving the flat to those that need
     * it), in the town's land ({@code held}, any if empty).
     */
    static Optional<Field> site(TownPlans.Plan plan, FieldMaps.TownMap m, FieldMaps.Ring ring, Set<dev.b28.enterprise.hex.Hex> held,
            Item crop) {
        BlockPos bell = plan.bell();
        boolean hill = onHills(crop);
        String state = "crop:" + FieldTiles.name(crop);
        String first = "arable:" + FieldTiles.name(crop) + ",";
        BlockPos at = bell;
        return m.fields.stream().filter(f -> f.state.startsWith("arable:") && !f.state.startsWith(first))
                .filter(f -> ringOf(plan, f) == ring)
                .filter(f -> hill ? f.relief <= HILL : f.relief <= FLAT)
                .filter(f -> held.isEmpty() || held.contains(dev.b28.enterprise.hex.Hex.at((f.x0 + f.x1) / 2.0, (f.z0 + f.z1) / 2.0)))
                .min(Comparator.comparingInt((Field f) -> hill && f.relief <= FLAT ? 1 : 0)
                        .thenComparingDouble(f -> Math.hypot((f.x0 + f.x1) / 2.0 - bell.getX(), (f.z0 + f.z1) / 2.0 - bell.getZ())));
    }

    /** The ring a field's middle lies in. */
    static FieldMaps.Ring ringOf(TownPlans.Plan plan, Field f) {
        return FieldMaps.in(plan, FieldMaps.Ring.GARDEN, (f.x0 + f.x1) / 2.0, (f.z0 + f.z1) / 2.0) ? FieldMaps.Ring.GARDEN
                : FieldMaps.Ring.GRAIN;
    }

    /** A field re-sown with {@code crop}, as the town planted it; its hedge stands. */
    static void sow(ServerLevel level, TownPlans.Plan plan, FieldMaps.TownMap m, Map<Integer, int[]> lots, long town, Field f, Item crop) {
        List<double[][]>[] paths = paths(plan);
        FieldMaps.Parcel p = m.parcels.get(f.holding);
        int[] off = offsets(town, f.holding);
        List<int[]> cells = new ArrayList<>();
        for (int[] c : cells(plan, m, paths, lots, town, f)) {
            int[] kn = place(p, off, c[0], c[1]);
            int y = Earthworks.height(level, c[0], c[1]);
            BlockState over = y == Integer.MIN_VALUE ? Blocks.AIR.defaultBlockState() : level.getBlockState(new BlockPos(c[0], y + 1, c[1]));
            if (!over.is(BlockTags.LEAVES) && !over.is(BlockTags.FENCES) && !over.is(BlockTags.LOGS)) {
                cells.add(new int[] {c[0], c[1], kn[2], kn[3]});
            }
        }
        sow(level, f, cells, crop);
    }

    /**
     * A field sown with {@code crop} in rows down the holding's way: every ninth row a water channel (each crop within 4
     * of one), the rest farmland and the crop near ripe (a field ripens together, now and then a straggler). A hill crop on
     * a hill's field sown on contour terraces at the land's own height, a channel's water only where nothing lower stands
     * beside it (a farmland a step below still drinks from it); any other levelled at its blocks' median.
     */
    static void sow(ServerLevel level, Field f, List<int[]> cells, Item crop) {
        List<Integer> heights = new ArrayList<>();
        for (int[] c : cells) {
            int y = Earthworks.height(level, c[0], c[1]);
            if (y != Integer.MIN_VALUE) {
                heights.add(y);
            }
        }
        if (heights.isEmpty()) {
            return;
        }
        heights.sort(Integer::compare);
        int median = heights.get(heights.size() / 2);
        boolean terraced = onHills(crop) && f.relief > FLAT;
        int behind = (int) Math.floorMod(Zoning.mix(f.x0, f.z0), 3L);
        for (int[] c : cells) {
            int x = c[0], z = c[1], across = c[2];
            int y0 = Earthworks.height(level, x, z);
            if (y0 == Integer.MIN_VALUE || Trails.kept(level, new BlockPos(x, y0, z))) {
                continue;
            }
            int y = terraced ? y0 : median;
            boolean channel = crop == Items.SUGAR_CANE ? across % 3 == 1 : across % CHANNEL == CHANNEL / 2;
            if (terraced && channel) {
                for (int[] d : new int[][] {{1, 0}, {-1, 0}, {0, 1}, {0, -1}}) {
                    int ny = Earthworks.height(level, x + d[0], z + d[1]);
                    if (ny != Integer.MIN_VALUE && ny < y) {
                        channel = false; // it would run down the hill
                    }
                }
            }
            if (!terraced) {
                Plots.level(level, x, z, x, z, y);
            }
            fell(level, x, y, z);
            BlockPos ground = new BlockPos(x, y, z), over = ground.above();
            level.setBlock(over, Blocks.AIR.defaultBlockState(), 2);
            long h = Zoning.mix(x * 31L, z);
            if (channel) {
                level.setBlock(ground, Blocks.WATER.defaultBlockState(), 2);
            } else if (crop == Items.SWEET_BERRIES) {
                level.setBlock(ground, Blocks.GRASS_BLOCK.defaultBlockState(), 2);
                if (across % 2 == 0) {
                    level.setBlock(over, Blocks.SWEET_BERRY_BUSH.defaultBlockState()
                            .setValue(net.minecraft.world.level.block.SweetBerryBushBlock.AGE, 1 + (int) Math.floorMod(h, 3L)), 2);
                }
            } else if (crop == Items.SUGAR_CANE) {
                level.setBlock(ground, Blocks.GRASS_BLOCK.defaultBlockState(), 2);
                int tall = 1 + (int) Math.floorMod(h, 3L);
                for (int up = 0; up < tall; up++) {
                    level.setBlock(over.above(up), Blocks.SUGAR_CANE.defaultBlockState(), 2);
                }
            } else {
                level.setBlock(ground, Blocks.FARMLAND.defaultBlockState()
                        .setValue(net.minecraft.world.level.block.FarmBlock.MOISTURE, 7), 2);
                CropBlock block = (CropBlock) seedling(crop).getBlock();
                int ripe = block.getMaxAge();
                int stage = Math.max(0, ripe - behind - (Math.floorMod(h, 7L) == 0 ? 2 : 0));
                level.setBlock(over, block.getStateForAge(stage), 2);
            }
        }
        f.state = "crop:" + FieldTiles.name(crop);
    }

    private static BlockState seedling(Item crop) {
        if (crop == Items.CARROT) {
            return Blocks.CARROTS.defaultBlockState();
        } else if (crop == Items.POTATO) {
            return Blocks.POTATOES.defaultBlockState();
        } else if (crop == Items.BEETROOT) {
            return Blocks.BEETROOTS.defaultBlockState();
        }
        return Blocks.WHEAT.defaultBlockState();
    }
}
