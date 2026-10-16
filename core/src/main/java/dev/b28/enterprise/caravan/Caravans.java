package dev.b28.enterprise.caravan;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.cargo.Crates;
import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.territory.Chokepoints;
import dev.b28.enterprise.territory.Comforts;
import dev.b28.enterprise.territory.Terrain;
import dev.b28.enterprise.territory.Works;
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
import net.minecraft.nbt.CompoundTag;
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


    /** A town keeps this many grown farm animals per villager in its pasture; below that it buys them from Shepherds. */
    public static final int ANIMALS_PER_VILLAGER = 2;

    /** A squad takes a Librarian when the destination's books are older than this, a Cartographer when its map is. */
    public static final long STALE_BOOKS_TICKS = 3 * 24000L;
    public static final long STALE_MAP_TICKS = 10 * 24000L;

    /** A caravan carries this much of what a town wants of a comfort: it may have used more since its stocktake. */
    public static final double COMFORT_MARGIN = 1.5;

    /** A caravan with a saddle (a master Leatherworker's) rides this much faster; a trip wears this much of one. */
    public static final double SADDLED_PACE = 4 / 3.0;
    public static final double SADDLE_WEAR_PER_TRIP = 0.1;

    /** At most one departure per route per in-game day. */
    public static final long DEPARTURE_INTERVAL = 24000;

    private static final long POLL_TICKS = 100;
    private static final int TICKET_TICKS = 200;
    static final TicketType<ChunkPos> CARAVAN =
            TicketType.create("enterprise_caravan", Comparator.comparingLong(ChunkPos::toLong), TICKET_TICKS);

    private static MinecraftServer server(ServerLevel level) {
        return level.getServer();
    }

    /** A purchase at the far end, in both sets of books: the far market sold it, the caravan's log bought it. */
    static void boughtThere(MinecraftServer server, Route route, Caravan caravan, Item item, int count, int spurs, long now) {
        dev.b28.enterprise.merchant.Books.get(server).enter(server, route.to, now,
                dev.b28.enterprise.merchant.Books.Kind.SOLD, route.from, item, count, spurs);
        caravan.log.add(new dev.b28.enterprise.merchant.Books.Entry(now / 24000,
                dev.b28.enterprise.merchant.Books.Kind.BOUGHT, route.to, item, count, spurs));
    }

    /** A foreign realm's Merchants haggle as Experts, and two of its people go with each caravan or fleet. */
    static final int FOREIGN_MERCHANT_LEVEL = 4;
    static final int FOREIGN_CREW = 2;

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
        dev.b28.enterprise.contract.ContractBook.get(server).review(server, now);
        dev.b28.enterprise.contract.Rents.get(server).review(dev.b28.enterprise.contract.ContractBook.get(server), now);
        dev.b28.enterprise.contract.Bills.get(server).lapse(server, dev.b28.enterprise.contract.ContractBook.get(server), now);
        ledger.walkersHome(now).forEach(walker -> {
            if (dev.b28.enterprise.merchant.OffMap.get(server).is(walker.home())) {
                ledger.arrived(walker); // home off the map (a mercenary over the sea): gone
                return;
            }
            ServerLevel level = server.getLevel(walker.home().dimension());
            if (level != null && ensureLoaded(level, walker.home().pos())) {
                Squad.release(level, walker.home().pos(), walker.villager());
                territory.bellOf(walker.home()).ifPresent(bell -> dev.b28.enterprise.settlement.Settlements.get(server)
                        .setAway(bell, walker.villager().getUUID("UUID"), false));
                ledger.arrived(walker);
            }
        });
        for (Route route : List.copyOf(ledger.routes())) {
            var caravan = ledger.caravan(route.id);
            if (caravan.isEmpty()) {
                if (route.stopping) {
                    ledger.retire(route);
                } else if (now - route.lastDeparture >= DEPARTURE_INTERVAL
                        && !dispatch(server, ledger, territory, route, now)) {
                    // Why no caravan left today, in the books with the rest of its reasons.
                    dev.b28.enterprise.merchant.Books.get(server).why(route.from, now, route.to, Items.AIR,
                            "no trip: " + route.status, 0);
                }
            } else if (now >= caravan.get().arrives) {
                Caravan c = caravan.get();
                if (c.stop != null && !c.stopped && c.leg != Caravan.Leg.WAITING_FOR_FOOD) {
                    callAtStop(server, ledger, territory, route, c, now);
                } else {
                    arrive(server, ledger, territory, route, c, now);
                }
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
        // A foreign realm's market (off the map) sends its own traders: its Merchant and crew are its own, never seen.
        boolean foreign = dev.b28.enterprise.trade.Realms.get(server).isForeign(route.from);
        var hubMerchant = foreign ? java.util.Optional.<net.minecraft.world.entity.npc.Villager>empty()
                : Merchant.workingAt(level, route.from.pos(), Lockers.MERCHANT_RANGE);
        if (!foreign && hubMerchant.isEmpty()) {
            route.status = "no Merchant at the hub";
            ledger.setDirty();
            return false;
        }
        Warehouse.get(server).recordLevel(route.from, foreign ? FOREIGN_MERCHANT_LEVEL : hubMerchant.get().getVillagerData().getLevel());
        MerchantStock stock = MerchantStock.at(level, route.from.pos());
        territory.touch(level, route.from.pos(), stock);
        Mint mint = Mint.get(server);
        double rate = mint.rate(Mint.DEFAULT_FACTION, now);
        // By land, and (if a Fisherman could captain it) by boat: is the water way better?
        List<net.minecraft.world.entity.npc.Villager> home = Squad.available(level, route.from, Squad.reach());
        RoutePlanner.Plan byLand = plan(server, territory, route, rate, false);
        boolean captain = foreign || home.stream().anyMatch(v -> Squad.trade(v).equals(Squad.FISHERMAN));
        RoutePlanner.Plan byBoat = captain ? plan(server, territory, route, rate, true) : byLand;
        boolean water = byBoat.found() && waterHexes(server, route.from.dimension(), byBoat.path()) > 0
                && (!byLand.found() || byBoat.cost() < byLand.cost());
        // A lead, and the trades this trip has a use for, while every other route waiting here keeps a lead.
        long waiting = ledger.routes().stream()
                .filter(r -> r.from.equals(route.from) && ledger.caravan(r.id).isEmpty())
                .count();
        // Who'd be of use there: those whose services the town wants, and packs for more than a wagon holds.
        Map<String, Integer> skills = new LinkedHashMap<>();
        home.forEach(v -> skills.merge(Squad.trade(v), v.getVillagerData().getLevel(), Math::max));
        Set<String> wanted = new LinkedHashSet<>(
                dev.b28.enterprise.service.Services.wantedAbroad(level, territory, route, skills, now));
        int stacks = wantedStacks(stock, wantedThere(server, route.to));
        int porters = 0;
        if (stacks > CARGO_SLOTS) {
            wanted.add(Squad.LEATHERWORKER);
            boolean packs = skills.containsKey(Squad.LEATHERWORKER);
            porters = (int) Math.ceil((stacks - CARGO_SLOTS - (packs ? Squad.PACK_STACKS : 0)) / (double) Squad.PACK_STACKS);
        }
        Squad.Needs needs = new Squad.Needs(wanted, water, Math.max(0, porters));
        int extras = (int) Math.max(0, home.size() - waiting);
        List<net.minecraft.world.entity.npc.Villager> mustered =
                Squad.muster(level, route.from, Squad.reach(), needs, extras);
        if (mustered.isEmpty() && !foreign) {
            // No one to lead it: the Merchant takes on a hand of the town waiting for work, if it can pay one.
            int routes = (int) ledger.routes().stream().filter(r -> r.from.equals(route.from) && !r.stopping).count();
            int out = (int) ledger.routes().stream()
                    .filter(r -> r.from.equals(route.from) && ledger.caravan(r.id).isPresent()).count();
            var hired = Squad.hire(level, route.from, stock, routes, out, now);
            if (hired.isPresent()) {
                Enterprise.LOGGER.info("{} takes on a hand as a travelling Merchant, for its run to {}",
                        dev.b28.enterprise.settlement.Names.label(route.from), dev.b28.enterprise.settlement.Names.label(route.to));
                mustered = Squad.muster(level, route.from, Squad.reach(), needs, extras);
            }
        }
        if (mustered.isEmpty() && !foreign) {
            route.status = "no squad at home: no hand waiting for work to take on as a travelling Merchant, or no money to pay one";
            ledger.setDirty();
            return false;
        }
        boolean boat = (foreign || mustered.stream().anyMatch(v -> Squad.trade(v).equals(Squad.FISHERMAN))) && water;
        if (captain || waterHexes(server, route.from.dimension(), byLand.path()) > 0 || !byLand.found()) {
            Enterprise.LOGGER.debug("Route {}: by land {} ({} hexes, {}), by boat {} ({} hexes, {}); captain {}, mustered {}",
                    route.id, byLand.found(), byLand.path().size(), Math.round(byLand.cost()), byBoat.found(),
                    byBoat.path().size(), Math.round(byBoat.cost()), captain, mustered.stream().map(Squad::trade).toList());
        }
        RoutePlanner.Plan plan = boat ? byBoat : byLand;
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
        // Each member eats; one more goes only if the town can spare its food beyond the caravans' par.
        int members = mustered.size();
        while (members > 1 && stock.foodNutrition() - Warehouse.FOOD_PAR_POINTS < foodNeeded(route.distance()) * members) {
            members--;
        }
        List<net.minecraft.world.entity.npc.Villager> squad = List.copyOf(mustered.subList(0, members));
        int food = foodNeeded(route.distance()) * (foreign ? FOREIGN_CREW : squad.size());
        // Where the far end won't or can't feed the squad for the road home (the front sells nothing; a town may be
        // short), it takes that food along too, if a caravan can carry both legs'.
        // (what other caravans on their way there will buy of its food first doesn't count)
        boolean fedThere = dev.b28.enterprise.merchant.OffMap.get(server).sells(route.to)
                && foodKnownAt(server, level, route.to) - ledger.foodPromisedAt(route.to) >= food;
        // A town too poor to pay the far end for the road home's food packs that too, from food it can spare beyond
        // its par: a town rich in bread and poor in coin can still send its goods out.
        boolean ownFood = !foreign && stock.foodNutrition() - Warehouse.FOOD_PAR_POINTS >= 2 * food
                && Treasury.available(level, route.from.pos(), stock, rate) - territory.reserve(route.from)
                        < food * PURSE_PER_FOOD_POINT;
        int provisions = (!fedThere || ownFood) && 2 * foodNeeded(route.distance()) <= FOOD_CAPACITY ? food : 0;
        double roadPurse = provisions > 0 ? 0 : food * PURSE_PER_FOOD_POINT;
        // A town sending its own caravan gives it money to bring home the comforts it lacks.
        Map<String, Double> shopping = Commissions.shoppingList(territory, route.from, stock);
        double spare = Math.max(0, Treasury.available(level, route.from.pos(), stock, rate) - territory.reserve(route.from)
                - roadPurse);
        double shoppingMoney = Math.min(spare, shopping.values().stream().mapToDouble(Double::doubleValue).sum());
        // Shopping is worth the trip only if it's worth twice the road food it costs.
        if (shoppingMoney < Math.max(0.5, 2 * food * PURSE_PER_FOOD_POINT)) {
            shopping = Map.of();
            shoppingMoney = 0;
        }
        // And the money to pay for what it collects there Ex Works, as far as the town can spare it.
        double exWorks = Math.min(exWorksPurse(server, route, now + travelTicks(route.distance()))
                        + dev.b28.enterprise.contract.Bills.get(server).dueAt(route.from, route.to,
                                now + travelTicks(route.distance())), // and the bills on it that lie there
                Math.max(0, spare - shoppingMoney));
        // A bank's caravan carries money for its dealings there too, as far as its market can spare.
        double bankFloat = dev.b28.enterprise.market.Bank.get(server).is(route.from)
                ? Math.max(0, Math.min(dev.b28.enterprise.market.Bank.FLOAT, spare - shoppingMoney - exWorks)) : 0;
        // And the capital to buy back the rents it owes there, each when it can spare twice that.
        double buyBack = 0;
        for (var rent : dev.b28.enterprise.contract.Rents.get(server).owed(route.from, route.to)) {
            if (spare - shoppingMoney - exWorks - bankFloat - buyBack >= 2 * rent.capital()) {
                buyBack += rent.capital();
            }
        }
        double purse = roadPurse + shoppingMoney + exWorks + bankFloat + buyBack;
        if (stock.foodNutrition() < food + provisions) {
            route.status = "waiting for food at the hub";
            ledger.setDirty();
            return false;
        }
        if (Treasury.available(level, route.from.pos(), stock, rate) < purse) {
            route.status = "the hub can't fund the trip";
            ledger.setDirty();
            return false;
        }
        // Llamas from the pen carry packs too: the town's own, home with the caravan.
        var llamas = Squad.llamas(level, route.from.pos());
        int slots = CARGO_SLOTS + Squad.PACK_STACKS * (int) squad.stream()
                .filter(v -> Squad.carries(Squad.trade(v))).count() // Leatherworkers' packs, porters' backs
                + Squad.LLAMA_STACKS * llamas.size();
        boolean shepherd = squad.stream().anyMatch(v -> Squad.trade(v).equals("minecraft:shepherd"));
        var herd = shepherd ? Squad.herd(level, route.from.pos()) : List.<net.minecraft.world.entity.animal.Animal>of();
        // Goods owed under contracts go first, then comforts, then what the destination's trades want.
        java.util.Map<Integer, Integer> contracted = new LinkedHashMap<>();
        Crates.receive(server, level, route.from, stock, now); // crates consigned here, a player brought them
        dev.b28.enterprise.market.Procurement.procure(level, route.from, stock); // its villagers sell what it owes
        // At a freight hub, the crates booked on this run load first: they're paid for.
        List<ItemStack> cargo = new ArrayList<>(dev.b28.enterprise.market.Freight.isHub(server, route.from)
                ? dev.b28.enterprise.market.Freight.get(server).loadBooked(route.from, stock,
                        new dev.b28.enterprise.market.Freight.Leg(route, true), slots)
                : List.of());
        cargo.addAll(pickContracted(server, level, route, stock, now + travelTicks(route.distance()),
                slots - Crates.stacks(cargo), contracted, now));
        cargo.addAll(pickForwarded(server, level, route, stock, now + travelTicks(route.distance()),
                slots - Crates.stacks(cargo), now));
        // Its stranded paper, to sell at the hub this run goes to or calls at (see Stranded).
        var strandedHub = dev.b28.enterprise.market.Freight.isHub(server, route.to) ? java.util.Optional.of(route.to)
                : dev.b28.enterprise.market.Freight.stopOn(server, route).map(dev.b28.enterprise.market.Freight.Stop::hub);
        if (strandedHub.isPresent() && !foreign) {
            GlobalPos hub = strandedHub.get();
            int[] room = {slots - Crates.stacks(cargo)};
            cargo.addAll(stock.removeLoose(s -> {
                if (room[0] <= 0 || !dev.b28.enterprise.market.Stranded.crate(server, hub, route.from, s)) {
                    return false;
                }
                room[0] -= Crates.stacks(List.of(s));
                return true;
            }));
            cargo.addAll(stock.removeLoose(s -> s.is(dev.b28.enterprise.cargo.Cargo.DOCUMENTS.get())
                    && dev.b28.enterprise.contract.Bills.get(server).in(s).stream()
                            .anyMatch(b -> dev.b28.enterprise.market.Stranded.bill(server, hub, route.from, b, now))));
        }
        // Comforts the far end wants, out of what this town can spare beyond its own par of each kind.
        Map<String, Double> wantedThereNow = new LinkedHashMap<>(
                comfortWants(server, territory, route.to, now + travelTicks(route.distance())));
        double ownPar = territory.comfortPar(route.from);
        Map<String, Double> heldHere = Comforts.onHand(stock);
        var books = dev.b28.enterprise.merchant.Books.get(server);
        wantedThereNow.forEach((kind, want) -> {
            if (heldHere.getOrDefault(kind, 0.0) <= ownPar) {
                books.why(route.from, now, route.to, Items.AIR, "not sent: they're short of " + kind
                        + (heldHere.getOrDefault(kind, 0.0) > 0 ? ", but we keep our own par" : ", and we have none"), 0);
            }
        });
        wantedThereNow.replaceAll((kind, want) -> Math.min(want,
                Math.max(0, heldHere.getOrDefault(kind, 0.0) - ownPar) / COMFORT_MARGIN));
        wantedThereNow.values().removeIf(want -> want <= 0);
        int before = cargo.size();
        cargo.addAll(pickComforts(stock, wantedThereNow));
        while (Crates.stacks(cargo) > slots && cargo.size() > before) {
            stock.insert(cargo.remove(cargo.size() - 1)); // more comforts than the squad can carry: leave the rest
        }
        dev.b28.enterprise.merchant.Books.byGood(cargo.subList(before, cargo.size())).forEach((item, count) ->
                books.why(route.from, now, route.to, item, "sent: they're short of "
                        + Comforts.kindOf(item).map(Comforts::name).orElse("it"), count));
        // Goods for sale go packed, a crate to a good: the buyer stores them as they come, nine stacks to a slot.
        cargo.addAll(lots(level, route.from, stock, pickCargo(server, route.from, route.to, stock,
                wantsThere(server, route.to), slots - Crates.stacks(cargo), now), now));
        Warehouse.get(server).recordStocktake(route.from, stock.totals()); // for buying decisions while away
        var army = dev.b28.enterprise.army.Army.get(server);
        boolean toFront = army.front().filter(route.to::equals).isPresent();
        if (cargo.isEmpty() && herd.isEmpty() && exWorks <= 0 && !hasBusinessAt(server, ledger, route, now)
                && !hasShoppingAt(server, route.to, shopping) && !(toFront && army.waiting() > 0)
                && !recruitsOwedAt(server, route)
                && dev.b28.enterprise.contract.ContractBook.get(server).copiesFor(route.from, route.to).isEmpty()
                && dev.b28.enterprise.contract.ContractBook.get(server).copiesFor(route.to, route.from).isEmpty()) {
            route.status = "nothing to sell there and nothing wanted from there";
            ledger.setDirty();
            return false; // no empty trips: the road food would be wasted
        }
        stock.eat(food + provisions); // the hub's own food: already paid for
        Treasury.Withdrawal taken = Treasury.withdraw(level, route.from.pos(), stock, purse, mint, now);
        int spurs = taken.spurs() + mint.sell(Mint.DEFAULT_FACTION, taken.emeralds(), now);
        List<CompoundTag> crew = new ArrayList<>();
        territory.bellOf(route.from).ifPresent(bell -> squad.forEach(member ->
                dev.b28.enterprise.settlement.Settlements.get(server).setAway(bell, member.getUUID(), true)));
        squad.forEach(member -> ledger.recordService(route.from, member.getUUID()));
        squad.forEach(member -> crew.add(Squad.stow(member)));
        // Saddled, the caravan rides: a third faster. Each trip wears a tenth of a saddle.
        double pace = 1;
        if (stock.count(new ItemCost(Items.SADDLE, 1)) > 0) {
            pace = SADDLED_PACE;
            route.saddleWear += SADDLE_WEAR_PER_TRIP;
            if (route.saddleWear >= 1) {
                stock.take(new ItemCost(Items.SADDLE, 1), 1);
                route.saddleWear -= 1;
            }
        }
        cargo.addAll(dev.b28.enterprise.contract.Bills.takeBillsOn(stock, route.to)); // bills to present there
        List<CompoundTag> recruits = toFront ? army.embark(Integer.MAX_VALUE) : List.of(); // to the war, with the kit
        Caravan leaving = new Caravan(route.id, Caravan.Leg.OUTBOUND, now + (long) (travelTicks(route.distance()) / pace),
                cargo, spurs, foreign ? FOREIGN_MERCHANT_LEVEL : Squad.haggleLevel(crew));
        leaving.pace = pace;
        leaving.crew.addAll(crew);
        // Fishermen fish the water they cross: a catch for each on each water hex, sold where food is short.
        long fishermen = crew.stream().filter(m -> Squad.trade(m).equals(Squad.FISHERMAN)).count();
        boolean master = crew.stream().anyMatch(m -> Squad.trade(m).equals(Squad.FISHERMAN) && Squad.level(m) >= 5);
        int catchCount = (int) (fishermen * waterHexes(server, route.from.dimension(), route.path()) * Squad.FISH_PER_WATER_HEX
                * (master ? 1.5 : 1)); // a master Fisherman knows the deep water
        for (int left = catchCount; left > 0; left -= 64) {
            leaving.cargo.add(new ItemStack(Items.COD, Math.min(64, left)));
        }
        // And squid at sea: ink, for the clerks of the markets inland.
        for (int left = catchCount / INK_PER_CATCH; left > 0; left -= 64) {
            leaving.cargo.add(new ItemStack(Items.INK_SAC, Math.min(64, left)));
        }
        route.fishCaught += catchCount;
        leaving.shopping.putAll(shopping);
        leaving.contracted.putAll(contracted);
        leaving.recruits.addAll(recruits);
        leaving.provisions = provisions;
        Atlas.get(server).learnBoth(route.from, route.to, now);
        herd.forEach(animal -> leaving.herd.add(Squad.stow(animal)));
        llamas.forEach(llama -> leaving.pack.add(Squad.stow(llama)));
        route.lastCapacity = slots;
        route.lastCrew = Math.max(1, crew.size());
        setStop(server, route, leaving, now, true, 1);
        ledger.put(leaving);
        // Into the books: what went out, and where to.
        dev.b28.enterprise.merchant.Books.byGood(leaving.cargo).forEach((item, count) -> books.enter(server, route.from, now,
                dev.b28.enterprise.merchant.Books.Kind.SENT, route.to, item, count, 0));
        route.lastDeparture = now;
        route.foodEaten += food;
        route.status = "on the way out";
        ledger.setDirty();
        Enterprise.LOGGER.debug("Caravan {} leaves {} with a squad of {}, {} items, {} animals and a purse of {} spurs",
                route.id, route.from.pos().toShortString(), crew.size(), cargo.stream().mapToInt(ItemStack::getCount).sum(),
                leaving.herd.size(), spurs);
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
        int food = foodNeeded(route.distance()) * Math.max(1, caravan.crew.size());
        if (caravan.leg == Caravan.Leg.OUTBOUND) {
            int residentLevel = Merchant.workingAt(level, route.to.pos(), Lockers.MERCHANT_RANGE)
                    .map(m -> m.getVillagerData().getLevel())
                    .orElse(Warehouse.get(server).level(route.to));
            Warehouse.get(server).recordLevel(route.to, residentLevel);
            // The caravan carries its home's copies of contracts with this town, and collects what it's owed Ex Works.
            var book = dev.b28.enterprise.contract.ContractBook.get(server);
            book.copiesFor(route.from, route.to).forEach(c -> book.deliverCopy(c.id(), route.to));
            // Crates it carried here on freight tickets are delivered into the warehouse, consigned here.
            for (var it = caravan.cargo.iterator(); it.hasNext(); ) {
                ItemStack crate = it.next();
                if (dev.b28.enterprise.market.Freight.ticketed(crate) && Crates.consignedTo(crate, route.to)
                        && stock.hasRoomFor(crate)) {
                    it.remove();
                    stock.insert(crate);
                }
            }
            Crates.receive(server, level, route.to, stock, now);
            if (dev.b28.enterprise.market.Freight.isHub(server, route.to)) {
                caravan.spurs -= dev.b28.enterprise.market.Freight.get(server).land(level, route.to, caravan.spurs, now);
                atHub(server, level, ledger, route, stock, caravan, now);
            }
            if (dev.b28.enterprise.market.Freight.isHub(server, route.from)) {
                buyStrandedAtTown(server, level, route, stock, caravan, rate, now);
                collectForwarding(server, level, route, route.from, stock, caravan, rate, now);
            } else {
                // A run that calls at a hub on its way home takes this town's goods for markets the hub serves.
                var stop = dev.b28.enterprise.market.Freight.stopOn(server, route);
                if (stop.isPresent()) {
                    collectForwarding(server, level, route, stop.get().hub(), stock, caravan, rate, now);
                }
            }
            dev.b28.enterprise.market.Procurement.procure(level, route.to, stock);
            caravan.cargo.addAll(Crates.spareEmpties(stock)); // returnable crates go back
            // Crates here made out to the caravan's home (bought and paid for, waiting to be fetched) go home with it.
            List<ItemStack> consigned = stock.removeLoose(s -> Crates.consignedTo(s, route.from));
            for (ItemStack crate : consigned) {
                if (Crates.stacks(caravan.cargo) + Crates.stacks(List.of(crate)) <= caravan.capacity()) {
                    caravan.cargo.add(crate);
                } else {
                    stock.insert(crate);
                }
            }
            // Recruits it brought to the front join the army there.
            var army = dev.b28.enterprise.army.Army.get(server);
            if (army.front().filter(route.to::equals).isPresent()) {
                caravan.recruits.forEach(recruit -> army.enlist(recruit, GlobalPos.CODEC.parse(net.minecraft.nbt.NbtOps.INSTANCE,
                        recruit.get("enterprise_home")).result().orElse(route.from), now));
                caravan.recruits.clear();
            }
            // It reads the shop here: what the town is after, and at what price, is news it carries home.
            dev.b28.enterprise.market.ShopBoard.get(server).sighted(route.to, rate);
            // The caravan brings its home's board: the Merchant here reads the tenders on it, and takes what pays.
            dev.b28.enterprise.market.Tenders.get(server).read(level, route.to, route.from, now);
            // And its requisitions: the Merchant here bids what its town can supply; a caravan come to the buyer
            // brings its home's Merchant to bid on the buyer's board.
            var requisitions = dev.b28.enterprise.market.Requisitions.get(server);
            requisitions.read(level, route.to, route.from, now);
            requisitions.read(level, route.from, route.to, now);
            settleOwing(server, level, route, stock, caravan, mint, rate, now);
            payAdvances(server, level, route, stock, caravan, mint, rate, now);
            // From a bank's market: it buys the town's bills and lends against its contracts, for coin now.
            dev.b28.enterprise.market.Bank.deal(server, level, route, stock, caravan, mint, rate, now);
            presentBills(server, level, route, stock, caravan, mint, rate, now);
            raiseLevy(server, level, route, territory, caravan, now);
            int earned = settleContracts(server, level, route, stock, caravan, mint, rate, now);
            // Crates made out here that no contract in force took: their papers filed, their goods for sale like any.
            for (java.util.ListIterator<ItemStack> it = caravan.cargo.listIterator(); it.hasNext(); ) {
                ItemStack stack = it.next();
                if (Crates.consignedTo(stack, route.to)) {
                    it.remove();
                    Crates.open(server, route.to, stock, stack).forEach(it::add);
                }
            }
            collectExWorks(server, level, route, stock, caravan, mint, rate, now);
            Map<Item, double[]> sold = new LinkedHashMap<>();
            int takings = sell(server, level, route.to, stock, caravan, residentLevel, mint, rate, now, route, territory, sold);
            // What the town here lacks of the crops a farm town could grow, it buys forward from the caravan's.
            ForwardSales.offer(server, level, route, stock, caravan, mint, rate, now);
            // At a hub: its Merchant deals in rents with the caravan's town.
            dev.b28.enterprise.market.RentDealer.deal(server, level, route.to, route.from, stock, caravan, mint, rate, now);
            // A foreign trader pays the crown's customs on what it sells inside the realm.
            takings -= dev.b28.enterprise.trade.Realms.get(server).customs(server, route.from, takings);
            // Both sides keep books: the Merchant here what it bought, the caravan's log what it sold.
            for (Map.Entry<Item, double[]> line : sold.entrySet()) {
                int count = (int) line.getValue()[0];
                int spursFor = (int) Math.round(line.getValue()[1] * rate);
                dev.b28.enterprise.merchant.Books.get(server).enter(server, route.to, now,
                        dev.b28.enterprise.merchant.Books.Kind.BOUGHT, route.from, line.getKey(), count, spursFor);
                caravan.log.add(new dev.b28.enterprise.merchant.Books.Entry(now / 24000,
                        dev.b28.enterprise.merchant.Books.Kind.SOLD, route.to, line.getKey(), count, spursFor));
            }
            earned += takings;
            caravan.spurs += earned;
            route.salesSpurs += earned;
            // Then the squad's services: deals, droving, the survey and the books, a blessing, help with the harvest...
            int paid = dev.b28.enterprise.service.Services.abroad(level, territory, route, stock, caravan, sold, now,
                    caravan.news);
            caravan.spurs += paid;
            route.serviceSpurs += paid;
            sendPortersHome(ledger, route, caravan, now);
            food = foodNeeded(route.distance()) * Math.max(1, caravan.crew.size());
            int keepForFood = caravan.provisions >= food ? 0 : (int) Math.ceil(food * PURSE_PER_FOOD_POINT * rate);
            buyForHome(level, route, stock, caravan, residentLevel, keepForFood, territory, mint, rate, now);
            buySurplus(server, ledger, level, route, stock, caravan, residentLevel, keepForFood, territory, mint, rate, now);
            buyBackhaul(server, level, route, stock, caravan, keepForFood, territory, rate, now);
        }
        if (caravan.provisions >= food) {
            // It brought its food for the road home.
            caravan.provisions = 0;
            route.foodEaten += food;
            caravan.leg = Caravan.Leg.RETURNING;
            caravan.arrives = now + (long) (travelTicks(route.distance()) / caravan.pace);
            setStop(server, route, caravan, now, false, 1);
            route.status = "on the way home";
            ledger.setDirty();
            return;
        }
        Map<Item, Integer> meal = stock.eat(food);
        if (meal.isEmpty() && buyFoodAtShops(level, route, stock, caravan, food, rate) > 0) {
            meal = stock.eat(food); // bought at the town's shops, eaten from its warehouse like any
        }
        if (meal.isEmpty()) {
            if (caravan.waitingSince < 0) {
                caravan.waitingSince = now;
            }
            if (now - caravan.waitingSince >= MAX_WAIT_TICKS) {
                // It won't sit there for days: home on short rations, foraging, at half the pace.
                caravan.waitingSince = -1;
                caravan.leg = Caravan.Leg.RETURNING;
                caravan.arrives = now + (long) (travelTicks(route.distance()) / caravan.pace * 2);
                setStop(server, route, caravan, now, false, 2);
                route.shortRations++;
                route.status = "on the way home on short rations";
            } else {
                caravan.leg = Caravan.Leg.WAITING_FOR_FOOD;
                caravan.arrives = now; // try again next poll
                route.status = "waiting for food at the destination";
            }
        } else {
            caravan.waitingSince = -1;
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
            caravan.arrives = now + (long) (travelTicks(route.distance()) / caravan.pace);
            setStop(server, route, caravan, now, false, 1);
            route.status = "on the way home";
        }
        ledger.setDirty();
    }

    /**
     * A caravan calling at a freight hub: it collects what its run's freight shop there took; hands the forwarder its
     * crates bound further on (paying the forwarding) and sells it its bills on those markets for coin; leaves the
     * hub copies of contracts for markets the hub passes them on to; and loads the crates booked on its run home.
     */
    static void atHub(MinecraftServer server, ServerLevel level, CaravanLedger ledger, Route route, MerchantStock stock,
            Caravan caravan, long now) {
        var freight = dev.b28.enterprise.market.Freight.get(server);
        var home = new dev.b28.enterprise.market.Freight.Leg(route, false);
        caravan.spurs += freight.collectFares(level, home);
        sellStrandedAtHub(server, level, route.to, route.from, stock, caravan, now);
        int[] purse = {caravan.spurs};
        freight.handOver(level, route.to, stock, caravan.cargo, purse);
        caravan.spurs = purse[0];
        caravan.spurs += freight.discountBills(level, route.to, route.from, stock, caravan.cargo, now);
        freight.forward(level, route.to, stock, now);
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        book.copiesVia(route.from, route.to, party -> !dev.b28.enterprise.market.Freight.legs(server, route.to, party)
                .isEmpty()).forEach(c -> book.deliverCopy(c.id(), route.to));
        caravan.cargo.addAll(freight.loadBooked(route.to, stock, home, caravan.capacity() - Crates.stacks(caravan.cargo)));
    }

    /**
     * A hub's caravan calling at one of its towns collects for its forwarder: the town's goods owed to markets beyond
     * the hub are crated and handed over, the town paying the forwarding (freight and carting, and the margin) from
     * its money as far as it can, and selling the forwarder its bills on those markets for coin from the caravan's
     * purse. At home the forwarder carts them in and books them on.
     */
    static void collectForwarding(MinecraftServer server, ServerLevel level, Route route, GlobalPos hub,
            MerchantStock stock, Caravan caravan, double rate, long now) {
        var freight = dev.b28.enterprise.market.Freight.get(server);
        boolean own = hub.equals(route.from); // the hub's own caravan, or another's that calls there on its way home
        // The carrier's fare to the hub, and the forwarding on from there (freight, carting, the margin): prepaid.
        double perStack = dev.b28.enterprise.market.Freight.forwardingPrice(
                        new dev.b28.enterprise.market.Freight.Leg(route, false)) // home: a backhaul
                + dev.b28.enterprise.market.Freight.cartPrice() * 2 / dev.b28.enterprise.market.Freight.CART
                * (1 + dev.b28.enterprise.market.Freight.FORWARD_MARGIN)
                + (own ? 0 : dev.b28.enterprise.market.Freight.legPrice(
                        new dev.b28.enterprise.market.Freight.Leg(route, false))); // its fare to the hub: on its way home
        double can = Math.max(0, Treasury.available(level, route.to.pos(), stock, rate)
                - Territory.get(server).reserve(route.to));
        int room = Math.min(caravan.capacity() - Crates.stacks(caravan.cargo), (int) (can / perStack));
        List<ItemStack> picked = new ArrayList<>(pickForwarded(server, level, route, route.to, hub, stock,
                now + travelTicks(route.distance()), room, now, c -> true));
        // The hub's own caravan carries more on credit (the town's bill to it, due in a forwarder's term), secured on
        // the goods: those whose stack is worth the fare over the lien's share. Another's wants cash.
        double credit = 0;
        if (own) {
            int more = caravan.capacity() - Crates.stacks(caravan.cargo) - Crates.stacks(picked);
            List<ItemStack> onCredit = pickForwarded(server, level, route, route.to, hub, stock,
                    now + travelTicks(route.distance()), more, now, c -> c.delivery().map(d -> d.unitPrice()
                            * d.item().getDefaultMaxStackSize() * dev.b28.enterprise.Dials.FREIGHT_LIEN.get() >= perStack)
                            .orElse(false));
            credit = Crates.stacks(onCredit.stream().filter(Crates::isCrate).toList()) * perStack;
            picked.addAll(onCredit);
        }
        if (picked.isEmpty()) {
            return;
        }
        List<ItemStack> crates = new ArrayList<>();
        for (ItemStack stack : picked) {
            // The hub's own caravan takes them into its forwarder's hands now; another's hands them over at the hub.
            crates.add(own && Crates.isCrate(stack) ? freight.forwarding(stack) : stack);
        }
        int stacks = Crates.stacks(crates.stream().filter(Crates::isCrate).toList());
        Treasury.Withdrawal paid = Treasury.withdraw(level, route.to.pos(), stock, Math.max(0, stacks * perStack - credit),
                Mint.get(server), now);
        int fee = paid.spurs() + Mint.get(server).sell(Mint.DEFAULT_FACTION, paid.emeralds(), now);
        caravan.spurs += fee;
        if (credit > 0.01) {
            // The rest on the town's bill to the hub, thirty days hence (it rides home in the caravan's papers).
            caravan.cargo.add(Crates.documents(dev.b28.enterprise.cargo.Papers.NONE.with(dev.b28.enterprise.contract.Bills
                    .get(server).draw(route.to, hub, credit, now, -1, dev.b28.enterprise.Dials.FREIGHT_CREDIT_DAYS.ticks()))));
        }
        if (own) {
            freight.forwarded(route.from, stacks, fee);
        }
        // The shipper's bills on the buyers: bought now, from the purse, by the hub's forwarder or by the carrier
        // (who sells them to the forwarder at the hub).
        caravan.spurs -= freight.discountBills(level, hub, route.from, route.to, crates, caravan.spurs, now);
        caravan.cargo.addAll(crates);
    }

    /**
     * A hub's own caravan at a town buys its stranded paper (see {@link dev.b28.enterprise.market.Stranded}), out of
     * the town's warehouse, with coin from its purse beyond its road food and the hub's bill for the rest; the bills
     * and goods ride home to the hub's files and its forwarder.
     */
    static void buyStrandedAtTown(MinecraftServer server, ServerLevel level, Route route, MerchantStock stock,
            Caravan caravan, double rate, long now) {
        GlobalPos hub = route.from;
        GlobalPos town = route.to;
        List<ItemStack> crates = stock.removeLoose(s -> dev.b28.enterprise.market.Stranded.crate(server, hub, town, s));
        List<ItemStack> bundles = stock.removeLoose(s -> s.is(dev.b28.enterprise.cargo.Cargo.DOCUMENTS.get())
                && !dev.b28.enterprise.contract.Bills.ids(s).isEmpty());
        if (crates.isEmpty() && bundles.isEmpty()) {
            return;
        }
        int keep = (int) Math.ceil(foodNeeded(route.distance()) * Math.max(1, caravan.crew.size()) * PURSE_PER_FOOD_POINT
                * rate);
        double means = Territory.get(server).bellOf(hub).map(b -> Territory.get(server).treasury(b)).orElse(0.0)
                - Territory.get(server).reserve(hub) + Territory.get(server).depositsOnCall(hub); // at home, last counted
        var deal = dev.b28.enterprise.market.Stranded.buy(server, hub, town, crates, bundles,
                Math.max(0, caravan.spurs - keep) / rate, Math.max(0, means), rate, now);
        crates.forEach(stock::insert);
        bundles.stream().filter(b -> !b.getOrDefault(dev.b28.enterprise.cargo.Cargo.PAPERS.get(),
                dev.b28.enterprise.cargo.Papers.NONE).documents().isEmpty()).forEach(stock::insert);
        deal.ifPresent(d -> {
            caravan.spurs -= d.spurs();
            Treasury.deposit(level, town.pos(), stock, d.spurs());
            d.hubBills().forEach(doc -> dev.b28.enterprise.contract.Bills.file(stock, doc));
            caravan.cargo.addAll(d.crates());
            caravan.cargo.addAll(d.bills());
            Enterprise.LOGGER.info("{} buys {}'s stranded paper: {} emeralds of bills and {} crates, {} spurs in coin",
                    dev.b28.enterprise.settlement.Names.label(hub), dev.b28.enterprise.settlement.Names.label(town),
                    Math.round(d.face()), d.crates().size(), d.spurs());
        });
    }

    /**
     * A town's caravan at a hub sells it the stranded paper it brought: coin from the hub's treasury beyond its
     * reserve, and the hub's bill for the rest (carried home); the bills go in the hub's files, the crates to its
     * forwarder. What the hub won't buy rides home again.
     */
    static void sellStrandedAtHub(MinecraftServer server, ServerLevel level, GlobalPos hub, GlobalPos town,
            MerchantStock stock, Caravan caravan, long now) {
        List<ItemStack> crates = new ArrayList<>();
        List<ItemStack> bundles = new ArrayList<>();
        for (var it = caravan.cargo.iterator(); it.hasNext(); ) {
            ItemStack stack = it.next();
            if (dev.b28.enterprise.market.Stranded.crate(server, hub, town, stack)) {
                crates.add(stack);
                it.remove();
            } else if (stack.is(dev.b28.enterprise.cargo.Cargo.DOCUMENTS.get())
                    && dev.b28.enterprise.contract.Bills.get(server).in(stack).stream()
                            .anyMatch(b -> dev.b28.enterprise.market.Stranded.bill(server, hub, town, b, now))) {
                bundles.add(stack);
                it.remove();
            }
        }
        if (crates.isEmpty() && bundles.isEmpty()) {
            return;
        }
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        Territory territory = Territory.get(server);
        double cash = Math.max(0, Treasury.available(level, hub.pos(), stock, rate) - territory.reserve(hub));
        // Its households' savings count too: deposits the Merchant may lend on such paper.
        double spare = cash + territory.depositsOnCall(hub);
        var deal = dev.b28.enterprise.market.Stranded.buy(server, hub, town, crates, bundles, spare, spare, rate, now);
        caravan.cargo.addAll(crates);
        bundles.stream().filter(b -> !b.getOrDefault(dev.b28.enterprise.cargo.Cargo.PAPERS.get(),
                dev.b28.enterprise.cargo.Papers.NONE).documents().isEmpty()).forEach(caravan.cargo::add);
        deal.ifPresent(d -> {
            if (d.spurs() / rate > cash) {
                territory.borrowDeposits(level, hub, stock, d.spurs() / rate - cash);
            }
            Treasury.Withdrawal paid = Treasury.withdraw(level, hub.pos(), stock, d.spurs() / rate, Mint.get(server), now);
            caravan.spurs += paid.spurs() + Mint.get(server).sell(Mint.DEFAULT_FACTION, paid.emeralds(), now);
            if (!d.hubBills().isEmpty()) {
                caravan.cargo.add(Crates.documents(new dev.b28.enterprise.cargo.Papers(d.hubBills())));
            }
            for (ItemStack bundle : d.bills()) {
                bundle.getOrDefault(dev.b28.enterprise.cargo.Cargo.PAPERS.get(), dev.b28.enterprise.cargo.Papers.NONE)
                        .documents().forEach(doc -> dev.b28.enterprise.contract.Bills.file(stock, doc));
            }
            d.crates().forEach(stock::insert);
            Enterprise.LOGGER.info("{} buys the stranded paper {} brought: {} emeralds of bills and {} crates, {} spurs in coin",
                    dev.b28.enterprise.settlement.Names.label(hub), dev.b28.enterprise.settlement.Names.label(town),
                    Math.round(d.face()), d.crates().size(), d.spurs());
        });
    }

    /** How far ahead a shipper sends goods on through a hub: the road beyond it takes time. */
    public static final long FORWARD_LEAD = 3 * 24000L;

    /**
     * Goods owed under contracts to a market beyond the destination, where the destination is a freight hub with a
     * carrier's run there: crated, consigned to the buyer, with a bill of lading marked paid by bill, and a bill of
     * exchange drawn on the buyer for them (the forwarder at the hub may buy it). Returns the crates and bills.
     */
    static List<ItemStack> pickForwarded(MinecraftServer server, ServerLevel level, Route route, MerchantStock stock,
            long arrival, int slots, long now) {
        return pickForwarded(server, level, route, route.from, route.to, stock, arrival, slots, now);
    }

    /** As above, for goods {@code shipper} sends on through {@code hub}, on a caravan of {@code route}. */
    static List<ItemStack> pickForwarded(MinecraftServer server, ServerLevel level, Route route, GlobalPos shipper,
            GlobalPos hub, MerchantStock stock, long arrival, int slots, long now) {
        return pickForwarded(server, level, route, shipper, hub, stock, arrival, slots, now, c -> true);
    }

    /** As above, only for the contracts {@code ok} passes (those the carrier will take on credit, say). */
    static List<ItemStack> pickForwarded(MinecraftServer server, ServerLevel level, Route route, GlobalPos shipper,
            GlobalPos hub, MerchantStock stock, long arrival, int slots, long now,
            java.util.function.Predicate<dev.b28.enterprise.contract.Contract> ok) {
        List<ItemStack> cargo = new ArrayList<>();
        if (!dev.b28.enterprise.market.Freight.isHub(server, hub) || slots <= 0) {
            return cargo;
        }
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        for (var contract : book.of(shipper)) {
            var delivery = contract.delivery();
            if (delivery.isEmpty() || !contract.inForce() || !contract.seller().equals(shipper)
                    || contract.buyer().equals(hub) || !contract.incoterm().sellerCarries
                    || dev.b28.enterprise.market.Freight.legs(server, hub, contract.buyer()).isEmpty() || !ok.test(contract)) {
                continue;
            }
            int owed = contract.owed(arrival + FORWARD_LEAD) - contract.shipped();
            int room = (slots - Crates.stacks(cargo)) * delivery.get().item().getDefaultMaxStackSize();
            if (owed <= 0 || room <= 0) {
                continue;
            }
            List<ItemStack> goods = new ArrayList<>(stock.take(delivery.get().cost(server.registryAccess()), Math.min(owed, room)));
            int taken = goods.stream().mapToInt(ItemStack::getCount).sum();
            if (taken <= 0) {
                continue;
            }
            var papers = Crates.shipping(contract, shipper, contract.buyer(), goods, route.id, now, false);
            papers.first(dev.b28.enterprise.cargo.Papers.BILL_OF_LADING).ifPresent(d -> d.data().putBoolean("billed", true));
            cargo.addAll(Crates.pack(level, shipper, stock, goods, papers, now));
            double worth = taken * delivery.get().unitPrice();
            cargo.add(Crates.documents(dev.b28.enterprise.cargo.Papers.NONE.with(
                    dev.b28.enterprise.contract.Bills.get(server).draw(contract.buyer(), shipper, worth, now))));
            book.ship(contract, taken, now);
        }
        return cargo;
    }

    /**
     * A leg that passes a hub calls there: the caravan's arrival is set to the hub (so far along the road), and the rest
     * of the road kept for after. {@code slow}: the leg's pace slowed (short rations).
     */
    static void setStop(MinecraftServer server, Route route, Caravan caravan, long now, boolean outbound, double slow) {
        caravan.stop = null;
        caravan.stopped = false;
        var stop = dev.b28.enterprise.market.Freight.stopOn(server, route);
        if (stop.isEmpty()) {
            return;
        }
        double along = (double) stop.get().index() / (route.path().size() - 1);
        double share = outbound ? along : 1 - along;
        long leg = (long) (travelTicks(route.distance()) / caravan.pace * slow);
        caravan.stop = stop.get().hub();
        caravan.arrives = now + (long) (leg * share);
        caravan.remaining = leg - (long) (leg * share);
    }

    /**
     * Calling at a hub on the way: the caravan buys food there for the road if its own may fall short, collects what
     * its run's freight shop at the hub took, hands the forwarder crates bound for markets the hub serves (and sells it
     * its bills on them), loads crates booked on its run, carries contract copies the hub passes on, reads the hub's
     * board, and goes on.
     */
    static void callAtStop(MinecraftServer server, CaravanLedger ledger, Territory territory, Route route, Caravan caravan,
            long now) {
        ServerLevel level = server.getLevel(caravan.stop.dimension());
        if (level == null || !ensureLoaded(level, caravan.stop.pos())) {
            return; // waits a poll for the hub to load
        }
        GlobalPos hub = caravan.stop;
        boolean outbound = caravan.leg == Caravan.Leg.OUTBOUND;
        GlobalPos bound = outbound ? route.to : route.from;
        MerchantStock stock = MerchantStock.at(level, hub.pos());
        territory.touch(level, hub.pos(), stock);
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        var freight = dev.b28.enterprise.market.Freight.get(server);
        var leg = new dev.b28.enterprise.market.Freight.Leg(route, outbound, hub);
        caravan.spurs += freight.collectFares(level, leg);
        caravan.spurs -= freight.land(level, hub, caravan.spurs, now); // the hub's landing fee: it stops here
        dev.b28.enterprise.market.RentDealer.deal(server, level, hub, route.from, stock, caravan, Mint.get(server),
                rate, now); // and it deals in rents with the caravan's town
        sellStrandedAtHub(server, level, hub, route.from, stock, caravan, now); // and buys its stranded paper
        // Food: outbound, for the road home too where the far end may not feed it; it buys here, where it can.
        if (outbound && caravan.provisions == 0) {
            int food = foodNeeded(route.distance()) * Math.max(1, caravan.crew.size());
            if (foodKnownAt(server, level, route.to) - ledger.foodPromisedAt(route.to) < food
                    && stock.foodNutrition() >= food + Warehouse.FOOD_PAR_POINTS) {
                Map<Item, Integer> meal = stock.eat(food);
                double cost = meal.entrySet().stream().mapToDouble(e -> e.getValue() * Prices.emeraldsEach(e.getKey())).sum();
                int price = Math.min(caravan.spurs, (int) Math.ceil(cost * rate));
                caravan.spurs -= price;
                Treasury.deposit(level, hub.pos(), stock, price);
                caravan.provisions = food;
            }
        }
        // Freight: crates bound for markets the hub serves (not where this caravan goes) are handed to its forwarder.
        List<ItemStack> elsewhere = new ArrayList<>(caravan.cargo.stream()
                .filter(s -> Crates.isCrate(s) && Crates.papers(s).consignee().filter(c -> !c.equals(bound)
                        && !c.equals(route.from) && !c.equals(route.to)).isPresent())
                .toList());
        if (!elsewhere.isEmpty()) {
            caravan.cargo.removeAll(elsewhere);
            int[] purse = {caravan.spurs};
            freight.handOver(level, hub, stock, elsewhere, purse);
            caravan.spurs = purse[0];
            caravan.cargo.addAll(elsewhere); // what it couldn't hand over rides on
        }
        caravan.spurs += freight.discountBills(level, hub, route.from, stock, caravan.cargo, now);
        freight.forward(level, hub, stock, now);
        caravan.cargo.addAll(freight.loadBooked(hub, stock, leg, caravan.capacity() - Crates.stacks(caravan.cargo)));
        // Papers the hub holds for either end ride on; the caravan's for markets the hub serves are left with it.
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        book.copiesFor(hub, bound).forEach(c -> book.deliverCopy(c.id(), bound));
        book.copiesVia(outbound ? route.from : route.to, hub, party -> !dev.b28.enterprise.market.Freight
                .legs(server, hub, party).isEmpty()).forEach(c -> book.deliverCopy(c.id(), hub));
        dev.b28.enterprise.market.Requisitions.get(server).read(level, route.from, hub, now);
        Atlas.get(server).learnBoth(route.from, hub, now);
        caravan.stopped = true;
        caravan.arrives = now + caravan.remaining;
        route.stops++;
        route.status = (outbound ? "on the way out" : "on the way home") + ", called at "
                + dev.b28.enterprise.settlement.Names.of(server, hub);
        ledger.setDirty();
    }

    /** A caravan waits this long at the far end for food before it sets off home on short rations. */
    public static final long MAX_WAIT_TICKS = 12000;

    /**
     * The far end's warehouse can't feed the squad home: the travelling Merchant buys food at the shops round its desk
     * (players' and the town's own), lot by lot, from its purse, into that warehouse to be eaten. Returns points bought.
     */
    static int buyFoodAtShops(ServerLevel level, Route route, MerchantStock stock, Caravan caravan, int food, double rate) {
        int bought = 0;
        for (var shop : dev.b28.enterprise.market.Shops.near(level, route.to.pos())) {
            ItemStack lot = shop.getFilterItem();
            var kind = lot.get(net.minecraft.core.component.DataComponents.FOOD);
            if (shop.getMode() != dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity.Mode.SELL
                    || kind == null || kind.nutrition() <= 0) {
                continue;
            }
            while (bought < food && dev.b28.enterprise.market.Shops.lots(shop) > 0 && caravan.spurs >= shop.getTotalPrice()
                    && stock.hasRoomFor(lot)) {
                ItemStack goods = dev.b28.enterprise.market.Shops.buyFrom(shop);
                if (goods.isEmpty()) {
                    break;
                }
                caravan.spurs -= shop.getTotalPrice();
                stock.insert(goods);
                bought += goods.getCount() * kind.nutrition();
            }
        }
        route.foodFromShops += bought;
        return bought;
    }

    /**
     * The porters have delivered: they walk home on their own (as long as a caravan's leg takes), unless what's left
     * aboard needs their backs for the trip home.
     */
    static void sendPortersHome(CaravanLedger ledger, Route route, Caravan caravan, long now) {
        List<CompoundTag> porters = caravan.crew.stream().filter(m -> Squad.porter(Squad.trade(m))).toList();
        if (porters.isEmpty() || caravan.crew.size() == porters.size()
                || Crates.stacks(caravan.cargo) > caravan.capacity() - Squad.PACK_STACKS * porters.size()) {
            return;
        }
        caravan.crew.removeAll(porters);
        porters.forEach(porter -> ledger.walkHome(route.from, porter, now + travelTicks(route.distance())));
        route.portersSent += porters.size();
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
        // Home's books: what came back, and the caravan's log of its dealings there, handed in.
        var books = dev.b28.enterprise.merchant.Books.get(server);
        long home = level.getGameTime();
        dev.b28.enterprise.merchant.Books.byGood(caravan.cargo).forEach((item, count) -> books.enter(server, route.from, home,
                dev.b28.enterprise.merchant.Books.Kind.HOME, route.to, item, count, 0));
        books.file(server, route.from, home, caravan.log);
        caravan.log.clear();
        for (Iterator<ItemStack> it = caravan.cargo.iterator(); it.hasNext(); ) {
            ItemStack rest = stock.insert(it.next());
            if (rest.isEmpty()) {
                it.remove();
            }
        }
        Treasury.deposit(level, route.from.pos(), stock, caravan.spurs);
        Crates.receive(server, level, route.from, stock, level.getGameTime()); // what it collected, crated
        if (dev.b28.enterprise.market.Freight.isHub(server, route.from)) {
            // What it collected to forward: carted into the hub's warehouse and booked on.
            dev.b28.enterprise.market.Freight.get(server).forward(level, route.from, stock, level.getGameTime());
        }
        // And the destination's board: the Merchant at home reads its tenders.
        dev.b28.enterprise.market.Tenders.get(server).read(level, route.from, route.to, level.getGameTime());
        // The contracts drafted on the way ride home: now both parties hold a copy.
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        caravan.drafts.forEach(id -> book.deliverCopy(id, route.from));
        book.copiesFor(route.to, route.from).forEach(c -> book.deliverCopy(c.id(), route.from));
        // Towns whose land it crossed heard of both its ends; home hears what the squad learned.
        Atlas atlas = Atlas.get(server);
        caravan.news.forEach(market -> atlas.learn(route.from, market, level.getGameTime()));
        for (Hex hex : route.path()) {
            territory.owner(route.from.dimension(), hex).flatMap(territory::desk).ifPresent(town -> {
                atlas.learn(town, route.from, level.getGameTime());
                atlas.learn(town, route.to, level.getGameTime());
            });
        }
        // The squad comes home, with any animals that didn't sell.
        var bell = territory.bellOf(route.from);
        for (CompoundTag member : caravan.crew) {
            Squad.release(level, route.from.pos(), member);
            bell.ifPresent(b -> dev.b28.enterprise.settlement.Settlements.get(server).setAway(b, member.getUUID("UUID"), false));
        }
        caravan.herd.forEach(animal -> Squad.release(level, route.from.pos(), animal));
        caravan.pack.forEach(llama -> Squad.release(level, route.from.pos(), llama));
        // The levy's recruits wait here, in the capital's barracks, for a caravan to the front.
        caravan.recruits.forEach(recruit -> {
            if (!recruit.contains("enterprise_home")) {
                GlobalPos.CODEC.encodeStart(net.minecraft.nbt.NbtOps.INSTANCE, route.to)
                        .ifSuccess(t -> recruit.put("enterprise_home", t));
            }
            dev.b28.enterprise.army.Army.get(server).muster(recruit);
        });
        caravan.recruits.clear();
        route.spursCollected += caravan.spurs;
        route.trips++;
        route.status = "home";
        ledger.remove(route.id);
        ledger.setDirty();
        Enterprise.LOGGER.debug("Caravan {} home with {} spurs", route.id, caravan.spurs);
    }

    /** A standing order runs this many installments, or as many as the seller can cover, but at least the minimum. */
    public static final int CONTRACT_INSTALLMENTS = 10;
    public static final int CONTRACT_MIN_INSTALLMENTS = 3;

    /** A good is worth a standing order once a caravan has sold at least this many of it in one visit. */
    public static final int CONTRACT_MIN_ITEMS = 16;

    /**
     * Goods owed under contracts in force from this desk to the destination by the time the caravan arrives, taken
     * from the stock first (within {@code slots}). Records how many ride for each contract.
     */
    static List<ItemStack> pickContracted(MinecraftServer server, ServerLevel level, Route route, MerchantStock stock,
            long arrival, int slots, Map<Integer, Integer> contracted, long now) {
        List<ItemStack> cargo = new ArrayList<>();
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        for (var contract : book.inForce(route.from, route.to)) {
            var delivery = contract.delivery();
            int owed = contract.owed(arrival) - contract.shipped();
            if (delivery.isEmpty() || owed <= 0 || !contract.incoterm().sellerCarries) {
                continue; // Ex Works: the buyer's caravan collects it
            }
            int room = (slots - Crates.stacks(cargo)) * delivery.get().item().getDefaultMaxStackSize();
            int taken = 0;
            // Its crates earmarked for it first, as they are; then loose goods for the rest.
            for (ItemStack crate : ForwardSales.takeEarmarked(stock, contract.id(), Math.min(owed, room))) {
                taken += ForwardSales.goodsIn(crate);
                cargo.add(crate);
            }
            List<ItemStack> goods = new ArrayList<>();
            for (ItemStack piece : stock.take(delivery.get().cost(server.registryAccess()),
                    Math.max(0, Math.min(owed, room) - taken))) {
                taken += piece.getCount();
                goods.add(piece);
            }
            if (taken > 0) {
                // Crated with its bill of lading and a copy of the contract.
                cargo.addAll(Crates.pack(level, route.from, stock, goods,
                        Crates.shipping(contract, route.from, route.to, goods, route.id, now, false), now));
                contracted.put(contract.id(), taken);
                book.ship(contract, taken, arrival); // on the road: the buyer's, where it bears the road
            }
        }
        return cargo;
    }

    /**
     * At the destination, before any other selling: goods carried under contracts are delivered, as far as owed and
     * as the warehouse has room, and paid for at the locked price, whatever the market says now. Returns spurs earned.
     */
    static int settleContracts(MinecraftServer server, ServerLevel level, Route route, MerchantStock stock,
            Caravan caravan, Mint mint, double rate, long now) {
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        int earned = 0;
        for (Map.Entry<Integer, Integer> carried : caravan.contracted.entrySet()) {
            var contract = book.contract(carried.getKey()).orElse(null);
            if (contract == null || !contract.inForce() || contract.delivery().isEmpty()) {
                continue;
            }
            var delivery = contract.delivery().get();
            int toDeliver = Math.min(carried.getValue(), contract.owed(now));
            book.arrived(contract, carried.getValue());
            int delivered = 0;
            double worth = 0; // what it's owed, bonuses and all
            // Its crates first: into store as they are, their papers filed; one with more than is owed is opened.
            for (java.util.ListIterator<ItemStack> it = caravan.cargo.listIterator(); it.hasNext() && delivered < toDeliver; ) {
                ItemStack crate = it.next();
                if (!Crates.consignedTo(crate, route.to)
                        || Crates.papers(crate).contract().filter(id -> id == contract.id()).isEmpty()) {
                    continue;
                }
                List<ItemStack> goods = Crates.goods(crate);
                int count = goods.stream().filter(g -> delivery.matches(g, server.registryAccess()))
                        .mapToInt(ItemStack::getCount).sum();
                if (count == goods.stream().mapToInt(ItemStack::getCount).sum() && count <= toDeliver - delivered) {
                    if (!stock.insert(Crates.store(server, route.to, crate)).isEmpty()) {
                        break; // no room even for a crate: the rest rides home
                    }
                    it.remove();
                    delivered += count;
                    worth += goods.stream().mapToDouble(g -> contract.priceOf(g) * g.getCount()).sum();
                } else {
                    it.remove();
                    Crates.open(server, route.to, stock, crate).forEach(it::add);
                }
            }
            for (Iterator<ItemStack> it = caravan.cargo.iterator(); it.hasNext() && delivered < toDeliver; ) {
                ItemStack stack = it.next();
                if (Crates.isCrate(stack) || !delivery.matches(stack, server.registryAccess())) {
                    continue;
                }
                double each = contract.priceOf(stack);
                ItemStack part = stack.split(Math.min(stack.getCount(), toDeliver - delivered));
                while (!part.isEmpty() && !stock.hasRoomFor(part)) {
                    // Keep the warehouse's free slots for locker results and coins: deliver less.
                    int less = Math.min(part.getCount(), part.getMaxStackSize());
                    stack.grow(less);
                    part.shrink(less);
                }
                ItemStack rest = stock.insert(part);
                delivered += part.getCount() - rest.getCount();
                worth += (part.getCount() - rest.getCount()) * each;
                stack.grow(rest.getCount());
                if (stack.isEmpty()) {
                    it.remove();
                }
                if (!rest.isEmpty()) {
                    break; // the warehouse is full: the rest rides home
                }
            }
            if (delivered == 0) {
                continue;
            }
            // What was paid up front is worked off: its share of this delivery's price.
            double recoup = contract.recoupFrom(worth, delivered);
            book.recouped(contract, recoup);
            // Where the buyer pays the tolls, it pays on top what the posts will take of the payment on its way home.
            double owedMoney = worth - recoup;
            if (!contract.incoterm().sellerPaysTolls) {
                double share = tollShare(Territory.get(server), route);
                owedMoney = share >= 1 ? owedMoney : owedMoney / (1 - share);
            }
            Treasury.Withdrawal paid = Treasury.withdraw(level, route.to.pos(), stock, owedMoney, mint, now);
            double value = paid.value(rate);
            earned += paid.spurs() + mint.sell(Mint.DEFAULT_FACTION, paid.emeralds(), now);
            if (value < owedMoney * 0.99 && (book.trust(route.from, route.to, now) >= dev.b28.enterprise.contract.ContractBook.MIN_TRUST
                    || dev.b28.enterprise.market.Sovereign.get(server).isCrown(route.to))) {
                // Short of coin, the buyer pays the rest with a bill on its own treasury: it rides home to the seller.
                // The crown's paper (its capital's, its army's) is good whatever its record.
                caravan.cargo.add(Crates.documents(dev.b28.enterprise.cargo.Papers.NONE.with(
                        dev.b28.enterprise.contract.Bills.get(server).draw(route.to, route.from, owedMoney - value, now))));
                value = owedMoney;
            }
            book.settle(contract, delivered, value + recoup, now);
            if (value < owedMoney * 0.9) {
                book.unpaid(contract, now);
            }
            route.itemsDelivered += delivered;
        }
        caravan.contracted.clear();
        return earned;
    }

    /**
     * Goods this caravan's town buys here Ex Works: the caravan takes what's owed from the seller's warehouse, as far
     * as it holds them and the caravan has room and money, and pays the locked price into the seller's treasury.
     */
    static void collectExWorks(MinecraftServer server, ServerLevel level, Route route, MerchantStock stock,
            Caravan caravan, Mint mint, double rate, long now) {
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        for (var contract : book.inForce(route.to, route.from)) {
            var delivery = contract.delivery();
            if (delivery.isEmpty() || contract.incoterm().sellerCarries) {
                continue;
            }
            int owed = contract.owed(now);
            int room = (caravan.capacity() - Crates.stacks(caravan.cargo)) * delivery.get().item().getDefaultMaxStackSize();
            int affordable = (int) Math.floor(caravan.spurs / Math.max(1e-9, delivery.get().unitPrice() * rate));
            // Short of coin, the buyer's Merchant pays the rest with a bill drawn on its own treasury, where trusted.
            boolean credit = book.trust(route.from, route.to, now) >= dev.b28.enterprise.contract.ContractBook.MIN_TRUST;
            int quantity = Math.min(owed, credit ? room : Math.min(room, affordable));
            if (quantity <= 0) {
                continue;
            }
            int taken = 0;
            List<ItemStack> goods = new ArrayList<>();
            // What the seller crated for it first (opened, to be crated again with the Ex Works papers), then loose.
            for (ItemStack crate : ForwardSales.takeEarmarked(stock, contract.id(), quantity)) {
                for (ItemStack piece : Crates.goods(crate)) {
                    taken += piece.getCount();
                    goods.add(piece);
                }
                stock.insert(new ItemStack(dev.b28.enterprise.cargo.Cargo.CRATE_ITEM.get()));
            }
            for (ItemStack piece : stock.take(delivery.get().cost(server.registryAccess()), Math.max(0, quantity - taken))) {
                taken += piece.getCount();
                goods.add(piece);
            }
            if (taken == 0) {
                continue;
            }
            // The seller crates it with its bill of lading, made out to the buyer: delivered here, Ex Works.
            caravan.cargo.addAll(Crates.pack(level, route.to, stock, goods,
                    Crates.shipping(contract, route.to, route.from, goods, route.id, now, true), now));
            double worth = goods.stream().mapToDouble(g -> contract.priceOf(g) * g.getCount()).sum(); // with bonuses
            double recoup = contract.recoupFrom(worth, taken); // what was paid up front is worked off
            book.recouped(contract, recoup);
            int cost = (int) Math.ceil((worth - recoup) * rate);
            // A loan from this caravan's bank on the contract comes out of the payment first.
            double repaid = dev.b28.enterprise.market.Bank.get(server).repayFrom(route.from, contract, cost / rate);
            int spurs = Math.min(caravan.spurs, cost - (int) Math.floor(repaid * rate));
            caravan.spurs -= spurs;
            Treasury.deposit(level, route.to.pos(), stock, spurs);
            double onPaper = (cost - spurs) / rate - repaid;
            if (onPaper > 0.01) {
                dev.b28.enterprise.contract.Bills.file(stock, dev.b28.enterprise.contract.Bills.get(server)
                        .draw(route.from, route.to, onPaper, now));
            }
            book.settle(contract, taken, cost / rate + recoup, now);
            route.itemsBought += taken;
            route.purchaseSpurs += spurs;
            boughtThere(server, route, caravan, goods.get(0).getItem(), taken, spurs + (int) Math.round(repaid * rate), now);
        }
    }

    /**
     * Money owed between the two ends for goods delivered without a caravan (a player carried the crate): a caravan
     * from the seller collects it from the buyer and carries it home; one from the buyer carries it to the seller.
     */
    static void settleOwing(MinecraftServer server, ServerLevel level, Route route, MerchantStock stock, Caravan caravan,
            Mint mint, double rate, long now) {
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        for (var contract : book.owing(route.from, route.to)) { // the buyer here pays the caravan's town
            Treasury.Withdrawal paid = Treasury.withdraw(level, route.to.pos(), stock, contract.owing(), mint, now);
            double value = paid.value(rate);
            caravan.spurs += paid.spurs() + mint.sell(Mint.DEFAULT_FACTION, paid.emeralds(), now);
            if (value > 0) {
                book.paidOwing(contract, value);
            }
        }
        for (var contract : book.owing(route.to, route.from)) { // the caravan's town pays the seller here
            double repaid = dev.b28.enterprise.market.Bank.get(server).repayFrom(route.from, contract, contract.owing());
            if (repaid > 0) {
                book.paidOwing(contract, repaid); // a loan on it, from this caravan's bank, comes out first
            }
            int spurs = Math.min(caravan.spurs, (int) Math.ceil(contract.owing() * rate));
            caravan.spurs -= spurs;
            Treasury.deposit(level, route.to.pos(), stock, spurs);
            if (spurs > 0) {
                book.paidOwing(contract, spurs / rate);
            }
        }
        // Advances not worked off when a contract broke are paid back, as far as the seller can spare.
        for (var contract : book.refunds(route.to, route.from)) { // the seller here pays the caravan's town
            double can = Math.max(0, Treasury.available(level, route.to.pos(), stock, rate)
                    - Territory.get(server).reserve(route.to));
            if (can > 0.01) {
                Treasury.Withdrawal paid = Treasury.withdraw(level, route.to.pos(), stock, Math.min(can, contract.refund()),
                        mint, now);
                caravan.spurs += paid.spurs() + mint.sell(Mint.DEFAULT_FACTION, paid.emeralds(), now);
                book.refunded(contract, paid.value(rate));
            }
        }
        for (var contract : book.refunds(route.from, route.to)) { // the caravan's town pays back the buyer here
            int spurs = Math.min(caravan.spurs, (int) Math.ceil(contract.refund() * rate));
            caravan.spurs -= spurs;
            Treasury.deposit(level, route.to.pos(), stock, spurs);
            if (spurs > 0) {
                book.refunded(contract, spurs / rate);
            }
        }
    }

    /**
     * Money before the goods: advances the caravan's town owes the town here under contracts whose copies it has
     * brought are paid from its purse (keeping its road food), the rest with a bill on its home where trusted; and
     * advances the town here owes the caravan's town are paid into the purse, from what it can spare beyond its
     * reserve.
     */
    static void payAdvances(MinecraftServer server, ServerLevel level, Route route, MerchantStock stock, Caravan caravan,
            Mint mint, double rate, long now) {
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        int keep = (int) Math.ceil(foodNeeded(route.distance()) * Math.max(1, caravan.crew.size()) * PURSE_PER_FOOD_POINT
                * rate);
        for (var contract : book.advancesDue(route.to, route.from)) { // the caravan's town buys from the town here
            if (!contract.knownBy(route.to)) {
                continue;
            }
            double due = contract.advanceDue();
            int spurs = Math.max(0, Math.min(caravan.spurs - keep, (int) Math.ceil(due * rate)));
            caravan.spurs -= spurs;
            Treasury.deposit(level, route.to.pos(), stock, spurs);
            double paid = spurs / rate;
            if (due - paid > 0.01 && book.trust(route.from, route.to, now) >= dev.b28.enterprise.contract.ContractBook.MIN_TRUST) {
                dev.b28.enterprise.contract.Bills.file(stock, dev.b28.enterprise.contract.Bills.get(server)
                        .draw(route.from, route.to, due - paid, now)); // a promise to pay the rest
                paid = due;
            }
            if (paid > 0.01) {
                book.advanced(contract, paid, now);
                route.advanceSpurs += spurs;
            }
        }
        for (var contract : book.advancesDue(route.from, route.to)) { // the town here buys from the caravan's town
            if (!contract.knownBy(route.to)) {
                continue;
            }
            double can = Math.max(0, Treasury.available(level, route.to.pos(), stock, rate)
                    - Territory.get(server).reserve(route.to));
            double pay = Math.min(can, contract.advanceDue());
            if (pay > 0.01) {
                Treasury.Withdrawal taken = Treasury.withdraw(level, route.to.pos(), stock, pay, mint, now);
                caravan.spurs += taken.spurs() + mint.sell(Mint.DEFAULT_FACTION, taken.emeralds(), now);
                book.advanced(contract, taken.value(rate), now);
            }
        }
    }

    /**
     * A levy on this town, its copy now here: it hands its recruits to the caravan, its idle hands first, then those
     * of a trade it has others of, then the least skilled of the rest; never a child, nor the Merchant at its desk. They go to the army at the caravan's
     * home. Raised, a soldier counts as delivered under the levy.
     */
    static void raiseLevy(MinecraftServer server, ServerLevel level, Route route, Territory territory, Caravan caravan,
            long now) {
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        if (dev.b28.enterprise.merchant.OffMap.get(server).is(route.to)) {
            hireMercenaries(server, level, route, caravan, now);
            return;
        }
        var bell = territory.bellOf(route.to);
        if (bell.isEmpty()) {
            return;
        }
        var roster = dev.b28.enterprise.settlement.Settlements.get(server).roster(bell.get());
        for (var contract : book.of(route.to)) {
            var levy = contract.levy();
            if (levy.isEmpty() || !contract.inForce() || !contract.seller().equals(route.to)
                    || !contract.buyer().equals(route.from)) {
                continue;
            }
            int wanted = levy.get().soldiers() - contract.delivered();
            if (wanted <= 0) {
                continue;
            }
            java.util.Set<String> needed = new java.util.HashSet<>(
                    dev.b28.enterprise.market.Apprentices.neededTrades(server, route.to));
            dev.b28.enterprise.market.Sovereign.TRAINED.forEach(t -> needed.add(t.trade()));
            java.util.Map<String, Long> trades = roster.values().stream().filter(r -> !r.baby() && !r.away())
                    .collect(java.util.stream.Collectors.groupingBy(r -> r.profession(), java.util.stream.Collectors.counting()));
            List<net.minecraft.world.entity.npc.Villager> able = level.getEntitiesOfClass(
                            net.minecraft.world.entity.npc.Villager.class, new net.minecraft.world.phys.AABB(route.to.pos()).inflate(48),
                            v -> !v.isBaby() && roster.containsKey(v.getUUID()) && !roster.get(v.getUUID()).away()
                                    && v.getBrain().getMemory(net.minecraft.world.entity.ai.memory.MemoryModuleType.JOB_SITE)
                                            .filter(route.to::equals).isEmpty())
                    .stream()
                    // A levy is no choice: idle hands first, then a trade it has others of, then whoever is least
                    // missed; last, the only one of a trade its contracts need or the crown wants trained.
                    .sorted(Comparator.comparingInt((net.minecraft.world.entity.npc.Villager v) ->
                                    dev.b28.enterprise.territory.Labour.idle(Squad.trade(v)) ? 0
                                            : trades.getOrDefault(Squad.trade(v), 0L) > 1 ? 1
                                            : needed.contains(Squad.trade(v)) ? 3 : 2)
                            .thenComparingInt(v -> v.getVillagerData().getLevel()))
                    .limit(wanted).toList();
            for (var recruit : able) {
                trades.merge(Squad.trade(recruit), -1L, Long::sum);
                dev.b28.enterprise.settlement.Settlements.get(server).setAway(bell.get(), recruit.getUUID(), true);
                CompoundTag tag = Squad.stow(recruit);
                GlobalPos.CODEC.encodeStart(net.minecraft.nbt.NbtOps.INSTANCE, route.to).ifSuccess(t -> tag.put("enterprise_home", t));
                caravan.recruits.add(tag);
            }
            if (!able.isEmpty()) {
                book.settle(contract, able.size(), 0, now);
            }
        }
    }

    /**
     * Mercenaries hired at an off-map market (one across the sea): as many as its contracts with the caravan's home
     * still owe, ready to go (the market has them to hire), their home that market: they go back there when they're
     * done, or desert.
     */
    static void hireMercenaries(MinecraftServer server, ServerLevel level, Route route, Caravan caravan, long now) {
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        for (var contract : book.of(route.to)) {
            var levy = contract.levy();
            if (levy.isEmpty() || !contract.inForce() || !contract.seller().equals(route.to)
                    || !contract.buyer().equals(route.from)) {
                continue;
            }
            int wanted = levy.get().soldiers() - contract.delivered();
            for (int i = 0; i < wanted; i++) {
                var mercenary = net.minecraft.world.entity.EntityType.VILLAGER.create(level);
                if (mercenary == null) {
                    break;
                }
                mercenary.setCustomName(net.minecraft.network.chat.Component.literal("Mercenary"));
                CompoundTag tag = Squad.stow(mercenary);
                GlobalPos.CODEC.encodeStart(net.minecraft.nbt.NbtOps.INSTANCE, route.to)
                        .ifSuccess(t -> tag.put("enterprise_home", t));
                caravan.recruits.add(tag);
            }
            if (wanted > 0) {
                book.settle(contract, wanted, 0, now);
            }
        }
    }

    /**
     * Bills of exchange between the two ends: those drawn on the caravan's home that lie here are presented to it, and
     * it pays what's due from its purse; those it carries drawn on this town are presented here, and this town pays
     * what's due into the caravan, to carry home. Bills not yet due, or not paid, go back where they came from.
     */
    static void presentBills(MinecraftServer server, ServerLevel level, Route route, MerchantStock stock, Caravan caravan,
            Mint mint, double rate, long now) {
        var bills = dev.b28.enterprise.contract.Bills.get(server);
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        var here = dev.b28.enterprise.contract.Bills.takeBillsOn(stock, route.from);
        if (!here.isEmpty()) {
            var presented = bills.present(here, caravan.spurs / rate, now, book);
            int spurs = (int) Math.ceil(presented.paid() * rate);
            caravan.spurs -= Math.min(caravan.spurs, spurs);
            Treasury.deposit(level, route.to.pos(), stock, spurs);
            presented.returned().forEach(stock::insert);
            route.billSpurs += spurs;
        }
        List<ItemStack> carried = new ArrayList<>();
        for (Iterator<ItemStack> it = caravan.cargo.iterator(); it.hasNext(); ) {
            ItemStack stack = it.next();
            if (stack.is(dev.b28.enterprise.cargo.Cargo.DOCUMENTS.get())
                    && !dev.b28.enterprise.contract.Bills.ids(stack).isEmpty()
                    && stack.getOrDefault(dev.b28.enterprise.cargo.Cargo.PAPERS.get(), dev.b28.enterprise.cargo.Papers.NONE)
                            .first(dev.b28.enterprise.cargo.Papers.BILL_OF_EXCHANGE)
                            .flatMap(d -> dev.b28.enterprise.cargo.Papers.pos(d.data(), "drawee"))
                            .filter(route.to::equals).isPresent()) {
                carried.add(stack);
                it.remove();
            }
        }
        if (!carried.isEmpty()) {
            Territory territory = Territory.get(server);
            double cash = Math.max(0, Treasury.available(level, route.to.pos(), stock, rate) - territory.reserve(route.to));
            // Short of coin, the Merchant honours its bills with its households' savings, as far as it may borrow.
            double can = cash + territory.depositsOnCall(route.to);
            var presented = bills.present(carried, can, now, book);
            if (presented.paid() > cash) {
                territory.borrowDeposits(level, route.to, stock, presented.paid() - cash);
            }
            if (presented.paid() > 0) {
                Treasury.Withdrawal paid = Treasury.withdraw(level, route.to.pos(), stock, presented.paid(), mint, now);
                int spurs = paid.spurs() + mint.sell(Mint.DEFAULT_FACTION, paid.emeralds(), now);
                caravan.spurs += spurs;
                route.billSpurs += spurs;
            }
            caravan.cargo.addAll(presented.returned());
        }
    }

    /** What this town's caravan to {@code seller} must carry to pay for what it collects there Ex Works, in emeralds. */
    static double exWorksPurse(MinecraftServer server, Route route, long arrival) {
        double emeralds = 0;
        for (var contract : dev.b28.enterprise.contract.ContractBook.get(server).owing(route.to, route.from)) {
            emeralds += contract.owing(); // and what it owes there for goods a player brought
        }
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        for (var contract : book.inForce(route.to, route.from)) {
            if (!contract.incoterm().sellerCarries && contract.delivery().isPresent()) {
                double worth = contract.owed(arrival) * contract.delivery().get().unitPrice();
                emeralds += worth - contract.recoupFrom(worth, 0);
            }
        }
        for (var contract : book.advancesDue(route.to, route.from)) {
            emeralds += contract.advanceDue(); // and what it pays up front, with the copies it carries
        }
        for (var contract : book.refunds(route.from, route.to)) {
            emeralds += contract.refund(); // and advances it owes back there
        }
        return emeralds;
    }

    /**
     * A travelling Merchant who sold a good well here drafts a standing order with the resident Merchant: the
     * destination takes, each round trip, what its lockers actually use in one (not more than it bought this visit;
     * in sixteens), ten times, at this visit's price. The resident won't promise to take more than it sells. Needs a
     * Merchant in the squad and trust between the two; one contract per good.
     */
    static void draftContracts(MinecraftServer server, Route route, Caravan caravan, Map<Item, double[]> sold, long now) {
        if (!Squad.has(caravan.crew, "enterprise:merchant")) {
            return;
        }
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        if (book.trust(route.from, route.to, now) < dev.b28.enterprise.contract.ContractBook.MIN_TRUST) {
            return;
        }
        long every = Math.max(DEPARTURE_INTERVAL, 2 * travelTicks(route.distance()));
        Warehouse warehouse = Warehouse.get(server);
        sold.forEach((item, tally) -> {
            double used = warehouse.perDay(route.to, item, now) * every / 24000.0;
            int quantity = (int) Math.min(tally[0], Math.round(used / 16.0) * 16.0) / 16 * 16;
            if (quantity < CONTRACT_MIN_ITEMS || book.covers(route.from, route.to, item)) {
                return;
            }
            // And the travelling Merchant won't promise more than it can see: what's still aboard, and what its home
            // warehouse held when it left.
            int aboard = Crates.contents(caravan.cargo).stream().filter(stack -> stack.is(item)).mapToInt(ItemStack::getCount).sum();
            int installments = Math.min(CONTRACT_INSTALLMENTS, (warehouse.onHand(route.from, item) + aboard) / quantity);
            if (installments < CONTRACT_MIN_INSTALLMENTS) {
                return;
            }
            var contract = book.draftDelivery(route.from, route.to, item, quantity, tally[1] / tally[0], now + every,
                    every, installments, now, route.to);
            caravan.drafts.add(contract.id());
        });
    }

    /** Does the destination's last known stock have any of the comforts on the shopping list? */
    static boolean hasShoppingAt(MinecraftServer server, GlobalPos desk, Map<String, Double> shopping) {
        Warehouse warehouse = Warehouse.get(server);
        return shopping.keySet().stream().anyMatch(kind -> Comforts.kind(kind)
                .map(tag -> java.util.stream.StreamSupport.stream(BuiltInRegistries.ITEM.getTagOrEmpty(tag).spliterator(), false)
                        .anyMatch(holder -> warehouse.onHand(desk, holder.value()) > 0))
                .orElse(false));
    }

    /**
     * The caravan's own town's shopping: comforts the destination holds beyond its own par, cheapest first, up to the
     * worth on the list, at a price bargained up to their full value (this is for home, not for resale). Keeps
     * {@code keepForFood} spurs and stays within its cargo space.
     */
    private static void buyForHome(ServerLevel level, Route route, MerchantStock stock, Caravan caravan,
            int residentLevel, int keepForFood, Territory territory, Mint mint, double rate, long now) {
        if (caravan.shopping.isEmpty() || !dev.b28.enterprise.merchant.OffMap.get(level.getServer()).sells(route.to)) {
            return;
        }
        Map<String, Double> theirs = Comforts.onHand(stock);
        double theirPar = territory.comfortPar(route.to);
        for (Map.Entry<String, Double> want : caravan.shopping.entrySet()) {
            var kind = Comforts.kind(want.getKey()).orElseThrow();
            double spareWorth = theirs.getOrDefault(want.getKey(), 0.0) - theirPar;
            double budget = Math.min(want.getValue(), spareWorth);
            List<Item> goods = stock.totals().keySet().stream()
                    .map(id -> BuiltInRegistries.ITEM.get(ResourceLocation.parse(id)))
                    .filter(item -> item.builtInRegistryHolder().is(kind))
                    .sorted(Comparator.comparingDouble(Prices::emeraldsEach))
                    .toList();
            for (Item item : goods) {
                if (budget <= 0 || Crates.stacks(caravan.cargo) >= caravan.capacity()) {
                    break;
                }
                double value = Prices.emeraldsEach(item);
                double each = Bargain.between(Bargain.sellerFloor(value), value, residentLevel, caravan.merchantLevel);
                int quantity = (int) Math.min(Math.ceil(budget / value), Math.floor((caravan.spurs - keepForFood) / (each * rate)));
                if (quantity <= 0) {
                    continue;
                }
                int taken = 0;
                for (ItemStack piece : stock.take(new ItemCost(item, 1), quantity)) {
                    taken += piece.getCount();
                    caravan.cargo.add(piece);
                }
                int spurs = (int) Math.ceil(taken * each * rate);
                caravan.spurs -= spurs;
                Treasury.deposit(level, route.to.pos(), stock, spurs);
                budget -= taken * value;
                route.itemsBought += taken;
                route.purchaseSpurs += spurs;
                boughtThere(server(level), route, caravan, item, taken, spurs, now);
            }
        }
        caravan.shopping.clear();
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
            int toll = posted ? Math.min(carried, (int) Math.floor(carried * territory.tollRate(owner)
                    + territory.passageFee(owner))) : 0;
            if (toll > 0) {
                territory.oweToll(owner, toll);
                paid += toll;
            }
        }
        return paid;
    }

    /** The share of coins carried on this route's path that its toll posts would take: for terms where the buyer pays tolls. */
    /** The handling fees a caravan on this route pays at the posts on its path, in spurs a trip. */
    public static double passageFees(Territory territory, Route route) {
        var dimension = route.from.dimension();
        Set<GlobalPos> ends = ends(territory, route);
        Set<GlobalPos> posted = new LinkedHashSet<>();
        for (Hex hex : route.path()) {
            territory.postOwner(dimension, hex).filter(owner -> !ends.contains(owner)).ifPresent(posted::add);
        }
        return posted.stream().mapToDouble(territory::passageFee).sum();
    }

    public static double tollShare(Territory territory, Route route) {
        var dimension = route.from.dimension();
        Set<GlobalPos> ends = ends(territory, route);
        Set<GlobalPos> posted = new LinkedHashSet<>();
        for (Hex hex : route.path()) {
            territory.postOwner(dimension, hex).filter(owner -> !ends.contains(owner)).ifPresent(posted::add);
        }
        double kept = 1;
        for (GlobalPos owner : posted) {
            kept *= 1 - territory.tollRate(owner);
        }
        return 1 - kept;
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

    /** How hard each hex on the route's side of the world is to cross ({@link Terrain}); a bridged hex is open ground. */
    static ToDoubleFunction<Hex> terrain(MinecraftServer server, Route route) {
        return terrain(server, route.from.dimension());
    }

    public static ToDoubleFunction<Hex> terrain(MinecraftServer server, net.minecraft.resources.ResourceKey<net.minecraft.world.level.Level> dimension) {
        ServerLevel level = server.getLevel(dimension);
        if (level == null) {
            return RoutePlanner.FLAT;
        }
        Terrain terrain = Terrain.get(server);
        Works works = Works.get(server);
        // A bridge carries caravans over water, not over a cliff: a bridged hex is open ground if it's mostly water.
        return hex -> works.bridged(dimension, hex) && terrain.waterShare(level, hex) > Terrain.IMPASSABLE
                ? 1 : terrain.crossingFactor(level, hex);
    }

    /**
     * What a bridge on {@code hex} would bring its town a day: on each route it would carry (not the town's own), the
     * toll the town could ask there, the lesser of the most it may ask and 80% of what the bridge saves over the
     * cheapest way without it, the town's own tolls left out (a route with no way at all would save its whole trade),
     * one trip a day. A second bridge beside the first is worth only the shorter way it opens. Also the neighbours the busiest route would cross
     * between, for the deck.
     */
    public record BridgeValue(Hex hex, double tollsPerDay, Hex from, Hex to) {}

    public static BridgeValue bridgeValue(MinecraftServer server, Territory territory, GlobalPos town, Hex hex, double rate) {
        return bridgeValue(CaravanLedger.get(server).routes(), territory, town, hex, rate, terrain(server, town.dimension()));
    }

    /** As {@link #bridgeValue(MinecraftServer, Territory, GlobalPos, Hex, double)}, over these routes and terrain. */
    public static BridgeValue bridgeValue(java.util.Collection<Route> routes, Territory territory, GlobalPos town, Hex hex,
            double rate, ToDoubleFunction<Hex> now) {
        double tolls = 0;
        double best = 0;
        Hex from = null;
        Hex to = null;
        for (Route route : routes) {
            var dimension = route.from.dimension();
            Set<GlobalPos> ends = ends(territory, route);
            if (!dimension.equals(town.dimension()) || ends.contains(town)) {
                continue;
            }
            Hex start = Hex.at(route.from.pos());
            Hex goal = Hex.at(route.to.pos());
            if (hex.equals(start) || hex.equals(goal)) {
                continue;
            }
            ToDoubleFunction<Hex> bridged = h -> h.equals(hex) ? 1 : now.applyAsDouble(h);
            double value = route.expectedValue();
            // The town's own posts count for nothing: a bridge that lets caravans dodge its other posts gains it nothing.
            Set<GlobalPos> free = new LinkedHashSet<>(ends);
            free.add(town);
            RoutePlanner.Plan without = RoutePlanner.plan(territory, dimension, start, goal, free, perHex(rate), now, value, Set.of());
            RoutePlanner.Plan with = RoutePlanner.plan(territory, dimension, start, goal, free, perHex(rate), bridged, value, Set.of());
            int at = with.path().indexOf(hex);
            if (at <= 0 || at >= with.path().size() - 1) {
                continue; // the bridge wouldn't be on this route's way
            }
            double saves = without.found() ? without.cost() - with.cost() : value;
            double toll = Math.min(Territory.MAX_TOLL_RATE * value, Chokepoints.DETOUR_SHARE * saves);
            tolls += Math.max(0, toll);
            if (toll > best) {
                best = toll;
                from = with.path().get(at - 1);
                to = with.path().get(at + 1);
            }
        }
        return new BridgeValue(hex, tolls, from, to);
    }

    /**
     * The cheapest hex path for the next caravan: a hex costs its road food both ways (at bread prices) times how hard
     * it is to cross, and each town's toll post costs its toll on what caravans on this route usually carry home.
     */
    public static RoutePlanner.Plan plan(MinecraftServer server, Territory territory, Route route, double rate) {
        return plan(server, territory, route, rate, false);
    }

    /** As {@link #plan(MinecraftServer, Territory, Route, double)}; {@code boat}: a Fisherman can sail the water. */
    public static RoutePlanner.Plan plan(MinecraftServer server, Territory territory, Route route, double rate, boolean boat) {
        ToDoubleFunction<Hex> land = terrain(server, route);
        ToDoubleFunction<Hex> ground = boat ? hex -> {
            double hard = land.applyAsDouble(hex);
            return Double.isInfinite(hard) ? BOAT_FACTOR : hard;
        } : land;
        return RoutePlanner.plan(territory, route.from.dimension(), Hex.at(route.from.pos()), Hex.at(route.to.pos()),
                ends(territory, route), perHex(rate), ground, route.expectedValue(), Set.of());
    }

    /** Food points of one of this item as rations (0 for comforts and non-food). */
    static int foodPoints(ItemStack stack) {
        var food = stack.get(net.minecraft.core.component.DataComponents.FOOD);
        return food == null || Comforts.isComfort(stack) ? 0 : food.nutrition();
    }

    /** Sailing a water hex costs this much a hex of open ground: boats are quick. */
    public static final double BOAT_FACTOR = 0.8;

    /** Hexes on a path a caravan can't walk: water, unless bridged. */
    static int waterHexes(MinecraftServer server, net.minecraft.resources.ResourceKey<net.minecraft.world.level.Level> dimension,
            List<Hex> path) {
        ToDoubleFunction<Hex> land = terrain(server, dimension);
        return (int) path.stream().filter(hex -> Double.isInfinite(land.applyAsDouble(hex))).count();
    }

    /** Stacks of the goods a destination wants that the stock holds. */
    static int wantedStacks(MerchantStock stock, Set<Item> wanted) {
        return wanted.stream().mapToInt(item -> (stock.count(new ItemCost(item, 1)) + 63) / 64).sum();
    }

    /**
     * What the destination will want of each kind of comfort when a caravan leaving now arrives (at {@code arrival}),
     * in emeralds' worth: its par less what it held at its last stocktake, less what its villagers will have used
     * since (none taken yet: the whole par). The stocktake is taken just after a caravan sells, when the shelves are
     * fullest; without the using up, every other caravan would bring nothing.
     */
    static Map<String, Double> comfortWants(MinecraftServer server, Territory territory, GlobalPos desk, long arrival) {
        Warehouse warehouse = Warehouse.get(server);
        double par = territory.comfortPar(desk);
        long taken = warehouse.stocktakeTime(desk);
        double days = taken < 0 ? 0 : Math.max(0, (arrival - taken) / 24000.0);
        double used = territory.villagers(territory.bellOf(desk).orElse(desk)) * Comforts.RATION_EMERALDS * days;
        Map<String, Double> wants = new LinkedHashMap<>();
        for (var kind : Comforts.kinds()) {
            double held = 0;
            for (var holder : BuiltInRegistries.ITEM.getTagOrEmpty(kind)) {
                held += warehouse.onHand(desk, holder.value()) * Prices.emeraldsEach(holder.value());
            }
            held = Math.max(0, held - used);
            if (par - held > 0) {
                wants.put(Comforts.name(kind), par - held);
            }
        }
        return wants;
    }

    public static Map<String, Double> comfortWantsForTest(MinecraftServer server, Territory territory, GlobalPos desk,
            long arrival) {
        return comfortWants(server, territory, desk, arrival);
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

    /**
     * Items the destination's work takes: first what it needs to make what it has contracted to sell, then what its
     * switched-on trades take, read from the trade keys ("costA+costB>result").
     */
    public static Set<Item> wantedAt(MinecraftServer server, GlobalPos desk) {
        return new LinkedHashSet<>(wantsAt(server, desk).keySet());
    }

    /** As {@link #wantedAt}, each good with why the market wants it (the first reason found). */
    public static Map<Item, String> wantsAt(MinecraftServer server, GlobalPos desk) {
        MerchantBook book = MerchantBook.get(server);
        Map<Item, String> items = new LinkedHashMap<>();
        Warehouse.get(server).inputs(desk).keySet().forEach(item -> items.putIfAbsent(item, "an input to goods it's contracted to sell"));
        // A foreign realm wants what its people can't make.
        dev.b28.enterprise.trade.Realms.get(server).of(desk).ifPresent(realm ->
                realm.wants().forEach(item -> items.putIfAbsent(item, "the realm's people use it")));
        // What the town's own workshop turns into its goods: the inputs of each conversion a resident's trade can do.
        Territory.get(server).bellOf(desk).ifPresent(bell -> {
            Set<String> trades = new java.util.HashSet<>();
            dev.b28.enterprise.settlement.Settlements.get(server).roster(bell).values()
                    .forEach(r -> trades.add(r.profession()));
            for (var conversion : dev.b28.enterprise.territory.Workshop.CONVERSIONS) {
                if (conversion.trades().stream().anyMatch(trades::contains)) {
                    String why = "its workshop makes " + BuiltInRegistries.ITEM.getKey(conversion.output()).getPath() + " of it";
                    items.putIfAbsent(conversion.input(), why);
                    if (conversion.input2() != null) {
                        items.putIfAbsent(conversion.input2(), why);
                    }
                }
            }
        });
        // Stationery, where it draws up papers enough to keep some on hand.
        if (dev.b28.enterprise.cargo.Paperwork.get(server).busy(desk, server.overworld().getGameTime())) {
            List.of(Items.PAPER, Items.INK_SAC, Items.FEATHER).forEach(item -> items.putIfAbsent(item, "stationery for its papers"));
        }
        // Axes for the hands working its land, while some lack a tool.
        Territory territory = Territory.get(server);
        territory.bellOf(desk).ifPresent(bell -> {
            int hands = dev.b28.enterprise.territory.Labour.hands(
                    dev.b28.enterprise.settlement.Settlements.get(server).roster(bell)).size();
            if (territory.handTools(bell) < hands) {
                items.putIfAbsent(Items.IRON_AXE, "tools for its hands");
            }
        });
        for (BlockPos workstation : book.workstations(desk)) {
            for (String key : book.enabledTrades(desk, workstation)) {
                String costs = key.substring(0, key.indexOf('>'));
                for (String id : costs.split("\\+")) {
                    if (!id.isEmpty() && !id.equals("minecraft:emerald")) {
                        Item item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(id));
                        if (item != Items.AIR) {
                            items.putIfAbsent(item, "a trade it has switched on takes it");
                        }
                    }
                }
            }
        }
        return items;
    }

    /** What a caravan should bring a desk: what its lockers trade, and what players' shops there have ordered. */
    static Set<Item> wantedThere(MinecraftServer server, GlobalPos desk) {
        return new LinkedHashSet<>(wantsThere(server, desk).keySet());
    }

    /** As {@link #wantedThere}, each good with why. */
    static Map<Item, String> wantsThere(MinecraftServer server, GlobalPos desk) {
        Map<Item, String> items = wantsAt(server, desk);
        dev.b28.enterprise.market.Exchange.get(server).wanted(desk).keySet()
                .forEach(item -> items.putIfAbsent(item, "a player's shop there ordered it"));
        return items;
    }

    /**
     * Goods packed for sale: each good of two stacks or more in crates of its own (the shipper's empties first, else new
     * ones it pays for), the rest loose.
     */
    static List<ItemStack> lots(ServerLevel level, GlobalPos shipper, MerchantStock stock, List<ItemStack> goods, long now) {
        Map<String, List<ItemStack>> byGood = new LinkedHashMap<>();
        for (ItemStack piece : goods) {
            byGood.computeIfAbsent(BuiltInRegistries.ITEM.getKey(piece.getItem()) + "|" + piece.getComponentsPatch(),
                    k -> new ArrayList<>()).add(piece);
        }
        List<ItemStack> packed = new ArrayList<>();
        for (List<ItemStack> good : byGood.values()) {
            if (good.size() >= 2 && good.get(0).getItem().canFitInsideContainerItems()) {
                packed.addAll(Crates.pack(level, shipper, stock, good, dev.b28.enterprise.cargo.Papers.NONE, now));
            } else {
                packed.addAll(good);
            }
        }
        return packed;
    }

    /**
     * Up to {@code slots} stacks, shared across the wanted items in order, each up to {@link #WANT_PER_ITEM}; with
     * more wanted items than slots, a stack each of the first.
     */
    /**
     * As {@link #pickCargo(MerchantStock, Set, int)}, but of each good no more than the far end will take: its par less
     * what it held when a caravan last saw its stock. A Merchant doesn't haul what it can't sell. Its reasons, good by
     * good, go in {@code home}'s books.
     */
    static List<ItemStack> pickCargo(MinecraftServer server, GlobalPos home, GlobalPos there, MerchantStock stock,
            Map<Item, String> wanted, int slots, long now) {
        Warehouse warehouse = Warehouse.get(server);
        var books = dev.b28.enterprise.merchant.Books.get(server);
        Map<Item, Integer> limits = new LinkedHashMap<>();
        for (Map.Entry<Item, String> want : wanted.entrySet()) {
            Item item = want.getKey();
            int room = warehouse.par(there, item, true, now) - warehouse.onHand(there, item);
            if (room <= 0) {
                books.why(home, now, there, item, "not sent: they hold their fill (" + want.getValue() + ")",
                        warehouse.onHand(there, item));
            } else if (stock.count(new ItemCost(item, 1)) <= 0) {
                books.why(home, now, there, item, "not sent: we have none (" + want.getValue() + ")", room);
            } else if (stock.count(new ItemCost(item, 1)) <= ForwardSales.setAside(server, home, item, stock, now)) {
                books.why(home, now, there, item, "not sent: set aside for a forward sale (" + want.getValue() + ")", room);
            } else {
                limits.put(item, Math.min(room, stock.count(new ItemCost(item, 1))
                        - ForwardSales.setAside(server, home, item, stock, now)));
            }
        }
        List<ItemStack> cargo = new ArrayList<>();
        if (limits.isEmpty() || slots <= 0) {
            return cargo;
        }
        int perItem = Math.min(WANT_PER_ITEM, Math.max(1, slots / limits.size()) * 64);
        for (Map.Entry<Item, Integer> want : limits.entrySet()) {
            Item item = want.getKey();
            int room = (slots - Crates.stacks(cargo)) * item.getDefaultMaxStackSize();
            if (room <= 0) {
                books.why(home, now, there, item, "not sent: no room aboard", want.getValue());
                continue;
            }
            int taken = 0;
            for (ItemStack piece : stock.take(new ItemCost(item, 1), Math.min(Math.min(perItem, want.getValue()), room))) {
                taken += piece.getCount();
                cargo.add(piece);
            }
            books.why(home, now, there, item, "sent: " + wanted.get(item), taken);
        }
        return cargo;
    }

    public static List<ItemStack> pickCargoForTest(MinecraftServer server, GlobalPos home, GlobalPos there,
            MerchantStock stock, Map<Item, String> wanted, int slots, long now) {
        return pickCargo(server, home, there, stock, wanted, slots, now);
    }

    static List<ItemStack> pickCargo(MerchantStock stock, Set<Item> wanted, int slots) {
        List<ItemStack> cargo = new ArrayList<>();
        if (wanted.isEmpty() || slots <= 0) {
            return cargo;
        }
        int perItem = Math.min(WANT_PER_ITEM, Math.max(1, slots / wanted.size()) * 64);
        for (Item item : wanted) {
            int room = (slots - Crates.stacks(cargo)) * item.getDefaultMaxStackSize();
            if (room <= 0) {
                break;
            }
            for (ItemStack piece : stock.take(new ItemCost(item, 1), Math.min(perItem, room))) {
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
            Caravan caravan, int residentLevel, Mint mint, double rate, long now, Route route, Territory territory,
            Map<Item, double[]> tallies) {
        Warehouse warehouse = Warehouse.get(server);
        Set<Item> wanted = wantedAt(server, desk);
        boolean foreign = dev.b28.enterprise.trade.Realms.get(server).isForeign(desk);
        int earned = 0;
        for (java.util.ListIterator<ItemStack> it = caravan.cargo.listIterator(); it.hasNext(); ) {
            ItemStack entry = it.next();
            // A crate of one good for sale is sold as its goods: whole if wanted whole, kept crated.
            java.util.Optional<ItemStack> lot = Crates.lot(entry);
            if (Crates.isCrate(entry) && lot.isEmpty()) {
                continue; // someone's shipment, or empties
            }
            ItemStack stack = lot.orElse(entry);
            Item item = stack.getItem();
            int onHand = stock.count(new ItemCost(item, 1));
            double value = Prices.emeraldsEach(item);
            // A realm's people want some comforts (the Coast, books) as a country does: by its par, like any good.
            var comfort = foreign ? java.util.Optional.<net.minecraft.tags.TagKey<Item>>empty() : Comforts.kindOf(item);
            int par;
            int ownPar = Integer.MAX_VALUE;
            java.util.Optional<Desires.Outlet> outlet = java.util.Optional.empty();
            double money = Treasury.available(level, desk.pos(), stock, rate);
            if (comfort.isPresent()) {
                // Comforts: up to the kind's par in worth, paid from money the town doesn't need for wages and guards.
                double held = Comforts.onHand(stock).getOrDefault(Comforts.name(comfort.get()), 0.0);
                // Whole items: a town short of a kind buys one even if it costs more than the gap (a pair of boots).
                par = onHand + (int) Math.max(0, Math.ceil((territory.comfortPar(desk) - held) / value));
                ownPar = par;
                // And a Merchant supplying other towns buys for them, where it can sell on at a margin after the road.
                outlet = Desires.outlet(server, desk, route.from, item, now);
                if (outlet.isPresent() && Desires.worthBuying(value, outlet.get())) {
                    par += outlet.get().units();
                }
                money -= territory.reserve(desk);
            } else {
                money -= territory.payroll(desk); // never the farmers' wages or the guards' pay
                par = Math.min(warehouse.par(desk, item, wanted.contains(item), now), roomFor(stock, wanted.size(), item));
                int nutrition = foodPoints(stack);
                if (nutrition > 0) {
                    // Food: a town short of it (under twice the caravans' par) buys enough to get there.
                    par = Math.max(par, onHand + Math.max(0, 2 * Warehouse.FOOD_PAR_POINTS - stock.foodNutrition()) / nutrition);
                }
            }
            int quantity = Math.min(stack.getCount(), par - onHand);
            // Why it won't take the rest, for the caravan's log: the last thing that cut what it takes.
            String why = quantity >= stack.getCount() ? null
                    : comfort.isPresent() ? "unsold: they hold their fill of " + Comforts.name(comfort.get())
                    : wanted.contains(item) || foodPoints(stack) > 0 ? "unsold: they hold their fill"
                    : "unsold: they've no use for it";
            double each = -1;
            for (int pass = 0; pass < 2 && quantity > 0; pass++) {
                // Price at the middle of the batch: the resident's shortage shrinks as the shelf fills.
                double midShortage = Bargain.shortage(onHand + quantity / 2, par);
                each = Bargain.price(value, midShortage, caravan.merchantLevel, residentLevel);
                int afford = each <= 0 ? 0 : (int) Math.floor(money / each);
                if (afford < quantity) {
                    why = "unsold: they can't pay";
                }
                quantity = Math.min(quantity, afford);
            }
            if (outlet.isPresent() && each > 0 && !Desires.worthBuying(each, outlet.get())
                    && ownPar - onHand < quantity) {
                quantity = Math.max(0, ownPar - onHand); // too dear to sell on: only its own
                why = "unsold: too dear for them to sell on";
            }
            if (quantity <= 0) {
                logUnsold(caravan, desk, item, why, stack.getCount(), now);
                continue;
            }
            int delivered;
            if (lot.isPresent() && quantity >= stack.getCount() && stock.insert(entry.copy()).isEmpty()) {
                delivered = stack.getCount(); // the whole crate, into store as it is
                it.remove();
            } else {
                int wanted0 = quantity;
                while (quantity > 0 && !stock.hasRoomFor(stack.copyWithCount(quantity))) {
                    quantity -= Math.min(quantity, item.getDefaultMaxStackSize()); // keep the warehouse's free slots
                }
                if (quantity < wanted0) {
                    why = "unsold: no room in their warehouse";
                }
                if (quantity <= 0) {
                    logUnsold(caravan, desk, item, why, stack.getCount(), now);
                    continue;
                }
                if (lot.isPresent()) {
                    // Part of a crate: what's sold comes out loose, the rest stays crated aboard.
                    delivered = 0;
                    for (ItemStack piece : Crates.split(entry, quantity, it)) {
                        ItemStack rest = stock.insert(piece);
                        delivered += piece.getCount() - rest.getCount();
                    }
                } else {
                    ItemStack sold = stack.split(quantity);
                    ItemStack rest = stock.insert(sold);
                    delivered = sold.getCount() - rest.getCount();
                    stack.grow(rest.getCount());
                }
            }
            Treasury.Withdrawal paid = Treasury.withdraw(level, desk.pos(), stock, delivered * each, mint, now);
            earned += paid.spurs() + mint.sell(Mint.DEFAULT_FACTION, paid.emeralds(), now);
            route.itemsDelivered += delivered;
            logUnsold(caravan, desk, item, why, lot.isPresent() ? stack.getCount() - delivered : stack.getCount(), now);
            double[] tally = tallies.computeIfAbsent(item, i -> new double[2]);
            tally[0] += delivered;
            tally[1] += delivered * each;
            if (lot.isEmpty() && stack.isEmpty()) {
                it.remove();
            }
        }
        return earned;
    }

    /** Why the far end wouldn't take the rest of a good, in the caravan's log: its home's Merchant reads it there. */
    private static void logUnsold(Caravan caravan, GlobalPos desk, Item item, String why, int left, long now) {
        if (why != null && left > 0) {
            caravan.log.add(dev.b28.enterprise.merchant.Books.Entry.why(now, desk, item, why, left));
        }
    }

    /**
     * Food points a caravan's home knows the far end holds: an off-map market's ledger, or a town's last stocktake (and
     * plenty, if it has none yet).
     */
    static int foodKnownAt(MinecraftServer server, ServerLevel level, GlobalPos desk) {
        if (dev.b28.enterprise.merchant.OffMap.get(server).is(desk)) {
            return MerchantStock.at(level, desk.pos()).foodNutrition();
        }
        if (Warehouse.get(server).stocktakeTime(desk) < 0) {
            return Integer.MAX_VALUE;
        }
        int points = 0;
        for (Map.Entry<String, Integer> held : Warehouse.get(server).stocktake(desk).entrySet()) {
            var food = BuiltInRegistries.ITEM.get(ResourceLocation.parse(held.getKey())).getDefaultInstance()
                    .get(net.minecraft.core.component.DataComponents.FOOD);
            points += food == null ? 0 : food.nutrition() * held.getValue();
        }
        return points;
    }

    /** Whether a caravan with no cargo would still do business: the network wants goods this town can spare. */
    /** Does a levy (or a hire) there still owe the caravan's home soldiers, to be fetched? */
    static boolean recruitsOwedAt(MinecraftServer server, Route route) {
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        return book.of(route.to).stream().anyMatch(c -> c.inForce() && c.seller().equals(route.to)
                && c.buyer().equals(route.from) && c.levy().filter(l -> l.soldiers() > c.delivered()).isPresent());
    }

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
        // And what players' shops at home have ordered: the caravan buys it up on its way.
        dev.b28.enterprise.market.Exchange.get(server).wanted(route.from).forEach((item, count) ->
                wants.merge(item, count, Integer::sum));
        wants.replaceAll((item, want) -> want - warehouse.onHand(route.from, item)
                - Crates.contents(caravan.cargo).stream().filter(stack -> stack.is(item)).mapToInt(ItemStack::getCount).sum());
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
    /** A Fisherman's catch at sea brings an ink sac (a squid) for every this many fish. */
    public static final int INK_PER_CATCH = 4;

    /** A glut is held beyond this many times the par (or a stack, where it has no par). */
    public static final double GLUT = 2;
    /** An overstocked market sells its glut this much under its worth for each par it holds beyond, at most half. */
    public static final double GLUT_DISCOUNT = 0.25;
    public static final double MOST_GLUT_DISCOUNT = 0.5;

    /**
     * A backhaul: the caravan goes home anyway, so what it carries back costs it nothing more, and any margin is
     * profit. With the space left after the rest, it buys what this market holds a glut of, at the glut's discount
     * (the seller would otherwise pay to store it, or throw it out), wherever home has a use for it: an input its
     * lockers or crafts take, food while its own runs short, a comfort its people are short of. Home sells it on, or
     * uses it, at its worth.
     */
    private static void buyBackhaul(MinecraftServer server, ServerLevel level, Route route, MerchantStock stock,
            Caravan caravan, int keepForFood, Territory territory, double rate, long now) {
        GlobalPos desk = route.to;
        if (!dev.b28.enterprise.merchant.OffMap.get(server).sells(desk)) {
            return;
        }
        Warehouse warehouse = Warehouse.get(server);
        Set<Item> wantedHere = wantedAt(server, desk);
        Set<Item> usedAtHome = wantedAt(server, route.from);
        int homeFood = foodKnownAt(server, level, route.from);
        Set<String> shortAtHome = territory.bellOf(route.from).map(bell -> territory.comfortsMet(bell).entrySet().stream()
                .filter(e -> e.getValue() < 1).map(Map.Entry::getKey).collect(java.util.stream.Collectors.toSet()))
                .orElse(Set.of());
        int bought = 0;
        for (Map.Entry<String, Integer> held : stock.totals().entrySet()) {
            int room = caravan.capacity() - Crates.stacks(caravan.cargo);
            if (room <= 0) {
                break;
            }
            Item item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(held.getKey()));
            if (item == Items.EMERALD || item instanceof CoinItem || held.getKey().endsWith("shulker_box")
                    || item == dev.b28.enterprise.cargo.Cargo.CRATE_ITEM.get() || item == dev.b28.enterprise.cargo.Cargo.DOCUMENTS.get()) {
                continue;
            }
            var food = item.getDefaultInstance().get(net.minecraft.core.component.DataComponents.FOOD);
            boolean isFood = food != null && food.nutrition() > 0;
            ItemStack sample = item.getDefaultInstance();
            boolean useAtHome = usedAtHome.contains(item)
                    || isFood && homeFood < 2 * Warehouse.FOOD_PAR_POINTS
                    || Comforts.isComfort(sample) && Comforts.kindOf(item).map(Comforts::name).filter(shortAtHome::contains)
                            .isPresent();
            if (!useAtHome) {
                continue;
            }
            int par = Math.max(item.getDefaultMaxStackSize(), warehouse.par(desk, item, wantedHere.contains(item), now));
            if (isFood) {
                par = Math.max(par, 2 * Warehouse.FOOD_PAR_POINTS / food.nutrition()); // the far end's own food, and callers'
            }
            int glut = held.getValue() - (int) (GLUT * par);
            if (glut <= 0) {
                continue;
            }
            double discount = Math.min(MOST_GLUT_DISCOUNT, GLUT_DISCOUNT * ((double) held.getValue() / par - GLUT));
            double each = Prices.emeraldsEach(item) * (1 - discount);
            int quantity = Math.min(glut, room * item.getDefaultMaxStackSize());
            quantity = Math.min(quantity, (int) Math.floor((caravan.spurs - keepForFood) / Math.max(1e-9, each * rate)));
            if (quantity <= 0) {
                continue;
            }
            int taken = 0;
            for (ItemStack piece : stock.take(new ItemCost(item, 1), quantity)) {
                taken += piece.getCount();
                caravan.cargo.add(piece);
            }
            int spurs = (int) Math.ceil(taken * each * rate);
            caravan.spurs -= spurs;
            Treasury.deposit(level, desk.pos(), stock, spurs);
            route.itemsBought += taken;
            route.purchaseSpurs += spurs;
            boughtThere(server(level), route, caravan, item, taken, spurs, now);
            route.backhaulItems += taken;
            bought += taken;
        }
        if (bought > 0) {
            Enterprise.LOGGER.debug("Caravan {} backhauls {} items from {}", route.id, bought, desk.pos().toShortString());
        }
    }

    private static void buySurplus(MinecraftServer server, CaravanLedger ledger, ServerLevel level, Route route,
            MerchantStock stock, Caravan caravan, int residentLevel, int keepForFood, Territory territory, Mint mint,
            double rate, long now) {
        GlobalPos desk = route.to;
        if (!dev.b28.enterprise.merchant.OffMap.get(server).sells(desk)) {
            return; // the front keeps what it has for the army
        }
        Warehouse warehouse = Warehouse.get(server);
        Set<Item> wanted = wantedAt(server, desk);
        Map<Item, Integer> networkWants = networkWants(server, ledger, route, caravan, now);
        int spareFood = stock.foodNutrition() - Warehouse.FOOD_PAR_POINTS;
        for (Map.Entry<String, Integer> held : stock.totals().entrySet()) {
            if (Crates.stacks(caravan.cargo) >= caravan.capacity()) {
                return;
            }
            Item item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(held.getKey()));
            if (item == Items.EMERALD || item instanceof CoinItem || held.getKey().endsWith("shulker_box")
                    || item == dev.b28.enterprise.cargo.Cargo.CRATE_ITEM.get() || item == dev.b28.enterprise.cargo.Cargo.DOCUMENTS.get()) {
                continue;
            }
            int network = networkWants.getOrDefault(item, 0);
            // Comforts the towns it supplies are short of, where they'd pay more than this costs and the road.
            java.util.Optional<Desires.Outlet> outlet = Desires.outlet(server, route.from, route.to, item, now);
            int theirPar = warehouse.par(desk, item, wanted.contains(item), now);
            if (outlet.isPresent()) {
                int inCargo = Crates.contents(caravan.cargo).stream().filter(st -> st.is(item)).mapToInt(ItemStack::getCount).sum();
                network = Math.max(network, outlet.get().units() - warehouse.onHand(route.from, item) - inCargo);
                theirPar = (int) Math.ceil(territory.comfortPar(desk) / Math.max(1e-9, Prices.emeraldsEach(item))); // they keep their own
            }
            int surplus = Math.min(held.getValue() - theirPar, network);
            var food = item.getDefaultInstance().get(net.minecraft.core.component.DataComponents.FOOD);
            if (food != null && food.nutrition() > 0) {
                surplus = Math.min(surplus, Math.max(0, spareFood) / food.nutrition());
            }
            int room = (caravan.capacity() - Crates.stacks(caravan.cargo)) * item.getDefaultMaxStackSize();
            double value = Prices.emeraldsEach(item);
            double each = Bargain.between(Bargain.sellerFloor(value), value * RESALE_CEILING, residentLevel,
                    caravan.merchantLevel);
            int quantity = Math.min(surplus, room);
            if (outlet.isPresent() && !Desires.worthBuying(each, outlet.get())) {
                continue; // no margin in it
            }
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
            boughtThere(server, route, caravan, item, taken, spurs, now);
        }
    }

    /** True if the desk and the chests touching it are loaded; otherwise asks for them to load and returns false. */
    private static boolean ensureLoaded(ServerLevel level, BlockPos desk) {
        if (dev.b28.enterprise.merchant.OffMap.get(level.getServer()).is(GlobalPos.of(level.dimension(), desk))) {
            return true; // off the map: nothing to load
        }
        boolean loaded = true;
        List<ChunkPos> chunks = new ArrayList<>();
        for (int dx = -1; dx <= 1; dx++) {
            for (int dz = -1; dz <= 1; dz++) {
                chunks.add(new ChunkPos(desk.offset(dx, 0, dz)));
            }
        }
        // And the buildings the market keeps its goods in: they're part of its stock.
        dev.b28.enterprise.merchant.Depots.get(level.getServer()).of(GlobalPos.of(level.dimension(), desk))
                .forEach(depot -> chunks.addAll(depot.chunks()));
        for (ChunkPos chunk : chunks) {
            if (!level.hasChunk(chunk.x, chunk.z)) {
                level.getChunkSource().addRegionTicket(CARAVAN, chunk, 0, chunk);
                loaded = false;
            }
        }
        return loaded;
    }
}
