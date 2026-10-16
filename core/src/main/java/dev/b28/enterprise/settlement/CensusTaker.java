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
import net.minecraft.core.SectionPos;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.tags.PoiTypeTags;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.entity.ai.village.poi.PoiTypes;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import dev.b28.enterprise.territory.Territory;
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
        final List<Integer> farmers = new ArrayList<>(); // levels of the farmers counted
        // What the village's people would buy for emeralds, item id -> items a day; and each such trade by job site.
        final java.util.Map<String, Double> demand = new java.util.HashMap<>();
        final List<Territory.VillageTrade> trades = new ArrayList<>();
        final java.util.Map<UUID, Settlements.Resident> seen = new java.util.LinkedHashMap<>();
        java.util.Set<BlockPos> lockers; // job sites the town's Merchant manages as lockers, found when first wanted

        /** Job sites the Merchant trades at: a villager's trades meet the town's goods only through a locker. */
        java.util.Set<BlockPos> lockers() {
            if (lockers == null) {
                lockers = new java.util.HashSet<>();
                Territory.nearestDesk(level, bell).ifPresent(desk -> dev.b28.enterprise.merchant.Merchant
                        .workingAt(level, desk, dev.b28.enterprise.locker.Lockers.MERCHANT_RANGE)
                        .ifPresent(merchant -> dev.b28.enterprise.locker.Lockers.near(level, desk,
                                        dev.b28.enterprise.locker.Lockers.MERCHANT_RANGE,
                                        dev.b28.enterprise.merchant.Merchant.lockerLimit(
                                                merchant.getVillagerData().getLevel()))
                                .forEach(locker -> lockers.add(locker.workstation()))));
            }
            return lockers;
        }

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

    /** Bells a round has just loaded: counted next, ahead of the polling schedule. */
    private static final java.util.Deque<Bell> requested = new ArrayDeque<>();

    /** Counts this bell next (a round has loaded it), if it's due. */
    public static void request(ServerLevel level, BlockPos bell) {
        requested.add(new Bell(level, bell));
    }

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
        while (!requested.isEmpty()) {
            Bell bell = requested.poll();
            var last = Settlements.get(server).census(GlobalPos.of(bell.level().dimension(), bell.pos()));
            long now = bell.level().getGameTime();
            if (last.isEmpty() || now - last.get().gameTime() >= INTERVAL_TICKS) {
                current = start(bell.level(), bell.pos(), RADIUS);
                return;
            }
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
        // Known settlements whose bell a round has loaded are counted too, once their villagers have loaded:
        // entities load a moment after their chunk, and counting before then finds an empty village.
        for (GlobalPos known : Settlements.get(server).all().keySet()) {
            ServerLevel level = server.getLevel(known.dimension());
            if (level != null && level.isLoaded(known.pos()) && entitiesLoadedAround(level, known.pos())) {
                lookouts.add(new Bell(level, known.pos()));
            }
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

    /** Entities loaded in the chunks a round loads around the bell: the bell's block and its neighbours. */
    private static boolean entitiesLoadedAround(ServerLevel level, BlockPos pos) {
        for (int dx = -1; dx <= 1; dx++) {
            for (int dz = -1; dz <= 1; dz++) {
                BlockPos around = pos.offset(dx, 0, dz);
                long chunk = ChunkPos.asLong(
                        SectionPos.blockToSectionCoord(around.getX()), SectionPos.blockToSectionCoord(around.getZ()));
                if (!level.areEntitiesLoaded(chunk)) {
                    return false;
                }
            }
        }
        return true;
    }

    private static Job start(ServerLevel level, BlockPos bell, int radius) {
        PoiManager pois = level.getPoiManager();
        pois.ensureLoadedAndValid(level, bell, radius);
        int beds = (int) pois.getInRange(type -> type.is(PoiTypes.HOME), bell, radius, PoiManager.Occupancy.ANY).count();
        int workstations = (int) pois.getInRange(
                type -> type.is(PoiTypeTags.ACQUIRABLE_JOB_SITE), bell, radius, PoiManager.Occupancy.ANY).count();
        List<UUID> villagers = new ArrayList<>();
        level.getEntitiesOfClass(Villager.class, new AABB(bell).inflate(radius),
                        villager -> villager.blockPosition().closerThan(bell, radius))
                .forEach(villager -> villagers.add(villager.getUUID()));
        return new Job(level, bell, radius, villagers, beds, workstations);
    }

    private static void work(MinecraftServer server, Job job, int budget) {
        while (budget > 0 && !job.villagers.isEmpty()) {
            if (job.level.getEntity(job.villagers.poll()) instanceof Villager villager) {
                job.counted++;
                boolean baby = villager.isBaby();
                if (!baby && villager.getVillagerData().getProfession() == VillagerProfession.FARMER) {
                    job.farmers.add(villager.getVillagerData().getLevel());
                }
                boolean free = !baby && isFree(villager, job.bell); // a baby is a resident, not yet a worker
                if (!baby) {
                    noteDemand(villager, job);
                }
                if (free) {
                    job.free++;
                }
                job.seen.put(villager.getUUID(), new Settlements.Resident(villager.getUUID(),
                        villager.chunkPosition().toLong(),
                        villager.getBrain().getMemory(MemoryModuleType.HOME).map(GlobalPos::pos),
                        net.minecraft.core.registries.BuiltInRegistries.VILLAGER_PROFESSION
                                .getKey(villager.getVillagerData().getProfession()).toString(),
                        villager.getVillagerData().getLevel(), free, baby, job.level.getGameTime(), false));
                budget -= 3;
            }
        }
        if (job.villagers.isEmpty() && job == current) {
            finish(server, job);
            current = null;
        }
    }

    /**
     * The villager's trades that buy one good for emeralds, switched on or not: what the village would buy. Vanilla
     * restocks twice a day, so a trade takes its cost times its uses twice a day. Only a villager at a locker the
     * Merchant manages buys from the town: one waiting for a higher Merchant level, or with no chest, buys nothing.
     */
    private static void noteDemand(Villager villager, Job job) {
        var site = villager.getBrain().getMemory(MemoryModuleType.JOB_SITE).map(GlobalPos::pos);
        if (site.isEmpty() || !job.lockers().contains(site.get())) {
            return;
        }
        for (var offer : villager.getOffers()) {
            if (!offer.getResult().is(net.minecraft.world.item.Items.EMERALD) || !offer.getCostB().isEmpty()
                    || offer.getBaseCostA().isEmpty()) {
                continue;
            }
            String item = net.minecraft.core.registries.BuiltInRegistries.ITEM.getKey(offer.getBaseCostA().getItem()).toString();
            job.demand.merge(item, 2.0 * offer.getBaseCostA().getCount() * offer.getMaxUses(), Double::sum);
            site.ifPresent(at -> job.trades.add(
                    new Territory.VillageTrade(at, dev.b28.enterprise.merchant.TradeKeys.of(offer), item)));
        }
    }

    private static final net.minecraft.server.level.TicketType<ChunkPos> RESIDENT = net.minecraft.server.level.TicketType
            .create("enterprise_resident", java.util.Comparator.comparingLong(ChunkPos::toLong), 400);

    /** Loads the chunks round where a resident was last seen, for a while: it's found at the next count. */
    private static void sendFor(ServerLevel level, ChunkPos chunk) {
        level.getChunkSource().addRegionTicket(RESIDENT, chunk, 1, chunk);
    }

    /** Residents unseen this long are taken to have left. */
    private static final long FORGET_AFTER_TICKS = 10 * 24000L;

    private static Census finish(MinecraftServer server, Job job) {
        GlobalPos bell = GlobalPos.of(job.level.dimension(), job.bell);
        Settlements settlements = Settlements.get(server);
        // Residents not seen still count as last seen: out with a caravan, in a chunk that isn't loaded, or wandered off
        // the chunks loaded round the town (a villager walking out of them stops where the loaded ground ends). Those at
        // home are sent for: the chunks round where they were last seen are loaded, so the next count finds them. Only
        // death (see Settlements#died), or days of being looked for where it was and not found, takes one off the roster.
        java.util.Map<UUID, Settlements.Resident> roster = new java.util.LinkedHashMap<>(job.seen);
        long now = job.level.getGameTime();
        for (Settlements.Resident resident : settlements.roster(bell).values()) {
            if (roster.containsKey(resident.id())) {
                continue;
            }
            if (now - resident.lastSeen() > FORGET_AFTER_TICKS && !resident.away()) {
                continue;
            }
            if (!resident.away()) {
                if (!job.level.areEntitiesLoaded(resident.chunk())) {
                    resident = resident.assumedAt(now); // nobody looked: it's where it was
                }
                sendFor(job.level, new ChunkPos(resident.chunk()));
            }
            roster.put(resident.id(), resident);
            job.counted++;
            if (resident.free()) {
                job.free++;
            }
            if (!resident.baby() && !resident.away() && resident.profession().equals("minecraft:farmer")) {
                job.farmers.add(resident.level());
            }
        }
        Census result = new Census(job.counted, job.free, job.beds, job.workstations, now);
        if (incomplete(settlements.census(bell), result)) {
            Enterprise.LOGGER.debug("Census at {} looks incomplete ({} villagers); not recorded, will retry",
                    job.bell.toShortString(), result.villagers());
            return result;
        }
        // Culture for the days since the last census; a first census counts as one day.
        double days = settlements.census(bell)
                .map(last -> Math.max(0, result.gameTime() - last.gameTime()) / 24000.0)
                .orElse(1.0);
        if (result.isSettlement() || job.counted > 0) {
            settlements.record(bell, result);
            settlements.setRoster(bell, roster);
        } else {
            settlements.forget(bell);
        }
        if (result.isSettlement()) {
            Territory territory = Territory.get(server);
            // The Merchant reviews its clients: the lockers it serves (counted from tomorrow's census).
            Territory.nearestDesk(job.level, job.bell).ifPresent(desk -> dev.b28.enterprise.locker.Lockers.review(job.level, desk));
            territory.recordDemand(bell, job.demand);
            territory.afterCensus(job.level, job.bell, result, days, job.farmers);
            territory.mediate(job.level, bell, job.trades);
        }
        Enterprise.LOGGER.debug("Census at {}: {} villagers ({} free), {} beds, {} workstations, value {}",
                job.bell.toShortString(), result.villagers(), result.free(), result.beds(), result.workstations(),
                result.value());
        return result;
    }

    /**
     * Fewer than half the villagers of a census from the last 3 days: probably villagers still loading in the
     * market's other chunks. A real collapse is recorded once the last good census is 3 days old.
     */
    static boolean incomplete(Optional<Census> previous, Census result) {
        return previous.isPresent()
                && result.villagers() < previous.get().villagers() / 2.0
                && result.gameTime() - previous.get().gameTime() < 3 * 24000L;
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
