package dev.b28.enterprise.service;

import dev.b28.enterprise.caravan.Caravan;
import dev.b28.enterprise.caravan.Route;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.territory.Territory;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;

/**
 * One villager doing a service for a town: who it comes from, the town served, how skilled it is, and (unless a trip
 * is only being planned) the town's warehouse and what it can spend. Abroad, the caravan and what it sold there.
 */
public final class Visit {
    private final ServerLevel level;
    private final Territory territory;
    private final GlobalPos from;
    private final GlobalPos at;
    private final int skill;
    private final long now;
    private final MerchantStock stock;
    private final double budget;
    private final Caravan caravan;
    private final Route route;
    private final Map<Item, double[]> sold;
    /** Markets the worker's town learns of through this: at once at home, once the worker is back from abroad. */
    final List<GlobalPos> heard = new ArrayList<>();

    private Visit(ServerLevel level, Territory territory, GlobalPos from, GlobalPos at, int skill, long now,
            MerchantStock stock, double budget, Caravan caravan, Route route, Map<Item, double[]> sold) {
        this.level = level;
        this.territory = territory;
        this.from = from;
        this.at = at;
        this.skill = Math.max(1, skill);
        this.now = now;
        this.stock = stock;
        this.budget = Math.max(0, budget);
        this.caravan = caravan;
        this.route = route;
        this.sold = sold;
    }

    /** For its own town, at the desk {@code desk}: no fee. */
    public static Visit home(ServerLevel level, Territory territory, GlobalPos desk, int skill, MerchantStock stock,
            long now) {
        return new Visit(level, territory, desk, desk, skill, now, stock, 0, null, null, Map.of());
    }

    /** Before a trip from {@code from} to {@code to}: would it be wanted there? No stock to see, only the ledgers. */
    public static Visit planning(ServerLevel level, Territory territory, Route route, int skill, long now) {
        return new Visit(level, territory, route.from(), route.to(), skill, now, null, 0, null, route, Map.of());
    }

    /** Arrived at {@code at} with {@code caravan}, the town able to spend {@code budget} emeralds on services. */
    public static Visit abroad(ServerLevel level, Territory territory, GlobalPos from, GlobalPos at, int skill,
            MerchantStock stock, double budget, Caravan caravan, Route route, Map<Item, double[]> sold, long now) {
        return new Visit(level, territory, from, at, skill, now, stock, budget, caravan, route, sold);
    }

    public ServerLevel level() {
        return level;
    }

    public MinecraftServer server() {
        return level.getServer();
    }

    /** The territory ledger the towns are on. */
    public Territory territory() {
        return territory;
    }

    /** The desk of the worker's town. */
    public GlobalPos from() {
        return from;
    }

    /** The desk of the town served. */
    public GlobalPos at() {
        return at;
    }

    /** The town served's bell, if it's a settlement. */
    public Optional<GlobalPos> bell() {
        return territory().bellOf(at);
    }

    /** The worker's level, 1 to 5. */
    public int skill() {
        return skill;
    }

    public long now() {
        return now;
    }

    public double rate() {
        return Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, now);
    }

    /** Only planning a trip: there's no stock to see, and nothing is done. */
    public boolean planning() {
        return stock == null;
    }

    public boolean abroad() {
        return !from.equals(at);
    }

    /** The served town's warehouse (null while planning). */
    public MerchantStock stock() {
        return stock;
    }

    /** Emeralds the served town can spend on services, beyond its reserve; nothing at home. */
    public double budget() {
        return budget;
    }

    /** The caravan the worker came with, abroad. */
    public Optional<Caravan> caravan() {
        return Optional.ofNullable(caravan);
    }

    /** The route travelled, abroad or planning a trip. */
    public Optional<Route> route() {
        return Optional.ofNullable(route);
    }

    /** What the caravan sold here, by item: {count, emeralds}. */
    public Map<Item, double[]> sold() {
        return sold;
    }

    public List<GlobalPos> heardForTest() {
        return List.copyOf(heard);
    }

    /** The worker's town learns of {@code market}. */
    public void hear(GlobalPos market) {
        if (!market.equals(from) && !heard.contains(market)) {
            heard.add(market);
        }
    }
}
