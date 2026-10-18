package dev.b28.enterprise.core;

import dev.b28.enterprise.wire.Command;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.SaveFile;
import org.junit.jupiter.api.DynamicTest;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestFactory;

import java.util.ArrayList;
import java.util.List;
import java.util.stream.IntStream;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The trap path (docs/binding-plan.md 1.1): an instance that traps is dropped, a fresh one replays the society to just
 * before the request, and the host carries on. The traps here are real (a read past the module's memory, through the
 * module's own `ent_call`), made on purpose before a step's request.
 */
class TrapTest {
    /**
     * Each scenario with a trap at every step in turn: the trapped request is thrown back, sent again, and from there
     * every answer is the one the core gave without a trap.
     */
    @TestFactory
    Stream<DynamicTest> aTrapAtAnyStepRecoversToTheSameAnswers() {
        return Sim.scenarios().stream().flatMap(sc -> IntStream.range(0, sc.steps().size()).mapToObj(at ->
                DynamicTest.dynamicTest(sc.name() + " trapped at step " + at, () -> {
                    List<String> wrong = new ArrayList<>();
                    try (Core core = Core.start(Sim.compiled())) {
                        SaveFile[] saves = new SaveFile[1];
                        for (int i = 0; i < sc.steps().size(); i++) {
                            Sim.Step step = sc.steps().get(i);
                            if (i == at) {
                                int replay = core.replay().size();
                                core.trapNext(1);
                                CoreTrap trap = assertThrows(CoreTrap.class, () -> core.callBytes(step.bytes(saves[0])));
                                assertFalse(trap.stuck(), trap.getMessage());
                                assertEquals(replay, trap.replayed(), "replayed the checkpoint and the looks since");
                                assertEquals(1, core.traps());
                            }
                            String w = Sim.play(core, step, saves);
                            if (w != null) {
                                wrong.add("step " + i + ": " + w);
                            }
                        }
                    }
                    assertTrue(wrong.isEmpty(), String.join("\n", wrong));
                })));
    }

    /** What a trap replays: the last Create, Load or save, and every Apply the core took since; never an Ask or a refusal. */
    @Test
    void theReplayIsTheCheckpointAndTheLooksSince() {
        List<Sim.Scenario> all = Sim.scenarios();
        Sim.Scenario stocktake = all.stream().filter(s -> s.name().equals("stocktake")).findFirst().orElseThrow();
        try (Core core = Core.start(Sim.compiled())) {
            assertEquals(0, core.replay().size(), "no society, nothing to replay");
            SaveFile[] saves = new SaveFile[1];
            for (Sim.Step step : stocktake.steps()) {
                int before = core.replay().size();
                Response got = Response.fromBytes(core.callBytes(step.bytes(saves[0])));
                if (got instanceof Response.Saved s) {
                    saves[0] = s.save();
                    assertEquals(1, core.replay().size(), step.note() + ": a save is the new checkpoint");
                    assertInstanceOf(Command.Load.class, dev.b28.enterprise.wire.Request.fromBytes(core.replay().get(0)).command());
                } else if (got instanceof Response.Refused) {
                    assertEquals(before, core.replay().size(), step.note() + ": a refusal isn't kept");
                }
            }
            assertTrue(core.replay().size() >= 1);
        }
    }

    /** A trap the replay can't get past (it traps too): the core says it's stuck, and takes no more calls. */
    @Test
    void aTrapThatTrapsAgainOnReplayLeavesTheCoreStuck() {
        Sim.Scenario stocktake = Sim.scenarios().stream().filter(s -> s.name().equals("stocktake")).findFirst().orElseThrow();
        try (Core core = Core.start(Sim.compiled())) {
            SaveFile[] saves = new SaveFile[1];
            List<Sim.Step> steps = stocktake.steps();
            for (int i = 0; i < 3; i++) {
                assertEquals(null, Sim.play(core, steps.get(i), saves));
            }
            core.trapNext(2);
            CoreTrap trap = assertThrows(CoreTrap.class, () -> core.callBytes(steps.get(3).bytes(saves[0])));
            assertTrue(trap.stuck(), trap.getMessage());
            assertTrue(core.stuck());
            assertThrows(IllegalStateException.class, () -> core.callBytes(steps.get(3).bytes(saves[0])));
        }
    }

    /** Before any society there's nothing to replay: a fresh instance is the recovery, and it says there's no society. */
    @Test
    void aTrapBeforeAnySocietyLeavesAFreshCore() {
        try (Core core = Core.start(Sim.compiled())) {
            core.trapNext(1);
            CoreTrap trap = assertThrows(CoreTrap.class, () -> core.callBytes(Sim.bytes(new Command.Save())));
            assertEquals(0, trap.replayed());
            Response r = Response.fromBytes(core.callBytes(Sim.bytes(new Command.Save())));
            assertInstanceOf(Response.Refused.class, r);
        }
    }
}
