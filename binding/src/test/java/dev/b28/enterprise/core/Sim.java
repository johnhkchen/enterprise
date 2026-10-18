package dev.b28.enterprise.core;

import com.dylibso.chicory.compiler.MachineFactoryCompiler;
import com.dylibso.chicory.wasm.Parser;
import com.dylibso.chicory.wasm.WasmModule;
import dev.b28.enterprise.wire.Command;
import dev.b28.enterprise.wire.Refusal;
import dev.b28.enterprise.wire.Request;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.SaveFile;
import dev.b28.enterprise.wire.Wire;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.lang.reflect.Constructor;
import java.lang.reflect.RecordComponent;
import java.math.BigDecimal;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import java.util.stream.Stream;

/** What the tests share: where sim/ is, the core's module three ways, the scenarios, and how a host compares answers. */
final class Sim {
    private Sim() {}

    static Path sim() {
        String dir = System.getProperty("enterprise.sim");
        if (dir == null) {
            throw new IllegalStateException("enterprise.sim isn't set: run through Gradle (:binding:test)");
        }
        return Path.of(dir);
    }

    static Path wasm() {
        return Path.of(System.getProperty("enterprise.wasm"));
    }

    private static CoreModule compiled;
    private static CoreModule interpreted;
    private static CoreModule runtimeCompiled;

    /** The module the jar ships: compiled at build time. */
    static synchronized CoreModule compiled() {
        if (compiled == null) {
            compiled = CoreModule.compiled();
        }
        return compiled;
    }

    /** The module itself, in Chicory's interpreter. */
    static synchronized CoreModule interpreted() {
        if (interpreted == null) {
            interpreted = CoreModule.interpreted(wasm());
        }
        return interpreted;
    }

    /** The module compiled when it loads (Chicory's runtime compiler), not at build time. */
    static synchronized CoreModule runtimeCompiled() {
        if (runtimeCompiled == null) {
            runtimeCompiled = runtimeCompiled(wasm());
        }
        return runtimeCompiled;
    }

    static CoreModule runtimeCompiled(Path wasm) {
        WasmModule module = Parser.parse(wasm);
        return CoreModule.of("runtime-compiled", module, MachineFactoryCompiler.compile(module));
    }

    static byte[] unhex(String hex) {
        return HexFormat.of().parseHex(hex);
    }

    static String fnv(byte[] bytes) {
        long h = 0xcbf29ce484222325L;
        for (byte b : bytes) {
            h = (h ^ (b & 0xff)) * 0x100000001b3L;
        }
        return String.format("%016x", h);
    }

    // ---- scenarios ----

    /**
     * A step of a scenario: what to send (a request's bytes; raw bytes the core must refuse unread; or the last save
     * loaded) and what it must answer.
     */
    record Step(String note, byte[] request, boolean raw, boolean loadSaved, byte[] response, Map<String, Object> saved) {
        /** The request's bytes, given the last save a step gave. */
        byte[] bytes(SaveFile lastSave) {
            if (!loadSaved) {
                return request;
            }
            if (lastSave == null) {
                throw new AssertionError(note + ": no save to load yet");
            }
            return Wire.request(new Command.Load(lastSave)).toBytes();
        }
    }

    record Scenario(String name, List<Step> steps) {}

