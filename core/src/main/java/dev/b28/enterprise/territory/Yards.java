package dev.b28.enterprise.territory;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.Random;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.packs.resources.ResourceManager;

/**
 * A plot's yards from its kingdom's yard tiles (the studio's batch 003, `town_kits/<kingdom>/yards.json`; John:
 * "front yard, back yard leeway ... interchangeable doodad regions"): a strip as wide as the plot filled left to right
 * (as seen from the street) with tiles 1 to 3 wide, neighbours' edges matching (hedge to hedge, picket to picket, `*`
 * any), the front's path tile at the door's column, a lamp at most every 5 columns (the house's door lantern counting
 * for the front), and the tiles chosen by their tags for the plot's tier and its occupant's trade, by the seed.
 */
public final class Yards {
    private Yards() {}

    /** A tile of the set. */
    public record Tile(String id, String part, String slot, int width, int depth, Set<String> tags, String left,
            String right, boolean lamp) {}

    /** A tile laid: which, and its first column. */
    public record Laid(Tile tile, int at) {}

    /** The kingdom's yard tiles, if it has a set. */
    public static List<Tile> tiles(ResourceManager resources, String kingdom) {
        List<Tile> out = new ArrayList<>();
        WallKit.json(resources, "town_kits/" + kingdom + "/yards.json").ifPresent(set -> {
            for (JsonElement e : set.getAsJsonArray("tiles")) {
                JsonObject t = e.getAsJsonObject();
                JsonObject sockets = t.getAsJsonObject("sockets");
                Set<String> tags = new java.util.TreeSet<>();
                t.getAsJsonArray("tags").forEach(x -> tags.add(x.getAsString()));
                out.add(new Tile(t.get("id").getAsString(), t.get("part").getAsString(), t.get("slot").getAsString(),
                        t.get("width").getAsInt(), t.get("depth").getAsInt(), tags, sockets.get("left").getAsString(),
                        sockets.get("right").getAsString(), t.has("lamp") && t.get("lamp").getAsBoolean()));
            }
        });
        return out;
    }

    static boolean meets(String right, String left) {
        return right.equals("*") || left.equals("*") || right.equals(left);
    }

    /**
     * A strip of {@code width} columns of the slot's tiles: the path tile at {@code door} (none if {@code door} is
     * negative), each lamp at most 5 columns after the last (the strip's start counting as a lamp at -3, the door as
     * one: the house's lantern), its end within 2 of a lamp, edges matching, the tiles {@code like} weighted up.
     * Empty if no arrangement keeps every rule.
     */
    public static Optional<List<Laid>> fill(List<Tile> all, String slot, int width, int door, Set<String> like, long seed) {
        List<Tile> tiles = all.stream().filter(t -> t.slot().equals(slot) && t.width() <= width).toList();
        if (tiles.isEmpty()) {
            return Optional.empty();
        }
        List<Laid> out = new ArrayList<>();
        return search(tiles, width, door, like, new Random(seed), 0, null, -3, out) ? Optional.of(out) : Optional.empty();
    }

    /** The lamp gap: a lamp at most so many columns after the last. */
    static final int LAMPS = 5;

    private static boolean search(List<Tile> tiles, int width, int door, Set<String> like, Random random, int x, Tile last,
            int lastLamp, List<Laid> out) {
        if (x == width) {
            return width - 1 - lastLamp <= 2;
        }
        List<Tile> order = new ArrayList<>();
        for (Tile t : tiles) {
            if (x + t.width() > width || last != null && !meets(last.right(), t.left())) {
                continue;
            }
            boolean path = t.tags().contains("path"), overDoor = door >= x && door < x + t.width();
            if (overDoor != path || path && door != x) {
                continue; // the path tile exactly at the door's column, and nowhere else
            }
            int lamp = lampOf(t, x, door, lastLamp);
            // A lamp no more than LAMPS after the last; a tile with none must leave room for the next in time.
            if (lamp != lastLamp ? lamp - lastLamp > LAMPS : x + t.width() - 1 - lastLamp >= LAMPS) {
                continue;
            }
            order.add(t);
        }
        // A tile beside itself is a repeat the eye catches (three wash lines in a row): a third of the weight.
        order.sort(java.util.Comparator.comparingDouble(t -> -score(t, like) * (t == last ? 0.33 : 1) * (0.5 + random.nextDouble())));
        for (Tile t : order) {
            out.add(new Laid(t, x));
            if (search(tiles, width, door, like, random, x + t.width(), t, lampOf(t, x, door, lastLamp), out)) {
                return true;
            }
            out.remove(out.size() - 1);
        }
        return false;
    }

    /** The last lamp once a tile is laid at {@code x}: its own, the door's lantern over it, or the one before. */
    private static int lampOf(Tile t, int x, int door, int lastLamp) {
        if (t.lamp()) {
            return x + t.width() / 2;
        }
        return door >= x && door < x + t.width() ? door : lastLamp;
    }

    private static double score(Tile t, Set<String> like) {
        double s = 1 + t.width() * 0.3;
        for (String tag : t.tags()) {
            if (like.contains(tag)) {
                s += tag.startsWith("trade:") ? 6 : 2;
            }
        }
        return s;
    }

    /** The turn that brings a strip authored with its street to the south to face {@code door}. */
    static int quarter(Direction door) {
        return switch (door) {
            case WEST -> 1;
            case NORTH -> 2;
            case EAST -> 3;
            default -> 0;
        };
    }

    /**
     * Lays a strip of tiles in a plot: the tiles at plot row {@code lz} (as authored), the plot's corner at the house's
     * ground (the tiles' surface row, layer 2, on it), turned to its street.
     */
    public static int lay(ServerLevel level, List<Laid> strip, BlockPos corner, int plotWidth, int plotDepth,
            Direction door, int lz) {
        var resources = level.getServer().getResourceManager();
        var blocks = Compose.blocks(level);
        int laid = 0;
        for (Laid l : strip) {
            Optional<Compose.Part> part = Compose.part(resources, blocks, l.tile().part());
            if (part.isEmpty()) {
                continue;
            }
            int[] at = Plots.world(corner.getX(), corner.getZ(), plotWidth, plotDepth, door, l.at(), lz, l.tile().width(),
                    l.tile().depth());
            Compose.Grid g = new Compose.Grid(0, 0, 0);
            g.paste(part.get(), 0, 0, 0, quarter(door), false);
            int oy = corner.getY() - 2;
            Kits.prepare(level, net.minecraft.world.level.levelgen.structure.BoundingBox.fromCorners(
                    new BlockPos(at[0], oy, at[1]), new BlockPos(at[0] + at[2] - 1, oy + part.get().h() - 1, at[1] + at[3] - 1)));
            g.cells.forEach((p, s) -> level.setBlock(new BlockPos(at[0] + p.getX(), oy + p.getY(), at[1] + p.getZ()), s, 2 | 16));
            laid++;
        }
        return laid;
    }

    /** What a plot likes in its yards: its tier, and its occupant's trade (`trade:<trade>`). */
    public static Set<String> likes(String tier, String trade) {
        Set<String> out = new java.util.TreeSet<>(Set.of("plain"));
        out.add(switch (tier) {
            case "hamlet", "village" -> "cottage";
            default -> "town";
        });
        out.addAll(Set.of("kitchen", "woodpile", "wash", "well", "hens"));
        if (trade != null && !trade.isEmpty()) {
            out.add("trade:" + trade);
        }
        return out;
    }

}
