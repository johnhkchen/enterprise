package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.caravan.Expeditions;
import dev.b28.enterprise.binding.CoreClock;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.territory.Buildings;
import dev.b28.enterprise.territory.Crops;
import dev.b28.enterprise.territory.Docks;
import dev.b28.enterprise.territory.Fields;
import dev.b28.enterprise.territory.Growth;
import dev.b28.enterprise.territory.Labour;
import dev.b28.enterprise.territory.Terrain;
import dev.b28.enterprise.territory.Trails;
import dev.b28.enterprise.territory.Works;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.Count;
import dev.b28.enterprise.wire.GroundView;
import dev.b28.enterprise.wire.HexAt;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.LandKind;
import dev.b28.enterprise.wire.Output;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.Work;
import dev.b28.enterprise.wire.WorkView;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.TreeSet;
import java.util.concurrent.CompletableFuture;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.TicketType;
import net.minecraft.tags.BiomeTags;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.biome.Biome;
import net.neoforged.neoforge.common.Tags;
import net.neoforged.neoforge.event.level.ChunkEvent;

/**
 * A town's land, from the core (docs/binding-plan.md 2, concern 7). The host surveys its world and lays what the core's
 * towns build; whose land is whose, what it grows, what's worth building and when are the core's ({@code land.rs},
 * {@code territory.rs}, {@code works.rs}, {@code farms.rs}).
 *
 * <ul>
 *   <li><b>Ground.</b> The overworld's chunks are surveyed as they load and unload ({@link Terrain}: their surface
 *       columns under water, their heights; a chunk never loaded guessed from its biome); a hex whose survey moved is
 *       sent as a {@code Ground} at the next look (its share under water, its relief and its biome's land at its
 *       centre), at most {@value #GROUND_PER_LOOK} a look. The first time a run sees a town, the hexes within {@value
 *       #REACH} of it whose ground the core has otherwise are sent too, so the roads it plans over know the lakes and
 *       cliffs nobody has walked. Both modes: the core's map is the world's whichever rules run.</li>
 *   <li><b>Survey</b> (core mode). Each look, every town whose desk is loaded: its bell (the nearest to its desk, or
 *       the desk) as a {@code Survey}, and once every {@value #CROPS_EVERY} looks (and the first look of a run) the crop
 *       plots in every loaded chunk of the hexes it holds ({@link Crops}, within 16 blocks of the bell's height) as
 *       {@code Crops}. The core keeps each chunk's last count, so a chunk not loaded keeps what it had.</li>
 *   <li><b>Builds</b> (core mode). The core asks for its towns' works at the end of each look that surveyed them
 *       ({@code Build}): a bridge's deck ({@link Works#layDeck}), a wall's columns from what's shown to what's due a
 *       quarter at a time ({@link Works#raiseColumns}, round the bell), a hall at its site ({@link Buildings#site},
 *       {@link Buildings#build}), a field of its crop on zoned ground in the town's land ({@link Growth#farmSite},
 *       {@link Fields#plant}, its hex's crops counted at once), a family's home by the bell ({@link Growth#homeSite},
 *       {@link Growth#buildHome}, as the Java rules build one), at the start of the next look, once every look sent is
 *       shown (so asking the core while laying never shows a look inside another's outcomes, and a work laid and not yet
 *       said is never laid again). Laid, it's said ({@code Built}, with that look); not yet laid (its chunks not loaded:
 *       they're held loaded past the look after, so the look that asks again finds them; no site), it's asked for
 *       again at that look's end.</li>
 * </ul>
 *
 * <p>The adapter keeps no land of its own: the hexes a town holds and its works are the core's ({@link #land}), the
 * survey's chunks {@link Terrain}'s. In Java mode the Java rules own the land ({@code Territory}, {@code Works}) and
 * only the ground is sent. The core's map is one plane: only the overworld's ground is sent.
 */
public final class LandAdapter {
    /** Rings of hexes round a town first seen in a run whose ground is sent: past the six a road may stray, and two more. */
    public static final int REACH = 8;
    /** Hexes whose ground is sent in one look, at most (each is about 40 chunks' surveys). */
    public static final int GROUND_PER_LOOK = 64;
    /** Looks between counts of a town's crops: a game day at a look a minute. */
    public static final int CROPS_EVERY = 20;
    /**
     * Holds a work's chunks loaded while it waits for them: four looks, past the look that asks for it again (laid at the
     * start of the one after). {@code Works}' own ticket lasts 200 ticks, for its 100-tick poll.
     */
    private static final TicketType<ChunkPos> LAYING =
            TicketType.create("enterprise_land", Comparator.comparingLong(ChunkPos::toLong), (int) (4 * CoreClock.EVERY));

