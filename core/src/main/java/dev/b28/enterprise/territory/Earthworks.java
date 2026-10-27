package dev.b28.enterprise.territory;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.BlockTags;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Heightmap;

/**
 * Retaining walls where a plot's levelling cut a slope or built up a fill (the studio's earthworks set, batch 013; its
 * render of Hearthglen showed the bare cut faces): along each edge of the plot, the land's own ground just outside
 * against the plot's, and where they differ by 2 or more, a run of the kingdom's straight retaining tiles of that rise
 * (1 to 4), turned so their face looks at the low side, a lamp every 5th column, a buttress every 4th from rise 3, the
 * coursing varied by place. A cut stands outside the plot, its foot on the plot's edge; a fill stands outside too, its
 * top level with the plot.
 */
public final class Earthworks {
    private Earthworks() {}

    /** A retaining tile of the set. */
    record Tile(String part, int width, int depth, int rise, java.util.Set<String> tags) {}

    static List<Tile> tiles(ServerLevel level, String kingdom) {
        List<Tile> out = new ArrayList<>();
        WallKit.json(level.getServer().getResourceManager(), "town_kits/" + kingdom + "/earthworks.json").ifPresent(set -> {
            for (JsonElement e : set.getAsJsonArray("tiles")) {
                JsonObject t = e.getAsJsonObject();
                java.util.Set<String> tags = new java.util.TreeSet<>();
                if (t.has("tags")) {
                    t.getAsJsonArray("tags").forEach(x -> tags.add(x.getAsString()));
                }
                if (t.has("kind_of")) {
                    tags.add(t.get("kind_of").getAsString());
                }
                out.add(new Tile(t.get("part").getAsString(), t.get("width").getAsInt(), t.get("depth").getAsInt(),
                        t.get("rise").getAsInt(), tags));
            }
        });
        return out;
    }

    /** The least difference worth a wall: a 1-block step is a bank villagers and the eye take in their stride. */
    static final int LEAST = 2;

    /**
     * Retains a plot's edges: the plot {@code x0, z0} to {@code x1, z1} (inclusive) levelled at {@code ground}. Returns
     * the tiles laid.
     */
    public static int retain(ServerLevel level, String kingdom, int x0, int z0, int x1, int z1, int ground) {
        return retain(level, kingdom, x0, z0, x1, z1, ground, null);
    }

    /**
     * As {@link #retain(ServerLevel, String, int, int, int, int, int)}, the plot's street to {@code street}: a built wall
     * where a cut or fill faces the street, the land's own cut face (stone, moss, roots) on the plot's other sides (the
     * studio's rule, batch 014).
     */
    public static int retain(ServerLevel level, String kingdom, int x0, int z0, int x1, int z1, int ground,
            net.minecraft.core.Direction street) {
        List<Tile> tiles = tiles(level, kingdom);
        if (tiles.isEmpty()) {
            return 0;
        }
        int laid = 0;
        // Each edge: its columns along it, the way out (to look at the land beyond), and the turn for a wall whose high
        // ground lies that way (authored with the high ground to the north: q 0).
        int[][] edges = {
            {x0, z0 - 1, 1, 0, 0, -1}, // north edge: columns along x, outside to the north
            {x1 + 1, z0, 0, 1, 1, 0},  // east edge: along z, outside to the east
            {x0, z1 + 1, 1, 0, 0, 1},  // south edge
            {x0 - 1, z0, 0, 1, -1, 0}, // west edge
        };
        int[] lengths = {x1 - x0 + 1, z1 - z0 + 1, x1 - x0 + 1, z1 - z0 + 1};
        for (int k = 0; k < 4; k++) {
            int[] e = edges[k];
            for (int i = 0; i < lengths[k]; i++) {
                int ox = e[0] + e[2] * i, oz = e[1] + e[3] * i;
                int outside = height(level, ox, oz);
                if (outside == Integer.MIN_VALUE) {
                    continue;
                }
                int rise = Math.min(4, Math.abs(outside - ground));
                if (rise < LEAST) {
                    continue;
                }
                boolean cut = outside > ground;
                // Which way the high ground lies: outward for a cut, inward (the plot) for a fill.
                int hx = cut ? e[4] : -e[4], hz = cut ? e[5] : -e[5];
                int q = hz < 0 ? 0 : hx > 0 ? 1 : hz > 0 ? 2 : 3;
                boolean onStreet = street != null && e[4] == street.getStepX() && e[5] == street.getStepZ();
                String kind = !onStreet ? "cut_face" : i % 5 == 2 ? "lamp" : rise >= 3 && i % 4 == 0 ? "buttress" : "straight";
                Optional<Tile> tile = pick(tiles, rise, kind, ox * 31 + oz);
                if (tile.isEmpty()) {
                    continue;
                }
                laid += lay(level, tile.get(), ox, oz, cut ? ground : outside, q, e[4], e[5], cut);
            }
        }
        return laid;
    }

