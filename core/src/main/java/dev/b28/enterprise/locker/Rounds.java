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

    /** How long a round keeps the footprint loaded; catch-up starts 2 seconds after the villagers load. */
    private static final int ROUND_TICKS = 300;

    /** No rounds or catch-up while the average tick takes longer than this. */
    private static final long TICK_BUDGET_NANOS = 30_000_000L;

    /** At most this many chunks per round. */
    private static final int MAX_CHUNKS = 25;

    private static final TicketType<ChunkPos> ROUND =
            TicketType.create("enterprise_round", Comparator.comparingLong(ChunkPos::toLong), ROUND_TICKS);

    private static long busyUntil;

    private Rounds() {}

    public static boolean hasHeadroom(MinecraftServer server) {
        return server.getAverageTickTimeNanos() <= TICK_BUDGET_NANOS;
    }

    public static void onServerTick(ServerTickEvent.Post event) {
        MinecraftServer server = event.getServer();
        long now = server.overworld().getGameTime();
        if (now % CHECK_EVERY_TICKS != 0 || now < busyUntil || !hasHeadroom(server)) {
            return;
        }
        MerchantBook book = MerchantBook.get(server);
        book.desks().stream()
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
        Set<ChunkPos> chunks = footprint(desk.pos(), book.workstations(desk));
        chunks.forEach(chunk -> level.getChunkSource().addRegionTicket(ROUND, chunk, 0, chunk));
        LockerCatchUp.enqueueNear(level, desk.pos(), Lockers.MERCHANT_RANGE); // villagers already loaded but idle
        book.setLastRound(desk, now);
        busyUntil = now + ROUND_TICKS;
        Enterprise.LOGGER.debug("Round: visiting the Merchant's Desk at {} ({} chunks)",
                desk.pos().toShortString(), chunks.size());
    }

    /** Chunks holding the desk, its locker workstations, and anything touching them, nearest the desk first. */
    public static Set<ChunkPos> footprint(BlockPos desk, Collection<BlockPos> workstations) {
        Set<ChunkPos> chunks = new LinkedHashSet<>();
        addAround(chunks, desk);
        workstations.stream()
                .sorted(Comparator.comparingDouble(pos -> pos.distSqr(desk)))
                .forEach(pos -> addAround(chunks, pos));
        return chunks.stream().limit(MAX_CHUNKS).collect(LinkedHashSet::new, Set::add, Set::addAll);
    }

    private static void addAround(Set<ChunkPos> chunks, BlockPos pos) {
        for (int dx = -1; dx <= 1; dx++) {
            for (int dz = -1; dz <= 1; dz++) {
                chunks.add(new ChunkPos(pos.offset(dx, 0, dz)));
            }
        }
    }
}
