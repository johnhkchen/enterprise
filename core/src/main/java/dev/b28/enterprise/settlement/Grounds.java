package dev.b28.enterprise.settlement;

import dev.b28.enterprise.Enterprise;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.gametest.framework.GameTestServer;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.TickTask;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.BlockTags;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.DoorBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.phys.AABB;

/**
 * A town's ground made safe once, when it's founded: the narrow pits and trenches round its desk filled (a villager in
 * one can't climb out, and one more than 3 deep hurts to fall in), the drops left beside them fenced at the top, and
 * anyone stuck down a pit lifted out as it fills. Only the lie of the land is touched: a pit is filled only between
 * banks of natural ground (dirt, stone, sand...), or one natural and one a building's wall if it's 1 wide (an alley a
 * village left between a house and a hill cut, its roof's eave a step off the hill: fenced on top once filled), with
 * the natural bank's block, and only where nothing but air or plants stands and no door opens; a fence only on
 * natural ground, never over water (a fall into water hurts no one). Ground not loaded is left.
 */
public final class Grounds {
    /** How far round the desk the ground is made safe: the town's own streets and fields. */
    public static final int RADIUS = 40;
    /** A pit at least this deep is filled: a villager can't jump out of 2. */
    static final int DEEP = 2;
    /** The widest pit filled, in blocks: wider is a valley, fenced at its edges instead. */
    static final int WIDEST = 3;
    /** A drop at least this high is fenced: a fall of 4 hurts. */
    static final int DROP = 4;

    /** What was done: columns filled, fences put up, creatures lifted out of pits. */
    public record Made(int filled, int fenced, int lifted) {}

    private Grounds() {}

    /**
     * The ground round a town's desk made safe, as {@link #make}, at the server's next turn (a desk can be founded as
     * its chunk loads, when no block may change); not in game tests, whose arenas sit side by side.
     */
    public static void founded(ServerLevel level, BlockPos desk, String name) {
        MinecraftServer server = level.getServer();
        if (server instanceof GameTestServer) {
            return;
        }
        server.tell(new TickTask(server.getTickCount(), () -> {
            Made made = make(level, desk, RADIUS);
            if (made.filled() + made.fenced() > 0) {
                Enterprise.LOGGER.info("{}'s ground made safe: {} columns of pits filled, {} fences at drops, {} lifted out",
                        name, made.filled(), made.fenced(), made.lifted());
            }
        }));
    }

