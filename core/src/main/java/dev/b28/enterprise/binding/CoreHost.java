package dev.b28.enterprise.binding;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.adapters.HandsAdapter;
import dev.b28.enterprise.binding.adapters.LandAdapter;
import dev.b28.enterprise.binding.adapters.LockerAdapter;
import dev.b28.enterprise.binding.adapters.MoneyAdapter;
import dev.b28.enterprise.binding.adapters.PeopleAdapter;
import dev.b28.enterprise.binding.adapters.PointsAdapter;
import dev.b28.enterprise.binding.adapters.StockAdapter;
import dev.b28.enterprise.core.CoreModule;
import dev.b28.enterprise.core.CoreTrap;
import dev.b28.enterprise.core.CoreWorker;
import dev.b28.enterprise.core.SaveStore;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.Command;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.Output;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Request;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.Wire;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.storage.LevelResource;
import net.neoforged.bus.api.EventPriority;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.neoforge.event.level.LevelEvent;
import net.neoforged.neoforge.event.server.ServerStartedEvent;
import net.neoforged.neoforge.event.server.ServerStoppedEvent;

/**
 * The core in a running server (docs/binding-plan.md 1.5): one society in the Rust core, run on a thread of its own
 * ({@link CoreWorker}), saved with the world. The host makes the society from the mod's content pack at a new world
 * ({@link ModPack}, seeded with the world's seed) or loads the world's {@value SaveStore#FILE}, saves it when the world
 * saves, and drops it when the server stops. Its adapters ({@link PointsAdapter}, {@link HandsAdapter}, {@link
 * PeopleAdapter}, {@link StockAdapter}, {@link MoneyAdapter}, {@link LandAdapter}, {@link LockerAdapter}) turn Minecraft's happenings into the core's inputs and its outputs into things
 * in the world; {@link CoreClock} hands them a look at a time. The host keeps no society ids: which block is which town's and which villager
 * is which hand are the core's, read back by the adapters when it starts.
 *
 * <p><b>The save is a file of its own, not a {@code SavedData}.</b> {@value SaveStore#FILE} sits in the world's folder
 * beside {@code level.dat}, written by {@link SaveStore} (whole, through a temporary file, with the migration's
 * {@value SaveStore#PREV} and the refusal that pauses the store) whenever the overworld saves (autosave, {@code
 * /save-all}, the server stopping). A {@code SavedData} would wrap the same bytes in NBT and need {@link SaveStore}'s
 * rules again around them; the file is in the world's folder either way, so copying a world copies its society.
 *
 * <p><b>Threads.</b> The server thread calls the host; the core runs on its own. A look is sent and its outputs shown a
 * tick or so later ({@link #poll}); a question or a registration waits for its answer (a few microseconds, or the rest
 * of a look already running). Requests run in the order sent, so the core sees the sequence the server sent.
 *
 * <p><b>Paused.</b> A save the core won't take (a newer core's) pauses the host: nothing is sent and nothing written
 * over the file, and {@link #pausedWhy} says why. A trap the core can't recover from pauses it too.
 */
public final class CoreHost implements AutoCloseable {
    /** How long the server thread waits for an answer before giving up on it: a look's worst, with room. */
    private static final long WAIT_SECONDS = 30;

    private static CoreModule module;
    private static CoreHost running;

    private final CoreWorker worker;
    private final SaveStore store;
    private final PointsAdapter points;
    private final HandsAdapter hands;
    private final PeopleAdapter people;
    private final StockAdapter stock;
    private final MoneyAdapter money;
    private final LandAdapter land;
    private final LockerAdapter lockers;
    private final Outcomes outcomes;
    private final List<CompletableFuture<Response>> looks = new ArrayList<>();
    private String pausedWhy;
    private long looked = -1;

