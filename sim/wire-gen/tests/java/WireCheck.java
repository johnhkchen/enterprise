// The generated wire's codec against the core's own bytes: run by sim/wire-gen/tests/java.rs (`just wire-javac`),
// which writes the samples.

import dev.b28.enterprise.wire.*;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HexFormat;
import java.util.List;
import java.util.Optional;

public final class WireCheck {
    private static int checked;
    private static int failed;

    public static void main(String[] args) throws Exception {
        for (String line : Files.readAllLines(Path.of(args[0]))) {
            String[] f = line.split(" ");
            try {
                switch (f[0]) {
                    case "OK" -> roundTrip(f[1], bytes(f[2]));
                    case "BAD" -> refused(f[1], bytes(f[2]));
                    case "PRIM" -> prim(f[1], f[2], bytes(f[3]));
                    case "BADPRIM" -> badPrim(f[1], bytes(f[2]));
                    default -> throw new IllegalStateException("what's " + f[0] + "?");
                }
            } catch (Throwable t) {
                fail(shorten(line) + ": " + t);
            }
            checked++;
        }
        checked += javaOnly();
        if (failed > 0) {
            System.out.println("WireCheck: " + failed + " of " + checked + " failed");
            System.exit(1);
        }
        System.out.println("WireCheck: all " + checked + " checks pass");
    }

    private static String shorten(String line) {
        return line.length() > 120 ? line.substring(0, 120) + "..." : line;
    }

    private static void fail(String why) {
        failed++;
        System.out.println("FAIL " + why);
    }

    private static byte[] bytes(String hex) {
        return hex.equals("-") ? new byte[0] : HexFormat.of().parseHex(hex);
    }

    private static Class<?> type(String name) throws ClassNotFoundException {
        return Class.forName("dev.b28.enterprise.wire." + name);
    }

    private static Object decode(String name, byte[] bytes) throws Throwable {
        Method from = type(name).getMethod("fromBytes", byte[].class);
        try {
            return from.invoke(null, (Object) bytes);
        } catch (InvocationTargetException e) {
            throw e.getCause();
        }
    }

    private static void roundTrip(String name, byte[] bytes) throws Throwable {
        Object value = decode(name, bytes);
        if (!type(name).isInstance(value)) {
            throw new AssertionError("decoded a " + value.getClass());
        }
        byte[] again = (byte[]) value.getClass().getMethod("toBytes").invoke(value);
        if (!java.util.Arrays.equals(bytes, again)) {
            throw new AssertionError("encoded again as " + HexFormat.of().formatHex(again));
        }
        // Equal to itself decoded again (records compare by value, Bytes by content, NaN to NaN).
        Object twice = decode(name, again);
        if (!value.equals(twice) || value.hashCode() != twice.hashCode()) {
            throw new AssertionError("not equal to itself decoded again: " + value);
        }
    }

    private static void refused(String name, byte[] bytes) throws Throwable {
        try {
            Object v = decode(name, bytes);
            throw new AssertionError("read " + v);
        } catch (WireException expected) {
            // as postcard refuses it
        }
    }

    private static void prim(String kind, String value, byte[] bytes) {
        PostcardOut out = new PostcardOut();
        PostcardIn in = new PostcardIn(bytes);
        Object read;
        Object want;
        switch (kind) {
            case "bool" -> { boolean v = Boolean.parseBoolean(value); out.bool(v); want = v; read = in.bool(); }
            case "u8" -> { int v = Integer.parseInt(value); out.u8(v); want = v; read = in.u8(); }
            case "i8" -> { byte v = Byte.parseByte(value); out.i8(v); want = v; read = in.i8(); }
            case "u16" -> { int v = Integer.parseInt(value); out.u16(v); want = v; read = in.u16(); }
            case "i16" -> { short v = Short.parseShort(value); out.i16(v); want = v; read = in.i16(); }
            case "u32" -> { long v = Long.parseLong(value); out.u32(v); want = v; read = in.u32(); }
            case "i32" -> { int v = Integer.parseInt(value); out.i32(v); want = v; read = in.i32(); }
            case "u64" -> { long v = Long.parseUnsignedLong(value); out.u64(v); want = v; read = in.u64(); }
            case "i64" -> { long v = Long.parseLong(value); out.i64(v); want = v; read = in.i64(); }
            case "f32" -> {
                int bits = Integer.parseUnsignedInt(value, 16);
                out.f32(Float.intBitsToFloat(bits));
                want = bits;
                read = Float.floatToRawIntBits(in.f32());
            }
            case "f64" -> {
                long bits = Long.parseUnsignedLong(value, 16);
                out.f64(Double.longBitsToDouble(bits));
                want = bits;
                read = Double.doubleToRawLongBits(in.f64());
            }
            case "string" -> {
                String v = new String(bytes(value), StandardCharsets.UTF_8);
                out.string(v);
                want = v;
                read = in.string();
            }
            default -> throw new IllegalStateException("what's a " + kind + "?");
        }
        in.finish();
        if (!want.equals(read)) {
            throw new AssertionError("read " + read + ", not " + want);
        }
        if (!java.util.Arrays.equals(bytes, out.toByteArray())) {
            throw new AssertionError("wrote " + HexFormat.of().formatHex(out.toByteArray()));
        }
    }

