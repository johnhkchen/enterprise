package dev.b28.enterprise.territory;

import java.util.Comparator;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.BlockTags;
import net.minecraft.tags.PoiTypeTags;
import net.minecraft.tags.StructureTags;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.entity.ai.village.poi.PoiTypes;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.structure.BoundingBox;
import net.minecraft.world.level.levelgen.structure.PoolElementStructurePiece;
import net.minecraft.world.level.levelgen.structure.StructurePiece;
import net.minecraft.world.level.levelgen.structure.StructureStart;

/**
 * The vanilla village a planned town grew from, replaced a house at a time (John: "replace one by one"; the studio's
 * planned towns, thread 0012: "no vanilla houses inside the walls"): when the town has laid a house of its kingdom's, the
 * vanilla house nearest its bell with no more beds than the new one gave and no job site in it (nobody loses work) is
 * cleared to grass, so its people take the new beds and the town never has fewer.
 */
public final class Vanilla {
    private Vanilla() {}

    /**
     * Clears the vanilla house nearest {@code bell} that a house of {@code beds} new beds can take the people of: its
     * beds 1 to {@code beds}, no job site. The beds it held, or 0 if none was cleared.
     */
    public static int replace(ServerLevel level, BlockPos bell, int beds) {
        if (beds <= 0) {
            return 0;
        }
        StructureStart village = level.structureManager().getStructureWithPieceAt(bell, StructureTags.VILLAGE);
        if (village == null || !village.isValid()) {
            return 0;
        }
        PoiManager pois = level.getPoiManager();
        Optional<BoundingBox> pick = village.getPieces().stream().filter(Vanilla::isHouse).map(StructurePiece::getBoundingBox)
                .filter(box -> level.isLoaded(new BlockPos(box.minX(), 0, box.minZ())) && level.isLoaded(new BlockPos(box.maxX(), 0, box.maxZ())))
                .filter(box -> {
                    long held = count(pois, box, true);
                    return held > 0 && held <= beds && count(pois, box, false) == 0;
                })
                .min(Comparator.comparingDouble(box -> box.getCenter().distSqr(bell)));
        if (pick.isEmpty()) {
            return 0;
        }
        BoundingBox box = pick.get();
        int held = (int) count(pois, box, true);
        for (int x = box.minX(); x <= box.maxX(); x++) {
            for (int z = box.minZ(); z <= box.maxZ(); z++) {
                for (int y = box.maxY(); y > box.minY(); y--) {
                    BlockPos at = new BlockPos(x, y, z);
                    if (!level.getBlockState(at).isAir()) {
                        level.setBlock(at, Blocks.AIR.defaultBlockState(), 2 | 16);
                    }
                }
                BlockPos floor = new BlockPos(x, box.minY(), z);
                BlockState was = level.getBlockState(floor);
                if (!was.is(BlockTags.DIRT) || was.is(Blocks.DIRT_PATH)) {
                    level.setBlock(floor, Blocks.GRASS_BLOCK.defaultBlockState(), 2 | 16);
                }
            }
        }
        dev.b28.enterprise.Enterprise.LOGGER.info("The vanilla house at {} cleared ({} beds): its people move to the new",
                box.getCenter().toShortString(), held);
        return held;
    }

    /** Is the piece one of the village's houses (not a street, a well, a lamp or a decoration)? */
    static boolean isHouse(StructurePiece piece) {
        return piece instanceof PoolElementStructurePiece p && p.getElement().toString().contains("/houses/");
    }

    /** The beds ({@code beds}) or job sites in a box. */
    private static long count(PoiManager pois, BoundingBox box, boolean beds) {
        BlockPos mid = box.getCenter();
        int reach = Math.max(box.getXSpan(), box.getZSpan()) / 2 + 1;
        return pois.getInSquare(type -> beds ? type.is(PoiTypes.HOME) : type.is(PoiTypeTags.ACQUIRABLE_JOB_SITE), mid, reach,
                PoiManager.Occupancy.ANY).filter(r -> box.isInside(r.getPos())).count();
    }
}
