package dev.b28.enterprise.territory;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.Reader;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.tags.BlockTags;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.structure.BoundingBox;

/**
 * A town's inner wall from its kingdom's wall kit (`docs/assets/README.md` C5; the studio's stone kit, batch 006): each
 * side of the plan's rectangle tiled as the kit's `tileSide` does (scripts/assets/lordaeron-ref/wall-tiling.mjs: a
 * corner at each end, a gatehouse centred on each avenue, towers about every 24 in equal stretches, curtains of 8 and 4,
 * fillers to close exactly), each piece turned to face out and stood at its own ground on its foundation. The pieces
 * go up in order round the wall, north then east, south and west, as the town's wall work is done.
 */
public final class WallKit {
    private WallKit() {}

    /** A piece of a side: its kit name and where its west end lies along the side (corners at -3). */
    public record Piece(String piece, int at, int length) {}

    static final Map<String, Integer> LENGTHS = Map.of("corner_7x7", 7, "gatehouse_11x7", 11, "tower_7x7", 7,
            "curtain_8", 8, "curtain_4", 4, "filler_1", 1, "filler_2", 2, "filler_3", 3);

    /**
     * One side of length {@code L} (corner column to corner column) with gates centred at {@code gates}: its pieces in
     * order, the corner at its start only (each corner starts the next side round).
     */
    public static List<Piece> tileSide(int L, List<Integer> gates, int towerEvery) {
        List<Piece> out = new ArrayList<>();
        out.add(new Piece("corner_7x7", -3, 7));
        List<int[]> fixed = gates.stream().sorted().map(c -> new int[] {c - 5, 11}).toList();
        fixed.forEach(f -> out.add(new Piece("gatehouse_11x7", f[0], 11)));
        List<int[]> stops = new ArrayList<>();
        stops.add(new int[] {Integer.MIN_VALUE, 4});
        fixed.forEach(f -> stops.add(new int[] {f[0], f[0] + f[1]}));
        stops.add(new int[] {L - 4, Integer.MAX_VALUE});
        for (int i = 0; i + 1 < stops.size(); i++) {
            int x = stops.get(i)[1], gap = stops.get(i + 1)[0] - x;
            if (gap < 0) {
                throw new IllegalArgumentException("pieces overlap at " + x + ": a gate too near another or a corner");
            }
            int towers = Math.max(0, (gap + 7) / (towerEvery + 7));
            double stretch = (gap - towers * 7.0) / (towers + 1);
            for (int t = 0; t <= towers; t++) {
                int run = (int) Math.round(stretch * (t + 1)) - (int) Math.round(stretch * t);
                while (run >= 8) {
                    out.add(new Piece("curtain_8", x, 8));
                    x += 8;
                    run -= 8;
                }
                while (run >= 4) {
                    out.add(new Piece("curtain_4", x, 4));
                    x += 4;
                    run -= 4;
                }
                if (run > 0) {
                    out.add(new Piece("filler_" + run, x, run));
                    x += run;
                }
                if (t < towers) {
                    out.add(new Piece("tower_7x7", x, 7));
                    x += 7;
                }
            }
        }
        out.sort(java.util.Comparator.comparingInt(Piece::at));
        return out;
    }

    /** A piece placed: its kit part, its corner (x, z) in the world, and its quarter turns. */
    public record Placed(String part, int x, int z, int q, int length) {}

    /**
     * The whole wall of a plan's inner rectangle, in the order it goes up: north (west to east, as authored), east
     * (north to south, a quarter turn), south (east to west, a half), west (south to north, three quarters).
     */
    public static List<Placed> wall(TownPlans.Plan plan, String partPrefix) {
        TownPlans.Rect r = plan.inner();
        int bx = plan.bell().getX(), bz = plan.bell().getZ();
        int lx = r.x1() - r.x0(), lz = r.z1() - r.z0();
        List<Placed> out = new ArrayList<>();
        for (Piece p : tileSide(lx, List.of(bx - r.x0()), 24)) { // north
            out.add(new Placed(partPrefix + p.piece() + "_a", r.x0() + p.at(), r.z0() - 3, 0, p.length()));
        }
        for (Piece p : tileSide(lz, List.of(bz - r.z0()), 24)) { // east
            out.add(new Placed(partPrefix + p.piece() + "_a", r.x1() - 3, r.z0() + p.at(), 1, p.length()));
        }
        for (Piece p : tileSide(lx, List.of(r.x1() - bx), 24)) { // south
            out.add(new Placed(partPrefix + p.piece() + "_a", r.x1() - p.at() - (p.length() - 1), r.z1() - 3, 2, p.length()));
        }
        for (Piece p : tileSide(lz, List.of(r.z1() - bz), 24)) { // west
            out.add(new Placed(partPrefix + p.piece() + "_a", r.x0() - 3, r.z1() - p.at() - (p.length() - 1), 3, p.length()));
        }
        return out;
    }

