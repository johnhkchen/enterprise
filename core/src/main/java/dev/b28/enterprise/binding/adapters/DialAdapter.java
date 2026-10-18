package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.DialView;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Refusal;
import dev.b28.enterprise.wire.Response;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;

/**
 * The world's dials, from the core (docs/binding-plan.md 2, concern 1): {@code /enterprise dial} lists the core's
 * ({@code Query.Dials}: the value now, the value the world started at, what it does) and sets one with a {@code
 * SetDial} happening, or puts it back to the world's start (the content pack's own, {@link
 * dev.b28.enterprise.binding.ModPack#dials}). The core keeps them, saved with its society, and checks the name and
 * the value; this adapter keeps none and knows no dial's name.
 *
 * <p>In Java mode the Java rules still read {@link dev.b28.enterprise.Dials}, so the command sets those first and this
 * adapter mirrors the set into the core where the core has the dial ({@link #mirror}), so the core beside the Java
 * rules says the same ({@link #sync} at the server's start, for what the world had set before). In core mode the Java
 * catalogue isn't read by any rule and the command goes through here alone.
 */
public final class DialAdapter {
    private DialAdapter() {}

    /** Every dial the core lists, in its order. Throws like {@link CoreHost#ask} when the host is paused. */
    public static List<DialView> list(CoreHost host) {
        return host.ask(List.of(new Query.Dials())).stream()
                .filter(Answer.Dials.class::isInstance).map(a -> ((Answer.Dials) a).dials()).findFirst().orElse(List.of());
    }

    /** The names of the core's dials, for the command's suggestions; none when the core isn't there to ask. */
    public static List<String> names(CoreHost host) {
        if (host == null || host.pausedWhy() != null) {
            return List.of();
        }
        return list(host).stream().map(DialView::name).toList();
    }

    /**
     * Sets a dial in the core ({@code value}), or puts it back to the world's start ({@code null}). Returns null when
     * it's done, or why the core wouldn't: a name it hasn't, a value the dial won't take, or the host isn't running.
     */
    public static String set(CoreHost host, String name, Double value) {
        if (host == null) {
            return "The core isn't running";
        }
        if (host.pausedWhy() != null) {
            return host.pausedWhy();
        }
        Response r = host.apply(List.of(new Input.SetDial(name, Optional.ofNullable(value))));
        return switch (r) {
            case Response.Done done -> null;
            case Response.Refused refused -> refused.why() instanceof Refusal.Input input ? input.why() : refused.why().toString();
            case Response.Saved saved -> "the core answered a dial with a save";
        };
    }

    /**
     * Java mode: the core follows a dial the Java rules were just given. A dial the core hasn't (the mod's {@code
     * JAVA_ONLY} ones) is nothing to it and not an error; any other refusal is logged.
     */
    public static void mirror(CoreHost host, String name, Double value) {
        if (host == null || host.pausedWhy() != null) {
            return;
        }
        String why = set(host, name, value);
        if (why != null && !why.startsWith("no dial called")) {
            Enterprise.LOGGER.warn("The core wouldn't take dial {}: {}", name, why);
        }
    }

    /** Java mode, at the server's start: the core follows every dial the world had set before it started. */
    public static void sync(CoreHost host, Map<String, Double> settings) {
        if (host == null || host.pausedWhy() != null || settings.isEmpty()) {
            return;
        }
        List<Input> inputs = new ArrayList<>();
        List<String> known = names(host);
        settings.forEach((name, value) -> {
            if (known.contains(name)) {
                inputs.add(new Input.SetDial(name, Optional.of(value)));
            }
        });
        if (!inputs.isEmpty()) {
            host.apply(inputs);
        }
    }
}