    private final CoreHost host;
    /** Whether the core's towns' land is the core's to survey and build (core mode, a test's host), or only the ground. */
    private final boolean ownsLand;
    /** The core's goods by number, and the number of each item that's one. */
    private List<Item> items = List.of();
    private Map<Item, Integer> numbers = Map.of();
    /** Overworld hexes whose survey moved since they were last sent (heard on the server thread, read there). */
    private final Set<Hex> moved = new LinkedHashSet<>();
    /** Hexes to send the ground of because a town was first seen near them, queued behind {@link #moved}. */
    private final Set<Hex> near = new LinkedHashSet<>();
    /** The towns this run has seen. */
    private final Set<Long> seen = new HashSet<>();
    /** Looks since each town's crops were counted. */
    private final Map<Long, Integer> sinceCounted = new HashMap<>();
    /** What the host laid of the core's builds, and the crops of the fields it laid, for the next look. */
    private final List<Input> pending = new ArrayList<>();
    /**
     * What was said in a look the core refused or trapped on (heard on the core's thread): said again with the next, so
     * a work laid is never asked for, and laid, twice.
     */
    private final java.util.concurrent.ConcurrentLinkedQueue<Input> unsaid = new java.util.concurrent.ConcurrentLinkedQueue<>();
    /** The builds the core asked for since the last look, by town and work (the largest share asked), laid at the next. */
    private final Map<List<Object>, Output.Build> asked = new LinkedHashMap<>();
    /** The towns a host's settlers founded that the core asked to have built, by town, till the next look. */
    private final Map<Long, Output.Found> founding = new LinkedHashMap<>();
    /** Trails shown since the last look, for the world's {@link Trails}. */
    private final List<Trails.Trail> trailsShown = new ArrayList<>();

    public LandAdapter(CoreHost host, boolean ownsLand) {
        this.host = host;
        this.ownsLand = ownsLand;
    }

    /** Reads the goods from the core again: what its numbers mean. */
    public void reload() {
        if (host.ask(List.of(new Query.Goods())).get(0) instanceof Answer.Goods g) {
            List<Item> byNumber = new ArrayList<>(g.goods().size());
            Map<Item, Integer> byItem = new HashMap<>();
            for (int i = 0; i < g.goods().size(); i++) {
                ResourceLocation id = ResourceLocation.tryParse(g.goods().get(i));
                Item item = id == null ? Items.AIR : BuiltInRegistries.ITEM.get(id);
                byNumber.add(item);
                if (item != Items.AIR) {
                    byItem.put(item, i);
                }
            }
            items = List.copyOf(byNumber);
            numbers = Map.copyOf(byItem);
        }
    }

    /** Whether the core's towns' land is surveyed and built here (core mode), or only the ground sent. */
    public boolean ownsLand() {
        return ownsLand;
    }

    /**
     * A chunk loaded: {@link Terrain} has surveyed it (heard after it, at the lowest priority), so its hex's
     * ground goes to the core with the next look. On the server thread only, as the survey is.
     */
    public static void onChunkLoad(ChunkEvent.Load event) {
        onChunk(event);
    }

    /** A chunk unloaded: surveyed again by {@link Terrain} as it goes, as {@link #onChunkLoad}. */
    public static void onChunkUnload(ChunkEvent.Unload event) {
        onChunk(event);
    }

    private static void onChunk(ChunkEvent event) {
        if (event.getLevel() instanceof ServerLevel level && level.getServer().isSameThread()) {
            CoreHost host = CoreHost.of(level.getServer());
            if (host != null && host.pausedWhy() == null) {
                host.land().surveyed(level, event.getChunk().getPos());
            }
        }
    }

    /** The chunk at {@code chunk} was surveyed: its hex (the one its middle lies in, as {@link Terrain} counts it) moved. */
    public void surveyed(ServerLevel level, ChunkPos chunk) {
        if (level.dimension() == Level.OVERWORLD) {
            moved.add(Hex.at(chunk.getMiddleBlockX() + 0.5, chunk.getMiddleBlockZ() + 0.5));
        }
    }

