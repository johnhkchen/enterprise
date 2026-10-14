package dev.b28.enterprise.market;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.army.Army;
import dev.b28.enterprise.contract.Bonus;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.contract.Incoterm;
import dev.b28.enterprise.contract.Levy;
import dev.b28.enterprise.contract.Term;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.MoneyChanger;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.territory.Labour;
import dev.b28.enterprise.territory.Territory;
import dev.ithundxr.createnumismatics.Numismatics;
import dev.ithundxr.createnumismatics.content.backend.BankAccount;
import dev.ithundxr.createnumismatics.content.bank.CardItem;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import java.util.ArrayList;
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
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.saveddata.SavedData;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * A sovereign shifting the region onto a war footing, through the region's own institutions. Its capital is a desk
 * whose Blaze Banker is the royal treasury, which it mints coin into each day (debasing the spur). Mobilising, it
 * stockpiles kit: its capital's Merchant contracts towns for it at a premium, with bonus clauses for good enchantments,
 * and posts as wanted what no town can supply; towns answer by making it, buying it from their own villagers, and
 * putting idle hands to the trades it calls for, their smiths trading their way up to master (who sell diamond gear).
 * It buys gold through a Numismatics shop by the capital. At war it levies soldiers from each town by contract (paid a
 * day each, owed home), outfits them from the capital's warehouse, and replaces what the campaign wears out. At peace
 * the soldiers walk home and the minting stops.
 */
public final class Sovereign extends SavedData {
    private static final String NAME = "enterprise_sovereign";

    public enum Phase { PEACE, MOBILISING, WAR }

    /** Mobilising, it pays half again over worth; at war, two and a half times. */
    public static final double MOBILISING_PREMIUM = 1.5;
    public static final double WAR_PREMIUM = 2.5;
    /** It buys gold at three times its worth. */
    public static final double GOLD_PREMIUM = 3;
    /** A soldier is paid this much a day, owed to its town. */
    public static final double SOLDIER_PAY = 0.25;
    /** A town sends its idle hands and this share of the rest, at least one and at most four. */
    public static final double LEVY_SHARE = 0.2;
    public static final int LEVY_MOST = 4;

    /** The stockpile it builds while mobilising, and keeps up at war. */
    public static final Map<Item, Integer> STOCKPILE = stockpile();

    private static Map<Item, Integer> stockpile() {
        Map<Item, Integer> m = new LinkedHashMap<>();
        m.put(Items.DIAMOND_SWORD, 4);
        m.put(Items.IRON_SWORD, 12);
        m.put(Items.STONE_SWORD, 6);
        m.put(Items.DIAMOND_CHESTPLATE, 4);
        m.put(Items.IRON_CHESTPLATE, 12);
        m.put(Items.CHAINMAIL_CHESTPLATE, 6);
        m.put(Items.LEATHER_CHESTPLATE, 6);
        m.put(Items.SHIELD, 12);
        m.put(Items.BOW, 8);
        m.put(Items.ARROW, 256);
        m.put(Items.BREAD, 64); // and rations put by
        return m;
    }

    /** Bonus clauses on its contracts: quality is paid for, never required. */
    public static List<Term> bonuses(Item item, Army.Theatre theatre) {
        String path = BuiltInRegistries.ITEM.getKey(item).getPath();
        List<Term> terms = new ArrayList<>();
        if (path.endsWith("_sword")) {
            terms.add(new Bonus(Map.of("minecraft:unbreaking", 1, "minecraft:sharpness", 2), 25));
            if (theatre == Army.Theatre.UNDEAD) {
                terms.add(new Bonus(Map.of("minecraft:smite", 3), 30));
            }
        } else if (path.endsWith("_chestplate")) {
            terms.add(new Bonus(Map.of(theatre.armourEnchant, 2), 25));
            terms.add(new Bonus(Map.of("minecraft:unbreaking", 2), 15));
        } else if (item == Items.BOW) {
            terms.add(new Bonus(Map.of("minecraft:power", 2), 25));
            terms.add(new Bonus(Map.of("minecraft:unbreaking", 1), 10));
        } else if (item == Items.SHIELD) {
            terms.add(new Bonus(Map.of("minecraft:unbreaking", 1), 10));
        }
        return terms;
    }

    private Phase phase = Phase.PEACE;
    private GlobalPos capital;
    private UUID treasury;
    private int mintPerDay;
    private long lastDay = -1;
    private long phaseSince = -1;
    private final List<BlockPos> shops = new ArrayList<>();
    private final Map<String, Integer> received = new java.util.TreeMap<>(); // gold bought at the shop
    private final Map<Item, Integer> unmet = new LinkedHashMap<>(); // kit no town could be contracted for

