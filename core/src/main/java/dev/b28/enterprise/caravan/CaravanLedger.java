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

    public boolean removeRoute(int id) {
        caravans.remove(id);
        boolean removed = routes.remove(id) != null;
        setDirty();
        return removed;
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
        return ledger;
    }
}
