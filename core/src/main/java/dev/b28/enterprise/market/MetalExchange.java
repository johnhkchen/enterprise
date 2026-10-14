package dev.b28.enterprise.market;

import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.OffMap;
import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.territory.Territory;
import dev.ithundxr.createnumismatics.Numismatics;
import dev.ithundxr.createnumismatics.content.backend.BankAccount;
import dev.ithundxr.createnumismatics.content.bank.CardItem;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.saveddata.SavedData;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * The capital's metal exchange: the capital's Merchant keeping a stock of iron, coal, flint and diamonds (and gold, the
 * war chest) for the region, through the region's own trade. Each day, for each metal short of what it aims to hold, it
 * contracts the nearest market that holds some: a town with a mine first, and failing those a market across the sea
 * (off the map, at the end of its own road), whose metal the capital's caravans collect. It pays more the shorter it
 * is ({@link #PRESSURE}), so a mine near the capital has a buyer paying well when metal is scarce. Its prices stand as
 * buying shops at the port too (Numismatics vendors), for players. Mobilising it aims to hold twice as much, at war
 * three times: the war's demand reaches the mines through the price. What it holds, the capital's caravans carry to
 * towns that want it, as they carry anything.
 */
public final class MetalExchange extends SavedData {
    private static final String NAME = "enterprise_metal_exchange";

    /** A metal it keeps: the stock it aims to hold in peace, and what the market across the sea keeps of it (0: none). */
    public record Metal(Item item, int target, int overseas) {}

    public static final List<Metal> METALS = List.of(
            new Metal(Items.IRON_INGOT, 192, 512),
            new Metal(Items.COAL, 192, 512),
            new Metal(Items.FLINT, 96, 256),
            new Metal(Items.DIAMOND, 12, 32),
            new Metal(Items.GOLD_INGOT, 32, 0)); // gold the region's mines bring: the war chest

    /** How far its price moves with its stock: half again when empty, down to three quarters when over. */
    public static final double PRESSURE = 0.5;
    /** Its shops buy from players at this share of the price it pays towns. */
    public static final double SHOP_SHARE = 0.9;
    /** The market across the sea lies this far west of the capital. */
    public static final int OVERSEAS_DISTANCE = 2400;
    /** The metals its shops buy from players, a lot at a time. */
    public static final List<Item> SHOP_METALS = List.of(Items.IRON_INGOT, Items.COAL, Items.GOLD_INGOT);
    public static final int SHOP_LOT = 16;

    private GlobalPos desk;
    private UUID overseasAccount;
    private long lastDay = -1;
    private final Map<Item, Integer> fromPlayers = new LinkedHashMap<>();
    private final Map<Item, Integer> held = new LinkedHashMap<>(); // at its last count
    private final Map<Item, BlockPos> shops = new LinkedHashMap<>();

    public static MetalExchange get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(MetalExchange::new, MetalExchange::load, null), NAME);
    }

    public Optional<GlobalPos> desk() {
        return Optional.ofNullable(desk);
    }

    /** Is this the exchange's desk? */
    public boolean at(GlobalPos where) {
        return desk != null && desk.equals(where);
    }

    /** The market across the sea, given the capital. */
    public Optional<GlobalPos> overseas() {
        return desk().map(d -> GlobalPos.of(d.dimension(), d.pos().offset(-OVERSEAS_DISTANCE, 0, 0)));
    }

    public List<BlockPos> shops() {
        return List.copyOf(shops.values());
    }

    public int fromPlayers(Item item) {
        return fromPlayers.getOrDefault(item, 0);
    }

    /**
     * Opens the exchange at the capital's desk: the market across the sea (off the map, with an account of its own) and
     * the road to it, and the port's buying shops, paid from {@code treasury}.
     */
    public void open(ServerLevel level, GlobalPos at, UUID treasury) {
        MinecraftServer server = level.getServer();
        desk = at;
        lastDay = -1;
        if (overseasAccount == null) {
            overseasAccount = UUID.randomUUID();
            Numismatics.BANK.getOrCreateAccount(overseasAccount, BankAccount.Type.PLAYER);
        }
        GlobalPos far = overseas().orElseThrow();
        OffMap.get(server).open(far, "the market across the sea", overseasAccount, true);
        Warehouse.get(server).recordLevel(far, 5);
        var ledger = dev.b28.enterprise.caravan.CaravanLedger.get(server);
        if (ledger.route(at, far).isEmpty()) {
            ledger.addRoute(at, far);
        }
        dev.b28.enterprise.caravan.Atlas.get(server).learnBoth(at, far, level.getGameTime());
        Block vendor = BuiltInRegistries.BLOCK.get(ResourceLocation.parse("numismatics:vendor"));
        Item card = BuiltInRegistries.ITEM.get(ResourceLocation.parse("numismatics:red_card"));
        for (int i = 0; i < SHOP_METALS.size(); i++) {
            BlockPos pos = at.pos().offset(2 + 2 * i, 0, -10);
            level.setBlock(pos, vendor.defaultBlockState(), 3);
            if (level.getBlockEntity(pos) instanceof VendorBlockEntity shop) {
                shop.setMode(VendorBlockEntity.Mode.BUY);
                shop.filterContainer.setItem(0, new ItemStack(SHOP_METALS.get(i), SHOP_LOT));
                ItemStack bound = new ItemStack(card);
                CardItem.set(bound, treasury);
                shop.cardContainer.setItem(0, bound);
                shop.setChanged();
                shops.put(SHOP_METALS.get(i), pos);
            }
        }
        setDirty();
    }

    public static Optional<Metal> metal(Item item) {
        return METALS.stream().filter(m -> m.item() == item).findFirst();
    }

    /** The stock it aims to hold now: more while the sovereign mobilises, more again at war. */
    public static int target(Metal metal, Sovereign.Phase phase) {
        return metal.target() * switch (phase) {
            case PEACE -> 1;
            case MOBILISING -> 2;
            case WAR -> 3;
        };
    }

    public int held(Item item) {
        return held.getOrDefault(item, 0);
    }

    /** How much over its worth it pays for a metal: more the shorter it is. */
    public double premium(Metal metal, Sovereign.Phase phase) {
        int target = target(metal, phase);
        double gap = target <= 0 ? 0 : (target - held(metal.item())) / (double) target;
        return Math.max(0.75, Math.min(1 + PRESSURE, 1 + PRESSURE * gap));
    }

    public static void onServerTick(ServerTickEvent.Post event) {
        MinecraftServer server = event.getServer();
        MetalExchange exchange = get(server);
        if (exchange.desk == null) {
            return;
        }
        ServerLevel level = server.getLevel(exchange.desk.dimension());
        long now = server.overworld().getGameTime();
        if (level == null || !level.isLoaded(exchange.desk.pos())
                || exchange.lastDay >= 0 && now - exchange.lastDay < 24000) {
            return;
        }
        exchange.lastDay = now;
        exchange.day(level, now);
    }

    /**
     * A day at the exchange: the market across the sea makes its metal; what players sold the port's shops goes into
     * the capital's warehouse; and what the exchange is short of, it contracts for, nearest holder first.
     */
    public void day(ServerLevel level, long now) {
        MinecraftServer server = level.getServer();
        overseas().ifPresent(far -> produceOverseas(level, far, now));
        MerchantStock stock = MerchantStock.at(level, desk.pos());
        collectFromShops(level, stock);
        count(stock);
        Sovereign.Phase phase = Sovereign.get(server).phase();
        Territory territory = Territory.get(server);
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        for (Metal metal : METALS) {
            int need = target(metal, phase) - held(metal.item());
            double premium = premium(metal, phase);
            if (need > 0) {
                Broker.source(level, desk, territory, metal.item(), need, now, premium, List.of());
            }
            BlockPos at = shops.get(metal.item());
            if (at != null && level.isLoaded(at) && level.getBlockEntity(at) instanceof VendorBlockEntity shop) {
                // The port's shop: buys from players at a little under what it pays towns, while it's short.
                int price = (int) Math.ceil(Prices.emeraldsEach(metal.item()) * premium * SHOP_SHARE * SHOP_LOT * rate);
                Shops.setPrice(shop, need > 0 ? Math.max(1, price) : 0);
                shop.setChanged();
            }
        }
        setDirty();
    }

    /** The bread the market across the sea keeps for travellers: their food for the road home. */
    public static final int OVERSEAS_BREAD = 192;

    /**
     * Overseas, mines, forges and bakers work on: its warehouse is topped up to what it keeps of each metal, and bread
     * for caravans (the source).
     */
    static void produceOverseas(ServerLevel level, GlobalPos far, long now) {
        MerchantStock stock = MerchantStock.at(level, far.pos());
        for (int left = OVERSEAS_BREAD - stock.count(new ItemCost(Items.BREAD, 1)); left > 0; left -= 64) {
            stock.insert(new ItemStack(Items.BREAD, Math.min(64, left)));
        }
        for (Metal metal : METALS) {
            int short_ = metal.overseas() - stock.count(new ItemCost(metal.item(), 1));
            for (int left = short_; left > 0; left -= metal.item().getDefaultMaxStackSize()) {
                if (!stock.insert(new ItemStack(metal.item(), Math.min(left, metal.item().getDefaultMaxStackSize()))).isEmpty()) {
                    break;
                }
            }
        }
        Warehouse.get(level.getServer()).recordStocktake(far, stock.totals(), now); // what the capital's Merchant hears of it
    }

    /** What players sold the port's shops goes into the capital's warehouse. */
    void collectFromShops(ServerLevel level, MerchantStock stock) {
        shops.forEach((item, pos) -> {
            if (level.isLoaded(pos) && level.getBlockEntity(pos) instanceof VendorBlockEntity shop) {
                for (int i = 0; i < shop.items.size(); i++) {
                    ItemStack got = shop.items.get(i);
                    if (!got.isEmpty()) {
                        fromPlayers.merge(got.getItem(), got.getCount(), Integer::sum);
                        shop.items.set(i, stock.insert(got.copy()));
                    }
                }
                shop.setChanged();
            }
        });
    }

    /** Counts its metal in the capital's warehouse. */
    public void count(MerchantStock stock) {
        for (Metal metal : METALS) {
            held.put(metal.item(), stock.count(new ItemCost(metal.item(), 1)));
        }
        setDirty();
    }

    /** Metal still to come under its contracts. */
    public int coming(MinecraftServer server, Item item) {
        if (desk == null) {
            return 0;
        }
        return ContractBook.get(server).of(desk).stream()
                .filter(c -> c.buyer().equals(desk) && (c.state() == dev.b28.enterprise.contract.Contract.State.ACTIVE
                        || c.state() == dev.b28.enterprise.contract.Contract.State.DRAFTED))
                .mapToInt(c -> c.delivery().filter(d -> d.item() == item)
                        .map(d -> d.quantity() * c.installments() - c.delivered()).orElse(0))
                .sum();
    }

    /** For tests: what it holds of a metal. */
    public void setHeldForTest(Item item, int n) {
        held.put(item, n);
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        if (desk != null) {
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> tag.put("desk", t));
        }
        if (overseasAccount != null) {
            tag.putUUID("overseasAccount", overseasAccount);
        }
        tag.putLong("lastDay", lastDay);
        tag.put("fromPlayers", items(fromPlayers));
        tag.put("held", items(held));
        CompoundTag shopTag = new CompoundTag();
        shops.forEach((item, pos) -> shopTag.putLong(BuiltInRegistries.ITEM.getKey(item).toString(), pos.asLong()));
        tag.put("shops", shopTag);
        return tag;
    }

    private static CompoundTag items(Map<Item, Integer> map) {
        CompoundTag tag = new CompoundTag();
        map.forEach((item, n) -> tag.putInt(BuiltInRegistries.ITEM.getKey(item).toString(), n));
        return tag;
    }

    private static void readItems(CompoundTag tag, Map<Item, Integer> into) {
        tag.getAllKeys().forEach(k -> into.put(BuiltInRegistries.ITEM.get(ResourceLocation.parse(k)), tag.getInt(k)));
    }

    public static MetalExchange load(CompoundTag tag, HolderLookup.Provider registries) {
        MetalExchange exchange = new MetalExchange();
        exchange.desk = tag.contains("desk")
                ? GlobalPos.CODEC.parse(NbtOps.INSTANCE, tag.get("desk")).result().orElse(null) : null;
        exchange.overseasAccount = tag.hasUUID("overseasAccount") ? tag.getUUID("overseasAccount") : null;
        exchange.lastDay = tag.contains("lastDay") ? tag.getLong("lastDay") : -1;
        readItems(tag.getCompound("fromPlayers"), exchange.fromPlayers);
        readItems(tag.getCompound("held"), exchange.held);
        CompoundTag shopTag = tag.getCompound("shops");
        shopTag.getAllKeys().forEach(k -> exchange.shops.put(BuiltInRegistries.ITEM.get(ResourceLocation.parse(k)),
                BlockPos.of(shopTag.getLong(k))));
        return exchange;
    }
}
