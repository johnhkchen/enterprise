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
 * A town's public works: bridges, walls and buildings (see {@link Buildings}). A bridge carries caravans over a hex too wet to cross, so the town that
 * builds one holds a chokepoint it made itself. A wall rings the town; a walled town feels safe and gathers culture. Masons build it for wages, laying stone quarried from the town's land:
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

    /** Something masons build for a town: wages and stone go in, blocks laid come out. */
    public abstract static class Project {
        final GlobalPos owner;
        final ResourceKey<Level> dimension;
        int blocksNeeded;
        double blocksLaid;
        double wageOwed; // emeralds owed to the masons, fractions carried
        double wagesPaid;
        boolean done;
        long lastWork = -1;

        Project(GlobalPos owner, ResourceKey<Level> dimension) {
            this.owner = owner;
            this.dimension = dimension;
        }

        public GlobalPos owner() {
            return owner;
        }

        public boolean done() {
            return done;
        }

        public double progress() {
            return blocksNeeded == 0 ? 1 : Math.min(1, blocksLaid / blocksNeeded);
        }

        public double wagesPaid() {
            return wagesPaid;
        }

        void save(CompoundTag entry) {
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, owner).ifSuccess(t -> entry.put("owner", t));
            entry.putString("dimension", dimension.location().toString());
            entry.putDouble("blocksLaid", blocksLaid);
            entry.putDouble("wageOwed", wageOwed);
            entry.putDouble("wagesPaid", wagesPaid);
            entry.putBoolean("done", done);
            entry.putLong("lastWork", lastWork);
        }

        void load(CompoundTag entry) {
            blocksLaid = entry.getDouble("blocksLaid");
            wageOwed = entry.getDouble("wageOwed");
            wagesPaid = entry.getDouble("wagesPaid");
            done = entry.getBoolean("done");
            lastWork = entry.getLong("lastWork");
        }
    }

    public static final class Bridge extends Project {
        final Hex hex;
        final Hex from; // the neighbouring hexes the deck runs between
        final Hex to;
        boolean placed;

        Bridge(GlobalPos owner, ResourceKey<Level> dimension, Hex hex, Hex from, Hex to) {
            super(owner, dimension);
            this.hex = hex;
            this.from = from;
            this.to = to;
            this.blocksNeeded = deck(hex, from, to).size();
        }

        public Hex hex() {
            return hex;
        }

        public boolean placed() {
            return placed;
        }
    }

    /**
     * A town wall: a ring of columns {@link #WALL_RADIUS} blocks round the bell, {@link #WALL_HEIGHT} high, with gates
     * at the four points of the compass. It goes up in the world a quarter at a time as the masons lay it.
     */
    public static final class Wall extends Project {
        final BlockPos centre;
        final int radius;
        int columnsPlaced; // of the ring, in order round from the east

        Wall(GlobalPos owner, ResourceKey<Level> dimension, BlockPos centre, int radius) {
            super(owner, dimension);
            this.centre = centre;
            this.radius = radius;
            this.blocksNeeded = ring(centre, radius).size() * WALL_HEIGHT;
        }

        public int columnsPlaced() {
            return columnsPlaced;
        }

        /** Columns the masons have laid so far. */
        int columnsLaid() {
            return (int) Math.min(ring(centre, radius).size(), Math.floor(blocksLaid / WALL_HEIGHT));
        }
    }

    /** A building for the town's trades or its people (see {@link Buildings}), laid in the world once finished. */
    public static final class Building extends Project {
        final Buildings.Kind kind;
        final BlockPos corner;
        boolean placed;

        Building(GlobalPos owner, ResourceKey<Level> dimension, Buildings.Kind kind, BlockPos corner) {
            super(owner, dimension);
            this.kind = kind;
            this.corner = corner;
            this.blocksNeeded = kind.blocks();
        }

        public Buildings.Kind kind() {
            return kind;
        }

        public BlockPos corner() {
            return corner;
        }

        public boolean placed() {
            return placed;
        }
    }

    /** Walls ring the bell at this distance: past the homes (up to 32 out), inside the census (48). */
    public static final int WALL_RADIUS = 40;
    public static final int WALL_HEIGHT = 3;

    /** A walled town feels safe: this much mood, and this much culture a day, when the wall is whole. */
    public static final double WALL_MOOD = 1;
    public static final double WALL_CULTURE_PER_DAY = 2;

    /** A town starts a wall at this mood or better, with at least this many emeralds to spare. */
    public static final double WALL_MOOD_TO_START = 0.5;
    public static final double WALL_START_EMERALDS = 12;

    private final Map<Territory.Place, Bridge> bridges = new LinkedHashMap<>();
    private final Map<GlobalPos, Wall> walls = new LinkedHashMap<>();
    private final Map<GlobalPos, Map<String, Building>> buildings = new LinkedHashMap<>(); // by owner, by kind

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

    /** Raises a stretch of wall now (for tests). */
    public static void raiseForTest(ServerLevel level, Wall wall, List<BlockPos> stretch) {
        raise(level, wall, stretch);
    }

    /** Finishes a project at once (for tests and commands). */
    public void finish(Project bridge) {
        bridge.blocksLaid = bridge.blocksNeeded;
        bridge.done = true;
        setDirty();
    }

    /** The town's wall, if it has started one. */
    public Optional<Wall> wall(GlobalPos owner) {
        return Optional.ofNullable(walls.get(owner));
    }

    /** How much of the town's wall stands, 0 to 1. */
    public double walled(GlobalPos owner) {
        return wall(owner).map(Project::progress).orElse(0.0);
    }

    public Wall startWall(GlobalPos owner, int radius) {
        Wall wall = new Wall(owner, owner.dimension(), owner.pos(), radius);
        walls.put(owner, wall);
        setDirty();
        return wall;
    }

    /** Starts a building of {@code kind} for the town, its corner at {@code corner}. */
    public Building startBuilding(GlobalPos owner, Buildings.Kind kind, BlockPos corner) {
        Building building = new Building(owner, owner.dimension(), kind, corner);
        buildings.computeIfAbsent(owner, o -> new LinkedHashMap<>()).put(kind.id(), building);
        setDirty();
        return building;
    }

    public void forgetForTest(GlobalPos owner) {
        buildings.remove(owner);
        walls.remove(owner);
        setDirty();
    }

    /** The town's buildings, started or finished, by kind. */
    public Map<String, Building> buildingsOf(GlobalPos owner) {
        return Map.copyOf(buildings.getOrDefault(owner, Map.of()));
    }

    /** Is a building of this kind finished for the town? */
    public boolean built(GlobalPos owner, String kind) {
        Building building = buildings.getOrDefault(owner, Map.of()).get(kind);
        return building != null && building.done;
    }

    /** Kinds of building the town has finished. */
    public Set<String> builtOf(GlobalPos owner) {
        Set<String> done = new LinkedHashSet<>();
        buildings.getOrDefault(owner, Map.of()).forEach((kind, b) -> {
            if (b.done) {
                done.add(kind);
            }
        });
        return done;
    }

    /** The town's project being built now, if any. */
    public Optional<Project> underway(GlobalPos owner) {
        return projectsOf(owner).stream().filter(project -> !project.done).findFirst();
    }

    /** A visiting Mason lays {@code blocks} on a project. */
    public void help(Project project, double blocks) {
        project.blocksLaid = Math.min(project.blocksNeeded, project.blocksLaid + blocks);
        if (project.blocksLaid >= project.blocksNeeded) {
            project.done = true;
        }
        setDirty();
    }

    /** The town's projects under way or done: bridges, then its wall, then its buildings. */
    List<Project> projectsOf(GlobalPos owner) {
        List<Project> projects = new ArrayList<>(bridgesOf(owner));
        wall(owner).ifPresent(projects::add);
        projects.addAll(buildings.getOrDefault(owner, Map.of()).values());
        return projects;
    }

    /** Is a project of this town still being built? Masons work on one at a time. */
    public boolean busy(GlobalPos owner) {
        return projectsOf(owner).stream().anyMatch(project -> !project.done);
    }

    /**
     * Masons' wages and stone for the days since the town's last touch, for its unfinished project (one at a time);
     * a project whose blocks are all laid is done.
     */
    public void work(ServerLevel level, BlockPos desk, MerchantStock stock, GlobalPos owner, int hexesHeld, long now) {
        for (Project bridge : projectsOf(owner)) {
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
                bridge.blocksLaid = bridge.blocksNeeded;
                bridge.done = true;
                Enterprise.LOGGER.debug("{} finished for the settlement at {}",
                        bridge instanceof Bridge b ? "Bridge on " + b.hex
                                : bridge instanceof Building b ? "A " + b.kind.id() : "Wall", owner.pos().toShortString());
            }
            setDirty();
            return; // one project at a time
        }
    }

    /**
     * At a toll review: a town with no bridge under way looks at each impassable hex it holds, and starts the bridge
     * whose tolls would repay its wages soonest, if within {@link #PAYBACK_DAYS} and it can afford half the wages now.
     */
    public void consider(ServerLevel level, BlockPos desk, MerchantStock stock, GlobalPos owner, Territory territory,
            long now) {
        if (busy(owner)) {
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
            double spare = Treasury.available(level, desk, stock, rate) - territory.reserve(GlobalPos.of(level.dimension(), desk));
            Enterprise.LOGGER.debug("Settlement at {}: a bridge on {} would cost {} emeralds, repaid in {} days; {} to spare",
                    owner.pos().toShortString(), hex, String.format("%.1f", wages), String.format("%.1f", payback),
                    String.format("%.1f", spare));
            if (payback < bestPayback && spare >= wages / 2) {
                best = value;
                bestPayback = payback;
            }
        }
        if (best != null) {
            start(owner, level.dimension(), best.hex(), best.from(), best.to());
            Enterprise.LOGGER.debug("Settlement at {} starts a bridge on {} (pays back in {} days)",
                    owner.pos().toShortString(), best.hex(), String.format("%.1f", bestPayback));
            return;
        }
        // Nothing that pays: a content town with money to spare walls itself in.
        if (!walls.containsKey(owner) && territory.mood(owner) >= WALL_MOOD_TO_START
                && Treasury.available(level, desk, stock, rate) - territory.reserve(GlobalPos.of(level.dimension(), desk))
                        >= WALL_START_EMERALDS) {
            startWall(owner, WALL_RADIUS);
            Enterprise.LOGGER.debug("Settlement at {} starts a wall", owner.pos().toShortString());
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
        for (Map<String, Building> ofTown : works.buildings.values()) {
            for (Building building : ofTown.values()) {
                if (!building.done || building.placed) {
                    continue;
                }
                ServerLevel level = server.getLevel(building.dimension);
                if (level == null) {
                    continue;
                }
                int n = Math.max(building.kind.size(), Docks.PIER * 2 + 1);
                boolean loaded = true;
                for (ChunkPos chunk : List.of(new ChunkPos(building.corner.offset(-n, 0, -n)), new ChunkPos(building.corner),
                        new ChunkPos(building.corner.offset(n, 0, n))).stream().distinct().toList()) {
                    if (!level.hasChunk(chunk.x, chunk.z)) {
                        level.getChunkSource().addRegionTicket(LAYING, chunk, 0, chunk);
                        loaded = false;
                    }
                }
                if (loaded) {
                    Buildings.build(level, building.kind, building.corner);
                    building.placed = true;
                    works.setDirty();
                }
            }
        }
        for (Wall wall : works.walls.values()) {
            List<BlockPos> ring = ring(wall.centre, wall.radius);
            int laid = wall.columnsLaid();
            // A quarter at a time, and the last stretch when done.
            if (laid - wall.columnsPlaced < ring.size() / 4 && !(wall.done && laid > wall.columnsPlaced)) {
                continue;
            }
            ServerLevel level = server.getLevel(wall.dimension);
            if (level == null) {
                continue;
            }
            List<BlockPos> stretch = ring.subList(wall.columnsPlaced, laid);
            boolean loaded = true;
            for (ChunkPos chunk : stretch.stream().map(ChunkPos::new).distinct().toList()) {
                if (!level.hasChunk(chunk.x, chunk.z)) {
                    level.getChunkSource().addRegionTicket(LAYING, chunk, 0, chunk);
                    loaded = false;
                }
            }
            if (loaded) {
                raise(level, wall, stretch);
                wall.columnsPlaced = laid;
                works.setDirty();
            }
        }
    }

    /**
     * The wall's columns, in order round from the east: every column {@code radius} blocks (rounded) from the centre,
     * leaving gates three wide at the four points of the compass. Only x and z count.
     */
    public static List<BlockPos> ring(BlockPos centre, int radius) {
        List<BlockPos> columns = new ArrayList<>();
        for (int dx = -radius - 1; dx <= radius + 1; dx++) {
            for (int dz = -radius - 1; dz <= radius + 1; dz++) {
                if (Math.round(Math.hypot(dx, dz)) != radius || Math.abs(dx) <= 1 || Math.abs(dz) <= 1) {
                    continue; // off the ring, or a gate
                }
                columns.add(new BlockPos(centre.getX() + dx, 0, centre.getZ() + dz));
            }
        }
        columns.sort(Comparator.comparingDouble(
                c -> (Math.atan2(c.getZ() - centre.getZ(), c.getX() - centre.getX()) + 2 * Math.PI) % (2 * Math.PI)));
        return columns;
    }

    /**
     * Raises a stretch of wall {@link #WALL_HEIGHT} high on natural ground, of what the land offers: a log palisade on
     * grass and in woods, sandstone on sand, cobblestone on stone. Where the ground is a field, a path, water or
     * something built, or the air above isn't clear, it leaves a gap: villagers' paths become gates.
     */
    static void raise(ServerLevel level, Wall wall, List<BlockPos> stretch) {
        int raised = 0;
        for (BlockPos column : stretch) {
            int top = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, column.getX(), column.getZ()) - 1;
            BlockPos ground = new BlockPos(column.getX(), top, column.getZ());
            var under = level.getBlockState(ground);
            Block material;
            if (under.is(net.minecraft.tags.BlockTags.SAND)) {
                material = Blocks.SANDSTONE;
            } else if (under.is(net.minecraft.tags.BlockTags.BASE_STONE_OVERWORLD) || under.is(Blocks.GRAVEL)) {
                material = Blocks.COBBLESTONE;
            } else if (under.is(Blocks.GRASS_BLOCK) || under.is(Blocks.DIRT) || under.is(Blocks.PODZOL)
                    || under.is(Blocks.COARSE_DIRT) || under.is(Blocks.SNOW_BLOCK) || under.is(Blocks.MUD)) {
                var biome = level.getBiome(ground);
                material = biome.is(BiomeTags.IS_TAIGA) || biome.is(net.minecraft.tags.BiomeTags.SPAWNS_SNOW_FOXES)
                        ? Blocks.SPRUCE_LOG : Blocks.OAK_LOG;
            } else {
                continue; // a field, a path, a building: leave a gap
            }
            boolean clear = true;
            for (int up = 1; up <= WALL_HEIGHT; up++) {
                var above = level.getBlockState(ground.above(up));
                clear &= above.canBeReplaced() && above.getFluidState().isEmpty();
            }
            if (!clear) {
                continue;
            }
            for (int up = 1; up <= WALL_HEIGHT; up++) {
                level.setBlock(ground.above(up), material.defaultBlockState(), 3);
            }
            raised++;
        }
        Enterprise.LOGGER.debug("Raised {} of {} columns of wall for the settlement at {}", raised, stretch.size(),
                wall.owner.pos().toShortString());
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
            bridge.save(entry);
            entry.putIntArray("hexes", new int[] {bridge.hex.q(), bridge.hex.r(), bridge.from.q(), bridge.from.r(),
                    bridge.to.q(), bridge.to.r()});
            entry.putBoolean("placed", bridge.placed);
            list.add(entry);
        }
        tag.put("bridges", list);
        ListTag wallList = new ListTag();
        for (Wall wall : walls.values()) {
            CompoundTag entry = new CompoundTag();
            wall.save(entry);
            entry.putInt("radius", wall.radius);
            entry.putInt("columnsPlaced", wall.columnsPlaced);
            wallList.add(entry);
        }
        tag.put("walls", wallList);
        ListTag buildingList = new ListTag();
        buildings.values().forEach(ofTown -> ofTown.values().forEach(building -> {
            CompoundTag entry = new CompoundTag();
            building.save(entry);
            entry.putString("kind", building.kind.id());
            entry.putLong("corner", building.corner.asLong());
            entry.putBoolean("placed", building.placed);
            buildingList.add(entry);
        }));
        tag.put("buildings", buildingList);
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
                bridge.load(entry);
                bridge.placed = entry.getBoolean("placed");
                works.bridges.put(new Territory.Place(dimension, bridge.hex), bridge);
            });
        }
        for (Tag t : tag.getList("walls", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("owner")).ifSuccess(owner -> {
                Wall wall = new Wall(owner, owner.dimension(), owner.pos(), entry.getInt("radius"));
                wall.load(entry);
                wall.columnsPlaced = entry.getInt("columnsPlaced");
                works.walls.put(owner, wall);
            });
        }
        for (Tag t : tag.getList("buildings", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            var kind = Buildings.kind(entry.getString("kind"));
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("owner")).ifSuccess(owner -> kind.ifPresent(k -> {
                Building building = new Building(owner, owner.dimension(), k, BlockPos.of(entry.getLong("corner")));
                building.load(entry);
                building.placed = entry.getBoolean("placed");
                works.buildings.computeIfAbsent(owner, o -> new LinkedHashMap<>()).put(k.id(), building);
            }));
        }
        return works;
    }
}
