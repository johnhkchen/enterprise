package dev.b28.enterprise.locker;

import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.level.Level;
import net.minecraft.world.phys.AABB;
import net.neoforged.neoforge.event.entity.EntityJoinLevelEvent;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * Lockers keep trading while their village isn't running: when a villager with a locker loads back in (a player came
 * by, or a {@link Rounds round} loaded it), or a round finds it loaded but idle, it replays the restocks it missed,
 * two a day. The replay is spread over
 * ticks within a budget and waits while the server is busy.
 */
public final class LockerCatchUp {
    /** Days of missed restocks a villager replays at most. */
    public static final int MAX_DAYS = 60;

    /** Wait this long after loading, so the Merchant's desk and chests nearby have loaded too. */
    private static final long DELAY_TICKS = 40;

    /** A villager waiting for its Merchant to load looks again after this long. */
    private static final long RECHECK_TICKS = 100;

    /** Restocks replayed per server tick, across all villagers. */
    private static final int RESTOCKS_PER_TICK = 8;

    private static final class Job {
        final ResourceKey<Level> dimension;
        final UUID villager;
        long due;
        int remaining = -1; // counted when the job starts
        long settledUntil; // the villager's restock clock once the replay is done

        Job(ResourceKey<Level> dimension, UUID villager, long due) {
            this.dimension = dimension;
            this.villager = villager;
            this.due = due;
        }
    }

    // One job per villager, in the order they were queued.
    private static final Map<UUID, Job> JOBS = new LinkedHashMap<>();

    private LockerCatchUp() {}

    public static void onJoin(EntityJoinLevelEvent event) {
        if (event.loadedFromDisk()
                && event.getLevel() instanceof ServerLevel level
                && event.getEntity() instanceof Villager villager) {
            enqueue(level, villager);
        }
    }

    /** Queues catch-up for the villagers already loaded within {@code radius} of {@code pos}, ticking or not. */
    public static void enqueueNear(ServerLevel level, BlockPos pos, int radius) {
        level.getEntitiesOfClass(Villager.class, new AABB(pos).inflate(radius)).forEach(v -> enqueue(level, v));
    }

    private static void enqueue(ServerLevel level, Villager villager) {
        JOBS.putIfAbsent(villager.getUUID(),
                new Job(level.dimension(), villager.getUUID(), level.getGameTime() + DELAY_TICKS));
    }

    public static void onServerTick(ServerTickEvent.Post event) {
        runJobs(event.getServer());
    }

    /** One tick's worth of replays, within the budget. */
    public static void runJobs(MinecraftServer server) {
        if (JOBS.isEmpty() || !Rounds.hasHeadroom(server)) {
            return;
        }
        int budget = RESTOCKS_PER_TICK;
        for (Iterator<Job> it = JOBS.values().iterator(); it.hasNext() && budget > 0; ) {
            Job job = it.next();
            ServerLevel level = server.getLevel(job.dimension);
            if (level == null) {
                it.remove();
                continue;
            }
            if (level.getGameTime() < job.due) {
                continue;
            }
            if (!(level.getEntity(job.villager) instanceof Villager villager)) {
                it.remove(); // unloaded again, or gone; it will catch up next time it loads
                continue;
            }
            if (job.remaining < 0) {
                boolean locker = LockerTrading.hasLocker(villager);
                if (locker && !LockerTrading.managed(villager)) {
                    // Its Merchant isn't loaded (a caravan loaded only the desk's chunk, say): replaying now would
                    // spend the missed restocks with no one to trade through. Look again later; a round loads the
                    // whole market.
                    job.due = level.getGameTime() + RECHECK_TICKS;
                    continue;
                }
                if (locker) {
                    startClock(villager);
                }
                job.remaining = locker ? missedRestocks(villager) : 0;
                job.settledUntil = settledUntil(villager);
            }
            while (job.remaining > 0 && budget > 0) {
                villager.restock();
                job.remaining--;
                budget--;
            }
            if (job.remaining == 0) {
                villager.lastRestockGameTime = job.settledUntil;
                it.remove();
            }
        }
    }

    /** Replays every restock this villager missed at once, if it has a locker. Returns restocks replayed. */
    public static int catchUp(Villager villager) {
        if (!LockerTrading.hasLocker(villager)) {
            return 0;
        }
        startClock(villager);
        int restocks = missedRestocks(villager);
        long settled = settledUntil(villager);
        for (int i = 0; i < restocks; i++) {
            villager.restock();
        }
        villager.lastRestockGameTime = settled;
        return restocks;
    }

    /**
     * The restock clock after replaying the whole days owed: advanced by exactly those days, not set to now, so the
     * part-day left over counts toward the next catch-up instead of being dropped.
     */
    private static long settledUntil(Villager villager) {
        long last = villager.lastRestockGameTime;
        long now = villager.level().getGameTime();
        if (last == 0) {
            return 0;
        }
        long days = (now - last) / 24000L;
        return days > MAX_DAYS ? now : last + days * 24000L;
    }

    /**
     * A villager with a locker that has never restocked starts its restock clock now, so time from here on counts.
     * Otherwise a village set up and left alone before its villagers' first restock would never trade.
     */
    private static void startClock(Villager villager) {
        if (villager.lastRestockGameTime == 0) {
            villager.lastRestockGameTime = villager.level().getGameTime();
        }
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