    private CoreHost(Path dir, long seed, boolean ownsChests) {
        this.worker = CoreWorker.start(module());
        this.store = new SaveStore(dir, this::callNow);
        this.points = new PointsAdapter(this);
        this.hands = new HandsAdapter(this);
        this.people = new PeopleAdapter(this, ownsChests);
        this.stock = new StockAdapter(this, ownsChests);
        this.money = new MoneyAdapter(this, ownsChests);
        this.land = new LandAdapter(this, ownsChests);
        this.lockers = new LockerAdapter(this, ownsChests);
        this.outcomes = new Outcomes(people, stock, money, land);
        try {
            switch (store.open()) {
                case SaveStore.Opened.Fresh fresh -> {
                    if (callNow(Wire.request(new Command.Create(ModPack.of(), seed))) instanceof Response.Refused refused) {
                        pause("the core wouldn't take the mod's content pack: " + refused.why());
                    }
                }
                case SaveStore.Opened.Loaded loaded -> Enterprise.LOGGER.info("The core loaded {} (format {}, saved by {})",
                        dir.resolve(SaveStore.FILE), loaded.format(), loaded.file().core());
                case SaveStore.Opened.Refused refused -> pause(refused.message());
            }
            if (pausedWhy == null) {
                points.reload();
                hands.reload();
                stock.reload();
                land.reload();
            }
        } catch (RuntimeException e) {
            worker.close();
            throw e;
        }
    }

    /**
     * A host for the world folder {@code dir}: its society loaded from {@value SaveStore#FILE} there, or made from the
     * mod's content pack with {@code seed} when there's none, its moves written to the chests and its people's outcomes
     * to the villagers ({@code ownsChests}: the server's own in core mode) or every move sent back unmoved and no
     * villager spawned, discarded or written (Java mode, where the Java rules own the chests and the people). The
     * server's own is {@link #of}; a test makes its own.
     */
    public static CoreHost open(Path dir, long seed, boolean ownsChests) {
        return new CoreHost(dir, seed, ownsChests);
    }

    /** A host of its own for the world folder {@code dir}, writing its moves to the chests (a test's). */
    public static CoreHost open(Path dir, long seed) {
        return open(dir, seed, true);
    }

    /** The running server's host, or null when there's none (no server, or before it started). */
    public static CoreHost of(MinecraftServer server) {
        return running;
    }

    /** The module, compiled to JVM classes when the binding was built: parsed once, for every host. */
    private static synchronized CoreModule module() {
        if (module == null) {
            module = CoreModule.compiled();
        }
        return module;
    }

    /** The server's host follows its lifecycle: made when it has started, saved with the overworld, dropped at its stop. */
    public static void register(IEventBus bus) {
        bus.addListener(CoreHost::onStarted);
        bus.addListener(CoreHost::onSave);
        bus.addListener(CoreHost::onStopped);
        // Heard last, so a placement or break another listener cancels doesn't reach the core.
        bus.addListener(EventPriority.LOWEST, StockAdapter::onPlaced);
        bus.addListener(EventPriority.LOWEST, StockAdapter::onBroken);
        bus.addListener(EventPriority.LOWEST, HandsAdapter::onDeath);
        bus.addListener(StockAdapter::onChunkLoad);
        // After the Terrain survey of the chunk (heard at the normal priority).
        bus.addListener(EventPriority.LOWEST, LandAdapter::onChunkLoad);
        bus.addListener(EventPriority.LOWEST, LandAdapter::onChunkUnload);
    }

    private static void onStarted(ServerStartedEvent event) {
        MinecraftServer server = event.getServer();
        long started = System.nanoTime();
        try {
            running = open(server.getWorldPath(LevelResource.ROOT), server.getWorldData().worldGenOptions().seed(),
                    Society.isCore());
            running.stock().takeUnseenDesks();
            // The core brought to the world's time at once, on its own thread: a society new to an old world catches
            // up before its first desk, not in that desk's first look.
            running.look(server, server.overworld().getGameTime());
            if (!Society.isCore()) {
                // The Java rules read the world's dials; the core beside them follows what the world had set.
                dev.b28.enterprise.binding.adapters.DialAdapter.sync(running, dev.b28.enterprise.Dials.get(server).settings());
            }
            Enterprise.LOGGER.info("The core is running ({} ms to start)", (System.nanoTime() - started) / 1_000_000);
        } catch (RuntimeException | LinkageError e) {
            // The game runs on without the society the core keeps: the Java rules are what plays (Java mode).
            Enterprise.LOGGER.error("The core didn't start; the world runs without it", e);
            running = null;
        }
    }

    private static void onSave(LevelEvent.Save event) {
        if (running != null && event.getLevel() instanceof net.minecraft.server.level.ServerLevel level
                && level.dimension() == net.minecraft.world.level.Level.OVERWORLD) {
            long started = System.nanoTime();
            if (running.save()) {
                Enterprise.LOGGER.debug("The core saved in {} ms", (System.nanoTime() - started) / 1e6);
            }
        }
    }

