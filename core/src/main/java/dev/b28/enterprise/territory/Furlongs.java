package dev.b28.enterprise.territory;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.BlockTags;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;

/**
 * A town's farmland as polygons (John: "Fields should expand and be more irregularly shaped to properly fill space.
 * Straight lines still, but polygons rather than squares"): every block of a ring belongs to the ring's holding whose seed
 * is nearest (each holding a Voronoi cell, its edges straight lines at odd angles where holdings meet), and a holding is
 * cut into fields by straight lines of its own: strips a field wide with a balk between, running its way, each cut into
 * furlongs a few fields long. A field is its strip's furlong's piece of the cell, clipped by the ring and the tracks: a
 * trapezoid, a triangle, a long strip, whatever the land leaves, and together they leave no land unused.
 *
 * <p>Unsown, a field is dressed fallow (ploughed ridges), hay or meadow, nothing a census counts; when the town plants a
 * crop, a field is sown with it, up to the plots it paid for, a water channel down each strip's middle, the rest left
 * fallow. Where two holdings meet, the holding's edge (a hedge, a dry-stone wall, a rail fence) runs along the line.
 */
public final class Furlongs {
    private Furlongs() {}

    /** A strip's width; its balk; a furlong's length along the strip (with its headland). */
    static final int STRIP = 9, BALK = 1, LENGTH = 27;
    static final int PITCH = STRIP + BALK, RUN = LENGTH + BALK;
    /** A holding's reach from its seed, to look for its cell in. */
    static final int REACH = 2 * FieldMaps.HOLDING;
    /** The fewest blocks a field is worth: less, and it's left as grass. */
    static final int LEAST = 24;

    /** A field: its holding, its strip and furlong in the holding's grid, its bounds, and what's on it. */
    static final class Field {
        final int holding, k, n, x0, z0, x1, z1;
        String state;

