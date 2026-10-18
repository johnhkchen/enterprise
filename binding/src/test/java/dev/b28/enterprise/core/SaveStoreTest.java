package dev.b28.enterprise.core;

import dev.b28.enterprise.wire.Bytes;
import dev.b28.enterprise.wire.Command;
import dev.b28.enterprise.wire.Refusal;
import dev.b28.enterprise.wire.Request;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.SaveFile;
import dev.b28.enterprise.wire.Wire;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * What a world's save file does about formats (docs/binding-plan.md 1.4): a newer world is refused and left alone, an
 * older one is migrated and the file it replaces is kept as {@code enterprise_core.prev.dat}. The real core has no
 * migration yet (format 1 is the only one), so the migrating case plays against a core that has: a stand-in that takes
 * a format-0 save and gives format 1, as the Rust template test ({@code sim/core/tests/saves.rs}) shows the chain does.
 */
class SaveStoreTest {
    private static Path path(Path dir) {
        return dir.resolve(SaveStore.FILE);
    }

    @Test
    void aWorldWithNoSaveIsFreshAndASaveRoundTrips(@TempDir Path dir) throws IOException {
        byte[] written;
        try (Core core = VersionsTest.created()) {
            SaveStore store = new SaveStore(dir, core::call);
            assertInstanceOf(SaveStore.Opened.Fresh.class, store.open());
            assertTrue(store.save());
            written = Files.readAllBytes(path(dir));
            assertFalse(Files.exists(dir.resolve(SaveStore.PREV)), "no migration, no previous save");
            assertTrue(store.save(), "saving again");
            assertArrayEquals(written, Files.readAllBytes(path(dir)), "a core that hasn't moved writes the same save");
        }
        try (Core core = Core.start(Sim.compiled())) {
            SaveStore store = new SaveStore(dir, core::call);
            SaveStore.Opened opened = store.open();
            assertEquals(1, assertInstanceOf(SaveStore.Opened.Loaded.class, opened).format());
            assertTrue(store.save());
            assertArrayEquals(written, Files.readAllBytes(path(dir)), "loaded and saved is the file it was");
            assertFalse(Files.exists(dir.resolve(SaveStore.PREV)), "a save of this format keeps no previous one");
            assertFalse(store.paused());
        }
    }

    @Test
    void aWorldOfANewerFormatIsRefusedPausedAndLeftUntouched(@TempDir Path dir) throws IOException {
        SaveFile mine;
        try (Core core = VersionsTest.created()) {
            mine = assertInstanceOf(Response.Saved.class, core.call(Wire.request(new Command.Save()))).save();
        }
        SaveFile newer = new SaveFile(mine.format() + 1, mine.schema() + 1, "a newer core", mine.blob());
        Files.write(path(dir), newer.toBytes());
        try (Core core = Core.start(Sim.compiled())) {
            SaveStore store = new SaveStore(dir, core::call);
            SaveStore.Opened.Refused refused = assertInstanceOf(SaveStore.Opened.Refused.class, store.open());
            assertEquals(new Refusal.SaveFormat(mine.format(), mine.format() + 1), refused.why());
            assertTrue(refused.message().contains("format " + (mine.format() + 1)) && refused.message().contains("a newer core")
                    && refused.message().contains("up to format " + mine.format()), refused.message());
            assertTrue(store.paused());

            // Nothing is ever written over it, even if the host makes a society and keeps saving.
            assertEquals(new Response.Done(List.of(), List.of()), core.call(Wire.request(VersionsTest.createCommand())));
            assertFalse(store.save());
            assertArrayEquals(newer.toBytes(), Files.readAllBytes(path(dir)));
            assertFalse(Files.exists(dir.resolve(SaveStore.PREV)));
        }
    }

    @Test
    void aWorldTooOldForTheChainOrNotASaveAtAllIsRefusedAndLeftUntouched(@TempDir Path dir) throws IOException {
        SaveFile mine;
        try (Core core = VersionsTest.created()) {
            mine = assertInstanceOf(Response.Saved.class, core.call(Wire.request(new Command.Save()))).save();
        }
        for (byte[] file : new byte[][] {new SaveFile(0, mine.schema(), "old", mine.blob()).toBytes(), "no".getBytes()}) {
            Files.write(path(dir), file);
            try (Core core = Core.start(Sim.compiled())) {
                SaveStore store = new SaveStore(dir, core::call);
                SaveStore.Opened.Refused refused = assertInstanceOf(SaveStore.Opened.Refused.class, store.open());
                assertTrue(store.paused() && !store.save(), refused.message());
                assertArrayEquals(file, Files.readAllBytes(path(dir)));
            }
        }
    }

    /** A core that has a migration from format 0: takes a format-0 save, and saves in format 1. */
    private static final class Migrating implements Function<Request, Response> {
        final List<Command> seen = new ArrayList<>();
        final SaveFile saves = new SaveFile(1, Wire.SCHEMA, "the new core", Bytes.of(new byte[] {1, 1, 1}));

        @Override
        public Response apply(Request request) {
            seen.add(request.command());
            return switch (request.command()) {
                case Command.Load load when load.save().format() <= 1 -> new Response.Done(List.of(), List.of());
                case Command.Save save -> new Response.Saved(saves);
                default -> new Response.Refused(new Refusal.NoSociety());
            };
        }
    }

    @Test
    void anOlderWorldIsMigratedOnLoadAndItsFileKeptBesideTheNewOne(@TempDir Path dir) throws IOException {
        SaveFile old = new SaveFile(0, 0, "the old core", Bytes.of(new byte[] {9, 9}));
        Files.write(path(dir), old.toBytes());
        Migrating core = new Migrating();
        SaveStore store = new SaveStore(dir, core);
        assertEquals(0, assertInstanceOf(SaveStore.Opened.Loaded.class, store.open()).format());
        assertArrayEquals(old.toBytes(), Files.readAllBytes(path(dir)), "loading migrates in the core: the file is the old one until a save");
        assertFalse(Files.exists(dir.resolve(SaveStore.PREV)));

        assertTrue(store.save());
        assertArrayEquals(core.saves.toBytes(), Files.readAllBytes(path(dir)), "the new format, in its place");
        assertArrayEquals(old.toBytes(), Files.readAllBytes(dir.resolve(SaveStore.PREV)), "the old one, kept for a rollback");

        // Later saves are in the new format already: the kept one is left as it is, not replaced by the migrated one.
        assertTrue(store.save());
        assertTrue(store.save());
        assertArrayEquals(old.toBytes(), Files.readAllBytes(dir.resolve(SaveStore.PREV)));
        assertArrayEquals(core.saves.toBytes(), Files.readAllBytes(path(dir)));
    }

    @Test
    void aMigrationThatNeverReachedASaveLeavesTheOldFileToMigrateAgain(@TempDir Path dir) throws IOException {
        SaveFile old = new SaveFile(0, 0, "the old core", Bytes.of(new byte[] {9, 9}));
        Files.write(path(dir), old.toBytes());
        for (int boot = 0; boot < 2; boot++) {
            SaveStore store = new SaveStore(dir, new Migrating());
            assertInstanceOf(SaveStore.Opened.Loaded.class, store.open());
            assertArrayEquals(old.toBytes(), Files.readAllBytes(path(dir)));
            assertFalse(Files.exists(dir.resolve(SaveStore.PREV)));
        }
    }
}
