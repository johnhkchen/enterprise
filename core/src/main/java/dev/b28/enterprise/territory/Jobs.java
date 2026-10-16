package dev.b28.enterprise.territory;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.MerchantBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.TradeKeys;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.money.Treasury;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.phys.AABB;

/**
 * Work for a town's idle hands. A town with an unemployed adult (never a baby) puts up a work stall, a workstation
 * with a chest against it (so it's a locker), for a trade drawn from a bag of marbles (see {@link #bag}): the more of
 * the region's demand a trade would meet, the likelier it is. The villager takes the job at once and its selling trades are switched on,
 * so the town earns from what it grows and what caravans bring.
 */
public final class Jobs {
    /** A stall's workstation, chest and labour, in emeralds. */
    public static final double STALL_EMERALDS = 6;

    /** A trade a town can set up: its profession, workstation, and the goods its villagers buy. */
    public record Trade(VillagerProfession profession, Block workstation, List<Item> buys) {}

    public static final List<Trade> TRADES = List.of(
            new Trade(VillagerProfession.FARMER, Blocks.COMPOSTER,
                    List.of(Items.WHEAT, Items.POTATO, Items.CARROT, Items.BEETROOT, Items.PUMPKIN, Items.MELON)),
            new Trade(VillagerProfession.LIBRARIAN, Blocks.LECTERN, List.of(Items.PAPER, Items.BOOK, Items.INK_SAC)),
            new Trade(VillagerProfession.CARTOGRAPHER, Blocks.CARTOGRAPHY_TABLE,
                    List.of(Items.PAPER, Items.GLASS_PANE, Items.COMPASS)),
            new Trade(VillagerProfession.FLETCHER, Blocks.FLETCHING_TABLE,
                    List.of(Items.STICK, Items.FLINT, Items.STRING, Items.FEATHER)),
            new Trade(VillagerProfession.FISHERMAN, Blocks.BARREL, List.of(Items.STRING, Items.COAL)),
            new Trade(VillagerProfession.SHEPHERD, Blocks.LOOM, List.of(Items.WHITE_WOOL)),
            new Trade(VillagerProfession.CLERIC, Blocks.BREWING_STAND,
                    List.of(Items.ROTTEN_FLESH, Items.GOLD_INGOT, Items.GLASS_BOTTLE, Items.NETHER_WART)),
            new Trade(VillagerProfession.BUTCHER, Blocks.SMOKER,
                    List.of(Items.CHICKEN, Items.PORKCHOP, Items.RABBIT, Items.COAL, Items.SWEET_BERRIES)),
            new Trade(VillagerProfession.LEATHERWORKER, Blocks.CAULDRON, List.of(Items.LEATHER, Items.FLINT)),
            new Trade(VillagerProfession.MASON, Blocks.STONECUTTER, List.of(Items.CLAY_BALL, Items.STONE)),
            new Trade(VillagerProfession.ARMORER, Blocks.BLAST_FURNACE, List.of(Items.COAL, Items.IRON_INGOT)),
            new Trade(VillagerProfession.TOOLSMITH, Blocks.SMITHING_TABLE, List.of(Items.COAL, Items.IRON_INGOT, Items.FLINT)),
            new Trade(VillagerProfession.WEAPONSMITH, Blocks.GRINDSTONE, List.of(Items.COAL, Items.IRON_INGOT, Items.FLINT)));

    /** Stalls are 3 x 3 (a block of clearance around), at least this far from the bell. */
    private static final int STALL_SIZE = 3;
    private static final int NEAREST = 6;

    private Jobs() {}

    /**
     * What the town has to sell, by item id, in emeralds: 4 days of its farms' output and what its warehouse holds
     * (each good's local value).
     */
    public static Map<String, Double> supply(Map<String, Double> farmPerDay, MerchantStock stock) {
        Map<String, Double> worth = new java.util.HashMap<>();
        farmPerDay.forEach((id, perDay) -> worth.merge(id, perDay * 4 / Prices.itemsPerEmerald(id), Double::sum));
        stock.totals().forEach((id, count) -> worth.merge(id, count / Prices.itemsPerEmerald(id), Double::sum));
        return worth;
    }

    /** The worth of the town's supply a trade's villagers would buy, in emeralds. */
    static double buys(Trade trade, Map<String, Double> supply) {
        return trade.buys().stream()
                .mapToDouble(item -> supply.getOrDefault(BuiltInRegistries.ITEM.getKey(item).toString(), 0.0)).sum();
    }

