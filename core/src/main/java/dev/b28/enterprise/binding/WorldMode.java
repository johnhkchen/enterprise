package dev.b28.enterprise.binding;

import dev.b28.enterprise.Enterprise;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Locale;

/**
 * Which society a world began under (docs/alpha-plan.md 4): {@value #FILE} in the world's folder records it at the
 * world's first start, and a world opened under the other one pauses the core and says why, rather than drift (the
 * Java rules' saved data and the core's save each going stale under the other). A world with no marker that has the
 * Java rules' saved data in {@code data/} and no core save beside it, or the Java caravans' book, began in Java mode.
 */
public final class WorldMode {
    public static final String FILE = "enterprise_society.txt";
    /** Saved data the Java rules' caravans keep (core mode keeps its caravans in the core's save). */
    private static final String JAVA_CARAVANS = "enterprise_caravans.dat";

    private WorldMode() {}

    /** Why the world at {@code dir} can't run under {@code mode}, or null when it can (the marker written if it had none). */
    public static String check(Path dir, Society mode) {
        Society began = began(dir);
        if (began == null) {
            write(dir, mode);
            return null;
        }
        if (began == mode) {
            return null;
        }
        return "this world began in %s mode and this game runs in %s mode; set society=%s in config/enterprise.properties "
                .formatted(name(began), name(mode), name(began))
                + "(or ENTERPRISE_SOCIETY) to open it, or start a new world";
    }

    /** The society the world's marker names, or Java for a marker-less world with the Java rules' data, else null. */
    static Society began(Path dir) {
        Path marker = dir.resolve(FILE);
        if (Files.isRegularFile(marker)) {
            try {
                return Society.parse(null, null, Files.readString(marker));
            } catch (IOException e) {
                Enterprise.LOGGER.warn("couldn't read {}: {}", marker, e.toString());
                return null;
            }
        }
        Path data = dir.resolve("data");
        boolean coreSave = Files.isRegularFile(dir.resolve(dev.b28.enterprise.core.SaveStore.FILE));
        if (Files.isRegularFile(data.resolve(JAVA_CARAVANS)) || (!coreSave && javaData(data))) {
            write(dir, Society.JAVA);
            return Society.JAVA;
        }
        return null;
    }

    private static boolean javaData(Path data) {
        if (!Files.isDirectory(data)) {
            return false;
        }
        try (var files = Files.list(data)) {
            return files.anyMatch(f -> f.getFileName().toString().startsWith("enterprise_"));
        } catch (IOException e) {
            return false;
        }
    }

    private static void write(Path dir, Society mode) {
        try {
            Files.writeString(dir.resolve(FILE), name(mode) + "\n");
        } catch (IOException e) {
            Enterprise.LOGGER.warn("couldn't write {}: {}", dir.resolve(FILE), e.toString());
        }
    }

    private static String name(Society mode) {
        return mode.name().toLowerCase(Locale.ROOT);
    }
}
