package dev.b28.enterprise.tool;

import com.dylibso.chicory.build.time.compiler.Config;
import com.dylibso.chicory.build.time.compiler.Generator;
import com.dylibso.chicory.compiler.InterpreterFallback;
import com.dylibso.chicory.wasm.Parser;
import com.dylibso.chicory.wasm.WasmModule;
import com.dylibso.chicory.wasm.types.NameCustomSection;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;
import java.util.stream.Stream;

/**
 * The core's WebAssembly module turned into JVM classes at build time (Chicory's build-time compiler, run by Gradle's
 * {@code compileCoreWasm} as a plain {@code JavaExec}: docs/binding-plan.md B1.6). Writes {@code EnterpriseCoreMachine}
 * and its function groups under {@code classes/}, and {@code EnterpriseCore.meta} under {@code meta/}: the module with
 * every compiled function's body emptied, which the runtime parses for its memory, data and tables, and which keeps the
 * body of any function too big for one JVM method (those run in the interpreter: {@code InterpreterFallback.WARN}).
 * The report says which, by name when a module with names is given.
 *
 * <p>Arguments: the module, the classes folder, the meta folder, the report file, and optionally the same module built
 * with its names (for the report only).
 */
public final class CompileCore {
    /** The compiled module's name: its classes are {@code EnterpriseCoreMachine*} in this package. */
    public static final String NAME = "dev.b28.enterprise.core.EnterpriseCore";

    public static void main(String[] args) throws IOException {
        if (args.length < 4) {
            throw new IllegalArgumentException("CompileCore WASM CLASSES META REPORT [NAMED_WASM]");
        }
        Path wasm = Path.of(args[0]);
        Path classes = Path.of(args[1]);
        Path meta = Path.of(args[2]);
        Path report = Path.of(args[3]);
        Path named = args.length > 4 ? Path.of(args[4]) : null;
        for (Path dir : List.of(classes, meta)) {
            clear(dir);
            Files.createDirectories(dir);
        }
        Path sources = Files.createTempDirectory("enterprise-core-sources");

        long started = System.nanoTime();
        Config config = Config.builder()
                .withWasmFile(wasm)
                .withName(NAME)
                .withTargetClassFolder(classes)
                .withTargetSourceFolder(sources)
                .withTargetWasmFolder(meta)
                .withInterpreterFallback(InterpreterFallback.WARN)
                .build();
        Generator generator = new Generator(config);
        Set<Integer> interpreted = new TreeSet<>(generator.generateResources());
        generator.generateMetaWasm(interpreted);
        long ms = (System.nanoTime() - started) / 1_000_000;
        clear(sources);

        WasmModule module = Parser.parse(wasm);
        int imported = module.importSection().importCount();
        int functions = module.codeSection().functionBodyCount();
        NameCustomSection names = null;
        if (named != null && Files.exists(named)) {
            WasmModule withNames = Parser.parse(named);
            if (withNames.codeSection().functionBodyCount() == functions) {
                names = withNames.nameSection();
            }
        }
        List<String> lines = new ArrayList<>();
        lines.add("module: " + wasm + " (" + Files.size(wasm) + " bytes)");
        lines.add("functions: " + functions + " (and " + imported + " imported)");
        lines.add("compiled in: " + ms + " ms");
        lines.add("classes: " + count(classes) + " files, " + size(classes) + " bytes");
        lines.add("meta: " + size(meta) + " bytes");
        lines.add("interpreted: " + interpreted.size());
        for (int id : interpreted) {
            int body = module.codeSection().getFunctionBody(id - imported).instructions().size();
            String name = names == null ? null : names.nameOfFunction(id);
            lines.add("  " + id + " (" + body + " instructions)" + (name == null ? "" : " " + name));
        }
        // The compiled functions HotSpot won't JIT (each wasm function is a method `func_N`).
        List<String> huge = new ArrayList<>();
        long hugeBytes = 0;
        try (Stream<Path> walk = Files.walk(classes)) {
            for (Path f : walk.filter(p -> p.toString().endsWith(".class")).sorted().toList()) {
                for (MethodSizes.Method m : MethodSizes.of(f)) {
                    if (m.bytes() > MethodSizes.HUGE) {
                        String fn = m.name().startsWith("func_") ? m.name().substring(5) : null;
                        String name = fn == null || names == null ? null : names.nameOfFunction(Integer.parseInt(fn));
                        huge.add("  " + f.getFileName().toString().replace(".class", "") + "." + m.name() + " ("
                                + m.bytes() + " bytes)" + (name == null ? "" : " " + name));
                        hugeBytes += m.bytes();
                    }
                }
            }
        }
        lines.add("too big for HotSpot to JIT (over " + MethodSizes.HUGE + " bytes of bytecode, so interpreted by the JVM): "
                + huge.size() + " methods, " + hugeBytes + " bytes");
        lines.addAll(huge);
        Files.createDirectories(report.getParent());
        Files.write(report, lines);
        lines.forEach(System.out::println);
        // A function Chicory couldn't fit in one JVM method runs in its interpreter, about 10x slower, and WARN alone
        // says so only in this report: farms::Store::goods is a few hundred bytes under the limit, so a core change
        // could push it over with no gate failing. Fail instead, unless the build says it knows (the spike's variants).
        if (!interpreted.isEmpty() && System.getenv("ENT_ALLOW_INTERPRETED") == null) {
            throw new IllegalStateException(interpreted.size() + " functions fell back to Chicory's interpreter (" + report
                    + "): split them in the core, or set ENT_ALLOW_INTERPRETED=1 to build anyway");
        }
    }

    private static void clear(Path dir) throws IOException {
        if (!Files.exists(dir)) {
            return;
        }
        try (Stream<Path> walk = Files.walk(dir)) {
            walk.sorted(Comparator.reverseOrder()).forEach(p -> {
                try {
                    Files.delete(p);
                } catch (IOException e) {
                    throw new UncheckedIOException(e);
                }
            });
        }
    }

    private static long count(Path dir) throws IOException {
        try (Stream<Path> walk = Files.walk(dir)) {
            return walk.filter(Files::isRegularFile).count();
        }
    }

    private static long size(Path dir) throws IOException {
        try (Stream<Path> walk = Files.walk(dir)) {
            return walk.filter(Files::isRegularFile).mapToLong(p -> {
                try {
                    return Files.size(p);
                } catch (IOException e) {
                    throw new UncheckedIOException(e);
                }
            }).sum();
        }
    }
}
