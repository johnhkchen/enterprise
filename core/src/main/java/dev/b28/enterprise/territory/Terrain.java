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
 * <p>
 * Rivers: which of those 16 columns are water is kept too (a chunk never loaded guessed from its biome at each, a
 * river's or an ocean's being water), and a hex whose water parts its land into sides that each alone lead to some
 * neighbour is cut by a river ({@link #river}): the core's wagons cross it only by a bridge, its llamas ford it.
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
    // dimension -> chunk -> which of its 16 survey columns are water (bit x / 4 + 4 (z / 4)); only where not the guess
    private final Map<ResourceKey<Level>, Map<Long, Integer>> wet = new HashMap<>();
    // Wet-column guesses for chunks never surveyed, from the biome at each column (deterministic, not saved).
    private final Map<ResourceKey<Level>, Map<Long, Integer>> wetGuesses = new HashMap<>();
    // Which survey columns stand in an ocean's biome, by chunk (deterministic, not saved).
    private final Map<ResourceKey<Level>, Map<Long, Integer>> seaGuesses = new HashMap<>();
    // Height guesses for chunks never surveyed: the generator's surface at the chunk's middle (deterministic, not saved).
    private final Map<ResourceKey<Level>, Map<Long, Integer>> heightGuesses = new HashMap<>();
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
            terrain.recordWet(level, chunk, surveyWet(event.getChunk()));
        }
    }

    /** Which surface columns of the chunk's 4 x 4 survey grid are water (or any fluid): bit {@code x / 4 + 4 (z / 4)}. */
    public static int surveyWet(ChunkAccess chunk) {
        int bits = 0;
        for (int x = 2; x < 16; x += 4) {
            for (int z = 2; z < 16; z += 4) {
                int top = chunk.getHeight(Heightmap.Types.MOTION_BLOCKING, x, z);
                if (!chunk.getFluidState(chunk.getPos().getBlockAt(x, top, z)).isEmpty()) {
                    bits |= 1 << (x / 4 + 4 * (z / 4));
                }
            }
        }
        return bits;
    }

    /** A chunk's surveyed wet columns ({@link #surveyWet}); stored only where they differ from the biomes' guess. */
    public void recordWet(ServerLevel level, long chunk, int bits) {
        Map<Long, Integer> surveyed = wet.computeIfAbsent(level.dimension(), d -> new HashMap<>());
        Integer before = bits == wetGuess(level, chunk) ? surveyed.remove(chunk) : surveyed.put(chunk, bits);
        if (before == null ? surveyed.containsKey(chunk) : before != bits) {
            setDirty();
        }
    }

    /** Open survey cells on the edge a neighbour must have, and a side of the river touch it by, to count. */
    public static final int RIVER_TOUCH = 2;

    /**
     * Whether a river cuts the hex: its survey cells (4 x 4 blocks each, a chunk's 16 survey columns) that are dry
     * flooded into stretches of land, and two stretches, each the only way into some neighbouring hex
     * ({@link #walledShut}'s reckoning, the water standing for the wall). A river along the hex's edge, a pond or a
     * lake inside it leave one side: no cut. A hex mostly water isn't a river's: it's impassable as it is. The sea isn't
     * a river: water in an ocean's biome is open here, so a coast, a bay or a headland cuts nothing (a river's mouth
     * neither).
     */
    public boolean river(ServerLevel level, Hex hex) {
        if (waterShare(level, hex) > IMPASSABLE) {
            return false;
        }
        Map<Long, Integer> surveyed = wet.getOrDefault(level.dimension(), Map.of());
        java.util.List<Long> chunks = chunksOf(hex);
        int[] box = box(chunks);
        int w = (box[2] - box[0] + 1) * 4, h = (box[3] - box[1] + 1) * 4;
        byte[] cell = new byte[w * h];
        boolean any = false;
        for (long c : chunks) {
            Integer counted = surveyed.get(c);
            int bits = (counted != null ? counted : wetGuess(level, c)) & ~sea(level, c);
            any |= bits != 0;
            int ox = (ChunkPos.getX(c) - box[0]) * 4, oz = (ChunkPos.getZ(c) - box[1]) * 4;
            for (int i = 0; i < 16; i++) {
                cell[(ox + (i & 3)) + w * (oz + (i >> 2))] = (byte) ((bits & (1 << i)) != 0 ? 2 : 1);
            }
        }
        return any && ways(cell, w, h, box[0] * 16, box[1] * 16, 4, RIVER_TOUCH) > 1;
    }

    /** The chunks' least and greatest x and z: {minX, minZ, maxX, maxZ}. */
    private static int[] box(java.util.List<Long> chunks) {
        int minX = Integer.MAX_VALUE, minZ = Integer.MAX_VALUE, maxX = Integer.MIN_VALUE, maxZ = Integer.MIN_VALUE;
        for (long c : chunks) {
            minX = Math.min(minX, ChunkPos.getX(c));
            maxX = Math.max(maxX, ChunkPos.getX(c));
            minZ = Math.min(minZ, ChunkPos.getZ(c));
            maxZ = Math.max(maxZ, ChunkPos.getZ(c));
        }
        return new int[] {minX, minZ, maxX, maxZ};
    }

    /** Which of a chunk's survey columns stand in an ocean's biome (the generator's, so the same loaded or not). */
    private int sea(ServerLevel level, long chunk) {
        return seaGuesses.computeIfAbsent(level.dimension(), d -> new HashMap<>()).computeIfAbsent(chunk, c -> {
            ChunkPos pos = new ChunkPos(c);
            var source = level.getChunkSource();
            int bits = 0;
            for (int x = 2; x < 16; x += 4) {
                for (int z = 2; z < 16; z += 4) {
                    var biome = source.getGenerator().getBiomeSource().getNoiseBiome(
                            QuartPos.fromBlock(pos.getBlockX(x)), QuartPos.fromBlock(level.getSeaLevel()),
                            QuartPos.fromBlock(pos.getBlockZ(z)), source.randomState().sampler());
                    if (biome.is(BiomeTags.IS_OCEAN) || biome.is(BiomeTags.IS_DEEP_OCEAN)) {
                        bits |= 1 << (x / 4 + 4 * (z / 4));
                    }
                }
            }
            return bits;
        });
    }

    /** A chunk never surveyed: which of its survey columns stand in a river's or an ocean's biome. */
    private int wetGuess(ServerLevel level, long chunk) {
        return wetGuesses.computeIfAbsent(level.dimension(), d -> new HashMap<>()).computeIfAbsent(chunk, c -> {
            ChunkPos pos = new ChunkPos(c);
            var source = level.getChunkSource();
            int bits = 0;
            for (int x = 2; x < 16; x += 4) {
                for (int z = 2; z < 16; z += 4) {
                    var biome = source.getGenerator().getBiomeSource().getNoiseBiome(
                            QuartPos.fromBlock(pos.getBlockX(x)), QuartPos.fromBlock(level.getSeaLevel()),
                            QuartPos.fromBlock(pos.getBlockZ(z)), source.randomState().sampler());
                    if (biome.is(BiomeTags.IS_RIVER) || biome.is(BiomeTags.IS_OCEAN) || biome.is(BiomeTags.IS_DEEP_OCEAN)) {
                        bits |= 1 << (x / 4 + 4 * (z / 4));
                    }
                }
            }
            return bits;
        });
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
        int[] box = box(chunks);
        int w = (box[2] - box[0] + 1) * 16, h = (box[3] - box[1] + 1) * 16;
        // 0: not the hex's; 1: open; 2: wall
        byte[] cell = new byte[w * h];
        for (long c : chunks) {
            long[] bits = surveyed.get(c);
            int ox = (ChunkPos.getX(c) - box[0]) * 16, oz = (ChunkPos.getZ(c) - box[1]) * 16;
            for (int i = 0; i < 256; i++) {
                boolean walled = bits != null && (bits[i >> 6] & (1L << (i & 63))) != 0;
                cell[(ox + (i & 15)) + w * (oz + (i >> 4))] = (byte) (walled ? 2 : 1);
            }
        }
        int ways = ways(cell, w, h, box[0] * 16, box[1] * 16, 1, WALLED_TOUCH);
        return ways < 0 || ways > 1;
    }

    /**
     * A hex's cells ({@code cell}: 0 not the hex's, 1 open, 2 shut; {@code w} by {@code h}, each {@code scale} blocks
     * a side, the first at block {@code baseX, baseZ}) flooded from each open cell into stretches, and how many are the
     * way into some neighbouring hex: the only stretch touching it by {@code touch} cells (or the one touching it most,
     * when none does by that many). -1 when no neighbour is touched by that many: shut in all round.
     */
    static int ways(byte[] cell, int w, int h, int baseX, int baseZ, int scale, int touch) {
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
                        long chunk = ChunkPos.asLong((baseX + nx * scale) >> 4, (baseZ + nz * scale) >> 4);
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
        // when none touches it by `touch`). Two such ways that aren't one stretch: the hex is parted.
        java.util.List<Hex> ways = total.entrySet().stream().filter(e -> e.getValue() >= touch)
                .map(Map.Entry::getKey).toList();
        if (ways.isEmpty()) {
            return -1;
        }
        java.util.Set<Integer> only = new java.util.HashSet<>();
        for (Hex way : ways) {
            int touching = 0, last = -1, most = -1;
            for (int i = 0; i < touches.size(); i++) {
                int n = touches.get(i).getOrDefault(way, 0);
                if (most < 0 || n > touches.get(most).getOrDefault(way, 0)) {
                    most = i;
                }
                if (n >= touch) {
                    touching++;
                    last = i;
                }
            }
            if (touching <= 1) {
                only.add(touching == 1 ? last : most);
            }
        }
        return only.size();
    }

    /** A chunk's surveyed lowest and highest surface (a flat one too: it marks its hex's ground level). */
    public void recordHeights(ServerLevel level, long chunk, int[] lowHigh) {
        Integer packed = ((lowHigh[0] + 512) << 16) | (lowHigh[1] + 512);
        if (!packed.equals(heights.computeIfAbsent(level.dimension(), d -> new HashMap<>()).put(chunk, packed))) {
            setDirty();
        }
    }

    /** Has all of the hex's ground been surveyed (every chunk whose middle lies in it)? */
    public boolean explored(ServerLevel level, Hex hex) {
        Map<Long, Integer> surveyed = heights.getOrDefault(level.dimension(), Map.of());
        return chunksOf(hex).stream().allMatch(surveyed::containsKey);
    }

    /** Every hex any of whose ground has been surveyed: the world's known land (`/enterprise core ground`). */
    public java.util.SortedSet<Hex> surveyedHexes(ServerLevel level) {
        java.util.SortedSet<Hex> out = new java.util.TreeSet<>(java.util.Comparator.comparingInt(Hex::q).thenComparingInt(Hex::r));
        for (long c : heights.getOrDefault(level.dimension(), Map.of()).keySet()) {
            ChunkPos pos = new ChunkPos(c);
            out.add(Hex.at(pos.getMiddleBlockX() + 0.5, pos.getMiddleBlockZ() + 0.5));
        }
        return out;
    }

    /** Has any of the hex's ground been surveyed (so its relief is known, not guessed flat)? */
    public boolean surveyed(ServerLevel level, Hex hex) {
        Map<Long, Integer> surveyed = heights.getOrDefault(level.dimension(), Map.of());
        return chunksOf(hex).stream().anyMatch(surveyed::containsKey);
    }

    /**
     * The ground's relief in the hex: its highest surface less its lowest (a chunk never surveyed guessed). A hex
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
            } else {
                int guess = heightGuess(level, chunk);
                low = Math.min(low, guess);
                high = Math.max(high, guess);
            }
        }
        return high < low ? 0 : high - low;
    }

    /**
     * A chunk never surveyed: the height of its surface at its middle, as the world's generator would make it (from its
     * noise, without generating the chunk). What players built there since isn't in it; the survey replaces it once
     * the chunk loads.
     */
    private int heightGuess(ServerLevel level, long chunk) {
        return heightGuesses.computeIfAbsent(level.dimension(), d -> new HashMap<>()).computeIfAbsent(chunk, c -> {
            ChunkPos pos = new ChunkPos(c);
            var source = level.getChunkSource();
            return source.getGenerator().getBaseHeight(pos.getMiddleBlockX(), pos.getMiddleBlockZ(),
                    Heightmap.Types.WORLD_SURFACE_WG, level, source.randomState()) - 1;
        });
    }

    /**
     * The height of the hex's ground: the median of its chunks' lowest surfaces (trees, houses and walls stand above a
     * chunk's lowest; a ravine is one chunk of many), a chunk never surveyed guessed from the world's generator. The
     * core's wagons climb from hex to hex by it ({@code map.rs}): so the land between towns, never loaded, has its
     * hills and valleys from the start.
     */
    public java.util.Optional<Double> height(ServerLevel level, Hex hex) {
        Map<Long, Integer> surveyed = heights.getOrDefault(level.dimension(), Map.of());
        int[] lows = chunksOf(hex).stream()
                .mapToInt(chunk -> surveyed.containsKey(chunk) ? (surveyed.get(chunk) >>> 16) - 512 : heightGuess(level, chunk))
                .sorted().toArray();
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
        ListTag rivers = new ListTag();
        wet.forEach((dimension, chunks) -> {
            CompoundTag entry = new CompoundTag();
            entry.putString("dimension", dimension.location().toString());
            entry.putLongArray("chunks", chunks.keySet().stream().mapToLong(Long::longValue).toArray());
            entry.putIntArray("bits", chunks.values().stream().mapToInt(Integer::intValue).toArray());
            rivers.add(entry);
        });
        tag.put("wet", rivers);
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
        for (Tag t : tag.getList("wet", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            ResourceKey<Level> dimension =
                    ResourceKey.create(Registries.DIMENSION, ResourceLocation.parse(entry.getString("dimension")));
            long[] positions = entry.getLongArray("chunks");
            int[] bits = entry.getIntArray("bits");
            Map<Long, Integer> chunks = terrain.wet.computeIfAbsent(dimension, d -> new HashMap<>());
            for (int i = 0; i < Math.min(positions.length, bits.length); i++) {
                chunks.put(positions[i], bits[i]);
            }
        }
        return terrain;
    }
}
