package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.caravan.Squad;
import dev.b28.enterprise.merchant.MerchantView;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.MarketLine;
import dev.b28.enterprise.wire.Output;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.RouteLine;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtAccounter;
import net.minecraft.nbt.NbtIo;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.neoforged.neoforge.server.ServerLifecycleHooks;

/**
 * A town's wagons, from the core (docs/binding-plan.md 2, concern 8). The caravans are the core's: which routes a town
 * runs, when a wagon leaves, who goes with it, what it carries, sells and brings home ({@code squad.rs}, {@code
 * rules/plan.rs}, {@code wagon_host.rs}); a wagon has no entity in the world, as it has none in the Java rules. What the
 * world shows of it is its crew, and this adapter shows that:
 *
 * <ul>
 *   <li><b>Stowed by hand</b>: a hand gone away ({@code HandAway}, a wagon's crew or porter, a soldier) takes its
 *       villager off the world into a record ({@link Squad#stow}), kept by the hand's id; a villager not loaded then is
 *       stowed when its chunk is, at a look ({@link #catchUp}). A hand back ({@code HandBack}) has its villager let out
 *       by its town's desk ({@link Squad#release}, as the Java rules' {@code walkersHome}/{@code arriveHome} do), with the
 *       trade, level and XP the core has for it now (a hand taken on as a travelling Merchant comes home one); a desk not
 *       loaded keeps it until it is. A hand back the host never named has no villager to let out: the people adapter
 *       spawns one and names it, as for a hand born. A record that won't load is made again, a villager of the hand's
 *       trade under its UUID.
 *   <li><b>Started and stopped</b> ({@link #send}, {@code Input.Route}): the Merchant's screen ({@code
 *       CommissionPayload}) and {@code /enterprise route add}/{@code remove} in core mode; whether a far end is within a
 *       leg's food is the core's ({@code Answer.Routes}), never {@code Caravans.inRange}.
 *   <li><b>The screen</b> ({@link #caravans}): the markets the desk's town knows, nearest first, with how far, which
 *       way, whether its wagons go there and how they're doing, and its squad at home ({@code Query.Routes}).
 * </ul>
 *
 * <p>The stowed villagers are the view's own save ({@value #FILE} beside the core's, written when the host saves): the
 * core keeps who's away, this keeps what they looked like. The adapter keeps no routes and no ids.
 *
 * <p>Not here (John, 2026-10-18: pallets live in the core only): the crates a wagon carries and their papers are the
 * core's pallets, drawn by {@code pallet-views} from the pallet outputs, with no crate or paper rules in Java.
 *
 * <p>In Java mode the Java rules stow and release their own crews ({@code Caravans}); this adapter shows nothing.
 */
public final class WagonAdapter {
    /** The stowed villagers' file, in the world's folder beside the core's save. */
    public static final String FILE = "enterprise_core_stowed.dat";

    private final CoreHost host;
    /** The core's outcomes are shown in the world: the server's host in core mode, or a test's. */
    private final boolean ownsVillagers;
    private final Path file;
    /** Hands away, by id: the villager as it left ({@code null} until it's stowed: not loaded when it went). */
    private final Map<Long, Away> away = new LinkedHashMap<>();
    /** Hands back whose villager waits for its town's desk to load, by id. */
    private final Map<Long, Back> back = new LinkedHashMap<>();
    private boolean dirty;

    private record Away(long town, String tag, CompoundTag entity) {}

    private record Back(long town, String tag, CompoundTag entity, String trade, int level, long xp) {}

    public WagonAdapter(CoreHost host, boolean ownsVillagers, Path dir) {
        this.host = host;
        this.ownsVillagers = ownsVillagers;
        this.file = dir.resolve(FILE);
        read();
    }

