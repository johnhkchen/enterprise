package dev.b28.enterprise.core;

import com.dylibso.chicory.runtime.ExportFunction;
import com.dylibso.chicory.runtime.ImportValues;
import com.dylibso.chicory.runtime.Instance;
import com.dylibso.chicory.runtime.Memory;
import com.dylibso.chicory.wasi.WasiOptions;
import com.dylibso.chicory.wasi.WasiPreview1;
import dev.b28.enterprise.wire.Command;
import dev.b28.enterprise.wire.PostcardIn;
import dev.b28.enterprise.wire.Request;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.Wire;
import dev.b28.enterprise.wire.WireException;

import java.io.ByteArrayOutputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

/**
 * The core, running: one society in one instance of its WebAssembly module (docs/binding-plan.md 1.1, 1.5). A
 * {@link Request} in, a {@link Response} out, through the module's four exports: {@code ent_alloc} a buffer, write the
 * request's postcard bytes, {@code ent_call}, read the response from where it says, {@code ent_free} both.
 *
 * <p><b>A trap.</b> The core is built with {@code panic = "abort"}: a panic (or any trap: a bad memory access, the
 * stack run out) leaves the instance unusable. {@code Core} keeps what it needs to rebuild it: the request that last set
 * the society whole (a {@code Create}, a {@code Load}, or a {@code Load} of the last {@code Saved} it passed back) and
 * every {@code Apply} the core took since. On a trap it drops the instance, makes a fresh one, replays those, and throws
 * {@link CoreTrap} with the request that trapped and what the core printed (a panic's message): the society is as it was
 * before that request, and the request isn't retried. If the replay traps too, or the core refuses what it once took,
 * the core is stuck: the trap says so, and every later call throws until the host loads a save into a new {@code Core}.
 * A bug report is the checkpoint, the requests since ({@link #replay()}), and the request that trapped.
 *
 * <p>Asking ({@code Ask}) and saving change nothing, so they aren't kept; nor is a refused request (nothing of it was
 * applied). A host that saves at each autosave keeps a day or so of looks: a few KB each.
 *
 * <p>Not thread-safe: one server thread calls it (Chicory instances aren't shared).
 */
public final class Core implements AutoCloseable {
    /** What the core prints to stderr (a panic's message) is kept to this many bytes. */
    private static final int MAX_PRINTED = 64 * 1024;

    private final CoreModule module;
    private Instance instance;
    private WasiPreview1 wasi;
    private Printed printed;
    private ExportFunction alloc;
    private ExportFunction call;
    private ExportFunction free;

    private byte[] checkpoint;
    private final List<byte[]> since = new ArrayList<>();
    private CoreTrap stuck;
    private int traps;
    private int trapsToMake;

    private Core(CoreModule module) {
        this.module = Objects.requireNonNull(module);
        instantiate();
    }

    /**
     * A fresh instance of {@code module}, with no society yet (send a {@code Create} or a {@code Load}).
     *
     * @throws IllegalStateException if the module was built for another ABI than this binding's generated wire
     */
    public static Core start(CoreModule module) {
        return new Core(module);
    }

    /** The module this core runs. */
    public CoreModule module() {
        return module;
    }

    /**
     * Runs {@code request}.
     *
     * @throws CoreTrap if the core trapped on it (the society is back as it was before it, unless {@link CoreTrap#stuck()})
     * @throws IllegalStateException if the core is stuck from an earlier trap
     */
    public Response call(Request request) {
        return Response.fromBytes(callBytes(request.toBytes()));
    }

    /**
     * Runs a request's postcard bytes as they are (for a scenario's raw bytes, or a host that encoded them itself) and
     * returns the response's.
     *
     * @throws CoreTrap if the core trapped on it
     * @throws IllegalStateException if the core is stuck from an earlier trap
     */
    public byte[] callBytes(byte[] request) {
        Objects.requireNonNull(request);
        if (stuck != null) {
            throw new IllegalStateException("the core is stuck since an earlier trap: load a save into a new Core", stuck);
        }
        byte[] out;
        try {
            out = invoke(request);
        } catch (RuntimeException | StackOverflowError e) {
            throw recover(request, e);
        }
        keep(request, out);
        return out;
    }

    /** The requests a trap would replay now: the checkpoint first (if there's a society), then each {@code Apply} since. */
    public List<byte[]> replay() {
        List<byte[]> all = new ArrayList<>(since.size() + 1);
        if (checkpoint != null) {
            all.add(checkpoint.clone());
        }
        since.forEach(b -> all.add(b.clone()));
        return all;
    }

    /** Traps recovered from so far. */
    public int traps() {
        return traps;
    }

    /** True once a trap couldn't be recovered from. */
    public boolean stuck() {
        return stuck != null;
    }

    /** The instance's linear memory, in bytes. */
    public long memoryBytes() {
        return (long) instance.memory().pages() * 65_536L;
    }

