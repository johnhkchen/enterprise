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
 * with a chest against it (so it's a locker), for the trade that would buy most of what the town has to sell: its
 * farms' output and its warehouse's stock. The villager takes the job at once and its selling trades are switched on,
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

    /** The trade, not yet in the town, whose villagers would buy most of its supply; none if they'd buy nothing. */
    public static Optional<Trade> best(Map<String, Double> supply, Set<VillagerProfession> present) {
        return TRADES.stream()
                .filter(trade -> !present.contains(trade.profession()))
                .map(trade -> Map.entry(trade, trade.buys().stream()
                        .mapToDouble(item -> supply.getOrDefault(BuiltInRegistries.ITEM.getKey(item).toString(), 0.0))
                        .sum()))
                .filter(entry -> entry.getValue() > 0)
                .max(Comparator.comparingDouble((Map.Entry<Trade, Double> e) -> e.getValue())
                        .thenComparing(e -> -TRADES.indexOf(e.getKey())))
                .map(Map.Entry::getKey);
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
        if (idle.isEmpty()) {
            return Optional.empty();
        }
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        if (Treasury.available(level, desk.pos(), stock, rate) - territory.reserve(desk) < STALL_EMERALDS) {
            return Optional.empty();
        }
        // The town's trades from its roster: everyone, loaded or not.
        Set<VillagerProfession> present = new java.util.HashSet<>();
        dev.b28.enterprise.settlement.Settlements.get(level.getServer()).roster(GlobalPos.of(level.dimension(), bell))
                .values().forEach(r -> BuiltInRegistries.VILLAGER_PROFESSION.getOptional(
                        net.minecraft.resources.ResourceLocation.parse(r.profession())).ifPresent(present::add));
        GlobalPos bellPos = GlobalPos.of(level.dimension(), bell);
        Optional<Trade> trade = forContracts(level, desk, territory, bellPos, present)
                .or(() -> forSignals(level, desk, territory, bellPos, present))
                .or(() -> best(supply(territory.farmPlan(bellPos), stock), present));
        Optional<BlockPos> site = trade.isPresent() ? Growth.site(level, bell, STALL_SIZE, NEAREST) : Optional.empty();
        if (site.isEmpty()) {
            return Optional.empty();
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
        // A trade it has contracted to train someone in, and hasn't: an idle hand takes it up.
        for (var contract : book.trainingFor(desk)) {
            String profession = contract.training().get().profession();
            boolean has = present.stream().anyMatch(p -> BuiltInRegistries.VILLAGER_PROFESSION.getKey(p).toString()
                    .equals(profession));
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
     * A trade for goods a market this town knows has posted as wanted (no market it knows has them): an opportunity,
     * taken by its idle hands; the richest want first.
     */
    static Optional<Trade> forSignals(ServerLevel level, GlobalPos desk, Territory territory, GlobalPos bell,
            Set<VillagerProfession> present) {
        var server = level.getServer();
        java.util.List<dev.b28.enterprise.market.Exchange.Signal> signals = new java.util.ArrayList<>();
        for (GlobalPos market : dev.b28.enterprise.caravan.Atlas.get(server).known(server, desk)) {
            signals.addAll(dev.b28.enterprise.market.Exchange.get(server).signals(market));
        }
        signals.sort(Comparator.comparingDouble((dev.b28.enterprise.market.Exchange.Signal s) -> -(double) s.count() * s.spursEach()));
        return tradeFor(signals.stream().map(dev.b28.enterprise.market.Exchange.Signal::item).toList(), territory, bell,
                present);
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
    public static void hire(ServerLevel level, Villager villager, BlockPos workstation, Trade trade, GlobalPos desk) {
        level.getPoiManager().take(type -> true, (type, at) -> at.equals(workstation), workstation, 1);
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
