package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.settlement.Names;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.Count;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.Point;
import dev.b28.enterprise.wire.PointKind;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.TownPoint;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.Level;
import net.neoforged.neoforge.event.level.BlockEvent;
import net.neoforged.neoforge.items.IItemHandler;

/**
 * A desk's stock, from the core (docs/binding-plan.md 2, concern 3, its first slice): a Merchant's desk is a town in
 * the core and its trade point ({@code AddTown}); at each look the chests touching every loaded desk are counted into
 * a {@code Stocktake}, and the core books what changed since the last as goods delivered or taken; what the town holds
 * is the core's to say ({@code Query.Stock}). Chests stay physical: players, hoppers and Create move what's in them,
 * and the core counts it.
 *
 * <p>The adapter keeps no society ids of its own: the desks and their towns are the core's points, and the goods'
 * numbers the core's content, both read back from the core when the host starts ({@link #reload}) and when a desk is
 * registered. An item that isn't one of the core's goods isn't counted.
 *
 * <p>Not yet (B4.3 {@code stock-adapter}): the shulkers kept in a desk's chests, its depots, a desk broken or moved
 * without the host seeing it, and the core's own movements ({@code Moved}) written back to the chests.
 */
public final class StockAdapter {
    private final CoreHost host;
    /** The core's goods by number, and the number of each item that's one. */
    private List<String> goods = List.of();
    private Map<Item, Integer> numbers = Map.of();
    /** The desks the core keeps, by place, with their towns. */
    private Map<GlobalPos, Long> desks = Map.of();

    public StockAdapter(CoreHost host) {
        this.host = host;
    }

    /** Reads the goods and the desks from the core again: what its numbers mean, and which desks are towns. */
    public void reload() {
        List<Answer> answers = host.ask(List.of(new Query.Goods(), new Query.Points()));
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
        if (answers.get(1) instanceof Answer.Points p) {
            Map<GlobalPos, Long> byPlace = new LinkedHashMap<>();
            for (TownPoint tp : p.points()) {
                if (tp.point().kind() == PointKind.Desk) {
                    byPlace.put(place(tp.point()), tp.town());
                }
            }
            desks = byPlace;
        }
    }

    /** The town at {@code desk}, if the core keeps one there. */
    public Long townAt(GlobalPos desk) {
        return desks.get(desk);
    }

    /** Every desk the core keeps, with its town. */
    public Map<GlobalPos, Long> desks() {
        return desks;
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
        Long known = desks.get(at);
        if (known != null) {
            return known;
        }
        Point point = point(at);
        Response r = host.apply(List.of(new Input.AddTown(name, point), new Input.Stocktake(point, count(level, desk))));
        if (r instanceof Response.Refused refused) {
            throw new IllegalStateException("the core wouldn't take a town at " + desk.toShortString() + ": " + refused.why());
        }
        reload();
        Long town = desks.get(at);
        if (town == null) {
            throw new IllegalStateException("the core took a town at " + desk.toShortString() + " but doesn't list it");
        }
        return town;
    }

    /** The desk at {@code desk} broken: no longer the core's point (its town keeps what it held). */
    public void forget(ServerLevel level, BlockPos desk) {
        GlobalPos at = GlobalPos.of(level.dimension(), desk);
        if (desks.containsKey(at)) {
            host.apply(List.of(new Input.RemovePoint(point(at))));
            reload();
        }
    }

    /** A Merchant's desk placed by a player (or anything that places as one): a town in the core, its chests counted. */
    public static void onPlaced(BlockEvent.EntityPlaceEvent event) {
        if (event.getLevel() instanceof ServerLevel level && event.getPlacedBlock().is(Merchant.DESK.get())) {
            CoreHost host = CoreHost.of(level.getServer());
            if (host != null && host.pausedWhy() == null) {
                GlobalPos at = GlobalPos.of(level.dimension(), event.getPos());
                host.stock().found(level, event.getPos(), Names.of(level.getServer(), at));
            }
        }
    }

    /** A Merchant's desk broken: its point gone from the core, its town and what it held kept. */
    public static void onBroken(BlockEvent.BreakEvent event) {
        if (event.getLevel() instanceof ServerLevel level && event.getState().is(Merchant.DESK.get())) {
            CoreHost host = CoreHost.of(level.getServer());
            if (host != null && host.pausedWhy() == null) {
                host.stock().forget(level, event.getPos());
            }
        }
    }

    /** A look's happenings: a stocktake at every desk the core keeps whose block is loaded and still a desk. */
    public List<Input> happenings(MinecraftServer server) {
        List<Input> inputs = new ArrayList<>();
        for (GlobalPos desk : desks.keySet()) {
            ServerLevel level = server.getLevel(desk.dimension());
            if (level != null && level.isLoaded(desk.pos()) && level.getBlockState(desk.pos()).is(Merchant.DESK.get())) {
                inputs.add(new Input.Stocktake(point(desk), count(level, desk.pos())));
            }
        }
        return inputs;
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
        Long town = desks.get(desk);
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
        BlockPos p = desk.pos();
        return new Point(PointKind.Desk, desk.dimension().location().toString(), p.getX(), p.getY(), p.getZ());
    }

    private static GlobalPos place(Point point) {
        ResourceKey<Level> dim = ResourceKey.create(Registries.DIMENSION, ResourceLocation.parse(point.dim()));
        return GlobalPos.of(dim, new BlockPos(point.x(), point.y(), point.z()));
    }
}
