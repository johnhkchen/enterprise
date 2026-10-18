package dev.b28.enterprise.core;

import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.Command;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.SaveFile;

import java.io.IOException;
import java.lang.management.ManagementFactory;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.function.Supplier;

/**
 * The binding's spike (docs/binding-plan.md B1.6 and section 4): what the plan rests on, measured in this JVM. Run by
 * {@code just binding-spike}, which first writes the look scripts (the study host's {@code --wire-looks} and
 * {@code --world}) and plays them natively and under Node, so the three columns are the same requests on the same
 * module, and every response must be the same bytes on all three.
 *
 * <p>Writes the tables to {@code binding/build/spike.md} and prints them.
 */
public final class Spike {
    private static final int LOOKS_A_DAY = 20;
    private static final StringBuilder md = new StringBuilder();

    private Spike() {}

    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.ROOT);
        Path spike = Sim.sim().resolve("target/spike");
        if (args.length > 0 && args[0].equals("heap")) {
            heap(spike);
            return;
        }
        say("# Spike: the core in the JVM through Chicory 1.7.5");
        say("");
        say("JVM " + System.getProperty("java.vm.name") + " " + Runtime.version() + ", " + System.getProperty("os.arch")
                + ", " + Runtime.getRuntime().availableProcessors() + " cores, -Xmx"
                + Runtime.getRuntime().maxMemory() / (1 << 20) + "m; the machine's load average as it began "
                + String.format("%.1f", ManagementFactory.getOperatingSystemMXBean().getSystemLoadAverage())
                + " (other trees' builds and tests share it: wall times swing with it, a thread's CPU time much less)");
        say("");

        // ---- instantiation ----
        say("## Instantiation");
        say("");
        say("| Engine | Module ready (parse, load classes or compile) | First instance | Another instance (a trap's recovery) |");
        say("|---|---|---|---|");
        instantiation("compiled at build time (what the mod ships)", Sim::compiled);
        instantiation("interpreted", Sim::interpreted);
        instantiation("compiled at load (runtime compiler)", Sim::runtimeCompiled);
        say("");

        // ---- an empty call ----
        say("## A call's round trip");
        say("");
        say("| Engine | Ask with no questions | Ask for one town's stock |");
        say("|---|---|---|");
        emptyCall(Sim.compiled(), 20_000);
        emptyCall(Sim.interpreted(), 2_000);
        say("");

        // ---- looks ----
        say("## Looks (an Apply advancing a minute: `look_ticks` 1,200, twenty a day)");
        say("");
        say("Each script is a Load of a world's save, 7,300 looks (a year) and a Save, played natively (release build, the"
                + " module's own framing), under Node's V8 (the module itself) and here; every response is compared, byte"
                + " for byte, with the native one. A look's mean counts the looks that cross a day (where the day's work"
                + " runs) and those that don't; *a day* is twenty looks.");
        say("");
        say("| World | Host | Load | A look (mean) | Its CPU (the thread's) | p50 | p99 | Slowest | A day | Save | Same bytes as native |");
        say("|---|---|---|---|---|---|---|---|---|---|---|");
        Path study = spike.resolve("study.looks");
        Path world = spike.resolve("world.looks");
        String studyName = "study (9 towns, 14 routes, day 30; save " + kb(loadSize(study)) + ")";
        String worldName = "random world 1234 (25 towns, 80 routes, day 150; save " + kb(loadSize(world)) + ")";
        external(studyName, "native", spike.resolve("study.native.times"));
        external(studyName, "Node", spike.resolve("study.node.times"));
        play(studyName, "JVM, compiled", Sim::compiled, study, spike.resolve("study.native.out"), Integer.MAX_VALUE);
        play(studyName, "JVM, compiled at load", Sim::runtimeCompiled, study, spike.resolve("study.native.out"), 30 * LOOKS_A_DAY);
        play(studyName, "JVM, interpreted", Sim::interpreted, study, spike.resolve("study.native.out"), 3 * LOOKS_A_DAY);
        external(worldName, "native", spike.resolve("world.native.times"));
        external(worldName, "Node", spike.resolve("world.node.times"));
        play(worldName, "JVM, compiled", Sim::compiled, world, spike.resolve("world.native.out"), Integer.MAX_VALUE);
        play(worldName, "JVM, interpreted", Sim::interpreted, world, spike.resolve("world.native.out"), LOOKS_A_DAY);
        say("");
        say("(The JVM's runs that stop early play the Load, that many looks and the Save; their columns are over those.)");
        say("");

        // ---- the scenario world ----
        say("## A look on the scenario world (stocktake.json's one town, then looks)");
        say("");
        scenarioWorld(Sim.compiled(), 2_000);
        scenarioWorld(Sim.interpreted(), 200);
        say("");

        // ---- save and load ----
        say("## Save and load of the random world's " + kb(loadSize(world)) + " blob (compiled, mean of 50 after 10)");
        say("");
        saveAndLoad(world);
        say("");

        // ---- heap (measured in a JVM of its own first: `spike heap`) ----
        Path heap = Path.of("build/spike-heap.md");
        if (Files.exists(heap)) {
            for (String line : Files.readAllLines(heap)) {
                say(line);
            }
        }

        // ---- the compiler and the jar ----
        say("## What the build-time compiler left to the interpreter, and the jar");
        say("");
        Path report = Path.of(System.getProperty("enterprise.report"));
        for (String line : Files.readAllLines(report)) {
            say("    " + line);
        }
        say("");
        jar();

        Path out = Path.of("build/spike.md");
        Files.writeString(out, md.toString());
        System.out.println("\nwrote " + out.toAbsolutePath());
    }

    // ---- measurements ----

    /** In a JVM of its own: the heap the compiled core holds after the study's year, against the JVM's before it. */
    private static void heap(Path spike) throws IOException {
        long before = usedHeap();
        heapBefore = before;
        long[] heap = play("study", "JVM, compiled", Sim::compiled, spike.resolve("study.looks"),
                spike.resolve("study.native.out"), Integer.MAX_VALUE);
        md.setLength(0);
        say("## Heap after the study's 365-day run (compiled, a JVM of its own)");
        say("");
        say("Used heap after a full GC with the core's instance live, the year played: **" + mb(heap[0]) + "**, of which"
                + " the module's linear memory " + mb(heap[1]) + "; the JVM held " + mb(before) + " before the module"
                + " loaded. So the core costs about " + mb(heap[0] - before) + ".");
        say("");
        Files.writeString(Path.of("build/spike-heap.md"), md.toString());
    }

    private static long heapBefore = -1;

    private static void instantiation(String name, Supplier<CoreModule> make) {
        if (heapBefore < 0) {
            heapBefore = usedHeap();
        }
        long t0 = System.nanoTime();
        CoreModule module = make.get();
        long t1 = System.nanoTime();
        Core first = Core.start(module);
        long t2 = System.nanoTime();
        first.close();
        long best = Long.MAX_VALUE;
        for (int i = 0; i < 5; i++) {
            long s = System.nanoTime();
            Core.start(module).close();
            best = Math.min(best, System.nanoTime() - s);
        }
        say("| " + name + " | " + ms(t1 - t0) + " | " + ms(t2 - t1) + " | " + ms(best) + " |");
    }

    private static void emptyCall(CoreModule module, int n) {
        try (Core core = Core.start(module)) {
            created(core);
            byte[] empty = Sim.bytes(new Command.Ask(List.of()));
            byte[] stock = Sim.bytes(new Command.Ask(List.of(new Query.Stock(0))));
            for (int i = 0; i < n / 4; i++) {
                core.callBytes(empty);
                core.callBytes(stock);
            }
            long t = System.nanoTime();
            for (int i = 0; i < n; i++) {
                core.callBytes(empty);
            }
            long e = (System.nanoTime() - t) / n;
            t = System.nanoTime();
            for (int i = 0; i < n; i++) {
                core.callBytes(stock);
            }
            long s = (System.nanoTime() - t) / n;
            say("| " + module.name() + " | " + us(e) + " | " + us(s) + " |");
        }
    }

    /** A society of the scenarios' pack with one town at its desk, as stocktake.json starts. */
    private static void created(Core core) {
        Sim.Scenario stocktake = Sim.scenarios().stream().filter(s -> s.name().equals("stocktake")).findFirst().orElseThrow();
        SaveFile[] saves = new SaveFile[1];
        for (Sim.Step step : stocktake.steps()) {
            String wrong = Sim.play(core, step, saves);
            if (wrong != null) {
                throw new AssertionError(wrong);
            }
        }
    }

    private static void scenarioWorld(CoreModule module, int looks) {
        try (Core core = Core.start(module)) {
            created(core);
            Response r = core.call(Sim.request(new Command.Ask(List.of(new Query.Stock(0)))));
            if (!(r instanceof Response.Done d) || !(d.answers().get(0) instanceof Answer.Stock)) {
                throw new AssertionError(r.toString());
            }
            long now = 240_000; // the scenario's last advance: day 10
            long[] took = new long[looks];
            for (int i = 0; i < looks; i++) {
                byte[] look = Sim.bytes(new Command.Apply(List.of(new Input.Advance(now + (i + 1) * 1_200L))));
                long t = System.nanoTime();
                core.callBytes(look);
                took[i] = System.nanoTime() - t;
            }
            long[] steady = Arrays.copyOfRange(took, looks / 2, looks);
            say("- " + module.name() + ": " + ms(mean(steady)) + " a look (mean of the last " + steady.length + " of "
                    + looks + "), " + ms(mean(took)) + " over all of them");
        }
    }

    private static void saveAndLoad(Path script) throws IOException {
        List<byte[]> frames = frames(script);
        byte[] load = frames.get(0);
        byte[] save = Sim.bytes(new Command.Save());
        try (Core core = Core.start(Sim.compiled())) {
            long[] loads = new long[50];
            long[] saves = new long[50];
            int blob = 0;
            for (int i = -10; i < 50; i++) {
                long t = System.nanoTime();
                core.callBytes(load);
                long l = System.nanoTime() - t;
                t = System.nanoTime();
                Response r = Response.fromBytes(core.callBytes(save));
                long s = System.nanoTime() - t;
                blob = ((Response.Saved) r).save().blob().length();
                if (i >= 0) {
                    loads[i] = l;
                    saves[i] = s;
                }
            }
            say("- Load: " + ms(mean(loads)) + " (p99 " + ms(pct(loads, 0.99)) + "); Save: " + ms(mean(saves))
                    + " (p99 " + ms(pct(saves, 0.99)) + "), its blob " + kb(blob) + ", the Request's bytes "
                    + kb(load.length) + ". Both include the copy across the boundary and the Java-side postcard decoding.");
        }
    }

    /**
     * Plays a script through {@code module}: the Load, up to {@code most} looks, the Save. Every response must be the
     * native one's bytes (for a run cut short, the final Save is compared only if it played the whole year).
     */
    private static long[] play(String world, String host, Supplier<CoreModule> make, Path script, Path nativeOut, int most)
            throws IOException {
        List<byte[]> frames = frames(script);
        List<byte[]> want = frames(nativeOut);
        int looks = Math.min(most, frames.size() - 2);
        boolean whole = looks == frames.size() - 2;
        List<byte[]> playing = new ArrayList<>(frames.subList(0, 1 + looks));
        playing.add(frames.get(frames.size() - 1));
        long[] took = new long[playing.size()];
        int differ = 0;
        long[] heap = new long[3];
        java.lang.management.ThreadMXBean threads = ManagementFactory.getThreadMXBean();
        long cpu = 0;
        try (Core core = Core.start(make.get())) {
            for (int i = 0; i < playing.size(); i++) {
                long c = threads.getCurrentThreadCpuTime();
                long t = System.nanoTime();
                byte[] out = core.callBytes(playing.get(i));
                took[i] = System.nanoTime() - t;
                if (i > 0 && i < playing.size() - 1) {
                    cpu += threads.getCurrentThreadCpuTime() - c;
                }
                int w = i == playing.size() - 1 ? want.size() - 1 : i;
                if ((whole || i < playing.size() - 1) && !Arrays.equals(out, want.get(w))) {
                    differ++;
                }
            }
            if (whole) {
                heap[0] = usedHeap();
                heap[1] = core.memoryBytes();
                heap[2] = heapBefore;
            }
        }
        long[] lookTimes = Arrays.copyOfRange(took, 1, took.length - 1);
        String same = differ == 0 ? "yes (" + (whole ? playing.size() : playing.size() - 1) + ")" : "**no: " + differ + " differ**";
        row(world, host + (whole ? "" : " (" + looks + " looks)"), took[0], lookTimes, cpu / Math.max(1, looks),
                took[took.length - 1], same);
        return heap;
    }

    private static void external(String world, String host, Path times) throws IOException {
        long[] t = Files.readAllLines(times).stream().filter(l -> !l.isBlank()).mapToLong(Long::parseLong).toArray();
        row(world, host, t[0], Arrays.copyOfRange(t, 1, t.length - 1), -1, t[t.length - 1], "");
    }

    private static void row(String world, String host, long load, long[] looks, long cpu, long save, String same) {
        long day = mean(looks) * LOOKS_A_DAY;
        say("| " + world + " | " + host + " | " + ms(load) + " | " + ms(mean(looks)) + " | "
                + (cpu < 0 ? "" : ms(cpu)) + " | " + ms(pct(looks, 0.5)) + " | "
                + ms(pct(looks, 0.99)) + " | " + ms(Arrays.stream(looks).max().orElse(0)) + " | " + ms(day) + " | "
                + ms(save) + " | " + same + " |");
    }

    private static void jar() throws IOException {
        Path jar = Path.of(System.getProperty("enterprise.jar"));
        say("| What the mod's jar gains | Bytes |");
        say("|---|---|");
        long total = Files.size(jar);
        say("| binding.jar (the compiled machine, its meta module, the generated wire, Core) | " + String.format("%,d", total) + " |");
        for (String entry : System.getProperty("enterprise.runtime").split(java.io.File.pathSeparator)) {
            Path p = Path.of(entry);
            if (Files.isRegularFile(p)) {
                long size = Files.size(p);
                total += size;
                say("| " + p.getFileName() + " | " + String.format("%,d", size) + " |");
            }
        }
        say("| **all of it** | **" + String.format("%,d", total) + "** |");
    }

    // ---- reading ----

    /** A script or responses file: each frame a u32 little-endian length and its bytes. */
    static List<byte[]> frames(Path file) throws IOException {
        ByteBuffer in = ByteBuffer.wrap(Files.readAllBytes(file)).order(ByteOrder.LITTLE_ENDIAN);
        List<byte[]> frames = new ArrayList<>();
        while (in.hasRemaining()) {
            byte[] b = new byte[in.getInt()];
            in.get(b);
            frames.add(b);
        }
        return frames;
    }

    private static int loadSize(Path script) throws IOException {
        Command.Load load = (Command.Load) dev.b28.enterprise.wire.Request.fromBytes(frames(script).get(0)).command();
        return load.save().blob().length();
    }

    // ---- numbers ----

    private static long usedHeap() {
        for (int i = 0; i < 3; i++) {
            System.gc();
        }
        return ManagementFactory.getMemoryMXBean().getHeapMemoryUsage().getUsed();
    }

    private static long mean(long[] xs) {
        return xs.length == 0 ? 0 : (long) Arrays.stream(xs).average().orElse(0);
    }

    private static long pct(long[] xs, double p) {
        if (xs.length == 0) {
            return 0;
        }
        long[] s = xs.clone();
        Arrays.sort(s);
        return s[Math.min(s.length - 1, (int) Math.floor(p * s.length))];
    }

    private static String ms(long nanos) {
        double ms = nanos / 1e6;
        return ms >= 100 ? String.format("%.0f ms", ms) : ms >= 1 ? String.format("%.2f ms", ms) : String.format("%.3f ms", ms);
    }

    private static String us(long nanos) {
        return String.format("%.1f µs", nanos / 1e3);
    }

    private static String kb(long bytes) {
        return String.format("%.0f KB", bytes / 1000.0);
    }

    private static String mb(long bytes) {
        return String.format("%.0f MB", bytes / 1e6);
    }

    private static void say(String line) {
        System.out.println(line);
        md.append(line).append('\n');
    }
}
