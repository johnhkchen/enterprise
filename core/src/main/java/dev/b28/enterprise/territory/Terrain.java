package dev.b28.enterprise.territory;

import dev.b28.enterprise.hex.Hex;
import java.util.HashMap;
import java.util.Map;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.QuartPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.Tag;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.BiomeTags;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.saveddata.SavedData;
import net.neoforged.neoforge.event.level.ChunkEvent;

/**
 * Where caravans can't go: open water, and steep ground. The ground's relief in a hex (its highest surface less its
 * lowest) makes it harder going: a caravan climbs, or picks its way round, and a cliff stops it.
 * <p>
 * Water: Each chunk is surveyed when it loads and when it unloads (16 columns, is the
 * surface water?), and the counts are kept; a chunk never loaded is guessed from its biome (ocean is water, a river is
 * half water). A hex mostly under water is impassable, so a lake or an arm of the sea makes the land around it a
 * chokepoint. Only counts that differ from the biome's guess are stored.
 */
public final class Terrain extends SavedData {
    private static final String NAME = "enterprise_terrain";

    /** Columns surveyed per chunk: a 4 x 4 grid. */
    public static final int SAMPLES = 16;

    /** A hex more than this share under water can't be crossed by a caravan. */
    public static final double IMPASSABLE = 0.5;
    /** Every this many blocks of relief in a hex make it as hard again to cross as open ground. */
    public static final double RELIEF_PER_FACTOR = 8;
    /** A hex with this much relief or more (a cliff, a mountain's flank) can't be crossed by a caravan. */
    public static final int CLIFF = 32;

    private final Map<ResourceKey<Level>, Map<Long, Integer>> water = new HashMap<>();
    // dimension -> chunk -> its surface's lowest and highest survey heights, packed; only chunks with relief
    private final Map<ResourceKey<Level>, Map<Long, Integer>> heights = new HashMap<>();
    // Biome guesses for chunks never surveyed (deterministic, so not saved).
    private final Map<ResourceKey<Level>, Map<Long, Integer>> guesses = new HashMap<>();

