package dev.b28.enterprise.core;

import dev.b28.enterprise.wire.Command;
import dev.b28.enterprise.wire.Request;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.SaveFile;
import dev.b28.enterprise.wire.Wire;
import org.junit.jupiter.api.DynamicTest;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestFactory;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The journal: every request that changed the society, written as the core took it, is the whole run. Played into a
 * fresh core, it ends where the run did, to the byte of its save.
 */
class JournalTest {
    @TempDir
    Path dir;

    /** Each scenario journaled, with a trap at its middle step: the journal replayed saves what the run saved. */
    @TestFactory
    Stream<DynamicTest> aJournalReplayedEndsWhereTheRunDid() {
        return Sim.scenarios().stream().map(sc -> DynamicTest.dynamicTest(sc.name(), () -> {
            Path file = dir.resolve(sc.name() + ".ejl");
            SaveFile ran;
            try (Core core = Core.start(Sim.compiled())) {
                core.journal(Journal.create(file));
                SaveFile[] saves = new SaveFile[1];
                List<Sim.Step> steps = sc.steps();
                for (int i = 0; i < steps.size(); i++) {
                    Sim.Step step = steps.get(i);
                    if (i == steps.size() / 2) {
                        core.trapNext(1);
                        assertThrows(CoreTrap.class, () -> core.callBytes(step.bytes(saves[0])));
                    }
                    assertNull(Sim.play(core, step, saves), "step " + i);
                }
                ran = save(core);
                assertNull(core.journalFailure());
            }
            List<byte[]> journal = Journal.read(file);
            assertTrue(!journal.isEmpty(), "nothing journaled");
            for (byte[] r : journal) {
                Command c = Request.fromBytes(r).command();
                assertTrue(c instanceof Command.Create || c instanceof Command.Load || c instanceof Command.Apply,
                        "journaled a " + c.getClass().getSimpleName());
            }
            try (Core replayed = Core.start(Sim.compiled())) {
                for (byte[] r : journal) {
                    assertInstanceOf(Response.Done.class, Response.fromBytes(replayed.callBytes(r)));
                }
                if (ran == null) {
                    return; // a scenario that ends with no society
                }
                assertArrayEquals(ran.blob().toArray(), save(replayed).blob().toArray(), "the replay's save differs from the run's");
            }
        }));
    }

    /** A request cut short at the end (a crash mid-write) is left out; a file that isn't a journal is refused. */
    @Test
    void aTornLastRequestIsLeftOutAndAStrangerIsRefused() throws IOException {
        Path file = dir.resolve("torn.ejl");
        try (Journal j = Journal.create(file)) {
            j.write(new byte[] {1, 2, 3});
            j.write(new byte[] {4, 5});
            assertEquals(2, j.requests());
        }
        byte[] whole = Files.readAllBytes(file);
        Files.write(file, Arrays.copyOf(whole, whole.length - 1));
        List<byte[]> read = Journal.read(file);
        assertEquals(1, read.size());
        assertArrayEquals(new byte[] {1, 2, 3}, read.get(0));
        Path stranger = dir.resolve("stranger.ejl");
        Files.writeString(stranger, "not a journal at all");
        assertThrows(IOException.class, () -> Journal.read(stranger));
    }

    private static SaveFile save(Core core) {
        try {
            Response r = core.call(Wire.request(new Command.Save()));
            return r instanceof Response.Saved s ? s.save() : null;
        } catch (IllegalStateException e) {
            return null;
        }
    }
}