    /** Idle plots must yield at least this much a day, in emeralds, to put Farmers' marbles in the bag. */
    public static final double FARMER_IDLE_EMERALDS = 0.5;

    /**
     * The bag of marbles an idle hand draws its trade from: each trade's marbles are the emeralds of demand it would
     * meet. What its villagers would buy of the town's supply (shared with those of the trade already there); goods
     * that markets the town knows have posted as wanted ({@code signals}, by the trade that would supply them); four
     * days of what its untended plots would yield (a Farmer's); and the halls it wants and could pay for now (a
     * Mason's, until it has one: see {@link dev.b28.enterprise.service.BuildServices#work}). The likeliest trade is
     * the one most wanted, not the only one.
     */
    public static Map<Trade, Double> bag(Map<String, Double> supply, Map<VillagerProfession, Integer> present,
            Map<String, Double> idlePerDay, double buildWork, Map<Trade, Double> signals) {
        Map<Trade, Double> bag = new java.util.LinkedHashMap<>();
        for (Trade trade : TRADES) {
            double buys = buys(trade, supply) / (1 + present.getOrDefault(trade.profession(), 0));
            if (buys > 0) {
                bag.merge(trade, buys, Double::sum);
            }
        }
        signals.forEach((trade, emeralds) -> bag.merge(trade, emeralds, Double::sum));
        double idle = idlePerDay.entrySet().stream()
                .mapToDouble(e -> e.getValue() / Prices.itemsPerEmerald(e.getKey())).sum();
        if (idle >= FARMER_IDLE_EMERALDS) {
            bag.merge(trade(VillagerProfession.FARMER), idle * 4, Double::sum);
        }
        if (buildWork > 0 && !present.containsKey(VillagerProfession.MASON)) {
            bag.merge(trade(VillagerProfession.MASON), buildWork, Double::sum);
        }
        return bag;
    }

    /**
     * Marbles for the comforts the town lacks: each kind short of its ration puts in a share of what its households
     * have saved (what they'd spend on it, could they get it), for the trade whose workshop makes it (carpets a
     * Shepherd's, boots a Leatherworker's), if the town has none of that trade. A town nobody brings comforts makes its
     * own.
     */
    public static Map<Trade, Double> comfortMarbles(Map<String, Double> met, double savings,
            Map<VillagerProfession, Integer> present) {
        Map<Trade, Double> marbles = new java.util.LinkedHashMap<>();
        double share = dev.b28.enterprise.Dials.COMFORT_MARBLES.get();
        for (Workshop.Conversion c : Workshop.CONVERSIONS) {
            var kind = Comforts.kindOf(c.output());
            if (kind.isEmpty()) {
                continue;
            }
            double lacking = 1 - met.getOrDefault(Comforts.name(kind.get()), 0.0);
            if (lacking <= 0.01) {
                continue;
            }
            for (String id : c.trades()) {
                TRADES.stream().filter(t -> BuiltInRegistries.VILLAGER_PROFESSION.getKey(t.profession()).toString().equals(id)
                                && !present.containsKey(t.profession()))
                        .findFirst().ifPresent(t -> marbles.merge(t, lacking * savings * share, Double::sum));
            }
        }
        return marbles;
    }

    /** The markets' signals, and the comforts the town lacks (see {@link #comfortMarbles}). */
    static Map<Trade, Double> withComforts(Map<Trade, Double> signals, Territory territory, GlobalPos bell,
            Map<VillagerProfession, Integer> present) {
        Map<Trade, Double> all = new java.util.LinkedHashMap<>(signals);
        comfortMarbles(territory.comfortsMet(bell), territory.households(bell)[0], present)
                .forEach((t, m) -> all.merge(t, m, Double::sum));
        return all;
    }

    /** A marble drawn from the bag: each trade as likely as its share of the marbles. None from an empty bag. */
    public static Optional<Trade> draw(Map<Trade, Double> bag, net.minecraft.util.RandomSource random) {
        double total = bag.values().stream().mapToDouble(Double::doubleValue).sum();
        if (total <= 0) {
            return Optional.empty();
        }
        double marble = random.nextDouble() * total;
        Trade last = null;
        for (Map.Entry<Trade, Double> e : bag.entrySet()) {
            last = e.getKey();
            marble -= e.getValue();
            if (marble < 0) {
                return Optional.of(e.getKey());
            }
        }
        return Optional.ofNullable(last);
    }

    private static Trade trade(VillagerProfession profession) {
        return TRADES.stream().filter(t -> t.profession() == profession).findFirst().orElseThrow();
    }