    private static void badPrim(String kind, byte[] bytes) {
        PostcardIn in = new PostcardIn(bytes);
        try {
            Object v = switch (kind) {
                case "bool" -> in.bool();
                case "u16" -> in.u16();
                case "i16" -> in.i16();
                case "u32" -> in.u32();
                case "i32" -> in.i32();
                case "u64" -> in.u64();
                case "i64" -> in.i64();
                case "f32" -> in.f32();
                case "f64" -> in.f64();
                case "string" -> in.string();
                default -> throw new IllegalStateException("what's a " + kind + "?");
            };
            in.finish();
            throw new AssertionError("read " + v);
        } catch (WireException expected) {
            // as postcard refuses it
        }
    }

    /** What only Java can get wrong: values its types hold and Rust's don't, nulls, and the lists' immutability. */
    private static int javaOnly() {
        int n = 0;
        n += refuses("a u8 of 256", () -> new PostcardOut().u8(256));
        n += refuses("a u8 of -1", () -> new PostcardOut().u8(-1));
        n += refuses("a u16 of 65536", () -> new PostcardOut().u16(65536));
        n += refuses("a u32 of 2^32", () -> new PostcardOut().u32(1L << 32));
        n += refuses("a u32 of -1", () -> new PostcardOut().u32(-1));
        n += refuses("an unpaired surrogate", () -> new PostcardOut().string("a\uD800b"));
        n += refuses("an Abi refusal of a u32 past its range",
                () -> new Refusal.Abi(1L << 32, 0).toBytes());
        n += throwsNull("a null dim", () -> new Point(PointKind.Desk, null, 0, 0, 0));
        n += throwsNull("a null list", () -> new Command.Apply(null));
        n += throwsNull("a null in a list", () -> new Command.Apply(java.util.Arrays.asList((Input) null)));
        n += throwsNull("a null option", () -> new PackGood("a", 1, 1, 0, 0, null, List.of(), 1));
        List<Input> mutable = new java.util.ArrayList<>(List.of(new Input.Advance(5)));
        Command.Apply apply = new Command.Apply(mutable);
        mutable.add(new Input.Advance(6));
        n++;
        if (apply.inputs().size() != 1) {
            fail("a record's list changed with the list it was made from");
        }
        n++;
        try {
            apply.inputs().add(new Input.Advance(7));
            fail("a record's list took an element");
        } catch (UnsupportedOperationException expected) {
            // immutable
        }
        // A request built here reads in Rust as the same bytes: the envelope's numbers, an unsigned u64.
        Request r = Wire.request(new Command.Apply(List.of(new Input.Advance(-1L))));
        byte[] b = r.toBytes();
        n++;
        if (!Request.fromBytes(b).equals(r) || Wire.SCHEMA != r.schema()) {
            fail("a request doesn't read back");
        }
        PackGood g = new PackGood("minecraft:book", 0.5, 64, 0, 0, Optional.of("reading"), List.of(GoodFlag.Crop), 2.5);
        n++;
        if (!PackGood.fromBytes(g.toBytes()).equals(g)) {
            fail("a good doesn't read back");
        }
        SaveFile s = new SaveFile(1, Wire.SCHEMA, "core", Bytes.of(new byte[] {1, 2, 3}));
        n++;
        if (!SaveFile.fromBytes(s.toBytes()).equals(s) || s.blob().length() != 3) {
            fail("a save doesn't read back");
        }
        return n;
    }

    private static int refuses(String what, Runnable r) {
        try {
            r.run();
            fail(what + " was written");
        } catch (WireException expected) {
            // refused
        }
        return 1;
    }

    private static int throwsNull(String what, Runnable r) {
        try {
            r.run();
            fail(what + " was taken");
        } catch (NullPointerException expected) {
            // refused
        }
        return 1;
    }
}
