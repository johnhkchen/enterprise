package dev.b28.enterprise.binding;

import dev.b28.enterprise.Enterprise;
import java.io.IOException;
import java.io.Reader;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Locale;
import java.util.Properties;
import net.neoforged.fml.loading.FMLPaths;

/**
 * Which society runs the world's rules (docs/binding-plan.md B3.2): the Rust core ({@link #CORE}, the default since the
 * alpha, docs/alpha-plan.md 4), whose adapters make the mod a view, or the Java rule classes ({@link #JAVA}). Chosen once, as the mod loads, so the
 * rule tick listeners are registered or not and never change under a running server.
 *
 * <p>The choice is read in this order: the system property {@value #PROPERTY} (what {@code just gametest-core} sets),
 * the environment variable {@value #ENV}, then {@code society=java|core} in {@code config/enterprise.properties}. Only
 * {@code java} is Java; anything else, or nothing, is the core.
 */
public enum Society {
    JAVA,
    CORE;

    public static final String PROPERTY = "enterprise.society";
    public static final String ENV = "ENTERPRISE_SOCIETY";
    public static final String FILE = "enterprise.properties";

    private static Society chosen;

    /** The society this run uses; read once. */
    public static synchronized Society mode() {
        if (chosen == null) {
            chosen = parse(System.getProperty(PROPERTY), System.getenv(ENV), fromConfigFile());
            Enterprise.LOGGER.info("society = {}", chosen.name().toLowerCase(Locale.ROOT));
        }
        return chosen;
    }

    public static boolean isCore() {
        return mode() == CORE;
    }

    /** The first of the three that says something, as a society; the core if none does or it isn't {@code java}. */
    static Society parse(String property, String env, String file) {
        for (String s : new String[] {property, env, file}) {
            if (s != null && !s.isBlank()) {
                return "java".equals(s.strip().toLowerCase(Locale.ROOT)) ? JAVA : CORE;
            }
        }
        return CORE;
    }

    /**
     * Whether the server's core writes a journal of the run (`binding/core/Journal`, in the world's folder): yes unless
     * the system property {@code enterprise.journal}, the environment variable {@code ENTERPRISE_JOURNAL} or {@code
     * journal=} in {@code config/enterprise.properties}, the first that says something, says {@code off}.
     */
    public static boolean journaling() {
        for (String s : new String[] {System.getProperty("enterprise.journal"), System.getenv("ENTERPRISE_JOURNAL"),
                fromConfigFile("journal")}) {
            if (s != null && !s.isBlank()) {
                return !"off".equals(s.strip().toLowerCase(Locale.ROOT));
            }
        }
        return true;
    }

    private static String fromConfigFile() {
        return fromConfigFile("society");
    }

    private static String fromConfigFile(String key) {
        Path file = FMLPaths.CONFIGDIR.get().resolve(FILE);
        if (!Files.isRegularFile(file)) {
            return null;
        }
        try (Reader in = Files.newBufferedReader(file)) {
            Properties p = new Properties();
            p.load(in);
            return p.getProperty(key);
        } catch (IOException e) {
            Enterprise.LOGGER.warn("couldn't read {}: {}", file, e.toString());
            return null;
        }
    }
}
