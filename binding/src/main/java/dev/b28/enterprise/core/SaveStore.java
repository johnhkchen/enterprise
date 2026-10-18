package dev.b28.enterprise.core;

import dev.b28.enterprise.wire.Command;
import dev.b28.enterprise.wire.Refusal;
import dev.b28.enterprise.wire.Request;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.SaveFile;
import dev.b28.enterprise.wire.Wire;
import dev.b28.enterprise.wire.WireException;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Objects;
import java.util.function.Function;

/**
 * The core's save, kept in a world's folder (docs/binding-plan.md 1.4): {@value #FILE} holds one {@link SaveFile}'s
 * postcard bytes, and what each load and save does about the formats lives here, so the mod's host is a thin caller.
 *
 * <ul>
 *   <li><b>A world of an older format</b> is migrated by the core on {@code Load}. The old file isn't touched until the
 *       next save, which writes the new format over it; the file it replaces is first kept beside it as
 *       {@value #PREV} (a rollback to the older mod has it). Later saves, in the same format, leave {@value #PREV}
 *       alone: it is always the last save written by a core of another format, the one before the latest migration.
 *   <li><b>A world of a newer format</b> than this core reads (or one older than its migrations reach, or a file that
 *       isn't a save) is <em>refused</em>: the store is {@linkplain #paused paused}, the file is never written, and
 *       {@link #save()} does nothing. The host runs without the society and tells the player why ({@link
 *       Opened.Refused#message()}); nothing is lost, and the newer mod opens the world as it was.
 * </ul>
 *
 * <p>The mod's host passes {@code core::call} (a {@link Core}, or a {@link CoreWorker}'s answer waited for) so this
 * class stays Minecraft-free and testable with a core that migrates. Not thread-safe: the host's thread calls it.
 */
public final class SaveStore {
    /** The core's save, in the world's folder. */
    public static final String FILE = "enterprise_core.dat";
    /** The save a migration replaced: the last one written in another format. */
    public static final String PREV = "enterprise_core.prev.dat";

    /** What opening the world's save came to. */
    public sealed interface Opened {
        /** There is no save: the host makes a society ({@code Create}) and saves it in time. */
        record Fresh() implements Opened {}

        /** The save is the society now. {@code format} is the format the file was in. */
        record Loaded(SaveFile file) implements Opened {
            public long format() {
                return file.format();
            }
        }

        /**
         * The core wouldn't take the save. The store is paused and the file untouched. {@code why} is the core's
         * refusal, or null for a file that isn't a save at all (the core was never asked).
         */
        record Refused(Refusal why, String message) implements Opened {}
    }

    private final Path file;
    private final Path prev;
    private final Function<Request, Response> core;
    private boolean paused;
    private Long format;

    /** A store for the world folder {@code dir}, talking to {@code core}. */
    public SaveStore(Path dir, Function<Request, Response> core) {
        this.file = dir.resolve(FILE);
        this.prev = dir.resolve(PREV);
        this.core = Objects.requireNonNull(core);
    }

    /** The save in the world's folder, loaded into the core. */
    public Opened open() {
        byte[] bytes;
        try {
            if (!Files.exists(file)) {
                return new Opened.Fresh();
            }
            bytes = Files.readAllBytes(file);
        } catch (IOException e) {
            throw new UncheckedIOException("reading " + file, e);
        }
        SaveFile save;
        try {
            save = SaveFile.fromBytes(bytes);
        } catch (WireException e) {
            return refuse(null, FILE + " isn't a save this mod can read (" + e.getMessage() + ")");
        }
        Response r = core.apply(Wire.request(new Command.Load(save)));
        return switch (r) {
            case Response.Done done -> {
                format = save.format();
                yield new Opened.Loaded(save);
            }
            case Response.Refused refused -> refuse(refused.why(), describe(refused.why(), save));
            case Response.Saved saved -> throw new IllegalStateException("a Load answered with a save: " + saved);
        };
    }

    /**
     * Saves the core to the world's folder, if the store isn't paused: writes the file whole (to a temporary file, then
     * moved over it), after keeping the file of another format it replaces as {@value #PREV}.
     *
     * @return true if the save was written; false if the store is paused (nothing written)
     */
    public boolean save() {
        if (paused) {
            return false;
        }
        Response r = core.apply(Wire.request(new Command.Save()));
        if (!(r instanceof Response.Saved saved)) {
            throw new IllegalStateException("the core wouldn't save: " + r);
        }
        try {
            if (format != null && format != saved.save().format() && Files.exists(file)) {
                Files.copy(file, prev, StandardCopyOption.REPLACE_EXISTING);
            }
            Path tmp = file.resolveSibling(FILE + ".tmp");
            Files.write(tmp, saved.save().toBytes());
            try {
                Files.move(tmp, file, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
            } catch (AtomicMoveNotSupportedException e) {
                Files.move(tmp, file, StandardCopyOption.REPLACE_EXISTING);
            }
        } catch (IOException e) {
            throw new UncheckedIOException("writing " + file, e);
        }
        format = saved.save().format();
        return true;
    }

    /** True once the save was refused: the society is paused, and nothing is ever written over the file. */
    public boolean paused() {
        return paused;
    }

    private Opened refuse(Refusal why, String message) {
        paused = true;
        return new Opened.Refused(why, message);
    }

    /** A refusal in the words a player is told: the formats, and that the world's save is as it was. */
    private static String describe(Refusal why, SaveFile save) {
        String kept = "the society is paused and " + FILE + " is as it was";
        return switch (why) {
            case Refusal.SaveFormat f when f.save() > f.core() -> "This world's society was saved by a newer core ("
                    + save.core() + ") in format " + f.save() + "; this core reads up to format " + f.core() + ": " + kept
                    + ". Open it with the newer mod.";
            case Refusal.SaveFormat f -> "This world's society was saved in format " + f.save()
                    + ", older than this core brings forward (it reads up to format " + f.core() + "): " + kept + ".";
            case Refusal.Abi a -> "This mod and its core are built for different ABIs (" + a.core() + " and " + a.request() + "): " + kept;
            case Refusal.Schema s -> "This mod and its core are built from different commits (wire schema "
                    + Long.toHexString(s.core()) + " and " + Long.toHexString(s.request()) + "): " + kept;
            default -> "The core wouldn't load this world's society (" + why + "): " + kept;
        };
    }
}