    public static Sovereign get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Sovereign::new, Sovereign::load, null), NAME);
    }

    public Phase phase() {
        return phase;
    }

    public Optional<GlobalPos> desk() {
        return Optional.ofNullable(capital);
    }

    public Optional<BankAccount> treasury() {
        return treasury == null ? Optional.empty() : Optional.ofNullable(Numismatics.BANK.getAccount(treasury));
    }

    public List<BlockPos> shops() {
        return List.copyOf(shops);
    }

    public int mintPerDay() {
        return mintPerDay;
    }

    public Map<String, Integer> received() {
        return Map.copyOf(received);
    }

    /** What the war wants at this desk that no town could be contracted for: posted as wanted. */
    public Map<Item, Integer> unmet(GlobalPos desk) {
        return desk.equals(capital) ? Map.copyOf(unmet) : Map.of();
    }

    /**
     * Mobilisation: the capital's Blaze Banker becomes the royal treasury (a new account if it has none), minting
     * {@code perDay} spurs a day into it, and a shop goes up by the capital buying gold, paid from it.
     */
    public int mobilise(ServerLevel level, GlobalPos at, int perDay) {
        peace(level);
        capital = at;
        mintPerDay = perDay;
        treasury = MoneyChanger.bankAt(level, at.pos()).map(account -> account.id).orElseGet(() -> {
            UUID id = UUID.randomUUID();
            Numismatics.BANK.getOrCreateAccount(id, BankAccount.Type.PLAYER);
            return id;
        });
        phase = Phase.MOBILISING;
        phaseSince = level.getGameTime();
        lastDay = level.getGameTime(); // today's coin is this
        mint(level, level.getGameTime());
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        Block vendor = BuiltInRegistries.BLOCK.get(ResourceLocation.parse("numismatics:vendor"));
        Item card = BuiltInRegistries.ITEM.get(ResourceLocation.parse("numismatics:red_card"));
        BlockPos pos = at.pos().offset(0, 0, -10);
        level.setBlock(pos, vendor.defaultBlockState(), 3);
        if (level.getBlockEntity(pos) instanceof VendorBlockEntity shop) {
            shop.setMode(VendorBlockEntity.Mode.BUY);
            shop.filterContainer.setItem(0, new ItemStack(Items.GOLD_INGOT, 8));
            Shops.setPrice(shop, (int) Math.ceil(Prices.emeraldsEach(Items.GOLD_INGOT) * 8 * GOLD_PREMIUM * rate));
            ItemStack bound = new ItemStack(card);
            CardItem.set(bound, treasury);
            shop.cardContainer.setItem(0, bound);
            shop.setChanged();
            shops.add(pos);
        }
        setDirty();
        Enterprise.LOGGER.info("Mobilisation: the sovereign at {} mints {} spurs a day", at.pos().toShortString(), perDay);
        return shops.size();
    }

    /** War: each town it knows by a road is levied by contract, the copies riding with its caravans. Returns levies drafted. */
    public int declare(ServerLevel level, Army.Theatre theatre, long now) {
        if (capital == null) {
            return 0;
        }
        phase = Phase.WAR;
        phaseSince = now;
        MinecraftServer server = level.getServer();
        Army.get(server).setTheatre(theatre);
        ContractBook book = ContractBook.get(server);
        Territory territory = Territory.get(server);
        var settlements = dev.b28.enterprise.settlement.Settlements.get(server);
        int levies = 0;
        for (var route : dev.b28.enterprise.caravan.CaravanLedger.get(server).routes()) {
            if (!route.from().equals(capital)) {
                continue;
            }
            Optional<GlobalPos> bell = territory.bellOf(route.to());
            if (bell.isEmpty()) {
                continue;
            }
            var roster = settlements.roster(bell.get()).values();
            long idle = roster.stream().filter(r -> !r.baby() && Labour.idle(r.profession())).count();
            long rest = roster.stream().filter(r -> !r.baby() && !Labour.idle(r.profession())).count();
            int soldiers = (int) Math.max(1, Math.min(LEVY_MOST, idle + Math.round(rest * LEVY_SHARE)));
            book.draft(route.to(), capital, List.of(new Levy(soldiers, SOLDIER_PAY)), Incoterm.EXW, now, 24000, 1, now,
                    capital, false);
            levies++;
        }
        setDirty();
        Enterprise.LOGGER.info("War against {}: {} levies", theatre, levies);
        return levies;
    }

    /** Peace: the army walks home, the gold shop comes down, the minting stops. Returns the soldiers sent home. */
    public int peace(ServerLevel level) {
        int home = capital == null ? 0 : Army.get(level.getServer()).demobilise(level.getServer(), capital, level.getGameTime());
        shops.forEach(pos -> level.setBlock(pos, Blocks.AIR.defaultBlockState(), 3));
        shops.clear();
        mintPerDay = 0;
        phase = Phase.PEACE;
        phaseSince = level.getGameTime();
        unmet.clear();
        setDirty();
        return home;
    }

    /** A day's coin into the treasury, and the gold the shop bought sent to the mint. */
    void mint(ServerLevel level, long now) {
        for (BlockPos pos : shops) {
            if (level.isLoaded(pos) && level.getBlockEntity(pos) instanceof VendorBlockEntity shop) {
                for (int i = 0; i < shop.items.size(); i++) {
                    ItemStack stack = shop.items.get(i);
                    if (!stack.isEmpty()) {
                        received.merge(BuiltInRegistries.ITEM.getKey(stack.getItem()).getPath(), stack.getCount(),
                                Integer::sum);
                        shop.items.set(i, ItemStack.EMPTY);
                    }
                }
                shop.setChanged();
            }
        }
        treasury().ifPresent(account -> {
            account.deposit(mintPerDay);
            Mint.get(level.getServer()).mint(Mint.DEFAULT_FACTION, mintPerDay, now);
        });
        setDirty();
    }

    /** What it wants on hand at the capital now: the stockpile, and at war a set of kit for each soldier besides. */
    public Map<Item, Integer> targets(Army army) {
        Map<Item, Integer> targets = new LinkedHashMap<>(STOCKPILE);
        if (phase == Phase.WAR) {
            army.wants().forEach((item, n) -> targets.merge(item, n, Integer::sum));
        }
        return targets;
    }

    /**
     * Each day of mobilisation or war: mints, has its capital's Merchant contract towns for what the stockpile lacks
     * (at the premium, with bonus clauses), posts what no town can supply, and at war campaigns.
     */
    public static void onServerTick(ServerTickEvent.Post event) {
        MinecraftServer server = event.getServer();
        Sovereign sovereign = get(server);
        long now = server.overworld().getGameTime();
        if (sovereign.phase == Phase.PEACE || sovereign.capital == null) {
            return;
        }
        ServerLevel level = server.getLevel(sovereign.capital.dimension());
        if (level == null || !level.isLoaded(sovereign.capital.pos())) {
            return;
        }
        if (sovereign.lastDay >= 0 && now - sovereign.lastDay < 24000) {
            return;
        }
        sovereign.lastDay = now;
        sovereign.mint(level, now);
        Army army = Army.get(server);
        MerchantStock depot = MerchantStock.at(level, sovereign.capital.pos());
        double premium = sovereign.phase == Phase.WAR ? WAR_PREMIUM : MOBILISING_PREMIUM;
        Territory territory = Territory.get(server);
        sovereign.unmet.clear();
        for (Map.Entry<Item, Integer> target : sovereign.targets(army).entrySet()) {
            int need = target.getValue() - depot.count(new ItemCost(target.getKey(), 1));
            if (need <= 0) {
                continue;
            }
            boolean covered = Broker.source(level, sovereign.capital, territory, target.getKey(), need, now, premium,
                    bonuses(target.getKey(), army.theatre()));
            if (!covered) {
                sovereign.unmet.put(target.getKey(), need);
            }
        }
        if (sovereign.phase == Phase.WAR) {
            army.campaign(level, depot, sovereign.capital, now);
        }
        sovereign.setDirty();
    }

    /** Days in the current phase. */
    public double daysInPhase(long now) {
        return phaseSince < 0 ? 0 : (now - phaseSince) / 24000.0;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        tag.putString("phase", phase.name());
        if (capital != null) {
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, capital).ifSuccess(t -> tag.put("capital", t));
        }
        if (treasury != null) {
            tag.putUUID("treasury", treasury);
        }
        tag.putInt("mintPerDay", mintPerDay);
        tag.putLong("lastDay", lastDay);
        tag.putLong("phaseSince", phaseSince);
        tag.putLongArray("shops", shops.stream().mapToLong(BlockPos::asLong).toArray());
        CompoundTag got = new CompoundTag();
        received.forEach(got::putInt);
        tag.put("received", got);
        return tag;
    }

    public static Sovereign load(CompoundTag tag, HolderLookup.Provider registries) {
        Sovereign s = new Sovereign();
        s.phase = tag.contains("phase") ? Phase.valueOf(tag.getString("phase")) : Phase.PEACE;
        s.capital = tag.contains("capital")
                ? GlobalPos.CODEC.parse(NbtOps.INSTANCE, tag.get("capital")).result().orElse(null) : null;
        s.treasury = tag.hasUUID("treasury") ? tag.getUUID("treasury") : null;
        s.mintPerDay = tag.getInt("mintPerDay");
        s.lastDay = tag.contains("lastDay") ? tag.getLong("lastDay") : -1;
        s.phaseSince = tag.contains("phaseSince") ? tag.getLong("phaseSince") : -1;
        for (long pos : tag.getLongArray("shops")) {
            s.shops.add(BlockPos.of(pos));
        }
        CompoundTag got = tag.getCompound("received");
        got.getAllKeys().forEach(k -> s.received.put(k, got.getInt(k)));
        return s;
    }
}
