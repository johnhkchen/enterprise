package dev.b28.enterprise.territory;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.BlockTags;
import net.minecraft.tags.PoiTypeTags;
import net.minecraft.tags.StructureTags;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.entity.ai.village.poi.PoiTypes;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.structure.BoundingBox;
import net.minecraft.world.level.levelgen.structure.StructurePiece;
import net.minecraft.world.level.levelgen.structure.StructureStart;

/**
 * The vanilla village a town grew from, cleared whole once at its founding (John: "a onetime conversion of the town into
 * the new style at establishment instead of keeping the old look and streets"): its houses, streets, farms and lamps
 * taken down to the grass, so its plan's plots are free for its kingdom's houses. What it held is told back (its beds,
 * its job sites' blocks) for the town to lay again in the new style at once: nobody is left without a bed or a job.
 * The bell, the forum round it, and anything not vanilla's (the Merchant's desk, a player's build) stay.
 */
public final class Conversion {
    private Conversion() {}

    /** What the village held: its beds, and the block of each job site, to be laid again. */
    public record Cleared(int beds, List<BlockState> jobSites, int pieces) {}

    /**
     * Clears the vanilla village round {@code bell}, every piece of it, if all its ground is loaded. Empty if it isn't
     * yet (ask again); {@code Cleared(0, [], 0)} if the bell isn't a vanilla village's.
     */
    public static Optional<Cleared> clear(ServerLevel level, BlockPos bell) {
        StructureStart village = village(level, bell);
        if (village == null) {
            return Optional.of(new Cleared(0, List.of(), 0));
        }
        List<BoundingBox> boxes = village.getPieces().stream().map(StructurePiece::getBoundingBox).toList();
        for (BoundingBox box : boxes) {
            if (!level.isLoaded(new BlockPos(box.minX(), 0, box.minZ())) || !level.isLoaded(new BlockPos(box.maxX(), 0, box.maxZ()))) {
                return Optional.empty();
            }
        }
        PoiManager pois = level.getPoiManager();
        int beds = 0, pieces = 0;
        List<BlockState> jobs = new ArrayList<>();
        for (StructurePiece piece : village.getPieces()) {
            BoundingBox box = piece.getBoundingBox();
            // A piece with something of the mod's or a player's in it is left as it is.
            if (foreign(level, box)) {
                continue;
            }
            // A street follows the land: its path grassed over, the land under and round it left as it lies.
            if (street(piece)) {
                for (int x = box.minX(); x <= box.maxX(); x++) {
                    for (int z = box.minZ(); z <= box.maxZ(); z++) {
                        if (Math.max(Math.abs(x - bell.getX()), Math.abs(z - bell.getZ())) <= Forums.REACH + 1) {
                            continue;
                        }
                        BlockPos top = level.getHeightmapPos(net.minecraft.world.level.levelgen.Heightmap.Types.WORLD_SURFACE,
                                new BlockPos(x, 0, z)).below();
                        BlockState paved = level.getBlockState(top);
                        if (PATHS.contains(paved.getBlock())) {
                            level.setBlock(top, Blocks.GRASS_BLOCK.defaultBlockState(), 2 | 16);
                        } else if (paved.is(BlockTags.PLANKS) || paved.is(BlockTags.WOODEN_SLABS)) {
                            // A street's plank bridge over the shallows (the studio's r72: a town's bridge never laid
                            // over it): water where water was, the bank's grass where it stood on land.
                            BlockState under = level.getBlockState(top.below());
                            boolean wet = false;
                            for (var d : net.minecraft.core.Direction.Plane.HORIZONTAL) {
                                wet |= !level.getFluidState(top.relative(d)).isEmpty();
                            }
                            level.setBlock(top, wet || !under.getFluidState().isEmpty() ? Blocks.WATER.defaultBlockState()
                                    : Blocks.GRASS_BLOCK.defaultBlockState(), 2 | 16);
                        }
                    }
                }
                pieces++;
                continue;
            }
            BlockPos mid = box.getCenter();
            int reach = Math.max(box.getXSpan(), box.getZSpan()) / 2 + 1;
            for (var p : pois.getInSquare(t -> t.is(PoiTypes.HOME) || t.is(PoiTypeTags.ACQUIRABLE_JOB_SITE), mid, reach,
                    PoiManager.Occupancy.ANY).filter(r -> box.isInside(r.getPos())).toList()) {
                if (p.getPoiType().is(PoiTypes.HOME)) {
                    beds++;
                } else {
                    jobs.add(level.getBlockState(p.getPos()));
                }
            }
            for (int x = box.minX(); x <= box.maxX(); x++) {
                for (int z = box.minZ(); z <= box.maxZ(); z++) {
                    if (Math.max(Math.abs(x - bell.getX()), Math.abs(z - bell.getZ())) <= Forums.REACH + 1) {
                        continue; // the forum and its centrepiece
                    }
                    for (int y = box.maxY(); y > box.minY(); y--) {
                        BlockPos at = new BlockPos(x, y, z);
                        if (!level.getBlockState(at).isAir()) {
                            level.setBlock(at, Blocks.AIR.defaultBlockState(), 2 | 16);
                        }
                    }
                    BlockPos floor = new BlockPos(x, box.minY(), z);
                    BlockState was = level.getBlockState(floor), under = level.getBlockState(floor.below());
                    // A floor on piles over the river or a drop (a dock, a fishing hut) goes as what's under it: no
                    // grass squares left floating on the water (the studio's r66 render).
                    if (!under.getFluidState().isEmpty() || under.isAir()) {
                        level.setBlock(floor, under.getFluidState().isEmpty() ? Blocks.AIR.defaultBlockState()
                                : under.getFluidState().createLegacyBlock(), 2 | 16);
                    } else if (!was.is(BlockTags.DIRT) || was.is(Blocks.DIRT_PATH) || was.is(Blocks.FARMLAND)) {
                        level.setBlock(floor, Blocks.GRASS_BLOCK.defaultBlockState(), 2 | 16);
                    }
                }
            }
            pieces++;
        }
        dev.b28.enterprise.Enterprise.LOGGER.info("The vanilla village at {} cleared for the new town ({} of {} pieces, {} beds, "
                + "{} job sites to lay again)", bell.toShortString(), pieces, boxes.size(), beds, jobs.size());
        return Optional.of(new Cleared(beds, jobs, pieces));
    }