    /** Shows a wagon's outcome: its crew stowed or let out; a wagon's own comings and goings have nothing to show. */
    public void show(Output output) {
        MinecraftServer server = ServerLifecycleHooks.getCurrentServer();
        if (server == null || !ownsVillagers) {
            return;
        }
        switch (output) {
            case Output.HandAway o -> goes(server, o);
            case Output.HandBack o -> comes(server, o);
            case Output.WagonLeft o -> Enterprise.LOGGER.debug("Route {}'s wagon leaves with {} stacks", o.route(), o.stacks());
            case Output.WagonSold o -> Enterprise.LOGGER.debug("Route {}'s wagon sold {} at the far end", o.route(), o.sold());
            case Output.WagonHome o -> Enterprise.LOGGER.debug("Route {}'s wagon is home", o.route());
            default -> {}
        }
    }

    private void goes(MinecraftServer server, Output.HandAway o) {
        CompoundTag stowed = find(server, o.tag()) instanceof Villager v ? Squad.stow(v) : null;
        Away kept = away.get(o.hand());
        if (stowed == null && kept != null && kept.entity() != null) {
            stowed = kept.entity(); // away again by the core's books (its save and this file out of step): keep the villager
        }
        away.put(o.hand(), new Away(o.town(), o.tag(), stowed));
        dirty = true;
        host.hands().stale();
    }

    private void comes(MinecraftServer server, Output.HandBack o) {
        Away was = away.remove(o.hand());
        dirty = true;
        if (was != null && was.entity() == null && !o.tag().isEmpty()) {
            // Never stowed (not loaded while it was away): it's in the world, or will be when its chunk loads.
            if (find(server, o.tag()) instanceof Villager v) {
                host.people().writeTo(v, o.trade(), o.level(), o.xp());
            }
            return;
        }
        back.put(o.hand(), new Back(o.town(), o.tag(), was == null ? null : was.entity(), o.trade(), o.level(), o.xp()));
        release(server, o.hand(), false);
    }

    /** The hand is no more (its villager, if kept, is gone with it). */
    public void forget(long hand) {
        boolean wasAway = away.remove(hand) != null;
        boolean wasBack = back.remove(hand) != null;
        dirty |= wasAway || wasBack;
    }

    /**
     * At a look, before the census: each villager of a hand away that wasn't loaded when it went is stowed if it's
     * loaded now, and each hand back whose desk wasn't loaded is let out if it is now.
     */
    public void catchUp(MinecraftServer server) {
        if (!ownsVillagers) {
            return;
        }
        for (var e : List.copyOf(away.entrySet())) {
            if (e.getValue().entity() == null && find(server, e.getValue().tag()) instanceof Villager v) {
                away.put(e.getKey(), new Away(e.getValue().town(), e.getValue().tag(), Squad.stow(v)));
                dirty = true;
            }
        }
        for (long hand : List.copyOf(back.keySet())) {
            release(server, hand, true);
        }
    }

    /**
     * The hand's villager let out by its town's desk, if the desk is loaded (else it waits, kept). Let out at a look
     * ({@code now}), it takes the trade, level and XP the core has for the hand then, not those it came home with: the
     * core's looks go on while the desk isn't loaded, and a {@code HandLevel} meanwhile had no villager to go to.
     */
    private void release(MinecraftServer server, long hand, boolean now) {
        Back b = back.get(hand);
        GlobalPos desk = desk(b.town());
        ServerLevel level = desk == null ? null : server.getLevel(desk.dimension());
        if (level == null || !level.isLoaded(desk.pos())) {
            return;
        }
        if (now) {
            b = asNow(hand, b);
        }
        back.remove(hand);
        dirty = true;
        Entity out = b.entity() == null ? null : Squad.release(level, desk.pos(), b.entity()).orElse(null);
        if (out == null && !b.tag().isEmpty()) {
            out = find(server, b.tag()); // back somehow already (a release refused for a villager of its UUID there)
            if (out == null && b.entity() != null) {
                // Its record unreadable: a villager for the hand under its own name. (A hand back that was never seen
                // going has no record: its villager is in the world, perhaps where nothing is loaded, and none is made.)
                out = host.people().spawnFor(level, desk.pos(), hand, b.tag(), b.trade(), b.level(), b.xp());
            }
        }
        if (out instanceof Villager v) {
            host.people().writeTo(v, b.trade(), b.level(), b.xp());
        }
        host.hands().stale();
    }

