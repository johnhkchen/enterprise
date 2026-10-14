package dev.b28.enterprise.caravan;

import dev.b28.enterprise.merchant.MerchantBook;
import dev.b28.enterprise.territory.Territory;
import java.util.Comparator;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * What each market knows of others: the Merchant's Desks it could send caravans to. News travels with carriers: a
 * market knows its neighbours within {@link #WORD_OF_MOUTH} blocks, both ends of a route know each other once a
 * caravan has run it, a town whose land a caravan crosses learns both its ends, and a Cartographer maps the towns
 * along the way.
 */
public final class Atlas extends SavedData {
    private static final String NAME = "enterprise_atlas";

    /** Markets this close know of each other without anyone travelling. */
    public static final int WORD_OF_MOUTH = 400;

    private final Map<GlobalPos, Map<GlobalPos, Long>> known = new HashMap<>(); // desk -> other desk -> learned when

    public static Atlas get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Atlas::new, Atlas::load, null), NAME);
    }

    /** The market at {@code desk} hears of the one at {@code other}. */
    public void learn(GlobalPos desk, GlobalPos other, long now) {
        if (desk.equals(other) || !desk.dimension().equals(other.dimension())) {
            return;
        }
        if (known.computeIfAbsent(desk, d -> new LinkedHashMap<>()).putIfAbsent(other, now) == null) {
            setDirty();
        }
    }

    public void learnBoth(GlobalPos a, GlobalPos b, long now) {
        learn(a, b, now);
        learn(b, a, now);
    }

    /** Every market the one at {@code desk} knows of, nearest first: those it has heard of and its neighbours. */
    public List<GlobalPos> known(MinecraftServer server, GlobalPos desk) {
        Set<GlobalPos> towns = new LinkedHashSet<>(known.getOrDefault(desk, Map.of()).keySet());
        for (GlobalPos other : markets(server)) {
            if (!other.equals(desk) && other.dimension().equals(desk.dimension())
                    && other.pos().distSqr(desk.pos()) <= (double) WORD_OF_MOUTH * WORD_OF_MOUTH) {
                towns.add(other);
            }
        }
        return towns.stream().sorted(Comparator.comparingDouble(other -> other.pos().distSqr(desk.pos()))).toList();
    }

    /** Every Merchant's Desk the ledgers know: markets with lockers, settlements' desks, and caravans' ends. */
    public static Set<GlobalPos> markets(MinecraftServer server) {
        Set<GlobalPos> desks = new LinkedHashSet<>(MerchantBook.get(server).desks());
        desks.addAll(Territory.get(server).desks());
        CaravanLedger.get(server).routes().forEach(route -> {
            desks.add(route.from());
            desks.add(route.to());
        });
        return desks;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        known.forEach((desk, others) -> others.forEach((other, when) -> {
            CompoundTag entry = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> entry.put("desk", t));
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, other).ifSuccess(t -> entry.put("other", t));
            entry.putLong("when", when);
            list.add(entry);
        }));
        tag.put("known", list);
        return tag;
    }

    public static Atlas load(CompoundTag tag, HolderLookup.Provider registries) {
        Atlas atlas = new Atlas();
        for (Tag t : tag.getList("known", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            var desk = GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("desk")).result();
            var other = GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("other")).result();
            if (desk.isPresent() && other.isPresent()) {
                atlas.known.computeIfAbsent(desk.get(), d -> new LinkedHashMap<>()).put(other.get(), entry.getLong("when"));
            }
        }
        return atlas;
    }
}
