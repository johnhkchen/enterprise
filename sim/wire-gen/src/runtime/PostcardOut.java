// Written beside the generated wire by sim/wire-gen, from sim/wire-gen/src/runtime: edit it there.
package dev.b28.enterprise.wire;

import java.nio.ByteBuffer;
import java.nio.CharBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.List;
import java.util.Optional;
import java.util.function.BiConsumer;
import java.util.function.Consumer;

/**
 * Writes postcard, as the core's {@code postcard::to_allocvec} does: unsigned numbers of 16 bits and more as varints
 * (seven bits a byte, low first), signed ones zigzagged first, a byte as itself, floats as their IEEE bits
 * little-endian, a string or list as its length then its contents, an option as 0 or 1 then its value, an enum's
 * variant as its index. A number out of its Rust type's range is refused ({@link WireException}).
 */
public final class PostcardOut {
    private byte[] buf = new byte[64];
    private int len;

    /** The bytes of whatever {@code write} writes. */
    public static byte[] of(Consumer<PostcardOut> write) {
        PostcardOut out = new PostcardOut();
        write.accept(out);
        return out.toByteArray();
    }

    /** What's been written. */
    public byte[] toByteArray() {
        return Arrays.copyOf(buf, len);
    }

    /** How many bytes have been written. */
    public int size() {
        return len;
    }

    private void room(int n) {
        if (n > buf.length - len) {
            long want = Math.max((long) buf.length * 2, (long) len + n);
            if (want > Integer.MAX_VALUE - 8) {
                throw new WireException("a message over 2 GB");
            }
            buf = Arrays.copyOf(buf, (int) want);
        }
    }

    private void put(int b) {
        room(1);
        buf[len++] = (byte) b;
    }

    private void put(byte[] bytes, int from, int count) {
        room(count);
        System.arraycopy(bytes, from, buf, len, count);
        len += count;
    }

    /** {@code v}'s 64 bits, unsigned, seven a byte. */
    private void varint(long v) {
        while ((v & ~0x7FL) != 0) {
            put((int) ((v & 0x7F) | 0x80));
            v >>>= 7;
        }
        put((int) v);
    }

    private static void range(long v, long max, String type) {
        if (v < 0 || v > max) {
            throw new WireException("a " + type + " of " + v + ": it holds 0 to " + max);
        }
    }

    public void bool(boolean v) {
        put(v ? 1 : 0);
    }

    public void u8(int v) {
        range(v, 0xFF, "u8");
        put(v);
    }

    public void i8(byte v) {
        put(v);
    }

    public void u16(int v) {
        range(v, 0xFFFF, "u16");
        varint(v);
    }

    public void i16(short v) {
        varint(((v << 1) ^ (v >> 15)) & 0xFFFF);
    }

    public void u32(long v) {
        range(v, 0xFFFF_FFFFL, "u32");
        varint(v);
    }

    public void i32(int v) {
        varint(((v << 1) ^ (v >> 31)) & 0xFFFF_FFFFL);
    }

    /** A u64: {@code v}'s 64 bits read unsigned. */
    public void u64(long v) {
        varint(v);
    }

    public void i64(long v) {
        varint((v << 1) ^ (v >> 63));
    }

    public void f32(float v) {
        int bits = Float.floatToRawIntBits(v);
        for (int i = 0; i < 4; i++) {
            put(bits >>> (8 * i));
        }
    }

    public void f64(double v) {
        long bits = Double.doubleToRawLongBits(v);
        for (int i = 0; i < 8; i++) {
            put((int) (bits >>> (8 * i)));
        }
    }

    /** A string as UTF-8; one Rust can't hold (an unpaired surrogate) is refused. */
    public void string(String v) {
        ByteBuffer utf8;
        try {
            utf8 = StandardCharsets.UTF_8.newEncoder()
                    .onMalformedInput(CodingErrorAction.REPORT)
                    .onUnmappableCharacter(CodingErrorAction.REPORT)
                    .encode(CharBuffer.wrap(v));
        } catch (CharacterCodingException e) {
            throw new WireException("a string that isn't Unicode (an unpaired surrogate): " + e.getMessage());
        }
        length(utf8.remaining());
        while (utf8.hasRemaining()) {
            put(utf8.get());
        }
    }

    public void bytes(Bytes v) {
        length(v.length());
        put(v.array(), 0, v.length());
    }

    /** A string's, list's or bytes' length (a Rust {@code usize}). */
    public void length(int n) {
        range(n, Integer.MAX_VALUE, "length");
        varint(n);
    }

    /** An enum's variant, by its index. */
    public void variant(int index) {
        range(index, Integer.MAX_VALUE, "variant index");
        varint(index);
    }

    public <T> void option(Optional<T> v, BiConsumer<PostcardOut, T> element) {
        if (v.isPresent()) {
            put(1);
            element.accept(this, v.get());
        } else {
            put(0);
        }
    }

    public <T> void list(List<T> values, BiConsumer<PostcardOut, T> element) {
        length(values.size());
        for (T value : values) {
            element.accept(this, value);
        }
    }
}
