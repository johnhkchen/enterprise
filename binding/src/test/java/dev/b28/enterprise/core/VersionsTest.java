package dev.b28.enterprise.core;

import dev.b28.enterprise.wire.Bytes;
import dev.b28.enterprise.wire.Command;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Refusal;
import dev.b28.enterprise.wire.Request;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.SaveFile;
import dev.b28.enterprise.wire.Wire;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The versions (docs/binding-plan.md 1.4) through the real module: the ABI and schema checked on every call, a save of
 * a format the core can't read refused with the society kept. (The Rust side of the same: sim/core/tests/saves.rs,
 * wire.rs and the refusals scenario, which {@link ScenariosTest} plays here too.)
 */
class VersionsTest {
    static Command createCommand() {
        for (Sim.Scenario sc : Sim.scenarios()) {
            for (Sim.Step step : sc.steps()) {
                if (step.request() != null && !step.raw()) {
                    Request r = Request.fromBytes(step.request());
                    if (r.command() instanceof Command.Create c) {
                        return c;
                    }
                }
            }
        }
        throw new AssertionError("no scenario creates a society");
    }

    static Core created() {
        Core core = Core.start(Sim.compiled());
        assertEquals(new Response.Done(List.of(), List.of()), core.call(Wire.request(createCommand())));
        return core;
    }

    private static Refusal refusal(Response r) {
        return assertInstanceOf(Response.Refused.class, r, "refused").why();
    }

    @Test
    void aRequestBuiltAgainstAnotherSchemaOrAbiIsRefusedOnEveryCallWhateverItAsks() {
        try (Core core = created()) {
            for (Command command : List.of(new Command.Save(), new Command.Ask(List.of(new Query.Points())),
                    new Command.Apply(List.of()), createCommand())) {
                assertEquals(new Refusal.Schema(Wire.SCHEMA, Wire.SCHEMA ^ 1),
                        refusal(core.call(new Request(Wire.ABI, Wire.SCHEMA ^ 1, command))), command.getClass().getSimpleName());
                assertEquals(new Refusal.Abi(Wire.ABI, Wire.ABI + 1),
                        refusal(core.call(new Request(Wire.ABI + 1, Wire.SCHEMA, command))));
            }
            // Nothing of those was applied or lost: the society is still there, and a good request still works.
            assertInstanceOf(Response.Saved.class, core.call(Wire.request(new Command.Save())));
        }
    }

    @Test
    void aSaveOfAFormatTheCoreCantReadIsRefusedByItsNumbersAndTheSocietyKept() {
        try (Core core = created()) {
            SaveFile save = assertInstanceOf(Response.Saved.class, core.call(Wire.request(new Command.Save()))).save();
            long format = save.format();
            assertEquals(1, format, "the first release's format: dev worlds are rebuilt until then");
            for (long other : new long[] {format + 1, 0, 0xFFFF_FFFFL}) {
                SaveFile filed = new SaveFile(other, save.schema(), "elsewhere", save.blob());
                assertEquals(new Refusal.SaveFormat(format, other), refusal(core.call(Wire.request(new Command.Load(filed)))),
                        "format " + other);
            }
            // The blob's own header must say what the file says.
            byte[] blob = save.blob().toArray();
            blob[4] = (byte) (format + 1);
            Refusal mismatched = refusal(core.call(Wire.request(new Command.Load(
                    new SaveFile(format, save.schema(), save.core(), Bytes.of(blob))))));
            assertInstanceOf(Refusal.Save.class, mismatched);
            // The society is the one that was saved, still.
            assertEquals(save, assertInstanceOf(Response.Saved.class, core.call(Wire.request(new Command.Save()))).save());
            assertTrue(core.replay().size() >= 1);
        }
    }

    @Test
    void aSaveTheCoreWroteLoadsBackInANewCore() {
        SaveFile save;
        try (Core core = created()) {
            save = assertInstanceOf(Response.Saved.class, core.call(Wire.request(new Command.Save()))).save();
        }
        try (Core other = Core.start(Sim.compiled())) {
            assertEquals(new Response.Done(List.of(), List.of()), other.call(Wire.request(new Command.Load(save))));
            assertEquals(save.blob(), assertInstanceOf(Response.Saved.class, other.call(Wire.request(new Command.Save()))).save().blob());
        }
    }

    @Test
    void aCoreThatReadsAnotherSchemaIsSaidAtStartNotAtTheFirstCreate() {
        IllegalStateException e = assertThrows(IllegalStateException.class,
                () -> Core.startClaiming(Sim.compiled(), Wire.SCHEMA ^ 1));
        assertTrue(e.getMessage().contains("two commits") && e.getMessage().contains(Long.toHexString(Wire.SCHEMA)), e.getMessage());
    }
}
