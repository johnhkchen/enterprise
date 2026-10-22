package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.merchant.Depots;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.settlement.Names;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.Count;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.Output;
import dev.b28.enterprise.wire.Point;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Response;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.Level;
import net.neoforged.neoforge.event.level.BlockEvent;
import net.neoforged.neoforge.event.level.ChunkEvent;
import net.neoforged.neoforge.server.ServerLifecycleHooks;

/**
 * A town's stock in its chests, from the core (docs/binding-plan.md 1.5 and 2, concern 3). Chests stay physical:
 * players, hoppers and Create move what's in them. A Merchant's desk is a town in the core and its trade point ({@code
 * AddTown}); a depot is one more container of the town's warehouse, away from the desk (a storehouse's chest), a point
 * of its own ({@link #addDepots}). At each look every loaded desk and depot is counted into a {@code Stocktake} (its
 * goods, loose and in the shulker boxes kept there, and its empty slots), and the core books what changed since as
 * goods delivered or taken. Then the core shows the town's stock there: what its own rules moved since (a wagon's
 * sale, the shelf its Merchant stocks) comes back as {@code Moved}, written to the chests here ({@link #show}); what
 * didn't fit, or wasn't there to take, goes back as {@code Unmoved} before the point's next stocktake.
 *
 * <p><b>Never a blind write.</b> A move is a number of items worked out against a stocktake of the same look, never a
 * chest's whole contents, so a hopper filling a chest between the count and the write keeps what it put in. A chest
 * that wasn't counted (its chunks unloaded) isn't written; when its chunk loads it's counted at once ({@link
 * #onChunkLoad}, {@link #catchUps}), what changed in it meanwhile booked first, then what its town moved shown. No
 * stocktake is taken while a look's moves are still to be shown.
 *
 * <p><b>Whose chests.</b> In core mode the core's moves are written. In Java mode the Java rules own the chests and
 * the core beside them only counts them: every move goes back unmoved, and the core keeps it on the town's books.
 *
 * <p>The adapter keeps no society ids and no stock of its own: the desks, depots and their towns are the core's points
 * ({@link PointsAdapter}), the goods' numbers the core's content, read back when the host starts ({@link #reload}), and
 * what a town holds the core's to say ({@link #stockAt}). An item that isn't one of the core's goods isn't counted. A
 * desk or depot gone without the host seeing it (an explosion, a piston, {@code /setblock}) is removed at the next look
 * that finds it loaded; a desk placed unseen (a datapack, a structure) is founded when its chunk next loads.
 */
public final class StockAdapter {
    private final CoreHost host;
    /** Whether the core's moves are written to the chests (core mode, a test's host), or all sent back unmoved. */
    private final boolean writes;
    /** The core's goods by number, the item each is, and the number of each item that's one. */
    private List<String> goods = List.of();
    private List<Item> items = List.of();
    private Map<Item, Integer> numbers = Map.of();
    /** What the host couldn't do of the core's moves, for the next look (before any stocktake). */
    private final List<Input> pending = new ArrayList<>();
    /** Chunks loaded since the last catch-up, whose points are counted at once (heard on any thread, read on the server's). */
    private final Set<Loaded> loading = java.util.concurrent.ConcurrentHashMap.newKeySet();
    /** Whether desks found in a chunk just loaded are taken as towns (the server's host; a test's keeps to its own). */
    private boolean takesUnseen;

    private record Loaded(ResourceKey<Level> dim, long chunk) {}

    public StockAdapter(CoreHost host, boolean writes) {
        this.host = host;
        this.writes = writes;
    }

