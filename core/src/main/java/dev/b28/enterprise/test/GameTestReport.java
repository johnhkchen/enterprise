package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import net.minecraft.gametest.framework.GameTestInfo;
import net.minecraft.gametest.framework.GlobalTestReporter;
import net.minecraft.gametest.framework.LogTestReporter;
import net.minecraft.gametest.framework.TestReporter;
import net.neoforged.neoforge.event.server.ServerStartingEvent;

/**
 * One line a game test, for {@code just gametest-core} (docs/binding-plan.md B3.2): when the system property {@value
 * #PROPERTY} names a file, every test's result is appended to it as {@code pass|fail<TAB>name}, beside what the game's
 * own log reporter says. The run's script (scripts/gametest_core.py) counts passes per class from it and checks them
 * against the committed baseline. Off unless the property is set; the file is the run's, rewritten from empty.
 */
public final class GameTestReport implements TestReporter {
    public static final String PROPERTY = "enterprise.gametest.report";

    private final Path file;
    private final TestReporter log = new LogTestReporter();

    private GameTestReport(Path file) {
        this.file = file;
    }

    public static void onServerStarting(ServerStartingEvent event) {
        String where = System.getProperty(PROPERTY);
        if (where == null || where.isBlank()) {
            return;
        }
        Path file = Path.of(where);
        try {
            Files.createDirectories(file.toAbsolutePath().getParent());
            Files.writeString(file, "");
        } catch (IOException e) {
            Enterprise.LOGGER.error("game test report {} unusable: {}", file, e.toString());
            return;
        }
        GlobalTestReporter.replaceWith(new GameTestReport(file));
    }

    @Override
    public void onTestFailed(GameTestInfo test) {
        log.onTestFailed(test);
        append((test.isRequired() ? "fail" : "optional-fail") + "\t" + test.getTestName());
    }

    @Override
    public void onTestSuccess(GameTestInfo test) {
        log.onTestSuccess(test);
        append("pass\t" + test.getTestName());
    }

    private void append(String line) {
        try {
            Files.writeString(file, line + "\n", StandardOpenOption.APPEND);
        } catch (IOException e) {
            Enterprise.LOGGER.error("game test report {}: {}", file, e.toString());
        }
    }
}
