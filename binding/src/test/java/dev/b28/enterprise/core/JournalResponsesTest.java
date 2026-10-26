package dev.b28.enterprise.core;

import org.junit.jupiter.api.Assumptions;
import org.junit.jupiter.api.Test;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * A world's journal played through the core as the game runs it (Chicory, compiled), each response's FNV-1a written a
 * line to OUT: against `enterprise-sim --responses JOURNAL`, the first request the native core answers otherwise.
 * Only with `-Pjournal=FILE -PjournalOut=OUT` (`./gradlew :binding:test --tests '*JournalResponses*'`).
 */
class JournalResponsesTest {
    @Test
    void aJournalsResponsesAsTheGameAnswersThem() throws Exception {
        String file = System.getProperty("enterprise.journal", "");
        Assumptions.assumeFalse(file.isEmpty(), "no -Pjournal");
        List<String> out = new ArrayList<>();
        try (Core core = Core.start(Sim.compiled())) {
            for (byte[] request : Journal.read(Path.of(file))) {
                out.add(Long.toHexString(fnv(core.callBytes(request))));
            }
        }
        Files.write(Path.of(System.getProperty("enterprise.journalOut")), out);
    }

    private static long fnv(byte[] bytes) {
        long h = 0xcbf29ce484222325L;
        for (byte b : bytes) {
            h ^= b & 0xff;
            h *= 0x100000001b3L;
        }
        return h;
    }
}
