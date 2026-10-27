package dev.b28.enterprise.territory;

import dev.b28.enterprise.hex.Hex;
import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.BlockTags;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Each town's forum (John: "a forum/agora per town ... with major paths for trade routes"): a paved square round its
 * bell, laid free the first time the town is surveyed (core mode), its heart that the main roads leave from along the
 * trade routes (each road laid taut from bell to bell, {@link Trails#taut}, three wide through the forum's hex). Paved only where the ground is the land's own and near the bell's height: a village's
 * well, houses and paths are left as they stand. The Masons' Square (a work the town pays for, for its mood) is built
 * apart from it.
 *
 * <p>Saved with the world: the towns that have one, by their bell.
 */
public final class Forums extends SavedData {
    private static final String NAME = "enterprise_forums";
    /** A forum reaches this far each way from its bell: 13 x 13 (John: "town squares can be a bit bigger"). */
    public static final int REACH = 6;
    /** How far above or below the bell's ground the forum's paving may be. */
    static final int RISE = 1;
    /** A main road's width: a village street. */
    public static final int ROAD_WIDTH = 3;

    private final Map<Long, BlockPos> bells = new LinkedHashMap<>();

    public static Forums get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Forums::new, Forums::load, null), NAME);
    }

    /** The towns' forums, by town: their bells. */
    public Map<Long, BlockPos> bells() {
        return Map.copyOf(bells);
    }

    /**
     * The town's forum laid round {@code bell} if it has none yet and its ground is loaded. Returns the blocks paved
     * (none if it had one, or its ground isn't loaded: tried again at its next survey).
     */
    public int ensure(ServerLevel level, long town, BlockPos bell) {
        if (bells.containsKey(town)) {
            return 0;
        }
        for (int dx = -REACH; dx <= REACH; dx += 2 * REACH) {
            for (int dz = -REACH; dz <= REACH; dz += 2 * REACH) {
                if (!level.isLoaded(bell.offset(dx, 0, dz))) {
                    return 0;
                }
            }
        }
        int paved = pave(level, bell, REACH);
        bells.put(town, bell);
        setDirty();
        return paved;
    }

    /** The forum's paving, {@code reach} each way: each column round the bell whose top is the land's own ground, near its height, paved. */
    public static int pave(ServerLevel level, BlockPos bell, int reach) {
        // The forum's ground: the height most of its columns stand at.
        Map<Integer, Integer> heights = new java.util.HashMap<>();
        for (int dx = -reach; dx <= reach; dx++) {
            for (int dz = -reach; dz <= reach; dz++) {
                heights.merge(level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, bell.getX() + dx, bell.getZ() + dz) - 1,
                        1, Integer::sum);
            }
        }
        int ground = heights.entrySet().stream().max(Map.Entry.<Integer, Integer>comparingByValue()
                .thenComparing(Map.Entry.comparingByKey())).orElseThrow().getKey();
        int paved = 0;
        for (int dx = -reach; dx <= reach; dx++) {
            for (int dz = -reach; dz <= reach; dz++) {
                int x = bell.getX() + dx;
                int z = bell.getZ() + dz;
                int y = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z) - 1;
                BlockPos top = new BlockPos(x, y, z);
                BlockState state = level.getBlockState(top);
                if (Math.abs(y - ground) > RISE || !own(state)) {
                    continue;
                }
                BlockState above = level.getBlockState(top.above());
                if (!above.isAir() && above.canBeReplaced() && above.getFluidState().isEmpty()) {
                    level.setBlock(top.above(), Blocks.AIR.defaultBlockState(), 3);
                }
                level.setBlock(top, (dx + dz) % 2 == 0 ? Blocks.STONE_BRICKS.defaultBlockState()
                        : Blocks.POLISHED_ANDESITE.defaultBlockState(), 3);
                paved++;
            }
        }
        return paved;
    }

    /** The land's own ground (soil, sand, gravel, a path), not a field, water or anything built. */
    static boolean own(BlockState state) {
        return dev.b28.enterprise.settlement.Grounds.natural(state) && !state.is(Blocks.FARMLAND)
                && state.getFluidState().isEmpty() && !state.is(BlockTags.BASE_STONE_OVERWORLD);
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        CompoundTag towns = new CompoundTag();
        bells.forEach((town, bell) -> towns.putLong(Long.toString(town), bell.asLong()));
        tag.put("bells", towns);
        return tag;
    }

    public static Forums load(CompoundTag tag, HolderLookup.Provider registries) {
        Forums forums = new Forums();
        CompoundTag towns = tag.getCompound("bells");
        for (String key : towns.getAllKeys()) {
            forums.bells.put(Long.parseLong(key), BlockPos.of(towns.getLong(key)));
        }
        return forums;
    }
}
