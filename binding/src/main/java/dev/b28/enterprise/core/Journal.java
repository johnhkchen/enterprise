package dev.b28.enterprise.core;

import java.io.BufferedOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.EOFException;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/**
 * Every request that changed the society, in the order the core took them, in a file: the run, to replay off the game
 * (`enterprise-sim --trace FILE OUT.db` replays it through the native core and writes what happened into SQLite). The core
 * is deterministic, so a {@code Create} or {@code Load} and the {@code Apply}s after it are the whole run; asking and
 * saving change nothing and aren't written, nor is a refused request or one the core trapped on (nothing of either was
 * applied).
 *
 * <p>The file: {@link #MAGIC}, then each request as a 4-byte big-endian length and its postcard bytes. Each is flushed as
 * it's written, so a crash loses none the core took. A world's journals are in its {@value #DIR} folder, one a server
 * run.
 */
public final class Journal implements AutoCloseable {
    /** The file's first bytes: what it is and its version. */
    public static final byte[] MAGIC = "ENTJRNL1".getBytes(StandardCharsets.US_ASCII);
    /** A world's journals' folder. */
    public static final String DIR = "enterprise-journal";

    private final Path file;
    private final DataOutputStream out;
    /** Beside it ({@code FILE.fnv}): each request's response, its FNV-1a a line, to find where a replay parts from the run. */
    private final java.io.Writer answers;
    private long requests;
    private long bytes;

    private Journal(Path file, DataOutputStream out, java.io.Writer answers) {
        this.file = file;
        this.out = out;
        this.answers = answers;
    }

    /** A new journal at {@code file} (its folder made if missing; a file there already is replaced). */
    public static Journal create(Path file) throws IOException {
        Path parent = file.toAbsolutePath().getParent();
        if (parent != null) {
            Files.createDirectories(parent);
        }
        DataOutputStream out = new DataOutputStream(new BufferedOutputStream(Files.newOutputStream(file), 1 << 16));
        out.write(MAGIC);
        out.flush();
        java.io.Writer answers = Files.newBufferedWriter(file.resolveSibling(file.getFileName() + ".fnv"));
        return new Journal(file, out, answers);
    }

    /** Writes one request's bytes, flushed. */
    public synchronized void write(byte[] request) throws IOException {
        write(request, null);
    }

    /**
     * Writes one request's bytes, flushed, and its response's FNV-1a beside it (`enterprise-sim --responses` prints a
     * replay's, a line each: the first line that differs is the first request the replay answered otherwise).
     */
    public synchronized void write(byte[] request, byte[] response) throws IOException {
        out.writeInt(request.length);
        out.write(request);
        out.flush();
        if (response != null) {
            long h = 0xcbf29ce484222325L;
            for (byte b : response) {
                h ^= b & 0xff;
                h *= 0x100000001b3L;
            }
            answers.write(Long.toHexString(h));
            answers.write('\n');
            answers.flush();
        }
        requests++;
        bytes += 4 + request.length;
    }

    public Path file() {
        return file;
    }

    /** Requests written so far. */
    public synchronized long requests() {
        return requests;
    }

    /** Bytes written so far, the magic aside. */
    public synchronized long bytes() {
        return bytes;
    }

    @Override
    public synchronized void close() throws IOException {
        out.close();
        answers.close();
    }

    /**
     * A journal's requests, in order. A last request cut short (a crash mid-write) is left out.
     *
     * @throws IOException if the file can't be read or isn't a journal
     */
    public static List<byte[]> read(Path file) throws IOException {
        try (InputStream raw = Files.newInputStream(file); DataInputStream in = new DataInputStream(raw)) {
            byte[] magic = new byte[MAGIC.length];
            in.readFully(magic);
            if (!Arrays.equals(magic, MAGIC)) {
                throw new IOException(file + " isn't an Enterprise journal");
            }
            List<byte[]> all = new ArrayList<>();
            while (true) {
                int len;
                try {
                    len = in.readInt();
                } catch (EOFException e) {
                    return all;
                }
                byte[] request = new byte[len];
                try {
                    in.readFully(request);
                } catch (EOFException e) {
                    return all;
                }
                all.add(request);
            }
        }
    }
}
