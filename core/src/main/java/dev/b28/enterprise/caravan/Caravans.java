package dev.b28.enterprise.caravan;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.locker.Rounds;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.MoneyChanger;
import dev.ithundxr.createnumismatics.content.coins.CoinItem;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.Iterator;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.TicketType;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.ChunkPos;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * Runs caravans on the ledger. Once a day per route, the hub Merchant loads what the destination's switched-on
 * trades use and sends a caravan; it delivers, collects the coins in the destination Merchant's chest, and brings
 * them home. Caravans eat on the road: each leg's food comes from the settlement it leaves, and a caravan carries
 * only so much, which caps how long a leg can be.
 */
public final class Caravans {
    /** Travel speed: 1,200 blocks per in-game day. */
    public static final int TICKS_PER_BLOCK = 20;

    /** A caravan eats one food point (nutrition) per this many blocks. */
    public static final int BLOCKS_PER_FOOD = 10;

    /** The most food a caravan carries: a stack of bread. Longest leg: 3,200 blocks. */
    public static final int FOOD_CAPACITY = 320;

    /** Stacks of goods a caravan carries. */
    public static final int CARGO_SLOTS = 27;

    /** A caravan tops a destination up to this many of each item its trades use: ten stacks, a few days' trading. */
    public static final int WANT_PER_ITEM = 640;

    /** At most one departure per route per in-game day. */
    public static final long DEPARTURE_INTERVAL = 24000;

    private static final long POLL_TICKS = 100;
    private static final int TICKET_TICKS = 200;
    private static final TicketType<ChunkPos> CARAVAN =
            TicketType.create("enterprise_caravan", Comparator.comparingLong(ChunkPos::toLong), TICKET_TICKS);

    private Caravans() {}

    public static int foodNeeded(int distance) {
        return Math.max(1, (distance + BLOCKS_PER_FOOD - 1) / BLOCKS_PER_FOOD);
    }

    public static boolean inRange(int distance) {
        return foodNeeded(distance) <= FOOD_CAPACITY;
    }

    public static long travelTicks(int distance) {
        return Math.max(20, (long) distance * TICKS_PER_BLOCK);
    }

    public static void onServerTick(ServerTickEvent.Post event) {
        MinecraftServer server = event.getServer();
        long now = server.overworld().getGameTime();
        if (now % POLL_TICKS == 0 && Rounds.hasHeadroom(server)) {
            poll(server, CaravanLedger.get(server), now);
        }
    }

    /** One pass over every route: dispatch, arrive, return. */
    public static void poll(MinecraftServer server, CaravanLedger ledger, long now) {
        for (Route route : List.copyOf(ledger.routes())) {
            var caravan = ledger.caravan(route.id);
            if (caravan.isEmpty()) {
                if (now - route.lastDeparture >= DEPARTURE_INTERVAL) {
                    dispatch(server, ledger, route, now);
                }
            } else if (now >= caravan.get().arrives) {
                arrive(server, ledger, route, caravan.get(), now);
            }
        }
    }

    /** The hub sends a caravan if its Merchant is there and the leg's food is in its stock. Returns true if sent. */
    public static boolean dispatch(MinecraftServer server, CaravanLedger ledger, Route route, long now) {
        ServerLevel level = server.getLevel(route.from.dimension());
        if (level == null || !ensureLoaded(level, route.from.pos())) {
            return false;
        }
        if (Merchant.workingAt(level, route.from.pos(), Lockers.MERCHANT_RANGE).isEmpty()) {
            route.status = "no Merchant at the hub";
            ledger.setDirty();
            return false;
        }
        MerchantStock stock = MerchantStock.at(level, route.from.pos());
        int food = foodNeeded(route.distance());
        if (stock.eat(food) == 0) {
            route.status = "waiting for food at the hub";
            ledger.setDirty();
            return false;
        }
        List<ItemStack> cargo = pickCargo(stock, wantedAt(server, route.to));
        ledger.put(new Caravan(route.id, Caravan.Leg.OUTBOUND, now + travelTicks(route.distance()), cargo, 0));
        route.lastDeparture = now;
        route.foodEaten += food;
        route.status = "on the way out";
        ledger.setDirty();
        Enterprise.LOGGER.debug("Caravan {} leaves {} with {} items", route.id, route.from.pos().toShortString(),
                cargo.stream().mapToInt(ItemStack::getCount).sum());
        return true;
    }

    /** Arrival at either end; waits a poll if that end isn't loaded yet. */
    public static void arrive(MinecraftServer server, CaravanLedger ledger, Route route, Caravan caravan, long now) {
        if (caravan.leg == Caravan.Leg.RETURNING) {
            arriveHome(server, ledger, route, caravan);
        } else {
            arriveAtDestination(server, ledger, route, caravan, now);
        }
    }

