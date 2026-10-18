package dev.b28.enterprise.core;

import com.dylibso.chicory.runtime.Instance;
import com.dylibso.chicory.runtime.Machine;
import com.dylibso.chicory.wasm.Parser;
import com.dylibso.chicory.wasm.WasmModule;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.nio.file.Path;
import java.util.Objects;
import java.util.function.Function;

/**
 * The core's WebAssembly module, parsed once, and how its functions run: the module a {@link Core} instantiates (again
 * after a trap). Parsing is the slow part of starting, so one is made per server and kept.
 *
 * <ul>
 *   <li>{@link #compiled()}: what the mod ships. The module turned into JVM classes when the binding was built
 *       ({@code compileCoreWasm}: Chicory's build-time compiler), with the meta module they run against.
 *   <li>{@link #interpreted(Path)}: the module itself in Chicory's interpreter. Slow; for measuring, and for a module
 *       built after the jar.
 *   <li>{@link #of}: any other way (the tests' runtime compiler).
 * </ul>
 */
public final class CoreModule {
    /** The compiled machine's class, written by {@code CompileCore} beside its meta module. */
    static final String MACHINE = "dev.b28.enterprise.core.EnterpriseCoreMachine";
    static final String META = "EnterpriseCore.meta";

    private final String name;
    private final WasmModule module;
    private final Function<Instance, Machine> machine;

    private CoreModule(String name, WasmModule module, Function<Instance, Machine> machine) {
        this.name = Objects.requireNonNull(name);
        this.module = Objects.requireNonNull(module);
        this.machine = machine;
    }

    /**
     * The module compiled at build time, from this jar.
     *
     * @throws IllegalStateException if the jar was built without it
     */
    public static CoreModule compiled() {
        WasmModule meta;
        try (InputStream in = CoreModule.class.getResourceAsStream(META)) {
            if (in == null) {
                throw new IllegalStateException(META + " isn't in the binding's jar: build it with :binding:compileCoreWasm");
            }
            meta = Parser.parse(in);
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
        MethodHandle make;
        try {
            Class<?> type = Class.forName(MACHINE, true, CoreModule.class.getClassLoader());
            make = MethodHandles.publicLookup().findConstructor(type, MethodType.methodType(void.class, Instance.class));
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(MACHINE + " isn't in the binding's jar: build it with :binding:compileCoreWasm", e);
        }
        return new CoreModule("compiled", meta, instance -> {
            try {
                return (Machine) make.invoke(instance);
            } catch (RuntimeException | Error e) {
                throw e;
            } catch (Throwable t) {
                throw new IllegalStateException(t);
            }
        });
    }

    /** The module at {@code wasm}, run by Chicory's interpreter. */
    public static CoreModule interpreted(Path wasm) {
        return new CoreModule("interpreted", Parser.parse(wasm), null);
    }

    /** {@code module}, its functions run by {@code machine} (null: the interpreter). */
    public static CoreModule of(String name, WasmModule module, Function<Instance, Machine> machine) {
        return new CoreModule(name, module, machine);
    }

    /** How its functions run: {@code compiled}, {@code interpreted}, or what {@link #of} was given. */
    public String name() {
        return name;
    }

    WasmModule module() {
        return module;
    }

    /** The machine its instances run on, or null for Chicory's interpreter. */
    Function<Instance, Machine> machine() {
        return machine;
    }

    @Override
    public String toString() {
        return "CoreModule[" + name + "]";
    }
}
