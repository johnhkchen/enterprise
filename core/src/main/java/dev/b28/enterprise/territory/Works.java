package dev.b28.enterprise.territory;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.caravan.Caravans;
import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Treasury;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.TicketType;
import net.minecraft.tags.BiomeTags;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.biome.Biomes;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.saveddata.SavedData;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * A town's public works: for now, bridges. A bridge carries caravans over a hex too wet to cross, so the town that
 * builds one holds a chokepoint it made itself. Masons build it for wages, laying stone quarried from the town's land:
 * the bigger the town, the faster the stone comes. The ledger is the bridge; once finished, its deck (a causeway three
 * wide, with railings, of the local stone or wood) is laid in the world the next time its chunks can be loaded.
 */
public final class Works extends SavedData {
    private static final String NAME = "enterprise_works";

    /** Masons on a bridge, each paid this much a day and laying this many blocks a day. */
    public static final int MASONS = 2;
    public static final double MASON_PAY_PER_DAY = 1;
    public static final double BLOCKS_PER_MASON_DAY = 32;

    /** Stone each hex the town holds supplies a day: a town with little land builds slowly. */
    public static final double STONE_PER_HEX_DAY = 16;

    /** A town builds a bridge if its tolls should repay the masons' wages within this many days. */
    public static final double PAYBACK_DAYS = 30;

    /** The deck runs this far past the hex's edges, to land on the far banks. */
    private static final double LANDING = 6;

    private static final long POLL_TICKS = 100;
    private static final TicketType<ChunkPos> LAYING =
            TicketType.create("enterprise_bridge", Comparator.comparingLong(ChunkPos::toLong), 200);

    public static final class Bridge {
        final GlobalPos owner;
        final ResourceKey<Level> dimension;
        final Hex hex;
        final Hex from; // the neighbouring hexes the deck runs between
        final Hex to;
        int blocksNeeded;
        double blocksLaid;
        double wageOwed; // emeralds owed to the masons, fractions carried
        double wagesPaid;
        boolean done;
        boolean placed;
        long lastWork = -1;

        Bridge(GlobalPos owner, ResourceKey<Level> dimension, Hex hex, Hex from, Hex to) {
            this.owner = owner;
            this.dimension = dimension;
            this.hex = hex;
            this.from = from;
            this.to = to;
            this.blocksNeeded = deck(hex, from, to).size();
        }

        public GlobalPos owner() {
            return owner;
        }

        public Hex hex() {
            return hex;
        }

        public boolean done() {
            return done;
        }

        public boolean placed() {
            return placed;
        }

        public double progress() {
            return Math.min(1, blocksLaid / blocksNeeded);
        }

        public double wagesPaid() {
            return wagesPaid;
        }
    }

    private final Map<Territory.Place, Bridge> bridges = new LinkedHashMap<>();