    /** Reads the goods from the core again: what its numbers mean. */
    public void reload() {
        List<Answer> answers = host.ask(List.of(new Query.Goods()));
        if (answers.get(0) instanceof Answer.Goods g) {
            goods = g.goods();
            List<Item> byNumber = new ArrayList<>(goods.size());
            Map<Item, Integer> byItem = new HashMap<>();
            for (int i = 0; i < goods.size(); i++) {
                ResourceLocation id = ResourceLocation.tryParse(goods.get(i));
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

    /** The item the core's good {@code good} is here (air for a good with none). */
    public Item item(long good) {
        return good >= 0 && good < items.size() ? items.get((int) good) : Items.AIR;
    }

    /** The core's number for {@code item}, or null for an item that isn't one of its goods. */
    public Long number(Item item) {
        Integer n = numbers.get(item);
        return n == null ? null : n.longValue();
    }

    /** Whether the core's moves are written to the chests here. */
    public boolean writes() {
        return writes;
    }

    /** Desks found in chunks as they load are taken as towns from now on (the server's host does; a test's may). */
    public void takeUnseenDesks() {
        takesUnseen = true;
    }

    /** The town at {@code desk}, if the core keeps a desk there. */
    public Long townAt(GlobalPos desk) {
        PointsAdapter.Placed p = host.points().at(desk);
        return p != null && p.kind() == PointsAdapter.Kind.DESK ? p.town() : null;
    }

    /** Every desk the core keeps, with its town. */
    public Map<GlobalPos, Long> desks() {
        return host.points().of(PointsAdapter.Kind.DESK);
    }

    /** Every depot the core keeps, with its town. */
    public Map<GlobalPos, Long> depots() {
        return host.points().of(PointsAdapter.Kind.DEPOT);
    }

    /**
     * The desk at {@code desk} registered with the core as a town called {@code name}, and its chests counted now (its
     * first stocktake, from nothing). A desk the core keeps already is left as it is; one put back where a town's desk
     * stood is that town's again (the core says which).
     *
     * @return the desk's town
     * @throws IllegalStateException when the core refuses it
     */
    public long found(ServerLevel level, BlockPos desk, String name) {
        GlobalPos at = GlobalPos.of(level.dimension(), desk);
        Long known = townAt(at);
        if (known != null) {
            return known;
        }
        // Founded at this tick (the points adapter brings the core to now), not at the last look's, nor at tick 0 of a
        // society new to an old world; its first stocktake booked then too.
        long town = host.points().found(level, desk, name);
        Response r = host.apply(host.money().withCash(List.of(stocktake(level, PointsAdapter.Kind.DESK, at))));
        if (r instanceof Response.Refused refused) {
            throw new IllegalStateException("the core wouldn't count the desk at " + desk.toShortString() + ": " + refused.why());
        }
        return town;
    }

    /** The desk at {@code desk} broken: no longer the core's point, its chests' goods gone from its town with them. */
    public void forget(ServerLevel level, BlockPos desk) {
        GlobalPos at = GlobalPos.of(level.dimension(), desk);
        if (townAt(at) != null) {
            host.points().remove(at);
        }
    }

    /**
     * A building's containers between two corners (inclusive) registered as depots of the town at {@code desk}, each a
     * point of its own, counted at once (what they hold is delivered to the town). Containers touching the desk (the
     * desk's own chests), those another point stands at already, and the second half of a double chest are skipped.
     *
     * @return the depots added, in their places' order
     * @throws IllegalStateException when the core keeps no town at {@code desk}
     */
    public List<GlobalPos> addDepots(ServerLevel level, BlockPos desk, BlockPos from, BlockPos to) {
        Long town = townAt(GlobalPos.of(level.dimension(), desk));
        if (town == null) {
            throw new IllegalStateException("the core keeps no town at " + desk.toShortString());
        }
        Set<BlockPos> desks = new HashSet<>(dev.b28.enterprise.locker.Lockers.touchingContainers(level, desk));
        List<GlobalPos> added = new ArrayList<>();
        List<Input> counts = new ArrayList<>();
        for (BlockPos p : BlockPos.betweenClosed(from, to)) {
            if (!StockChests.isContainer(level, p)) {
                continue;
            }
            BlockPos key = dev.b28.enterprise.locker.Lockers.canonicalContainer(p.immutable(), level.getBlockState(p));
            GlobalPos at = GlobalPos.of(level.dimension(), key);
            if (desks.contains(key) || added.contains(at) || host.points().at(at) != null) {
                continue;
            }
            String why = host.points().add(at, PointsAdapter.Kind.DEPOT, town);
            if (why != null) {
                Enterprise.LOGGER.warn("The core wouldn't take a depot at {}: {}", key.toShortString(), why);
                continue;
            }
            added.add(at);
            counts.add(stocktake(level, PointsAdapter.Kind.DEPOT, at));
        }
        if (!counts.isEmpty()) {
            host.apply(counts);
        }
        return added;
    }

    /**
     * Containers in a building's floorplan that the core doesn't keep as depots, registered as its town's and counted at
     * once: a chest put back after it was broken (gone unseen, its point removed at a look), one added to the building
     * since, a double chest's other half once the half its point was keyed by is gone. The floorplan is the building's
     * record ({@code merchant/Depots}, what {@code /enterprise depot add} wrote, beside its kind, which the core doesn't
     * keep yet); its containers are the core's points, so the two can't drift apart. Buildings not wholly loaded, or of a
     * desk the core keeps no town at, are left for a later look.
     *
     * @return the depots added
     */
    public List<GlobalPos> joinDepots(MinecraftServer server) {
        List<GlobalPos> added = new ArrayList<>();
        for (Depots.Depot d : Depots.get(server).all()) {
            ServerLevel level = server.getLevel(d.desk().dimension());
            if (level != null && townAt(d.desk()) != null && d.loaded(level)) {
                added.addAll(addDepots(level, d.desk().pos(), d.min(), d.max()));
            }
        }
        return added;
    }

    /** The depots between two corners removed, their goods gone from their towns with them. Returns how many. */
    public int removeDepots(ServerLevel level, BlockPos from, BlockPos to) {
        int removed = 0;
        for (GlobalPos at : depots().keySet()) {
            BlockPos p = at.pos();
            if (at.dimension() == level.dimension() && between(p, from, to) && host.points().remove(at)) {
                removed++;
            }
        }
        return removed;
    }

    private static boolean between(BlockPos p, BlockPos a, BlockPos b) {
        return p.getX() >= Math.min(a.getX(), b.getX()) && p.getX() <= Math.max(a.getX(), b.getX())
                && p.getY() >= Math.min(a.getY(), b.getY()) && p.getY() <= Math.max(a.getY(), b.getY())
                && p.getZ() >= Math.min(a.getZ(), b.getZ()) && p.getZ() <= Math.max(a.getZ(), b.getZ());
    }

    /**
     * A Merchant's desk placed by a player (or anything that places as one): a town in the core, its chests counted.
     * Heard last ({@code EventPriority.LOWEST}, cancelled events not heard), so a placement another listener cancels
     * founds nothing. The core's failures stay here: logged, never thrown into the placement.
     */
    public static void onPlaced(BlockEvent.EntityPlaceEvent event) {
        if (event.getLevel() instanceof ServerLevel level && event.getPlacedBlock().is(Merchant.DESK.get())) {
            CoreHost host = CoreHost.of(level.getServer());
            if (host != null && host.pausedWhy() == null) {
                try {
                    GlobalPos at = GlobalPos.of(level.dimension(), event.getPos());
                    host.stock().found(level, event.getPos(), Names.of(level.getServer(), at));
                } catch (RuntimeException e) {
                    Enterprise.LOGGER.error("The core didn't take the desk at {}; the world goes on without it",
                            event.getPos().toShortString(), e);
                }
            }
        }
    }

    /**
     * A Merchant's desk broken: its point gone from the core, its chests' goods with it. Heard last, as {@link
     * #onPlaced}, so a break another listener cancels keeps the point; failures are logged, not thrown.
     */
    public static void onBroken(BlockEvent.BreakEvent event) {
        if (event.getLevel() instanceof ServerLevel level && event.getState().is(Merchant.DESK.get())) {
            CoreHost host = CoreHost.of(level.getServer());
            if (host != null && host.pausedWhy() == null) {
                try {
                    host.stock().forget(level, event.getPos());
                } catch (RuntimeException e) {
                    Enterprise.LOGGER.error("The core didn't forget the desk at {}", event.getPos().toShortString(), e);
                }
            }
        }
    }

    /** A chunk loaded: its desks and depots are counted at the next tick ({@link #catchUps}), before players reach them. */
    public static void onChunkLoad(ChunkEvent.Load event) {
        if (event.getLevel() instanceof ServerLevel level) {
            CoreHost host = CoreHost.of(level.getServer());
            if (host != null && host.pausedWhy() == null) {
                host.stock().loaded(level, event.getChunk().getPos());
            }
        }
    }

    /** The chunk at {@code chunk} loaded in {@code level}: its points to be counted at once. */
    public void loaded(ServerLevel level, ChunkPos chunk) {
        loading.add(new Loaded(level.dimension(), chunk.toLong()));
    }

    /**
     * A look's stocktakes: every desk, depot and locker the core keeps that is loaded (with every block touching it),
     * counted. A desk that isn't one any more, a depot that isn't a container, or a locker whose workstation no container
     * touches, is removed from the core first: gone unseen.
     */
    public List<Input> stocktakes(MinecraftServer server) {
        List<Input> inputs = new ArrayList<>();
        for (Map.Entry<GlobalPos, PointsAdapter.Placed> e : new ArrayList<>(host.points().all().entrySet())) {
            PointsAdapter.Kind kind = e.getValue().kind();
            if (!counted(kind)) {
                continue;
            }
            GlobalPos at = e.getKey();
            ServerLevel level = server.getLevel(at.dimension());
            if (level == null || !loaded(level, at.pos())) {
                continue;
            }
            if (!stands(level, kind, at.pos())) {
                Enterprise.LOGGER.info("The {} at {} is gone; the core forgets it", kind.name().toLowerCase(), at.pos().toShortString());
                host.points().remove(at);
                continue;
            }
            inputs.add(stocktake(level, kind, at));
        }
        return inputs;
    }

    /**
     * The stocktakes of the points in the chunks loaded since the last catch-up (and at their edges), and desks placed
     * unseen in them founded: what a look would count of them, now. Empty when no chunk has loaded.
     */
    public List<Input> catchUps(MinecraftServer server) {
        if (loading.isEmpty()) {
            return List.of();
        }
        // In a fixed order (dimension, then chunk), so desks found unseen in several chunks become towns in the same order
        // every run of the same world.
        Set<Loaded> chunks = new java.util.TreeSet<>(java.util.Comparator.comparing((Loaded l) -> l.dim().location().toString())
                .thenComparingLong(Loaded::chunk));
        for (var it = loading.iterator(); it.hasNext(); ) {
            chunks.add(it.next());
            it.remove();
        }
        for (Loaded c : takesUnseen ? chunks : Set.<Loaded>of()) {
            ServerLevel level = server.getLevel(c.dim());
            if (level != null) {
                foundUnseen(level, new ChunkPos(c.chunk()));
            }
        }
        List<Input> inputs = new ArrayList<>();
        for (Map.Entry<GlobalPos, PointsAdapter.Placed> e : host.points().all().entrySet()) {
            PointsAdapter.Kind kind = e.getValue().kind();
            GlobalPos at = e.getKey();
            ServerLevel level = server.getLevel(at.dimension());
            if (!counted(kind) || level == null
                    || !touches(chunks, at) || !loaded(level, at.pos()) || !stands(level, kind, at.pos())) {
                continue;
            }
            inputs.add(stocktake(level, kind, at));
        }
        return inputs;
    }

    /** Desks in a chunk just loaded that the core doesn't keep (placed while the host wasn't looking): founded. */
    private void foundUnseen(ServerLevel level, ChunkPos chunk) {
        level.getPoiManager().getInChunk(type -> type.is(Merchant.DESK_POI.getKey()), chunk, PoiManager.Occupancy.ANY)
                .map(poi -> poi.getPos().immutable())
                .filter(pos -> host.points().at(GlobalPos.of(level.dimension(), pos)) == null)
                .filter(pos -> level.getBlockState(pos).is(Merchant.DESK.get()) && loaded(level, pos))
                .toList()
                .forEach(pos -> {
                    try {
                        found(level, pos, Names.of(level.getServer(), GlobalPos.of(level.dimension(), pos)));
                        Enterprise.LOGGER.info("The core takes the desk at {}, placed unseen", pos.toShortString());
                    } catch (RuntimeException ex) {
                        Enterprise.LOGGER.error("The core didn't take the desk at {}", pos.toShortString(), ex);
                    }
                });
    }

    private static boolean touches(Set<Loaded> chunks, GlobalPos at) {
        if (chunks.contains(new Loaded(at.dimension(), ChunkPos.asLong(at.pos())))) {
            return true;
        }
        for (Direction dir : Direction.Plane.HORIZONTAL) {
            if (chunks.contains(new Loaded(at.dimension(), ChunkPos.asLong(at.pos().relative(dir))))) {
                return true;
            }
        }
        return false;
    }

    /** What the host couldn't do of the core's moves since the last look, for the next one (taken: sent once). */
    public List<Input> pending() {
        List<Input> out = List.copyOf(pending);
        pending.clear();
        return out;
    }

    /**
     * One of the core's moves at a desk, a depot or a locker: so many of a good put in or taken out of its chests (core mode), as
     * far as they go. What couldn't be done (no room, gone already, the chunk unloaded since the count, or Java mode)
     * is sent back as {@code Unmoved}. A move for a point the host no longer has is dropped: the core forgot it too.
     */
    public void show(Output.Moved moved) {
        GlobalPos at = PointsAdapter.place(moved.point());
        PointsAdapter.Placed p = host.points().at(at);
        if (p == null) {
            return;
        }
        long left = moved.count();
        Item item = moved.good() < items.size() ? items.get((int) moved.good()) : Items.AIR;
        MinecraftServer server = ServerLifecycleHooks.getCurrentServer();
        ServerLevel level = server == null ? null : server.getLevel(at.dimension());
        if (writes && item != Items.AIR && level != null && loaded(level, at.pos())) {
            StockChests chests = StockChests.at(level, p.kind(), at.pos());
            left = left > 0 ? chests.put(item, left) : -chests.take(item, -left);
        }
        if (left != 0) {
            Enterprise.LOGGER.debug("{} of good {} not moved at {}", left, moved.good(), at.pos().toShortString());
            pending.add(new Input.Unmoved(moved.point(), moved.good(), left));
        }
    }

    /** The point's chests counted: every good of the core's they hold (shulker boxes' goods included), and their empty slots. */
    public Input.Stocktake stocktake(ServerLevel level, PointsAdapter.Kind kind, GlobalPos at) {
        StockChests chests = StockChests.at(level, kind, at.pos());
        Map<Integer, Long> held = new TreeMap<>();
        chests.held().forEach((item, n) -> {
            Integer good = numbers.get(item);
            if (good != null) {
                held.merge(good, n, Long::sum);
            }
        });
        List<Count> counts = new ArrayList<>(held.size());
        held.forEach((good, n) -> counts.add(new Count(good, n.doubleValue())));
        return new Input.Stocktake(PointsAdapter.point(kind, at), counts, chests.free());
    }

    /** What the chests at a point hold now of each of the core's goods, by name (for a command or a test to compare). */
    public Map<String, Long> counted(ServerLevel level, PointsAdapter.Kind kind, BlockPos pos) {
        Map<String, Long> out = new LinkedHashMap<>();
        StockChests.at(level, kind, pos).held().forEach((item, n) -> {
            Integer good = numbers.get(item);
            if (good != null) {
                out.merge(goods.get(good), n, Long::sum);
            }
        });
        return out;
    }

    /**
     * The points whose chests are counted at each look and written the core's moves: desks and depots (the town's
     * warehouse), and lockers (a workstation's chests, which the Merchant empties into it: {@code LockerAdapter}).
     */
    private static boolean counted(PointsAdapter.Kind kind) {
        return kind == PointsAdapter.Kind.DESK || kind == PointsAdapter.Kind.DEPOT || kind == PointsAdapter.Kind.LOCKER;
    }

    /** Is the point still there: a desk still a desk, a depot still a container, a locker's workstation still touching one. */
    private static boolean stands(ServerLevel level, PointsAdapter.Kind kind, BlockPos pos) {
        return switch (kind) {
            case DESK -> level.getBlockState(pos).is(Merchant.DESK.get());
            case LOCKER -> !dev.b28.enterprise.locker.Lockers.touchingContainers(level, pos).isEmpty();
            default -> StockChests.isContainer(level, pos);
        };
    }

    /** The block at {@code pos} and the six touching it are in loaded chunks (a look never loads a chunk to count a chest). */
    static boolean loaded(ServerLevel level, BlockPos pos) {
        if (!level.isLoaded(pos)) {
            return false;
        }
        for (Direction dir : Direction.values()) {
            if (!level.isLoaded(pos.relative(dir))) {
                return false;
            }
        }
        return true;
    }

    /**
     * What the core says the town at {@code desk} holds, by good (those it holds any of, in the goods' order), or null
     * if the core keeps no town there.
     */
    public Map<String, Double> stockAt(GlobalPos desk) {
        Long town = townAt(desk);
        if (town == null) {
            return null;
        }
        Answer a = host.ask(List.of(new Query.Stock(town))).get(0);
        Map<String, Double> held = new LinkedHashMap<>();
        if (a instanceof Answer.Stock s) {
            for (int good = 0; good < s.stock().size() && good < goods.size(); good++) {
                double n = s.stock().get(good);
                if (n != 0) {
                    held.put(goods.get(good), n);
                }
            }
        }
        return held;
    }

    /** A point's chests as the core takes them to be: what they hold by good, their empty slots, what they came up short. */
    public record Chests(long town, Map<String, Double> held, long free, Map<String, Double> shortBy) {}

    /** The chests at {@code at} as the core takes them to be, or null if the core keeps no point there. */
    public Chests chestsAt(GlobalPos at) {
        PointsAdapter.Placed p = host.points().at(at);
        if (p == null) {
            return null;
        }
        if (host.ask(List.of(new Query.Chests(PointsAdapter.point(p.kind(), at)))).get(0) instanceof Answer.Chests c) {
            return new Chests(c.town(), byName(c.held()), c.free(), byName(c.short_()));
        }
        throw new IllegalStateException("the core answered Chests with something else");
    }

    private Map<String, Double> byName(List<Double> counts) {
        Map<String, Double> out = new LinkedHashMap<>();
        for (int good = 0; good < counts.size() && good < goods.size(); good++) {
            if (counts.get(good) != 0) {
                out.put(goods.get(good), counts.get(good));
            }
        }
        return out;
    }

    /** A desk's place as the core keeps it: the dimension's id and the block. */
    public static Point point(GlobalPos desk) {
        return PointsAdapter.point(PointsAdapter.Kind.DESK, desk);
    }
}
