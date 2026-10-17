package dev.b28.enterprise.market;

import dev.b28.enterprise.Dials;
import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.caravan.Caravans;
import dev.b28.enterprise.cargo.Crates;
import dev.b28.enterprise.cargo.Papers;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.money.Bargain;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.money.Treasury;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
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
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * The forwarder: a freight hub's Merchant arranging carriage for the towns it serves, so a small town needn't run a
 * wagon of its own. A town whose own wagon would go near empty books by post: its goods owed to markets beyond the
 * hub (contract freight, crated with their bills of lading) and its goods for sale (on consignment). The forwarder
 * collects once a town's bookings fill a good share of a wagon ({@code collect_stacks}), or they've waited
 * {@code collect_days}: its run there goes that day (a run on call where it has none). Contract crates it books on
 * as it always has. Goods on consignment it takes into the hub's stock as the town's lot and sells with its own,
 * through its runs; as they sell, the proceeds go back to the town by post, less its commission
 * ({@code consign_commission}) and the collection's fare. A lot not sold in {@code consign_days} it buys, at what
 * a trader would give.
 */
public final class Forwarder extends SavedData {
    private static final String NAME = "enterprise_forwarder";
    /** A consignment lot's bill of lading says so. */
    public static final String CONSIGN = "CONSIGN";

    /** A town's booking with a hub: its stacks to collect, what its goods for sale are worth, and since when. */
    static final class Booking {
        GlobalPos hub;
        GlobalPos town;
        int stacks;
        double worth;
        long since;
    }

    /** Goods on consignment at a hub: the town's, of an item, how many, how many sold and for what, since when. */
    static final class Lot {
        GlobalPos hub;
        GlobalPos town;
        Item item;
        int count;
        int sold;
        double proceeds; // spurs, not yet remitted
        double fare; // spurs owed for the collection, taken from what's remitted
        long since;
    }

    private final List<Booking> bookings = new ArrayList<>();
    private final List<Lot> lots = new ArrayList<>();
    private final List<GlobalPos[]> collecting = new ArrayList<>(); // {hub, town}: a collection due
    private double remitted; // spurs, all told
    private double commission; // spurs, all told
    private long consigned; // items left unsold by towns' wagons at hubs and taken on consignment, all told

