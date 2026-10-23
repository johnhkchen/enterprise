package dev.b28.enterprise.binding;

import net.minecraft.network.chat.Component;
import net.minecraft.server.level.ServerPlayer;
import net.neoforged.neoforge.event.entity.player.PlayerEvent;

/**
 * In core mode a society that isn't running says so (docs/alpha-plan.md 2): to every operator who joins, and at the top of
 * the ledger. A paused core (a save it refused, a trap) and a core that never started (a jar-in-jar failure) both show on
 * the first launch, not only in the log.
 */
public final class PausedNotice {
    private PausedNotice() {}

    /** Why the core's society isn't running, in a player's words, or null when it is (the caller knows it's core mode). */
    public static String why(CoreHost host) {
        if (host == null) {
            String failure = CoreHost.startFailure();
            return "The core didn't start; the world runs without it"
                    + (failure == null ? "" : ": " + failure) + ". See logs/latest.log.";
        }
        return host.pausedWhy() == null ? null : "The society is paused: " + host.pausedWhy();
    }

    public static void onJoin(PlayerEvent.PlayerLoggedInEvent event) {
        if (Society.isCore() && event.getEntity() instanceof ServerPlayer player && player.hasPermissions(2)) {
            String why = why(CoreHost.of(player.server));
            if (why != null) {
                player.sendSystemMessage(Component.literal(why));
            }
        }
    }
}