        Field(int holding, int k, int n, int x0, int z0, int x1, int z1, String state) {
            this.holding = holding;
            this.k = k;
            this.n = n;
            this.x0 = x0;
            this.z0 = z0;
            this.x1 = x1;
            this.z1 = z1;
            this.state = state;
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

    /** Whose field the block is in: the holding, or -1 (none: off the rings, on a track or a road, a farmstead's lot). */
    static int owner(TownPlans.Plan plan, FieldMaps.TownMap m, List<double[][]>[] paths, Map<Integer, int[]> lots, int x, int z) {
        double cx = x + 0.5, cz = z + 0.5;
        FieldMaps.Ring ring = FieldMaps.in(plan, FieldMaps.Ring.GARDEN, cx, cz) ? FieldMaps.Ring.GARDEN
                : FieldMaps.in(plan, FieldMaps.Ring.GRAIN, cx, cz) ? FieldMaps.Ring.GRAIN : null;
        if (ring == null) {
            return -1;
        }
        int best = -1;
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
        if (best < 0) {
            return -1;
        }
        for (FieldMaps.Track t : m.tracks) {
            if (FieldMaps.from(t.line(), cx, cz) < FieldMaps.TRACK_WIDTH / 2.0 + 1) {
                return -1;
            }
        }
        for (double[][] path : paths[ring.ordinal()]) {
            if (FieldMaps.from(path, cx, cz) < TownPlans.GATE / 2.0 + 2) {
                return -1;
            }
        }
        // The garden ring's suburbs along the avenues are the town's plots, not fields.
        if (ring == FieldMaps.Ring.GARDEN) {
            int band = TownPlans.GATE / 2 + 2 + plan.deepest() + 1;
            if (Math.abs(x - plan.bell().getX()) <= band || Math.abs(z - plan.bell().getZ()) <= band) {
                return -1;
            }
        }
        for (int[] lot : lots.values()) {
            if (x >= lot[0] - 1 && x <= lot[2] + 1 && z >= lot[1] - 1 && z <= lot[3] + 1) {
                return -1;
            }
        }
        return best;
    }

    /** The holding's strip and furlong a block falls in ({k, n}), or null on a balk or headland. */
    static int[] place(FieldMaps.Parcel p, int[] off, int x, int z) {
        int across = p.northSouth() ? x : z, along = p.northSouth() ? z : x;
        int a = Math.floorMod(across - off[0], PITCH), b = Math.floorMod(along - off[1], RUN);
        if (a >= STRIP || b >= LENGTH) {
            return null;
        }
        return new int[] {Math.floorDiv(across - off[0], PITCH), Math.floorDiv(along - off[1], RUN)};
    }

    /** A field's blocks: those in its bounds its holding owns, in its strip and furlong. */
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
     * Lays out and dresses a holding's fields: its cell found block by block, cut into its fields, each dressed (fallow,
     * hay, meadow by its hash) on the land's own ground, its trees felled, and its edge run where it meets another
     * holding. False if its ground isn't loaded yet.
     */
    static boolean dress(ServerLevel level, TownPlans.Plan plan, FieldMaps.TownMap m, Map<Integer, int[]> lots, long town,
            int holding) {
        FieldMaps.Parcel p = m.parcels.get(holding);
        if (!level.isLoaded(new BlockPos(p.x() - REACH, 0, p.z() - REACH)) || !level.isLoaded(new BlockPos(p.x() + REACH, 0, p.z() + REACH))
                || !level.isLoaded(new BlockPos(p.x() - REACH, 0, p.z() + REACH)) || !level.isLoaded(new BlockPos(p.x() + REACH, 0, p.z() - REACH))) {
            return false;
        }
        List<double[][]>[] paths = paths(plan);
        int[] off = offsets(town, holding);
        // Who owns each block round the seed (a block's margin over, for the edges), found once.
        int x0 = p.x() - REACH - 1, z0 = p.z() - REACH - 1, size = 2 * REACH + 3;
        int[][] own = new int[size][size];
        for (int i = 0; i < size; i++) {
            for (int j = 0; j < size; j++) {
                own[i][j] = owner(plan, m, paths, lots, x0 + i, z0 + j);
            }
        }
        Map<Long, List<int[]>> fields = new java.util.LinkedHashMap<>();
        BlockState edge = edge(town, holding);
        for (int i = 1; i < size - 1; i++) {
            for (int j = 1; j < size - 1; j++) {
                if (own[i][j] != holding) {
                    continue;
                }
                int x = x0 + i, z = z0 + j;
                // Its edge where it meets another holding (each line once: on the side of the lower holding).
                int[][] near = {{i + 1, j}, {i, j + 1}, {i - 1, j}, {i, j - 1}};
                for (int[] q : near) {
                    int o = own[q[0]][q[1]];
                    if (o >= 0 && o != holding && holding < o) {
                        int y = Earthworks.height(level, x, z);
                        BlockPos at = new BlockPos(x, y + 1, z);
                        if (y != Integer.MIN_VALUE && level.getBlockState(at).canBeReplaced()) {
                            fell(level, x, y, z);
                            level.setBlock(at, edge, 2);
                        }
                        break;
                    }
                }
                int[] kn = place(p, off, x, z);
                if (kn != null) {
                    fields.computeIfAbsent(((long) kn[0] << 32) ^ (kn[1] & 0xffffffffL), q -> new ArrayList<>()).add(new int[] {x, z});
                }
            }
        }
        for (var e : fields.entrySet()) {
            List<int[]> cells = e.getValue();
            if (cells.size() < LEAST) {
                continue;
            }
            int k = (int) (e.getKey() >> 32), n = e.getKey().intValue();
            int bx0 = Integer.MAX_VALUE, bz0 = Integer.MAX_VALUE, bx1 = Integer.MIN_VALUE, bz1 = Integer.MIN_VALUE;
            for (int[] c : cells) {
                bx0 = Math.min(bx0, c[0]);
                bz0 = Math.min(bz0, c[1]);
                bx1 = Math.max(bx1, c[0]);
                bz1 = Math.max(bz1, c[1]);
            }
            Field f = new Field(holding, k, n, bx0, bz0, bx1, bz1, dressing(Zoning.mix(town * 31 + holding, e.getKey())));
            m.fields.add(f);
            lay(level, p, off, f, cells);
        }
        return true;
    }

