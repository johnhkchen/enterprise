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
 * Where caravans can't go: open water, steep ground, and built walls with no way through. The ground's relief in a hex (its highest surface less its
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
    // dimension -> chunk -> its columns a built wall stands in (256 bits, x + 16 z); only chunks with any
    private final Map<ResourceKey<Level>, Map<Long, long[]>> walls = new HashMap<>();
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
            terrain.recordWalls(level, chunk, surveyWalls(event.getChunk()));
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

    /**
     * The chunk's columns a built wall stands in (bit {@code x + 16 z}), or null for none: a column whose surface is
     * built, not ground as the world made it ({@code Grounds.natural}), at least two blocks of it (a fence or a wall
     * block counts two: nobody steps over one). A door, gate or trapdoor on top is a way through, not a wall. Houses
     * are walls too: a wagon goes round them.
     */
    public static long[] surveyWalls(ChunkAccess chunk) {
        long[] bits = null;
        var pos = new net.minecraft.core.BlockPos.MutableBlockPos();
        for (int x = 0; x < 16; x++) {
            for (int z = 0; z < 16; z++) {
                int top = chunk.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z);
                if (wall(chunk, pos, chunk.getPos().getBlockX(x), top, chunk.getPos().getBlockZ(z))) {
                    if (bits == null) {
                        bits = new long[4];
                    }
                    int i = x + 16 * z;
                    bits[i >> 6] |= 1L << (i & 63);
                }
            }
        }
        return bits;
    }

    private static boolean wall(ChunkAccess chunk, net.minecraft.core.BlockPos.MutableBlockPos pos, int x, int top, int z) {
        int built = 0;
        for (int y = top; y > top - 2 && built < 2; y--) {
            var state = chunk.getBlockState(pos.set(x, y, z));
            if (state.getBlock() instanceof net.minecraft.world.level.block.DoorBlock
                    || state.getBlock() instanceof net.minecraft.world.level.block.FenceGateBlock
                    || state.getBlock() instanceof net.minecraft.world.level.block.TrapDoorBlock) {
                return false; // a way through
            }
            if (!state.blocksMotion() || !chunk.getFluidState(pos).isEmpty()
                    || dev.b28.enterprise.settlement.Grounds.natural(state)) {
                break;
            }
            built += state.is(net.minecraft.tags.BlockTags.FENCES) || state.is(net.minecraft.tags.BlockTags.WALLS) ? 2 : 1;
        }
        return built >= 2;
    }

    /** A chunk's surveyed walls ({@link #surveyWalls}; null for none). */
    public void recordWalls(ServerLevel level, long chunk, long[] bits) {
        Map<Long, long[]> surveyed = walls.computeIfAbsent(level.dimension(), d -> new HashMap<>());
        long[] before = bits == null ? surveyed.remove(chunk) : surveyed.put(chunk, bits);
        if (!java.util.Arrays.equals(before, bits)) {
            setDirty();
        }
    }

    /** Open columns on the edge a neighbour must have, and a stretch of the edge must touch it by, to count. */
    public static final int WALLED_TOUCH = 8;

    /**
     * Whether built walls shut the hex: its ground (the chunks counted as its) flooded from each open column into
     * stretches of open ground, and two stretches, each the only way into some neighbouring hex. A wall across it with
     * no gap leaves one stretch the way west and another the way east (the neighbours it runs on into are touched by
     * both, and decide nothing); one inside its edge leaves a strip the only way to the hex beyond; a gap in the wall (a
     * gateway, a door, a fence gate) makes them one stretch again. A walled yard or a house on the edge touches a
     * neighbour the rest of the hex touches too: the hex is still open. Water is open here: it's the water share's to
     * judge.
     */
    public boolean walledShut(ServerLevel level, Hex hex) {
        Map<Long, long[]> surveyed = walls.getOrDefault(level.dimension(), Map.of());
        java.util.List<Long> chunks = chunksOf(hex);
        if (chunks.stream().noneMatch(surveyed::containsKey)) {
            return false;
        }
        int minX = Integer.MAX_VALUE, minZ = Integer.MAX_VALUE, maxX = Integer.MIN_VALUE, maxZ = Integer.MIN_VALUE;
        for (long c : chunks) {
            minX = Math.min(minX, ChunkPos.getX(c));
            maxX = Math.max(maxX, ChunkPos.getX(c));
            minZ = Math.min(minZ, ChunkPos.getZ(c));
            maxZ = Math.max(maxZ, ChunkPos.getZ(c));
        }
        int w = (maxX - minX + 1) * 16, h = (maxZ - minZ + 1) * 16;
        // 0: not the hex's; 1: open; 2: wall
        byte[] cell = new byte[w * h];
        for (long c : chunks) {
            long[] bits = surveyed.get(c);
            int ox = (ChunkPos.getX(c) - minX) * 16, oz = (ChunkPos.getZ(c) - minZ) * 16;
            for (int i = 0; i < 256; i++) {
                boolean walled = bits != null && (bits[i >> 6] & (1L << (i & 63))) != 0;
                cell[(ox + (i & 15)) + w * (oz + (i >> 4))] = (byte) (walled ? 2 : 1);
            }
        }
        int baseX = minX * 16, baseZ = minZ * 16;
        Map<Long, Hex> beyond = new HashMap<>(); // the hex of each chunk outside this one's, by chunk
        // Each stretch of open ground's touches on each neighbour, and every neighbour's in all.
        java.util.List<Map<Hex, Integer>> touches = new java.util.ArrayList<>();
        Map<Hex, Integer> total = new HashMap<>();
        boolean[] seen = new boolean[w * h];
        int[] queue = new int[w * h];
        for (int start = 0; start < w * h; start++) {
            if (cell[start] != 1 || seen[start]) {
                continue;
            }
            Map<Hex, Integer> touched = new HashMap<>();
            int head = 0, tail = 0;
            queue[tail++] = start;
            seen[start] = true;
            while (head < tail) {
                int at = queue[head++];
                int x = at % w, z = at / w;
                for (int d = 0; d < 4; d++) {
                    int nx = x + (d == 0 ? 1 : d == 1 ? -1 : 0), nz = z + (d == 2 ? 1 : d == 3 ? -1 : 0);
                    if (nx < 0 || nz < 0 || nx >= w || nz >= h || cell[nx + w * nz] == 0) {
                        long chunk = ChunkPos.asLong((baseX + nx) >> 4, (baseZ + nz) >> 4);
                        Hex next = beyond.computeIfAbsent(chunk, c -> {
                            ChunkPos pos = new ChunkPos(c);
                            return Hex.at(pos.getMiddleBlockX() + 0.5, pos.getMiddleBlockZ() + 0.5);
                        });
                        touched.merge(next, 1, Integer::sum);
                        total.merge(next, 1, Integer::sum);
                        continue;
                    }
                    int n = nx + w * nz;
                    if (cell[n] == 1 && !seen[n]) {
                        seen[n] = true;
                        queue[tail++] = n;
                    }
                }
            }
            touches.add(touched);
        }
        // A stretch of open ground is the way to a neighbour when it's the only one touching it (or touches it most,
        // when none touches it by WALLED_TOUCH). Two such ways that aren't one stretch: the hex is shut.
        java.util.List<Hex> ways = total.entrySet().stream().filter(e -> e.getValue() >= WALLED_TOUCH)
                .map(Map.Entry::getKey).toList();
        if (ways.isEmpty()) {
            return true; // walled in all round
        }
        java.util.Set<Integer> only = new java.util.HashSet<>();
        for (Hex way : ways) {
            int touching = 0, last = -1, most = -1;
            for (int i = 0; i < touches.size(); i++) {
                int n = touches.get(i).getOrDefault(way, 0);
                if (most < 0 || n > touches.get(most).getOrDefault(way, 0)) {
                    most = i;
                }
                if (n >= WALLED_TOUCH) {
                    touching++;
                    last = i;
                }
            }
            if (touching <= 1) {
                only.add(touching == 1 ? last : most);
            }
        }
        return only.size() > 1;
    }

    /** A chunk's surveyed lowest and highest surface (a flat one too: it marks its hex's ground level). */
    public void recordHeights(ServerLevel level, long chunk, int[] lowHigh) {
        Integer packed = ((lowHigh[0] + 512) << 16) | (lowHigh[1] + 512);
        if (!packed.equals(heights.computeIfAbsent(level.dimension(), d -> new HashMap<>()).put(chunk, packed))) {
            setDirty();
        }
    }

    /** Has any of the hex's ground been surveyed (so its relief is known, not guessed flat)? */
    public boolean surveyed(ServerLevel level, Hex hex) {
        Map<Long, Integer> surveyed = heights.getOrDefault(level.dimension(), Map.of());
        return chunksOf(hex).stream().anyMatch(surveyed::containsKey);
    }

    /**
     * The ground's relief in the hex: its highest surveyed surface less its lowest (0 where none is surveyed). A hex
     * built walls shut ({@link #walledShut}) reads as a cliff: no way across for a wagon.
     */
    public int relief(ServerLevel level, Hex hex) {
        int ground = groundRelief(level, hex);
        return ground < CLIFF && walledShut(level, hex) ? CLIFF : ground;
    }

    private int groundRelief(ServerLevel level, Hex hex) {
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

    /**
     * The height of the hex's ground: the median of its surveyed chunks' lowest surfaces (trees, houses and walls stand
     * above a chunk's lowest; a ravine is one chunk of many), or none where none is surveyed. The core's wagons climb
     * from hex to hex by it ({@code map.rs}).
     */
    public java.util.Optional<Double> height(ServerLevel level, Hex hex) {
        Map<Long, Integer> surveyed = heights.getOrDefault(level.dimension(), Map.of());
        int[] lows = chunksOf(hex).stream().map(surveyed::get).filter(java.util.Objects::nonNull)
                .mapToInt(packed -> (packed >>> 16) - 512).sorted().toArray();
        if (lows.length == 0) {
            return java.util.Optional.empty();
        }
        int n = lows.length;
        return java.util.Optional.of(n % 2 == 1 ? lows[n / 2] : (lows[n / 2 - 1] + lows[n / 2]) / 2.0);
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
        ListTag built = new ListTag();
        walls.forEach((dimension, chunks) -> {
            CompoundTag entry = new CompoundTag();
            entry.putString("dimension", dimension.location().toString());
            entry.putLongArray("chunks", chunks.keySet().stream().mapToLong(Long::longValue).toArray());
            entry.putLongArray("bits", chunks.values().stream().flatMapToLong(java.util.Arrays::stream).toArray());
            built.add(entry);
        });
        tag.put("walls", built);
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
        for (Tag t : tag.getList("walls", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            ResourceKey<Level> dimension =
                    ResourceKey.create(Registries.DIMENSION, ResourceLocation.parse(entry.getString("dimension")));
            long[] positions = entry.getLongArray("chunks");
            long[] bits = entry.getLongArray("bits");
            Map<Long, long[]> chunks = terrain.walls.computeIfAbsent(dimension, d -> new HashMap<>());
            for (int i = 0; i < positions.length && 4 * i + 4 <= bits.length; i++) {
                chunks.put(positions[i], java.util.Arrays.copyOfRange(bits, 4 * i, 4 * i + 4));
            }
        }
        return terrain;
    }
}
