package dev.b28.enterprise.territory;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.BlockTags;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.entity.ai.village.poi.PoiRecord;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.FenceGateBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Heightmap;

/**
 * Pastures laid to the land (core mode's `Work::Pasture`; John: "use the terrain generated to build efficient
 * rectangular pastures"): the largest rectangle of the land's own open ground the town's zoned ground has near its bell, fenced
 * where the ground lies (it may roll a little, as a field may not), a gate on the side facing the bell. The core holds
 * an animal a `grass_per_animal` of the grass inside, so a town on broad meadows keeps bigger herds than one on a
 * hillside. Not the Java rules' 9 x 9 ({@link Husbandry#fence}).
 */
public final class Pastures {
    /** The rectangles tried, the largest first (outer size, fence included): wide ones before square ones of one area. */
    static final int[][] SIZES = {{18, 14}, {16, 14}, {16, 12}, {14, 12}, {12, 12}, {14, 10}, {12, 10}, {10, 10},
            {12, 8}, {10, 8}, {9, 9}, {8, 8}, {8, 7}, {7, 7}};
    /** How far the ground inside a pasture may rise and fall: cattle graze a slope. */
    static final int ROLL = 3;
    /** Nearest a pasture's middle may be to the bell. */
    static final int NEAREST = 10;

    private Pastures() {}

    /** A pasture's place: its corner (lowest x and z) and its outer size, x by z. */
    public record Site(BlockPos corner, int sizeX, int sizeZ) {}