    /** The hand back with its trade, level and XP as the core has them now (as it came home if the core can't say). */
    private Back asNow(long hand, Back b) {
        try {
            for (HandsAdapter.Hand h : host.hands().roster(b.town()).hands()) {
                if (h.id() == hand) {
                    return new Back(b.town(), b.tag(), b.entity(), h.trade(), h.level(), h.xp());
                }
            }
        } catch (IllegalStateException e) {
            // The host paused: the trade it came home with.
        }
        return b;
    }

    /** The town's desk, the first the core keeps for it. */
    private GlobalPos desk(long town) {
        for (var e : host.points().of(PointsAdapter.Kind.DESK).entrySet()) {
            if (e.getValue() == town) {
                return e.getKey();
            }
        }
        return null;
    }

    /** How many hands are away with their villagers kept, and how many back wait for their desk: for a command or test. */
    public int[] kept() {
        return new int[] {(int) away.values().stream().filter(a -> a.entity() != null).count(), back.size()};
    }

    // The screen and the commands.

    /** A town's routes, the markets it knows and its squad at home, as the core has them. */
    public record Town(long town, List<RouteLine> routes, List<MarketLine> markets, List<String> squad) {}

    /** A town's routes now. */
    public Town town(long town) {
        if (host.ask(List.of(new Query.Routes(town))).get(0) instanceof Answer.Routes a) {
            return new Town(a.town(), a.routes(), a.markets(), a.squad());
        }
        throw new IllegalStateException("the core answered Routes with something else");
    }

    /**
     * Starts ({@code send}) or stops the wagons from one town to another.
     *
     * @return the route open from one to the other now, if any
     * @throws IllegalStateException when the core refuses it (a town it hasn't, a town to itself)
     */
    public Optional<RouteLine> send(long from, long to, boolean send) {
        Response r = host.apply(List.of(new Input.Route(from, to, send)));
        if (r instanceof Response.Refused refused) {
            throw new IllegalStateException("the core wouldn't route " + from + " to " + to + ": " + refused.why());
        }
        return town(from).routes().stream().filter(l -> l.from() == from && l.to() == to && l.open()).findFirst();
    }

    /** The route by its number, wherever it runs (from a town that has a point). */
    public Optional<RouteLine> route(long id) {
        for (long town : host.points().towns()) {
            for (RouteLine l : town(town).routes()) {
                if (l.route() == id) {
                    return Optional.of(l);
                }
            }
        }
        return Optional.empty();
    }

    /** Every route, by number, from the towns that have a point. */
    public List<RouteLine> routes() {
        Map<Long, RouteLine> all = new LinkedHashMap<>();
        for (long town : host.points().towns()) {
            town(town).routes().forEach(l -> all.putIfAbsent(l.route(), l));
        }
        List<RouteLine> out = new ArrayList<>(all.values());
        out.sort(java.util.Comparator.comparingLong(RouteLine::route));
        return out;
    }

    /** The desk of a town as a block, if the core has one for it. */
    public Optional<BlockPos> deskOf(long town) {
        return Optional.ofNullable(desk(town)).map(GlobalPos::pos);
    }