    /**
     * A look's happenings: what the host laid since the last (and the crops of the fields it laid), the ground of the
     * hexes whose survey moved or that a town first seen stands near, and (with {@code survey}: core mode) every town
     * whose desk is loaded surveyed, its crops counted every {@value #CROPS_EVERY} looks.
     */
    public List<Input> happenings(MinecraftServer server, boolean survey) {
        layAsked(server);
        buildFounded(server);
        layTrails(server);
        List<Input> inputs = takePending();
        ServerLevel overworld = server.overworld();
        for (var desk : host.points().of(PointsAdapter.Kind.DESK).entrySet()) {
            if (seen.add(desk.getValue()) && desk.getKey().dimension() == Level.OVERWORLD) {
                reach(Hex.at(desk.getKey().pos()));
            }
        }
        inputs.addAll(grounds(overworld));
        if (survey && ownsLand) {
            for (var desk : host.points().of(PointsAdapter.Kind.DESK).entrySet()) {
                ServerLevel level = server.getLevel(desk.getKey().dimension());
                if (level == null || !level.isLoaded(desk.getKey().pos())
                        || !level.getBlockState(desk.getKey().pos()).is(Merchant.DESK.get())) {
                    continue;
                }
                long town = desk.getValue();
                BlockPos bell = PeopleAdapter.centre(level, desk.getKey().pos());
                // Its forum, laid free at its first survey: the heart its main roads leave from.
                int paved = dev.b28.enterprise.territory.Forums.get(server).ensure(level, town, bell);
                if (paved > 0) {
                    dev.b28.enterprise.Enterprise.LOGGER.info("Town {} lays its forum round its bell at {} ({} paved)", town,
                            bell.toShortString(), paved);
                }
                int since = sinceCounted.getOrDefault(town, CROPS_EVERY);
                if (since >= CROPS_EVERY) {
                    inputs.addAll(crops(level, town, desk.getKey().pos(), bell));
                    since = 0;
                }
                sinceCounted.put(town, since + 1);
                inputs.add(new Input.Survey(town, bell.getX(), bell.getZ()));
            }
        }
        return inputs;
    }

    /** The hexes within {@value #REACH} of {@code centre} queued for their ground (those the core has otherwise). */
    private void reach(Hex centre) {
        List<HexAt> ring = new ArrayList<>();
        for (int dq = -REACH; dq <= REACH; dq++) {
            for (int dr = Math.max(-REACH, -dq - REACH); dr <= Math.min(REACH, -dq + REACH); dr++) {
                ring.add(new HexAt(centre.q() + dq, centre.r() + dr));
            }
        }
        near.addAll(ring.stream().map(h -> new Hex(h.q(), h.r())).toList());
    }

    /**
     * The ground of up to {@value #GROUND_PER_LOOK} hexes: those whose survey moved first, then those near a town first
     * seen whose ground the core has otherwise (asked, so a hex the core has right isn't sent).
     */
    private List<Input> grounds(ServerLevel level) {
        List<Input.Ground> out = new ArrayList<>();
        var it = moved.iterator();
        while (it.hasNext() && out.size() < GROUND_PER_LOOK) {
            Hex hex = it.next();
            it.remove();
            near.remove(hex);
            out.add(ground(level, hex));
        }
        if (out.size() < GROUND_PER_LOOK && !near.isEmpty()) {
            List<Input.Ground> wanted = new ArrayList<>();
            var nit = near.iterator();
            while (nit.hasNext() && wanted.size() < GROUND_PER_LOOK - out.size()) {
                wanted.add(ground(level, nit.next()));
                nit.remove();
            }
            List<GroundView> had = groundOf(wanted.stream().map(Input.Ground::hex).toList());
            for (int i = 0; i < wanted.size(); i++) {
                Input.Ground g = wanted.get(i);
                GroundView h = had.get(i);
                if (h.water() != g.water() || h.relief() != g.relief() || h.land() != g.land()
                        || !h.height().equals(g.height()) || h.river() != g.river()) {
                    out.add(g);
                }
            }
        }
        return new ArrayList<>(out);
    }

    /** A hex's ground as the world's {@link Terrain} has surveyed it, with its biome's land at its centre. */
    public static Input.Ground ground(ServerLevel level, Hex hex) {
        return ground(Terrain.get(level.getServer()), level, hex);
    }