    /**
     * The vanilla village whose ground the bell stands in (its pieces' whole extent, not only a piece holding the bell: a
     * town's bell is often moved off its well's), or null.
     */
    public static StructureStart village(ServerLevel level, BlockPos bell) {
        StructureStart in = level.structureManager().getStructureWithPieceAt(bell, StructureTags.VILLAGE);
        if (in != null && in.isValid()) {
            return in;
        }
        var registry = level.registryAccess().registryOrThrow(net.minecraft.core.registries.Registries.STRUCTURE);
        for (var holder : registry.getTagOrEmpty(StructureTags.VILLAGE)) {
            StructureStart at = level.structureManager().getStructureAt(bell, holder.value());
            if (at != null && at.isValid()) {
                return at;
            }
        }
        return null;
    }

    /** What a vanilla street is paved with, in any of its villages. */
    static final java.util.Set<net.minecraft.world.level.block.Block> PATHS = java.util.Set.of(Blocks.DIRT_PATH, Blocks.GRAVEL,
            Blocks.COBBLESTONE, Blocks.MOSSY_COBBLESTONE, Blocks.SMOOTH_SANDSTONE, Blocks.PACKED_ICE);

    /** Is the piece one of the village's streets? */
    static boolean street(StructurePiece piece) {
        return piece instanceof net.minecraft.world.level.levelgen.structure.PoolElementStructurePiece p
                && p.getElement().toString().contains("/streets/");
    }

    /** Does the box hold a block that isn't vanilla's (the mod's desk or lockers, another mod's, a player's)? */
    static boolean foreign(ServerLevel level, BoundingBox box) {
        for (int x = box.minX(); x <= box.maxX(); x++) {
            for (int z = box.minZ(); z <= box.maxZ(); z++) {
                for (int y = box.minY(); y <= box.maxY(); y++) {
                    BlockState s = level.getBlockState(new BlockPos(x, y, z));
                    if (!s.isAir() && !BuiltInRegistries.BLOCK.getKey(s.getBlock()).getNamespace().equals("minecraft")) {
                        return true;
                    }
                }
            }
        }
        return false;
    }
}
