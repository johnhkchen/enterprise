package dev.b28.enterprise.territory;

import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.LadderBlock;

/**
 * A town's Toolsmiths mine its land: a shaft near the bell, from which each Toolsmith brings up
 * {@link #RAW_IRON_PER_LEVEL_DAY} raw iron and {@link #COAL_PER_LEVEL_DAY} coal a day per level, half again with
 * pickaxes to dig with (see {@link Smithing}). The ore is the start of the metal chain: an Armorer smelts it, and
 * smiths make tools, swords and armour of the iron.
 */
public final class Mines {
    public static final double RAW_IRON_PER_LEVEL_DAY = 2;
    public static final double COAL_PER_LEVEL_DAY = 0.5;
    public static final double RAW_GOLD_PER_LEVEL_DAY = 0.25;

    /** A mine sunk deep (by a master Toolsmith, shored by a Mason) brings up lapis too. */
    public static final double LAPIS_PER_LEVEL_DAY = 0.5;
    public static final double DEEP_EMERALDS = 8;

    /** A mine head costs this much, from money beyond the town's reserve. */
    public static final double MINE_EMERALDS = 6;

    /** The mine head is 3 x 3, at least this far from the bell; its shaft goes this deep, or to just above bedrock. */
    public static final int SIZE = 3;
    private static final int NEAREST = 10;
    private static final int DEPTH = 8;

    private Mines() {}

    /** The nearest corner (ground level) where a mine head fits. */
    public static Optional<BlockPos> site(ServerLevel level, BlockPos bell) {
        return Growth.farmSite(level, bell, SIZE, NEAREST);
    }

    /**
     * A cobblestone collar round a shaft with a ladder down it, two posts and a lantern over it. Returns the top of
     * the shaft.
     */
    public static BlockPos build(ServerLevel level, BlockPos corner) {
        for (int dx = 0; dx < SIZE; dx++) {
            for (int dz = 0; dz < SIZE; dz++) {
                level.setBlock(corner.offset(dx, 0, dz), Blocks.COBBLESTONE.defaultBlockState(), 3);
            }
        }
        BlockPos shaft = corner.offset(1, 0, 1);
        int bottom = Math.max(level.getMinBuildHeight() + 1, shaft.getY() - DEPTH);
        for (int y = shaft.getY(); y >= bottom; y--) {
            BlockPos at = new BlockPos(shaft.getX(), y, shaft.getZ());
            if (level.getBlockState(at).is(Blocks.BEDROCK)) {
                break;
            }
            level.setBlock(at.north(), Blocks.COBBLESTONE.defaultBlockState(), 3); // the ladder's wall
            level.setBlock(at, Blocks.LADDER.defaultBlockState().setValue(LadderBlock.FACING, Direction.SOUTH), 3);
        }
        level.setBlock(corner.above(), Blocks.OAK_LOG.defaultBlockState(), 3);
        level.setBlock(corner.offset(SIZE - 1, 1, 0), Blocks.OAK_LOG.defaultBlockState(), 3);
        level.setBlock(corner.offset(SIZE - 1, 2, 0), Blocks.LANTERN.defaultBlockState(), 3);
        return shaft;
    }
}
