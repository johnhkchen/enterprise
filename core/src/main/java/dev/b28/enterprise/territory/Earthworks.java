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

    /**
     * The least difference worth dressing: a step of one too (the studio, thread 0001: "a 1-high dirt edge is the
     * commonest cut in a town, so it's the one a visitor sees most"), a kerb on the street and a cut face elsewhere.
     */
    static final int LEAST = 1;

    /** The most a plot's edge may stand above or below the land round it: two courses of the highest wall. */
    static final int MOST = 8;

    /**
     * A flight of steps (the kingdom's 3-wide steps tile of the rise, its `path` one) over {@code rect} ({x, z, sx, sz}
     * in the world), its high ground toward {@code high}, its low surface at {@code low}; the rect cleared over it first.
     */
    public static boolean steps(ServerLevel level, String kingdom, int[] rect, net.minecraft.core.Direction high, int low) {
        Optional<Tile> tile = tiles(level, kingdom).stream().filter(t -> t.tags().contains("steps") && t.tags().contains("path")
                && t.rise() == 1 && t.width() == 3).findFirst();
        if (tile.isEmpty()) {
            return false;
        }
        Optional<Compose.Part> part = Compose.part(level.getServer().getResourceManager(), Compose.blocks(level), tile.get().part());
        if (part.isEmpty()) {
            return false;
        }
        int q = switch (high) {
            case EAST -> 1;
            case SOUTH -> 2;
            case WEST -> 3;
            default -> 0;
        };
        Compose.Grid g = new Compose.Grid(0, 0, 0);
        g.paste(part.get(), 0, 0, 0, q, false);
        int oy = low - 2;
        for (int x = rect[0]; x < rect[0] + rect[2]; x++) {
            for (int z = rect[1]; z < rect[1] + rect[3]; z++) {
                for (int y = oy + 3; y <= oy + part.get().h() + 3; y++) {
                    level.setBlock(new BlockPos(x, y, z), net.minecraft.world.level.block.Blocks.AIR.defaultBlockState(), 2);
                }
            }
        }
        for (var e : g.cells.entrySet()) {
            level.setBlock(new BlockPos(rect[0] + e.getKey().getX(), oy + e.getKey().getY(), rect[1] + e.getKey().getZ()),
                    e.getValue(), 2 | 16);
        }
        return true;
    }

    /**
     * Retains a plot's edges: the plot {@code x0, z0} to {@code x1, z1} (inclusive) levelled at {@code ground}. Returns
     * the tiles laid.
     */
    public static int retain(ServerLevel level, String kingdom, int x0, int z0, int x1, int z1, int ground) {
        return retain(level, kingdom, x0, z0, x1, z1, ground, null, null).tiles();
    }

    /** What retaining a plot laid: its tiles, and the deepest drop at its edge (the courses it took). */
    public record Banked(int tiles, int deepest) {}

    /**
     * As {@link #retain(ServerLevel, String, int, int, int, int, int)}, the plot's street to {@code street}: a built wall
     * where a cut or fill faces the street, the land's own cut face (stone, moss, roots) on the plot's other sides (the
     * studio's rule, batch 014).
     */
    public static Banked retain(ServerLevel level, String kingdom, int x0, int z0, int x1, int z1, int ground,
            net.minecraft.core.Direction street, int[] open) {
        List<Tile> tiles = tiles(level, kingdom);
        if (tiles.isEmpty()) {
            return new Banked(0, 0);
        }
        int laid = 0, deepest = 0;
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
                // Left open where the door's steps meet the street ({@code open}: {x, z, sx, sz}).
                if (open != null && ox >= open[0] && ox < open[0] + open[2] && oz >= open[1] && oz < open[1] + open[3]) {
                    continue;
                }
                int diff = Math.abs(outside - ground);
                deepest = Math.max(deepest, diff);
                boolean cut = outside > ground;
                // Which way the high ground lies: outward for a cut, inward (the plot) for a fill.
                int hx = cut ? e[4] : -e[4], hz = cut ? e[5] : -e[5];
                int q = hz < 0 ? 0 : hx > 0 ? 1 : hz > 0 ? 2 : 3;
                boolean onStreet = street != null && e[4] == street.getStepX() && e[5] == street.getStepZ();
                // Courses outward from the plot, each up to 4 high, the next standing a tile's depth beyond the last
                // (a terrace between): a drop of 6 is a wall of 4 and one of 2, not a dirt cliff.
                for (int done = 0, offset = 0; diff - done >= LEAST; ) {
                    int rise = Math.min(4, diff - done);
                    String kind = !onStreet ? "cut_face" : i % 5 == 2 ? "lamp" : rise >= 3 && i % 4 == 0 ? "buttress" : "straight";
                    Optional<Tile> tile = pick(tiles, rise, kind, ox * 31 + oz + offset);
                    if (tile.isEmpty()) {
                        break;
                    }
                    laid += lay(level, tile.get(), ox + e[4] * offset, oz + e[5] * offset,
                            cut ? ground + done : ground - done - rise, q, e[4], e[5], cut);
                    done += rise;
                    offset += tile.get().depth();
                    // The terrace before the next course lit every 5th column (the studio, r35: walkable, and dark).
                    if (diff - done >= LEAST && i % 5 == 2) {
                        light(level, ox + e[4] * (offset - 1), oz + e[5] * (offset - 1));
                    }
                }
            }
        }
        return new Banked(laid, deepest);
    }

    /** A lantern on a post of mossy cobble on the ground at a column. */
    private static void light(ServerLevel level, int x, int z) {
        int y = height(level, x, z);
        if (y == Integer.MIN_VALUE) {
            return;
        }
        BlockPos post = new BlockPos(x, y + 1, z);
        if (level.getBlockState(post).isAir() && level.getBlockState(post.above()).isAir()) {
            level.setBlock(post, net.minecraft.world.level.block.Blocks.MOSSY_COBBLESTONE_WALL.defaultBlockState(), 3);
            level.setBlock(post.above(), net.minecraft.world.level.block.Blocks.LANTERN.defaultBlockState(), 3);
        }
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
