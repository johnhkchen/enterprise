package dev.b28.enterprise.core;

import java.util.HexFormat;

/**
 * The core trapped on a request (a panic aborts: the build's {@code panic = "abort"}; or a bad memory access, the stack
 * run out). By the time this is thrown {@link Core} has dropped the instance, made a fresh one and replayed the society
 * up to before the request, unless {@link #stuck()}. The request isn't retried: the host logs it ({@link #request()})
 * with the save and the requests since for a bug report, and carries on without that look.
 */
public final class CoreTrap extends RuntimeException {
    private static final long serialVersionUID = 1L;

    private final transient byte[] request;
    private final String printed;
    private final int replayed;
    private final boolean stuck;

    CoreTrap(byte[] request, String printed, int replayed, boolean stuck, Throwable cause) {
        super(message(request, printed, replayed, stuck, cause), cause);
        this.request = request.clone();
        this.printed = printed;
        this.replayed = replayed;
        this.stuck = stuck;
    }

    private static String message(byte[] request, String printed, int replayed, boolean stuck, Throwable cause) {
        String what = "the core trapped on a request of " + request.length + " bytes (" + cause + ")";
        String said = printed.isEmpty() ? "" : "; it said: " + printed;
        String after = stuck
                ? "; the replay failed after " + replayed + " requests: the core is stuck"
                : "; recovered, " + replayed + " requests replayed";
        return what + said + after;
    }

    /** The request's bytes. */
    public byte[] request() {
        return request.clone();
    }

    /** The request's bytes in hex, for a log. */
    public String requestHex() {
        return HexFormat.of().formatHex(request);
    }

    /** What the core printed before it trapped (a panic's message), or nothing. */
    public String printed() {
        return printed;
    }

    /** How many requests the recovery replayed (the checkpoint and the looks since). */
    public int replayed() {
        return replayed;
    }

    /** True if the recovery failed too: the core takes no more calls. */
    public boolean stuck() {
        return stuck;
    }
}
