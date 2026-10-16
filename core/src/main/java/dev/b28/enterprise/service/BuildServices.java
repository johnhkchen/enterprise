package dev.b28.enterprise.service;

import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Treasury;
import dev.b28.enterprise.territory.Buildings;
import dev.b28.enterprise.territory.Territory;
import dev.b28.enterprise.territory.Works;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;

/**
 * A Mason's building: at home it builds the town what it lacks (see {@link Buildings}) when the town can pay for it;
 * abroad it lays a day's work on a town's works (a bridge, a wall, a building) for a Mason's day wage, where the town
 * has no Mason of its own.
 */
public final class BuildServices {
    /**
     * A town builds only with this many times the building's whole cost (materials and wages) to spare beyond its
     * reserve: it shouldn't build itself out of its comforts.
     */
    public static final double SAVINGS = 2;

    private BuildServices() {}

    static void register() {
        Services.register(new Service("enterprise:building", TownServices.MASON, Service.Where.HOME,
                "builds its town a square, a granary or root cellar where its harvest piles up, then halls for its trades "
                        + "(temple, library, smithy, watchtower, stone docks, caravanserai) as its own skill and theirs "
                        + "allow, for materials and wages",
                visit -> next(visit).isPresent(), BuildServices::build));
        Services.register(new Service("enterprise:masonry_abroad", TownServices.MASON, Service.Where.ABROAD,
                "lays a day's work on the works of a town without a Mason, " + Works.MASON_PAY_PER_DAY
                        + " emerald a level",
                BuildServices::helpWanted, BuildServices::help));
    }

    /** What the town would build next, and where: the first kind it lacks whose builders it has, if it can pay. */
    static Optional<Map.Entry<Buildings.Kind, BlockPos>> next(Visit visit) {
        Optional<GlobalPos> bell = visit.bell();
        if (bell.isEmpty() || visit.planning()) {
            return Optional.empty();
        }
        return next(visit.level(), visit.at(), visit.stock(), bell.get(), visit.territory(), visit.skill(), visit.rate());
    }

    /**
     * Builders a town without a Mason hires: journeymen, who can put up a hall of this Mason's level at most. Stone
     * docks and a caravanserai still wait for a Mason of the town's own, or one visiting.
     */
    public static final int HIRED_SKILL = 3;

    /**
     * What the town would build next with builders of {@code skill}, and where: the first kind it lacks whose
     * partners it has, if it can pay (see {@link #SAVINGS}).
     */
    public static Optional<Map.Entry<Buildings.Kind, BlockPos>> next(ServerLevel level, GlobalPos desk, MerchantStock stock,
            GlobalPos bell, Territory territory, int skill, double rate) {
        var server = level.getServer();
        Works works = Works.get(server);
        if (works.busy(bell)) {
            return Optional.empty();
        }
        Map<String, Integer> skills = Services.residents(level, bell);
        int merchant = Warehouse.get(server).level(desk);
        for (Buildings.Kind kind : Buildings.KINDS) {
            if (works.buildingsOf(bell).containsKey(kind.id()) || skill < kind.masonLevel()) {
                continue;
            }
            boolean partnered = kind.partnered(skills, merchant);
            if (!partnered || kind == Buildings.STONE_DOCKS && territory.dock(bell).isEmpty() || !wanted(level, desk,
                    stock, territory, kind)) {
                continue;
            }
            double spare = Treasury.available(level, desk.pos(), stock, rate) - territory.reserve(desk);
            if (territory.mood(bell) < Works.WALL_MOOD_TO_START || spare < SAVINGS * cost(stock, kind)) {
                return Optional.empty(); // saving up for it, and not before the town's content: build in order
            }
            Optional<BlockPos> site = Buildings.site(level, bell.pos(), kind, territory.dock(bell));
            if (site.isPresent()) {
                return Optional.of(Map.entry(kind, site.get()));
            }
        }
        return Optional.empty();
    }

    /**
     * A granary or root cellar only where that crop piles up, a counting house only where the papers do; any other hall
     * whenever its partners are there.
     */
    static boolean wanted(ServerLevel level, GlobalPos desk, MerchantStock stock, Territory territory, Buildings.Kind kind) {
        if (kind == Buildings.COUNTING_HOUSE) {
            return Buildings.countingWanted(level.getServer(), desk, level.getGameTime());
        }
        return Buildings.store(kind).map(store -> territory.wantsStore(level, desk, stock, store, level.getGameTime()))
                .orElse(true);
    }

    /** A hall's materials still to buy and its builders' wages, in emeralds. */
    static double cost(MerchantStock stock, Buildings.Kind kind) {
        double wages = kind.blocks() / (Works.MASONS * Works.BLOCKS_PER_MASON_DAY) * Works.MASONS
                * Works.MASON_PAY_PER_DAY;
        return materials(stock, kind) + wages;
    }

