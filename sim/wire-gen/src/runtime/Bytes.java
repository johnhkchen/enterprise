// Written beside the generated wire by sim/wire-gen, from sim/wire-gen/src/runtime: edit it there.
package dev.b28.enterprise.wire;

import java.io.IOException;
import java.io.OutputStream;
import java.util.Arrays;

/** A Rust {@code Vec<u8>} (a save's blob): bytes that don't change, equal by their contents. */
public final class Bytes {
    public static final Bytes EMPTY = new Bytes(new byte[0]);

    private final byte[] data;

    private Bytes(byte[] data) {
        this.data = data;
    }

    /** A copy of {@code data}. */
    public static Bytes of(byte[] data) {
        return new Bytes(data.clone());
    }

    /** {@code data} itself, which no one else holds: the decoder's. */
    static Bytes wrap(byte[] data) {
        return new Bytes(data);
    }

    /** The bytes themselves, for the encoder: never handed out. */
    byte[] array() {
        return data;
    }

    public int length() {
        return data.length;
    }

    public byte get(int i) {
        return data[i];
    }

    /** A copy of the bytes. */
    public byte[] toArray() {
        return data.clone();
    }

    /** The bytes, written without a copy (a save to its file). */
    public void writeTo(OutputStream out) throws IOException {
        out.write(data);
    }

    @Override
    public boolean equals(Object o) {
        return o instanceof Bytes b && Arrays.equals(data, b.data);
    }

    @Override
    public int hashCode() {
        return Arrays.hashCode(data);
    }

    @Override
    public String toString() {
        StringBuilder s = new StringBuilder("Bytes[").append(data.length);
        if (data.length > 0) {
            s.append(": ");
            for (int i = 0; i < Math.min(data.length, 16); i++) {
                s.append(String.format("%02x", data[i] & 0xFF));
            }
            if (data.length > 16) {
                s.append("...");
            }
        }
        return s.append(']').toString();
    }
}