    /** The ground within {@code radius} of {@code centre} (a square) made safe. */
    public static Made make(ServerLevel level, BlockPos centre, int radius) {
        int n = 2 * radius + 1;
        int x0 = centre.getX() - radius;
        int z0 = centre.getZ() - radius;
        // Each column's top: the first free height over it, or MIN where there's none (not loaded, or water); and
        // whether that top is natural ground or something built (a roof, a wall).
        int[][] free = new int[n][n];
        boolean[][] natural = new boolean[n][n];
        for (int i = 0; i < n; i++) {
            for (int j = 0; j < n; j++) {
                free[i][j] = top(level, x0 + i, z0 + j);
                natural[i][j] = free[i][j] != Integer.MIN_VALUE
                        && natural(level.getBlockState(new BlockPos(x0 + i, free[i][j] - 1, z0 + j)));
            }
        }
        int filled = 0;
        int lifted = 0;
        int fenced = 0;
        // Pits: a run of up to WIDEST columns of natural ground along x or z, each DEEP or more below both banks,
        // filled to the lower bank with the natural bank's block. A bank can be a building's wall (an alley between a
        // house and a hill cut) if the other is natural ground, the alley 1 wide (a wider one is a street) and fenced
        // on top once filled (or its new ground leads onto the roof). Again till none is left (a filled trench can
        // leave a shallower one across it).
        for (int pass = 0; pass < 4; pass++) {
            int before = filled;
            for (int axis = 0; axis < 2; axis++) {
                for (int line = 0; line < n; line++) {
                    for (int s = 1; s < n - 1; s++) {
                        for (int w = 1; w <= WIDEST && s + w < n; w++) {
                            int bankA = at(free, axis, line, s - 1);
                            int bankB = at(free, axis, line, s + w);
                            boolean naturalA = at(natural, axis, line, s - 1);
                            boolean naturalB = at(natural, axis, line, s + w);
                            boolean alley = !naturalA || !naturalB;
                            if (bankA == Integer.MIN_VALUE || bankB == Integer.MIN_VALUE || !naturalA && !naturalB
                                    || alley && w > 1) {
                                continue;
                            }
                            int to = Math.min(bankA, bankB);
                            boolean pit = true;
                            for (int k = s; k < s + w && pit; k++) {
                                int h = at(free, axis, line, k);
                                pit = h != Integer.MIN_VALUE && at(natural, axis, line, k) && h <= to - DEEP;
                            }
                            if (!pit) {
                                continue;
                            }
                            int lower = naturalA && (bankA <= bankB || !naturalB) ? s - 1 : s + w;
                            BlockPos bank = pos(x0, z0, axis, line, lower, at(free, axis, line, lower) - 1);
                            for (int k = s; k < s + w; k++) {
                                BlockPos column = pos(x0, z0, axis, line, k, at(free, axis, line, k));
                                if (fill(level, column, to, level.getBlockState(bank))) {
                                    lifted += lift(level, column, to);
                                    set(free, axis, line, k, to);
                                    filled++;
                                    if (alley && fence(level, column.atY(to))) {
                                        fenced++;
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
            }
            if (filled == before) {
                break;
            }
        }
        // Drops: natural ground with a neighbour, side or corner, DROP or more below it (not water) fenced at the top.
        for (int i = 0; i < n; i++) {
            for (int j = 0; j < n; j++) {
                int h = free[i][j];
                if (h == Integer.MIN_VALUE || !natural[i][j]) {
                    continue;
                }
                boolean drop = false;
                for (int a = Math.max(0, i - 1); a <= Math.min(n - 1, i + 1); a++) {
                    for (int b = Math.max(0, j - 1); b <= Math.min(n - 1, j + 1); b++) {
                        drop |= free[a][b] != Integer.MIN_VALUE && free[a][b] <= h - DROP;
                    }
                }
                if (drop && fence(level, new BlockPos(x0 + i, h, z0 + j))) {
                    fenced++;
                }
            }
        }
        return new Made(filled, fenced, lifted);
    }

    /** The first free height over the ground at (x, z), natural or built, or MIN where it's water or not loaded. */
    private static int top(ServerLevel level, int x, int z) {
        if (!level.hasChunk(x >> 4, z >> 4)) {
            return Integer.MIN_VALUE;
        }
        int free = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z);
        return level.getFluidState(new BlockPos(x, free - 1, z)).isEmpty() ? free : Integer.MIN_VALUE;
    }

    /** A fence put up at {@code at}, if nothing but air or a plant is there. */
    private static boolean fence(ServerLevel level, BlockPos at) {
        if (!level.getBlockState(at).canBeReplaced() || !level.getFluidState(at).isEmpty()) {
            return false;
        }
        level.setBlock(at, Block.updateFromNeighbourShapes(Blocks.OAK_FENCE.defaultBlockState(), level, at), Block.UPDATE_ALL);
        return true;
    }

    /** Ground as the world made it: not a village's planks and cobbles, nor a player's. */
    static boolean natural(BlockState state) {
        return state.is(BlockTags.DIRT) || state.is(BlockTags.BASE_STONE_OVERWORLD) || state.is(BlockTags.SAND)
                || state.is(BlockTags.TERRACOTTA) || state.is(Blocks.GRAVEL) || state.is(Blocks.DIRT_PATH)
                || state.is(Blocks.FARMLAND) || state.is(Blocks.CLAY) || state.is(Blocks.SNOW_BLOCK)
                || state.is(Blocks.SANDSTONE) || state.is(Blocks.RED_SANDSTONE);
    }

    /**
     * The column from {@code column} up to (not into) {@code to} filled with the bank's block (dirt under grass or a
     * path), if all of it is air or plants and no door opens onto it. False, and nothing touched, if not.
     */
    private static boolean fill(ServerLevel level, BlockPos column, int to, BlockState bank) {
        for (int y = column.getY(); y < to; y++) {
            BlockPos p = column.atY(y);
            if (!level.getBlockState(p).canBeReplaced() || !level.getFluidState(p).isEmpty()) {
                return false;
            }
            for (Direction side : Direction.Plane.HORIZONTAL) {
                if (level.getBlockState(p.relative(side)).getBlock() instanceof DoorBlock) {
                    return false; // a door onto it: a way in, not a pit
                }
            }
        }
        BlockState under = bank.is(Blocks.GRASS_BLOCK) || bank.is(Blocks.DIRT_PATH) || bank.is(Blocks.FARMLAND)
                ? Blocks.DIRT.defaultBlockState() : bank;
        BlockState top = bank.is(Blocks.FARMLAND) ? Blocks.DIRT.defaultBlockState() : bank;
        for (int y = column.getY(); y < to; y++) {
            level.setBlock(column.atY(y), y == to - 1 ? top : under, Block.UPDATE_ALL);
        }
        return true;
    }

    /** Creatures in the column filled to {@code to} stood on its new ground; how many. */
    private static int lift(ServerLevel level, BlockPos column, int to) {
        AABB in = new AABB(column.getX(), column.getY() - 1, column.getZ(), column.getX() + 1, to, column.getZ() + 1);
        int n = 0;
        for (LivingEntity e : level.getEntitiesOfClass(LivingEntity.class, in)) {
            e.teleportTo(e.getX(), to, e.getZ());
            n++;
        }
        return n;
    }

    private static int at(int[][] free, int axis, int line, int k) {
        return axis == 0 ? free[k][line] : free[line][k];
    }

    private static boolean at(boolean[][] of, int axis, int line, int k) {
        return axis == 0 ? of[k][line] : of[line][k];
    }

    private static void set(int[][] free, int axis, int line, int k, int v) {
        if (axis == 0) {
            free[k][line] = v;
        } else {
            free[line][k] = v;
        }
    }

    private static BlockPos pos(int x0, int z0, int axis, int line, int k, int y) {
        return axis == 0 ? new BlockPos(x0 + k, y, z0 + line) : new BlockPos(x0 + line, y, z0 + k);
    }
}
