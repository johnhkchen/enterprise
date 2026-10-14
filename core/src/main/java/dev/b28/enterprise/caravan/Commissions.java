package dev.b28.enterprise.caravan;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.money.Treasury;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Comforts;
import dev.b28.enterprise.territory.Territory;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.stream.Collectors;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.phys.AABB;

/**
 * Any town can send caravans, not just a hub: a player commissions one at the Merchant's Desk, or a town commissions
 * its own when it lacks comforts and has money to spare, to the nearest market it knows that has them. A town with
 * routes trains its squad: an unemployed adult of its community learns the trade the squad lacks.
 */
public final class Commissions {
    /** What training a villager for a squad costs, in emeralds. */
    public static final double TRAINING_EMERALDS = 4;

    /** A town sends a caravan of its own only with this much money to spare beyond its reserve, in emeralds. */
    public static final double COMMISSION_SPARE = 8;

    private Commissions() {}

    /**
     * At a toll review: a town with no route of its own, short of some kind of comfort, with money to spare, commissions
     * a route to the nearest market it knows (within a leg's food) whose last known stock has that kind.
     */
    public static Optional<Route> consider(ServerLevel level, GlobalPos desk, MerchantStock stock, GlobalPos bell,
            Territory territory, long now) {
        var server = level.getServer();
        CaravanLedger ledger = CaravanLedger.get(server);
        if (ledger.routes().stream().anyMatch(r -> r.from.equals(desk))) {
            return Optional.empty();
        }
        Set<String> short_ = territory.comfortsMet(bell).entrySet().stream()
                .filter(e -> e.getValue() < 1).map(Map.Entry::getKey).collect(Collectors.toSet());
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        if (short_.isEmpty() || Treasury.available(level, desk.pos(), stock, rate) - territory.reserve(desk) < COMMISSION_SPARE) {
            return Optional.empty();
        }
        Warehouse warehouse = Warehouse.get(server);
        for (GlobalPos other : Atlas.get(server).known(server, desk)) {
            int distance = (int) Math.ceil(Math.sqrt(other.pos().distSqr(desk.pos())));
            if (!Caravans.inRange(distance)) {
                continue;
            }
            boolean hasThem = short_.stream().anyMatch(kind -> Comforts.kind(kind)
                    .map(tag -> BuiltInRegistries.ITEM.getTagOrEmpty(tag))
                    .map(items -> java.util.stream.StreamSupport.stream(items.spliterator(), false)
                            .anyMatch(holder -> warehouse.onHand(other, holder.value()) > 0))
                    .orElse(false));
            if (hasThem) {
                Route route = ledger.addRoute(desk, other);
                Enterprise.LOGGER.debug("Settlement at {} commissions a caravan to {} for {}", bell.pos().toShortString(),
                        other.pos().toShortString(), short_);
                return Optional.of(route);
            }
        }
        return Optional.empty();
    }

    /** Does the town's squad still lack members (it has routes of its own, and fewer than it wants)? */
    public static boolean squadShort(ServerLevel level, GlobalPos desk, BlockPos bell) {
        var server = level.getServer();
        long routes = CaravanLedger.get(server).routes().stream().filter(r -> r.from.equals(desk) && !r.stopping).count();
        if (routes == 0) {
            return false;
        }
        long members = Squad.available(level, desk, Squad.reach()).size()
                + Settlements.get(server).roster(GlobalPos.of(level.dimension(), bell)).values().stream()
                        .filter(Settlements.Resident::away).count();
        return members < Math.min(Squad.MAX, routes + 1);
    }

    /**
     * When counted while loaded: a town with routes of its own trains its squad up to a lead for each route and one
     * spare, at most {@link Squad#MAX}: an unemployed adult near the bell learns the trade the squad lacks (in
     * {@link Squad#TRADES} order), for {@link #TRAINING_EMERALDS} from money beyond the town's reserve. Trained, it
     * keeps its trade without a workstation.
     */
    public static Optional<Villager> train(ServerLevel level, GlobalPos desk, BlockPos bell, MerchantStock stock,
            Territory territory) {
        var server = level.getServer();
        long routes = CaravanLedger.get(server).routes().stream().filter(r -> r.from.equals(desk) && !r.stopping).count();
        if (routes == 0) {
            return Optional.empty();
        }
        List<String> trades = new java.util.ArrayList<>(Squad.available(level, desk, Squad.reach()).stream()
                .map(Squad::trade).toList());
        Settlements.get(server).roster(GlobalPos.of(level.dimension(), bell)).values().stream()
                .filter(Settlements.Resident::away).forEach(r -> trades.add(r.profession()));
        if (trades.size() >= Math.min(Squad.MAX, routes + 1)) {
            return Optional.empty();
        }
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        if (Treasury.available(level, desk.pos(), stock, rate) - territory.reserve(desk) < TRAINING_EMERALDS) {
            return Optional.empty();
        }
        var community = Squad.community(level, desk);
        var lockerSites = dev.b28.enterprise.merchant.MerchantBook.get(server).workstations(desk);
        Optional<Villager> pupil = level.getEntitiesOfClass(Villager.class, new AABB(bell).inflate(Squad.reach()),
                        v -> !v.isBaby() && v.getVillagerData().getProfession() == VillagerProfession.NONE
                                && Squad.connected(v, desk, community, lockerSites))
                .stream().findFirst();
        if (pupil.isEmpty()) {
            return Optional.empty();
        }
        String trade = Squad.TRADES.stream().filter(t -> !trades.contains(t)).findFirst()
                .orElse(Squad.TRADES.get(trades.size() % Squad.TRADES.size()));
        Villager villager = pupil.get();
        villager.setVillagerData(villager.getVillagerData()
                .setProfession(BuiltInRegistries.VILLAGER_PROFESSION.get(ResourceLocation.parse(trade))).setLevel(1));
        villager.setVillagerXp(Math.max(1, villager.getVillagerXp()));
        Treasury.withdraw(level, desk.pos(), stock, TRAINING_EMERALDS, Mint.get(server), level.getGameTime());
        Enterprise.LOGGER.debug("Settlement at {} trains a {} for its caravans", bell.toShortString(), trade);
        return pupil;
    }

    /**
     * What a town sending its own caravan wants it to bring home: each kind of comfort it holds less of than its par,
     * in emeralds' worth. A hub (no settlement) shops for nobody.
     */
    public static Map<String, Double> shoppingList(Territory territory, GlobalPos desk, MerchantStock stock) {
        Map<String, Double> list = new LinkedHashMap<>();
        if (territory.bellOf(desk).isEmpty()) {
            return list;
        }
        double par = territory.comfortPar(desk);
        Comforts.onHand(stock).forEach((kind, held) -> {
            if (par - held > 0) {
                list.put(kind, par - held);
            }
        });
        return list;
    }

    /** Spurs a caravan has to set aside to buy its shopping list at full local value. */
    static double shoppingMoney(Map<String, Double> list, double rate) {
        return list.values().stream().mapToDouble(Double::doubleValue).sum() * rate;
    }

    static double value(String itemId) {
        return 1 / Prices.itemsPerEmerald(itemId);
    }
}
