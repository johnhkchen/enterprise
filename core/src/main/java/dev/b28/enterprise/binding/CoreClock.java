package dev.b28.enterprise.binding;

import net.minecraft.server.MinecraftServer;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * The game's time, into the core (docs/binding-plan.md 1.5): the one server-tick listener the binding has. Every
 * {@link #EVERY} ticks of the overworld's game time it has the host send a look (the adapters' happenings and an
 * {@code Advance} to now); every tick it shows the outputs of looks that are done, and counts the chests in chunks that
 * loaded since (nothing when none did). Nothing else runs per tick.
 *
 * <p>{@link #EVERY} is how often the binding crosses to the core, not a rule: an {@code Advance} runs the core's own
 * looks ({@code look_ticks}, a dial of the core's) up to the tick it's given, however far that is.
 */
public final class CoreClock {
    /** A crossing a game minute, as the core's own look by default. */
    public static final long EVERY = 1200;

    private CoreClock() {}

    public static void onServerTick(ServerTickEvent.Post event) {
        MinecraftServer server = event.getServer();
        CoreHost host = CoreHost.of(server);
        if (host == null) {
            return;
        }
        host.poll();
        long now = server.overworld().getGameTime();
        if (now % EVERY == 0) {
            host.look(server, now);
        } else {
            host.catchUp(server);
        }
    }
}