    static List<Scenario> scenarios() {
        try (Stream<Path> files = Files.list(sim().resolve("scenarios"))) {
            List<Scenario> all = new ArrayList<>();
            for (Path f : files.filter(p -> p.toString().endsWith(".json")).sorted().toList()) {
                all.add(scenario(f));
            }
            if (all.isEmpty()) {
                throw new AssertionError("no scenarios in sim/scenarios");
            }
            return all;
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    @SuppressWarnings("unchecked")
    static Scenario scenario(Path file) throws IOException {
        Map<String, Object> root = (Map<String, Object>) Json.parse(Files.readString(file));
        List<Step> steps = new ArrayList<>();
        for (Object o : (List<Object>) root.get("steps")) {
            Map<String, Object> s = (Map<String, Object>) o;
            String note = (String) s.get("note");
            byte[] request = null;
            boolean raw = false;
            boolean load = false;
            if (s.containsKey("request")) {
                request = unhex((String) required(s, "request_postcard", note));
            } else if (s.containsKey("request_hex")) {
                request = unhex((String) s.get("request_hex"));
                raw = true;
            } else if ("saved".equals(s.get("load"))) {
                load = true;
            } else {
                throw new AssertionError(note + ": a step sends a request, request_hex, or loads \"saved\"");
            }
            byte[] response = s.containsKey("response") ? unhex((String) required(s, "response_postcard", note)) : null;
            Map<String, Object> saved = (Map<String, Object>) s.get("saved");
            if ((response == null) == (saved == null)) {
                throw new AssertionError(note + ": a step expects a response or a save");
            }
            steps.add(new Step(note, request, raw, load, response, saved));
        }
        return new Scenario((String) root.get("name"), steps);
    }

    private static Object required(Map<String, Object> step, String key, String note) {
        Object v = step.get(key);
        if (v == null) {
            throw new AssertionError(note + ": no " + key + " (`just wire-fixtures` writes the scenarios' postcard twins)");
        }
        return v;
    }

    /**
     * Plays one step on {@code core}: null if it gave its expected answer, else what went wrong. A save it gives is
     * kept in {@code saves[0]} for the next {@code load}.
     */
    static String play(Core core, Step step, SaveFile[] saves) {
        Response got = Response.fromBytes(core.callBytes(step.bytes(saves[0])));
        if (step.response() != null) {
            Response want = Response.fromBytes(step.response());
            if (!withoutProse(got).equals(withoutProse(want))) {
                return step.note() + ": wanted " + want + ", got " + got;
            }
            return null;
        }
        if (!(got instanceof Response.Saved s)) {
            return step.note() + ": wanted a save, got " + got;
        }
        saves[0] = s.save();
        Map<String, Object> want = step.saved();
        byte[] blob = s.save().blob().toArray();
        boolean same = num(want, "format") == s.save().format()
                && ((BigDecimal) want.get("schema")).toBigInteger().longValue() == s.save().schema()
                && num(want, "blob_len") == blob.length
                && want.get("blob_fnv").equals(fnv(blob));
        if (!same) {
            return step.note() + ": wanted a save of " + want + ", got format " + s.save().format() + ", schema "
                    + Long.toUnsignedString(s.save().schema()) + ", " + blob.length + " bytes, fnv " + fnv(blob);
        }
        return null;
    }

    private static long num(Map<String, Object> m, String key) {
        return ((BigDecimal) m.get(key)).longValueExact();
    }

    /**
     * A refusal's {@code why} is prose for people (a library's error text, the core's wording), so a host compares a
     * refusal by its kind and numbers only: every String in it blanked (as sim/core/tests/scenarios.rs does), by the
     * record's own components, so a new refusal needs nothing here.
     */
    static Response withoutProse(Response r) {
        if (!(r instanceof Response.Refused refused)) {
            return r;
        }
        Refusal why = refused.why();
        RecordComponent[] parts = why.getClass().getRecordComponents();
        Object[] values = new Object[parts.length];
        Class<?>[] types = new Class<?>[parts.length];
        try {
            for (int i = 0; i < parts.length; i++) {
                types[i] = parts[i].getType();
                values[i] = types[i] == String.class ? "" : parts[i].getAccessor().invoke(why);
            }
            Constructor<?> make = why.getClass().getDeclaredConstructor(types);
            return new Response.Refused((Refusal) make.newInstance(values));
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }

    /** A request for {@code command}, as bytes. */
    static byte[] bytes(Command command) {
        return Wire.request(command).toBytes();
    }

    static Request request(Command command) {
        return Wire.request(command);
    }
}
