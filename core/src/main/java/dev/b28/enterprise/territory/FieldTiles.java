package dev.b28.enterprise.territory;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;

/**
 * A kingdom's field tiles (the studio's fields set, `town_kits/<kingdom>/fields.json`, batch 023; John: "better looking
 * farmland that can cover more area"): 9 x 9 fields, each with a main crop the town plants it for and how many plots of
 * it, or none (fallow, meadow, orchard, a pumpkin garden: the land dressed, nothing counted), and 9 x 1 edges for the
 * balks between strips, one kind a holding.
 */
public final class FieldTiles {
    private FieldTiles() {}

    /** A tile of the set: its part, what it is (field, edge, corner), its main crop (null for none), its edge's socket. */
    public record Tile(String id, String part, String kind, String crop, int width, int depth, Set<String> tags,
            String socket) {}

    public static List<Tile> tiles(ServerLevel level, String kingdom) {
        List<Tile> out = new ArrayList<>();
        WallKit.json(level.getServer().getResourceManager(), "town_kits/" + kingdom + "/fields.json").ifPresent(set -> {
            for (JsonElement e : set.getAsJsonArray("tiles")) {
                JsonObject t = e.getAsJsonObject();
                Set<String> tags = new java.util.TreeSet<>();
                if (t.has("tags")) {
                    t.getAsJsonArray("tags").forEach(x -> tags.add(x.getAsString()));
                }
                out.add(new Tile(t.get("id").getAsString(), t.get("part").getAsString(), t.get("kind_of").getAsString(),
                        t.has("crop") && !t.get("crop").isJsonNull() ? t.get("crop").getAsString() : null,
                        t.get("width").getAsInt(), t.get("depth").getAsInt(), tags,
                        t.has("socket") ? t.get("socket").getAsString() : null));
            }
        });
        return out;
    }

    /** The set's name for a crop the town plants. */
    static String name(Item crop) {
        if (crop == Items.CARROT) {
            return "carrot";
        } else if (crop == Items.POTATO) {
            return "potato";
        } else if (crop == Items.BEETROOT) {
            return "beetroot";
        } else if (crop == Items.SUGAR_CANE) {
            return "sugar_cane";
        } else if (crop == Items.SWEET_BERRIES) {
            return "sweet_berries";
        }
        return "wheat";
    }

    /** A field of the crop, by the seed; empty if the set has none for it. */
    static Optional<Tile> field(List<Tile> tiles, Item crop, long seed) {
        String name = name(crop);
        return pick(tiles.stream().filter(t -> t.kind().equals("field") && name.equals(t.crop())).toList(), seed);
    }

    /**
     * Land dressed with no crop, by the seed, weighted as an open field's unsown land is: fallow mostly (hay, a
     * scarecrow), a meadow or an orchard now and then, a pumpkin garden rarely.
     */
    static Optional<Tile> dressing(List<Tile> tiles, long seed) {
        List<Tile> weighted = new ArrayList<>();
        for (Tile t : tiles) {
            if (!t.kind().equals("field") || t.crop() != null) {
                continue;
            }
            int w = t.tags().contains("fallow") ? 5 : t.tags().contains("meadow") || t.tags().contains("orchard") ? 2 : 1;
            for (int i = 0; i < w; i++) {
                weighted.add(t);
            }
        }
        return pick(weighted, seed);
    }

    /** A holding's edge (the long 9 x 1 kind, not the gate), the same for every seed of the holding. */
    static Optional<Tile> edge(List<Tile> tiles, long holding) {
        return pick(tiles.stream().filter(t -> t.kind().equals("edge") && t.width() == 9 && !"*".equals(t.socket()))
                .toList(), holding);
    }

    private static Optional<Tile> pick(List<Tile> fit, long seed) {
        if (fit.isEmpty()) {
            return Optional.empty();
        }
        long h = seed * 0x9E3779B97F4A7C15L;
        h ^= h >>> 29;
        return Optional.of(fit.get((int) Math.floorMod(h, (long) fit.size())));
    }

    /**
     * Lays a tile with its north-west corner at {@code (x, z)} after {@code q} quarter turns, its surface (layer 2) at
     * {@code y}. False if its part isn't there.
     */
    static boolean lay(ServerLevel level, Tile t, int x, int y, int z, int q) {
        Optional<Compose.Part> part = Compose.part(level.getServer().getResourceManager(), Compose.blocks(level), t.part());
        if (part.isEmpty()) {
            return false;
        }
        Compose.Grid g = new Compose.Grid(0, 0, 0);
        g.paste(part.get(), 0, 0, 0, q, false);
        int oy = y - 2;
        g.cells.forEach((p, s) -> level.setBlock(new BlockPos(x + p.getX(), oy + p.getY(), z + p.getZ()), s, 2 | 16));
        return true;
    }
}