    public static Terrain get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Terrain::new, Terrain::load, null), NAME);
    }

    /** Surveys a chunk as it loads or unloads, on the server thread. */
    public static void onChunk(ChunkEvent event) {
        if (event.getLevel() instanceof ServerLevel level && level.getServer().isSameThread()) {
            Terrain terrain = get(level.getServer());
            long chunk = event.getChunk().getPos().toLong();
            terrain.record(level, chunk, survey(event.getChunk()));
            terrain.recordHeights(level, chunk, surveyHeights(event.getChunk()));
        }
    }

    /** Surface columns of the chunk's 4 x 4 survey grid that are water (or any fluid). */
    public static int survey(ChunkAccess chunk) {
        int wet = 0;
        for (int x = 2; x < 16; x += 4) {
            for (int z = 2; z < 16; z += 4) {
                int top = chunk.getHeight(Heightmap.Types.MOTION_BLOCKING, x, z); // the surface block's y
                var pos = chunk.getPos().getBlockAt(x, top, z);
                if (!chunk.getFluidState(pos).isEmpty()) {
                    wet++;
                }
            }
        }
        return wet;
    }

    /** The lowest and highest surface of the chunk's 4 x 4 survey grid. */
    public static int[] surveyHeights(ChunkAccess chunk) {
        int low = Integer.MAX_VALUE;
        int high = Integer.MIN_VALUE;
        for (int x = 2; x < 16; x += 4) {
            for (int z = 2; z < 16; z += 4) {
                int top = chunk.getHeight(Heightmap.Types.MOTION_BLOCKING, x, z);
                low = Math.min(low, top);
                high = Math.max(high, top);
            }
        }
        return new int[] {low, high};
    }

    /** A chunk's surveyed lowest and highest surface (a flat one too: it marks its hex's ground level). */
    public void recordHeights(ServerLevel level, long chunk, int[] lowHigh) {
        Integer packed = ((lowHigh[0] + 512) << 16) | (lowHigh[1] + 512);
        if (!packed.equals(heights.computeIfAbsent(level.dimension(), d -> new HashMap<>()).put(chunk, packed))) {
            setDirty();
        }
    }

    /** The ground's relief in the hex: its highest surveyed surface less its lowest (0 where none is surveyed). */
    public int relief(ServerLevel level, Hex hex) {
        Map<Long, Integer> surveyed = heights.getOrDefault(level.dimension(), Map.of());
        int low = Integer.MAX_VALUE;
        int high = Integer.MIN_VALUE;
        for (long chunk : chunksOf(hex)) {
            Integer packed = surveyed.get(chunk);
            if (packed != null) {
                low = Math.min(low, (packed >>> 16) - 512);
                high = Math.max(high, (packed & 0xFFFF) - 512);
            }
        }
        return high < low ? 0 : high - low;
    }

    /** A chunk's surveyed water count; stored only where it differs from the biome's guess. */
    public void record(ServerLevel level, long chunk, int wet) {
        Map<Long, Integer> surveyed = water.computeIfAbsent(level.dimension(), d -> new HashMap<>());
        Integer before = wet == guess(level, chunk) ? surveyed.remove(chunk) : surveyed.put(chunk, wet);
        if (before == null || before != wet) {
            setDirty();
        }
    }

    /** Share of the survey columns under water in the chunks whose middle lies in the hex, 0 to 1. */
    public double waterShare(ServerLevel level, Hex hex) {
        Map<Long, Integer> surveyed = water.getOrDefault(level.dimension(), Map.of());
        int wet = 0;
        int columns = 0;
        for (long chunk : chunksOf(hex)) {
            Integer counted = surveyed.get(chunk);
            wet += counted != null ? counted : guess(level, chunk);
            columns += SAMPLES;
        }
        return columns == 0 ? 0 : (double) wet / columns;
    }

    /**
     * How much harder a hex is to cross than open ground: 1 on dry land, up to 2 with water to skirt, infinite when
     * mostly under water.
     */
    public double crossingFactor(ServerLevel level, Hex hex) {
        double share = waterShare(level, hex);
        int relief = relief(level, hex);
        if (share > IMPASSABLE || relief >= CLIFF) {
            return Double.POSITIVE_INFINITY;
        }
        return (1 + 2 * share) * (1 + relief / RELIEF_PER_FACTOR);
    }

    /** The chunks whose middle lies in the hex: each chunk counts toward one hex only. */
    public static java.util.List<Long> chunksOf(Hex hex) {
        return Crops.chunksOf(hex).stream()
                .filter(chunk -> {
                    ChunkPos pos = new ChunkPos(chunk);
                    return Hex.at(pos.getMiddleBlockX() + 0.5, pos.getMiddleBlockZ() + 0.5).equals(hex);
                })
                .toList();
    }

    private int guess(ServerLevel level, long chunk) {
        return guesses.computeIfAbsent(level.dimension(), d -> new HashMap<>()).computeIfAbsent(chunk, c -> {
            ChunkPos pos = new ChunkPos(c);
            var source = level.getChunkSource();
            var biome = source.getGenerator().getBiomeSource().getNoiseBiome(
                    QuartPos.fromBlock(pos.getMiddleBlockX()), QuartPos.fromBlock(level.getSeaLevel()),
                    QuartPos.fromBlock(pos.getMiddleBlockZ()), source.randomState().sampler());
            if (biome.is(BiomeTags.IS_OCEAN) || biome.is(BiomeTags.IS_DEEP_OCEAN)) {
                return SAMPLES;
            }
            return biome.is(BiomeTags.IS_RIVER) ? SAMPLES / 2 : 0;
        });
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag dimensions = new ListTag();
        water.forEach((dimension, chunks) -> {
            CompoundTag entry = new CompoundTag();
            entry.putString("dimension", dimension.location().toString());
            long[] positions = new long[chunks.size()];
            byte[] counts = new byte[chunks.size()];
            int i = 0;
            for (Map.Entry<Long, Integer> chunk : chunks.entrySet()) {
                positions[i] = chunk.getKey();
                counts[i++] = (byte) (int) chunk.getValue();
            }
            entry.putLongArray("chunks", positions);
            entry.putByteArray("water", counts);
            dimensions.add(entry);
        });
        tag.put("dimensions", dimensions);
        ListTag ground = new ListTag();
        heights.forEach((dimension, chunks) -> {
            CompoundTag entry = new CompoundTag();
            entry.putString("dimension", dimension.location().toString());
            entry.putLongArray("chunks", chunks.keySet().stream().mapToLong(Long::longValue).toArray());
            entry.putIntArray("heights", chunks.values().stream().mapToInt(Integer::intValue).toArray());
            ground.add(entry);
        });
        tag.put("heights", ground);
        return tag;
    }

    public static Terrain load(CompoundTag tag, HolderLookup.Provider registries) {
        Terrain terrain = new Terrain();
        for (Tag t : tag.getList("dimensions", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            ResourceKey<Level> dimension =
                    ResourceKey.create(Registries.DIMENSION, ResourceLocation.parse(entry.getString("dimension")));
            long[] positions = entry.getLongArray("chunks");
            byte[] counts = entry.getByteArray("water");
            Map<Long, Integer> chunks = terrain.water.computeIfAbsent(dimension, d -> new HashMap<>());
            for (int i = 0; i < Math.min(positions.length, counts.length); i++) {
                chunks.put(positions[i], (int) counts[i]);
            }
        }
        for (Tag t : tag.getList("heights", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            ResourceKey<Level> dimension =
                    ResourceKey.create(Registries.DIMENSION, ResourceLocation.parse(entry.getString("dimension")));
            long[] positions = entry.getLongArray("chunks");
            int[] packed = entry.getIntArray("heights");
            Map<Long, Integer> chunks = terrain.heights.computeIfAbsent(dimension, d -> new HashMap<>());
            for (int i = 0; i < Math.min(positions.length, packed.length); i++) {
                chunks.put(positions[i], packed[i]);
            }
        }
        return terrain;
    }
}
