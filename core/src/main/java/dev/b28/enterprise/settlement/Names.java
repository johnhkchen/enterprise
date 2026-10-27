package dev.b28.enterprise.settlement;

import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.territory.Terrain;
import dev.b28.enterprise.territory.Territory;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.QuartPos;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.BiomeTags;
import net.minecraft.world.level.biome.Biomes;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Names for markets, so people (and the records) can tell them apart: given once, from a list of settlements'
 * names ({@link #SETTLEMENTS}), and kept. Markets off the map keep their own names (the front, the market across the
 * sea). A player may rename one.
 */
public final class Names extends SavedData {
    private static final String NAME = "enterprise_names";


    private final Map<GlobalPos, String> names = new HashMap<>();
    private final Set<GlobalPos> settled = new HashSet<>(); // names given once the land round them was known

    public static Names get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Names::new, Names::load, null), NAME);
    }

    /** The market's name, given now if it has none. For a settlement's bell, its market's. */
    public static String of(MinecraftServer server, GlobalPos at) {
        Names names = get(server);
        GlobalPos desk = deskFor(server, at);
        String name = names.names.get(desk);
        if (name == null || !names.settled.contains(desk)) {
            // Kept from the first asking; given again, for good, once the land round it has been seen (else a pass town
            // might keep a name for farmland).
            ServerLevel level = server.getLevel(desk.dimension());
            boolean seen = level == null || dev.b28.enterprise.merchant.OffMap.get(server).is(desk)
                    || Hex.at(desk.pos()).neighbors().stream().allMatch(h -> Terrain.get(server).surveyed(level, h))
                            && Terrain.get(server).surveyed(level, Hex.at(desk.pos()));
            if (name == null || seen) {
                names.names.remove(desk);
                name = names.give(server, desk);
            }
            if (seen) {
                names.settled.add(desk);
            }
        }
        return name;
    }

    /** The name, from anywhere a server is running (a report); the position where none is. */
    public static String label(GlobalPos at) {
        MinecraftServer server = net.neoforged.neoforge.server.ServerLifecycleHooks.getCurrentServer();
        return server == null ? at.pos().toShortString() : of(server, at);
    }

    /** Renames a market. */
    public void rename(GlobalPos desk, String name) {
        names.put(desk, name);
        settled.add(desk);
        setDirty();
    }

    private static GlobalPos deskFor(MinecraftServer server, GlobalPos at) {
        Territory territory = Territory.get(server);
        if (territory.bellOf(at).isPresent()) {
            return at; // a desk
        }
        for (GlobalPos desk : territory.desks()) {
            if (territory.bellOf(desk).filter(at::equals).isPresent()) {
                return desk;
            }
        }
        return at;
    }

    private String give(MinecraftServer server, GlobalPos desk) {
        Optional<String> offMap = dev.b28.enterprise.merchant.OffMap.get(server).market(desk)
                .map(dev.b28.enterprise.merchant.OffMap.Market::name);
        String name = offMap.orElseGet(() -> invent(server, desk));
        names.put(desk, name);
        setDirty();
        return name;
    }

    /**
     * The settlements' names, for now (John: "let's use WoW human settlement names"): the human towns and villages of
     * the Eastern Kingdoms, each given once.
     */
    static final List<String> SETTLEMENTS = List.of("Goldshire", "Lakeshire", "Darkshire", "Moonbrook", "Sentinel Hill",
            "Southshore", "Tarren Mill", "Ambermill", "Hillsbrad", "Stromgarde", "Refuge Pointe", "Thelsamar",
            "Menethil Harbor", "Northshire", "Raven Hill", "Andorhal", "Darrowshire", "Hearthglen", "Pyrewood",
            "Corin's Crossing", "Brill", "Durnholde", "Nethergarde", "Theramore", "Eastvale", "Brightwood",
            "Westbrook", "Jerod's Landing", "Morgan's Vigil", "Sorrow Hill", "Felstone", "Dalson's Tears");

    /** A name of {@link #SETTLEMENTS}, by its place, one no market has; numbered once they're all taken. */
    String invent(MinecraftServer server, GlobalPos desk) {
        Set<String> taken = new HashSet<>(names.values());
        long h = mix(desk.pos().getX() * 31L + desk.pos().getZ() * 1_000_003L);
        for (int i = 0; i < SETTLEMENTS.size(); i++) {
            String name = SETTLEMENTS.get((int) Math.floorMod(h + i, (long) SETTLEMENTS.size()));
            if (!taken.contains(name)) {
                return name;
            }
        }
        return SETTLEMENTS.get((int) Math.floorMod(h, (long) SETTLEMENTS.size())) + " " + (names.size() + 1);
    }

    /** SplitMix64: neighbouring positions get unrelated words. */
    private static long mix(long z) {
        z += 0x9E3779B97F4A7C15L;
        z = (z ^ (z >>> 30)) * 0xBF58476D1CE4E5B9L;
        z = (z ^ (z >>> 27)) * 0x94D049BB133111EBL;
        return z ^ (z >>> 31);
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        names.forEach((desk, name) -> {
            CompoundTag e = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> e.put("desk", t));
            e.putString("name", name);
            e.putBoolean("settled", settled.contains(desk));
            list.add(e);
        });
        tag.put("names", list);
        return tag;
    }

    public static Names load(CompoundTag tag, HolderLookup.Provider registries) {
        Names out = new Names();
        for (Tag t : tag.getList("names", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("desk")).result()
                    .ifPresent(desk -> {
                        out.names.put(desk, e.getString("name"));
                        if (e.getBoolean("settled")) {
                            out.settled.add(desk);
                        }
                    });
        }
        return out;
    }
}