    /** A hex's ground as {@code terrain} has surveyed it, with its biome's land at its centre. */
    public static Input.Ground ground(Terrain terrain, ServerLevel level, Hex hex) {
        return new Input.Ground(new HexAt(hex.q(), hex.r()), terrain.waterShare(level, hex), terrain.relief(level, hex),
                kind(Labour.biome(level, hex)), terrain.height(level, hex), terrain.river(level, hex));
    }

    /**
     * The core's land for a biome: the mod's biome tags in the order {@code Labour.hauls} reads them (water, beach,
     * badlands and desert, swamp, jungle and savanna, hills, snow and taiga, forest, the rest open ground).
     */
    public static LandKind kind(Holder<Biome> biome) {
        if (biome.is(BiomeTags.IS_OCEAN) || biome.is(BiomeTags.IS_DEEP_OCEAN) || biome.is(BiomeTags.IS_RIVER)) {
            return LandKind.Water;
        }
        if (biome.is(BiomeTags.IS_BEACH)) {
            return LandKind.Shore;
        }
        if (biome.is(BiomeTags.IS_BADLANDS) || biome.is(Tags.Biomes.IS_DESERT)) {
            return LandKind.Desert;
        }
        if (biome.is(Tags.Biomes.IS_SWAMP) || biome.is(BiomeTags.IS_JUNGLE) || biome.is(BiomeTags.IS_SAVANNA)) {
            return LandKind.Woods;
        }
        if (biome.is(BiomeTags.IS_MOUNTAIN) || biome.is(BiomeTags.IS_HILL)) {
            return LandKind.Hills;
        }
        if (biome.is(Tags.Biomes.IS_SNOWY) || biome.is(BiomeTags.IS_TAIGA)) {
            return LandKind.Taiga;
        }
        if (biome.is(BiomeTags.IS_FOREST)) {
            return LandKind.Woods;
        }
        return LandKind.Plains;
    }

    /** The ground of these hexes as the core has it, in the order given. */
    public List<GroundView> groundOf(List<HexAt> hexes) {
        if (hexes.isEmpty()) {
            return List.of();
        }
        if (host.ask(List.of(new Query.Ground(hexes))).get(0) instanceof Answer.Ground g) {
            return g.ground();
        }
        throw new IllegalStateException("the core answered Ground with something else");
    }

    /**
     * The crops counted in every loaded chunk of the hexes a town holds (before it holds any, its home hex: the one its
     * desk's block is in, as the core places it), within 16 blocks of its bell's height.
     */
    public List<Input> crops(ServerLevel level, long town, BlockPos desk, BlockPos bell) {
        List<Hex> hexes = land(town).hexes();
        if (hexes.isEmpty()) {
            hexes = List.of(Hex.at((double) desk.getX(), (double) desk.getZ()));
        }
        List<Input> counts = new ArrayList<>();
        for (Hex hex : hexes) {
            counts.addAll(cropsIn(level, hex, bell.getY()));
        }
        return counts;
    }

    /** The crop plots in each loaded chunk of {@code hex} within 16 blocks of {@code y}, by the core's goods. */
    public List<Input> cropsIn(ServerLevel level, Hex hex, int y) {
        List<Input> counts = new ArrayList<>();
        for (long chunk : Crops.chunksOf(hex)) {
            Map<Item, Integer> plots = Crops.countChunk(level, chunk, hex, y);
            if (plots == null) {
                continue; // not loaded: the core keeps its last count
            }
            List<Count> found = new ArrayList<>();
            new java.util.TreeMap<>(byNumber(plots)).forEach((good, n) -> found.add(new Count(good, n)));
            counts.add(new Input.Crops(new HexAt(hex.q(), hex.r()), ChunkPos.getX(chunk), ChunkPos.getZ(chunk), found));
        }
        return counts;
    }

    private Map<Long, Double> byNumber(Map<Item, Integer> plots) {
        Map<Long, Double> out = new HashMap<>();
        plots.forEach((item, n) -> {
            Integer good = numbers.get(item);
            if (good != null && n > 0) {
                out.merge((long) good, (double) n, Double::sum);
            }
        });
        return out;
    }

