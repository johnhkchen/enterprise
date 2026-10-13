package dev.b28.enterprise.settlement;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.locker.Rounds;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.HashSet;
import java.util.List;
import java.util.Optional;
import java.util.Set;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.tags.PoiTypeTags;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.entity.ai.village.poi.PoiTypes;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.level.PathNavigationRegion;
import net.minecraft.world.level.pathfinder.Path;
import net.minecraft.world.level.pathfinder.PathFinder;
import net.minecraft.world.level.pathfinder.WalkNodeEvaluator;
import net.minecraft.world.phys.AABB;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * Counts settlements: around a bell, its villagers, beds and workstations, and which villagers are free (can walk to
 * their own bed, their workstation and the bell). Polled, never constant: bells near players are counted at most
 * once a day each, when the server has room, with path checks spread over ticks.
 */
public final class CensusTaker {
    /** How far a settlement reaches from its bell. */
    public static final int RADIUS = 48;

    /** Bells this close to a player are found and counted. */
    private static final int DISCOVERY_RANGE = 64;

    private static final long CHECK_EVERY_TICKS = 200;
    public static final long INTERVAL_TICKS = 24000;

    /** Path checks per tick, across the census in progress. */
    private static final int PATHS_PER_TICK = 6;

    private static final class Job {
        final ServerLevel level;
        final BlockPos bell;
        final int radius;
        final Deque<UUID> villagers;
        final int beds;
        final int workstations;
        int counted;
        int free;

        Job(ServerLevel level, BlockPos bell, int radius, List<UUID> villagers, int beds, int workstations) {
            this.level = level;
            this.bell = bell;
            this.radius = radius;
            this.villagers = new ArrayDeque<>(villagers);
            this.beds = beds;
            this.workstations = workstations;
        }
    }

    private static Job current;

    private CensusTaker() {}

    public static void onServerTick(ServerTickEvent.Post event) {
        MinecraftServer server = event.getServer();
        if (!Rounds.hasHeadroom(server)) {
            return;
        }
        if (current != null) {
            work(server, current, PATHS_PER_TICK);
            return;
        }
        if (server.overworld().getGameTime() % CHECK_EVERY_TICKS == 0) {
            nextDueBell(server).ifPresent(bell -> current = start(bell.level(), bell.pos(), RADIUS));
        }
    }

    /** Takes a whole census now, on the spot. For commands and tests; the poller spreads the same work out. */
    public static Census countNow(ServerLevel level, BlockPos bell, int radius) {
        Job job = start(level, bell, radius);
        while (!job.villagers.isEmpty()) {
            work(level.getServer(), job, Integer.MAX_VALUE);
        }
        return finish(level.getServer(), job);
    }

    /** A position in a level: a bell, or a point to look for bells around. */
    public record Bell(ServerLevel level, BlockPos pos) {}

    /** The bell near a player that has gone longest without a census, if any is due. */
    public static Optional<Bell> nextDueBell(MinecraftServer server) {
        List<Bell> lookouts = new ArrayList<>();
        for (ServerPlayer player : server.getPlayerList().getPlayers()) {
            lookouts.add(new Bell(player.serverLevel(), player.blockPosition()));
        }
        return nextDueBell(server, lookouts);
    }

    /** The bell near any of the lookout points that has gone longest without a census, if any is due. */
    public static Optional<Bell> nextDueBell(MinecraftServer server, List<Bell> lookouts) {
        Settlements settlements = Settlements.get(server);
        long now = server.overworld().getGameTime();
        Bell due = null;
        long oldest = Long.MAX_VALUE;
        Set<GlobalPos> seen = new HashSet<>();
        for (Bell lookout : lookouts) {
            ServerLevel level = lookout.level();
            List<BlockPos> bells = level.getPoiManager()
                    .findAll(type -> type.is(PoiTypes.MEETING), pos -> true, lookout.pos(),
                            DISCOVERY_RANGE, PoiManager.Occupancy.ANY)
                    .toList();
            for (BlockPos pos : bells) {
                GlobalPos bell = GlobalPos.of(level.dimension(), pos);
                if (!seen.add(bell)) {
                    continue;
                }
                Optional<Census> last = settlements.census(bell);
                if (last.isPresent() && now - last.get().gameTime() < INTERVAL_TICKS) {
                    continue;
                }
                long counted = last.map(Census::gameTime).orElse(-1L); // never counted sorts first
                if (due == null || counted < oldest) {
                    oldest = counted;
                    due = new Bell(level, pos);
                }
            }
        }
        return Optional.ofNullable(due);
    }

