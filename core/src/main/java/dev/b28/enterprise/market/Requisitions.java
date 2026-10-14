package dev.b28.enterprise.market;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.army.Army;
import dev.b28.enterprise.contract.Advance;
import dev.b28.enterprise.contract.Contract;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.contract.Delivery;
import dev.b28.enterprise.contract.Term;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.territory.Territory;
import dev.ithundxr.createnumismatics.content.bank.CardItem;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.StringTag;
import net.minecraft.nbt.Tag;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Requisitions: how the crown's procurement agent buys, from its capital. Each day it posts what it wants for the
 * coming period (so much of a good, by a day, at no more than a price) on the capital's board, and puts up a buying
 * shop for each (a Numismatics vendor, paid from the treasury) that anyone may sell into at that price. The board's
 * news rides with caravans: a town's Merchant who reads it bids what its town can supply by the day (its spare stock,
 * and what its people can make meanwhile) at a price of its own (its cost, and half the way to the crown's price). After
 * {@link #BID_DAYS} the agent takes the best offers, cheapest first, until the want is covered: each a contract for the
 * whole lot, a third paid up front, its papers going back with the next caravan. A good may be wanted as a kind (meat,
 * counted in food points: any kind will do).
 */
public final class Requisitions extends SavedData {
    private static final String NAME = "enterprise_requisitions";

    /** A requisition is for this many days' wants, and due at the end of them. */
    public static final int PERIOD_DAYS = 5;
    /** Bids are taken for this long after it's posted. */
    public static final int BID_DAYS = 2;
    /** A town making goods to order (rather than selling from stock) asks this much more. */
    public static final double MAKE_MARKUP = 0.25;
    /** A Merchant bids its cost and this share of the way to the crown's price. */
    public static final double BID_SHARE = 0.5;
    /** The crown's price: the good's worth, the suppliers' margin and carriage, times the phase's premium. */
    public static final double CEILING_CARRIAGE = 1.1;
    /** Its buying shops stand in a row this far north of the desk, two blocks apart. */
    public static final int SHOP_ROW = -14;

    /** A seller's offer: so many of a good, at so much each, filed on a day. */
    public record Bid(GlobalPos seller, Item item, int count, double each, long filed) {}

    /** A posted want: its buyer, the goods any of which will do, how much is wanted (and still open), by when. */
    public static final class Requisition {
        final int id;
        final GlobalPos buyer;
        final List<Item> goods;
        final boolean byPoints; // counted in food points (a kind of food), else in items
        int wanted;
        int open;
        final double premium;
        final long posted;
        final long due;
        final List<Bid> bids = new ArrayList<>();
        boolean awarded;
        final List<BlockPos> shops = new ArrayList<>();

        Requisition(int id, GlobalPos buyer, List<Item> goods, boolean byPoints, int wanted, double premium, long posted,
                long due) {
            this.id = id;
            this.buyer = buyer;
            this.goods = List.copyOf(goods);
            this.byPoints = byPoints;
            this.wanted = wanted;
            this.open = wanted;
            this.premium = premium;
            this.posted = posted;
            this.due = due;
        }

        /** What one of {@code item} counts toward it: its food points, or one. */
        public int unit(Item item) {
            return byPoints ? Math.max(1, Army.nutrition(item)) : 1;
        }

        /** The crown's price for one of {@code item}: no bid above it is taken, and its shops pay it. */
        public double ceiling(Item item) {
            return Prices.emeraldsEach(item) * (1 + Broker.SUPPLIER_MARGIN) * CEILING_CARRIAGE * premium;
        }

        public int id() { return id; }
        public List<Item> goods() { return goods; }
        public int wanted() { return wanted; }
        public int open() { return open; }
        public List<Bid> bids() { return List.copyOf(bids); }
        public boolean awarded() { return awarded; }
        public long due() { return due; }

        String describe() {
            String what = byPoints ? wanted + " food points of " + goods.size() + " kinds"
                    : wanted + " " + BuiltInRegistries.ITEM.getKey(goods.get(0)).getPath();
            return "#%d %s by day %d".formatted(id, what, due / 24000);
        }
    }

    private final List<Requisition> requisitions = new ArrayList<>();
    private int nextId = 1;
    // what's been awarded and swept in, for the record: contracts, items bought at its shops
    private int contracts;
    private int boughtAtShops;

    public static Requisitions get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Requisitions::new, Requisitions::load, null), NAME);
    }

    public List<Requisition> all() {
        return List.copyOf(requisitions);
    }

    public int contracts() {
        return contracts;
    }

    public int boughtAtShops() {
        return boughtAtShops;
    }

    /** Open requisitions posted on {@code board}'s board (the buyer's own). */
    public List<Requisition> on(GlobalPos board) {
        return requisitions.stream().filter(r -> r.buyer.equals(board) && r.open > 0).toList();
    }

    /** What's still open, in units, of the buyer's requisitions for this good. */
    public int open(GlobalPos buyer, List<Item> goods) {
        return requisitions.stream().filter(r -> r.buyer.equals(buyer) && r.goods.equals(goods)).mapToInt(r -> r.open)
                .sum();
    }

    /** Has the buyer's want of this good gone a whole bidding window without an offer? */
    public boolean unanswered(GlobalPos buyer, List<Item> goods) {
        return requisitions.stream().anyMatch(r -> r.buyer.equals(buyer) && r.goods.equals(goods) && r.awarded
                && r.open == r.wanted);
    }

    /** Is this one of the requisitions' buying shops? (Its own Merchant doesn't fill them: they buy for the crown.) */
    public boolean isShop(BlockPos pos) {
        return requisitions.stream().anyMatch(r -> r.shops.contains(pos));
    }

    /**
     * Posts a want: {@code units} of the goods (in food points if {@code byPoints}), for the coming period, at the
     * phase's {@code premium}; tops up one still taking bids instead of posting another. Its shops go up by the desk,
     * paid from {@code treasury}.
     */
    public Requisition post(ServerLevel level, GlobalPos buyer, List<Item> goods, boolean byPoints, int units,
            double premium, UUID treasury, long now) {
        for (Requisition r : requisitions) {
            if (r.buyer.equals(buyer) && r.goods.equals(goods) && !r.awarded) {
                r.wanted += units;
                r.open += units;
                setDirty();
                return r;
            }
        }
        Requisition r = new Requisition(nextId++, buyer, goods, byPoints, units, premium, now,
                now + PERIOD_DAYS * 24000L);
        requisitions.add(r);
        openShops(level, r, treasury, now);
        Exchange.get(level.getServer()).note(buyer, now, "requisitioned " + r.describe());
        setDirty();
        return r;
    }

    /**
     * The requisition's buying shops by the desk: one for each good (two at most for a kind), its lot a stack (or one
     * piece of kit), at the crown's price, paid from the treasury's card.
     */
    void openShops(ServerLevel level, Requisition r, UUID treasury, long now) {
        if (treasury == null) {
            return;
        }
        Block vendor = BuiltInRegistries.BLOCK.get(ResourceLocation.parse("numismatics:vendor"));
        Item card = BuiltInRegistries.ITEM.get(ResourceLocation.parse("numismatics:red_card"));
        double rate = dev.b28.enterprise.money.Mint.get(level.getServer())
                .rate(dev.b28.enterprise.money.Mint.DEFAULT_FACTION, now);
        for (Item item : r.goods.subList(0, Math.min(2, r.goods.size()))) {
            Optional<BlockPos> free = freeSpot(level, r.buyer.pos());
            if (free.isEmpty()) {
                return;
            }
            BlockPos pos = free.get();
            level.setBlock(pos, vendor.defaultBlockState(), 3);
            if (level.getBlockEntity(pos) instanceof VendorBlockEntity shop) {
                int lot = Math.min(16, item.getDefaultMaxStackSize());
                shop.setMode(VendorBlockEntity.Mode.BUY);
                shop.filterContainer.setItem(0, new ItemStack(item, lot));
                Shops.setPrice(shop, Math.max(1, (int) Math.floor(r.ceiling(item) * lot * rate)));
                ItemStack bound = new ItemStack(card);
                CardItem.set(bound, treasury);
                shop.cardContainer.setItem(0, bound);
                shop.setChanged();
                r.shops.add(pos);
            }
        }
    }

    private Optional<BlockPos> freeSpot(ServerLevel level, BlockPos desk) {
        for (int i = 0; i < 12; i++) {
            BlockPos pos = desk.offset(-10 + 2 * i, 0, SHOP_ROW);
            boolean taken = requisitions.stream().anyMatch(r -> r.shops.contains(pos));
            if (!taken && level.getBlockState(pos).isAir()) {
                return Optional.of(pos);
            }
        }
        return Optional.empty();
    }

    /**
     * What's been sold into the requisitions' shops goes into the buyer's warehouse, and counts toward them. Returns
     * the items swept in.
     */
    public int sweep(ServerLevel level, MerchantStock depot) {
        int swept = 0;
        for (Requisition r : requisitions) {
            for (BlockPos pos : r.shops) {
                if (!level.isLoaded(pos) || !(level.getBlockEntity(pos) instanceof VendorBlockEntity shop)) {
                    continue;
                }
                for (int i = 0; i < shop.items.size(); i++) {
                    ItemStack stack = shop.items.get(i);
                    if (stack.isEmpty() || !depot.hasRoomFor(stack)) {
                        continue;
                    }
                    r.open = Math.max(0, r.open - stack.getCount() * r.unit(stack.getItem()));
                    swept += stack.getCount();
                    depot.insert(stack.copy());
                    shop.items.set(i, ItemStack.EMPTY);
                }
                shop.setChanged();
            }
        }
        if (swept > 0) {
            boughtAtShops += swept;
            setDirty();
        }
        return swept;
    }

    /** Requisitions past their day (or fully met) close, their shops coming down. Returns those closed. */
    public List<Requisition> lapse(ServerLevel level, long now) {
        List<Requisition> closed = requisitions.stream().filter(r -> now >= r.due || r.open <= 0 && r.awarded).toList();
        for (Requisition r : closed) {
            r.shops.forEach(pos -> {
                if (level.isLoaded(pos) && level.getBlockEntity(pos) instanceof VendorBlockEntity shop) {
                    shop.items.clear(); // swept already; nothing left to lose
                }
                level.setBlock(pos, Blocks.AIR.defaultBlockState(), 3);
            });
        }
        requisitions.removeAll(closed);
        if (!closed.isEmpty()) {
            setDirty();
        }
        return closed;
    }

    /**
     * A caravan between two markets: {@code reader}'s Merchant reads {@code board}'s requisitions and bids on those its
     * town can supply. Returns bids filed.
     */
    public int read(ServerLevel level, GlobalPos reader, GlobalPos board, long now) {
        int filed = 0;
        for (Requisition r : on(board)) {
            if (r.awarded || r.buyer.equals(reader) || r.bids.stream().anyMatch(b -> b.seller.equals(reader))) {
                continue;
            }
            Optional<Bid> bid = bid(level, reader, r, now);
            if (bid.isPresent()) {
                r.bids.add(bid.get());
                filed++;
                Exchange.get(level.getServer()).note(reader, now, "bid on %s's requisition %s: %d %s at %.3f each"
                        .formatted(board.pos().toShortString(), r.describe(), bid.get().count(),
                                BuiltInRegistries.ITEM.getKey(bid.get().item()).getPath(), bid.get().each()));
            }
        }
        if (filed > 0) {
            setDirty();
        }
        return filed;
    }

    /**
     * What {@code seller}'s Merchant offers: of the goods its town can supply most of, what it can spare and what its
     * people can make before the day (less the road), at its cost (more for what's made) and half the way to the
     * crown's price. Empty if it can't supply any, or not under the crown's price.
     */
    public static Optional<Bid> bid(ServerLevel level, GlobalPos seller, Requisition r, long now) {
        MinecraftServer server = level.getServer();
        Warehouse warehouse = Warehouse.get(server);
        Territory territory = Territory.get(server);
        Bid best = null;
        for (Item item : r.goods) {
            Optional<Broker.Means> found = Broker.means(level, r.buyer, seller, item, now);
            if (found.isEmpty()) {
                continue;
            }
            Broker.Means means = found.get();
            double road = dev.b28.enterprise.caravan.Caravans.travelTicks(means.route().distance()) / 24000.0;
            double days = Math.max(0, (r.due - now) / 24000.0 - road - 1);
            int perDay = !means.makes() ? 0 : Math.max(warehouse.offered(seller, item), Broker.makerLot(item) / 2);
            int made = (int) (perDay * days);
            int unit = r.unit(item);
            int count = Math.min((r.open + unit - 1) / unit, means.spare() + made);
            if (!means.trusted()) {
                count = Math.min(count, Broker.TRIAL_LOT); // a trial lot, to rebuild the record
            }
            if (count <= 0) {
                continue;
            }
            double madeShare = Math.max(0, count - means.spare()) / (double) count;
            double cost = Broker.landed(territory, item, means.route(), means.incoterm(), 1) * (1 + MAKE_MARKUP * madeShare);
            double ceiling = r.ceiling(item);
            if (cost > ceiling) {
                continue;
            }
            double each = cost + BID_SHARE * (ceiling - cost);
            Bid bid = new Bid(seller, item, count, each, now);
            if (best == null || bid.count * unit > best.count * r.unit(best.item)) {
                best = bid;
            }
        }
        return Optional.ofNullable(best);
    }

    /**
     * The bidding over: the agent takes the best offers on {@code buyer}'s requisitions, cheapest a unit first (the
     * more trusted first at a price), each a contract for the lot (as much as is still wanted), due on the
     * requisition's day, a third paid up front where they trust each other, with the crown's bonus clauses. Returns
     * contracts drafted.
     */
    public int award(ServerLevel level, GlobalPos buyer, Army.Theatre theatre, long now) {
        MinecraftServer server = level.getServer();
        ContractBook book = ContractBook.get(server);
        int drafted = 0;
        for (Requisition r : requisitions) {
            if (!r.buyer.equals(buyer) || r.awarded || now < r.posted + BID_DAYS * 24000L) {
                continue;
            }
            r.awarded = true;
            List<Bid> offers = new ArrayList<>(r.bids);
            offers.sort(Comparator.comparingDouble((Bid b) -> b.each / r.unit(b.item))
                    .thenComparingDouble(b -> -book.trust(b.seller, buyer, now)));
            for (Bid bid : offers) {
                int unit = r.unit(bid.item);
                int n = Math.min(bid.count, (r.open + unit - 1) / unit);
                Optional<Broker.Means> means = Broker.means(level, buyer, bid.seller, bid.item, now);
                if (n <= 0 || means.isEmpty()) {
                    continue;
                }
                List<Term> terms = new ArrayList<>();
                terms.add(new Delivery(bid.item, n, bid.each));
                terms.addAll(Sovereign.bonuses(bid.item, theatre));
                if (means.get().trusted()) {
                    terms.add(new Advance(Broker.MAKER_ADVANCE)); // the papers go back with money to start with
                }
                Contract contract = book.draft(bid.seller, buyer, terms, means.get().incoterm(), r.due, 24000, 1, now,
                        buyer, false);
                r.open = Math.max(0, r.open - n * unit);
                drafted++;
                contracts++;
                Exchange.get(server).note(buyer, now, "requisition %s: took %s's offer of %d %s at %.3f each (#%d)"
                        .formatted(r.describe(), bid.seller.pos().toShortString(), n,
                                BuiltInRegistries.ITEM.getKey(bid.item).getPath(), bid.each, contract.id()));
            }
            Enterprise.LOGGER.debug("Requisition {}: {} offers, {} units still open", r.describe(), offers.size(), r.open);
        }
        if (drafted > 0) {
            setDirty();
        }
        return drafted;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        for (Requisition r : requisitions) {
            CompoundTag e = new CompoundTag();
            e.putInt("id", r.id);
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, r.buyer).ifSuccess(t -> e.put("buyer", t));
            ListTag goods = new ListTag();
            r.goods.forEach(item -> goods.add(StringTag.valueOf(BuiltInRegistries.ITEM.getKey(item).toString())));
            e.put("goods", goods);
            e.putBoolean("byPoints", r.byPoints);
            e.putInt("wanted", r.wanted);
            e.putInt("open", r.open);
            e.putDouble("premium", r.premium);
            e.putLong("posted", r.posted);
            e.putLong("due", r.due);
            e.putBoolean("awarded", r.awarded);
            e.putLongArray("shops", r.shops.stream().mapToLong(BlockPos::asLong).toArray());
            ListTag bids = new ListTag();
            for (Bid b : r.bids) {
                CompoundTag bt = new CompoundTag();
                GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, b.seller).ifSuccess(t -> bt.put("seller", t));
                bt.putString("item", BuiltInRegistries.ITEM.getKey(b.item).toString());
                bt.putInt("count", b.count);
                bt.putDouble("each", b.each);
                bt.putLong("filed", b.filed);
                bids.add(bt);
            }
            e.put("bids", bids);
            list.add(e);
        }
        tag.put("requisitions", list);
        tag.putInt("nextId", nextId);
        tag.putInt("contracts", contracts);
        tag.putInt("boughtAtShops", boughtAtShops);
        return tag;
    }

    public static Requisitions load(CompoundTag tag, HolderLookup.Provider registries) {
        Requisitions out = new Requisitions();
        for (Tag t : tag.getList("requisitions", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            var buyer = GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("buyer")).result();
            if (buyer.isEmpty()) {
                continue;
            }
            List<Item> goods = new ArrayList<>();
            e.getList("goods", Tag.TAG_STRING).forEach(g -> goods.add(BuiltInRegistries.ITEM.get(
                    ResourceLocation.parse(g.getAsString()))));
            Requisition r = new Requisition(e.getInt("id"), buyer.get(), goods, e.getBoolean("byPoints"),
                    e.getInt("wanted"), e.getDouble("premium"), e.getLong("posted"), e.getLong("due"));
            r.open = e.getInt("open");
            r.awarded = e.getBoolean("awarded");
            for (long pos : e.getLongArray("shops")) {
                r.shops.add(BlockPos.of(pos));
            }
            for (Tag bt : e.getList("bids", Tag.TAG_COMPOUND)) {
                CompoundTag b = (CompoundTag) bt;
                GlobalPos.CODEC.parse(NbtOps.INSTANCE, b.get("seller")).result().ifPresent(seller -> r.bids.add(new Bid(
                        seller, BuiltInRegistries.ITEM.get(ResourceLocation.parse(b.getString("item"))),
                        b.getInt("count"), b.getDouble("each"), b.getLong("filed"))));
            }
            out.requisitions.add(r);
        }
        out.nextId = Math.max(1, tag.getInt("nextId"));
        out.contracts = tag.getInt("contracts");
        out.boughtAtShops = tag.getInt("boughtAtShops");
        return out;
    }
}
