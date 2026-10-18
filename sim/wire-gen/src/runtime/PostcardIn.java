// Written beside the generated wire by sim/wire-gen, from sim/wire-gen/src/runtime: edit it there.
package dev.b28.enterprise.wire;

import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.Optional;
import java.util.function.Function;

/**
 * Reads postcard, as the core's {@code postcard::from_bytes} does under WebAssembly (32-bit lengths), and refuses
 * what it refuses ({@link WireException}): bytes that end mid-value, a varint longer than its type or past its
 * range, a bool or option tag other than 0 or 1, a string that isn't UTF-8, an index no variant has. Lengths are
 * checked against the bytes left before anything is made for them.
 */
public final class PostcardIn {
    private final byte[] bytes;
    private int at;

    public PostcardIn(byte[] bytes) {
        this.bytes = Objects.requireNonNull(bytes, "bytes");
    }

    /** {@code decode} over all of {@code bytes}: bytes left over are refused. */
    public static <T> T read(byte[] bytes, Function<PostcardIn, T> decode) {
        PostcardIn in = new PostcardIn(bytes);
        T value = decode.apply(in);
        in.finish();
        return value;
    }

    /** Where the next byte is read from. */
    public int position() {
        return at;
    }

    /** The bytes not yet read. */
    public int remaining() {
        return bytes.length - at;
    }

    /** Refuses bytes left over. */
    public void finish() {
        if (at != bytes.length) {
            throw fail(at, (bytes.length - at) + " bytes left over after the value");
        }
    }

    /** An index no variant of {@code type} has. */
    public WireException unknownVariant(String type, int index) {
        return fail(at, "no variant " + index + " of " + type);
    }

    private WireException fail(int position, String what) {
        return new WireException(what + " (at byte " + position + " of " + bytes.length + ")");
    }

    private int take() {
        if (at >= bytes.length) {
            throw fail(at, "the bytes end mid-value");
        }
        return bytes[at++] & 0xFF;
    }

    /**
     * A varint of at most {@code maxBytes} bytes, the last of which may hold no more than {@code lastMax}: postcard's
     * {@code try_take_varint_*}.
     */
    private long varint(int maxBytes, int lastMax, String type) {
        int start = at;
        long out = 0;
        for (int i = 0; i < maxBytes; i++) {
            int b = take();
            out |= (long) (b & 0x7F) << (7 * i);
            if ((b & 0x80) == 0) {
                if (i == maxBytes - 1 && b > lastMax) {
                    throw fail(start, "a " + type + " varint past its range");
                }
                return out;
            }
        }
        throw fail(start, "a " + type + " varint longer than " + maxBytes + " bytes");
    }

    public boolean bool() {
        int b = take();
        if (b > 1) {
            throw fail(at - 1, "a bool of " + b);
        }
        return b == 1;
    }

    public int u8() {
        return take();
    }

    public byte i8() {
        return (byte) take();
    }

    public int u16() {
        return (int) varint(3, 0x03, "u16");
    }

    public short i16() {
        int n = u16();
        return (short) ((n >>> 1) ^ -(n & 1));
    }

    public long u32() {
        return varint(5, 0x0F, "u32");
    }

    public int i32() {
        long n = u32();
        return (int) ((n >>> 1) ^ -(n & 1));
    }

    /** A u64: its 64 bits in a long, read unsigned. */
    public long u64() {
        return varint(10, 0x01, "u64");
    }

    public long i64() {
        long n = u64();
        return (n >>> 1) ^ -(n & 1);
    }

    public float f32() {
        int bits = 0;
        for (int i = 0; i < 4; i++) {
            bits |= take() << (8 * i);
        }
        return Float.intBitsToFloat(bits);
    }

    public double f64() {
        long bits = 0;
        for (int i = 0; i < 8; i++) {
            bits |= (long) take() << (8 * i);
        }
        return Double.longBitsToDouble(bits);
    }

    /**
     * A length (a Rust {@code usize}, 32 bits under WebAssembly) of things each at least {@code minBytes} long: one
     * longer than the bytes left could hold is refused.
     */
    public int length(int minBytes) {
        int start = at;
        long n = varint(5, 0x0F, "length");
        if (n > Integer.MAX_VALUE) {
            throw fail(start, "a length of " + n);
        }
        if (minBytes > 0 && n > remaining() / minBytes) {
            throw fail(start, "a length of " + n + " with " + remaining() + " bytes left");
        }
        return (int) n;
    }

    public String string() {
        int n = length(1);
        try {
            String s = StandardCharsets.UTF_8.newDecoder()
                    .onMalformedInput(CodingErrorAction.REPORT)
                    .onUnmappableCharacter(CodingErrorAction.REPORT)
                    .decode(ByteBuffer.wrap(bytes, at, n))
                    .toString();
            at += n;
            return s;
        } catch (CharacterCodingException e) {
            throw fail(at, "a string that isn't UTF-8");
        }
    }

    public Bytes bytes() {
        int n = length(1);
        Bytes b = Bytes.wrap(java.util.Arrays.copyOfRange(bytes, at, at + n));
        at += n;
        return b;
    }

    /** An enum's variant index. */
    public int variant() {
        int start = at;
        long n = u32();
        if (n > Integer.MAX_VALUE) {
            throw fail(start, "a variant index of " + n);
        }
        return (int) n;
    }

    public <T> Optional<T> option(Function<PostcardIn, T> element) {
        int tag = take();
        return switch (tag) {
            case 0 -> Optional.empty();
            case 1 -> Optional.of(element.apply(this));
            default -> throw fail(at - 1, "an option tag of " + tag);
        };
    }

    /** A list of elements each at least {@code minBytes} long. */
    public <T> List<T> list(int minBytes, Function<PostcardIn, T> element) {
        int n = length(minBytes);
        List<T> values = new ArrayList<>(Math.min(n, 4096));
        for (int i = 0; i < n; i++) {
            values.add(element.apply(this));
        }
        return List.copyOf(values);
    }
}