    /**
     * A town surveyed now (a command, a test): its crops counted, its bell sent, its works to lay shown at once.
     *
     * @throws IllegalStateException when the core refuses it (a town it hasn't)
     */
    public void surveyNow(ServerLevel level, long town, BlockPos desk) {
        layAsked(level.getServer()); // every look shown and what it asked for laid first: said with this survey
        BlockPos bell = PeopleAdapter.centre(level, desk);
        List<Input> inputs = takePending();
        List<Input> said = inputs.stream().filter(i -> i instanceof Input.Built).toList();
        inputs.addAll(crops(level, town, desk, bell));
        inputs.add(new Input.Survey(town, bell.getX(), bell.getZ()));
        sinceCounted.put(town, 1);
        Response r = host.apply(inputs);
        if (!(r instanceof Response.Done)) {
            unsaid(said);
            throw new IllegalStateException("the core wouldn't take town " + town + "'s survey: "
                    + (r instanceof Response.Refused refused ? refused.why() : host.pausedWhy()));
        }
        layAsked(level.getServer());
    }

    /**
     * A look sent with {@code inputs}: the works it says were laid are said again with the next look if the core refuses
     * it or traps on it, so a work laid is never asked for, and laid, twice.
     */
    public CompletableFuture<Response> watch(CompletableFuture<Response> look, List<Input> inputs) {
        List<Input> said = inputs.stream().filter(i -> i instanceof Input.Built).toList();
        if (said.isEmpty()) {
            return look;
        }
        return look.whenComplete((r, e) -> {
            if (e != null || r instanceof Response.Refused) {
                unsaid(said);
            }
        });
    }

    /** What the host has laid and not yet said, sent now (a command, a test, a save). */
    public void sayNow() {
        List<Input> inputs = takePending();
        if (!inputs.isEmpty() && !(host.apply(inputs) instanceof Response.Done)) {
            unsaid(inputs.stream().filter(i -> i instanceof Input.Built).toList());
        }
    }

    /** What's to be said: what a refused look didn't say, then what was laid since. */
    private List<Input> takePending() {
        List<Input> inputs = new ArrayList<>();
        for (Input i; (i = unsaid.poll()) != null; ) {
            inputs.add(i);
        }
        inputs.addAll(pending);
        pending.clear();
        return inputs;
    }

    /**
     * What was said in a look the core didn't take (refused, or trapped on): said again with the next. Safe from the
     * core's thread.
     */
    public void unsaid(List<Input> said) {
        unsaid.addAll(said);
    }

    /** What the host has laid and not yet said: for the next look. */
    public List<Input> pending() {
        List<Input> all = new ArrayList<>(unsaid);
        all.addAll(pending);
        return List.copyOf(all);
    }

    /** A town's land as the core has it: the hexes it holds, its culture, its fields' plots by item, its works. */
    public record Land(long town, List<Hex> hexes, double culture, double nextHex, Map<String, Double> plots, List<WorkView> works) {
        /** How much of a work the host has shown, as the core has it (0 if it has no such work). */
        public double shown(Work work) {
            return works.stream().filter(w -> w.work().equals(work)).mapToDouble(WorkView::shown).findFirst().orElse(0);
        }
    }

    public Land land(long town) {
        if (host.ask(List.of(new Query.Land(town))).get(0) instanceof Answer.Land a) {
            Map<String, Double> plots = new LinkedHashMap<>();
            for (Count c : a.plots()) {
                plots.put(name(c.good()), c.count());
            }
            return new Land(a.town(), a.hexes().stream().map(h -> new Hex(h.q(), h.r())).toList(), a.culture(), a.nextHex(),
                    Map.copyOf(plots), a.works());
        }
        throw new IllegalStateException("the core answered Land with something else");
    }

    /** A good's item name by the core's number. */
    public String name(long good) {
        return good >= 0 && good < items.size() ? BuiltInRegistries.ITEM.getKey(items.get((int) good)).toString() : "#" + good;
    }

    /**
     * One of the core's builds, to lay at the start of the next look if the host owns the land (core mode). Shown among
     * a look's outcomes, where asking the core would show a later look inside this one's: kept until then.
     */
    public void show(Output.Build build) {
        if (ownsLand) {
            asked.merge(List.of(build.town(), build.work()), build, (a, b) -> b.share() >= a.share() ? b : a);
        }
    }

