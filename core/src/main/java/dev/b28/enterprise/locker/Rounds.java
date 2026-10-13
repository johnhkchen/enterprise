package dev.b28.enterprise.locker;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.MerchantBook;
import java.util.Collection;
import java.util.Comparator;
import java.util.LinkedHashSet;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.TicketType;
import net.minecraft.world.level.ChunkPos;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * Rounds: markets whose villagers aren't running (unloaded, or loaded but idle like spawn chunks with no player
 * near) get a short visit when the server has room. The most overdue Merchant's desk has its footprint (desk and
 * locker chunks) loaded on a ticket that expires by itself; its villagers, loading in or already loaded, get
 * {@link LockerCatchUp}. One round at a time, at most one per desk per in-game day, none while ticks run long.
 */
public final class Rounds {
    /** How often to consider a round. */
    private static final long CHECK_EVERY_TICKS = 200;

    /** A desk is due a round when it hasn't had one for a day. */
    public static final long INTERVAL_TICKS = 24000;

    /** Ticks a round holds the footprint after its villagers have loaded, for catch-up and the census to run. */
    private static final int WORK_TICKS = 200;

    /** A round that can't load its footprint in this much real time gives up (loading is real time, not ticks). */
    private static final long LOAD_TIMEOUT_NANOS = 30_000_000_000L;

    /** No rounds or catch-up while the average tick takes longer than this. */
    private static final long TICK_BUDGET_NANOS = 30_000_000L;

    /** At most this many chunks per round. */
    private static final int MAX_CHUNKS = 25;

    // No timeout: a round holds its chunks until it has done its work. A tick-based timeout fails under /tick sprint,
    // where 300 ticks pass in milliseconds while chunks take real time to load.
    private static final TicketType<ChunkPos> ROUND =
            TicketType.create("enterprise_round", Comparator.comparingLong(ChunkPos::toLong));

    /** The round in progress: loading its footprint, then working once its villagers are in. */
    private static final class Visit {
        final ServerLevel level;
        final GlobalPos desk;
        final Set<ChunkPos> chunks;
        final long startedNanos = System.nanoTime();
        long workUntil = -1; // set once everything has loaded

        Visit(ServerLevel level, GlobalPos desk, Set<ChunkPos> chunks) {
            this.level = level;
            this.desk = desk;
            this.chunks = chunks;
        }
    }

    private static Visit visit;

    private Rounds() {}

    public static boolean hasHeadroom(MinecraftServer server) {
        return server.getAverageTickTimeNanos() <= TICK_BUDGET_NANOS;
    }

    public static void onServerTick(ServerTickEvent.Post event) {
        MinecraftServer server = event.getServer();
        long now = server.overworld().getGameTime();
        if (visit != null) {
            continueVisit(visit, now);
            return;
        }
        if (now % CHECK_EVERY_TICKS != 0 || !hasHeadroom(server)) {
            return;
        }
        MerchantBook book = MerchantBook.get(server);
        Set<GlobalPos> desks = new LinkedHashSet<>(book.desks());
        desks.addAll(dev.b28.enterprise.territory.Territory.get(server).farmDesks()); // farms run without trades
        desks.stream()
                .filter(desk -> {
                    ServerLevel level = server.getLevel(desk.dimension());
                    return level != null && !level.isPositionEntityTicking(desk.pos());
                })
                .filter(desk -> now - book.lastRound(desk) >= INTERVAL_TICKS)
                .min(Comparator.comparingLong(book::lastRound))
                .ifPresent(desk -> visit(server, book, desk, now));
    }

    private static void visit(MinecraftServer server, MerchantBook book, GlobalPos desk, long now) {
        ServerLevel level = server.getLevel(desk.dimension());
        Set<ChunkPos> chunks = footprint(server, desk, book.workstations(desk));
        chunks.forEach(chunk -> level.getChunkSource().addRegionTicket(ROUND, chunk, 0, chunk));
        book.setLastRound(desk, now);
        visit = new Visit(level, desk, chunks);
        Enterprise.LOGGER.debug("Round: visiting the Merchant's Desk at {} ({} chunks)",
                desk.pos().toShortString(), chunks.size());
    }

    /**
     * Once the footprint and its villagers have loaded: catch-up for villagers there (including any that were loaded
     * but idle) and a census of the settlement; then hold for {@link #WORK_TICKS} and release the chunks.
     */
    private static void continueVisit(Visit current, long now) {
        if (current.workUntil < 0) {
            boolean loaded = current.chunks.stream().allMatch(chunk ->
                    current.level.getChunkSource().hasChunk(chunk.x, chunk.z)
                            && current.level.areEntitiesLoaded(chunk.toLong()));
            if (loaded) {
                LockerCatchUp.enqueueNear(current.level, current.desk.pos(), Lockers.MERCHANT_RANGE);
                dev.b28.enterprise.territory.Territory.get(current.level.getServer()).bellOf(current.desk)
                        .ifPresent(bell -> dev.b28.enterprise.settlement.CensusTaker.request(current.level, bell.pos()));
                current.workUntil = now + WORK_TICKS;
            } else if (System.nanoTime() - current.startedNanos > LOAD_TIMEOUT_NANOS) {
                Enterprise.LOGGER.debug("Round: gave up loading {}", current.desk.pos().toShortString());
                release(current);
            }
        } else if (now >= current.workUntil) {
            release(current);
        }
    }

    private static void release(Visit current) {
        current.chunks.forEach(chunk -> current.level.getChunkSource().removeRegionTicket(ROUND, chunk, 0, chunk));
        visit = null;
    }

    /**
     * What a round loads, most important first, at most {@link #MAX_CHUNKS}: the market around the desk and the
     * settlement around its bell (where the Merchant and villagers spend their time), the locker workstations, and
     * the chunks of the settlement's known fields, so its census reads them fresh.
     */
    static Set<ChunkPos> footprint(MinecraftServer server, GlobalPos desk, Collection<BlockPos> workstations) {
        Set<ChunkPos> chunks = new LinkedHashSet<>();
        addAround(chunks, desk.pos(), MARKET_REACH);
        var territory = dev.b28.enterprise.territory.Territory.get(server);
        var bell = territory.bellOf(desk);
        bell.ifPresent(b -> addAround(chunks, b.pos(), MARKET_REACH));
        workstations.stream()
                .sorted(Comparator.comparingDouble(pos -> pos.distSqr(desk.pos())))
                .forEach(pos -> addAround(chunks, pos, 1));
        bell.ifPresent(b -> territory.fieldChunks(b).forEach(chunk -> chunks.add(new ChunkPos(chunk))));
        return chunks.stream().limit(MAX_CHUNKS).collect(LinkedHashSet::new, Set::add, Set::addAll);
    }

    /** Blocks around the desk and bell a round loads: a market and its pen. */
    private static final int MARKET_REACH = 16;

    private static void addAround(Set<ChunkPos> chunks, BlockPos pos, int reach) {
        for (int dx = -reach; dx <= reach; dx += Math.max(1, reach)) {
            for (int dz = -reach; dz <= reach; dz += Math.max(1, reach)) {
                chunks.add(new ChunkPos(pos.offset(dx, 0, dz)));
            }
        }
    }
}