    /**
     * When counted while loaded: an unemployed adult of the town (on its census roster) gets a stall for the best trade, if there's open
     * ground for it and {@link #STALL_EMERALDS} to spare beyond the reserve. One a count.
     */
    public static Optional<Villager> employ(ServerLevel level, BlockPos bell, GlobalPos desk, MerchantStock stock,
            Territory territory) {
        var residents = dev.b28.enterprise.settlement.Settlements.get(level.getServer())
                .roster(GlobalPos.of(level.dimension(), bell)).keySet();
        Optional<Villager> idle = level.getEntitiesOfClass(Villager.class, new AABB(bell).inflate(48),
                        v -> !v.isBaby() && v.getVillagerData().getProfession() == VillagerProfession.NONE
                                && residents.contains(v.getUUID()))
                .stream().findFirst();
        // With no one idle: a hand whose trade has done nothing for the town for days may take up one it needs.
        boolean retraining = idle.isEmpty();
        if (retraining) {
            idle = slack(level, desk, residents);
        }
        if (idle.isEmpty()) {
            return Optional.empty();
        }
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        double cash = Treasury.available(level, desk.pos(), stock, rate) - territory.reserve(desk);
        // Short of coin for the stall, the Merchant may borrow its households' savings.
        if (cash + territory.depositsOnCall(desk) < STALL_EMERALDS) {
            return Optional.empty();
        }
        // The town's trades from its roster: everyone, loaded or not.
        Map<VillagerProfession, Integer> present = new java.util.HashMap<>();
        dev.b28.enterprise.settlement.Settlements.get(level.getServer()).roster(GlobalPos.of(level.dimension(), bell))
                .values().stream().filter(r -> !r.baby()).forEach(r -> BuiltInRegistries.VILLAGER_PROFESSION.getOptional(
                        net.minecraft.resources.ResourceLocation.parse(r.profession()))
                        .ifPresent(p -> present.merge(p, 1, Integer::sum)));
        GlobalPos bellPos = GlobalPos.of(level.dimension(), bell);
        // What it has signed for comes first; otherwise the hand draws a marble.
        Optional<Trade> trade = forContracts(level, desk, territory, bellPos, present.keySet())
                .or(() -> draw(bag(supply(territory.farmPlan(bellPos), stock), present, territory.idlePlots(bellPos),
                        dev.b28.enterprise.service.BuildServices.work(level, desk, stock, bellPos, territory, rate),
                        withComforts(signals(level, desk, territory, bellPos, present.keySet()), territory, bellPos,
                                present)), level.random));
        if (retraining && (trade.isEmpty() || present.containsKey(trade.get().profession()))) {
            return Optional.empty(); // nothing it needs that the town hasn't: the hand keeps its trade
        }
        Optional<BlockPos> site = trade.isPresent() ? Growth.site(level, bell, STALL_SIZE, NEAREST) : Optional.empty();
        // Out of its own money, the accountant's say; on its households' savings, theirs.
        if (site.isEmpty() || cash >= STALL_EMERALDS && !dev.b28.enterprise.money.Accountant.approve(level, desk, stock, "a stall",
                STALL_EMERALDS, level.getGameTime())) {
            return Optional.empty();
        }
        if (retraining) {
            Enterprise.LOGGER.info("{}: its {} has done nothing for the town for {} days, and takes up the {}'s trade",
                    dev.b28.enterprise.settlement.Names.label(desk),
                    BuiltInRegistries.VILLAGER_PROFESSION.getKey(idle.get().getVillagerData().getProfession()).getPath(),
                    dev.b28.enterprise.Dials.SLACK_DAYS.getInt(),
                    BuiltInRegistries.VILLAGER_PROFESSION.getKey(trade.get().profession()).getPath());
            leaveTrade(level, idle.get(), desk);
        }
        if (cash < STALL_EMERALDS) {
            territory.borrowDeposits(level, desk, stock, STALL_EMERALDS - cash);
        }
        BlockPos workstation = buildStall(level, site.get(), trade.get());
        hire(level, idle.get(), workstation, trade.get(), desk);
        Treasury.withdraw(level, desk.pos(), stock, STALL_EMERALDS, Mint.get(level.getServer()), level.getGameTime());
        Enterprise.LOGGER.debug("Settlement at {} puts up a {} stall at {}", bell.toShortString(),
                BuiltInRegistries.VILLAGER_PROFESSION.getKey(trade.get().profession()), workstation.toShortString());
        return idle;
    }

