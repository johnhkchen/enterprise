package dev.b28.enterprise.core;

import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.SaveFile;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** The core on its own thread: the same answers, in the order sent, and a trap that doesn't stop the next request. */
class CoreWorkerTest {
    private static Sim.Scenario stocktake() {
        return Sim.scenarios().stream().filter(s -> s.name().equals("stocktake")).findFirst().orElseThrow();
    }

    /** Every step submitted at once, before any answer is read: they run in order and give the core's answers. */
    @Test
    void requestsSubmittedTogetherRunInOrderWithTheSameAnswers() {
        Sim.Scenario sc = stocktake();
        List<String> wrong = new ArrayList<>();
        try (CoreWorker worker = CoreWorker.start(Sim.compiled())) {
            // A load step needs the save before it: submit up to the first load, then the rest.
            SaveFile[] saves = new SaveFile[1];
            List<CompletableFuture<byte[]>> pending = new ArrayList<>();
            List<Sim.Step> sent = new ArrayList<>();
            for (Sim.Step step : sc.steps()) {
                if (step.loadSaved()) {
                    check(pending, sent, saves, wrong);
                }
                pending.add(worker.submitBytes(step.bytes(saves[0])));
                sent.add(step);
            }
            check(pending, sent, saves, wrong);
            assertEquals(0, worker.inspect(Core::traps).join());
        }
        assertTrue(wrong.isEmpty(), String.join("\n", wrong));
    }

    private static void check(List<CompletableFuture<byte[]>> pending, List<Sim.Step> sent, SaveFile[] saves,
            List<String> wrong) {
        for (int i = 0; i < pending.size(); i++) {
            Sim.Step step = sent.get(i);
            Response got = Response.fromBytes(pending.get(i).join());
            if (got instanceof Response.Saved s) {
                saves[0] = s.save();
            } else if (!Sim.withoutProse(got).equals(Sim.withoutProse(Response.fromBytes(step.response())))) {
                wrong.add(step.note() + ": " + got);
            }
        }
        pending.clear();
        sent.clear();
    }

    @Test
    void aTrapFailsItsRequestAndTheNextRuns() {
        Sim.Scenario sc = stocktake();
        try (CoreWorker worker = CoreWorker.start(Sim.compiled())) {
            SaveFile[] saves = new SaveFile[1];
            for (int i = 0; i < 3; i++) {
                worker.submitBytes(sc.steps().get(i).bytes(saves[0])).join();
            }
            worker.inspect(core -> {
                core.trapNext(1);
                return null;
            }).join();
            Sim.Step step = sc.steps().get(3);
            CompletionException e = assertThrows(CompletionException.class, () -> worker.submitBytes(step.bytes(saves[0])).join());
            assertInstanceOf(CoreTrap.class, e.getCause());
            Response again = Response.fromBytes(worker.submitBytes(step.bytes(saves[0])).join());
            assertEquals(Sim.withoutProse(Response.fromBytes(step.response())), Sim.withoutProse(again));
        }
    }
}
