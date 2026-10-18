// Written beside the generated wire by sim/wire-gen, from sim/wire-gen/src/runtime: edit it there.
package dev.b28.enterprise.wire;

/** Bytes that aren't a message of the wire, or a value its Rust type can't hold. */
public final class WireException extends RuntimeException {
    private static final long serialVersionUID = 1L;

    public WireException(String message) {
        super(message);
    }
}
