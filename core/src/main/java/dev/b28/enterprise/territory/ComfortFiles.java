package dev.b28.enterprise.territory;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.Reader;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;

/**
 * What a comfort good serves, one file each: {@code data/enterprise/comforts/<id>.json} (docs/markets-plan.md 6; John:
 * "properly assign happiness values to different items that serve it at different degrees"). A file: {@code item},
 * {@code days} (the villager-days of its kind one serves: a book, passed round and read again, ten) and {@code
 * quality} (1 plain, up to 2: a day met with it lifts the mood so many plain days'). The item's kind is its tag's
 * ({@code enterprise:comforts/<kind>}); a comfort no file names covers its worth in rations, as before. Read from the
 * server's resources the first time they're asked for, again after a reload; the core takes them with the content pack
 * ({@code binding/ModPack}).
 */
public final class ComfortFiles {
    /** A comfort file, read. */
    public record Comfort(ResourceLocation id, Item item, double days, double quality) {}

    private static volatile List<Comfort> loaded;

    private ComfortFiles() {}

    /** Forgets the files read, for the server's resources reloading. */
    public static void forget() {
        loaded = null;
    }

    /** Every comfort file, by its name: the running server's. */
    public static List<Comfort> all() {
        List<Comfort> comforts = loaded;
        if (comforts == null) {
            MinecraftServer server = net.neoforged.neoforge.server.ServerLifecycleHooks.getCurrentServer();
            if (server == null) {
                throw new IllegalStateException("no server: the comfort files are its resources");
            }
            comforts = of(server.getResourceManager());
            loaded = comforts;
        }
        return comforts;
    }

    /** The comfort files in {@code resources}, by name. A file the mod can't read is left out and logged, with why. */
    public static List<Comfort> of(ResourceManager resources) {
        List<Comfort> comforts = new ArrayList<>();
        resources.listResources("comforts", p -> p.getNamespace().equals("enterprise") && p.getPath().endsWith(".json"))
                .forEach((path, resource) -> {
                    try (Reader reader = resource.openAsReader()) {
                        comforts.add(read(path, JsonParser.parseReader(reader).getAsJsonObject()));
                    } catch (Exception e) {
                        dev.b28.enterprise.Enterprise.LOGGER.warn("Comfort file {} left out: {}", path, e.getMessage());
                    }
                });
        comforts.sort(Comparator.comparing(c -> c.id().toString()));
        return List.copyOf(comforts);
    }

    /** A comfort file, or what's wrong with it: an item the game hasn't, days not above nothing, a quality beyond 1 to 2. */
    public static Comfort read(ResourceLocation id, JsonObject o) {
        List<String> wrong = new ArrayList<>();
        String name = o.has("item") ? o.get("item").getAsString() : null;
        Item item = name == null ? Items.AIR : BuiltInRegistries.ITEM.get(ResourceLocation.parse(name));
        if (item == Items.AIR) {
            wrong.add(name == null ? "no item" : "no item " + name);
        }
        double days = o.has("days") ? o.get("days").getAsDouble() : 0;
        if (!(days > 0 && Double.isFinite(days))) {
            wrong.add("serves " + days + " days, not a number above nothing");
        }
        double quality = o.has("quality") ? o.get("quality").getAsDouble() : 1;
        if (!(quality >= 1 && quality <= 2)) {
            wrong.add("quality " + quality + ", not a number from 1 to 2");
        }
        if (!wrong.isEmpty()) {
            throw new IllegalArgumentException(String.join("; ", wrong));
        }
        return new Comfort(id, item, days, quality);
    }
}
