package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.binding.PausedNotice;
import dev.b28.enterprise.binding.adapters.CommandsAdapter;
import dev.b28.enterprise.core.SaveStore;
import dev.b28.enterprise.merchant.LedgerView;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.stream.Stream;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/**
 * The alpha's stubs (docs/alpha-plan.md 2): in core mode an action reaches the core or is refused in plain words. A host
 * of its own on a folder of its own, as the core binding tests have.
 */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class AlphaStubsTests {
    /**
     * A world whose save the core can't read: the host pauses, and the notice an operator gets on joining (and the
     * ledger's only line) says the society is paused and why, with the save left as it was. The refusals' words say
     * what to do instead.
     */
    @GameTest(template = "floor")
    public static void aRefusedSaveSaysTheSocietyIsPaused(GameTestHelper helper) {
        Path dir = folder();
        try {
            byte[] junk = "not a save".getBytes();
            Files.write(dir.resolve(SaveStore.FILE), junk);
            try (CoreHost host = CoreHost.open(dir, 7)) {
                helper.assertTrue(host.pausedWhy() != null, "the host runs over a save it can't read");
                String why = PausedNotice.why(host);
                helper.assertTrue(why != null && why.startsWith("The society is paused: ") && why.contains(SaveStore.FILE),
                        "the notice: " + why);
                LedgerView ledger = LedgerView.note(why);
                helper.assertTrue(ledger.isEmpty() && ledger.toText().equals(java.util.List.of(why)),
                        "the ledger: " + ledger.toText());
            }
            helper.assertTrue(java.util.Arrays.equals(Files.readAllBytes(dir.resolve(SaveStore.FILE)), junk),
                    "the refused save was changed");
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        } finally {
            delete(dir);
        }
        helper.assertTrue(CommandsAdapter.notYet("toll", "No tolls in the alpha.")
                .equals("/enterprise toll: not in the alpha yet. No tolls in the alpha."), "the not-yet words");
        helper.assertTrue(CommandsAdapter.gone("tenders", "See the contracts.")
                .equals("/enterprise tenders: gone. See the contracts."), "the gone words");
        helper.succeed();
    }

    private static Path folder() {
        try {
            return Files.createTempDirectory("enterprise-stubs-test");
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    private static void delete(Path dir) {
        try (Stream<Path> all = Files.walk(dir)) {
            all.sorted(Comparator.reverseOrder()).forEach(p -> p.toFile().delete());
        } catch (IOException e) {
            // a temporary folder left behind
        }
    }
}
