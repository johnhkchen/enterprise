package dev.b28.enterprise.market;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.caravan.Caravans;
import dev.b28.enterprise.caravan.Commissions;
import dev.b28.enterprise.caravan.Desires;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.money.Treasury;
import dev.b28.enterprise.territory.Comforts;
import dev.b28.enterprise.territory.Territory;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
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
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * The town's shop: up to {@link #SIZE} Numismatics vendors touching its Merchant's desk (built when the town can pay
 * for them), where the Merchant shows the town what it's after. A good the town wants and hasn't got is listed out of
 * stock, at what the town would pay for it now (see {@link Desires}): the price floats with how short the town is and
 * what its people can afford. The listing is honest: it stands only while the Merchant is procuring the good (its
 * caravans shop for it, or its lockers take it, with a road for it to come by) and has the money for a lot of it.
 * When the good comes in, the shop sells it at that price and the takings go to the treasury.
 *
 * The listings are news: a caravan calling here reads them, and Merchants elsewhere value the good at the price their
 * caravans last saw (nothing crosses distance without a carrier).
 */
public final class ShopBoard extends SavedData {
    private static final String NAME = "enterprise_shop_board";

    public static final int SIZE = 3;
    /** A vendor's brass and glass, in emeralds. */
    public static final double VENDOR_EMERALDS = 4;
    /** The board is looked over at most this often. */
    private static final long EVERY = 1200;

    /** What stands for a kind of comfort the town has none of: the good its Merchant lists. */
    private static final Map<String, Item> STANDS_FOR = Map.of(
            "treats", Items.COOKIE, "reading", Items.BOOK, "clothing", Items.LEATHER_BOOTS,
            "furnishings", Items.WHITE_CARPET, "leisure", dev.b28.enterprise.trade.Wares.PIPE_TOBACCO.get());

    /** A listing: the good, the lot, its price in spurs; whether it was out of stock when listed. */
    public record Listing(Item item, int lot, int spurs, boolean wanted) {
        public double emeraldsEach(double rate) {
            return spurs / rate / lot;
        }
    }

    private final Map<GlobalPos, List<BlockPos>> vendors = new HashMap<>(); // the board's own, by desk
    private final Map<GlobalPos, Map<Item, Double>> seen = new HashMap<>(); // emeralds each, as a caravan last saw
    private final Map<GlobalPos, Long> looked = new HashMap<>();
    private final Map<GlobalPos, Map<BlockPos, Listing>> listings = new HashMap<>();

