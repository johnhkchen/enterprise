package dev.b28.enterprise.caravan;

import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.Map;
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
        return ledger;
    }
}
