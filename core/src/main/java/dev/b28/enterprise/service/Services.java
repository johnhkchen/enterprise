package dev.b28.enterprise.service;

import dev.b28.enterprise.caravan.Atlas;
import dev.b28.enterprise.caravan.Caravan;
import dev.b28.enterprise.caravan.CaravanServices;
import dev.b28.enterprise.caravan.Route;
import dev.b28.enterprise.caravan.Squad;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Treasury;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Territory;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.GlobalPos;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;

/**
 * Every service, by name, in the order they're done, and the runners: at home whenever a town is touched, abroad when
 * a caravan reaches a town, and before a trip, to choose who goes. Each is done by the most skilled of its trade who
 * is there (at home: on the roster and not away; abroad: in the squad). Abroad, the town served pays from its money
 * beyond its reserve, into the caravan's purse, and each paid service raises the two towns' trust a little.
 */
public final class Services {
    /** Each paid service abroad raises trust between the two towns by this much. */
    public static final double TRUST_PER_SERVICE = 0.02;

    private static final List<Service> ALL = new ArrayList<>();

    static {
        CaravanServices.register();
        TownServices.register();
    }

    private Services() {}

    public static void register(Service service) {
        if (ALL.stream().anyMatch(s -> s.id().equals(service.id()))) {
            throw new IllegalArgumentException("Service registered twice: " + service.id());
        }
        ALL.add(service);
    }

    public static List<Service> all() {
        return List.copyOf(ALL);
    }

    public static Optional<Service> get(String id) {
        return ALL.stream().filter(s -> s.id().equals(id)).findFirst();
    }

    /** What a trade does, everywhere. */
    public static List<Service> of(String trade) {
        return ALL.stream().filter(s -> s.trade().equals(trade)).toList();
    }

    /** The best level of each trade in a squad. */
    public static Map<String, Integer> skills(List<CompoundTag> crew) {
        Map<String, Integer> skills = new LinkedHashMap<>();
        crew.forEach(member -> skills.merge(Squad.trade(member), Squad.level(member), Math::max));
        return skills;
    }

    /** The best level of each trade among a town's grown residents at home. */
    public static Map<String, Integer> residents(ServerLevel level, GlobalPos bell) {
        Map<String, Integer> skills = new LinkedHashMap<>();
        Settlements.get(level.getServer()).roster(bell).values().stream()
                .filter(r -> !r.away() && !r.baby())
                .forEach(r -> skills.merge(r.profession(), Math.max(1, r.level()), Math::max));
        return skills;
    }

    /**
     * Before a trip on {@code route}: the trades, of those that could go ({@code skills}), whose
     * services abroad the town there would want, as far as the ledgers show.
     */
    public static Set<String> wantedAbroad(ServerLevel level, Territory territory, Route route,
            Map<String, Integer> skills, long now) {
        Set<String> trades = new LinkedHashSet<>();
        for (Service service : ALL) {
            Integer skill = skills.get(service.trade());
            if (service.where() == Service.Where.ABROAD && skill != null && !trades.contains(service.trade())
                    && service.wanted().test(Visit.planning(level, territory, route, skill, now))) {
                trades.add(service.trade());
            }
        }
        return trades;
    }

    /**
     * A caravan on {@code route} has reached its far end: its squad does each service abroad it can that the town
     * wants and can pay for. Markets its town learns of are added to {@code heard} (they ride home). Returns the
     * spurs paid into the caravan's purse.
     */
    public static int abroad(ServerLevel level, Territory territory, Route route, MerchantStock stock,
            Caravan caravan, Map<Item, double[]> sold, long now, List<GlobalPos> heard) {
        GlobalPos from = route.from();
        GlobalPos at = route.to();
        Map<String, Integer> skills = skills(caravan.crew());
        Mint mint = Mint.get(level.getServer());
        double rate = mint.rate(Mint.DEFAULT_FACTION, now);
        int spurs = 0;
        for (Service service : ALL) {
            Integer skill = skills.get(service.trade());
            if (service.where() != Service.Where.ABROAD || skill == null) {
                continue;
            }
            double budget = Treasury.available(level, at.pos(), stock, rate) - territory.reserve(at);
            Visit visit = Visit.abroad(level, territory, from, at, skill, stock, budget, caravan, route, sold, now);
            if (!service.wanted().test(visit)) {
                continue;
            }
            double fee = Math.min(visit.budget(), service.perform().applyAsDouble(visit));
            double paid = 0;
            if (fee > 0) {
                Treasury.Withdrawal taken = Treasury.withdraw(level, at.pos(), stock, fee, mint, now);
                paid = taken.value(rate);
                spurs += taken.spurs() + mint.sell(Mint.DEFAULT_FACTION, taken.emeralds(), now);
                ContractBook.get(level.getServer()).adjustTrust(from, at, TRUST_PER_SERVICE);
            }
            heard.addAll(visit.heard);
            ServiceLedger.get(level.getServer()).record(service.id(), from, at, paid, now);
        }
        return spurs;
    }

    /**
     * The town at {@code desk} was touched: its residents do each service at home they can that it wants. Markets
     * they learn of are known at once.
     */
    public static void home(ServerLevel level, Territory territory, GlobalPos desk, GlobalPos bell, MerchantStock stock,
            long now) {
        Map<String, Integer> skills = residents(level, bell);
        for (Service service : ALL) {
            Integer skill = skills.get(service.trade());
            if (service.where() != Service.Where.HOME || skill == null) {
                continue;
            }
            Visit visit = Visit.home(level, territory, desk, skill, stock, now);
            if (!service.wanted().test(visit)) {
                continue;
            }
            service.perform().applyAsDouble(visit);
            visit.heard.forEach(market -> Atlas.get(level.getServer()).learn(desk, market, now));
            ServiceLedger.get(level.getServer()).record(service.id(), desk, desk, 0, now);
        }
    }
}
