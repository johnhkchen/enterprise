package dev.b28.enterprise.caravan;

import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.saveddata.SavedData;

/** Every caravan route and the caravan on each, in world save data. One caravan per route at a time. */
public final class CaravanLedger extends SavedData {
    private static final String NAME = "enterprise_caravans";

    private final Map<Integer, Route> routes = new LinkedHashMap<>();
    private final Map<Integer, Caravan> caravans = new LinkedHashMap<>();
    // desk -> villagers who have gone out on its caravans: part of its community wherever they live
    private final Map<GlobalPos, java.util.Set<java.util.UUID>> served = new java.util.HashMap<>();

    /** Villagers who have gone out on caravans from this desk. */
    public java.util.Set<java.util.UUID> served(GlobalPos desk) {
        return java.util.Set.copyOf(served.getOrDefault(desk, java.util.Set.of()));
    }

    void recordService(GlobalPos desk, java.util.UUID villager) {
        if (served.computeIfAbsent(desk, d -> new java.util.LinkedHashSet<>()).add(villager)) {
            setDirty();
        }
    }
    private int nextId = 1;

    /** A squad member walking home on its own (a porter whose load is delivered), there at {@code arrives}. */
    public record Walker(GlobalPos home, CompoundTag villager, long arrives) {}

    private final List<Walker> walkers = new java.util.ArrayList<>();

    public void walkHome(GlobalPos home, CompoundTag villager, long arrives) {
        walkers.add(new Walker(home, villager, arrives));
        setDirty();
    }

    /** Walkers due home by {@code now}. */
    List<Walker> walkersHome(long now) {
        return walkers.stream().filter(w -> w.arrives() <= now).toList();
    }

    void arrived(Walker walker) {
        walkers.remove(walker);
        setDirty();
    }

    public List<Walker> walkers() {
        return List.copyOf(walkers);
    }

