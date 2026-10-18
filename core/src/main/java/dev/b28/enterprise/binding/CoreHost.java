package dev.b28.enterprise.binding;

import dev.b28.enterprise.Enterprise;
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
import net.neoforged.bus.api.IEventBus;
import net.neoforged.neoforge.event.level.LevelEvent;
import net.neoforged.neoforge.event.server.ServerStartedEvent;
import net.neoforged.neoforge.event.server.ServerStoppedEvent;

/**
 * The core in a running server (docs/binding-plan.md 1.5): one society in the Rust core, run on a thread of its own
 * ({@link CoreWorker}), saved with the world. The host makes the society from the mod's content pack at a new world
 * ({@link ModPack}, seeded with the world's seed) or loads the world's {@value SaveStore#FILE}, saves it when the world
 * saves, and drops it when the server stops. Its adapters ({@link StockAdapter}) turn Minecraft's happenings into the
 * core's inputs and its outputs into things in the world; {@link CoreClock} hands them a look at a time.
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
    private final StockAdapter stock;
    private final Outcomes outcomes;
    private final List<CompletableFuture<Response>> looks = new ArrayList<>();
    private String pausedWhy;
    private long looked = -1;

    private CoreHost(Path dir, long seed) {
        this.worker = CoreWorker.start(module());
        this.store = new SaveStore(dir, this::callNow);
        this.stock = new StockAdapter(this);
        this.outcomes = new Outcomes();
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
                stock.reload();
            }
        } catch (RuntimeException e) {
            worker.close();
            throw e;
        }
    }

    /**
     * A host for the world folder {@code dir}: its society loaded from {@value SaveStore#FILE} there, or made from the
     * mod's content pack with {@code seed} when there's none. The server's own is {@link #of}; a test makes its own.
     */
    public static CoreHost open(Path dir, long seed) {
        return new CoreHost(dir, seed);
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
        bus.addListener(StockAdapter::onPlaced);
        bus.addListener(StockAdapter::onBroken);
    }

    private static void onStarted(ServerStartedEvent event) {
        MinecraftServer server = event.getServer();
        long started = System.nanoTime();
        try {
            running = open(server.getWorldPath(LevelResource.ROOT), server.getWorldData().worldGenOptions().seed());
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

    /** The stock adapter: the desks the core keeps, their chests counted at each look. */
    public StockAdapter stock() {
        return stock;
    }

    /** Why the host is paused (the save refused, the core stuck), or null while it runs. */
    public String pausedWhy() {
        return pausedWhy;
    }

    /**
     * A look: the adapters' happenings since the last, then time passed to {@code now} (the overworld's game time), in
     * one {@code Apply} sent to the core's thread. Its outputs are shown when it's done ({@link #poll}). Nothing is sent
     * while paused, or twice for one tick.
     */
    public void look(MinecraftServer server, long now) {
        if (pausedWhy != null || now == looked) {
            return;
        }
        looked = now;
        List<Input> inputs = new ArrayList<>(stock.happenings(server));
        inputs.add(new Input.Advance(now));
        looks.add(worker.submit(Wire.request(new Command.Apply(inputs))));
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
        switch (r) {
            case Response.Done done -> done.outputs().forEach(outcomes::show);
            case Response.Refused refused -> Enterprise.LOGGER.warn("The core refused a look: {}", refused.why());
            case Response.Saved saved -> Enterprise.LOGGER.warn("A look answered with a save");
        }
    }

    /** Happenings applied now, outside a look (a desk registered): their outputs shown, refusals returned. */
    public Response apply(List<Input> inputs) {
        if (pausedWhy != null) {
            return null;
        }
        Response r = callNow(Wire.request(new Command.Apply(inputs)));
        if (r instanceof Response.Done done) {
            done.outputs().forEach(outcomes::show);
        }
        return r;
    }

    /**
     * Asks the core, after every look sent: the answers in the order asked.
     *
     * @throws IllegalStateException when the host is paused or the core refused the question
     */
    public List<Answer> ask(List<Query> queries) {
        if (pausedWhy != null) {
            throw new IllegalStateException(pausedWhy);
        }
        settle();
        return switch (callNow(Wire.request(new Command.Ask(queries)))) {
            case Response.Done done -> done.answers();
            case Response.Refused refused -> throw new IllegalStateException("the core refused " + queries + ": " + refused.why());
            case Response.Saved saved -> throw new IllegalStateException("a question answered with a save");
        };
    }

    /** Saves the society to the world's folder, after every look sent. False when paused (nothing written). */
    public boolean save() {
        if (pausedWhy != null) {
            return false;
        }
        settle();
        try {
            return store.save();
        } catch (RuntimeException e) {
            Enterprise.LOGGER.error("The core's save failed; the world's last save of it stands", e);
            return false;
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