    /**
     * The Merchant's screen's caravans for the desk at {@code desk} (core mode): the markets its town knows, nearest
     * first, and its squad at home. Empty when the core keeps no town there.
     */
    public MerchantView.CaravanView caravans(ServerLevel level, BlockPos desk) {
        Long town = host.points().townAt(GlobalPos.of(level.dimension(), desk));
        if (town == null) {
            return new MerchantView.CaravanView(List.of(), List.of());
        }
        Town t = town(town);
        List<MerchantView.TownView> towns = new ArrayList<>();
        for (MarketLine m : t.markets()) {
            if (m.desk().isEmpty()) {
                continue; // a town the host never placed: nowhere to show
            }
            BlockPos at = new BlockPos(m.desk().get().x(), m.desk().get().y(), m.desk().get().z());
            String status = t.routes().stream().filter(l -> l.from() == town && l.to() == m.town() && l.open())
                    .map(RouteLine::status).findFirst().orElse("");
            towns.add(new MerchantView.TownView(at, m.blocks().map(Long::intValue).orElse(0),
                    MerchantView.direction(at.getX() - desk.getX(), at.getZ() - desk.getZ()), m.settlement(), m.routed(),
                    status, m.reachable()));
        }
        List<String> squad = t.squad().stream().map(this::professionId).toList();
        return new MerchantView.CaravanView(towns, squad);
    }

    /**
     * The screen's "Send caravans" or "Stop" for the market at {@code destination}, from the desk at {@code desk}: only
     * a market the core says the town knows, started only if a leg's food reaches it. True if anything changed.
     */
    public boolean commission(ServerLevel level, BlockPos desk, BlockPos destination, boolean send) {
        Long town = host.points().townAt(GlobalPos.of(level.dimension(), desk));
        if (town == null) {
            return false;
        }
        Optional<MarketLine> market = town(town).markets().stream()
                .filter(m -> m.desk().isPresent() && m.desk().get().x() == destination.getX()
                        && m.desk().get().y() == destination.getY() && m.desk().get().z() == destination.getZ())
                .findFirst();
        if (market.isEmpty()) {
            return false; // only markets the screen was shown
        }
        MarketLine m = market.get();
        if (send && !m.routed() && m.reachable()) {
            return send(town, m.town(), true).isPresent();
        }
        if (!send && m.routed()) {
            send(town, m.town(), false);
            return true;
        }
        return false;
    }

    /** A route in a line, for a command: its ends, its road, its wagons and where it stands. */
    public static String describe(RouteLine l) {
        String wagon = l.wagon().map(w -> " | its wagon %s, there at tick %d, %d stack(s), crew hand(s) %s%s".formatted(
                w.out() ? "out" : "home", w.arrives(), w.stacks(), w.crew(), w.porter() ? ", a porter" : "")).orElse("");
        return "#%d town %d -> town %d: %d blocks, %.2f days a leg, a wagon every %.1f days%s, %d trip(s), %.1f sold; %s%s"
                .formatted(l.route(), l.from(), l.to(), l.blocks(), l.days(), l.everyDays(), l.onCall() ? " (on call)" : "",
                        l.trips(), l.sold(), l.status(), wagon);
    }

    /** A town's routes, the markets it knows and its squad at home, a line each, for a command. */
    public List<String> lines(long town) {
        Town t = town(town);
        List<String> out = new ArrayList<>();
        out.add("The core's town %d: %d route(s), %d market(s) known, its squad at home: %s".formatted(town,
                t.routes().size(), t.markets().size(), t.squad().isEmpty() ? "none" : String.join(", ", t.squad())));
        t.routes().forEach(l -> out.add("  " + describe(l)));
        for (MarketLine m : t.markets()) {
            out.add("  market: town %d%s, %s%s%s%s".formatted(m.town(),
                    m.desk().map(d -> " at %d %d %d".formatted(d.x(), d.y(), d.z())).orElse(""),
                    m.blocks().map(b -> b + " blocks").orElse("how far unknown"), m.settlement() ? ", a settlement" : "",
                    m.reachable() ? "" : ", beyond a leg's food", m.routed() ? ", wagons go there" : ""));
        }
        return out;
    }