    private static void onStopped(ServerStoppedEvent event) {
        if (running != null) {
            running.close();
            running = null;
        }
    }

    /** The points adapter: every trade point the core keeps, by place, with its town. */
    public PointsAdapter points() {
        return points;
    }

    /** The hands adapter: which villager is which of the core's hands. */
    public HandsAdapter hands() {
        return hands;
    }

    /** The people adapter: each town's census into the core, its villagers the core's hands' views. */
    public PeopleAdapter people() {
        return people;
    }

    /** The stock adapter: the desks and depots the core keeps, their chests counted at each look and written its moves. */
    public StockAdapter stock() {
        return stock;
    }

    /** The money adapter: the desks' chests and Blaze Bankers counted at each look, written the core's money. */
    public MoneyAdapter money() {
        return money;
    }

    /** What the host couldn't do of the core's moves (goods, then money) since the last look, for the next one. */
    private List<Input> pending() {
        List<Input> out = new ArrayList<>(stock.pending());
        out.addAll(money.pending());
        return out;
    }

    /** The land adapter: the world's ground into the core's map, its towns surveyed and their works laid. */
    public LandAdapter land() {
        return land;
    }

    /** The locker adapter: where the core's hands' lockers stand, and the Merchant's screen. */
    public LockerAdapter lockers() {
        return lockers;
    }

    /** Why the host is paused (the save refused, the core stuck), or null while it runs. */
    public String pausedWhy() {
        return pausedWhy;
    }

    /**
     * A look: the adapters' happenings since the last (what the host couldn't move, then the stocktakes and money counts), then time
     * passed to {@code now} (the overworld's game time), in one {@code Apply} sent to the core's thread. Its outputs are
     * shown when it's done ({@link #poll}). While an earlier look's moves are still to be shown no chest is counted (a
     * count would miss them and book them again): that look sends only its time. Nothing is sent while paused, or twice
     * for one tick.
     */
    public void look(MinecraftServer server, long now) {
        if (pausedWhy != null || now == looked) {
            return;
        }
        looked = now;
        if (looks.isEmpty()) {
            stock.joinDepots(server); // a building's containers the core doesn't keep yet, its depots now (counted as they join)
            lockers.place(server); // where the loaded villagers' lockers stand (core mode), before their chests are counted
        }
        List<Input> inputs = pending();
        // A census only in core mode: in Java mode the Java rules' census keeps the people (CensusTaker).
        inputs.addAll(people.happenings(server, Society.isCore()));
        // The ground in both modes; the towns surveyed (their crops, their bell) only in core mode, where their land is the core's.
        List<Input> landInputs = land.happenings(server, Society.isCore());
        inputs.addAll(landInputs);
        if (looks.isEmpty()) {
            inputs.addAll(money.withCash(stock.stocktakes(server)));
        }
        inputs.add(new Input.Advance(now));
        looks.add(land.watch(worker.submit(Wire.request(new Command.Apply(inputs))), landInputs));
    }

    /**
     * The chests in chunks loaded since the last tick counted at once, so what their towns moved while they were
     * unloaded is written before a player reaches them ({@link StockAdapter#catchUps}); sent like a look, with no time
     * passed. Not while a look's moves are still to be shown: the chunks wait for the next tick.
     */
    public void catchUp(MinecraftServer server) {
        if (pausedWhy != null || !looks.isEmpty()) {
            return;
        }
        List<Input> counts = money.withCash(stock.catchUps(server));
        if (!counts.isEmpty()) {
            List<Input> inputs = pending();
            inputs.addAll(counts);
            looks.add(worker.submit(Wire.request(new Command.Apply(inputs))));
        }
    }

    /** Shows the outputs of every look that's done, in the order sent: each to its adapter ({@link Outcomes}). */
    public void poll() {
        while (!looks.isEmpty() && looks.get(0).isDone()) {
            show(looks.remove(0));
        }
    }

