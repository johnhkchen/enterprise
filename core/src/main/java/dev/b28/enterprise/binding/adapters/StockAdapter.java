package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.settlement.Names;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.Count;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.Point;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Response;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.neoforged.neoforge.event.level.BlockEvent;
import net.neoforged.neoforge.items.IItemHandler;

/**
 * A desk's stock, from the core (docs/binding-plan.md 2, concern 3, its first slice): a Merchant's desk is a town in
 * the core and its trade point ({@code AddTown}); at each look the chests touching every loaded desk are counted into
 * a {@code Stocktake}, and the core books what changed since the last as goods delivered or taken; what the town holds
 * is the core's to say ({@code Query.Stock}). Chests stay physical: players, hoppers and Create move what's in them,
 * and the core counts it.
 *
 * <p>The adapter keeps no society ids of its own: the desks and their towns are the core's points ({@link
 * PointsAdapter}), and the goods' numbers the core's content, read back from the core when the host starts ({@link
 * #reload}). An item that isn't one of the core's goods isn't counted.
 *
 * <p>Not yet (B4.3 {@code stock-adapter}): the shulkers kept in a desk's chests, its depots, a desk broken or moved
 * without the host seeing it, and the core's own movements ({@code Moved}) written back to the chests.
 */
public final class StockAdapter {
    private final CoreHost host;
    /** The core's goods by number, and the number of each item that's one. */
    private List<String> goods = List.of();
    private Map<Item, Integer> numbers = Map.of();

    public StockAdapter(CoreHost host) {
        this.host = host;
    }

    /** Reads the goods from the core again: what its numbers mean. */
    public void reload() {
        List<Answer> answers = host.ask(List.of(new Query.Goods()));
        if (answers.get(0) instanceof Answer.Goods g) {
            goods = g.goods();
            Map<Item, Integer> byItem = new HashMap<>();
            for (int i = 0; i < goods.size(); i++) {
                ResourceLocation id = ResourceLocation.tryParse(goods.get(i));
                Item item = id == null ? Items.AIR : BuiltInRegistries.ITEM.get(id);
                if (item != Items.AIR) {
                    byItem.put(item, i);
                }
            }
            numbers = Map.copyOf(byItem);
        }
    }

    /** The town at {@code desk}, if the core keeps a desk there. */
    public Long townAt(GlobalPos desk) {
        PointsAdapter.Placed p = host.points().at(desk);
        return p != null && p.kind() == PointsAdapter.Kind.DESK ? p.town() : null;
    }

    /** Every desk the core keeps, with its town. */
    public Map<GlobalPos, Long> desks() {
        return host.points().of(PointsAdapter.Kind.DESK);
    }

    /**
     * The desk at {@code desk} registered with the core as a town called {@code name}, and its chests counted now (its
     * first stocktake, from nothing). A desk the core keeps already is left as it is.
     *
     * @return the desk's town
     * @throws IllegalStateException when the core refuses it
     */
    public long found(ServerLevel level, BlockPos desk, String name) {
        GlobalPos at = GlobalPos.of(level.dimension(), desk);
        Long known = townAt(at);
        if (known != null) {
            return known;
        }
        // Founded at this tick (the points adapter brings the core to now), not at the last look's, nor at tick 0 of a
        // society new to an old world; its first stocktake booked then too.
        long town = host.points().found(level, desk, name);
        Response r = host.apply(List.of(new Input.Stocktake(point(at), count(level, desk))));
        if (r instanceof Response.Refused refused) {
            throw new IllegalStateException("the core wouldn't count the desk at " + desk.toShortString() + ": " + refused.why());
        }
        return town;
    }

    /** The desk at {@code desk} broken: no longer the core's point (its town keeps what it held). */
    public void forget(ServerLevel level, BlockPos desk) {
        GlobalPos at = GlobalPos.of(level.dimension(), desk);
        if (townAt(at) != null) {
            host.points().remove(at);
        }
    }