    /** Does the kingdom have a wall kit (`town_kits/<kingdom>/wall_stone.json`)? */
    public static boolean has(ResourceManager resources, String kingdom) {
        return resources.getResource(ResourceLocation.fromNamespaceAndPath("enterprise", "town_kits/" + kingdom
                + "/wall_stone.json")).isPresent();
    }

    /** The kit's foundation: how many layers under a piece's ground row. */
    static final int GROUND = 4;

    /**
     * Lays the wall's pieces from {@code from} to {@code to} (indices into {@link #wall}), each at its own ground: the
     * natural ground's top at the piece's middle on the wall line. Returns how many were laid (a piece whose ground
     * isn't loaded stops it: laid again from there next time).
     */
    public static int lay(ServerLevel level, String kingdom, TownPlans.Plan plan, int from, int to) {
        ResourceManager resources = level.getServer().getResourceManager();
        var blocks = Compose.blocks(level);
        List<Placed> all = wall(plan, "enterprise:" + kingdom + "/wall/");
        int laid = 0;
        for (int k = Math.max(0, from); k < Math.min(to, all.size()); k++) {
            Placed p = all.get(k);
            Optional<Compose.Part> part = Compose.part(resources, blocks, p.part());
            if (part.isEmpty()) {
                return laid;
            }
            Compose.Grid g = new Compose.Grid(0, 0, 0);
            g.paste(part.get(), 0, 0, 0, p.q(), false);
            int sx = p.q() % 2 == 0 ? part.get().w() : part.get().d(), sz = p.q() % 2 == 0 ? part.get().d() : part.get().w();
            int mx = p.x() + sx / 2, mz = p.z() + sz / 2;
            if (!level.isLoaded(new BlockPos(p.x(), 0, p.z())) || !level.isLoaded(new BlockPos(p.x() + sx - 1, 0, p.z() + sz - 1))) {
                return laid;
            }
            int y = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, mx, mz) - 1;
            BlockState top = level.getBlockState(new BlockPos(mx, y, mz));
            while ((top.is(BlockTags.LOGS) || top.is(BlockTags.LEAVES)) && y > level.getMinBuildHeight()) {
                top = level.getBlockState(new BlockPos(mx, --y, mz));
            }
            int oy = y - GROUND;
            Kits.prepare(level, BoundingBox.fromCorners(new BlockPos(p.x(), oy, p.z()),
                    new BlockPos(p.x() + sx - 1, oy + part.get().h() - 1, p.z() + sz - 1)));
            g.cells.forEach((pos, state) -> level.setBlock(new BlockPos(p.x() + pos.getX(), oy + pos.getY(), p.z() + pos.getZ()),
                    state, 2 | 16));
            laid++;
        }
        return laid;
    }

    /** How many pieces the plan's whole wall takes. */
    public static int pieces(TownPlans.Plan plan, String kingdom) {
        return wall(plan, "enterprise:" + kingdom + "/wall/").size();
    }

    /** Reads a JSON resource (for the kit's rules), if there. */
    public static Optional<JsonObject> json(ResourceManager resources, String path) {
        return resources.getResource(ResourceLocation.fromNamespaceAndPath("enterprise", path)).flatMap(r -> {
            try (Reader reader = r.openAsReader()) {
                return Optional.of(JsonParser.parseReader(reader).getAsJsonObject());
            } catch (Exception e) {
                return Optional.empty();
            }
        });
    }
}
