package dev.b28.enterprise.locker;

import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.UUID;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.level.Level;
import net.neoforged.neoforge.event.entity.EntityJoinLevelEvent;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * Lockers keep trading while their village is unloaded: when a villager with a locker loads back in, it replays the
 * restocks it missed (two a day), each running the usual locker trade. Computed on touch, not ticked.
 */
public final class LockerCatchUp {
    /** Days of missed restocks a villager replays at most. */
    public static final int MAX_DAYS = 60;

    /** Wait this long after loading, so the Merchant's desk and chests nearby have loaded too. */
    private static final long DELAY_TICKS = 40;

    private record Pending(ResourceKey<Level> dimension, UUID villager, long due) {}

    private static final List<Pending> PENDING = new ArrayList<>();

    private LockerCatchUp() {}

    public static void onJoin(EntityJoinLevelEvent event) {
        if (event.loadedFromDisk()
                && event.getLevel() instanceof ServerLevel level
                && event.getEntity() instanceof Villager villager) {
            PENDING.add(new Pending(level.dimension(), villager.getUUID(), level.getGameTime() + DELAY_TICKS));
        }
    }

    public static void onServerTick(ServerTickEvent.Post event) {
        if (PENDING.isEmpty()) {
            return;
        }
        MinecraftServer server = event.getServer();
        for (Iterator<Pending> it = PENDING.iterator(); it.hasNext(); ) {
            Pending pending = it.next();
            ServerLevel level = server.getLevel(pending.dimension());
            if (level == null) {
                it.remove();
            } else if (level.getGameTime() >= pending.due()) {
                it.remove();
                if (level.getEntity(pending.villager()) instanceof Villager villager) {
                    catchUp(villager);
                }
            }
        }
    }

    /** Replays the restocks this villager missed, if it has a locker. Returns restocks replayed. */
    public static int catchUp(Villager villager) {
        if (!LockerTrading.hasLocker(villager)) {
            return 0;
        }
        int restocks = missedRestocks(villager);
        for (int i = 0; i < restocks; i++) {
            villager.restock();
        }
        return restocks;
    }

    /** Two restocks per whole day since the last one, up to {@link #MAX_DAYS} days. Never restocked: none owed. */
    public static int missedRestocks(Villager villager) {
        long last = villager.lastRestockGameTime;
        if (last == 0) {
            return 0; // vanilla's value for a villager that has never restocked
        }
        long days = (villager.level().getGameTime() - last) / 24000L;
        return (int) Math.min(days, MAX_DAYS) * 2;
    }
}