    /**
     * {@code /enterprise route add} in core mode: wagons started from one town to another; what came of it, in words.
     *
     * @return the route's number and the words, or a negative number when none opened
     */
    public Map.Entry<Long, String> addRoute(long from, long to) {
        try {
            Optional<RouteLine> r = send(from, to, true);
            if (r.isPresent()) {
                return Map.entry(r.get().route(), "Route " + describe(r.get()));
            }
            Optional<MarketLine> m = town(from).markets().stream().filter(x -> x.town() == to).findFirst();
            return Map.entry(-1L, m.isPresent() && !m.get().reachable()
                    ? "Too far: %s blocks needs more food than a caravan carries. Add a settlement in between as a waystation."
                            .formatted(m.get().blocks().map(String::valueOf).orElse("?"))
                    : "The core opened no route from town %d to town %d (beyond a leg's food, or no way there)".formatted(from, to));
        } catch (IllegalStateException e) {
            return Map.entry(-1L, e.getMessage());
        }
    }

    /** {@code /enterprise route remove} in core mode: the route by its number wound down. False if there's none open. */
    public boolean removeRoute(long id) {
        Optional<RouteLine> r = route(id).filter(RouteLine::open);
        r.ifPresent(l -> send(l.from(), l.to(), false));
        return r.isPresent();
    }

    /** A trade by the core's name as the screen names professions (a registry id: {@code minecraft:librarian}). */
    private String professionId(String trade) {
        VillagerProfession p = host.people().profession(trade);
        return p == null ? trade : BuiltInRegistries.VILLAGER_PROFESSION.getKey(p).toString();
    }

    private static Entity find(MinecraftServer server, String tag) {
        UUID id;
        try {
            id = tag.isEmpty() ? null : UUID.fromString(tag);
        } catch (IllegalArgumentException e) {
            return null;
        }
        if (id == null) {
            return null;
        }
        for (ServerLevel level : server.getAllLevels()) {
            Entity e = level.getEntity(id);
            if (e != null) {
                return e;
            }
        }
        return null;
    }

    // The view's save.

    /** Writes the stowed villagers beside the core's save (whole, through a temporary file), if anything changed. */
    public void save() {
        if (!dirty) {
            return;
        }
        CompoundTag root = new CompoundTag();
        ListTag aways = new ListTag();
        away.forEach((hand, a) -> {
            CompoundTag t = new CompoundTag();
            t.putLong("hand", hand);
            t.putLong("town", a.town());
            t.putString("tag", a.tag());
            if (a.entity() != null) {
                t.put("entity", a.entity());
            }
            aways.add(t);
        });
        ListTag backs = new ListTag();
        back.forEach((hand, b) -> {
            CompoundTag t = new CompoundTag();
            t.putLong("hand", hand);
            t.putLong("town", b.town());
            t.putString("tag", b.tag());
            if (b.entity() != null) {
                t.put("entity", b.entity());
            }
            t.putString("trade", b.trade());
            t.putInt("level", b.level());
            t.putLong("xp", b.xp());
            backs.add(t);
        });
        root.put("away", aways);
        root.put("back", backs);
        try {
            Path tmp = file.resolveSibling(FILE + ".tmp");
            NbtIo.writeCompressed(root, tmp);
            Files.move(tmp, file, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
            dirty = false;
        } catch (IOException e) {
            Enterprise.LOGGER.error("The stowed villagers weren't saved to {}", file, e);
        }
    }

    private void read() {
        if (!Files.exists(file)) {
            return;
        }
        try {
            CompoundTag root = NbtIo.readCompressed(file, NbtAccounter.unlimitedHeap());
            for (Tag x : root.getList("away", Tag.TAG_COMPOUND)) {
                CompoundTag t = (CompoundTag) x;
                away.put(t.getLong("hand"), new Away(t.getLong("town"), t.getString("tag"),
                        t.contains("entity") ? t.getCompound("entity") : null));
            }
            for (Tag x : root.getList("back", Tag.TAG_COMPOUND)) {
                CompoundTag t = (CompoundTag) x;
                back.put(t.getLong("hand"), new Back(t.getLong("town"), t.getString("tag"),
                        t.contains("entity") ? t.getCompound("entity") : null, t.getString("trade"), t.getInt("level"),
                        t.getLong("xp")));
            }
        } catch (IOException e) {
            Enterprise.LOGGER.error("The stowed villagers in {} couldn't be read", file, e);
        }
    }
}
