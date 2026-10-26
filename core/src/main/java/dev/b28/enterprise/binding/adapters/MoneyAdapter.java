package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.money.MoneyChanger;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.Output;
import dev.b28.enterprise.wire.PointKind;
import dev.b28.enterprise.wire.Query;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Items;
import net.neoforged.neoforge.server.ServerLifecycleHooks;

/**
 * A town's money in its desk's chests and Blaze Banker, from the core (docs/binding-plan.md 1.5 and 2, concern 4). Money
 * stays physical: emeralds and Numismatics coins in the chests, a balance in the Banker's account. A town is one number
 * of emerald-worth in the core (its treasury); this adapter is how the world holds it.
 *
 * <p>At each look the chests and Banker of every desk that is counted for stock are counted too ({@link #withCash}, a
 * {@code Cashcount} sent right after the desk's {@code Stocktake}): whole emeralds, spurs of coin loose in the chests,
 * the Banker's balance. The core books what a player or a hopper did since as money paid in or taken out, and then shows
 * its treasury there ({@code Output.Cash}, {@link #show}): the emeralds the town has beyond its reserve sold to the mint
 * for coin (into the Banker's account if one touches the desk), its earnings put in as whole emeralds, what it spent
 * taken out, emeralds first, then coin, then the Banker, an emerald changed at the mint if those run short. What couldn't
 * be done (no room, a hopper first, an unloaded chunk) goes back as {@code CashUnmoved} before the next count.
 *
 * <p><b>Whose money.</b> In core mode the core's moves are written. In Java mode the Java rules own the chests and the
 * Banker ({@code money/Treasury}, {@code MoneyChanger}), and the core beside them only counts: the count says it
 * doesn't write, so the core sells nothing to the mint and moves nothing there.
 *
 * <p>No rule is here: the mint's rate, the reserve, what a spur is worth, the order money comes out of the till in, all
 * are the core's. What is here is Minecraft: which items are emeralds and coins, how coin is minted into the fewest
 * stacks ({@link StockChests#mint}), a Blaze Banker's account ({@link MoneyChanger#bankAt}, kept as the view onto it).
 */
public final class MoneyAdapter {
    private final CoreHost host;
    /** Whether the core's moves are written to the chests and the Banker (core mode, a test's host), or all sent back unmoved. */
    private final boolean writes;
    /** What the host couldn't do of the core's moves, for the next look. */
    private final List<Input> pending = new ArrayList<>();
    /**
     * Each desk's till as its last count left it and the core's moves since made it (emeralds, spurs of coin): a count
     * that finds otherwise is money moved by something outside the core, logged with what stands and moves by the desk.
     */
    private final Map<GlobalPos, long[]> expected = new HashMap<>();

    public MoneyAdapter(CoreHost host, boolean writes) {
        this.host = host;
        this.writes = writes;
    }

    /** Whether the core's moves are written to the chests and Banker here. */
    public boolean writes() {
        return writes;
    }

    /**
     * The inputs with each desk's money counted right after its stocktake (the {@code Cashcount} after the
     * {@code Stocktake}: the core shows the stock and the money at the end of the look, counted together). A desk that
     * isn't in a loaded level gets none.
     */
    public List<Input> withCash(List<Input> inputs) {
        MinecraftServer server = ServerLifecycleHooks.getCurrentServer();
        List<Input> out = new ArrayList<>(inputs.size() * 2);
        for (Input input : inputs) {
            out.add(input);
            if (server != null && input instanceof Input.Stocktake st && st.point().kind() == PointKind.Desk) {
                GlobalPos at = PointsAdapter.place(st.point());
                ServerLevel level = server.getLevel(at.dimension());
                if (level != null) {
                    out.add(cashcount(level, at));
                }
            }
        }
        return out;
    }

    /** The desk's chests and Banker counted: emeralds (loose and in shulker boxes), coin loose, the Banker's balance. */
    public Input.Cashcount cashcount(ServerLevel level, GlobalPos at) {
        StockChests chests = StockChests.at(level, PointsAdapter.Kind.DESK, at.pos());
        long emeralds = chests.held().getOrDefault(Items.EMERALD, 0L);
        Optional<Long> bank = MoneyChanger.bankAt(level, at.pos()).map(account -> (long) account.getBalance());
        long[] was = expected.put(at, new long[] {emeralds, chests.coinSpurs()});
        if (was != null && (was[0] != emeralds || was[1] != chests.coinSpurs())) {
            Enterprise.LOGGER.info("Till at {} moved outside the core: {} emeralds and {} spurs counted, {} and {} expected; by it: {}",
                    at.pos().toShortString(), emeralds, chests.coinSpurs(), was[0], was[1], around(level, at.pos()));
        }
        return new Input.Cashcount(PointsAdapter.point(PointsAdapter.Kind.DESK, at), emeralds, chests.coinSpurs(), bank,
                chests.coinRoom(), writes);
    }

    /** What the host couldn't do of the core's moves since the last look, for the next one (taken: sent once). */
    public List<Input> pending() {
        List<Input> out = List.copyOf(pending);
        pending.clear();
        return out;
    }

