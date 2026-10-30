package dev.b28.enterprise.territory;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.packs.resources.ResourceManager;

/**
 * The land picks the people (the studio's `styles/settlement.json` and `settle.mjs`, thread 0015): the biome at a
 * town's centre, by its tags, chooses a list of candidates (a style, and for a race with clans its clan), and one is
 * drawn by weight from the town's seed. A settlers' town takes the land's people too (John, 2026-10-30: "Let's do land
 * it's on for now"). Only styles the mod ships (`town_rules/<style>.json`) found towns: the drafts are passed over, and
 * the draw falls to the rest of the list, or to the fallback's.
 */
public final class Settle {
    private Settle() {}

    /** Who founds a town: its style (the kingdom its buildings are chosen in) and its clan (empty for a style with none). */
    public record Who(String biome, String style, String clan) {}

    /** The town's people drawn for the land at {@code centre}, {@code seed} the world's mixed with the town's. */
    public static Who settle(ServerLevel level, BlockPos centre, long seed) {
        Set<String> tags = level.getBiome(centre).tags().map(t -> t.location().toString()).collect(Collectors.toSet());
        level.getBiome(centre).unwrapKey().ifPresent(k -> tags.add(k.location().getPath()));
        return settle(level.getServer().getResourceManager(), tags, (int) (seed ^ seed >>> 32));
    }

    /** As settle.mjs's `settle(biome, seed)`, with only the shipped styles. */
    public static Who settle(ResourceManager resources, Set<String> tags, int seed) {
        var data = WallKit.json(resources, "styles/settlement.json");
        if (data.isEmpty()) {
            return new Who("fallback", Kingdoms.DEFAULT, "");
        }
        JsonObject s = data.get();
        JsonObject entry = null;
        String biome = "fallback";
        for (JsonElement e : s.getAsJsonArray("biomes")) {
            JsonObject b = e.getAsJsonObject();
            boolean match = tags.contains(b.get("id").getAsString());
            for (JsonElement t : b.getAsJsonArray("tags")) {
                match |= tags.contains(t.getAsString());
            }
            if (match) {
                entry = b;
                biome = b.get("id").getAsString();
                break;
            }
        }
        List<JsonObject> list = shipped(resources, entry == null ? s.getAsJsonObject("fallback").getAsJsonArray("candidates")
                : entry.getAsJsonArray("candidates"));
        if (list.isEmpty()) {
            list = shipped(resources, s.getAsJsonObject("fallback").getAsJsonArray("candidates"));
        }
        if (list.isEmpty()) {
            return new Who(biome, Kingdoms.DEFAULT, "");
        }
        double total = list.stream().mapToDouble(c -> c.get("weight").getAsDouble()).sum();
        // settle.mjs's draw: ((Math.imul(seed ^ 0x9e3779b9, 0x85ebca6b) >>> 0) % 1000) / 1000 * total.
        double r = Integer.toUnsignedLong((seed ^ 0x9e3779b9) * 0x85ebca6b) % 1000 / 1000.0 * total;
        for (JsonObject c : list) {
            if ((r -= c.get("weight").getAsDouble()) < 0) {
                return who(biome, c);
            }
        }
        return who(biome, list.get(list.size() - 1));
    }

    private static Who who(String biome, JsonObject c) {
        return new Who(biome, c.get("style").getAsString(), c.has("clan") ? c.get("clan").getAsString() : "");
    }

    /** The candidates whose style the mod ships. */
    private static List<JsonObject> shipped(ResourceManager resources, JsonArray candidates) {
        List<JsonObject> out = new ArrayList<>();
        for (JsonElement e : candidates) {
            JsonObject c = e.getAsJsonObject();
            if (WallKit.json(resources, "town_rules/" + c.get("style").getAsString() + ".json").isPresent()) {
                out.add(c);
            }
        }
        return out;
    }
}
