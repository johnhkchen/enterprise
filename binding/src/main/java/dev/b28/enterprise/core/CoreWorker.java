package dev.b28.enterprise.core;

import dev.b28.enterprise.wire.Request;
import dev.b28.enterprise.wire.Response;

import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.function.Function;

/**
 * A {@link Core} on a thread of its own, so a look never runs on the server's tick (the spike, docs/binding-plan.md
 * section 4: a compiled look on the study world takes a couple of milliseconds in HotSpot, the look that crosses a day
 * tens, where the tick has 50 in all). The server thread gathers a look's happenings, {@link #submit}s the request and
 * carries on; the outputs come back on the future, and the server thread shows them when it next looks (a tick later,
 * as a chest's sync is anyway).
 *
 * <p>Requests run one at a time, in the order submitted, on the one thread that owns the core: the core sees exactly the
 * sequence the server sent, so the same inputs still give the same world. A trap ({@link CoreTrap}) completes that
 * request's future exceptionally; the core has recovered by then, and the next request runs as usual.
 */
public final class CoreWorker implements AutoCloseable {
    private final ExecutorService thread;
    private final Core core;

    private CoreWorker(CoreModule module) {
        this.thread = Executors.newSingleThreadExecutor(r -> {
            Thread t = new Thread(r, "enterprise-core");
            t.setDaemon(true);
            return t;
        });
        try {
            this.core = thread.submit(() -> Core.start(module)).get();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            thread.shutdownNow();
            throw new IllegalStateException("interrupted starting the core", e);
        } catch (java.util.concurrent.ExecutionException e) {
            thread.shutdownNow();
            throw e.getCause() instanceof RuntimeException r ? r : new IllegalStateException(e.getCause());
        }
    }

    /** A core of {@code module} on a thread of its own, started (no society yet). */
    public static CoreWorker start(CoreModule module) {
        return new CoreWorker(module);
    }

    /** Runs {@code request} on the core's thread, after every request submitted before it. */
    public CompletableFuture<Response> submit(Request request) {
        return CompletableFuture.supplyAsync(() -> core.call(request), thread);
    }

    /** Runs request bytes as they are (see {@link Core#callBytes}), in order with the rest. */
    public CompletableFuture<byte[]> submitBytes(byte[] request) {
        byte[] copy = request.clone();
        return CompletableFuture.supplyAsync(() -> core.callBytes(copy), thread);
    }

    /** Reads the core on its own thread, in order with the requests (its replay log, its traps, its memory). */
    public <T> CompletableFuture<T> inspect(Function<Core, T> read) {
        return CompletableFuture.supplyAsync(() -> read.apply(core), thread);
    }

    /** Runs what was submitted, then stops the thread and drops the core. */
    @Override
    public void close() {
        thread.execute(core::close);
        thread.shutdown();
        try {
            if (!thread.awaitTermination(30, TimeUnit.SECONDS)) {
                thread.shutdownNow();
            }
        } catch (InterruptedException e) {
            thread.shutdownNow();
            Thread.currentThread().interrupt();
        }
    }
}
