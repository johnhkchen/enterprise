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
     * The settlements' names by their people (John: "We'll want name lists to be expanded to handle the different
     * races' settlements"), each given once; a town of no people the land has told yet takes Lordaeron's. Theramore and
     * Orgrimmar are the factions' capitals, off the map: no town of the region is named for them. As the core's
     * {@code settlers.rs} {@code SETTLEMENTS}.
     */
    static final Map<String, List<String>> SETTLEMENTS = Map.of(
            "stormwind",
            List.of(
                    "Goldshire", "Lakeshire", "Darkshire", "Moonbrook", "Sentinel Hill", "Northshire", "Eastvale",
                    "Brightwood", "Westbrook", "Raven Hill", "Jerod's Landing", "Morgan's Vigil", "Nethergarde",
                    "Sorrow Hill", "Stonewatch", "Three Corners", "Alther's Mill", "Saldean's Farm", "Tower of Azora",
                    "Redridge", "Elwynn Falls", "Crystal Lake", "Westfall", "Gold Coast", "Jasperlode", "Fargodeep",
                    "Stonefield", "Brackwell", "Addle's Stead", "Lion's Pride"),
            "lordaeron",
            List.of(
                    "Brill", "Tarren Mill", "Southshore", "Hillsbrad", "Hearthglen", "Andorhal", "Darrowshire",
                    "Corin's Crossing", "Felstone", "Dalson's Tears", "Durnholde", "Stratholme", "Caer Darrow",
                    "Light's Hope", "Tyr's Hand", "Agamand Mills", "Solliden", "Brightwater", "Garren's Haunt",
                    "Chillwind", "Gahrron's Withering", "Ruins of Lordaeron", "Northdale", "Eastweald", "Eastwall",
                    "Crown Guard", "Blackwood", "Mardenholde", "Thondroril", "Terrordale"),
            "dalaran",
            List.of(
                    "Ambermill", "Pyrewood", "Strahnbrad", "Lordamere", "Violet Hold", "Krasus' Landing",
                    "Alterac Fields", "Sofera's Naze", "Gallows' Corner", "Fenris Isle", "Malden's Orchard",
                    "Olsen's Farthing", "Silverpine", "Chillwind Point", "Arcanum's Rest", "Kirin Var",
                    "Antonidas' Reach", "Azurecrest", "Spellweave", "Moonrise Hollow", "Starwatch", "Glimmerdeep",
                    "Runewood", "Violetmere", "Shimmerfield", "Tidewatch", "Lumen Hill", "Ashbury", "Silvermoon Ford",
                    "Aegwynn's Rest"),
            "dwarf",
            List.of(
                    "Kharanos", "Thelsamar", "Menethil Harbor", "Anvilmar", "Gol'Bolar", "Brewnall", "Dun Algaz",
                    "Dun Modr", "Dun Garok", "Dun Agrath", "Dun Mandarr", "Dun Baldar", "Aerie Peak", "Thandol Span",
                    "Steelgrill's Depot", "Misty Pine", "Ironband's Camp", "Amberstill", "Shimmer Ridge",
                    "Thorium Point", "Kelsey's Rest", "Bael'dun", "Stonewrought", "Mistyreed", "Grim Batol",
                    "Deepforge", "Anvilhold", "Coldridge", "Hammerstone", "Bronzebeard Hold"),
            "nightelf",
            List.of(
                    "Dolanaar", "Aldrassil", "Auberdine", "Astranaar", "Lor'danel", "Feathermoon", "Nijel's Point",
                    "Talonbranch Glade", "Starfall", "Ban'ethil", "Shadowglen", "Rut'theran", "Maestra's Post",
                    "Forest Song", "Silverwind Refuge", "Raynewood", "Thalanaar", "Nighthaven", "Emerald Sanctuary",
                    "Mystral Lake", "Ameth'Aran", "Bashal'Aran", "Mathystra", "Moonwell Glade", "Elune's Reach",
                    "Shan'do's Rest", "Whisperwind", "Starbreeze", "Oracle Glade", "Teldrassil Vale"),
            "orc",
            List.of(
                    "Razor Hill", "the Crossroads", "Splintertree Post", "Kargath", "Hammerfall", "Stonard",
                    "Grom'gol", "Zoram'gar", "Valormok", "Bloodvenom Post", "Thrallmar", "Garadar", "Mok'Nathal",
                    "Brackenwall", "Frostwolf", "Drygulch", "Bladespire", "Dragonmaw Gates", "Kargathia",
                    "Durotar's Watch", "Grommash's Rest", "Blackrock Spire", "Warsong Hold", "Shadowmoon",
                    "Bleeding Hollow", "Thunderlord Camp", "Burning Blade", "Skullsplitter", "Dranosh's Rest",
                    "Ironwall"),
            "tauren",
            List.of(
                    "Bloodhoof Village", "Camp Narache", "Camp Taurajo", "Camp Mojache", "Freewind Post",
                    "Sun Rock Retreat", "Spirit Rise", "Red Cloud Mesa", "Winterhoof Well", "Thunderhorn",
                    "Desolation Hold", "Westreach Summit", "Ragetotem", "Mesa of the Rising Sun", "Grimtotem Post",
                    "Stonetalon Peak", "Feralas Hold", "Mulgore Plains", "Red Rocks", "Brambleblade", "Bael Modan",
                    "Highmountain", "Skyhorn", "Earthmother's Rest", "Cairne's Watch", "Rivermane", "Dawnhoof",
                    "Windfury Ridge", "Longhoof", "Tallgrass"),
            "goblin",
            List.of(
                    "Ratchet", "Booty Bay", "Gadgetzan", "Everlook", "Steamwheedle Port", "Bilgewater Harbor",
                    "Mudsprocket", "Area 52", "Bootlegger Outpost", "Gallywix Landing", "Sparkwater Port",
                    "Mirage Raceway", "Lost Rigger Cove", "Rustbolt", "Fuselight", "Kajaro Field", "Bilgewater Port",
                    "Coinfall", "Profit Point", "Cogspin", "Greasewick", "Copperdrift", "Tidemoney", "Kaboom Bay",
                    "Shipwreck Shore", "Trade Prince's Rest", "Venture Bay", "Slagtown", "Gearwhistle", "Gilded Wharf"));

    /**
     * A name of its people's {@link #SETTLEMENTS}, by its place, one no market has; numbered once they're all taken.
     */
    String invent(MinecraftServer server, GlobalPos desk) {
        Set<String> taken = new HashSet<>(names.values());
        List<String> list = SETTLEMENTS.getOrDefault(peopleOf(server, desk), SETTLEMENTS.get(LORDAERON));
        long h = mix(desk.pos().getX() * 31L + desk.pos().getZ() * 1_000_003L);
        for (int i = 0; i < list.size(); i++) {
            String name = list.get((int) Math.floorMod(h + i, (long) list.size()));
            if (!taken.contains(name)) {
                return name;
            }
        }
        return list.get((int) Math.floorMod(h, (long) list.size())) + " " + (names.size() + 1);
    }

    private static final String LORDAERON = "lordaeron";

    /** The people of the desk's town, if the land has told them yet (its first survey: {@code Kingdoms}). */
    private static String peopleOf(MinecraftServer server, GlobalPos desk) {
        var host = dev.b28.enterprise.binding.CoreHost.of(server);
        Long town = host == null ? null : host.points().of(dev.b28.enterprise.binding.adapters.PointsAdapter.Kind.DESK).get(desk);
        var kingdoms = dev.b28.enterprise.territory.Kingdoms.get(server);
        return town != null && kingdoms.has(town) ? kingdoms.of(town) : LORDAERON;
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