    /** An unsown field's dress: fallow mostly, hay or meadow now and then. */
    static String dressing(long h) {
        int r = (int) Math.floorMod(h, 10L);
        return r < 5 ? "fallow" : r < 8 ? "hay" : "meadow";
    }

    /** The holding's edge: a hedge, a dry-stone wall or a rail fence, one kind a holding. */
    static BlockState edge(long town, int holding) {
        return switch ((int) Math.floorMod(Zoning.mix(town, holding), 3L)) {
            case 0 -> Blocks.OAK_LEAVES.defaultBlockState().setValue(net.minecraft.world.level.block.LeavesBlock.PERSISTENT, true);
            case 1 -> Blocks.MOSSY_COBBLESTONE_WALL.defaultBlockState();
            default -> Blocks.SPRUCE_FENCE.defaultBlockState();
        };
    }

    /** A field dressed as its state says, block by block on the land's own ground. */
    static void lay(ServerLevel level, FieldMaps.Parcel p, int[] off, Field f, List<int[]> cells) {
        for (int[] c : cells) {
            int x = c[0], z = c[1];
            int y = Earthworks.height(level, x, z);
            if (y == Integer.MIN_VALUE) {
                continue;
            }
            fell(level, x, y, z);
            BlockState top = level.getBlockState(new BlockPos(x, y, z));
            if (!dev.b28.enterprise.settlement.Grounds.natural(top) || !top.getFluidState().isEmpty()) {
                continue; // rock, water, a path: left as it is
            }
            int across = Math.floorMod((p.northSouth() ? x : z) - off[0], PITCH);
            long h = Zoning.mix(x * 31L, z);
            BlockPos ground = new BlockPos(x, y, z), over = ground.above();
            switch (f.state) {
                case "fallow" -> {
                    // Ploughed ridges down the strip, grass coming back in the furrows.
                    level.setBlock(ground, across % 2 == 0 ? Blocks.COARSE_DIRT.defaultBlockState()
                            : Blocks.GRASS_BLOCK.defaultBlockState(), 2);
                    if (across % 2 == 1 && Math.floorMod(h, 3L) == 0) {
                        level.setBlock(over, Blocks.SHORT_GRASS.defaultBlockState(), 2);
                    }
                }
                case "hay" -> {
                    level.setBlock(ground, Blocks.GRASS_BLOCK.defaultBlockState(), 2);
                    long r = Math.floorMod(h, 60L);
                    level.setBlock(over, r == 0 ? Blocks.HAY_BLOCK.defaultBlockState()
                            : r < 30 ? Blocks.SHORT_GRASS.defaultBlockState() : Blocks.AIR.defaultBlockState(), 2);
                }
                default -> {
                    level.setBlock(ground, Blocks.GRASS_BLOCK.defaultBlockState(), 2);
                    long r = Math.floorMod(h, 20L);
                    BlockState flower = r == 0 ? Blocks.CORNFLOWER.defaultBlockState() : r == 1 ? Blocks.OXEYE_DAISY.defaultBlockState()
                            : r == 2 ? Blocks.POPPY.defaultBlockState() : r < 12 ? Blocks.SHORT_GRASS.defaultBlockState()
                            : Blocks.AIR.defaultBlockState();
                    level.setBlock(over, flower, 2);
                }
            }
        }
    }

    /** The trees and plants over a block cleared. */
    static void fell(ServerLevel level, int x, int y, int z) {
        for (int up = 1; up <= 14; up++) {
            BlockPos at = new BlockPos(x, y + up, z);
            BlockState s = level.getBlockState(at);
            if (s.is(BlockTags.LOGS) || s.is(BlockTags.LEAVES) && !s.getValue(net.minecraft.world.level.block.LeavesBlock.PERSISTENT)
                    || !s.isAir() && s.canBeReplaced() && s.getFluidState().isEmpty()) {
                level.setBlock(at, Blocks.AIR.defaultBlockState(), 2);
            }
        }
    }