    private static Job start(ServerLevel level, BlockPos bell, int radius) {
        PoiManager pois = level.getPoiManager();
        pois.ensureLoadedAndValid(level, bell, radius);
        int beds = (int) pois.getInRange(type -> type.is(PoiTypes.HOME), bell, radius, PoiManager.Occupancy.ANY).count();
        int workstations = (int) pois.getInRange(
                type -> type.is(PoiTypeTags.ACQUIRABLE_JOB_SITE), bell, radius, PoiManager.Occupancy.ANY).count();
        List<UUID> villagers = new ArrayList<>();
        level.getEntitiesOfClass(Villager.class, new AABB(bell).inflate(radius), villager -> !villager.isBaby()
                        && villager.blockPosition().closerThan(bell, radius))
                .forEach(villager -> villagers.add(villager.getUUID()));
        return new Job(level, bell, radius, villagers, beds, workstations);
    }

    private static void work(MinecraftServer server, Job job, int budget) {
        while (budget > 0 && !job.villagers.isEmpty()) {
            if (job.level.getEntity(job.villagers.poll()) instanceof Villager villager) {
                job.counted++;
                if (isFree(villager, job.bell)) {
                    job.free++;
                }
                budget -= 3;
            }
        }
        if (job.villagers.isEmpty() && job == current) {
            finish(server, job);
            current = null;
        }
    }

    private static Census finish(MinecraftServer server, Job job) {
        Census result = new Census(job.counted, job.free, job.beds, job.workstations, job.level.getGameTime());
        GlobalPos bell = GlobalPos.of(job.level.dimension(), job.bell);
        if (result.isSettlement() || job.counted > 0) {
            Settlements.get(server).record(bell, result);
        } else {
            Settlements.get(server).forget(bell);
        }
        Enterprise.LOGGER.debug("Census at {}: {} villagers ({} free), {} beds, {} workstations, value {}",
                job.bell.toShortString(), result.villagers(), result.free(), result.beds(), result.workstations(),
                result.value());
        return result;
    }

    /** Can this villager walk to its own bed, its workstation (if it has one) and the bell? */
    public static boolean isFree(Villager villager, BlockPos bell) {
        var brain = villager.getBrain();
        var home = brain.getMemory(MemoryModuleType.HOME);
        if (home.isEmpty() || !reaches(villager, home.get().pos(), 1)) {
            return false;
        }
        var job = brain.getMemory(MemoryModuleType.JOB_SITE);
        if (job.isPresent() && !reaches(villager, job.get().pos(), 1)) {
            return false;
        }
        return reaches(villager, bell, 2);
    }

    /**
     * A walking path from the villager to {@code target}, under villager rules (doors open). Uses its own path
     * finder: the villager's navigation won't plan while the villager is off the ground (jumping, sleeping, just
     * spawned), and the census shouldn't depend on that.
     */
    private static boolean reaches(Villager villager, BlockPos target, int accuracy) {
        BlockPos from = villager.blockPosition();
        if (from.closerThan(target, accuracy + 1)) {
            return true;
        }
        int range = (int) Math.min(Math.sqrt(from.distSqr(target)) + 16, 2.0 * RADIUS);
        WalkNodeEvaluator walker = new WalkNodeEvaluator();
        walker.setCanPassDoors(true);
        walker.setCanOpenDoors(true);
        PathFinder finder = new PathFinder(walker, range * 16);
        PathNavigationRegion region =
                new PathNavigationRegion(villager.level(), from.offset(-range, -range, -range), from.offset(range, range, range));
        Path path = finder.findPath(region, villager, Set.of(target), range, accuracy, 1.0F);
        return path != null && path.canReach();
    }
}