    /**
     * A trade the town's contracts call for and it hasn't: for goods it has contracted to sell, what makes or sells
     * them, and what that needs before it (iron goods want an Armorer to smelt, and a Toolsmith's mine for the ore).
     */
    static Optional<Trade> forContracts(ServerLevel level, GlobalPos desk, Territory territory, GlobalPos bell,
            Set<VillagerProfession> present) {
        var book = dev.b28.enterprise.contract.ContractBook.get(level.getServer());
        java.util.List<Item> wanted = new java.util.ArrayList<>();
        // A trade it has contracted to train someone in, and has no one to train (none of it, or all already at the
        // level: a contract for a second master wants a second smith): an idle hand takes it up.
        var roster = dev.b28.enterprise.settlement.Settlements.get(level.getServer()).roster(bell).values();
        for (var contract : book.trainingFor(desk)) {
            String profession = contract.training().get().profession();
            int target = contract.training().get().level();
            boolean has = roster.stream().anyMatch(r -> !r.baby() && r.profession().equals(profession)
                    && r.level() < target);
            Optional<Trade> trade = TRADES.stream().filter(t -> BuiltInRegistries.VILLAGER_PROFESSION
                    .getKey(t.profession()).toString().equals(profession)).findFirst();
            if (!has && trade.isPresent()) {
                return trade;
            }
        }
        for (var contract : book.of(desk)) {
            var delivery = contract.delivery();
            if (delivery.isPresent() && contract.seller().equals(desk) && contract.inForce()
                    && book.toDeliver(desk, delivery.get().item()) > 0) {
                wanted.add(delivery.get().item());
            }
        }
        return tradeFor(wanted, territory, bell, present);
    }

    /**
     * Goods markets this town knows have posted as wanted (no market it knows has them), by the trade that would
     * supply them (or the first it lacks of what that takes), in emeralds: opportunities for its idle hands.
     */
    static Map<Trade, Double> signals(ServerLevel level, GlobalPos desk, Territory territory, GlobalPos bell,
            Set<VillagerProfession> present) {
        var server = level.getServer();
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        Map<Trade, Double> marbles = new java.util.LinkedHashMap<>();
        for (GlobalPos market : dev.b28.enterprise.caravan.Atlas.get(server).known(server, desk)) {
            for (var signal : dev.b28.enterprise.market.Exchange.get(server).signals(market)) {
                tradeFor(List.of(signal.item()), territory, bell, present).ifPresent(trade ->
                        marbles.merge(trade, (double) signal.count() * signal.spursEach() / rate, Double::sum));
            }
        }
        return marbles;
    }

    /** The first trade, for the first of these goods the town can't yet make or sell, that it should take on. */
    static Optional<Trade> tradeFor(java.util.List<Item> goods, Territory territory, GlobalPos bell,
            Set<VillagerProfession> present) {
        Set<String> have = new java.util.HashSet<>();
        present.forEach(p -> have.add(BuiltInRegistries.VILLAGER_PROFESSION.getKey(p).toString()));
        for (Item item : goods) {
            Set<String> makers = dev.b28.enterprise.market.Procurement.suppliers(item);
            if (makers.stream().anyMatch(have::contains)) {
                continue;
            }
            java.util.List<String> chain = new java.util.ArrayList<>();
            boolean metal = item == Items.IRON_SWORD || item == Items.IRON_CHESTPLATE || item == Items.SHIELD
                    || item == Items.GOLD_INGOT || item == Items.IRON_INGOT;
            boolean ore = item == Items.COAL || item == Items.RAW_IRON || item == Items.RAW_GOLD;
            if ((metal || ore) && territory.mine(bell).isEmpty()) {
                chain.add("minecraft:toolsmith"); // a mine
            }
            if (metal) {
                chain.add("minecraft:armorer");
            }
            chain.addAll(makers);
            for (String needed : chain) {
                if (have.contains(needed)) {
                    continue;
                }
                Optional<Trade> trade = TRADES.stream()
                        .filter(t -> BuiltInRegistries.VILLAGER_PROFESSION.getKey(t.profession()).toString().equals(needed))
                        .findFirst();
                if (trade.isPresent()) {
                    return trade;
                }
            }
        }
        return Optional.empty();
    }

    /** A stall in a 3 x 3 plot: the workstation in the middle, a chest against its east side. Returns the workstation. */
    public static BlockPos buildStall(ServerLevel level, BlockPos corner, Trade trade) {
        BlockPos workstation = corner.offset(1, 1, 1);
        level.setBlock(workstation, trade.workstation().defaultBlockState(), 3);
        level.setBlock(workstation.east(), Blocks.CHEST.defaultBlockState(), 3);
        return workstation;
    }

