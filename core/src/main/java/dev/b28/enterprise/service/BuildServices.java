package dev.b28.enterprise.service;

import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Treasury;
import dev.b28.enterprise.territory.Buildings;
import dev.b28.enterprise.territory.Works;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;

/**
 * A Mason's building: at home it builds the town what it lacks (see {@link Buildings}) when the town can pay for it;
 * abroad it lays a day's work on a town's works (a bridge, a wall, a building) for a Mason's day wage, where the town
 * has no Mason of its own.
 */
public final class BuildServices {
    private BuildServices() {}

    static void register() {
        Services.register(new Service("enterprise:building", TownServices.MASON, Service.Where.HOME,
                "builds its town a square, then halls for its trades (temple, library, smithy, watchtower, stone docks, "
                        + "caravanserai) as its own skill and theirs allow, for materials and wages",
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
        Works works = Works.get(visit.server());
        if (works.busy(bell.get())) {
            return Optional.empty();
        }
        Map<String, Integer> skills = Services.residents(visit.level(), bell.get());
        int merchant = Warehouse.get(visit.server()).level(visit.at());
        for (Buildings.Kind kind : Buildings.KINDS) {
            if (works.buildingsOf(bell.get()).containsKey(kind.id()) || visit.skill() < kind.masonLevel()) {
                continue;
            }
            boolean partnered = kind.partners().isEmpty() || kind.partners().stream().anyMatch(trade ->
                    trade.equals("enterprise:merchant") ? merchant >= kind.partnerLevel()
                            : skills.getOrDefault(trade, 0) >= kind.partnerLevel());
            if (!partnered || kind == Buildings.STONE_DOCKS && visit.territory().dock(bell.get()).isEmpty()) {
                continue;
            }
            double rate = visit.rate();
            double spare = Treasury.available(visit.level(), visit.at().pos(), visit.stock(), rate)
                    - visit.territory().reserve(visit.at());
            double wages = kind.blocks() / (Works.MASONS * Works.BLOCKS_PER_MASON_DAY) * Works.MASONS
                    * Works.MASON_PAY_PER_DAY;
            if (spare < materials(visit, kind) + wages / 2) {
                return Optional.empty(); // saving up for it: build in order
            }
            Optional<BlockPos> site = Buildings.site(visit.level(), bell.get().pos(), kind,
                    visit.territory().dock(bell.get()));
            if (site.isPresent()) {
                return Optional.of(Map.entry(kind, site.get()));
            }
        }
        return Optional.empty();
    }

    /** Logs a town might hold. */
    static final java.util.List<Item> LOGS = java.util.List.of(Items.OAK_LOG, Items.SPRUCE_LOG, Items.BIRCH_LOG,
            Items.JUNGLE_LOG, Items.ACACIA_LOG, Items.DARK_OAK_LOG, Items.MANGROVE_LOG, Items.CHERRY_LOG);

    static int logs(Visit visit) {
        return LOGS.stream().mapToInt(log -> visit.stock().count(new ItemCost(log, 1))).sum();
    }

    static void takeLogs(Visit visit, int count) {
        for (Item log : LOGS) {
            if (count <= 0) {
                return;
            }
            count -= visit.stock().take(new ItemCost(log, 1), count).stream()
                    .mapToInt(net.minecraft.world.item.ItemStack::getCount).sum();
        }
    }

    /** The materials still to buy: logs in the warehouse stand in for some of the timber. */
    static double materials(Visit visit, Buildings.Kind kind) {
        int logs = logs(visit);
        return Math.max(0, kind.emeralds() - logs * Buildings.EMERALDS_PER_LOG);
    }

    private static double build(Visit visit) {
        var next = next(visit).orElseThrow();
        Buildings.Kind kind = next.getKey();
        int logs = (int) Math.min(logs(visit), Math.ceil(kind.emeralds() / Buildings.EMERALDS_PER_LOG));
        double toBuy = materials(visit, kind);
        takeLogs(visit, logs);
        Treasury.withdraw(visit.level(), visit.at().pos(), visit.stock(), toBuy, Mint.get(visit.server()), visit.now());
        Works.get(visit.server()).startBuilding(visit.bell().orElseThrow(), kind, next.getValue());
        dev.b28.enterprise.Enterprise.LOGGER.debug("Settlement at {} starts a {} at {} ({} logs, {} emeralds of materials)",
                visit.bell().orElseThrow().pos().toShortString(), kind.id(), next.getValue().toShortString(), logs,
                String.format("%.1f", toBuy));
        return 0;
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
