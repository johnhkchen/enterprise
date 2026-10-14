package dev.b28.enterprise.market;

import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.caravan.Caravans;
import dev.b28.enterprise.caravan.Route;
import dev.b28.enterprise.cargo.Cargo;
import dev.b28.enterprise.cargo.Crates;
import dev.b28.enterprise.cargo.Papers;
import dev.b28.enterprise.contract.Bills;
import dev.b28.enterprise.merchant.Depots;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Treasury;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.saveddata.SavedData;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * Freight: carriers selling space on their runs, forwarders buying it, and the hubs where goods change caravans. A
 * market with a storehouse (a warehouse it owns and runs) is a freight hub. Every route that touches it is a carrier's
 * run through it (its own caravans out, or another market's caravans calling and going home): each has a Numismatics
 * shop at the hub selling tickets for its next run from there, a stack of space a ticket, restocked each day to the
 * space it has to spare, at the road's cost a stack and a margin; what they take is the carrier's.
 * <p>
 * The hub's Merchant is the forwarder. A caravan that reaches the hub with crates bound further on hands them over for
 * a fee (the going freight and the forwarder's margin) and heads home; the forwarder buys just the space each needs on
 * the soonest run its way, puts the tickets in the crate's papers, and keeps it in the hub's warehouse till that run
 * loads it, booked crates first. Many shippers' crates ride one run. It's a banker too: it buys the shippers' bills of
 * exchange on the markets their goods are bound for, for coin now less a discount, and its own caravans present them.
 */
public final class Freight extends SavedData {
    private static final String NAME = "enterprise_freight";

    /** A freight ticket's document: a stack of space on a leg's run. */
    public static final String TICKET = "freight_space";
    /** A crate in a forwarder's hands, to be booked on. */
    public static final String FORWARDING = "forwarding";
    /** Carriers ask this much over the road's food cost a stack. */
    public static final double MARGIN = 0.25;
    /** And sell at most this share of a wagon's space: the rest is their own trade's. */
    public static final double SHARE = 0.5;
    /** A forwarder charges this much over the freight it buys. */
    public static final double FORWARD_MARGIN = 0.2;
    /** A forwarder discounts the bills it buys as a bank does: a fee, and so much a day to their due day. */
    public static final double DISCOUNT_FEE = 0.02;
    public static final double DISCOUNT_PER_DAY = 0.01;
    /**
     * Drayage: carting crates between the quay where caravans stand and the hub's warehouse. The hub's draymen (its
     * idle hands, and one more) each move this many stacks a day for a day's wage; it's sold at a shop of its own by the
     * cart, a ticket a cart of {@link #CART} stacks. A crate is carted in when it's handed over and out when its run
     * loads: the forwarder buys both up front.
     */
    public static final String DRAY = "drayage";
    public static final int DRAY_STACKS = 16;
    public static final double DRAY_WAGE = 0.5;
    public static final int CART = 4;

    /** Its shops stand in a row this far south of its desk, two blocks apart. */
    public static final int SHOP_ROW = 6;
    public static final long TICKS = 24000;

    /**
     * A carrier's run through a hub: its route, and which way (out from the route's home, or back to it). The carrier
     * is the route's home; the leg runs from the hub at one end to the market at the other.
     */
    public record Leg(Route route, boolean outbound) {
        public GlobalPos from() {
            return outbound ? route.from() : route.to();
        }

        public GlobalPos to() {
            return outbound ? route.to() : route.from();
        }

        public GlobalPos carrier() {
            return route.from();
        }

        int key() {
            return route.id() * 2 + (outbound ? 0 : 1);
        }
    }

    private final Map<Integer, BlockPos> shops = new HashMap<>(); // leg key -> its freight shop
    private final Map<GlobalPos, BlockPos> drayShops = new HashMap<>(); // hub -> its drayage shop
    // hub -> {stacks forwarded, forwarding fees, bills bought, their face, stacks carried, freight bought, drayage
    // wages taken, carts bought}
    private final Map<GlobalPos, double[]> books = new HashMap<>();
    private long lastDay = -1;

    public static Freight get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Freight::new, Freight::load, null), NAME);
    }

    /** Is this market a freight hub: does it run a storehouse? */
    public static boolean isHub(MinecraftServer server, GlobalPos desk) {
        return Depots.get(server).has(desk, Depots.Kind.STOREHOUSE);
    }

    /** Carriers' runs from the hub to {@code to}: its own caravans', and those of {@code to}'s that call there. */
    public static List<Leg> legs(MinecraftServer server, GlobalPos hub, GlobalPos to) {
        List<Leg> legs = new ArrayList<>();
        CaravanLedger.get(server).route(hub, to).ifPresent(r -> legs.add(new Leg(r, true)));
        CaravanLedger.get(server).route(to, hub).ifPresent(r -> legs.add(new Leg(r, false)));
        return legs;
    }

    /** Every carrier's run from the hub. */
    public static List<Leg> legsFrom(MinecraftServer server, GlobalPos hub) {
        List<Leg> legs = new ArrayList<>();
        for (Route r : CaravanLedger.get(server).routes()) {
            if (r.from().equals(hub)) {
                legs.add(new Leg(r, true));
            } else if (r.to().equals(hub)) {
                legs.add(new Leg(r, false));
            }
        }
        return legs;
    }

    /**
     * The hub between a shipper and a buyer: one the shipper's caravans go to (or whose caravans call at the shipper's),
     * with a carrier's run on to the buyer.
     */
    public static Optional<Leg> via(MinecraftServer server, GlobalPos shipper, GlobalPos buyer) {
        for (Route in : CaravanLedger.get(server).routes()) {
            GlobalPos hub = in.from().equals(shipper) ? in.to() : in.to().equals(shipper) ? in.from() : null;
            if (hub != null && isHub(server, hub) && !hub.equals(buyer)) {
                List<Leg> on = legs(server, hub, buyer);
                if (!on.isEmpty()) {
                    return Optional.of(on.get(0));
                }
            }
        }
        return Optional.empty();
    }

    /** What a stack of space on a run costs, in emeralds: its food both ways a wagon's stack, and a margin. */
    public static double stackPrice(Route route) {
        return Caravans.foodNeeded(route.distance()) * 2 * Caravans.PURSE_PER_FOOD_POINT / Caravans.CARGO_SLOTS
                * (1 + MARGIN);
    }

    /** What a forwarder asks a stack, in emeralds: the freight, and its margin. */
    public static double forwardingPrice(Route route) {
        return stackPrice(route) * (1 + FORWARD_MARGIN);
    }

    private static CompoundTag legTag(Leg leg) {
        CompoundTag tag = new CompoundTag();
        tag.putInt("route", leg.route().id());
        tag.putBoolean("outbound", leg.outbound());
        Papers.put(tag, "from", leg.from());
        Papers.put(tag, "to", leg.to());
        return tag;
    }

    /** A ticket for a stack of space on a leg's next run: the same papers each time, so a shop can sell them. */
    public static ItemStack ticket(Leg leg) {
        ItemStack ticket = new ItemStack(Cargo.DOCUMENTS.get());
        ticket.set(Cargo.PAPERS.get(), Papers.NONE.with(new Papers.Document(TICKET, legTag(leg))));
        return ticket;
    }

    /** Stacks of space booked on this leg in a crate's papers. */
    public static int booked(ItemStack crate, Leg leg) {
        if (!Crates.isCrate(crate)) {
            return 0;
        }
        return Crates.papers(crate).documents().stream()
                .filter(d -> d.kind().equals(TICKET) && d.data().getInt("route") == leg.route().id()
                        && d.data().getBoolean("outbound") == leg.outbound())
                .mapToInt(d -> Math.max(1, d.data().getInt("stacks"))).sum();
    }

    /** Does the crate carry a ticket for any run? */
    public static boolean ticketed(ItemStack crate) {
        return Crates.isCrate(crate) && Crates.papers(crate).first(TICKET).isPresent();
    }

    /** The crate, with {@code stacks} of tickets on the leg added to its papers. */
    public static ItemStack book(ItemStack crate, Leg leg, int stacks) {
        CompoundTag tag = legTag(leg);
        tag.putInt("stacks", stacks);
        return Crates.of(Crates.goods(crate), Crates.papers(crate).with(new Papers.Document(TICKET, tag)));
    }

    /** Is this one of the carriers' freight shops, or a hub's drayage? (Merchants don't shop there as players do.) */
    public boolean isShop(BlockPos pos) {
        return shops.containsValue(pos) || drayShops.containsValue(pos);
    }

    /** A ticket for a cart of drayage at the hub. */
    public static ItemStack cart(GlobalPos hub) {
        CompoundTag tag = new CompoundTag();
        Papers.put(tag, "at", hub);
        tag.putInt("stacks", CART);
        ItemStack ticket = new ItemStack(Cargo.DOCUMENTS.get());
        ticket.set(Cargo.PAPERS.get(), Papers.NONE.with(new Papers.Document(DRAY, tag)));
        return ticket;
    }

    /** What a cart costs, in emeralds: the draymen's wage for its stacks, and a margin. */
    public static double cartPrice() {
        return DRAY_WAGE / DRAY_STACKS * CART * (1 + MARGIN);
    }

    /** Carts the hub's draymen can move a day: its idle hands and one more, each {@link #DRAY_STACKS} stacks. */
    public static int cartsPerDay(MinecraftServer server, GlobalPos hub) {
        long idle = dev.b28.enterprise.territory.Territory.get(server).bellOf(hub)
                .map(bell -> dev.b28.enterprise.settlement.Settlements.get(server).roster(bell).values().stream()
                        .filter(r -> !r.baby() && dev.b28.enterprise.territory.Labour.idle(r.profession())).count())
                .orElse(0L);
        return (int) ((idle + 1) * DRAY_STACKS / CART);
    }

    public Optional<BlockPos> drayShop(GlobalPos hub) {
        return Optional.ofNullable(drayShops.get(hub));
    }

    public Optional<BlockPos> shop(Leg leg) {
        return Optional.ofNullable(shops.get(leg.key()));
    }

    /**
     * {stacks forwarded, forwarding fees in spurs, bills bought, their face, stacks carried, freight bought in spurs,
     * drayage wages in spurs, carts bought}.
     */
    public double[] book(GlobalPos hub) {
        return books.getOrDefault(hub, new double[8]).clone();
    }

    public Map<GlobalPos, double[]> books() {
        Map<GlobalPos, double[]> copy = new HashMap<>();
        books.forEach((hub, b) -> copy.put(hub, b.clone()));
        return copy;
    }

    double[] tally(GlobalPos hub) {
        setDirty();
        return books.computeIfAbsent(hub, h -> new double[8]);
    }

    /** Once a day, at each loaded hub: its carriers' shops up and restocked, and its forwarder books what it holds. */
    public static void onServerTick(ServerTickEvent.Post event) {
        MinecraftServer server = event.getServer();
        long now = server.overworld().getGameTime();
        Freight freight = get(server);
        if (freight.lastDay >= 0 && now - freight.lastDay < TICKS) {
            return;
        }
        freight.lastDay = now;
        for (var depot : Depots.get(server).all()) {
            if (depot.kind() != Depots.Kind.STOREHOUSE) {
                continue;
            }
            GlobalPos hub = depot.desk();
            ServerLevel level = server.getLevel(hub.dimension());
            if (level == null || !level.isLoaded(hub.pos())) {
                continue;
            }
            for (Leg leg : legsFrom(server, hub)) {
                freight.restock(level, leg, now);
            }
            freight.restockDray(level, hub, now);
            freight.forward(level, hub, MerchantStock.at(level, hub.pos()), now);
        }
        freight.setDirty();
    }

    /**
     * A leg's freight shop by its hub's desk: put up if there's none, and stocked with tickets for the space its next
     * run has to spare (its share of a wagon, less what's booked and waiting), at its price. The hub's own runs' takings
     * are banked with it; another carrier's wait in the shop for its caravan to collect.
     */
    void restock(ServerLevel level, Leg leg, long now) {
        MinecraftServer server = level.getServer();
        BlockPos pos = shops.get(leg.key());
        if (pos == null || !(level.getBlockEntity(pos) instanceof VendorBlockEntity)) {
            pos = putUp(level, leg).orElse(null);
        }
        if (pos == null || !(level.getBlockEntity(pos) instanceof VendorBlockEntity shop)) {
            return;
        }
        MerchantStock stock = MerchantStock.at(level, leg.from().pos());
        if (leg.carrier().equals(leg.from())) {
            int takings = Shops.collect(shop);
            if (takings > 0) {
                Treasury.deposit(level, leg.from().pos(), stock, takings);
            }
        }
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        Shops.setPrice(shop, Math.max(1, (int) Math.ceil(stackPrice(leg.route()) * rate)));
        int waiting = stock.looseStacks().stream().mapToInt(s -> booked(s, leg)).sum();
        int forSale = Math.max(0, (int) (Caravans.CARGO_SLOTS * SHARE) - waiting);
        shop.items.clear();
        for (int i = 0; i < Math.min(forSale, shop.items.size()); i++) {
            shop.items.set(i, ticket(leg)); // a ticket a slot: papers don't stack
        }
        shop.setChanged();
    }

    /** The hub's drayage shop: up if there's none, the day's wages taken, and stocked with a day's carts. */
    void restockDray(ServerLevel level, GlobalPos hub, long now) {
        MinecraftServer server = level.getServer();
        BlockPos pos = drayShops.get(hub);
        if (pos == null || !(level.getBlockEntity(pos) instanceof VendorBlockEntity)) {
            pos = putUp(level, hub.pos(), cart(hub)).orElse(null);
            if (pos != null) {
                drayShops.put(hub, pos);
            }
        }
        if (pos == null || !(level.getBlockEntity(pos) instanceof VendorBlockEntity shop)) {
            return;
        }
        MerchantStock stock = MerchantStock.at(level, hub.pos());
        int wages = Shops.collect(shop); // the draymen's pay: the town's people's, kept in its treasury
        if (wages > 0) {
            Treasury.deposit(level, hub.pos(), stock, wages);
            tally(hub)[6] += wages;
        }
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        Shops.setPrice(shop, Math.max(1, (int) Math.ceil(cartPrice() * rate)));
        shop.items.clear();
        for (int i = 0; i < Math.min(cartsPerDay(server, hub), shop.items.size()); i++) {
            shop.items.set(i, cart(hub));
        }
        shop.setChanged();
    }

    private Optional<BlockPos> putUp(ServerLevel level, Leg leg) {
        Optional<BlockPos> pos = putUp(level, leg.from().pos(), ticket(leg));
        pos.ifPresent(p -> {
            shops.put(leg.key(), p);
            setDirty();
        });
        return pos;
    }

    /** A selling shop for {@code ticket}s in the row by the desk. */
    private Optional<BlockPos> putUp(ServerLevel level, BlockPos desk, ItemStack ticket) {
        Block vendor = BuiltInRegistries.BLOCK.get(ResourceLocation.parse("numismatics:vendor"));
        for (int i = 0; i < 12; i++) {
            BlockPos pos = desk.offset(-6 + 2 * i, 0, SHOP_ROW);
            if (!isShop(pos) && level.getBlockState(pos).isAir()) {
                level.setBlock(pos, vendor.defaultBlockState(), 3);
                if (level.getBlockEntity(pos) instanceof VendorBlockEntity shop) {
                    shop.setMode(VendorBlockEntity.Mode.SELL);
                    shop.filterContainer.setItem(0, ticket.copy());
                    shop.setChanged();
                    return Optional.of(pos);
                }
            }
        }
        return Optional.empty();
    }

    /** A carrier's caravan calling at the hub collects what its shop there has taken. Returns spurs. */
    public int collectFares(ServerLevel level, Leg leg) {
        return shop(leg).filter(level::isLoaded)
                .map(pos -> level.getBlockEntity(pos) instanceof VendorBlockEntity shop ? Shops.collect(shop) : 0)
                .orElse(0);
    }

    /**
     * A caravan at a hub hands the forwarder its crates bound further on (where a carrier runs that way), paying from
     * its purse the forwarding (a stack's going freight and the margin, each stack); the crates go into the hub's
     * warehouse to be booked on. Crates it can't pay for, or no carrier runs on with, stay aboard. Returns stacks
     * handed over.
     */
    public int handOver(ServerLevel level, GlobalPos hub, MerchantStock stock, List<ItemStack> cargo, int[] purse) {
        MinecraftServer server = level.getServer();
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        int stacksIn = 0;
        for (var it = cargo.listIterator(); it.hasNext(); ) {
            ItemStack crate = it.next();
            Optional<GlobalPos> to = Crates.isCrate(crate) ? Crates.papers(crate).consignee() : Optional.empty();
            if (to.isEmpty() || to.get().equals(hub) || ticketed(crate)) {
                continue;
            }
            List<Leg> on = legs(server, hub, to.get());
            if (on.isEmpty() || !stock.hasRoomFor(crate)) {
                continue;
            }
            int stacks = Crates.stacks(List.of(crate));
            // Carted in from the quay and out to its run: the forwarder buys the drayage up front, at the hub's shop.
            int carts = (2 * stacks + CART - 1) / CART;
            Optional<VendorBlockEntity> dray = drayShop(hub).map(level::getBlockEntity)
                    .filter(VendorBlockEntity.class::isInstance).map(VendorBlockEntity.class::cast);
            if (dray.isEmpty() || Shops.lots(dray.get()) < carts) {
                continue; // no draymen free today: it stays aboard
            }
            double cheapest = on.stream().mapToDouble(l -> forwardingPrice(l.route())).min().orElse(0);
            int drayage = carts * dray.get().getTotalPrice();
            int fee = (int) Math.ceil(stacks * cheapest * rate + drayage * (1 + FORWARD_MARGIN));
            if (purse[0] < fee) {
                continue;
            }
            purse[0] -= fee;
            Treasury.deposit(level, hub.pos(), stock, fee - drayage);
            for (int i = 0; i < carts; i++) {
                Shops.buyFrom(dray.get()); // paid into the drayage shop: the draymen's wages
            }
            CompoundTag tag = new CompoundTag();
            Papers.put(tag, "to", to.get());
            CompoundTag carted = new CompoundTag();
            Papers.put(carted, "at", hub);
            carted.putInt("stacks", carts * CART);
            it.remove();
            stock.insert(Crates.of(Crates.goods(crate), Crates.papers(crate).with(new Papers.Document(FORWARDING, tag))
                    .with(new Papers.Document(DRAY, carted))));
            tally(hub)[7] += carts;
            tally(hub)[0] += stacks;
            tally(hub)[1] += fee;
            stacksIn += stacks;
        }
        return stacksIn;
    }

    /** A crate in the forwarder's hands: marked to be booked on toward its consignee. */
    public ItemStack forwarding(ItemStack crate) {
        CompoundTag tag = new CompoundTag();
        Crates.papers(crate).consignee().ifPresent(to -> Papers.put(tag, "to", to));
        return Crates.of(Crates.goods(crate), Crates.papers(crate).with(new Papers.Document(FORWARDING, tag)));
    }

    /** The forwarder's books: stacks it took on, and the fees it was paid. */
    public void forwarded(GlobalPos hub, int stacks, int fee) {
        tally(hub)[0] += stacks;
        tally(hub)[1] += fee;
    }

    /**
     * The forwarder's agent, on a hub caravan at a shipper's, buys the shipper's bills (among {@code papers}) drawn on
     * markets the hub runs freight to, not yet due, for coin from the caravan's purse (up to {@code purse}), less the
     * discount; the coin goes into the shipper's treasury, the bills, endorsed to the hub, ride home. Returns spurs paid.
     */
    public int discountBills(ServerLevel level, GlobalPos hub, GlobalPos shipper, List<ItemStack> papers, int purse,
            long now) {
        MinecraftServer server = level.getServer();
        Bills ledger = Bills.get(server);
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        int paid = 0;
        for (ItemStack bundle : papers) {
            if (!bundle.is(Cargo.DOCUMENTS.get())) {
                continue;
            }
            for (Papers.Document doc : bundle.getOrDefault(Cargo.PAPERS.get(), Papers.NONE).documents()) {
                var bill = ledger.bill(doc.data().getInt("bill")).orElse(null);
                if (bill == null || bill.state() != Bills.State.OUTSTANDING || !bill.payee().equals(shipper)
                        || bill.due() <= now || legs(server, hub, bill.drawee()).isEmpty()) {
                    continue;
                }
                double days = (bill.due() - now) / 24000.0;
                int spurs = (int) Math.floor(bill.emeralds() * (1 - DISCOUNT_FEE - DISCOUNT_PER_DAY * days) * rate);
                if (spurs <= 0 || purse - paid < spurs) {
                    continue;
                }
                paid += spurs;
                Treasury.deposit(level, shipper.pos(), MerchantStock.at(level, shipper.pos()), spurs);
                ledger.endorse(bill, hub);
                tally(hub)[2] += 1;
                tally(hub)[3] += bill.emeralds();
            }
        }
        return paid;
    }

    /**
     * The forwarder books the crates in its hands: for each not yet ticketed, just the space it needs on the soonest
     * carrier's run toward its consignee that has it, bought at that run's shop with the hub's money, the tickets put
     * in its papers. Returns stacks booked.
     */
    public int forward(ServerLevel level, GlobalPos hub, MerchantStock stock, long now) {
        MinecraftServer server = level.getServer();
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        List<ItemStack> held = stock.removeLoose(s -> Crates.isCrate(s) && Crates.papers(s).first(FORWARDING).isPresent()
                && !ticketed(s));
        int bookedStacks = 0;
        for (ItemStack crate : held) {
            Optional<GlobalPos> to = Crates.papers(crate).consignee();
            int stacks = Crates.stacks(List.of(crate));
            ItemStack result = crate;
            if (Crates.papers(crate).first(DRAY).isEmpty()) {
                // Collected from a town by the hub's own caravan: carted in and out now, from the hub's drayage.
                int carts = (2 * stacks + CART - 1) / CART;
                Optional<VendorBlockEntity> dray = drayShop(hub).map(level::getBlockEntity)
                        .filter(VendorBlockEntity.class::isInstance).map(VendorBlockEntity.class::cast);
                if (dray.isEmpty() || Shops.lots(dray.get()) < carts) {
                    stock.insert(crate); // no draymen free today: it waits on the quay
                    continue;
                }
                double cost = carts * dray.get().getTotalPrice() / rate;
                Treasury.withdraw(level, hub.pos(), stock, cost, Mint.get(server), now);
                for (int i = 0; i < carts; i++) {
                    Shops.buyFrom(dray.get());
                }
                CompoundTag carted = new CompoundTag();
                Papers.put(carted, "at", hub);
                carted.putInt("stacks", carts * CART);
                result = Crates.of(Crates.goods(crate), Crates.papers(crate).with(new Papers.Document(DRAY, carted)));
                tally(hub)[7] += carts;
                crate = result;
            }
            if (to.isPresent()) {
                List<Leg> on = new ArrayList<>(legs(server, hub, to.get()));
                on.sort(java.util.Comparator.comparingLong(l -> nextRun(server, l, now)));
                for (Leg leg : on) {
                    Optional<BlockPos> at = shop(leg);
                    if (at.isEmpty() || !(level.getBlockEntity(at.get()) instanceof VendorBlockEntity shop)
                            || Shops.lots(shop) < stacks) {
                        continue;
                    }
                    double cost = stacks * shop.getTotalPrice() / rate;
                    if (Treasury.available(level, hub.pos(), stock, rate)
                            - dev.b28.enterprise.territory.Territory.get(server).reserve(hub) < cost) {
                        continue;
                    }
                    Treasury.withdraw(level, hub.pos(), stock, cost, Mint.get(server), now);
                    for (int i = 0; i < stacks; i++) {
                        Shops.buyFrom(shop);
                    }
                    tally(hub)[5] += stacks * shop.getTotalPrice();
                    result = book(crate, leg, stacks);
                    bookedStacks += stacks;
                    break;
                }
            }
            stock.insert(result);
        }
        return bookedStacks;
    }

    /** When a leg's next run leaves the hub: the hub's own caravan's next departure, or a caller's next arrival. */
    static long nextRun(MinecraftServer server, Leg leg, long now) {
        CaravanLedger ledger = CaravanLedger.get(server);
        if (leg.outbound()) {
            return ledger.nextDeparture(leg.route(), now);
        }
        long travel = Caravans.travelTicks(leg.route().distance());
        return ledger.caravan(leg.route().id()).filter(c -> c.leg() == dev.b28.enterprise.caravan.Caravan.Leg.OUTBOUND)
                .map(c -> c.arrives()).orElse(ledger.nextDeparture(leg.route(), now) + travel);
    }

    /** Booked crates waiting at the hub for this leg's run, taken out to load first, as many as {@code slots} hold. */
    public List<ItemStack> loadBooked(GlobalPos hub, MerchantStock stock, Leg leg, int slots) {
        int[] room = {slots};
        List<ItemStack> load = stock.removeLoose(s -> {
            if (booked(s, leg) <= 0 || room[0] < Crates.stacks(List.of(s))) {
                return false;
            }
            room[0] -= Crates.stacks(List.of(s));
            return true;
        });
        if (!load.isEmpty()) {
            tally(hub)[4] += Crates.stacks(load);
        }
        return load;
    }

    /**
     * The forwarder buys the shipper's bills: those its caravan carries, drawn on a market the hub runs caravans to and
     * payable to the caravan's home, not yet due, for coin from the hub now (less the fee and the days to their due
     * day), endorsed to the hub and filed in its warehouse for its caravans to present. Returns spurs paid.
     */
    public int discountBills(ServerLevel level, GlobalPos hub, GlobalPos shipper, MerchantStock stock,
            List<ItemStack> cargo, long now) {
        MinecraftServer server = level.getServer();
        Bills ledger = Bills.get(server);
        CaravanLedger routes = CaravanLedger.get(server);
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        double spare = Treasury.available(level, hub.pos(), stock, rate)
                - dev.b28.enterprise.territory.Territory.get(server).reserve(hub);
        int paid = 0;
        List<Papers.Document> bought = new ArrayList<>();
        for (var it = cargo.listIterator(); it.hasNext(); ) {
            ItemStack bundle = it.next();
            if (!bundle.is(Cargo.DOCUMENTS.get()) || Bills.ids(bundle).isEmpty()) {
                continue;
            }
            List<Papers.Document> kept = new ArrayList<>();
            for (Papers.Document doc : bundle.getOrDefault(Cargo.PAPERS.get(), Papers.NONE).documents()) {
                var bill = ledger.bill(doc.data().getInt("bill")).orElse(null);
                if (bill == null || bill.state() != Bills.State.OUTSTANDING || !bill.payee().equals(shipper)
                        || bill.due() <= now || legs(server, hub, bill.drawee()).isEmpty()) {
                    kept.add(doc);
                    continue;
                }
                double days = (bill.due() - now) / 24000.0;
                double price = bill.emeralds() * (1 - DISCOUNT_FEE - DISCOUNT_PER_DAY * days);
                if (price <= 0 || spare < price) {
                    kept.add(doc);
                    continue;
                }
                spare -= price;
                Treasury.Withdrawal taken = Treasury.withdraw(level, hub.pos(), stock, price, Mint.get(server), now);
                paid += taken.spurs() + Mint.get(server).sell(Mint.DEFAULT_FACTION, taken.emeralds(), now);
                ledger.endorse(bill, hub);
                bought.add(doc);
                tally(hub)[2] += 1;
                tally(hub)[3] += bill.emeralds();
            }
            if (kept.size() != bundle.getOrDefault(Cargo.PAPERS.get(), Papers.NONE).documents().size()) {
                it.remove();
                if (!kept.isEmpty()) {
                    it.add(Crates.documents(new Papers(kept)));
                }
            }
        }
        if (!bought.isEmpty()) {
            stock.insert(Crates.documents(new Papers(bought)));
        }
        return paid;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        CompoundTag shopTag = new CompoundTag();
        shops.forEach((route, pos) -> shopTag.putLong(Integer.toString(route), pos.asLong()));
        tag.put("shops", shopTag);
        ListTag drays = new ListTag();
        drayShops.forEach((hub, pos) -> {
            CompoundTag e = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, hub).ifSuccess(t -> e.put("hub", t));
            e.putLong("pos", pos.asLong());
            drays.add(e);
        });
        tag.put("drayShops", drays);
        ListTag list = new ListTag();
        books.forEach((hub, b) -> {
            CompoundTag e = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, hub).ifSuccess(t -> e.put("hub", t));
            ListTag values = new ListTag();
            for (double v : b) {
                values.add(net.minecraft.nbt.DoubleTag.valueOf(v));
            }
            e.put("book", values);
            list.add(e);
        });
        tag.put("books", list);
        tag.putLong("lastDay", lastDay);
        return tag;
    }

    public static Freight load(CompoundTag tag, HolderLookup.Provider registries) {
        Freight freight = new Freight();
        CompoundTag shopTag = tag.getCompound("shops");
        shopTag.getAllKeys().forEach(k -> freight.shops.put(Integer.parseInt(k), BlockPos.of(shopTag.getLong(k))));
        for (Tag t : tag.getList("drayShops", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("hub")).result()
                    .ifPresent(hub -> freight.drayShops.put(hub, BlockPos.of(e.getLong("pos"))));
        }
        for (Tag t : tag.getList("books", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("hub")).result().ifPresent(hub -> {
                ListTag values = e.getList("book", Tag.TAG_DOUBLE);
                double[] b = new double[8];
                for (int i = 0; i < Math.min(8, values.size()); i++) {
                    b[i] = values.getDouble(i);
                }
                freight.books.put(hub, b);
            });
        }
        freight.lastDay = tag.contains("lastDay") ? tag.getLong("lastDay") : -1;
        return freight;
    }
}