    /**
     * The best pasture site round {@code bell}: of the {@link #SIZES}, the largest that fits somewhere on zoned ground
     * within {@link Growth#FIELD_FARTHEST} (none of the town's beds, workstations or bell within {@link Growth#YARD},
     * none farther out beside it), the nearest spot for it. Its ground is open grassland (no water, field, path or
     * building, nothing standing over it) within {@link #ROLL} of level and of the bell's height by
     * {@link Growth#FIELD_RISE}. Empty if nothing fits or the ground isn't loaded.
     */
    public static Optional<Site> site(ServerLevel level, BlockPos bell) {
        int reach = TownPlans.farmReach(level, bell, Growth.FIELD_FARTHEST);
        int span = 2 * reach + 1;
        int[] height = new int[span * span];
        boolean[] open = new boolean[span * span];
        int x0 = bell.getX() - reach;
        int z0 = bell.getZ() - reach;
        for (int i = 0; i < span; i++) {
            for (int j = 0; j < span; j++) {
                int x = x0 + i;
                int z = z0 + j;
                int k = i * span + j;
                if (!level.isLoaded(new BlockPos(x, 0, z))) {
                    height[k] = Integer.MIN_VALUE;
                    continue;
                }
                int y = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z) - 1;
                height[k] = y;
                BlockPos top = new BlockPos(x, y, z);
                open[k] = grassland(level.getBlockState(top)) && level.getBlockState(top.above()).canBeReplaced()
                        && level.getBlockState(top.above()).getFluidState().isEmpty()
                        && Math.abs(y - bell.getY()) <= Growth.FIELD_RISE;
            }
        }
        // Ground already a pasture's: its birch fence within reach every way round it (not the oak the ground's safety
        // rails its drops with, which ringed Meadowfell's hills).
        boolean[] fenced = new boolean[span * span];
        for (int k = 0; k < span * span; k++) {
            if (height[k] != Integer.MIN_VALUE) {
                BlockState top = level.getBlockState(new BlockPos(x0 + k / span, height[k], z0 + k % span));
                fenced[k] = top.is(FENCE) || top.is(GATE);
            }
        }
        for (int i = 0; i < span; i++) {
            for (int j = 0; j < span; j++) {
                if (open[i * span + j] && enclosed(fenced, span, i, j)) {
                    open[i * span + j] = false;
                }
            }
        }
        List<BlockPos> town = level.getPoiManager()
                .getInRange(type -> true, bell, reach + 24, PoiManager.Occupancy.ANY).map(PoiRecord::getPos).toList();
        for (int[] size : SIZES) {
            Optional<Site> found = place(level, bell, size[0], size[1], span, x0, z0, height, open, town);
            if (found.isEmpty() && size[0] != size[1]) {
                found = place(level, bell, size[1], size[0], span, x0, z0, height, open, town);
            }
            if (found.isPresent()) {
                return found;
            }
        }
        return Optional.empty();
    }

    /** How far a fence is looked for each way round a column, to see it's fenced in. */
    static final int ENCLOSED = 24;

    /** Is the column at ({@code i}, {@code j}) fenced in: a fence within {@link #ENCLOSED} north, south, east and west? */
    public static boolean enclosed(boolean[] fenced, int span, int i, int j) {
        int[][] ways = {{1, 0}, {-1, 0}, {0, 1}, {0, -1}};
        for (int[] w : ways) {
            boolean hit = false;
            for (int n = 1; n <= ENCLOSED && !hit; n++) {
                int a = i + w[0] * n;
                int b = j + w[1] * n;
                if (a < 0 || b < 0 || a >= span || b >= span) {
                    break;
                }
                hit = fenced[a * span + b];
            }
            if (!hit) {
                return false;
            }
        }
        return true;
    }

    /** The nearest corner for a {@code sx} x {@code sz} pasture on the surveyed ground, if any. */
    private static Optional<Site> place(ServerLevel level, BlockPos bell, int sx, int sz, int span, int x0, int z0,
            int[] height, boolean[] open, List<BlockPos> town) {
        List<int[]> corners = new ArrayList<>();
        for (int i = 0; i + sx <= span; i++) {
            for (int j = 0; j + sz <= span; j++) {
                double mx = x0 + i + sx / 2.0 - bell.getX();
                double mz = z0 + j + sz / 2.0 - bell.getZ();
                double d = Math.hypot(mx, mz);
                if (d >= NEAREST && d <= span / 2.0) {
                    corners.add(new int[] {i, j, (int) (d * 16)});
                }
            }
        }
        corners.sort(Comparator.<int[]>comparingInt(c -> c[2]).thenComparingInt(c -> c[0]).thenComparingInt(c -> c[1]));
        for (int[] c : corners) {
            int top = Integer.MIN_VALUE;
            int bottom = Integer.MAX_VALUE;
            boolean ok = true;
            for (int a = 0; a < sx && ok; a++) {
                for (int b = 0; b < sz; b++) {
                    int k = (c[0] + a) * span + c[1] + b;
                    if (!open[k]) {
                        ok = false;
                        break;
                    }
                    top = Math.max(top, height[k]);
                    bottom = Math.min(bottom, height[k]);
                }
            }
            if (!ok || top - bottom > ROLL) {
                continue;
            }
            int cx = x0 + c[0];
            int cz = z0 + c[1];
            BlockPos corner = new BlockPos(cx, height[c[0] * span + c[1]], cz);
            if (zoned(bell, cx, cz, sx, sz, town) && TownPlans.clear(level, cx, cz, sx, sz, true)) {
                return Optional.of(new Site(corner, sx, sz));
            }
        }
        return Optional.empty();
    }

    /** As {@link Growth#zoned}, for an {@code sx} x {@code sz} footprint. */
    static boolean zoned(BlockPos bell, int x0, int z0, int sx, int sz, List<BlockPos> town) {
        int x1 = x0 + sx - 1;
        int z1 = z0 + sz - 1;
        double fromBell = Math.sqrt(squaredToBox(bell.getX(), bell.getZ(), x0, z0, x1, z1));
        for (BlockPos place : town) {
            double near = Math.sqrt(squaredToBox(place.getX(), place.getZ(), x0, z0, x1, z1));
            if (near <= Growth.YARD) {
                return false;
            }
            if (near <= 12 && Math.hypot(place.getX() - bell.getX(), place.getZ() - bell.getZ()) > fromBell) {
                return false;
            }
        }
        return true;
    }

    private static double squaredToBox(int x, int z, int x0, int z0, int x1, int z1) {
        int dx = Math.max(0, Math.max(x0 - x, x - x1));
        int dz = Math.max(0, Math.max(z0 - z, z - z1));
        return (double) dx * dx + (double) dz * dz;
    }

    /** A pasture's fence and gate: birch, its own, so its ground is known for a pasture's. */
    static final net.minecraft.world.level.block.Block FENCE = Blocks.BIRCH_FENCE;
    static final net.minecraft.world.level.block.Block GATE = Blocks.BIRCH_FENCE_GATE;

    /**
     * Ground a pasture grazes: the land's own (grass and earth, sand, a desert's or a badland's), not a path, a field or
     * bare rock. Dustfell's desert, held to grass, laid none.
     */
    static boolean grassland(BlockState state) {
        return dev.b28.enterprise.settlement.Grounds.natural(state) && !state.is(Blocks.DIRT_PATH)
                && !state.is(Blocks.FARMLAND) && !state.is(BlockTags.BASE_STONE_OVERWORLD);
    }

    /**
     * Fences a pasture where its ground lies (each post on its own column's top), the gate in the middle of the side
     * facing {@code bell}, the earth inside grassed. Returns the blocks of grass inside the fence.
     */
    public static int fence(ServerLevel level, Site site, BlockPos bell) {
        int sx = site.sizeX();
        int sz = site.sizeZ();
        int x0 = site.corner().getX();
        int z0 = site.corner().getZ();
        double mx = x0 + sx / 2.0 - bell.getX();
        double mz = z0 + sz / 2.0 - bell.getZ();
        // The side facing the bell, and its middle.
        Direction toward = Math.abs(mx) > Math.abs(mz) ? (mx > 0 ? Direction.WEST : Direction.EAST)
                : (mz > 0 ? Direction.NORTH : Direction.SOUTH);
        int grass = 0;
        for (int a = 0; a < sx; a++) {
            for (int b = 0; b < sz; b++) {
                int x = x0 + a;
                int z = z0 + b;
                int y = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z) - 1;
                BlockPos top = new BlockPos(x, y, z);
                boolean edge = a == 0 || b == 0 || a == sx - 1 || b == sz - 1;
                if (!edge) {
                    if (level.getBlockState(top).is(BlockTags.DIRT) && !level.getBlockState(top).is(Blocks.GRASS_BLOCK)) {
                        level.setBlock(top, Blocks.GRASS_BLOCK.defaultBlockState(), 3);
                    }
                    grass++;
                    continue;
                }
                boolean gate = switch (toward) {
                    case WEST -> a == 0 && b == sz / 2;
                    case EAST -> a == sx - 1 && b == sz / 2;
                    case NORTH -> b == 0 && a == sx / 2;
                    default -> b == sz - 1 && a == sx / 2;
                };
                Direction facing = toward.getAxis() == Direction.Axis.X ? Direction.EAST : Direction.SOUTH;
                level.setBlock(top.above(), gate
                        ? GATE.defaultBlockState().setValue(FenceGateBlock.FACING, facing)
                        : FENCE.defaultBlockState(), 3);
            }
        }
        return grass;
    }
}