    /**
     * The unsown field a crop goes in: in the crop's ring first, nearest the bell, in the town's land ({@code held}, any
     * if empty). Its corner (its bounds' north-west, at the bell's height) names it.
     */
    static Optional<Field> site(TownPlans.Plan plan, FieldMaps.TownMap m, FieldMaps.Ring ring, java.util.Set<dev.b28.enterprise.hex.Hex> held) {
        BlockPos bell = plan.bell();
        return m.fields.stream().filter(f -> !f.sown())
                .filter(f -> m.tracks.get(m.parcels.get(f.holding).track()).ring() == ring)
                .filter(f -> held.isEmpty() || held.contains(dev.b28.enterprise.hex.Hex.at((f.x0 + f.x1) / 2.0, (f.z0 + f.z1) / 2.0)))
                .min(Comparator.comparingDouble(f -> Math.hypot((f.x0 + f.x1) / 2.0 - bell.getX(), (f.z0 + f.z1) / 2.0 - bell.getZ())));
    }

    /** The plots a field is sown with: what the town paid for, a field's worth. */
    static final int PLOTS = Fields.SIZE * Fields.SIZE - Fields.SIZE;

    /**
     * A field sown with {@code crop}: its blocks nearest the holding's track first, up to {@link #PLOTS} of the crop, each
     * strip's middle a water channel (every crop within 4 of it: the game's reach), the ground levelled along the channel's
     * height so the water stands; the rest left as it was.
     */
    static void sow(ServerLevel level, TownPlans.Plan plan, FieldMaps.TownMap m, Map<Integer, int[]> lots, long town, Field f, Item crop) {
        List<double[][]>[] paths = paths(plan);
        FieldMaps.Parcel p = m.parcels.get(f.holding);
        int[] off = offsets(town, f.holding);
        double[][] track = m.tracks.get(p.track()).line();
        List<int[]> cells = new ArrayList<>(cells(plan, m, paths, lots, town, f));
        cells.sort(Comparator.comparingDouble(c -> FieldMaps.from(track, c[0] + 0.5, c[1] + 0.5)));
        // One height for the sown part: its blocks' median, so channels hold their water.
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
        int y = heights.get(heights.size() / 2);
        int planted = 0;
        for (int[] c : cells) {
            if (planted >= PLOTS) {
                break;
            }
            int x = c[0], z = c[1];
            int across = Math.floorMod((p.northSouth() ? x : z) - off[0], PITCH);
            Plots.level(level, x, z, x, z, y);
            fell(level, x, y, z);
            BlockPos ground = new BlockPos(x, y, z), over = ground.above();
            level.setBlock(over, Blocks.AIR.defaultBlockState(), 2);
            if (crop == Items.SWEET_BERRIES) {
                level.setBlock(ground, Blocks.GRASS_BLOCK.defaultBlockState(), 3);
                if (across % 2 == 0) {
                    level.setBlock(over, Blocks.SWEET_BERRY_BUSH.defaultBlockState(), 3);
                    planted++;
                }
            } else if (crop == Items.SUGAR_CANE) {
                if (across % 3 == 1) {
                    level.setBlock(ground, Blocks.WATER.defaultBlockState(), 3);
                } else {
                    level.setBlock(ground, Blocks.GRASS_BLOCK.defaultBlockState(), 3);
                    level.setBlock(over, Blocks.SUGAR_CANE.defaultBlockState(), 3);
                    planted++;
                }
            } else if (across == STRIP / 2) {
                level.setBlock(ground, Blocks.WATER.defaultBlockState(), 3);
            } else {
                level.setBlock(ground, Blocks.FARMLAND.defaultBlockState(), 3);
                level.setBlock(over, seedling(crop), 3);
                planted++;
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
