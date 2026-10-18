package dev.b28.enterprise.core;

import dev.b28.enterprise.wire.Request;
import dev.b28.enterprise.wire.SaveFile;
import org.junit.jupiter.api.DynamicContainer;
import org.junit.jupiter.api.DynamicTest;
import org.junit.jupiter.api.TestFactory;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Supplier;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Every scenario (sim/scenarios/NAME.json, the scripts the core plays natively and under Node) played through Chicory:
 * each step's request sent as its postcard bytes, its response decoded by the generated wire, and the answer the one
 * the core gave natively (a refusal by its kind and numbers). Through the module compiled at build time (what the mod
 * ships) and through Chicory's interpreter, so both of Chicory's ways of running it give the core's answers.
 */
class ScenariosTest {
    @TestFactory
    Stream<DynamicContainer> everyScenarioGivesTheCoresAnswersThroughChicory() {
        List<Sim.Scenario> scenarios = Sim.scenarios();
        return Stream.of(engine("compiled", Sim::compiled, scenarios), engine("interpreted", Sim::interpreted, scenarios));
    }

    private static DynamicContainer engine(String name, Supplier<CoreModule> module, List<Sim.Scenario> scenarios) {
        return DynamicContainer.dynamicContainer(name, scenarios.stream().map(sc -> DynamicTest.dynamicTest(sc.name(), () -> {
            List<String> wrong = new ArrayList<>();
            try (Core core = Core.start(module.get())) {
                SaveFile[] saves = new SaveFile[1];
                for (Sim.Step step : sc.steps()) {
                    String w = Sim.play(core, step, saves);
                    if (w != null) {
                        wrong.add(w);
                    }
                }
                assertEquals(0, core.traps());
            }
            assertTrue(wrong.isEmpty(), sc.name() + " (" + name + "):\n  " + String.join("\n  ", wrong));
        })));
    }

    /** Every request a scenario sends as JSON reads in the generated Java and writes back as the same bytes. */
    @TestFactory
    Stream<DynamicTest> everyScenarioRequestReadsAndWritesBackTheSame() {
        return Sim.scenarios().stream().map(sc -> DynamicTest.dynamicTest(sc.name(), () -> {
            int read = 0;
            for (Sim.Step step : sc.steps()) {
                if (step.request() == null || step.raw()) {
                    continue; // a load of the last save, or raw bytes the core must refuse unread
                }
                assertArrayEquals(step.request(), Request.fromBytes(step.request()).toBytes(), step.note());
                read++;
            }
            assertTrue(read > 0, sc.name() + ": no request read");
        }));
    }
}