    /**
     * A Merchant's desk placed by a player (or anything that places as one): a town in the core, its chests counted.
     * Heard last ({@code EventPriority.LOWEST}, cancelled events not heard), so a placement another listener cancels
     * founds nothing. The core's failures stay here: logged, never thrown into the placement.
     */
    public static void onPlaced(BlockEvent.EntityPlaceEvent event) {
        if (event.getLevel() instanceof ServerLevel level && event.getPlacedBlock().is(Merchant.DESK.get())) {
            CoreHost host = CoreHost.of(level.getServer());
            if (host != null && host.pausedWhy() == null) {
                try {
                    GlobalPos at = GlobalPos.of(level.dimension(), event.getPos());
                    host.stock().found(level, event.getPos(), Names.of(level.getServer(), at));
                } catch (RuntimeException e) {
                    Enterprise.LOGGER.error("The core didn't take the desk at {}; the world goes on without it",
                            event.getPos().toShortString(), e);
                }
            }
        }
    }

    /**
     * A Merchant's desk broken: its point gone from the core, its town and what it held kept. Heard last, as
     * {@link #onPlaced}, so a break another listener cancels keeps the point; failures are logged, not thrown.
     */
    public static void onBroken(BlockEvent.BreakEvent event) {
        if (event.getLevel() instanceof ServerLevel level && event.getState().is(Merchant.DESK.get())) {
            CoreHost host = CoreHost.of(level.getServer());
            if (host != null && host.pausedWhy() == null) {
                try {
                    host.stock().forget(level, event.getPos());
                } catch (RuntimeException e) {
                    Enterprise.LOGGER.error("The core didn't forget the desk at {}", event.getPos().toShortString(), e);
                }
            }
        }
    }

    /**
     * A look's happenings: a stocktake at every desk the core keeps that is still a desk, with it and every block
     * touching it loaded (a look never loads a chunk to count a chest).
     */
    public List<Input> happenings(MinecraftServer server) {
        List<Input> inputs = new ArrayList<>();
        for (GlobalPos desk : desks().keySet()) {
            ServerLevel level = server.getLevel(desk.dimension());
            if (level != null && loaded(level, desk.pos()) && level.getBlockState(desk.pos()).is(Merchant.DESK.get())) {
                inputs.add(new Input.Stocktake(point(desk), count(level, desk.pos())));
            }
        }
        return inputs;
    }

    /** The block at {@code pos} and the six touching it are in loaded chunks. */
    private static boolean loaded(ServerLevel level, BlockPos pos) {
        if (!level.isLoaded(pos)) {
            return false;
        }
        for (Direction dir : Direction.values()) {
            if (!level.isLoaded(pos.relative(dir))) {
                return false;
            }
        }
        return true;
    }

    /** What the chests touching {@code desk} hold now of each of the core's goods (others aren't counted). */
    public List<Count> count(ServerLevel level, BlockPos desk) {
        IItemHandler chests = Lockers.inventory(level, Lockers.touchingContainers(level, desk));
        Map<Integer, Long> held = new TreeMap<>();
        for (int slot = 0; slot < chests.getSlots(); slot++) {
            ItemStack stack = chests.getStackInSlot(slot);
            Integer good = stack.isEmpty() ? null : numbers.get(stack.getItem());
            if (good != null) {
                held.merge(good, (long) stack.getCount(), Long::sum);
            }
        }
        List<Count> counts = new ArrayList<>(held.size());
        held.forEach((good, n) -> counts.add(new Count(good, n.doubleValue())));
        return counts;
    }

    /**
     * What the core says the town at {@code desk} holds, by good (those it holds any of, in the goods' order), or null
     * if the core keeps no town there.
     */
    public Map<String, Double> stockAt(GlobalPos desk) {
        Long town = townAt(desk);
        if (town == null) {
            return null;
        }
        Answer a = host.ask(List.of(new Query.Stock(town))).get(0);
        Map<String, Double> held = new LinkedHashMap<>();
        if (a instanceof Answer.Stock s) {
            for (int good = 0; good < s.stock().size() && good < goods.size(); good++) {
                double n = s.stock().get(good);
                if (n != 0) {
                    held.put(goods.get(good), n);
                }
            }
        }
        return held;
    }

    /** A desk's place as the core keeps it: the dimension's id and the block. */
    public static Point point(GlobalPos desk) {
        return PointsAdapter.point(PointsAdapter.Kind.DESK, desk);
    }
}
