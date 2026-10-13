package dev.b28.enterprise.caravan;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.territory.Territory;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.locker.Rounds;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.MoneyChanger;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.money.Treasury;
import dev.ithundxr.createnumismatics.content.coins.CoinItem;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.Iterator;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
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

    /** What a Merchant pays a caravan for its cargo: this share of the goods' local value. */
    public static final double WHOLESALE = 0.75;

    /**
     * Emeralds a caravan takes from the hub per food point of the return leg, to buy that food at the destination:
     * bread is 30 points to the emerald; half again for dearer food.
     */
    public static final double PURSE_PER_FOOD_POINT = 1.5 / 30;

    /** Share of the coins a caravan carries that each other settlement whose land it crosses takes as a toll. */
    public static final double TOLL_RATE = 0.05;

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
            poll(server, CaravanLedger.get(server), Territory.get(server), now);
        }
    }

    /** One pass over every route: dispatch, arrive, return. */
    public static void poll(MinecraftServer server, CaravanLedger ledger, Territory territory, long now) {
        for (Route route : List.copyOf(ledger.routes())) {
            var caravan = ledger.caravan(route.id);
            if (caravan.isEmpty()) {
                if (now - route.lastDeparture >= DEPARTURE_INTERVAL) {
                    dispatch(server, ledger, territory, route, now);
                }
            } else if (now >= caravan.get().arrives) {
                arrive(server, ledger, territory, route, caravan.get(), now);
            }
        }
    }

    /**
     * The hub sends a caravan if its Merchant is there, the outbound leg's food is in its stock, and it can fund a purse
     * for the return leg's food (bought at the destination). Returns true if sent.
     */
    public static boolean dispatch(
            MinecraftServer server, CaravanLedger ledger, Territory territory, Route route, long now) {
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
        territory.touch(level, route.from.pos(), stock);
        int food = foodNeeded(route.distance());
        Mint mint = Mint.get(server);
        double rate = mint.rate(Mint.DEFAULT_FACTION, now);
        double purse = food * PURSE_PER_FOOD_POINT;
        if (stock.foodNutrition() < food) {
            route.status = "waiting for food at the hub";
            ledger.setDirty();
            return false;
        }
        if (Treasury.available(level, route.from.pos(), stock, rate) < purse) {
            route.status = "the hub can't fund the trip";
            ledger.setDirty();
            return false;
        }
        stock.eat(food); // the hub's own food: already paid for
        Treasury.Withdrawal taken = Treasury.withdraw(level, route.from.pos(), stock, purse, mint, now);
        int spurs = taken.spurs() + mint.sell(Mint.DEFAULT_FACTION, taken.emeralds(), now);
        List<ItemStack> cargo = pickCargo(stock, wantedAt(server, route.to));
        ledger.put(new Caravan(route.id, Caravan.Leg.OUTBOUND, now + travelTicks(route.distance()), cargo, spurs));
        route.lastDeparture = now;
        route.foodEaten += food;
        route.status = "on the way out";
        ledger.setDirty();
        Enterprise.LOGGER.debug("Caravan {} leaves {} with {} items and a purse of {} spurs", route.id,
                route.from.pos().toShortString(), cargo.stream().mapToInt(ItemStack::getCount).sum(), spurs);
        return true;
    }

    /** Arrival at either end; waits a poll if that end isn't loaded yet. */
    public static void arrive(
            MinecraftServer server, CaravanLedger ledger, Territory territory, Route route, Caravan caravan, long now) {
        if (caravan.leg == Caravan.Leg.RETURNING) {
            arriveHome(server, ledger, territory, route, caravan);
        } else {
            arriveAtDestination(server, ledger, territory, route, caravan, now);
        }
    }

    /**
     * At the destination the caravan sells its cargo to the Merchant at wholesale, as far as it wants and can pay
     * for, then buys the return leg's food there at the local price. Nothing is taken for free.
     */
    private static void arriveAtDestination(MinecraftServer server, CaravanLedger ledger, Territory territory,
            Route route, Caravan caravan, long now) {
        ServerLevel level = server.getLevel(route.to.dimension());
        if (level == null || !ensureLoaded(level, route.to.pos())) {
            return;
        }
        MerchantStock stock = MerchantStock.at(level, route.to.pos());
        territory.touch(level, route.to.pos(), stock);
        Mint mint = Mint.get(server);
        double rate = mint.rate(Mint.DEFAULT_FACTION, now);
        if (caravan.leg == Caravan.Leg.OUTBOUND) {
            int earned = sell(level, route.to.pos(), stock, caravan, mint, rate, now, route);
            caravan.spurs += earned;
            route.salesSpurs += earned;
        }
        int food = foodNeeded(route.distance());
        Map<Item, Integer> meal = stock.eat(food);
        if (meal.isEmpty()) {
            caravan.leg = Caravan.Leg.WAITING_FOR_FOOD;
            caravan.arrives = now; // try again next poll
            route.status = "waiting for food at the destination";
        } else {
            double cost = 0;
            for (Map.Entry<Item, Integer> eaten : meal.entrySet()) {
                cost += eaten.getValue() * Prices.emeraldsEach(eaten.getKey());
            }
            int price = Math.min(caravan.spurs, (int) Math.ceil(cost * rate));
            caravan.spurs -= price;
            Treasury.deposit(level, route.to.pos(), stock, price);
            route.foodSpurs += price;
            route.foodEaten += food;
            caravan.leg = Caravan.Leg.RETURNING;
            caravan.arrives = now + travelTicks(route.distance());
            route.status = "on the way home";
        }
        ledger.setDirty();
    }

    private static void arriveHome(
            MinecraftServer server, CaravanLedger ledger, Territory territory, Route route, Caravan caravan) {
        ServerLevel level = server.getLevel(route.from.dimension());
        if (level == null || !ensureLoaded(level, route.from.pos())) {
            return;
        }
        MerchantStock stock = MerchantStock.at(level, route.from.pos());
        territory.touch(level, route.from.pos(), stock);
        int tolls = payTolls(territory, route, caravan.spurs);
        caravan.spurs -= tolls;
        route.tollsPaid += tolls;
        for (Iterator<ItemStack> it = caravan.cargo.iterator(); it.hasNext(); ) {
            ItemStack rest = stock.insert(it.next());
            if (rest.isEmpty()) {
                it.remove();
            }
        }
        Treasury.deposit(level, route.from.pos(), stock, caravan.spurs);
        route.spursCollected += caravan.spurs;
        route.trips++;
        route.status = "home";
        ledger.remove(route.id);
        ledger.setDirty();
        Enterprise.LOGGER.debug("Caravan {} home with {} spurs", route.id, caravan.spurs);
    }

    /**
     * Each other settlement whose land the route crosses takes {@link #TOLL_RATE} of the coins carried, one after
     * another. The two ends' own settlements charge nothing. Returns the spurs paid in all.
     */
    public static int payTolls(Territory territory, Route route, int spurs) {
        var dimension = route.from.dimension();
        Set<GlobalPos> ends = new LinkedHashSet<>();
        territory.bellOf(route.from).ifPresent(ends::add);
        territory.bellOf(route.to).ifPresent(ends::add);
        Set<GlobalPos> crossed = new LinkedHashSet<>();
        for (Hex hex : Hex.line(Hex.at(route.from.pos()), Hex.at(route.to.pos()))) {
            territory.owner(dimension, hex).filter(owner -> !ends.contains(owner)).ifPresent(crossed::add);
        }
        int paid = 0;
        for (GlobalPos owner : crossed) {
            int toll = (int) Math.floor((spurs - paid) * TOLL_RATE);
            if (toll > 0) {
                territory.oweToll(owner, toll);
                paid += toll;
            }
        }
        return paid;
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

    /**
     * Sells cargo to the destination's Merchant at {@link #WHOLESALE} of its local value: of each item, up to what it
     * wants ({@link #WANT_PER_ITEM} on hand) and can pay for. Returns spurs earned; what isn't sold rides home.
     */
    private static int sell(ServerLevel level, BlockPos desk, MerchantStock stock, Caravan caravan, Mint mint,
            double rate, long now, Route route) {
        int earned = 0;
        for (Iterator<ItemStack> it = caravan.cargo.iterator(); it.hasNext(); ) {
            ItemStack stack = it.next();
            double each = Prices.emeraldsEach(stack.getItem()) * WHOLESALE;
            int room = WANT_PER_ITEM - stock.count(new ItemCost(stack.getItem(), 1));
            int affordable = (int) Math.floor(Treasury.available(level, desk, stock, rate) / each);
            int quantity = Math.min(stack.getCount(), Math.min(room, affordable));
            if (quantity <= 0) {
                continue;
            }
            ItemStack sold = stack.split(quantity);
            ItemStack rest = stock.insert(sold);
            int delivered = sold.getCount() - rest.getCount();
            stack.grow(rest.getCount());
            Treasury.Withdrawal paid = Treasury.withdraw(level, desk, stock, delivered * each, mint, now);
            earned += paid.spurs() + mint.sell(Mint.DEFAULT_FACTION, paid.emeralds(), now);
            route.itemsDelivered += delivered;
            if (stack.isEmpty()) {
                it.remove();
            }
        }
        return earned;
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