    /**
     * The core's money for a desk: emeralds put in its chests or taken out (as far as they go), coin minted in or out of
     * them as the fewest coins, spurs paid into or taken out of the Banker's account. What couldn't be done (no room, the
     * chunk unloaded since the count, Java mode) goes back as {@code CashUnmoved}. A sale whose coin doesn't fit leaves
     * its emeralds where they are.
     */
    public void show(Output.Cash cash) {
        GlobalPos at = PointsAdapter.place(cash.point());
        if (host.points().at(at) == null) {
            return; // the desk is gone and the core forgot it too
        }
        long emeralds = cash.emeralds();
        long coins = cash.coins();
        long bank = cash.bank();
        long leftEmeralds = emeralds;
        long leftCoins = coins;
        long leftBank = bank;
        MinecraftServer server = ServerLifecycleHooks.getCurrentServer();
        ServerLevel level = server == null ? null : server.getLevel(at.dimension());
        if (writes && level != null && loaded(level, at.pos())) {
            StockChests chests = StockChests.at(level, PointsAdapter.Kind.DESK, at.pos());
            // Coin and the account first, then the emeralds: a sale whose coin has no room keeps its emeralds.
            leftCoins = coins == 0 ? 0 : chests.mint(coins);
            leftBank = bank == 0 ? 0 : payBank(level, at.pos(), bank);
            if (emeralds < 0 && ((coins > 0 && leftCoins != 0) || (bank > 0 && leftBank != 0))) {
                leftEmeralds = emeralds; // the sale is undone: the core is told all of it
            } else if (emeralds > 0) {
                leftEmeralds = chests.put(Items.EMERALD, emeralds);
            } else if (emeralds < 0) {
                leftEmeralds = -chests.take(Items.EMERALD, -emeralds);
            }
        }
        long[] till = expected.get(at);
        if (till != null) {
            till[0] += emeralds - leftEmeralds;
            till[1] += coins - leftCoins;
        }
        if (leftEmeralds != 0 || leftCoins != 0 || leftBank != 0) {
            Enterprise.LOGGER.debug("Cash not moved at {}: {} emeralds, {} spurs of coin, {} in the bank", at.pos().toShortString(),
                    leftEmeralds, leftCoins, leftBank);
            pending.add(new Input.CashUnmoved(cash.point(), leftEmeralds, leftCoins, leftBank));
        }
    }

    /** What touches the desk and what moves within 8 blocks of it, for the log of a till moved outside the core. */
    private static String around(ServerLevel level, BlockPos desk) {
        StringBuilder out = new StringBuilder();
        for (net.minecraft.core.Direction dir : net.minecraft.core.Direction.values()) {
            BlockPos at = desk.relative(dir);
            out.append(dir.getName()).append('=')
                    .append(net.minecraft.core.registries.BuiltInRegistries.BLOCK.getKey(level.getBlockState(at).getBlock()).getPath())
                    .append(' ');
        }
        level.getEntities((net.minecraft.world.entity.Entity) null, new net.minecraft.world.phys.AABB(desk).inflate(8))
                .forEach(e -> out.append(net.minecraft.world.entity.EntityType.getKey(e.getType()).getPath()).append('@')
                        .append(e.blockPosition().toShortString()).append(' '));
        return out.toString().trim();
    }

    /** Spurs paid into the Banker's account (above nothing) or drawn from it; what wasn't done is returned as {@code CashUnmoved} says it. */
    private static long payBank(ServerLevel level, BlockPos desk, long spurs) {
        var account = MoneyChanger.bankAt(level, desk);
        if (account.isEmpty()) {
            return spurs; // no Banker touches the desk any more
        }
        long now = account.get().getBalance() + spurs;
        long set = Math.max(0, Math.min(Integer.MAX_VALUE, now));
        account.get().setBalance((int) set);
        return now < 0 ? now : now > Integer.MAX_VALUE ? now - Integer.MAX_VALUE : 0;
    }

    /** The block at {@code pos} and the six touching it are in loaded chunks (a look never loads a chunk to count a chest). */
    private static boolean loaded(ServerLevel level, BlockPos pos) {
        if (!level.isLoaded(pos)) {
            return false;
        }
        for (net.minecraft.core.Direction dir : net.minecraft.core.Direction.values()) {
            if (!level.isLoaded(pos.relative(dir))) {
                return false;
            }
        }
        return true;
    }

    /** A desk's till as the core takes it to be, with its town's treasury (all in emeralds, worth in thousandths of one). */
    public record Cash(long town, double treasury, long emeralds, long coins, double coinWorth, Long bank, double bankWorth, long reserve) {}

    /** The till at {@code desk} as the core takes it to be, or null if the core keeps no desk there. */
    public Cash cashAt(GlobalPos desk) {
        PointsAdapter.Placed p = host.points().at(desk);
        if (p == null || p.kind() != PointsAdapter.Kind.DESK) {
            return null;
        }
        if (host.ask(List.of(new Query.Cash(PointsAdapter.point(PointsAdapter.Kind.DESK, desk)))).get(0) instanceof Answer.Cash c) {
            return new Cash(c.town(), c.treasury() / 1000.0, c.emeralds(), c.coins(), c.coinWorth() / 1000.0,
                    c.bank().orElse(null), c.bankWorth() / 1000.0, c.reserve());
        }
        throw new IllegalStateException("the core answered Cash with something else");
    }

    /** The mint as the core has it: spurs an emerald fetches now, the selling pressure, the coin minted without backing, the debasement. */
    public Answer.Mint mint() {
        if (host.ask(List.of(new Query.Mint())).get(0) instanceof Answer.Mint m) {
            return m;
        }
        throw new IllegalStateException("the core answered Mint with something else");
    }
}
