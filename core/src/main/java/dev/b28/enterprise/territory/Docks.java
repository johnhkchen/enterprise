package dev.b28.enterprise.territory;

import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.levelgen.Heightmap;

/**
 * A town's Fishermen fish from a dock: a short pier out over water in its land. With a dock, each Fisherman lands
 * {@link #COD_PER_LEVEL_DAY} cod a day per level into the warehouse, food for rations, caravans and children, which
 * the Fishermen smoke (see {@link Workshop}) while food is short. Water stops being land the town can't use.
 */
public final class Docks {
    public static final double COD_PER_LEVEL_DAY = 4;

    /** A pier runs this many blocks out over water. */
    public static final int PIER = 4;

    private Docks() {}

    /**
     * The nearest shore within 32 blocks of the bell: a dry column with water for {@link #PIER} blocks out beside it.
     * Returns the first water column and the way out.
     */
    public static Optional<Pier> findShore(ServerLevel level, BlockPos bell) {
        for (int r = 2; r <= 32; r++) {
            for (int dx = -r; dx <= r; dx++) {
                for (int dz = -r; dz <= r; dz++) {
                    if (Math.max(Math.abs(dx), Math.abs(dz)) != r) {
                        continue; // only this ring
                    }
                    BlockPos land = surface(level, bell.getX() + dx, bell.getZ() + dz);
                    if (land == null || !level.getFluidState(land).isEmpty()) {
                        continue;
                    }
                    for (Direction out : Direction.Plane.HORIZONTAL) {
                        if (wetFor(level, land, out)) {
                            return Optional.of(new Pier(land.relative(out), out));
                        }
                    }
                }
            }
        }
        return Optional.empty();
    }

    /** Where a pier starts (its first block over water) and which way it runs. */
    public record Pier(BlockPos start, Direction out) {}

    private static boolean wetFor(ServerLevel level, BlockPos land, Direction out) {
        for (int i = 1; i <= PIER; i++) {
            BlockPos over = surface(level, land.getX() + out.getStepX() * i, land.getZ() + out.getStepZ() * i);
            if (over == null || over.getY() != land.getY() || level.getFluidState(over).isEmpty()) {
                return false;
            }
        }
        return true;
    }

    /** The top block of a loaded column, or null. */
    private static BlockPos surface(ServerLevel level, int x, int z) {
        BlockPos column = new BlockPos(x, 0, z);
        if (!level.isLoaded(column)) {
            return null;
        }
        return new BlockPos(x, level.getHeight(Heightmap.Types.MOTION_BLOCKING, x, z) - 1, z);
    }

    /** Lays the pier: planks at the water's surface, a fence post and lantern at its end. */
    public static void build(ServerLevel level, Pier pier) {
        BlockPos at = pier.start();
        for (int i = 0; i < PIER; i++) {
            level.setBlock(at, Blocks.SPRUCE_PLANKS.defaultBlockState(), 3);
            if (i < PIER - 1) {
                at = at.relative(pier.out());
            }
        }
        level.setBlock(at.above(), Blocks.SPRUCE_FENCE.defaultBlockState(), 3);
        level.setBlock(at.above(2), Blocks.LANTERN.defaultBlockState(), 3);
    }
}
