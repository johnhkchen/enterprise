package dev.b28.enterprise.trade;

import dev.b28.enterprise.caravan.Atlas;
import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.OffMap;
import dev.b28.enterprise.merchant.Warehouse;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.saveddata.SavedData;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * Realms beyond the border: other peoples, off the map, who trade with ours. Each has a market of its own off the map
 * (its warehouse on the ledger, its money in an account), makes what our villagers can't (wares, see {@link Wares}),
 * and wants some of what ours make. Its Merchants send their own caravans and fleets to the markets of ours they know,
 * on the same roads and with the same dealings as any caravan: they sell what our towns want, buy what home wants on
 * the way back, and pay for what they eat. Crossing the border into the crown's realm, they pay its customs (see
 * {@link #CUSTOMS}) on what they sell.
 *
 * The Coast, across the sea west of the capital (the market across the sea): its fleets land at the capital's port with
 * pipe tobacco and leaf, and take home grain and paper. The March, overland beyond the eastern hills: its caravans bring
 * leather and wool to the eastern towns (for their Librarians' books, Shepherds' carpets, Leatherworkers' boots), and
 * take home bread and wheat.
 */
public final class Realms extends SavedData {
    private static final String NAME = "enterprise_realms";

    /** Of each good a realm wants, its people use this many a day (and so it keeps buying). */
    public static final int USES_PER_DAY = 64;
    /** Its warehouse aims to hold this many days of each. */
    private static final long WANT_KEY = -42; // the realm's own "workstation" in the warehouse's capacity book

    /** Customs the crown takes on what a foreign trader sells inside the realm, of its takings. */
    public static final double CUSTOMS = 0.1;

    /** What a realm makes: so many a day, up to what it keeps on hand. */
    public record Make(Item item, int perDay, int keep) {}

    /** A realm: its name, its market off the map, what it makes and wants, and the markets of ours it trades with. */
    public static final class Realm {
        final String id;
        final String name;
        final GlobalPos home;
        final List<Make> makes;
        final Set<Item> wants;
        final List<GlobalPos> partners = new ArrayList<>();
        long lastDay = -1;
        long customsPaid; // spurs, in all
        long goodsMade;

        Realm(String id, String name, GlobalPos home, List<Make> makes, Set<Item> wants) {
            this.id = id;
            this.name = name;
            this.home = home;
            this.makes = makes;
            this.wants = wants;
        }

        public String name() {
            return name;
        }

        public GlobalPos home() {
            return home;
        }

        public Set<Item> wants() {
            return wants;
        }
    }

    private final Map<String, Realm> realms = new LinkedHashMap<>();

    public static Realms get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Realms::new, Realms::load, null), NAME);
    }

    /** The two realms: what each makes and wants. */
    static Realm define(String id, GlobalPos home) {
        return switch (id) {
            case "coast" -> new Realm(id, "the Coast", home, List.of(
                    new Make(Wares.PIPE_TOBACCO.get(), 32, 256), new Make(Wares.TOBACCO_LEAF.get(), 48, 384),
                    new Make(Items.COCOA_BEANS, 16, 128), new Make(Items.BREAD, 64, 192)),
                    Set.of(Items.WHEAT, Items.PAPER, Items.IRON_INGOT, Items.BOOK));
            case "march" -> new Realm(id, "the March", home, List.of(
                    new Make(Items.LEATHER, 24, 192), new Make(Items.WHITE_WOOL, 24, 192),
                    new Make(Items.BREAD, 48, 192)),
                    Set.of(Items.WHEAT, Items.BREAD, Items.CARROT, Items.IRON_INGOT));
            default -> throw new IllegalArgumentException("no realm called " + id);
        };
    }

    /**
     * Opens a realm's market at {@code home} (off the map), with {@code capital} spurs in its account, and its roads
     * to {@code partners}: its traders set out from there.
     */
    public Realm open(ServerLevel level, String id, BlockPos home, int capital, List<GlobalPos> partners) {
        MinecraftServer server = level.getServer();
        GlobalPos at = GlobalPos.of(level.dimension(), home);
        Realm realm = realms.computeIfAbsent(id, k -> define(id, at));
        UUID account = OffMap.get(server).market(at).flatMap(OffMap.Market::account).orElseGet(UUID::randomUUID);
        var bank = dev.ithundxr.createnumismatics.Numismatics.BANK.getOrCreateAccount(account,
                dev.ithundxr.createnumismatics.content.backend.BankAccount.Type.PLAYER);
        if (OffMap.get(server).market(at).isEmpty()) {
            OffMap.get(server).open(at, realm.name, account, true);
        }
        bank.deposit(capital);
        Warehouse.get(server).recordLevel(at, 4);
        // A country's demand, not a stall's: so many a day of each good it wants.
        Map<Item, Integer> perDay = new LinkedHashMap<>();
        realm.wants.forEach(item -> perDay.put(item, USES_PER_DAY));
        Warehouse.get(server).recordCapacity(at, WANT_KEY, perDay);
        CaravanLedger ledger = CaravanLedger.get(server);
        for (GlobalPos partner : partners) {
            if (!realm.partners.contains(partner)) {
                realm.partners.add(partner);
            }
            if (ledger.route(at, partner).isEmpty()) {
                ledger.addRoute(at, partner);
            }
            Atlas.get(server).learnBoth(at, partner, level.getGameTime());
        }
        make(level, realm, 3); // a few days' goods on hand when it opens
        realm.lastDay = level.getGameTime();
        setDirty();
        return realm;
    }

    public Optional<Realm> of(GlobalPos home) {
        return realms.values().stream().filter(r -> r.home.equals(home)).findFirst();
    }

    public boolean isForeign(GlobalPos desk) {
        return of(desk).isPresent();
    }

    public List<Realm> all() {
        return List.copyOf(realms.values());
    }

    /** A foreign trader sold {@code spurs} inside the realm: the crown's customs on it, to its treasury. Returns the duty. */
    public int customs(MinecraftServer server, GlobalPos home, int spurs) {
        Optional<Realm> realm = of(home);
        if (realm.isEmpty() || spurs <= 0) {
            return 0;
        }
        int duty = (int) Math.round(spurs * CUSTOMS);
        var treasury = dev.b28.enterprise.market.Sovereign.get(server).treasury();
        if (duty <= 0 || treasury.isEmpty()) {
            return 0;
        }
        treasury.get().deposit(duty);
        realm.get().customsPaid += duty;
        setDirty();
        return duty;
    }

    public static void onServerTick(ServerTickEvent.Post event) {
        MinecraftServer server = event.getServer();
        long now = server.overworld().getGameTime();
        if (now % 1200 != 0) {
            return;
        }
        Realms realms = get(server);
        for (Realm realm : realms.realms.values()) {
            ServerLevel level = server.getLevel(realm.home.dimension());
            if (level == null || realm.lastDay < 0 || now - realm.lastDay < 24000) {
                continue;
            }
            int days = (int) ((now - realm.lastDay) / 24000);
            realm.lastDay += days * 24000L;
            make(level, realm, Math.min(days, 5));
            realms.setDirty();
        }
    }

    /** The realm's people at work: {@code days} of what it makes, up to what it keeps on hand. */
    static void make(ServerLevel level, Realm realm, int days) {
        MerchantStock stock = MerchantStock.at(level, realm.home.pos());
        for (Item want : realm.wants) { // its people use what it bought
            stock.take(new ItemCost(want, 1), USES_PER_DAY * days);
        }
        for (Make make : realm.makes) {
            int held = stock.count(new ItemCost(make.item(), 1));
            int add = Math.min(make.perDay() * days, make.keep() - held);
            for (int left = add; left > 0; ) {
                int batch = Math.min(left, make.item().getDefaultMaxStackSize());
                ItemStack rest = stock.insert(new ItemStack(make.item(), batch));
                int put = batch - rest.getCount();
                left -= put;
                realm.goodsMade += put;
                if (!rest.isEmpty()) {
                    break;
                }
            }
        }
    }

    /** Each realm's trade: its routes' trips and takings, and the customs it paid. */
    public List<String> report(MinecraftServer server) {
        List<String> lines = new ArrayList<>();
        CaravanLedger ledger = CaravanLedger.get(server);
        for (Realm realm : realms.values()) {
            long trips = 0;
            long sales = 0;
            long bought = 0;
            for (var route : ledger.routes()) {
                if (route.from().equals(realm.home)) {
                    trips += route.trips();
                    sales += route.salesSpurs();
                    bought += route.purchaseSpurs();
                }
            }
            int balance = OffMap.get(server).market(realm.home).flatMap(OffMap.Market::account)
                    .map(dev.ithundxr.createnumismatics.Numismatics.BANK::getAccount).map(a -> a.getBalance()).orElse(0);
            lines.add("%s: %d trips to %d markets, %d spurs sold, %d bought, %d customs paid; %d goods made; %d spurs in hand"
                    .formatted(realm.name, trips, realm.partners.size(), sales, bought, realm.customsPaid, realm.goodsMade,
                            balance));
        }
        return lines;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        for (Realm realm : realms.values()) {
            CompoundTag e = new CompoundTag();
            e.putString("id", realm.id);
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, realm.home).ifSuccess(t -> e.put("home", t));
            ListTag partners = new ListTag();
            realm.partners.forEach(p -> GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, p).ifSuccess(partners::add));
            e.put("partners", partners);
            e.putLong("lastDay", realm.lastDay);
            e.putLong("customsPaid", realm.customsPaid);
            e.putLong("goodsMade", realm.goodsMade);
            list.add(e);
        }
        tag.put("realms", list);
        return tag;
    }

    public static Realms load(CompoundTag tag, HolderLookup.Provider registries) {
        Realms out = new Realms();
        for (Tag t : tag.getList("realms", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("home")).result().ifPresent(home -> {
                Realm realm = define(e.getString("id"), home);
                e.getList("partners", Tag.TAG_COMPOUND).forEach(p ->
                        GlobalPos.CODEC.parse(NbtOps.INSTANCE, p).result().ifPresent(realm.partners::add));
                realm.lastDay = e.getLong("lastDay");
                realm.customsPaid = e.getLong("customsPaid");
                realm.goodsMade = e.getLong("goodsMade");
                out.realms.put(realm.id, realm);
            });
        }
        return out;
    }
}
