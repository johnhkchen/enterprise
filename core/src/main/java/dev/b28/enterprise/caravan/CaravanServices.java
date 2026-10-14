package dev.b28.enterprise.caravan;

import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.money.Bargain;
import dev.b28.enterprise.service.Service;
import dev.b28.enterprise.service.Services;
import dev.b28.enterprise.service.Visit;
import java.util.Iterator;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.nbt.CompoundTag;

/**
 * The services a squad has always done at the far end, and those the caravan works itself on the road: a Merchant's
 * deals, a Shepherd's droving, a Cartographer's survey of the road, a Librarian's copy of the books; a
 * Leatherworker's packs, a Fisherman's boat and catch.
 */
public final class CaravanServices {
    private CaravanServices() {}

    public static void register() {
        Services.register(new Service("enterprise:droving", "minecraft:shepherd", Service.Where.ABROAD,
                "drives animals from the market pen to sell to a town short of them",
                CaravanServices::drovingWanted, CaravanServices::drove));
        Services.register(new Service("enterprise:deals", "enterprise:merchant", Service.Where.ABROAD,
                "strikes deals: haggles at its level, and drafts standing orders for goods that sold well",
                visit -> !visit.planning() && !visit.sold().isEmpty(), CaravanServices::deal));
        Services.register(new Service("enterprise:survey", "minecraft:cartographer", Service.Where.ABROAD,
                "maps the road: the towns along it are known at home, and a map comes back",
                CaravanServices::surveyWanted, CaravanServices::survey));
        Services.register(new Service("enterprise:books", "minecraft:librarian", Service.Where.ABROAD,
                "copies the books: what the town holds, so the next caravan knows what to bring",
                visit -> !visit.planning() || booksStale(visit), CaravanServices::copyBooks));
        Services.register(Service.road("enterprise:packs", Squad.LEATHERWORKER,
                "carries packs: " + Squad.PACK_STACKS + " more stacks of cargo"));
        Services.register(Service.road("enterprise:boat", Squad.FISHERMAN,
                "captains a boat across water, past bridges and their tolls"));
        Services.register(Service.road("enterprise:fishing", Squad.FISHERMAN,
                "fishes the water it crosses: " + Squad.FISH_PER_WATER_HEX + " cod a hex, sold where food is short"));
    }

    private static Optional<Route> route(Visit visit) {
        return visit.route();
    }

    // A Shepherd's droving.

    private static boolean drovingWanted(Visit visit) {
        if (visit.bell().isEmpty()) {
            return false;
        }
        if (visit.planning()) {
            return !Squad.herd(visit.level(), visit.from().pos()).isEmpty();
        }
        return visit.caravan().map(c -> !c.herd.isEmpty()).orElse(false) && visit.budget() > 0;
    }

    /**
     * The town buys as many animals as its pasture lacks (2 a villager, counted within 32 blocks of its bell), at a
     * bargained price, within what it can spend; they're turned out by its bell. The rest go home.
     */
    private static double drove(Visit visit) {
        Caravan caravan = visit.caravan().orElseThrow();
        GlobalPos bell = visit.bell().orElseThrow();
        var level = visit.level();
        int residentLevel = Warehouse.get(visit.server()).level(visit.at());
        int wanted = visit.territory().villagers(bell) * Caravans.ANIMALS_PER_VILLAGER - Squad.pasture(level, bell.pos());
        double owed = 0;
        for (Iterator<CompoundTag> it = caravan.herd.iterator(); it.hasNext() && wanted > 0; ) {
            CompoundTag animal = it.next();
            double each = Bargain.price(Squad.value(animal), Bargain.shortage(0, 1), caravan.merchantLevel, residentLevel);
            if (each <= 0 || owed + each > visit.budget() || Squad.release(level, bell.pos(), animal).isEmpty()) {
                continue;
            }
            it.remove();
            owed += each;
            wanted--;
            route(visit).ifPresent(r -> r.animalsSold++);
        }
        return owed;
    }

    // A Merchant's deals.

    private static double deal(Visit visit) {
        Caravan caravan = visit.caravan().orElseThrow();
        route(visit).ifPresent(route -> Caravans.draftContracts(visit.server(), route, caravan, visit.sold(), visit.now()));
        return 0;
    }

    // A Cartographer's survey.

    private static boolean surveyWanted(Visit visit) {
        return route(visit).map(r -> r.mappedAt < 0 || visit.now() - r.mappedAt > Caravans.STALE_MAP_TICKS).orElse(false);
    }

    /** The Cartographer has walked the road: the towns along it are known at home, and a map rides back. */
    private static double survey(Visit visit) {
        Route route = route(visit).orElseThrow();
        route.mappedAt = visit.now();
        List<GlobalPos> towns = Cartography.townsAlong(visit.level(), visit.territory(), route);
        towns.forEach(visit::hear);
        visit.caravan().orElseThrow().cargo.add(Cartography.mapOf(visit.level(), route, towns));
        route.townsMapped = Math.max(route.townsMapped, towns.size());
        return 0;
    }

    // A Librarian's copy of the books.

    private static boolean booksStale(Visit visit) {
        long taken = Warehouse.get(visit.server()).stocktakeTime(visit.at());
        return taken < 0 || visit.now() - taken > Caravans.STALE_BOOKS_TICKS;
    }

    private static double copyBooks(Visit visit) {
        Warehouse.get(visit.server()).recordStocktake(visit.at(), visit.stock().totals(), visit.now());
        return 0;
    }
}