    public static Works get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Works::new, Works::load, null), NAME);
    }

    /** Is there a finished bridge on this hex? */
    public boolean bridged(ResourceKey<Level> dimension, Hex hex) {
        Bridge bridge = bridges.get(new Territory.Place(dimension, hex));
        return bridge != null && bridge.done;
    }

    public Optional<Bridge> bridge(ResourceKey<Level> dimension, Hex hex) {
        return Optional.ofNullable(bridges.get(new Territory.Place(dimension, hex)));
    }

    public List<Bridge> bridgesOf(GlobalPos owner) {
        return bridges.values().stream().filter(b -> b.owner.equals(owner)).toList();
    }

    /** Starts a bridge on {@code hex}, its deck running between the neighbours {@code from} and {@code to}. */
    public Bridge start(GlobalPos owner, ResourceKey<Level> dimension, Hex hex, Hex from, Hex to) {
        Bridge bridge = new Bridge(owner, dimension, hex, from, to);
        bridges.put(new Territory.Place(dimension, hex), bridge);
        setDirty();
        return bridge;
    }

    /** Finishes a bridge at once (for tests and commands). */
    public void finish(Bridge bridge) {
        bridge.blocksLaid = bridge.blocksNeeded;
        bridge.done = true;
        setDirty();
    }

    /** Masons' wages and stone for the days since the town's last touch; a bridge whose blocks are all laid is done. */
    public void work(ServerLevel level, BlockPos desk, MerchantStock stock, GlobalPos owner, int hexesHeld, long now) {
        for (Bridge bridge : bridgesOf(owner)) {
            if (bridge.done) {
                continue;
            }
            if (bridge.lastWork < 0) {
                bridge.lastWork = now;
                setDirty();
                continue;
            }
            double days = Math.min(60, (now - bridge.lastWork) / 24000.0);
            bridge.lastWork = now;
            bridge.wageOwed += MASONS * MASON_PAY_PER_DAY * days;
            int whole = (int) Math.floor(bridge.wageOwed);
            double paidShare = 1;
            if (whole > 0) {
                Mint mint = Mint.get(level.getServer());
                double rate = mint.rate(Mint.DEFAULT_FACTION, now);
                double paid = Math.min(whole, Treasury.withdraw(level, desk, stock, whole, mint, now).value(rate));
                bridge.wageOwed -= whole;
                bridge.wagesPaid += paid;
                paidShare = paid / whole; // unpaid masons don't work, and aren't owed
            }
            double perDay = Math.min(MASONS * BLOCKS_PER_MASON_DAY, hexesHeld * STONE_PER_HEX_DAY);
            bridge.blocksLaid += perDay * days * paidShare;
            if (bridge.blocksLaid >= bridge.blocksNeeded) {
                bridge.done = true;
                Enterprise.LOGGER.debug("Bridge on {} finished for the settlement at {}", bridge.hex,
                        owner.pos().toShortString());
            }
            setDirty();
        }
    }

    /**
     * At a toll review: a town with no bridge under way looks at each impassable hex it holds, and starts the bridge
     * whose tolls would repay its wages soonest, if within {@link #PAYBACK_DAYS} and it can afford half the wages now.
     */
    public void consider(ServerLevel level, BlockPos desk, MerchantStock stock, GlobalPos owner, Territory territory,
            long now) {
        if (bridgesOf(owner).stream().anyMatch(b -> !b.done)) {
            return;
        }
        Terrain terrain = Terrain.get(level.getServer());
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, now);
        double perDay = Math.min(MASONS * BLOCKS_PER_MASON_DAY, territory.hexes(owner).size() * STONE_PER_HEX_DAY);
        Caravans.BridgeValue best = null;
        double bestPayback = PAYBACK_DAYS;
        for (Hex hex : territory.hexes(owner)) {
            if (bridges.containsKey(new Territory.Place(level.dimension(), hex))
                    || !Double.isInfinite(terrain.crossingFactor(level, hex))) {
                continue;
            }
            Caravans.BridgeValue value = Caravans.bridgeValue(level.getServer(), territory, owner, hex, rate);
            double net = value.tollsPerDay() / rate - Territory.guardCostPerDay(rate) / rate;
            if (net <= 0 || value.from() == null) {
                continue;
            }
            double wages = deck(hex, value.from(), value.to()).size() / perDay * MASONS * MASON_PAY_PER_DAY;
            double payback = wages / net;
            if (payback < bestPayback
                    && Treasury.available(level, desk, stock, rate) - territory.reserve(GlobalPos.of(level.dimension(), desk))
                            >= wages / 2) {
                best = value;
                bestPayback = payback;
            }
        }
        if (best != null) {
            start(owner, level.dimension(), best.hex(), best.from(), best.to());
            Enterprise.LOGGER.debug("Settlement at {} starts a bridge on {} (pays back in {} days)",
                    owner.pos().toShortString(), best.hex(), String.format("%.1f", bestPayback));
        }
    }

    /** Lays finished bridges' decks once their chunks are loaded. */
    public static void onServerTick(ServerTickEvent.Post event) {
        MinecraftServer server = event.getServer();
        if (server.overworld().getGameTime() % POLL_TICKS != 0) {
            return;
        }
        Works works = get(server);
        for (Bridge bridge : works.bridges.values()) {
            if (!bridge.done || bridge.placed) {
                continue;
            }
            ServerLevel level = server.getLevel(bridge.dimension);
            if (level == null) {
                continue;
            }
            List<long[]> columns = deck(bridge.hex, bridge.from, bridge.to);
            Set<ChunkPos> chunks = new LinkedHashSet<>();
            columns.forEach(c -> chunks.add(new ChunkPos(BlockPos.containing(c[0], 0, c[1]))));
            boolean loaded = true;
            for (ChunkPos chunk : chunks) {
                if (!level.hasChunk(chunk.x, chunk.z)) {
                    level.getChunkSource().addRegionTicket(LAYING, chunk, 0, chunk);
                    loaded = false;
                }
            }
            if (loaded) {
                lay(level, bridge, columns);
                bridge.placed = true;
                works.setDirty();
            }
        }
    }

    /** Puts the deck on every wet column (land is left as it is): deck at the water's surface, railings either side. */
    static void lay(ServerLevel level, Bridge bridge, List<long[]> columns) {
        var biome = level.getBiome(BlockPos.containing(bridge.hex.centerX(), level.getSeaLevel(), bridge.hex.centerZ()));
        Block deck = Blocks.STONE_BRICKS;
        Block rail = Blocks.STONE_BRICK_WALL;
        if (biome.is(BiomeTags.IS_FOREST) || biome.is(BiomeTags.IS_TAIGA) || biome.is(BiomeTags.IS_JUNGLE)) {
            deck = Blocks.SPRUCE_PLANKS;
            rail = Blocks.SPRUCE_FENCE;
        } else if (biome.is(Biomes.DESERT) || biome.is(BiomeTags.IS_BEACH) || biome.is(BiomeTags.IS_BADLANDS)) {
            deck = Blocks.SMOOTH_SANDSTONE;
            rail = Blocks.SANDSTONE_WALL;
        }
        int laid = 0;
        for (long[] column : columns) {
            int x = (int) column[0];
            int z = (int) column[1];
            int top = level.getHeight(Heightmap.Types.MOTION_BLOCKING, x, z) - 1;
            BlockPos surface = new BlockPos(x, top, z);
            if (level.getFluidState(surface).isEmpty()) {
                continue;
            }
            level.setBlock(surface, deck.defaultBlockState(), 3);
            if (column[2] == 1) {
                level.setBlock(surface.above(), rail.defaultBlockState(), 3);
            }
            laid++;
        }
        Enterprise.LOGGER.debug("Laid the bridge on {}: {} wet columns of {}", bridge.hex, laid, columns.size());
    }

    /**
     * The deck's columns {x, z, railing?}: from the edge facing {@code from}, through the hex's centre, to the edge
     * facing {@code to}, and a little past each edge; three wide with a railing either side.
     */
    public static List<long[]> deck(Hex hex, Hex from, Hex to) {
        Map<Long, long[]> cells = new LinkedHashMap<>();
        double cx = hex.centerX();
        double cz = hex.centerZ();
        for (Hex end : List.of(from, to)) {
            double ex = end.centerX() - cx;
            double ez = end.centerZ() - cz;
            double length = Math.hypot(ex, ez);
            double ux = ex / length;
            double uz = ez / length;
            double reach = length / 2 + LANDING;
            for (double t = 0; t <= reach; t += 0.5) {
                for (int w = -2; w <= 2; w++) {
                    long x = (long) Math.floor(cx + ux * t - uz * w);
                    long z = (long) Math.floor(cz + uz * t + ux * w);
                    long key = (x << 32) ^ (z & 0xffffffffL);
                    boolean railing = Math.abs(w) == 2;
                    long[] existing = cells.get(key);
                    if (existing == null || (existing[2] == 1 && !railing)) {
                        cells.put(key, new long[] {x, z, railing ? 1 : 0}); // deck wins over railing
                    }
                }
            }
        }
        return new ArrayList<>(cells.values());
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        for (Bridge bridge : bridges.values()) {
            CompoundTag entry = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, bridge.owner).ifSuccess(t -> entry.put("owner", t));
            entry.putString("dimension", bridge.dimension.location().toString());
            entry.putIntArray("hexes", new int[] {bridge.hex.q(), bridge.hex.r(), bridge.from.q(), bridge.from.r(),
                    bridge.to.q(), bridge.to.r()});
            entry.putDouble("blocksLaid", bridge.blocksLaid);
            entry.putDouble("wageOwed", bridge.wageOwed);
            entry.putDouble("wagesPaid", bridge.wagesPaid);
            entry.putBoolean("done", bridge.done);
            entry.putBoolean("placed", bridge.placed);
            entry.putLong("lastWork", bridge.lastWork);
            list.add(entry);
        }
        tag.put("bridges", list);
        return tag;
    }

    public static Works load(CompoundTag tag, HolderLookup.Provider registries) {
        Works works = new Works();
        for (Tag t : tag.getList("bridges", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            ResourceKey<Level> dimension =
                    ResourceKey.create(Registries.DIMENSION, ResourceLocation.parse(entry.getString("dimension")));
            int[] h = entry.getIntArray("hexes");
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("owner")).ifSuccess(owner -> {
                Bridge bridge = new Bridge(owner, dimension, new Hex(h[0], h[1]), new Hex(h[2], h[3]), new Hex(h[4], h[5]));
                bridge.blocksLaid = entry.getDouble("blocksLaid");
                bridge.wageOwed = entry.getDouble("wageOwed");
                bridge.wagesPaid = entry.getDouble("wagesPaid");
                bridge.done = entry.getBoolean("done");
                bridge.placed = entry.getBoolean("placed");
                bridge.lastWork = entry.getLong("lastWork");
                works.bridges.put(new Territory.Place(dimension, bridge.hex), bridge);
            });
        }
        return works;
    }
}