    /** A trail the core's wagons wore, to keep (saved) and lay once its ground is loaded (core mode: {@link Trails}). */
    public void show(Output.Trail trail) {
        if (ownsLand) {
            trailsShown.add(new Trails.Trail(new Hex(trail.hex().q(), trail.hex().r()),
                    new Hex(trail.from().q(), trail.from().r()), new Hex(trail.to().q(), trail.to().r()),
                    (int) Math.min(trail.width(), 8), trail.path().stream().map(h -> new Hex(h.q(), h.r())).toList(),
                    trail.llama()));
        }
    }

    /** Trails at most laid a look: each a hex's worth of path. */
    static final int TRAILS_PER_LOOK = 2;

    /**
     * The trails shown since the last look kept with the world's, and up to {@value #TRAILS_PER_LOOK} of those waiting
     * laid whose ground is loaded (the rest held loaded, laid at a later look).
     */
    private void layTrails(MinecraftServer server) {
        if (server == null) {
            return;
        }
        Trails trails = Trails.get(server);
        trailsShown.forEach(trails::add);
        trailsShown.clear();
        ServerLevel level = server.overworld();
        int laid = 0;
        var bells = dev.b28.enterprise.territory.Forums.get(server).bells();
        for (Trails.Trail t : trails.waiting()) {
            if (laid >= TRAILS_PER_LOOK) {
                break;
            }
            // Its road planned once its land is made and loaded (held loaded until then).
            if (!Trails.planned(t)) {
                if (!loadedFor(level, Trails.landOf(t, bells))) {
                    continue;
                }
                Trails.plan(level, t, bells);
            }
            if (loadedFor(level, Trails.chunks(t, bells))) {
                int blocks = Trails.lay(level, t, bells);
                trails.laid(t);
                dev.b28.enterprise.Enterprise.LOGGER.info("A trail laid across hex {}, {} wide ({} blocks of path)", t.hex(),
                        t.width(), blocks);
                laid++;
            }
        }
    }

    /**
     * A town the core's settlers founded, to build where they settled at the start of the next look (core mode): the
     * Java rules' square ({@link Expeditions#buildTown}) round the core's spot, and its desk told to the core.
     */
    public void show(Output.Found found) {
        if (ownsLand) {
            founding.put(found.town(), found);
        }
    }

    /**
     * The founded towns the core asked for built in the overworld, each whose ground is loaded (held loaded if it isn't,
     * and built when the core asks again): a bell, the Merchant's desk with its chest, a Farmer's stall, two cottages,
     * found round the core's spot on open ground; its desk a point of the town's, so the core asks no more and its
     * settlers are spawned there once it's counted. With no open ground there, it's left: the core asks again.
     */
    private void buildFounded(MinecraftServer server) {
        if (founding.isEmpty() || server == null) {
            return;
        }
        host.settle();
        List<Output.Found> todo = new ArrayList<>(founding.values());
        founding.clear();
        ServerLevel level = server.overworld();
        for (Output.Found f : todo) {
            if (host.points().of(PointsAdapter.Kind.DESK).containsValue(f.town())) {
                continue; // built, and said, since it was asked
            }
            ChunkPos centre = new ChunkPos(new BlockPos(f.x(), 0, f.z()));
            List<ChunkPos> chunks = new ArrayList<>();
            for (int dx = -FOUND_CHUNKS; dx <= FOUND_CHUNKS; dx++) {
                for (int dz = -FOUND_CHUNKS; dz <= FOUND_CHUNKS; dz++) {
                    chunks.add(new ChunkPos(centre.x + dx, centre.z + dz));
                }
            }
            if (!loadedFor(level, chunks)) {
                continue;
            }
            int y = level.getHeight(net.minecraft.world.level.levelgen.Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, f.x(), f.z());
            var town = Expeditions.buildTown(level, new BlockPos(f.x(), y, f.z()));
            if (town.isEmpty()) {
                Enterprise.LOGGER.info("No open ground to build {} at {} {}: asked again", f.name(), f.x(), f.z());
                continue;
            }
            String why = host.points().add(GlobalPos.of(level.dimension(), town.get().desk()), PointsAdapter.Kind.DESK, f.town());
            if (why != null) {
                Enterprise.LOGGER.warn("The core wouldn't take {}'s desk at {}: {}", f.name(), town.get().desk().toShortString(), why);
            } else {
                Enterprise.LOGGER.info("{} founded at {}", f.name(), town.get().desk().toShortString());
                dev.b28.enterprise.settlement.Grounds.founded(level, town.get().desk(), f.name());
            }
        }
    }

