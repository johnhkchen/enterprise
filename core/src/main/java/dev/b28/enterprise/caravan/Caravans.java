package dev.b28.enterprise.caravan;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.territory.Chokepoints;
import dev.b28.enterprise.territory.Comforts;
import dev.b28.enterprise.territory.Terrain;
import dev.b28.enterprise.territory.Territory;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.locker.Rounds;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.money.Bargain;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.MoneyChanger;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.money.Treasury;
import dev.ithundxr.createnumismatics.content.coins.CoinItem;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.ToDoubleFunction;
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

    /** Share of a warehouse's slots its wanted goods may fill; the rest stays free for results, coins and food. */
    public static final double STOCK_SHARE = 0.6;

    /** The most a travelling Merchant pays for a town's surplus: this share of its local value (it resells it). */
    public static final double RESALE_CEILING = 0.75;

    /**
     * Emeralds a caravan takes from the hub per food point of the return leg, to buy that food at the destination:
     * bread is 30 points to the emerald; half again for dearer food.
     */
    public static final double PURSE_PER_FOOD_POINT = 1.5 / 30;


    /** A caravan carries this much of what a town wants of a comfort: it may have used more since its stocktake. */
    public static final double COMFORT_MARGIN = 1.5;

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
        var hubMerchant = Merchant.workingAt(level, route.from.pos(), Lockers.MERCHANT_RANGE);
        if (hubMerchant.isEmpty()) {
            route.status = "no Merchant at the hub";
            ledger.setDirty();
            return false;
        }
        int merchantLevel = hubMerchant.get().getVillagerData().getLevel();
        Warehouse.get(server).recordLevel(route.from, merchantLevel);
        MerchantStock stock = MerchantStock.at(level, route.from.pos());
        territory.touch(level, route.from.pos(), stock);
        Mint mint = Mint.get(server);
        double rate = mint.rate(Mint.DEFAULT_FACTION, now);
        RoutePlanner.Plan plan = plan(server, territory, route, rate);
        if (!plan.found()) {
            route.status = "no way there by land";
            ledger.setDirty();
            return false;
        }
        route.path = plan.path();
        if (!inRange(route.distance())) {
            route.status = "the cheapest path is too long for one leg's food";
            ledger.setDirty();
            return false;
        }
        int food = foodNeeded(route.distance());
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
        List<ItemStack> cargo = pickComforts(stock, comfortWants(server, territory, route.to));
        cargo.addAll(pickCargo(stock, wantedAt(server, route.to), CARGO_SLOTS - cargo.size()));
        Warehouse.get(server).recordStocktake(route.from, stock.totals()); // for buying decisions while away
        if (cargo.isEmpty() && !hasBusinessAt(server, ledger, route, now)) {
            route.status = "nothing to sell there and nothing wanted from there";
            ledger.setDirty();
            return false; // no empty trips: the road food would be wasted
        }
        stock.eat(food); // the hub's own food: already paid for
        Treasury.Withdrawal taken = Treasury.withdraw(level, route.from.pos(), stock, purse, mint, now);
        int spurs = taken.spurs() + mint.sell(Mint.DEFAULT_FACTION, taken.emeralds(), now);
        ledger.put(new Caravan(
                route.id, Caravan.Leg.OUTBOUND, now + travelTicks(route.distance()), cargo, spurs, merchantLevel));
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
     * At the destination the travelling Merchant deals only with the resident Merchant: it sells cargo into the
     * town's warehouse up to par, buys the town's surplus for the trip home, and buys the return leg's food at the
     * local price. Prices are bargained between the two Merchants' levels. Nothing is taken for free.
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
        int food = foodNeeded(route.distance());
        if (caravan.leg == Caravan.Leg.OUTBOUND) {
            int residentLevel = Merchant.workingAt(level, route.to.pos(), Lockers.MERCHANT_RANGE)
                    .map(m -> m.getVillagerData().getLevel())
                    .orElse(Warehouse.get(server).level(route.to));
            Warehouse.get(server).recordLevel(route.to, residentLevel);
            int earned = sell(server, level, route.to, stock, caravan, residentLevel, mint, rate, now, route, territory);
            caravan.spurs += earned;
            route.salesSpurs += earned;
            int keepForFood = (int) Math.ceil(food * PURSE_PER_FOOD_POINT * rate);
            buySurplus(server, ledger, level, route, stock, caravan, residentLevel, keepForFood, territory, mint, rate, now);
        }
        Warehouse.get(server).recordStocktake(route.to, stock.totals()); // what the next caravan should bring
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
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        int tolls = payTolls(territory, route, caravan.spurs, perHex(rate), terrain(server, route));
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

    /** As {@link #payTolls(Territory, Route, int, double, ToDoubleFunction)} on open ground at 16 spurs an emerald. */
    public static int payTolls(Territory territory, Route route, int spurs) {
        return payTolls(territory, route, spurs, perHex(16), RoutePlanner.FLAT);
    }

    /**
     * Each other settlement whose land the route's path crosses sees the caravan go by, and what going around each of
     * its hexes would have cost it; those with a toll post on the path take their toll rate of the coins carried, one
     * after another. The two ends' own settlements charge nothing. Returns the spurs paid in all.
     */
    public static int payTolls(
            Territory territory, Route route, int spurs, double perHex, ToDoubleFunction<Hex> terrain) {
        var dimension = route.from.dimension();
        Set<GlobalPos> ends = ends(territory, route);
        Map<GlobalPos, List<Hex>> crossed = new LinkedHashMap<>();
        for (Hex hex : route.path()) {
            territory.owner(dimension, hex).filter(owner -> !ends.contains(owner))
                    .ifPresent(owner -> crossed.computeIfAbsent(owner, o -> new ArrayList<>()).add(hex));
        }
        int paid = 0;
        for (Map.Entry<GlobalPos, List<Hex>> town : crossed.entrySet()) {
            GlobalPos owner = town.getKey();
            int carried = spurs - paid;
            territory.recordTraffic(owner, carried);
            territory.observeCrossing(owner, route.id, carried, postOptions(territory, route, owner, ends, perHex, terrain));
            boolean posted = town.getValue().stream()
                    .anyMatch(hex -> territory.postOwner(dimension, hex).filter(owner::equals).isPresent());
            int toll = posted ? (int) Math.floor(carried * territory.tollRate(owner)) : 0;
            if (toll > 0) {
                territory.oweToll(owner, toll);
                paid += toll;
            }
        }
        return paid;
    }

    /**
     * What guarding the town's land would do to a caravan on this route. The town tries a guard on each hex of its land
     * on the caravan's cheapest path and keeps the one that makes going around dearest (on a tie: the narrowest, with
     * the most impassable neighbours, then the nearest its home); then does the same on the cheapest path avoiding the
     * posts so far, and so on, until the path keeps off its land, there is no way at all, or
     * {@link Chokepoints#MAX_POSTS_PER_ROUTE} posts. Costs are with the town charging nothing, in spurs.
     */
    static List<Chokepoints.Option> postOptions(Territory territory, Route route, GlobalPos town, Set<GlobalPos> ends,
            double perHex, ToDoubleFunction<Hex> terrain) {
        var dimension = route.from.dimension();
        Hex start = Hex.at(route.from.pos());
        Hex goal = Hex.at(route.to.pos());
        Hex home = Hex.at(town.pos());
        Set<GlobalPos> free = new LinkedHashSet<>(ends);
        free.add(town);
        double value = route.expectedValue();
        RoutePlanner.Plan through = RoutePlanner.plan(territory, dimension, start, goal, free, perHex, terrain, value, Set.of());
        List<Chokepoints.Option> options = new ArrayList<>();
        Set<Hex> posts = new LinkedHashSet<>();
        RoutePlanner.Plan path = through;
        while (path.found() && posts.size() < Chokepoints.MAX_POSTS_PER_ROUTE) {
            List<Hex> landOnPath = path.path().stream()
                    .filter(hex -> !hex.equals(start) && !hex.equals(goal))
                    .filter(hex -> territory.owner(dimension, hex).filter(town::equals).isPresent())
                    .toList();
            if (landOnPath.isEmpty()) {
                break; // this path keeps off the town's land
            }
            Hex pick = null;
            RoutePlanner.Plan picked = null;
            for (Hex hex : landOnPath) {
                Set<Hex> trial = new LinkedHashSet<>(posts);
                trial.add(hex);
                RoutePlanner.Plan around = RoutePlanner.plan(territory, dimension, start, goal, free, perHex, terrain, value, trial);
                if (pick == null || better(around, hex, picked, pick, home, terrain)) {
                    pick = hex;
                    picked = around;
                }
            }
            posts.add(pick);
            path = picked;
            options.add(new Chokepoints.Option(List.copyOf(posts),
                    path.found() ? Math.max(0, path.cost() - through.cost()) : Double.POSITIVE_INFINITY));
        }
        return options;
    }

    /** Is posting {@code hex} (leaving {@code around}) better for the town than posting {@code than}? */
    private static boolean better(RoutePlanner.Plan around, Hex hex, RoutePlanner.Plan aroundThan, Hex than, Hex home,
            ToDoubleFunction<Hex> terrain) {
        if (Math.abs(around.cost() - aroundThan.cost()) > 1e-6) {
            return around.cost() > aroundThan.cost();
        }
        int narrow = impassableNeighbours(hex, terrain);
        int narrowThan = impassableNeighbours(than, terrain);
        if (narrow != narrowThan) {
            return narrow > narrowThan;
        }
        return hex.distance(home) < than.distance(home);
    }

    private static int impassableNeighbours(Hex hex, ToDoubleFunction<Hex> terrain) {
        return (int) hex.neighbors().stream().filter(n -> Double.isInfinite(terrain.applyAsDouble(n))).count();
    }

    /** The towns at either end of a route: they charge their own caravans no toll. */
    static Set<GlobalPos> ends(Territory territory, Route route) {
        Set<GlobalPos> ends = new LinkedHashSet<>();
        territory.bellOf(route.from).ifPresent(ends::add);
        territory.bellOf(route.to).ifPresent(ends::add);
        return ends;
    }

    /** Road food both ways across a hex of open ground, at bread prices, in spurs. */
    public static double perHex(double rate) {
        return 2 * (Hex.ACROSS / BLOCKS_PER_FOOD) * (1 / 30.0) * rate;
    }

    /** How hard each hex on the route's side of the world is to cross ({@link Terrain}). */
    static ToDoubleFunction<Hex> terrain(MinecraftServer server, Route route) {
        ServerLevel level = server.getLevel(route.from.dimension());
        if (level == null) {
            return RoutePlanner.FLAT;
        }
        Terrain terrain = Terrain.get(server);
        return hex -> terrain.crossingFactor(level, hex);
    }

    /**
     * The cheapest hex path for the next caravan: a hex costs its road food both ways (at bread prices) times how hard
     * it is to cross, and each town's toll post costs its toll on what caravans on this route usually carry home.
     */
    public static RoutePlanner.Plan plan(MinecraftServer server, Territory territory, Route route, double rate) {
        return RoutePlanner.plan(territory, route.from.dimension(), Hex.at(route.from.pos()), Hex.at(route.to.pos()),
                ends(territory, route), perHex(rate), terrain(server, route), route.expectedValue(), Set.of());
    }

    /**
     * What the destination wants of each kind of comfort, in emeralds' worth: its par less what it held at its last
     * stocktake (none taken yet: the whole par).
     */
    static Map<String, Double> comfortWants(MinecraftServer server, Territory territory, GlobalPos desk) {
        Warehouse warehouse = Warehouse.get(server);
        double par = territory.comfortPar(desk);
        Map<String, Double> wants = new LinkedHashMap<>();
        for (var kind : Comforts.kinds()) {
            double held = 0;
            for (var holder : BuiltInRegistries.ITEM.getTagOrEmpty(kind)) {
                held += warehouse.onHand(desk, holder.value()) * Prices.emeraldsEach(holder.value());
            }
            if (par - held > 0) {
                wants.put(Comforts.name(kind), par - held);
            }
        }
        return wants;
    }

    /** Comforts from the hub's stock for what the destination wants, a little over, cheapest goods first. */
    static List<ItemStack> pickComforts(MerchantStock stock, Map<String, Double> wants) {
        List<ItemStack> cargo = new ArrayList<>();
        for (Map.Entry<String, Double> want : wants.entrySet()) {
            var kind = Comforts.kind(want.getKey()).orElseThrow();
            double worth = 0;
            List<Item> goods = stock.totals().keySet().stream()
                    .map(id -> BuiltInRegistries.ITEM.get(ResourceLocation.parse(id)))
                    .filter(item -> item.builtInRegistryHolder().is(kind))
                    .sorted(Comparator.comparingDouble(Prices::emeraldsEach))
                    .toList();
            for (Item item : goods) {
                double each = Prices.emeraldsEach(item);
                int count = (int) Math.ceil((want.getValue() * COMFORT_MARGIN - worth) / each);
                if (count <= 0) {
                    break;
                }
                for (ItemStack piece : stock.take(new ItemCost(item, 1), count)) {
                    worth += piece.getCount() * each;
                    cargo.add(piece);
                }
            }
        }
        return cargo;
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

    /** Up to {@code slots} stacks, shared across the wanted items, each up to {@link #WANT_PER_ITEM}. */
    static List<ItemStack> pickCargo(MerchantStock stock, Set<Item> wanted, int slots) {
        List<ItemStack> cargo = new ArrayList<>();
        if (wanted.isEmpty() || slots <= 0) {
            return cargo;
        }
        int perItem = Math.min(WANT_PER_ITEM, slots / wanted.size() * 64);
        for (Item item : wanted) {
            for (ItemStack piece : stock.take(new ItemCost(item, 1), perItem)) {
                cargo.add(piece);
            }
        }
        return cargo;
    }

    /**
     * Sells cargo into the resident's warehouse: of each good, only up to its par, at the price the two Merchants
     * settle (the resident pays more the shorter it is), and only what it can pay for. Returns spurs earned; what
     * isn't sold rides home.
     */
    private static int sell(MinecraftServer server, ServerLevel level, GlobalPos desk, MerchantStock stock,
            Caravan caravan, int residentLevel, Mint mint, double rate, long now, Route route, Territory territory) {
        Warehouse warehouse = Warehouse.get(server);
        Set<Item> wanted = wantedAt(server, desk);
        int earned = 0;
        for (Iterator<ItemStack> it = caravan.cargo.iterator(); it.hasNext(); ) {
            ItemStack stack = it.next();
            Item item = stack.getItem();
            int onHand = stock.count(new ItemCost(item, 1));
            double value = Prices.emeraldsEach(item);
            var comfort = Comforts.kindOf(item);
            int par;
            double money = Treasury.available(level, desk.pos(), stock, rate);
            if (comfort.isPresent()) {
                // Comforts: up to the kind's par in worth, paid from money the town doesn't need for wages and guards.
                double held = Comforts.onHand(stock).getOrDefault(Comforts.name(comfort.get()), 0.0);
                // Whole items: a town short of a kind buys one even if it costs more than the gap (a pair of boots).
                par = onHand + (int) Math.max(0, Math.ceil((territory.comfortPar(desk) - held) / value));
                money -= territory.reserve(desk);
            } else {
                par = Math.min(warehouse.par(desk, item, wanted.contains(item), now), roomFor(stock, wanted.size(), item));
            }
            int quantity = Math.min(stack.getCount(), par - onHand);
            double each = -1;
            for (int pass = 0; pass < 2 && quantity > 0; pass++) {
                // Price at the middle of the batch: the resident's shortage shrinks as the shelf fills.
                double midShortage = Bargain.shortage(onHand + quantity / 2, par);
                each = Bargain.price(value, midShortage, caravan.merchantLevel, residentLevel);
                quantity = each <= 0 ? 0 : Math.min(quantity, (int) Math.floor(money / each));
            }
            if (quantity <= 0) {
                continue;
            }
            while (quantity > 0 && !stock.hasRoomFor(stack.copyWithCount(quantity))) {
                quantity -= Math.min(quantity, item.getDefaultMaxStackSize()); // keep the warehouse's free slots
            }
            if (quantity <= 0) {
                continue;
            }
            ItemStack sold = stack.split(quantity);
            ItemStack rest = stock.insert(sold);
            int delivered = sold.getCount() - rest.getCount();
            stack.grow(rest.getCount());
            Treasury.Withdrawal paid = Treasury.withdraw(level, desk.pos(), stock, delivered * each, mint, now);
            earned += paid.spurs() + mint.sell(Mint.DEFAULT_FACTION, paid.emeralds(), now);
            route.itemsDelivered += delivered;
            if (stack.isEmpty()) {
                it.remove();
            }
        }
        return earned;
    }

    /** Whether a caravan with no cargo would still do business: the network wants goods this town can spare. */
    static boolean hasBusinessAt(MinecraftServer server, CaravanLedger ledger, Route route, long now) {
        Map<Item, Integer> wants = networkWants(server, ledger, route,
                new Caravan(route.id, Caravan.Leg.OUTBOUND, now, new java.util.ArrayList<>(), 0, 1), now);
        Warehouse warehouse = Warehouse.get(server);
        return wants.keySet().stream().anyMatch(item -> warehouse.onHand(route.to, item) > 0);
    }

    /**
     * What the hub's other towns want of each good and the hub doesn't already hold: the sum of their pars, less the
     * hub's stock at its last stocktake and what's already aboard.
     */
    static Map<Item, Integer> networkWants(MinecraftServer server, CaravanLedger ledger, Route route, Caravan caravan, long now) {
        Warehouse warehouse = Warehouse.get(server);
        Map<Item, Integer> wants = new java.util.HashMap<>();
        for (Route other : ledger.routes()) {
            if (other.from.equals(route.from) && !other.to.equals(route.to)) {
                for (Item item : wantedAt(server, other.to)) {
                    wants.merge(item, warehouse.par(other.to, item, true, now), Integer::sum);
                }
            }
        }
        wants.replaceAll((item, want) -> want - warehouse.onHand(route.from, item)
                - caravan.cargo.stream().filter(stack -> stack.is(item)).mapToInt(ItemStack::getCount).sum());
        wants.values().removeIf(want -> want <= 0);
        return wants;
    }

    /**
     * The most of one good the warehouse can hold and still trade: wanted goods share {@link #STOCK_SHARE} of its
     * slots, leaving the rest for locker results, coins and food.
     */
    static int roomFor(MerchantStock stock, int wantedGoods, Item item) {
        return (int) (stock.slots() * STOCK_SHARE / Math.max(1, wantedGoods)) * item.getDefaultMaxStackSize();
    }

    /**
     * Buys the town's surplus for the trip home, but only goods another town on the hub's routes wants and the hub
     * lacks ({@link #networkWants}): goods above this warehouse's par (and food beyond what it keeps for caravans),
     * at the price the two Merchants settle, the travelling Merchant paying at most {@link #RESALE_CEILING} of the
     * value since it has to resell. Keeps {@code keepForFood} spurs for road food and stays within its cargo space.
     * Crop sales count toward the town's farmers' share.
     */
    private static void buySurplus(MinecraftServer server, CaravanLedger ledger, ServerLevel level, Route route,
            MerchantStock stock, Caravan caravan, int residentLevel, int keepForFood, Territory territory, Mint mint,
            double rate, long now) {
        GlobalPos desk = route.to;
        Warehouse warehouse = Warehouse.get(server);
        Set<Item> wanted = wantedAt(server, desk);
        Map<Item, Integer> networkWants = networkWants(server, ledger, route, caravan, now);
        int spareFood = stock.foodNutrition() - Warehouse.FOOD_PAR_POINTS;
        for (Map.Entry<String, Integer> held : stock.totals().entrySet()) {
            if (caravan.cargo.size() >= CARGO_SLOTS) {
                return;
            }
            Item item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(held.getKey()));
            if (item == Items.EMERALD || item instanceof CoinItem || held.getKey().endsWith("shulker_box")) {
                continue;
            }
            int surplus = Math.min(held.getValue() - warehouse.par(desk, item, wanted.contains(item), now),
                    networkWants.getOrDefault(item, 0));
            var food = item.getDefaultInstance().get(net.minecraft.core.component.DataComponents.FOOD);
            if (food != null && food.nutrition() > 0) {
                surplus = Math.min(surplus, Math.max(0, spareFood) / food.nutrition());
            }
            int room = (CARGO_SLOTS - caravan.cargo.size()) * item.getDefaultMaxStackSize();
            double value = Prices.emeraldsEach(item);
            double each = Bargain.between(Bargain.sellerFloor(value), value * RESALE_CEILING, residentLevel,
                    caravan.merchantLevel);
            int quantity = Math.min(surplus, room);
            if (each > 0) {
                quantity = Math.min(quantity, (int) Math.floor((caravan.spurs - keepForFood) / (each * rate)));
            }
            if (each <= 0 || quantity <= 0) {
                continue;
            }
            int spurs = (int) Math.ceil(quantity * each * rate);
            int taken = 0;
            for (ItemStack piece : stock.take(new ItemCost(item, 1), quantity)) {
                taken += piece.getCount();
                caravan.cargo.add(piece);
            }
            spurs = (int) Math.ceil(taken * each * rate);
            caravan.spurs -= spurs;
            Treasury.deposit(level, desk.pos(), stock, spurs);
            if (food != null) {
                spareFood -= taken * food.nutrition();
            }
            if (Territory.isCrop(held.getKey())) {
                territory.recordCropSale(desk, spurs / rate);
            }
            route.itemsBought += taken;
            route.purchaseSpurs += spurs;
        }
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