    /**
     * The building work a town wants and could pay for now, in emeralds: each hall it lacks whose partners it has
     * and that it wants, in the order it builds them, as far as its money beyond the reserve goes. What a Mason of
     * its own would be paid to build; the Mason's marbles in the bag (see {@link dev.b28.enterprise.territory.Jobs#bag}).
     */
    public static double work(ServerLevel level, GlobalPos desk, MerchantStock stock, GlobalPos bell, Territory territory,
            double rate) {
        Works works = Works.get(level.getServer());
        Map<String, Integer> skills = Services.residents(level, bell);
        int merchant = Warehouse.get(level.getServer()).level(desk);
        double spare = Treasury.available(level, desk.pos(), stock, rate) - territory.reserve(desk);
        double work = 0;
        for (Buildings.Kind kind : Buildings.KINDS) {
            if (works.buildingsOf(bell).containsKey(kind.id())) {
                continue;
            }
            boolean partnered = kind.partnered(skills, merchant);
            if (!partnered || kind == Buildings.STONE_DOCKS && territory.dock(bell).isEmpty()
                    || !wanted(level, desk, stock, territory, kind)) {
                continue;
            }
            double cost = cost(stock, kind);
            if (work + cost > spare) {
                break;
            }
            work += cost;
        }
        return work;
    }

    /**
     * A town with no Mason of its own hires builders for its next hall, as it does masons for its walls: it buys the
     * materials now, and pays their wages as they build (see {@link Works#work}). Returns the hall started, if any.
     */
    public static Optional<Buildings.Kind> hire(ServerLevel level, GlobalPos desk, MerchantStock stock, GlobalPos bell,
            Territory territory, long now) {
        if (Services.residents(level, bell).containsKey(TownServices.MASON)) {
            return Optional.empty(); // its own Mason builds (the building service)
        }
        Mint mint = Mint.get(level.getServer());
        var next = next(level, desk, stock, bell, territory, HIRED_SKILL, mint.rate(Mint.DEFAULT_FACTION, now));
        next.ifPresent(n -> start(level, desk, stock, bell, n.getKey(), n.getValue(), now));
        return next.map(Map.Entry::getKey);
    }

    /** Logs a town might hold. */
    static final java.util.List<Item> LOGS = java.util.List.of(Items.OAK_LOG, Items.SPRUCE_LOG, Items.BIRCH_LOG,
            Items.JUNGLE_LOG, Items.ACACIA_LOG, Items.DARK_OAK_LOG, Items.MANGROVE_LOG, Items.CHERRY_LOG);

    static int logs(MerchantStock stock) {
        return LOGS.stream().mapToInt(log -> stock.count(new ItemCost(log, 1))).sum();
    }

    static void takeLogs(MerchantStock stock, int count) {
        for (Item log : LOGS) {
            if (count <= 0) {
                return;
            }
            count -= stock.take(new ItemCost(log, 1), count).stream()
                    .mapToInt(net.minecraft.world.item.ItemStack::getCount).sum();
        }
    }

    /** The materials still to buy: logs in the warehouse stand in for some of the timber. */
    static double materials(MerchantStock stock, Buildings.Kind kind) {
        int logs = logs(stock);
        return Math.max(0, kind.emeralds() - logs * Buildings.EMERALDS_PER_LOG);
    }

    private static double build(Visit visit) {
        var next = next(visit).orElseThrow();
        start(visit.level(), visit.at(), visit.stock(), visit.bell().orElseThrow(), next.getKey(), next.getValue(),
                visit.now());
        return 0;
    }

    /** Buys a hall's materials (logs in the warehouse first) and starts it. */
    private static void start(ServerLevel level, GlobalPos desk, MerchantStock stock, GlobalPos bell, Buildings.Kind kind,
            BlockPos corner, long now) {
        int logs = (int) Math.min(logs(stock), Math.ceil(kind.emeralds() / Buildings.EMERALDS_PER_LOG));
        double toBuy = materials(stock, kind);
        takeLogs(stock, logs);
        Treasury.withdraw(level, desk.pos(), stock, toBuy, Mint.get(level.getServer()), now);
        Works.get(level.getServer()).startBuilding(bell, kind, corner);
        dev.b28.enterprise.Enterprise.LOGGER.debug("Settlement at {} starts a {} at {} ({} logs, {} emeralds of materials)",
                bell.pos().toShortString(), kind.id(), corner.toShortString(), logs, String.format("%.1f", toBuy));
    }

    private static boolean helpWanted(Visit visit) {
        Optional<GlobalPos> bell = visit.bell();
        if (bell.isEmpty() || Crafts.hasOwn(visit, TownServices.MASON)) {
            return false;
        }
        return Works.get(visit.server()).underway(bell.get()).isPresent()
                && (visit.planning() || visit.budget() >= Works.MASON_PAY_PER_DAY * visit.skill());
    }

    private static double help(Visit visit) {
        Works works = Works.get(visit.server());
        works.underway(visit.bell().orElseThrow())
                .ifPresent(project -> works.help(project, Works.BLOCKS_PER_MASON_DAY * visit.skill()));
        return Works.MASON_PAY_PER_DAY * visit.skill();
    }
}