    /** Chunks either side of a founded town's spot held loaded to build it: its square, desk and cottages within reach. */
    static final int FOUND_CHUNKS = 2;

    /**
     * The builds the core asked for laid, once every look sent is shown: each but those laid and not yet said (asked for
     * again by a look that went before the {@code Built}).
     */
    private void layAsked(MinecraftServer server) {
        if (asked.isEmpty()) {
            return;
        }
        host.settle();
        List<Output.Build> builds = new ArrayList<>(asked.values());
        asked.clear();
        Map<List<Object>, Double> said = new HashMap<>();
        for (Input i : pending()) {
            if (i instanceof Input.Built b) {
                said.merge(List.of(b.town(), b.work()), b.share(), Math::max);
            }
        }
        for (Output.Build b : builds) {
            if (said.getOrDefault(List.of(b.town(), b.work()), -1.0) < b.share()) {
                layNow(server, b);
            }
        }
    }

    /**
     * One of the core's builds laid in the world, if its town's desk is loaded: where it goes found here, its chunks
     * loaded first (held loaded if they aren't, and laid when the core asks again).
     */
    private void layNow(MinecraftServer server, Output.Build build) {
        if (server == null) {
            return;
        }
        for (var desk : host.points().of(PointsAdapter.Kind.DESK).entrySet()) {
            if (desk.getValue() != build.town()) {
                continue;
            }
            ServerLevel level = server.getLevel(desk.getKey().dimension());
            if (level != null && level.isLoaded(desk.getKey().pos())) {
                lay(level, build.town(), PeopleAdapter.centre(level, desk.getKey().pos()), build.work(), build.share(),
                        site(level, build.town(), PeopleAdapter.centre(level, desk.getKey().pos()), build.work()));
            }
            return;
        }
    }

    /** Where a hall or a field goes (empty for a bridge or the wall, which go where they must): the mod's sites. */
    private Optional<BlockPos> site(ServerLevel level, long town, BlockPos bell, Work work) {
        return switch (work) {
            case Work.Hall h -> Buildings.kind(h.kind()).flatMap(kind -> kind == Buildings.STONE_DOCKS
                    ? Docks.findShore(level, bell).map(Docks.Pier::start) : Buildings.site(level, bell, kind, Optional.empty()));
            case Work.Field f -> {
                // On zoned ground near the bell, in the town's land (as `Fields.consider` plants).
                Set<Hex> held = new HashSet<>(land(town).hexes());
                Optional<BlockPos> site = Growth.farmSite(level, bell, Fields.SIZE, 8);
                yield site.filter(c -> held.isEmpty() || held.contains(Hex.at(c.offset(Fields.SIZE / 2, 0, Fields.SIZE / 2))));
            }
            case Work.Home h -> Growth.homeSite(level, bell);
            default -> Optional.empty();
        };
    }

