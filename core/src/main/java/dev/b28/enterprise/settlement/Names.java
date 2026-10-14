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
 * Names for markets, so people (and the records) can tell them apart: given once, from the lie of the land and a word
 * of its country, and kept. A town by a pass in the mountains ends in -gate, one in the hills in -fell, one by water in
 * -ford or -mere, one on the coast in -haven, and the rest in a farming word; its first part comes from what grows
 * there (Barley, Oak, Pine, Sand...). Markets off the map keep their own names (the front, the market across the sea).
 * A player may rename one.
 */
public final class Names extends SavedData {
    private static final String NAME = "enterprise_names";

    private static final List<String> FARMLAND = List.of("Barley", "Wheat", "Meadow", "Long", "Green", "Broad", "Ash",
            "Elm", "Thorn", "Mill", "Bramble", "Clover", "Hay", "Linden", "Rye", "Hollow");
    private static final List<String> WOODLAND = List.of("Oak", "Birch", "Holly", "Alder", "Hazel", "Rowan");
    private static final List<String> PINELAND = List.of("Pine", "Fir", "Spruce", "Larch");
    private static final List<String> DRYLAND = List.of("Sand", "Dust", "Amber", "Red");
    private static final List<String> SNOWLAND = List.of("Frost", "White", "Rime", "Winter");
    private static final List<String> FENLAND = List.of("Fen", "Reed", "Sedge", "Moss");
    private static final List<String> FARM_ENDS = List.of("field", "ton", "stead", "ley", "wick", "by", "worth", "ham");

    private final Map<GlobalPos, String> names = new HashMap<>();

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
        if (name == null) {
            // Given for good once the land round it has been seen (else a pass town might be named for farmland).
            ServerLevel level = server.getLevel(desk.dimension());
            boolean seen = level == null || dev.b28.enterprise.merchant.OffMap.get(server).is(desk)
                    || Hex.at(desk.pos()).neighbors().stream().allMatch(h -> Terrain.get(server).surveyed(level, h))
                            && Terrain.get(server).surveyed(level, Hex.at(desk.pos()));
            name = seen ? names.give(server, desk) : names.invent(server, desk);
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

    /** A name from the land: its country's word, and an ending for what lies about it. */
    String invent(MinecraftServer server, GlobalPos desk) {
        ServerLevel level = server.getLevel(desk.dimension());
        List<String> firsts = FARMLAND;
        String end = null;
        if (level != null) {
            var source = level.getChunkSource();
            var biome = source.getGenerator().getBiomeSource().getNoiseBiome(QuartPos.fromBlock(desk.pos().getX()),
                    QuartPos.fromBlock(level.getSeaLevel()), QuartPos.fromBlock(desk.pos().getZ()),
                    source.randomState().sampler());
            if (biome.is(BiomeTags.IS_TAIGA)) {
                firsts = PINELAND;
            } else if (biome.is(BiomeTags.IS_FOREST)) {
                firsts = WOODLAND;
            } else if (biome.is(Biomes.DESERT) || biome.is(BiomeTags.IS_BADLANDS) || biome.is(BiomeTags.IS_SAVANNA)) {
                firsts = DRYLAND;
            } else if (biome.is(Biomes.SNOWY_PLAINS) || biome.is(Biomes.ICE_SPIKES) || biome.is(Biomes.SNOWY_TAIGA)) {
                firsts = SNOWLAND;
            } else if (biome.is(Biomes.SWAMP) || biome.is(Biomes.MANGROVE_SWAMP)) {
                firsts = FENLAND;
            }
            // The lie of the land: the hexes round it.
            Terrain terrain = Terrain.get(server);
            Hex home = Hex.at(desk.pos());
            int steep = 0;
            int cliffs = 0;
            double wet = 0;
            List<Hex> around = new java.util.ArrayList<>();
            for (int dq = -2; dq <= 2; dq++) {
                for (int dr = -2; dr <= 2; dr++) {
                    Hex hex = new Hex(home.q() + dq, home.r() + dr);
                    if (hex.distance(home) <= 2) {
                        around.add(hex);
                    }
                }
            }
            for (Hex hex : around) {
                int relief = terrain.relief(level, hex);
                steep += relief >= Terrain.RELIEF_PER_FACTOR ? 1 : 0;
                cliffs += relief >= Terrain.CLIFF ? 1 : 0;
                wet = Math.max(wet, terrain.waterShare(level, hex));
            }
            long besideCliffs = home.neighbors().stream().filter(h -> terrain.relief(level, h) >= Terrain.CLIFF).count();
            if (besideCliffs >= 2 && terrain.relief(level, home) < Terrain.CLIFF) {
                end = "gate"; // a way through the mountains: cliffs on either hand
            } else if (steep >= 3 || cliffs >= 2) {
                end = "fell";
            } else if (biome.is(BiomeTags.IS_OCEAN) || biome.is(BiomeTags.IS_BEACH)) {
                end = "haven";
            } else if (biome.is(BiomeTags.IS_RIVER) || wet > 0.5) {
                end = "ford";
            } else if (wet > 0.2) {
                end = "mere";
            }
        }
        Set<String> taken = new HashSet<>(names.values());
        for (int i = 0; i < 64; i++) {
            long h = mix(desk.pos().getX() * 31L + desk.pos().getZ() * 1_000_003L + i);
            String first = firsts.get((int) Math.floorMod(h, (long) firsts.size()));
            String last = end != null ? end : FARM_ENDS.get((int) Math.floorMod(h >>> 17, (long) FARM_ENDS.size()));
            String name = first + last;
            if (!taken.contains(name)) {
                return name;
            }
        }
        return firsts.get(0) + (end != null ? end : "ton") + " " + (names.size() + 1);
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
                    .ifPresent(desk -> out.names.put(desk, e.getString("name")));
        }
        return out;
    }
}