    private static void arriveAtDestination(
            MinecraftServer server, CaravanLedger ledger, Route route, Caravan caravan, long now) {
        ServerLevel level = server.getLevel(route.to.dimension());
        if (level == null || !ensureLoaded(level, route.to.pos())) {
            return;
        }
        MerchantStock stock = MerchantStock.at(level, route.to.pos());
        if (caravan.leg == Caravan.Leg.OUTBOUND) {
            route.itemsDelivered += deliver(caravan, stock);
            List<ItemStack> coins = stock.removeLoose(stack -> stack.getItem() instanceof CoinItem);
            caravan.spurs += MoneyChanger.value(coins);
        }
        int food = foodNeeded(route.distance());
        if (stock.eat(food) == 0) {
            caravan.leg = Caravan.Leg.WAITING_FOR_FOOD;
            caravan.arrives = now; // try again next poll
            route.status = "waiting for food at the destination";
        } else {
            route.foodEaten += food;
            caravan.leg = Caravan.Leg.RETURNING;
            caravan.arrives = now + travelTicks(route.distance());
            route.status = "on the way home";
        }
        ledger.setDirty();
    }

    private static void arriveHome(MinecraftServer server, CaravanLedger ledger, Route route, Caravan caravan) {
        ServerLevel level = server.getLevel(route.from.dimension());
        if (level == null || !ensureLoaded(level, route.from.pos())) {
            return;
        }
        MerchantStock stock = MerchantStock.at(level, route.from.pos());
        for (Iterator<ItemStack> it = caravan.cargo.iterator(); it.hasNext(); ) {
            ItemStack rest = stock.insert(it.next());
            if (rest.isEmpty()) {
                it.remove();
            }
        }
        var bank = MoneyChanger.bankAt(level, route.from.pos());
        if (bank.isPresent()) {
            bank.get().deposit(caravan.spurs);
        } else {
            MoneyChanger.coins(caravan.spurs).forEach(stock::insert);
        }
        route.spursCollected += caravan.spurs;
        route.trips++;
        route.status = "home";
        ledger.remove(route.id);
        ledger.setDirty();
        Enterprise.LOGGER.debug("Caravan {} home with {} spurs", route.id, caravan.spurs);
    }

    /** Items the destination's switched-on trades take, read from the trade keys ("costA+costB>result"). */
    static Set<Item> wantedAt(MinecraftServer server, GlobalPos desk) {
        MerchantBook book = MerchantBook.get(server);
        Set<Item> items = new LinkedHashSet<>();
        for (BlockPos workstation : book.workstations(desk)) {
            for (String key : book.enabledTrades(desk, workstation)) {
                String costs = key.substring(0, key.indexOf('>'));
                for (String id : costs.split("\\+")) {
                    if (!id.isEmpty() && !id.equals("minecraft:emerald")) {
                        Item item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(id));
                        if (item != Items.AIR) {
                            items.add(item);
                        }
                    }
                }
            }
        }
        return items;
    }

    /** Up to {@link #CARGO_SLOTS} stacks, shared across the wanted items, each up to {@link #WANT_PER_ITEM}. */
    static List<ItemStack> pickCargo(MerchantStock stock, Set<Item> wanted) {
        List<ItemStack> cargo = new ArrayList<>();
        if (wanted.isEmpty()) {
            return cargo;
        }
        int perItem = Math.min(WANT_PER_ITEM, CARGO_SLOTS / wanted.size() * 64);
        for (Item item : wanted) {
            for (ItemStack piece : stock.take(new ItemCost(item, 1), perItem)) {
                cargo.add(piece);
            }
        }
        return cargo;
    }

    /** Delivers cargo up to what the destination wants of each item; the rest rides home. Returns items delivered. */
    private static int deliver(Caravan caravan, MerchantStock stock) {
        int delivered = 0;
        for (Iterator<ItemStack> it = caravan.cargo.iterator(); it.hasNext(); ) {
            ItemStack stack = it.next();
            int room = WANT_PER_ITEM - stock.count(new ItemCost(stack.getItem(), 1));
            if (room <= 0) {
                continue;
            }
            ItemStack drop = stack.split(Math.min(room, stack.getCount()));
            ItemStack rest = stock.insert(drop);
            delivered += drop.getCount() - rest.getCount();
            stack.grow(rest.getCount());
            if (stack.isEmpty()) {
                it.remove();
            }
        }
        return delivered;
    }

    /** True if the desk and the chests touching it are loaded; otherwise asks for them to load and returns false. */
    private static boolean ensureLoaded(ServerLevel level, BlockPos desk) {
        boolean loaded = true;
        for (int dx = -1; dx <= 1; dx++) {
            for (int dz = -1; dz <= 1; dz++) {
                BlockPos around = desk.offset(dx, 0, dz);
                if (!level.isLoaded(around)) {
                    ChunkPos chunk = new ChunkPos(around);
                    level.getChunkSource().addRegionTicket(CARAVAN, chunk, 0, chunk);
                    loaded = false;
                }
            }
        }
        return loaded;
    }
}