    /**
     * The villager takes the job at once (waiting for it to find the workstation is chancy in an unloaded town), and
     * the trades where it buys goods for emeralds are switched on for the desk's Merchant.
     */
    /** Trades whose work isn't in their locker's trades (fields, the desk, halls, blessings): never slack by them. */
    static final Set<VillagerProfession> KEPT = Set.of(VillagerProfession.FARMER, VillagerProfession.MASON,
            VillagerProfession.CLERIC, VillagerProfession.NONE, VillagerProfession.NITWIT);

    /**
     * In a town with no roads, a resident whose trade has done nothing for it at the Merchant's last {@code slack_days}
     * reviews (its locker's trades worth nothing to it: no goods of the town's to buy, nothing it wants to sell), if any.
     */
    static Optional<Villager> slack(ServerLevel level, GlobalPos desk, Set<java.util.UUID> residents) {
        // Only in a town no caravan comes to or goes from: elsewhere a trade's work may be its workshop's, a
        // service, or goods carried away, none of which shows in its locker's trades (measured: retraining
        // everywhere cost towns with roads up to a point of mood).
        if (dev.b28.enterprise.caravan.CaravanLedger.get(level.getServer()).routes().stream()
                .anyMatch(r -> !r.stopping() && (r.from().equals(desk) || r.to().equals(desk)))) {
            return Optional.empty();
        }
        MerchantBook book = MerchantBook.get(level.getServer());
        int days = dev.b28.enterprise.Dials.SLACK_DAYS.getInt();
        for (var locker : dev.b28.enterprise.locker.Lockers.near(level, desk.pos(),
                dev.b28.enterprise.locker.Lockers.MERCHANT_RANGE, Integer.MAX_VALUE)) {
            if (book.slackDays(desk, locker.workstation()) < days) {
                continue;
            }
            var villager = dev.b28.enterprise.merchant.Merchant.workingAt(level, locker.workstation(),
                    dev.b28.enterprise.locker.Lockers.MERCHANT_RANGE);
            if (villager.isPresent() && !villager.get().isBaby() && residents.contains(villager.get().getUUID())
                    && !KEPT.contains(villager.get().getVillagerData().getProfession())
                    && !dev.b28.enterprise.merchant.Merchant.isMerchant(villager.get())) {
                return villager;
            }
        }
        return Optional.empty();
    }

    /** A villager leaves its trade: its stall taken down, its trades and what it knew of them gone. */
    static void leaveTrade(ServerLevel level, Villager villager, GlobalPos desk) {
        MerchantBook book = MerchantBook.get(level.getServer());
        villager.getBrain().getMemory(MemoryModuleType.JOB_SITE).ifPresent(site -> {
            for (String key : Set.copyOf(book.enabledTrades(desk, site.pos()))) {
                book.setEnabled(desk, site.pos(), key, false);
            }
            book.noteSlack(desk, site.pos(), false);
            level.getPoiManager().release(site.pos());
            level.setBlock(site.pos(), Blocks.AIR.defaultBlockState(), 3);
        });
        villager.getBrain().eraseMemory(MemoryModuleType.JOB_SITE);
        villager.getBrain().eraseMemory(MemoryModuleType.POTENTIAL_JOB_SITE);
        villager.setVillagerXp(0);
        villager.setOffers(new net.minecraft.world.item.trading.MerchantOffers());
        villager.setVillagerData(villager.getVillagerData().setProfession(VillagerProfession.NONE).setLevel(1));
    }

    public static void hire(ServerLevel level, Villager villager, BlockPos workstation, Trade trade, GlobalPos desk) {
        level.getPoiManager().take(type -> true, (type, at) -> at.equals(workstation), workstation, 1);
        // A new hand has time to get going (its herd to breed, its first goods) before it's judged slack.
        MerchantBook.get(level.getServer()).startSlack(desk, workstation, 2 * dev.b28.enterprise.Dials.SLACK_DAYS.getInt());
        villager.getBrain().setMemory(MemoryModuleType.JOB_SITE, GlobalPos.of(level.dimension(), workstation));
        villager.setVillagerData(villager.getVillagerData().setProfession(trade.profession()).setLevel(1));
        MerchantBook book = MerchantBook.get(level.getServer());
        villager.getOffers().forEach(offer -> {
            String key = TradeKeys.of(offer);
            if (key.endsWith(">minecraft:emerald")) {
                book.setEnabled(desk, workstation, key, true);
            }
        });
    }

}