    @Override
    public void close() {
        dropInstance();
    }

    /** For tests: the next {@code calls} calls into the module trap (a read past its memory's end: a real trap). */
    void trapNext(int calls) {
        trapsToMake = calls;
    }

    // ---- the module ----

    private void instantiate() {
        printed = new Printed();
        wasi = WasiPreview1.builder()
                .withOptions(WasiOptions.builder().withStdout(printed).withStderr(printed).build())
                .build();
        Instance.Builder builder = Instance.builder(module.module())
                .withImportValues(ImportValues.builder().addFunction(wasi.toHostFunctions()).build());
        if (module.machine() != null) {
            builder.withMachineFactory(module.machine());
        }
        instance = builder.build();
        alloc = instance.export("ent_alloc");
        call = instance.export("ent_call");
        free = instance.export("ent_free");
        long abi = instance.export("ent_abi").apply()[0] & 0xFFFF_FFFFL;
        if (abi != Wire.ABI) {
            dropInstance();
            throw new IllegalStateException("the core's module is ABI " + abi + ", this binding's wire ABI " + Wire.ABI);
        }
    }

    private void dropInstance() {
        if (wasi != null) {
            wasi.close();
        }
        instance = null;
        wasi = null;
        alloc = call = free = null;
    }

    /** One crossing: the request into the module's memory, the call, the response out, both buffers freed. */
    private byte[] invoke(byte[] request) {
        Memory memory = instance.memory();
        if (trapsToMake > 0) {
            trapsToMake--;
            long past = (long) memory.pages() * 65_536L + 16;
            call.apply(past, 8);
        }
        int len = request.length;
        long ptr = alloc.apply(len)[0] & 0xFFFF_FFFFL;
        memory.write((int) ptr, request);
        long packed = call.apply(ptr, len)[0];
        long outPtr = packed >>> 32;
        int outLen = (int) (packed & 0xFFFF_FFFFL);
        byte[] out = memory.readBytes((int) outPtr, outLen);
        free.apply(outPtr, outLen);
        free.apply(ptr, len);
        return out;
    }

    // ---- what a trap replays ----

    /** Keeps what changed the society: a Create or Load taken (a new checkpoint), an Apply taken, a save given. */
    private void keep(byte[] request, byte[] response) {
        Response r;
        try {
            r = Response.fromBytes(response);
        } catch (WireException e) {
            return; // not this binding's schema: the core refused it, or a caller decodes it
        }
        switch (r) {
            case Response.Saved saved -> {
                checkpoint = Wire.request(new Command.Load(saved.save())).toBytes();
                since.clear();
            }
            case Response.Done done -> {
                switch (command(request)) {
                    case Command.Create c -> setCheckpoint(request);
                    case Command.Load l -> setCheckpoint(request);
                    case Command.Apply a -> since.add(request.clone());
                    case null, default -> { }
                }
            }
            case Response.Refused refused -> { }
        }
    }

    private void setCheckpoint(byte[] request) {
        checkpoint = request.clone();
        since.clear();
    }

    /** The request's command, read as the core reads it (bytes left over ignored), or null if it isn't one. */
    private static Command command(byte[] request) {
        try {
            return Request.decode(new PostcardIn(request)).command();
        } catch (WireException e) {
            return null;
        }
    }

    private CoreTrap recover(byte[] request, Throwable cause) {
        String said = printed.text();
        int replayed = 0;
        Throwable failed = null;
        try {
            dropInstance();
            instantiate();
            for (byte[] again : replay()) {
                Response r = Response.fromBytes(invoke(again));
                if (r instanceof Response.Refused refused) {
                    throw new IllegalStateException("the core refused on replay what it took before: " + refused.why());
                }
                replayed++;
            }
        } catch (RuntimeException | StackOverflowError e) {
            failed = e;
        }
        CoreTrap trap = new CoreTrap(request, said, replayed, failed != null, cause);
        if (failed != null) {
            trap.addSuppressed(failed);
            stuck = trap;
        } else {
            traps++;
        }
        return trap;
    }

    /** What the module prints (WASI stdout and stderr): an abort's panic message, the last {@link #MAX_PRINTED} bytes. */
    private static final class Printed extends OutputStream {
        private final ByteArrayOutputStream bytes = new ByteArrayOutputStream();

        @Override
        public void write(int b) {
            bytes.write(b);
            trim();
        }

        @Override
        public void write(byte[] b, int off, int len) {
            bytes.write(b, off, len);
            trim();
        }

        private void trim() {
            if (bytes.size() > 2 * MAX_PRINTED) {
                byte[] all = bytes.toByteArray();
                bytes.reset();
                bytes.write(all, all.length - MAX_PRINTED, MAX_PRINTED);
            }
        }

        String text() {
            return bytes.toString(StandardCharsets.UTF_8).strip();
        }
    }
}