    private static Optional<Tile> pick(List<Tile> tiles, int rise, String kind, long seed) {
        List<Tile> fit = tiles.stream().filter(t -> t.rise() == rise && t.width() == 1 && t.tags().contains(kind)
                && (kind.equals("straight") ? !t.tags().contains("lamp") && !t.tags().contains("buttress") : true)).toList();
        if (fit.isEmpty() && !kind.equals("straight")) {
            return pick(tiles, rise, "straight", seed);
        }
        return fit.isEmpty() ? Optional.empty() : Optional.of(fit.get((int) Math.floorMod(seed, (long) fit.size())));
    }

    /**
     * One tile: its low edge on the column {@code (ox, oz)} just outside the plot for a cut (the bank behind it outward),
     * or reaching outward from it for a fill (its high side against the plot), its low surface at {@code low}.
     */
    private static int lay(ServerLevel level, Tile t, int ox, int oz, int low, int q, int outX, int outZ, boolean cut) {
        var resources = level.getServer().getResourceManager();
        Optional<Compose.Part> part = Compose.part(resources, Compose.blocks(level), t.part());
        if (part.isEmpty()) {
            return 0;
        }
        Compose.Grid g = new Compose.Grid(0, 0, 0);
        g.paste(part.get(), 0, 0, 0, q, false);
        // The tile's low edge row (authored z = depth - 1, the south) after the turn, and where it must stand.
        int d = t.depth();
        // Its footprint once turned: 1 wide across the edge, d deep along the way out.
        int fx = Math.abs(outX) == 1 ? d : 1, fz = Math.abs(outZ) == 1 ? d : 1;
        // A cut: the low edge is the column next to the plot, the rest outward. A fill: the high edge next to the plot,
        // the low edge d - 1 outward.
        int nearX = ox, nearZ = oz;
        int farX = ox + outX * (d - 1), farZ = oz + outZ * (d - 1);
        int cx = Math.min(nearX, farX), cz = Math.min(nearZ, farZ);
        if (fx == 1) {
            cx = ox;
        }
        if (fz == 1) {
            cz = oz;
        }
        int oy = low - 2;
        for (var e : g.cells.entrySet()) {
            BlockPos at = new BlockPos(cx + e.getKey().getX(), oy + e.getKey().getY(), cz + e.getKey().getZ());
            level.setBlock(at, e.getValue(), 2 | 16);
        }
        return 1;
    }

    /** The land's own ground at a column, under any tree; {@link Integer#MIN_VALUE} if it isn't loaded. */
    static int height(ServerLevel level, int x, int z) {
        if (!level.isLoaded(new BlockPos(x, 0, z))) {
            return Integer.MIN_VALUE;
        }
        int y = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z) - 1;
        BlockState top = level.getBlockState(new BlockPos(x, y, z));
        while ((top.is(BlockTags.LOGS) || top.is(BlockTags.LEAVES)) && y > level.getMinBuildHeight()) {
            top = level.getBlockState(new BlockPos(x, --y, z));
        }
        return y;
    }
}