    public static Forwarder get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Forwarder::new, Forwarder::load, null), NAME);
    }

    /** The hub a town books with: the nearest freight hub a wagon reaches. */
    static Optional<GlobalPos> hubFor(MinecraftServer server, GlobalPos town) {
        return Post.hubsFor(server, town).stream().findFirst();
    }

    /** A stack's collection fare: the hub's wagon's space coming home from the town. */
    static double fare(MinecraftServer server, GlobalPos hub, GlobalPos town) {
        var route = CaravanLedger.get(server).route(hub, town);
        int distance = (int) Math.sqrt(hub.pos().distSqr(town.pos()));
        return Freight.runCost(distance) / Caravans.CARGO_SLOTS * (1 + Freight.MARGIN)
                * (route.isPresent() ? 1 : 1.5); // a run on call costs more a stack than a regular one
    }

    /** What the town would consign: its goods beyond its own par that the hub's network wants, a wagon's worth. */
    static Map<Item, Integer> forSale(MinecraftServer server, GlobalPos town, GlobalPos hub, MerchantStock stock, long now) {
        Warehouse warehouse = Warehouse.get(server);
        var wanted = new java.util.HashSet<>(Caravans.wantedAt(server, hub));
        for (var r : CaravanLedger.get(server).routes()) {
            if (r.from().equals(hub) && !r.to().equals(town)) {
                wanted.addAll(Caravans.wantedAt(server, r.to()));
            }
        }
        var own = Caravans.wantedAt(server, town);
        Map<Item, Integer> out = new java.util.LinkedHashMap<>();
        int stacks = 0;
        for (Map.Entry<String, Integer> held : stock.totals().entrySet()) {
            Item item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(held.getKey()));
            if (!wanted.contains(item) || Prices.emeraldsEach(item) <= 0) {
                continue;
            }
            int spare = held.getValue() - warehouse.par(town, item, own.contains(item), now)
                    - dev.b28.enterprise.caravan.ForwardSales.setAside(server, town, item, stock, now);
            int room = (Caravans.CARGO_SLOTS - stacks) * item.getDefaultMaxStackSize();
            int going = Math.min(spare, room);
            if (going <= 0) {
                continue;
            }
            out.put(item, going);
            stacks += (going + item.getDefaultMaxStackSize() - 1) / item.getDefaultMaxStackSize();
        }
        return out;
    }

    /** Crates the town holds bound for markets beyond the hub, under contract (with their bills of lading). */
    static int contractStacks(MinecraftServer server, GlobalPos town, GlobalPos hub, MerchantStock stock) {
        return Crates.stacks(stock.looseStacks().stream().filter(s -> Crates.isCrate(s)
                && Crates.papers(s).shipper().filter(town::equals).isPresent()
                && Crates.papers(s).consignee().filter(c -> !c.equals(town) && !c.equals(hub)
                        && !Freight.legs(server, hub, c).isEmpty()).isPresent()).toList());
    }

    /** Does the town ship for itself: a wagon of its own to the hub that isn't waiting on a load? */
    static boolean shipsItself(MinecraftServer server, GlobalPos town, GlobalPos hub) {
        return CaravanLedger.get(server).routes().stream().anyMatch(r -> r.from().equals(town) && r.to().equals(hub)
                && !r.stopping() && !r.status().startsWith("waiting for a fuller load"));
    }

    /**
     * Once a day, at a town: if it doesn't ship for itself and has goods worth carrying (contract crates, or goods for
     * sale worth twice their fare), it books with its hub by post. One booking at a time.
     */
    public void considerBooking(ServerLevel level, GlobalPos town, MerchantStock stock, long now) {
        MinecraftServer server = level.getServer();
        if (Freight.isHub(server, town) || Sovereign.get(server).desk().filter(town::equals).isPresent()) {
            return;
        }
        Optional<GlobalPos> hub = hubFor(server, town);
        if (hub.isEmpty() || shipsItself(server, town, hub.get())
                || bookings.stream().anyMatch(b -> b.town.equals(town))
                || Post.get(server).onTheRoad(hub.get(), Post.Kind.BOOK, town) > 0) {
            return;
        }
        Map<Item, Integer> goods = forSale(server, town, hub.get(), stock, now);
        int saleStacks = goods.entrySet().stream()
                .mapToInt(e -> (e.getValue() + e.getKey().getDefaultMaxStackSize() - 1) / e.getKey().getDefaultMaxStackSize())
                .sum();
        double worth = goods.entrySet().stream().mapToDouble(e -> e.getValue() * Prices.emeraldsEach(e.getKey())).sum();
        int contract = contractStacks(server, town, hub.get(), stock);
        double fare = fare(server, hub.get(), town);
        boolean worthIt = contract > 0 || saleStacks > 0 && worth >= 2 * fare * saleStacks;
        if (!worthIt) {
            return;
        }
        Post.get(server).book(level, stock, town, hub.get(), contract + (worth >= 2 * fare * saleStacks ? saleStacks : 0),
                worth, now);
    }

    /** The hub reads a booking. */
    void booked(GlobalPos hub, GlobalPos town, int stacks, double worth, long now) {
        bookings.removeIf(b -> b.town.equals(town));
        Booking b = new Booking();
        b.hub = hub;
        b.town = town;
        b.stacks = stacks;
        b.worth = worth;
        b.since = now;
        bookings.add(b);
        setDirty();
    }

    /**
     * Once a day, at a hub: collections for the towns whose bookings fill enough of a wagon or have waited long
     * enough (its run there due today; a run on call where it has none), and the proceeds of lots sold remitted.
     */
    public void review(ServerLevel level, GlobalPos hub, MerchantStock stock, long now) {
        MinecraftServer server = level.getServer();
        CaravanLedger ledger = CaravanLedger.get(server);
        for (Booking b : bookings) {
            if (!b.hub.equals(hub) || collecting.stream().anyMatch(c -> c[0].equals(hub) && c[1].equals(b.town))) {
                continue;
            }
            boolean enough = b.stacks >= Dials.COLLECT_STACKS.get();
            boolean waited = now - b.since >= Dials.COLLECT_DAYS.ticks();
            if (!enough && !waited) {
                continue;
            }
            collecting.add(new GlobalPos[] {hub, b.town});
            if (ledger.route(hub, b.town).isEmpty()) {
                ledger.addRoute(hub, b.town).setOnCall(true);
            }
            Post.get(server).bookCall(hub, b.town, now / 24000, b.worth);
            dev.b28.enterprise.Enterprise.LOGGER.info("{}'s forwarder collects at {}: {} stacks booked",
                    dev.b28.enterprise.settlement.Names.label(hub), dev.b28.enterprise.settlement.Names.label(b.town), b.stacks);
            setDirty();
        }
        remit(level, hub, stock, now);
    }

    /** Is a collection due on the hub's run to the town (so the run goes)? */
    public boolean collecting(GlobalPos hub, GlobalPos town) {
        return collecting.stream().anyMatch(c -> c[0].equals(hub) && c[1].equals(town));
    }

    /**
     * The hub's wagon at a booked town: takes its goods for sale on consignment (the town's lot, crated with a bill of
     * lading to the hub), as far as the wagon has room; the contract crates go with the ordinary collection. The
     * booking is done.
     */
    public void collect(ServerLevel level, GlobalPos hub, GlobalPos town, MerchantStock stock,
            List<ItemStack> cargo, int capacity, long now) {
        MinecraftServer server = level.getServer();
        if (!collecting(hub, town)) {
            return;
        }
        collecting.removeIf(c -> c[0].equals(hub) && c[1].equals(town));
        bookings.removeIf(b -> b.town.equals(town) && b.hub.equals(hub));
        setDirty();
        double fare = fare(server, hub, town) * Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        for (Map.Entry<Item, Integer> sale : forSale(server, town, hub, stock, now).entrySet()) {
            int room = (capacity - Crates.stacks(cargo)) * sale.getKey().getDefaultMaxStackSize();
            int going = Math.min(sale.getValue(), room);
            if (going <= 0) {
                continue;
            }
            List<ItemStack> goods = stock.take(new ItemCost(sale.getKey(), 1), going);
            int taken = goods.stream().mapToInt(ItemStack::getCount).sum();
            if (taken <= 0) {
                continue;
            }
            var bill = Papers.billOfLading(-1, town, hub, Crates.describe(goods), CONSIGN, -1, now);
            bill.data().putBoolean("settled", true); // no bill of exchange: the forwarder sells it on the town's account
            bill.data().putBoolean("consigned", true);
            for (ItemStack crate : Crates.pack(level, town, stock, goods, Papers.NONE.with(bill), now)) {
                cargo.add(crate);
            }
            Lot lot = new Lot();
            lot.hub = hub;
            lot.town = town;
            lot.item = sale.getKey();
            lot.count = taken;
            lot.fare = fare * Math.ceil(taken / (double) sale.getKey().getDefaultMaxStackSize());
            lot.since = now;
            lots.add(lot);
        }
    }

    /**
     * A town's wagon at the hub with goods it couldn't sell: what the hub's network wants (its own shelves and the
     * towns its runs serve, beyond what it holds and has on consignment already), the forwarder takes on consignment,
     * as far as the hub has room. Into the hub's stock as the town's lots, sold with its own; no collection fare (the
     * town brought them). The rest rides home. Returns how many it took.
     */
    public int consignUnsold(ServerLevel level, GlobalPos hub, GlobalPos town, MerchantStock stock,
            List<ItemStack> cargo, long now) {
        MinecraftServer server = level.getServer();
        Warehouse warehouse = Warehouse.get(server);
        Map<Item, Integer> room = new java.util.HashMap<>();
        java.util.function.Function<Item, Integer> roomFor = item -> room.computeIfAbsent(item, i -> {
            int want = warehouse.par(hub, i, Caravans.wantedAt(server, hub).contains(i), now);
            for (var r : CaravanLedger.get(server).routes()) {
                if (r.from().equals(hub) && !r.to().equals(town) && Caravans.wantedAt(server, r.to()).contains(i)) {
                    want += warehouse.par(r.to(), i, true, now);
                }
            }
            int onConsignment = lots.stream().filter(l -> l.hub.equals(hub) && l.item == i)
                    .mapToInt(l -> l.count - l.sold).sum();
            return Math.max(0, want - stock.count(new ItemCost(i, 1)) - onConsignment);
        });
        Map<Item, Integer> taken = new java.util.LinkedHashMap<>();
        for (var it = cargo.listIterator(); it.hasNext(); ) {
            ItemStack entry = it.next();
            var lot = Crates.lot(entry); // a crate of one good for sale
            if (Crates.isCrate(entry) && lot.isEmpty() || entry.is(dev.b28.enterprise.cargo.Cargo.DOCUMENTS.get())) {
                continue; // someone's shipment, empties, or papers
            }
            ItemStack goods = lot.orElse(entry);
            Item item = goods.getItem();
            int can = roomFor.apply(item);
            if (Prices.emeraldsEach(item) <= 0 || can < goods.getCount() || !stock.hasRoomFor(goods)) {
                continue;
            }
            it.remove();
            if (lot.isPresent()) {
                Crates.open(server, hub, stock, entry).forEach(stock::insert);
            } else {
                stock.insert(goods);
            }
            room.put(item, can - goods.getCount());
            taken.merge(item, goods.getCount(), Integer::sum);
        }
        taken.forEach((item, count) -> {
            Lot l = new Lot();
            l.hub = hub;
            l.town = town;
            l.item = item;
            l.count = count;
            l.since = now;
            lots.add(l);
            consigned += count;
        });
        if (!taken.isEmpty()) {
            setDirty();
        }
        return taken.values().stream().mapToInt(Integer::intValue).sum();
    }

    /** Goods of {@code item} the hub's caravan sold: credited to the town's lots of it, oldest first. */
    public void sold(GlobalPos hub, Item item, int count, double spurs) {
        if (count <= 0) {
            return;
        }
        double each = spurs / count;
        int left = count;
        for (Lot lot : lots) {
            if (left <= 0) {
                break;
            }
            if (!lot.hub.equals(hub) || lot.item != item || lot.sold >= lot.count) {
                continue;
            }
            int n = Math.min(left, lot.count - lot.sold);
            lot.sold += n;
            lot.proceeds += n * each;
            left -= n;
            setDirty();
        }
    }

    /**
     * The proceeds of the hub's lots sold, less its commission and the collection's fare, remitted to their towns by
     * post (from the hub's treasury, where the sales' coin went); a lot not sold in {@code consign_days} the forwarder
     * buys at what a trader would give.
     */
    void remit(ServerLevel level, GlobalPos hub, MerchantStock stock, long now) {
        MinecraftServer server = level.getServer();
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        for (var it = lots.iterator(); it.hasNext(); ) {
            Lot lot = it.next();
            if (!lot.hub.equals(hub)) {
                continue;
            }
            boolean done = lot.sold >= lot.count;
            if (!done && now - lot.since >= Dials.CONSIGN_DAYS.ticks()) {
                int rest = lot.count - lot.sold;
                lot.proceeds += rest * Bargain.sellerFloor(Prices.emeraldsEach(lot.item)) * rate; // bought: it's the hub's
                lot.sold = lot.count;
                done = true;
            }
            double take = lot.proceeds * Dials.CONSIGN_COMMISSION.get();
            double net = lot.proceeds - take - lot.fare;
            if (net < rate && !done) {
                continue; // less than an emerald yet: it waits for more to sell
            }
            if (net > 0) {
                double have = Math.max(0, Treasury.available(level, hub.pos(), stock, rate)
                        - dev.b28.enterprise.territory.Territory.get(server).reserve(hub));
                if (have * rate < net) {
                    continue; // the hub's short just now: next time
                }
                Treasury.withdraw(level, hub.pos(), stock, net / rate, Mint.get(server), now);
                Post.get(server).remit(level, stock, hub, lot.town, net, now);
                remitted += net;
            }
            commission += take;
            lot.fare = Math.max(0, -net) > 0 ? -net : 0; // what the fare left owing, from the next of its sales
            lot.proceeds = 0;
            if (done && lot.fare <= 0) {
                it.remove();
            }
            setDirty();
        }
    }

    /** For tests: a booking, as read at the hub. */
    public void bookedForTest(GlobalPos hub, GlobalPos town, int stacks, double worth, long now) {
        booked(hub, town, stacks, worth, now);
    }

    /** For tests: a lot of the town's goods on consignment at the hub, its collection's fare in spurs. */
    public void lotForTest(GlobalPos hub, GlobalPos town, Item item, int count, double fare, long now) {
        Lot lot = new Lot();
        lot.hub = hub;
        lot.town = town;
        lot.item = item;
        lot.count = count;
        lot.fare = fare;
        lot.since = now;
        lots.add(lot);
    }

    /** For tests: what's been remitted and kept, in spurs. */
    public double[] totalsForTest() {
        return new double[] {remitted, commission};
    }

    /** The forwarder's report: bookings, collections due, lots, what it's remitted and earned. */
    public List<String> report(MinecraftServer server) {
        List<String> out = new ArrayList<>();
        out.add(("Forwarders: %d bookings, %d collections due, %d lots on consignment (%d items left by towns' wagons, "
                + "all told); %.0f spurs remitted, %.0f commission")
                .formatted(bookings.size(), collecting.size(), lots.size(), consigned, remitted, commission));
        for (Booking b : bookings) {
            out.add("  booked: %s with %s, %d stacks, %.0f emeralds of goods, since day %d".formatted(
                    dev.b28.enterprise.settlement.Names.label(b.town), dev.b28.enterprise.settlement.Names.label(b.hub),
                    b.stacks, b.worth, b.since / 24000));
        }
        for (Lot l : lots) {
            out.add("  lot: %s's %d %s at %s, %d sold".formatted(dev.b28.enterprise.settlement.Names.label(l.town),
                    l.count, BuiltInRegistries.ITEM.getKey(l.item).getPath(), dev.b28.enterprise.settlement.Names.label(l.hub),
                    l.sold));
        }
        return out;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag b = new ListTag();
        for (Booking x : bookings) {
            CompoundTag e = new CompoundTag();
            put(e, "hub", x.hub);
            put(e, "town", x.town);
            e.putInt("stacks", x.stacks);
            e.putDouble("worth", x.worth);
            e.putLong("since", x.since);
            b.add(e);
        }
        tag.put("bookings", b);
        ListTag l = new ListTag();
        for (Lot x : lots) {
            CompoundTag e = new CompoundTag();
            put(e, "hub", x.hub);
            put(e, "town", x.town);
            e.putString("item", BuiltInRegistries.ITEM.getKey(x.item).toString());
            e.putInt("count", x.count);
            e.putInt("sold", x.sold);
            e.putDouble("proceeds", x.proceeds);
            e.putDouble("fare", x.fare);
            e.putLong("since", x.since);
            l.add(e);
        }
        tag.put("lots", l);
        ListTag c = new ListTag();
        for (GlobalPos[] x : collecting) {
            CompoundTag e = new CompoundTag();
            put(e, "hub", x[0]);
            put(e, "town", x[1]);
            c.add(e);
        }
        tag.put("collecting", c);
        tag.putDouble("remitted", remitted);
        tag.putDouble("commission", commission);
        tag.putLong("consigned", consigned);
        return tag;
    }

    private static void put(CompoundTag tag, String key, GlobalPos pos) {
        GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, pos).ifSuccess(t -> tag.put(key, t));
    }

    private static GlobalPos pos(CompoundTag tag, String key) {
        return GlobalPos.CODEC.parse(NbtOps.INSTANCE, tag.get(key)).result().orElse(null);
    }

    public static Forwarder load(CompoundTag tag, HolderLookup.Provider registries) {
        Forwarder out = new Forwarder();
        for (Tag t : tag.getList("bookings", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            Booking x = new Booking();
            x.hub = pos(e, "hub");
            x.town = pos(e, "town");
            x.stacks = e.getInt("stacks");
            x.worth = e.getDouble("worth");
            x.since = e.getLong("since");
            if (x.hub != null && x.town != null) {
                out.bookings.add(x);
            }
        }
        for (Tag t : tag.getList("lots", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            Lot x = new Lot();
            x.hub = pos(e, "hub");
            x.town = pos(e, "town");
            x.item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(e.getString("item")));
            x.count = e.getInt("count");
            x.sold = e.getInt("sold");
            x.proceeds = e.getDouble("proceeds");
            x.fare = e.getDouble("fare");
            x.since = e.getLong("since");
            if (x.hub != null && x.town != null) {
                out.lots.add(x);
            }
        }
        for (Tag t : tag.getList("collecting", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            GlobalPos hub = pos(e, "hub");
            GlobalPos town = pos(e, "town");
            if (hub != null && town != null) {
                out.collecting.add(new GlobalPos[] {hub, town});
            }
        }
        out.remitted = tag.getDouble("remitted");
        out.commission = tag.getDouble("commission");
        out.consigned = tag.getLong("consigned");
        return out;
    }
}