    /**
     * Lays {@code share} of a town's work round {@code bell} (a hall or a field at {@code site}), and says so with the
     * next look: a bridge's deck, the wall's columns from what the core has shown to {@code share}, a hall, a field
     * (its hexes' crops counted again with it, so its plots are the counted ones from then on). False when it can't be
     * laid now (its chunks loading, no site, a hall or a crop the mod hasn't): the core asks again.
     */
    public boolean lay(ServerLevel level, long town, BlockPos bell, Work work, double share, Optional<BlockPos> site) {
        // What the host says it laid: the work, or a pasture with the grass inside its fence.
        Work[] reported = {work};
        boolean laid = switch (work) {
            case Work.Pasture p -> {
                // The largest rectangle of open grassland on the town's zoned ground (`Pastures.site`), fenced.
                var found = dev.b28.enterprise.territory.Pastures.site(level, bell);
                if (found.isEmpty()) {
                    yield false;
                }
                BlockPos corner = found.get().corner();
                if (!loadedFor(level, List.of(new ChunkPos(corner),
                        new ChunkPos(corner.offset(found.get().sizeX(), 0, found.get().sizeZ()))).stream().distinct().toList())) {
                    yield false;
                }
                reported[0] = new Work.Pasture(dev.b28.enterprise.territory.Pastures.fence(level, found.get(), bell));
                Enterprise.LOGGER.info("Town {} fences a {} x {} pasture at {}", town, found.get().sizeX(),
                        found.get().sizeZ(), corner.toShortString());
                yield true;
            }
            case Work.Bridge b -> {
                Hex hex = new Hex(b.hex().q(), b.hex().r());
                List<long[]> deck = Works.deck(hex, new Hex(b.from().q(), b.from().r()), new Hex(b.to().q(), b.to().r()));
                Set<ChunkPos> chunks = new LinkedHashSet<>();
                deck.forEach(c -> chunks.add(new ChunkPos(BlockPos.containing(c[0], 0, c[1]))));
                if (!loadedFor(level, chunks)) {
                    yield false;
                }
                Works.layDeck(level, hex, deck);
                yield true;
            }
            case Work.Wall w -> {
                List<BlockPos> ring = Works.ring(bell, Works.WALL_RADIUS);
                int from = (int) Math.floor(land(town).shown(work) * ring.size());
                int to = (int) Math.floor(Math.min(1, share) * ring.size());
                List<BlockPos> stretch = ring.subList(Math.min(from, to), to);
                if (!loadedFor(level, stretch.stream().map(ChunkPos::new).distinct().toList())) {
                    yield false;
                }
                Works.raiseColumns(level, stretch);
                yield true;
            }
            case Work.Hall h -> {
                Optional<Buildings.Kind> kind = Buildings.kind(h.kind());
                if (kind.isEmpty() || site.isEmpty()) {
                    yield false;
                }
                BlockPos corner = site.get();
                int n = Math.max(kind.get().size(), Docks.PIER * 2 + 1);
                if (!loadedFor(level, List.of(new ChunkPos(corner.offset(-n, 0, -n)), new ChunkPos(corner),
                        new ChunkPos(corner.offset(n, 0, n))).stream().distinct().toList())) {
                    yield false;
                }
                Buildings.build(level, kind.get(), corner);
                yield true;
            }
            case Work.Home h -> {
                if (site.isEmpty()) {
                    yield false;
                }
                BlockPos corner = site.get();
                if (!loadedFor(level, List.of(new ChunkPos(corner), new ChunkPos(corner.offset(Growth.HOME_SIZE, 0, Growth.HOME_SIZE)))
                        .stream().distinct().toList())) {
                    yield false;
                }
                Growth.buildHome(level, corner);
                yield true;
            }
            case Work.Field f -> {
                Item crop = f.crop() >= 0 && f.crop() < items.size() ? items.get((int) f.crop()) : Items.AIR;
                if (crop == Items.AIR || site.isEmpty()) {
                    yield false;
                }
                BlockPos corner = site.get();
                Fields.plant(level, corner, crop);
                Set<Hex> hexes = new TreeSet<>(java.util.Comparator.comparingInt(Hex::q).thenComparingInt(Hex::r));
                for (BlockPos c : List.of(corner, corner.offset(Fields.SIZE - 1, 0, 0), corner.offset(0, 0, Fields.SIZE - 1),
                        corner.offset(Fields.SIZE - 1, 0, Fields.SIZE - 1))) {
                    hexes.add(Hex.at(c));
                }
                for (Hex hex : hexes) {
                    pending.addAll(cropsIn(level, hex, bell.getY()));
                }
                yield true;
            }
        };
        if (laid) {
            pending.add(new Input.Built(town, reported[0], Math.min(1, share)));
        } else {
            Enterprise.LOGGER.debug("Town {}'s {} can't be laid yet", town, work);
        }
        return laid;
    }

    /**
     * Whether every one of {@code chunks} is loaded, so a work can be laid there now; those that aren't are held loaded
     * ({@link #LAYING}) so the look that asks again finds them.
     */
    private static boolean loadedFor(ServerLevel level, java.util.Collection<ChunkPos> chunks) {
        boolean loaded = true;
        for (ChunkPos chunk : chunks) {
            if (!level.hasChunk(chunk.x, chunk.z)) {
                level.getChunkSource().addRegionTicket(LAYING, chunk, 0, chunk);
                loaded = false;
            }
        }
        return loaded;
    }

    /** The town at a desk, for commands: its desk's place in the world, the desk's chunk loaded. */
    public static GlobalPos deskOf(CoreHost host, long town) {
        for (var desk : host.points().of(PointsAdapter.Kind.DESK).entrySet()) {
            if (desk.getValue() == town) {
                return desk.getKey();
            }
        }
        return null;
    }
}
