package dev.b28.enterprise.territory;

import dev.b28.enterprise.hex.Hex;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.LongArrayTag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.material.Fluids;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * The towns the core razed laid waste (`Output.Razed`; John: "just placing TNT at the heart of each build and detonating
 * it can work, along with removing the water sources of farms"): the water that watered its fields (a block deep, by
 * its farmland: its channels and ponds, not a river or the sea) filled in across the land it held, then a charge of TNT
 * set off at the heart of each building round its bell (its beds' and workstations' points, those near each other one
 * building). Each step once a town, remembered with the world: the core says a ruin at every look. The charges go off
 * at once, not as lit TNT: a ruin's chunks are loaded for it but don't tick entities, so a lit charge there never
 * went off (seed 1717's first war, 16 lit round Town 0's bell, its houses all standing).
 */
public final class Ruins extends SavedData {
    private static final String NAME = "enterprise_ruins";
    /** Blocks round the bell a ruin's buildings are looked for in. */
    public static final int REACH = 64;
    /** Points of interest this near each other are one building's. */
    static final int ONE_BUILDING = 6;
    /** A field's water: still water this near farmland, a block deep (its channels: no river, no sea). */
    static final int WATERS = 4;
    /** A charge's power: TNT's. */
    static final float CHARGE = 4.0F;

    /** Towns whose fields' water is gone, and those blasted too (laid waste). */
    private final Set<Long> drained = new LinkedHashSet<>();
    private final Set<Long> blasted = new LinkedHashSet<>();

    public static Ruins get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Ruins::new, Ruins::load, null), NAME);
    }

    public boolean laidWaste(long town) {
        return blasted.contains(town);
    }

    public boolean drained(long town) {
        return drained.contains(town);
    }

    /** The chunks the land's hexes lie in (each by its middle's hex, as {@link Terrain} counts it). */
    public static List<ChunkPos> chunks(List<Hex> hexes) {
        Set<Hex> land = new HashSet<>(hexes);
        Set<ChunkPos> out = new LinkedHashSet<>();
        int reach = (int) Math.ceil(Hex.SIZE) + 16;
        for (Hex h : hexes) {
            int cx = (int) Math.round(h.centerX()), cz = (int) Math.round(h.centerZ());
            for (int x = cx - reach; x <= cx + reach; x += 16) {
                for (int z = cz - reach; z <= cz + reach; z += 16) {
                    ChunkPos c = new ChunkPos(new BlockPos(x, 0, z));
                    if (land.contains(Hex.at(c.getMiddleBlockX() + 0.5, c.getMiddleBlockZ() + 0.5))) {
                        out.add(c);
                    }
                }
            }
        }
        return new ArrayList<>(out);
    }

    /** The chunks round a bell its buildings stand in. */
    public static List<ChunkPos> round(BlockPos bell) {
        List<ChunkPos> out = new ArrayList<>();
        ChunkPos c = new ChunkPos(bell);
        int n = REACH / 16 + 1;
        for (int dx = -n; dx <= n; dx++) {
            for (int dz = -n; dz <= n; dz++) {
                out.add(new ChunkPos(c.x + dx, c.z + dz));
            }
        }
        return out;
    }

    /** The fields' water filled in, in these chunks (loaded): the number of blocks filled. */
    public static int drain(ServerLevel level, List<ChunkPos> chunks) {
        int filled = 0;
        for (ChunkPos c : chunks) {
            filled += drain(level, c.getMinBlockX(), c.getMinBlockZ(), c.getMaxBlockX(), c.getMaxBlockZ());
        }
        return filled;
    }

    /** The fields' water filled in, in a box of columns (loaded): the number of blocks filled. */
    public static int drain(ServerLevel level, int x0, int z0, int x1, int z1) {
        int filled = 0;
        for (int x = x0; x <= x1; x++) {
            for (int z = z0; z <= z1; z++) {
                // (Its ground, under any canopy: crops don't block, a roof or a ledge over a field a few blocks up may.)
                int top = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z);
                for (int y = top - 1; y >= top - 8; y--) {
                    BlockPos p = new BlockPos(x, y, z);
                    if (level.getBlockState(p).is(Blocks.FARMLAND)) {
                        filled += drainNear(level, p);
                        break;
                    }
                }
            }
        }
        return filled;
    }

    /**
     * The still water watering the farmland at {@code field} (as the game reckons it: within 4, level or one up) filled
     * where it's a block deep: a field's channels run long and join (seed 1717's ran past 40 blocks), but they're
     * shallow; a river or the sea is deeper.
     */
    private static int drainNear(ServerLevel level, BlockPos field) {
        int filled = 0;
        for (BlockPos p : BlockPos.betweenClosed(field.offset(-WATERS, 0, -WATERS), field.offset(WATERS, 1, WATERS))) {
            if (level.getFluidState(p).is(Fluids.WATER) && level.getFluidState(p.below()).isEmpty()) {
                level.setBlock(p, Blocks.DIRT.defaultBlockState(), 3);
                filled++;
            }
        }
        return filled;
    }

    /**
     * The hearts of the buildings within {@code reach} of a bell (loaded): its points of interest (beds, workstations, the bell), those
     * within {@link #ONE_BUILDING} of each other one building's, each building's at their middle.
     */
    public static List<BlockPos> hearts(ServerLevel level, BlockPos bell, int reach) {
        List<BlockPos> points = level.getPoiManager().getInRange(t -> true, bell, reach, PoiManager.Occupancy.ANY)
                .map(r -> r.getPos().immutable()).sorted(java.util.Comparator.comparingLong(BlockPos::asLong)).toList();
        List<List<BlockPos>> buildings = new ArrayList<>();
        for (BlockPos p : points) {
            List<BlockPos> into = null;
            for (List<BlockPos> b : buildings) {
                if (b.stream().anyMatch(q -> q.distManhattan(p) <= ONE_BUILDING)) {
                    into = b;
                    break;
                }
            }
            if (into == null) {
                buildings.add(into = new ArrayList<>());
            }
            into.add(p);
        }
        List<BlockPos> out = new ArrayList<>();
        for (List<BlockPos> b : buildings) {
            out.add(new BlockPos((int) Math.round(b.stream().mapToInt(BlockPos::getX).average().orElse(0)),
                    (int) Math.round(b.stream().mapToInt(BlockPos::getY).average().orElse(0)),
                    (int) Math.round(b.stream().mapToInt(BlockPos::getZ).average().orElse(0))));
        }
        return out;
    }

    /** A charge set off at each heart: the charges. */
    public static int blast(ServerLevel level, List<BlockPos> hearts) {
        for (BlockPos h : hearts) {
            level.explode(null, h.getX() + 0.5, h.getY() + 0.5, h.getZ() + 0.5, CHARGE, Level.ExplosionInteraction.TNT);
        }
        return hearts.size();
    }

    public void drained(long town, boolean yes) {
        if (yes && drained.add(town)) {
            setDirty();
        }
    }

    public void blasted(long town) {
        if (blasted.add(town)) {
            setDirty();
        }
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        tag.put("drained", new LongArrayTag(drained.stream().mapToLong(Long::longValue).toArray()));
        tag.put("blasted", new LongArrayTag(blasted.stream().mapToLong(Long::longValue).toArray()));
        return tag;
    }

    public static Ruins load(CompoundTag tag, HolderLookup.Provider registries) {
        Ruins out = new Ruins();
        for (long t : tag.getLongArray("drained")) {
            out.drained.add(t);
        }
        for (long t : tag.getLongArray("blasted")) {
            out.blasted.add(t);
        }
        return out;
    }
}
