package dev.b28.enterprise.core;

import dev.b28.enterprise.wire.PostcardIn;
import dev.b28.enterprise.wire.PostcardOut;
import org.junit.jupiter.api.DynamicTest;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestFactory;

import java.io.IOException;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;
import java.util.function.BiConsumer;
import java.util.function.Function;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The golden files (sim/wire/fixtures/NAME.bin, written by the core's tests/wire_fixtures.rs): each is a list of a wire
 * type's samples as postcard. Every file decodes with the generated decoder and encodes again to the same bytes, and
 * every generated wire type has its file, so the files and the wire move together.
 */
class FixturesTest {
    private static final String WIRE = "dev.b28.enterprise.wire.";
    /** Fixtures of a type with no Java class of its own: the core's `Fixed` crosses as its thousandths, an i64. */
    private static final Set<String> PRIMITIVE = Set.of("Fixed");

    private static Path dir() {
        return Sim.sim().resolve("wire/fixtures");
    }

    private static List<String> names() throws IOException {
        try (Stream<Path> files = Files.list(dir())) {
            return files.map(p -> p.getFileName().toString()).filter(n -> n.endsWith(".bin"))
                    .map(n -> n.substring(0, n.length() - 4)).sorted().toList();
        }
    }

    @TestFactory
    Stream<DynamicTest> everyFixtureDecodesAndEncodesToTheSameBytes() throws IOException {
        List<String> names = names();
        assertFalse(names.isEmpty(), "no fixtures in " + dir());
        return names.stream().map(name -> DynamicTest.dynamicTest(name, () -> {
            byte[] bytes = Files.readAllBytes(dir().resolve(name + ".bin"));
            Function<PostcardIn, Object> decode;
            BiConsumer<PostcardOut, Object> encode;
            if (PRIMITIVE.contains(name)) {
                decode = PostcardIn::i64;
                encode = (out, v) -> out.i64((Long) v);
            } else {
                Class<?> type = Class.forName(WIRE + name);
                Method from = type.getMethod("decode", PostcardIn.class);
                Method to = type.getMethod("encode", PostcardOut.class);
                decode = in -> invoke(from, null, in);
                encode = (out, v) -> invoke(to, v, out);
            }
            List<Object> samples = PostcardIn.read(bytes, in -> in.list(1, decode));
            assertFalse(samples.isEmpty(), name + ": no samples");
            byte[] again = PostcardOut.of(out -> out.list(samples, encode));
            assertArrayEquals(bytes, again, name + ": encoded again to other bytes");
            // Each sample alone, too: decoded from its own bytes, equal to itself.
            for (Object s : samples) {
                byte[] one = PostcardOut.of(out -> encode.accept(out, s));
                Object back = PostcardIn.read(one, decode);
                assertEquals(s, back, name + ": a sample reads back as another value");
            }
        }));
    }

    /** Every wire type the generator writes (a record, sealed interface or enum with a decoder) has a golden file. */
    @Test
    void everyWireTypeHasAFixture() throws Exception {
        Set<String> have = new TreeSet<>(names());
        Path generated = Path.of("src/generated/java/dev/b28/enterprise/wire");
        List<String> missing = new ArrayList<>();
        try (Stream<Path> files = Files.list(generated)) {
            for (Path f : files.sorted().toList()) {
                String name = f.getFileName().toString().replace(".java", "");
                Class<?> type = Class.forName(WIRE + name);
                boolean wire = Stream.of(type.getMethods()).anyMatch(m -> m.getName().equals("decode")
                        && Modifier.isStatic(m.getModifiers()) && m.getParameterCount() == 1
                        && m.getParameterTypes()[0] == PostcardIn.class);
                if (wire && !have.contains(name)) {
                    missing.add(name);
                }
            }
        }
        assertTrue(missing.isEmpty(), "wire types with no golden file (`just wire-fixtures`): " + missing);
    }

    private static Object invoke(Method m, Object on, Object arg) {
        try {
            return m.invoke(on, arg);
        } catch (InvocationTargetException e) {
            if (e.getCause() instanceof RuntimeException r) {
                throw r;
            }
            throw new IllegalStateException(e.getCause());
        } catch (IllegalAccessException e) {
            throw new IllegalStateException(e);
        }
    }
}