    public static CaravanLedger get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(CaravanLedger::new, CaravanLedger::load, null), NAME);
    }

    public Route addRoute(GlobalPos from, GlobalPos to) {
        Route route = new Route(nextId++, from, to);
        routes.put(route.id, route);
        setDirty();
        return route;
    }

    /**
     * Stops a route: at once if its caravan is home, else once it gets back (its squad and goods aren't stranded).
     * Returns false if there's no such route.
     */
    public boolean removeRoute(int id) {
        Route route = routes.get(id);
        if (route == null) {
            return false;
        }
        if (caravans.containsKey(id)) {
            route.stopping = true;
            route.status = "stopping when the caravan is home";
        } else {
            routes.remove(id);
        }
        setDirty();
        return true;
    }

    /** The route from one desk to another, if there is one. */
    public Optional<Route> route(GlobalPos from, GlobalPos to) {
        return routes.values().stream().filter(r -> r.from.equals(from) && r.to.equals(to) && !r.stopping).findFirst();
    }

    /** Takes a stopped route off the books once its caravan is home. */
    void retire(Route route) {
        routes.remove(route.id);
        setDirty();
    }

    public Collection<Route> routes() {
        return routes.values();
    }

    public Optional<Route> route(int id) {
        return Optional.ofNullable(routes.get(id));
    }

    public Optional<Caravan> caravan(int route) {
        return Optional.ofNullable(caravans.get(route));
    }

    /**
     * When this route's next caravan can set out: once the one on the road is home (outbound, it still has the way
     * there and back; waiting for food, the way back), and a departure interval after the last.
     */
    public long nextDeparture(Route route, long now) {
        long travel = Caravans.travelTicks(route.distance());
        long home = caravan(route.id).map(c -> switch (c.leg) {
            case OUTBOUND -> c.arrives + travel;
            case WAITING_FOR_FOOD -> Math.max(now, c.arrives) + travel;
            case RETURNING -> c.arrives;
        }).orElse(now);
        return Math.max(Math.max(now, home), route.lastDeparture + Caravans.DEPARTURE_INTERVAL);
    }

    /**
     * Food points caravans on their way to (or waiting at) this market will buy there for the road home: what its
     * stock must feed before another caravan can count on it.
     */
    public int foodPromisedAt(GlobalPos desk) {
        int promised = 0;
        for (Caravan c : caravans.values()) {
            Route route = routes.get(c.route);
            if (route != null && route.to.equals(desk) && c.leg != Caravan.Leg.RETURNING && c.provisions == 0) {
                promised += Caravans.foodNeeded(route.distance()) * Math.max(1, c.crew.size());
            }
        }
        return promised;
    }

    /** Recruits riding with caravans: to the capital from their towns, or on to the front. */
    public int recruitsOnTheRoad() {
        return caravans.values().stream().mapToInt(c -> c.recruits.size()).sum();
    }

    void put(Caravan caravan) {
        caravans.put(caravan.route, caravan);
        setDirty();
    }

    void remove(int route) {
        caravans.remove(route);
        setDirty();
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag routeList = new ListTag();
        routes.values().forEach(route -> routeList.add(route.save()));
        ListTag caravanList = new ListTag();
        caravans.values().forEach(caravan -> caravanList.add(caravan.save(registries)));
        tag.put("routes", routeList);
        tag.put("caravans", caravanList);
        tag.putInt("nextId", nextId);
        ListTag servedList = new ListTag();
        served.forEach((desk, ids) -> {
            CompoundTag entry = new CompoundTag();
            GlobalPos.CODEC.encodeStart(net.minecraft.nbt.NbtOps.INSTANCE, desk).ifSuccess(t -> entry.put("desk", t));
            ListTag members = new ListTag();
            ids.forEach(id -> members.add(net.minecraft.nbt.NbtUtils.createUUID(id)));
            entry.put("villagers", members);
            servedList.add(entry);
        });
        tag.put("served", servedList);
        ListTag walking = new ListTag();
        walkers.forEach(w -> {
            CompoundTag entry = new CompoundTag();
            GlobalPos.CODEC.encodeStart(net.minecraft.nbt.NbtOps.INSTANCE, w.home()).ifSuccess(t -> entry.put("home", t));
            entry.put("villager", w.villager());
            entry.putLong("arrives", w.arrives());
            walking.add(entry);
        });
        tag.put("walkers", walking);
        return tag;
    }

    public static CaravanLedger load(CompoundTag tag, HolderLookup.Provider registries) {
        CaravanLedger ledger = new CaravanLedger();
        for (Tag entry : tag.getList("routes", Tag.TAG_COMPOUND)) {
            Route route = Route.load((CompoundTag) entry);
            ledger.routes.put(route.id, route);
        }
        for (Tag entry : tag.getList("caravans", Tag.TAG_COMPOUND)) {
            Caravan caravan = Caravan.load((CompoundTag) entry, registries);
            ledger.caravans.put(caravan.route, caravan);
        }
        ledger.nextId = Math.max(1, tag.getInt("nextId"));
        for (Tag entry : tag.getList("served", Tag.TAG_COMPOUND)) {
            CompoundTag served = (CompoundTag) entry;
            GlobalPos.CODEC.parse(net.minecraft.nbt.NbtOps.INSTANCE, served.get("desk")).ifSuccess(desk -> {
                java.util.Set<java.util.UUID> ids = new java.util.LinkedHashSet<>();
                served.getList("villagers", Tag.TAG_INT_ARRAY).forEach(t -> ids.add(net.minecraft.nbt.NbtUtils.loadUUID(t)));
                ledger.served.put(desk, ids);
            });
        }
        for (Tag entry : tag.getList("walkers", Tag.TAG_COMPOUND)) {
            CompoundTag walker = (CompoundTag) entry;
            GlobalPos.CODEC.parse(net.minecraft.nbt.NbtOps.INSTANCE, walker.get("home")).ifSuccess(home ->
                    ledger.walkers.add(new Walker(home, walker.getCompound("villager"), walker.getLong("arrives"))));
        }
        return ledger;
    }
}