    public static ShopBoard get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(ShopBoard::new, ShopBoard::load, null), NAME);
    }

    /** Is this vendor one of the board's (so the broker leaves it be)? */
    public boolean isBoards(GlobalPos desk, BlockPos vendor) {
        return vendors.getOrDefault(desk, List.of()).contains(vendor);
    }

    /** The price a caravan last saw listed at {@code desk} for {@code item}, in emeralds each. */
    public Optional<Double> seen(GlobalPos desk, Item item) {
        return Optional.ofNullable(seen.getOrDefault(desk, Map.of()).get(item));
    }

    /** A caravan calls: what the shop lists is news it carries. */
    public void sighted(GlobalPos desk, double rate) {
        Map<Item, Double> prices = new HashMap<>();
        listings.getOrDefault(desk, Map.of()).values().forEach(l -> prices.put(l.item(), l.emeraldsEach(rate)));
        seen.put(desk, prices);
        setDirty();
    }

    public Map<BlockPos, Listing> listings(GlobalPos desk) {
        return Map.copyOf(listings.getOrDefault(desk, Map.of()));
    }

    /** When the market is touched: the takings banked, the board built if it can be, and its listings set. */
    public void keep(ServerLevel level, GlobalPos desk, MerchantStock stock, long now, Territory territory) {
        if (now - looked.getOrDefault(desk, Long.MIN_VALUE / 2) < EVERY || territory.bellOf(desk).isEmpty()) {
            return;
        }
        looked.put(desk, now);
        MinecraftServer server = level.getServer();
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        List<VendorBlockEntity> own = ownVendors(level, desk);
        for (VendorBlockEntity shop : own) {
            int takings = Shops.collect(shop);
            if (takings > 0) {
                Treasury.deposit(level, desk.pos(), stock, takings);
            }
        }
        List<Listing> wants = wants(level, desk, stock, now, rate, territory);
        while (own.size() < Math.min(SIZE, wants.size())) {
            var built = build(level, desk, stock, now, territory);
            if (built.isEmpty()) {
                break;
            }
            own.add(built.get());
        }
        Map<BlockPos, Listing> listed = new LinkedHashMap<>();
        for (int i = 0; i < own.size(); i++) {
            VendorBlockEntity shop = own.get(i);
            if (i >= wants.size()) {
                Shops.offer(shop, ItemStack.EMPTY, 0); // nothing wanted: the shop stands empty
                Shops.clearOthers(shop).forEach(stock::insert);
                continue;
            }
            Listing listing = wants.get(i);
            if (!shop.getFilterItem().is(listing.item()) || shop.getFilterItem().getCount() != listing.lot()) {
                Shops.offer(shop, new ItemStack(listing.item(), listing.lot()), listing.spurs());
                Shops.clearOthers(shop).forEach(back -> {
                    ItemStack rest = stock.insert(back);
                    if (!rest.isEmpty()) {
                        Shops.stock(shop, rest);
                    }
                });
            } else {
                Shops.setPrice(shop, listing.spurs()); // the price floats
            }
            // What's come in goes on the shelf at that price: a stack's worth, beyond what the town keeps for itself.
            int onShelf = shop.items.stream().filter(s -> s.is(listing.item())).mapToInt(ItemStack::getCount).sum();
            int spare = Math.max(0, stock.count(new ItemCost(listing.item(), 1)) - keepFor(territory, desk, listing.item()));
            int top = Math.min(listing.item().getDefaultMaxStackSize() - onShelf, spare);
            if (top > 0) {
                for (ItemStack goods : stock.take(new ItemCost(listing.item(), 1), top)) {
                    ItemStack rest = Shops.stock(shop, goods);
                    if (!rest.isEmpty()) {
                        stock.insert(rest);
                    }
                }
            }
            listed.put(shop.getBlockPos(), listing);
        }
        listings.put(desk, listed);
        setDirty();
    }

    /** What the town keeps of a comfort for its own villagers: the kind's par, in that good. */
    private static int keepFor(Territory territory, GlobalPos desk, Item item) {
        if (Comforts.kindOf(item).isEmpty()) {
            return Integer.MAX_VALUE; // a good its lockers use: none for sale
        }
        return (int) Math.ceil(territory.comfortPar(desk) / Math.max(1e-9, Prices.emeraldsEach(item)));
    }

    /**
     * What the town is after and hasn't got, most wanted first, at what it would pay: each kind of comfort it's short
     * of and holds none of, and each good its lockers take that it holds none of; only while its Merchant is truly
     * procuring it (on its caravans' shopping list or its lockers' wants, with a road for it to come by) and can pay
     * for a lot.
     */
    public List<Listing> wants(ServerLevel level, GlobalPos desk, MerchantStock stock, long now, double rate, Territory territory) {
        MinecraftServer server = level.getServer();
        boolean road = CaravanLedger.get(server).routes().stream()
                .anyMatch(r -> r.from().equals(desk) || r.to().equals(desk));
        if (!road) {
            return List.of();
        }
        double money = Math.max(0, Treasury.available(level, desk.pos(), stock, rate) - territory.reserve(desk))
                + territory.bellOf(desk).map(b -> territory.households(b)[0]).orElse(0.0);
        Map<String, Double> shopping = Commissions.shoppingList(territory, desk, stock);
        Map<String, Double> held = Comforts.onHand(stock);
        List<Map.Entry<Listing, Double>> ranked = new ArrayList<>();
        for (Map.Entry<String, Double> want : shopping.entrySet()) {
            if (held.getOrDefault(want.getKey(), 0.0) > 0 || !STANDS_FOR.containsKey(want.getKey())) {
                continue; // it has some: the kind isn't missing
            }
            Item item = STANDS_FOR.get(want.getKey());
            add(ranked, item, Desires.reckon(server, desk, item, now), want.getValue(), money, rate);
        }
        for (Item item : Caravans.wantedAt(server, desk)) {
            if (stock.count(new ItemCost(item, 1)) == 0 && Comforts.kindOf(item).isEmpty()) {
                add(ranked, item, Prices.emeraldsEach(item), Prices.emeraldsEach(item) * 64, money, rate);
            }
        }
        ranked.sort(Comparator.comparingDouble((Map.Entry<Listing, Double> e) -> -e.getValue()));
        return ranked.stream().map(Map.Entry::getKey).limit(SIZE).toList();
    }

    private static void add(List<Map.Entry<Listing, Double>> ranked, Item item, double each, double worthShort,
            double money, double rate) {
        if (each <= 0) {
            return;
        }
        int lot = (int) Math.max(1, Math.min(item.getDefaultMaxStackSize(), Math.ceil(2 / (each * rate))));
        int spurs = Math.max(1, (int) Math.ceil(each * lot * rate));
        if (money * rate < spurs) {
            return; // it couldn't pay for a lot: no honest listing
        }
        ranked.add(Map.entry(new Listing(item, lot, spurs, true), worthShort));
    }

    /** The board's vendors standing by the desk (loaded), forgetting any that are gone. */
    private List<VendorBlockEntity> ownVendors(ServerLevel level, GlobalPos desk) {
        List<VendorBlockEntity> out = new ArrayList<>();
        List<BlockPos> kept = new ArrayList<>();
        for (BlockPos pos : vendors.getOrDefault(desk, List.of())) {
            if (level.getBlockEntity(pos) instanceof VendorBlockEntity vendor) {
                out.add(vendor);
                kept.add(pos);
            }
        }
        vendors.put(desk, kept);
        return out;
    }

    /** A vendor built touching the desk, on open floor, paid from the treasury; none if there's no room or money. */
    private Optional<VendorBlockEntity> build(ServerLevel level, GlobalPos desk, MerchantStock stock, long now,
            Territory territory) {
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, now);
        if (Treasury.available(level, desk.pos(), stock, rate) - territory.reserve(desk) < VENDOR_EMERALDS) {
            return Optional.empty();
        }
        Block vendor = BuiltInRegistries.BLOCK.get(ResourceLocation.parse("numismatics:vendor"));
        for (int dx = -Shops.OWN; dx <= Shops.OWN; dx++) {
            for (int dz = -Shops.OWN; dz <= Shops.OWN; dz++) {
                BlockPos pos = desk.pos().offset(dx, 0, dz);
                if ((dx == 0 && dz == 0) || !level.isLoaded(pos) || !level.getBlockState(pos).isAir()
                        || level.getBlockState(pos.below()).isAir()) {
                    continue;
                }
                level.setBlock(pos, vendor.defaultBlockState(), 3);
                if (level.getBlockEntity(pos) instanceof VendorBlockEntity shop) {
                    shop.setMode(VendorBlockEntity.Mode.SELL);
                    Treasury.withdraw(level, desk.pos(), stock, VENDOR_EMERALDS, Mint.get(level.getServer()), now);
                    vendors.computeIfAbsent(desk, d -> new ArrayList<>()).add(pos);
                    setDirty();
                    Enterprise.LOGGER.debug("Market at {} builds a shop at {}", desk.pos().toShortString(), pos.toShortString());
                    return Optional.of(shop);
                }
            }
        }
        return Optional.empty();
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        Set<GlobalPos> desks = new java.util.HashSet<>(vendors.keySet());
        desks.addAll(seen.keySet());
        for (GlobalPos desk : desks) {
            CompoundTag e = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> e.put("desk", t));
            e.putLongArray("vendors", vendors.getOrDefault(desk, List.of()).stream().mapToLong(BlockPos::asLong).toArray());
            CompoundTag prices = new CompoundTag();
            seen.getOrDefault(desk, Map.of()).forEach((item, each) ->
                    prices.putDouble(BuiltInRegistries.ITEM.getKey(item).toString(), each));
            e.put("seen", prices);
            list.add(e);
        }
        tag.put("desks", list);
        return tag;
    }

    public static ShopBoard load(CompoundTag tag, HolderLookup.Provider registries) {
        ShopBoard out = new ShopBoard();
        for (Tag t : tag.getList("desks", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("desk")).result().ifPresent(desk -> {
                List<BlockPos> at = new ArrayList<>();
                for (long packed : e.getLongArray("vendors")) {
                    at.add(BlockPos.of(packed));
                }
                out.vendors.put(desk, at);
                Map<Item, Double> prices = new HashMap<>();
                CompoundTag p = e.getCompound("seen");
                p.getAllKeys().forEach(id -> prices.put(BuiltInRegistries.ITEM.get(ResourceLocation.parse(id)), p.getDouble(id)));
                out.seen.put(desk, prices);
            });
        }
        return out;
    }
}