    /** Waits for every look sent, and shows them: for a test, a save, or a command that must see the latest. */
    public void settle() {
        while (!looks.isEmpty()) {
            CompletableFuture<Response> look = looks.get(0);
            try {
                look.get(WAIT_SECONDS, TimeUnit.SECONDS);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                return;
            } catch (ExecutionException | TimeoutException e) {
                // shown (and logged) below
            }
            show(looks.remove(0));
        }
    }

    private void show(CompletableFuture<Response> look) {
        Response r;
        try {
            r = look.join();
        } catch (CompletionException e) {
            failed("a look", e.getCause());
            return;
        }
        hands.stale(); // a census, a birth or a death in the look: which villager is which hand is asked again
        switch (r) {
            case Response.Done done -> done.outputs().forEach(outcomes::show);
            case Response.Refused refused -> Enterprise.LOGGER.warn("The core refused a look: {}", refused.why());
            case Response.Saved saved -> Enterprise.LOGGER.warn("A look answered with a save");
        }
    }

    /**
     * Happenings applied now, outside a look (a desk registered), after every look sent is shown and what the host
     * couldn't move of it: their outputs shown, refusals returned.
     */
    public Response apply(List<Input> inputs) {
        if (pausedWhy != null) {
            return null;
        }
        settle();
        List<Input> all = pending();
        all.addAll(inputs);
        Response r = callNow(Wire.request(new Command.Apply(all)));
        if (r instanceof Response.Done done) {
            done.outputs().forEach(outcomes::show);
        }
        return r;
    }

    /**
     * Asks the core, after every look sent and what the host couldn't move of them: the answers in the order asked.
     *
     * @throws IllegalStateException when the host is paused or the core refused the question
     */
    public List<Answer> ask(List<Query> queries) {
        if (pausedWhy != null) {
            throw new IllegalStateException(pausedWhy);
        }
        settle();
        flush();
        return switch (callNow(Wire.request(new Command.Ask(queries)))) {
            case Response.Done done -> done.answers();
            case Response.Refused refused -> throw new IllegalStateException("the core refused " + queries + ": " + refused.why());
            case Response.Saved saved -> throw new IllegalStateException("a question answered with a save");
        };
    }

    /**
     * Saves the society to the world's folder, after every look sent, what the host couldn't move of them and the names
     * of the villagers just spawned for hands, so the save takes the chests to hold what they do and every villager the
     * world saves to be its hand's (a name left for the next look would be lost with a stop: the villager joined anew
     * as a second hand, and its hand spawned again). False when paused (nothing written).
     */
    public boolean save() {
        if (pausedWhy != null) {
            return false;
        }
        settle();
        flush();
        people.nameNow();
        land.sayNow(); // what was laid, said before the save: a stop before the next look would have it laid twice
        try {
            return store.save();
        } catch (RuntimeException e) {
            Enterprise.LOGGER.error("The core's save failed; the world's last save of it stands", e);
            return false;
        }
    }

    /** What the host couldn't move of the looks shown, told the core now (it has no stocktake: nothing moves back). */
    private void flush() {
        List<Input> unmoved = pending();
        if (!unmoved.isEmpty()) {
            apply(unmoved);
        }
    }

    @Override
    public void close() {
        settle();
        worker.close();
    }

    /** One request on the core's thread, after those sent before it, waited for. */
    private Response callNow(Request request) {
        try {
            return worker.submit(request).get(WAIT_SECONDS, TimeUnit.SECONDS);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException("interrupted waiting for the core", e);
        } catch (TimeoutException e) {
            throw new IllegalStateException("the core took more than " + WAIT_SECONDS + " s to answer", e);
        } catch (ExecutionException e) {
            failed("a request", e.getCause());
            throw e.getCause() instanceof RuntimeException r ? r : new IllegalStateException(e.getCause());
        }
    }

    private void failed(String what, Throwable cause) {
        if (cause instanceof CoreTrap trap) {
            Enterprise.LOGGER.error("The core trapped on {} ({}); {}", what, trap.getMessage(),
                    trap.stuck() ? "it can't recover: the society is paused" : "it is back as it was before it");
            if (trap.stuck()) {
                pause("the core trapped and couldn't recover: " + trap.getMessage());
            }
        } else {
            Enterprise.LOGGER.error("The core failed on {}", what, cause);
        }
    }

    private void pause(String why) {
        pausedWhy = why;
        Enterprise.LOGGER.warn("The core is paused: {}", why);
    }
}
