package dev.b28.enterprise.territory;

import java.util.List;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Heightmap;

/**
 * A house's plot on a ward with room for yards (John: "give it some front yard, back yard leeway. These act as
 * interchangeable doodad regions"): from the street inward its front yard ({@link TownPlans#FRONT_YARD} deep), the house
 * (a block of side yard either side), and its back yard (what's left of the plot's depth). The house is the town's
 * kingdom's home kit; each yard is a kit of its own ({@code yard:front}, {@code yard:back}, the studio's), chosen per
 * plot; and a path is cut from the street to the house's door wherever its door fell.
 *
 * <p>Positions in a plot are as authored: x along the street, z from the back (0) to the street ({@code depth - 1}),
 * the street to the south; {@link #world} turns them to the plot's street.
 */
public final class Plots {
    private Plots() {}

    /** A plot's width for a house {@code width} wide: a block of side yard either side. */
    public static int width(int house) {
        return house + 2;
    }

    /** Where the house stands in its plot: its corner (x, z) as authored, its width and depth. */
    static int[] house(int plotDepth, int width, int depth) {
        return new int[] {1, plotDepth - TownPlans.FRONT_YARD - depth, width, depth};
    }

    /**
     * A rectangle of a plot (as authored: {@code lx}, {@code lz}, {@code w} x {@code d}) in the world: its north-west
     * corner and its size there ({x, z, sx, sz}), the plot's corner {@code (x0, z0)}, {@code width} x {@code depth} as
     * authored, its street to {@code door} (as {@link Growth#buildHome} and {@link Kits#place} turn things).
     */
    public static int[] world(int x0, int z0, int width, int depth, Direction door, int lx, int lz, int w, int d) {
        return switch (door) {
            case NORTH -> new int[] {x0 + width - lx - w, z0 + depth - lz - d, w, d};
            case WEST -> new int[] {x0 + depth - lz - d, z0 + lx, d, w};
            case EAST -> new int[] {x0 + lz, z0 + width - lx - w, d, w};
            default -> new int[] {x0 + lx, z0 + lz, w, d};
        };
    }

    /** A single block of a plot (as authored) in the world. */
    static BlockPos world(BlockPos corner, int width, int depth, Direction door, int lx, int lz) {
        int[] r = world(corner.getX(), corner.getZ(), width, depth, door, lx, lz, 1, 1);
        return new BlockPos(r[0], corner.getY(), r[1]);
    }

    /** A plot found for a house: its corner in the world at the house's ground, its street's way, its size as authored. */
    public record Site(BlockPos corner, Direction door, int width, int depth) {}

    /**
     * The nearest plot of a town's plan, fronting a street, where {@code house} stands on ground level enough (or that
     * can be filled level) within reach of the bell's height, its plot off the plan's walls and streets; empty if the
     * plan has no yards or no plot fits.
     */
    public static Optional<Site> site(ServerLevel level, BlockPos bell, Kits.Meta house) {
        return site(level, TownPlans.at(level, bell), bell, house);
    }

    /** As {@link #site(ServerLevel, BlockPos, Kits.Meta)}, on the given plan (the town's own, found by its id). */
    public static Optional<Site> site(ServerLevel level, Optional<TownPlans.Plan> plan, BlockPos bell, Kits.Meta house) {
        if (plan.isEmpty() || !plan.get().lanes() || !plan.get().yards()) {
            return Optional.empty();
        }
        int width = width(house.width()), depth = plan.get().deepest();
        int[] h = house(depth, house.width(), house.depth());
        if (h[1] < 0) {
            return Optional.empty(); // deeper than a plot
        }
        // Why each frontage failed, for the log when none fits.
        int[] why = new int[4];
        var frontages = TownPlans.plots(plan.get(), width);
        for (TownPlans.Frontage f : frontages) {
            if (!TownPlans.clear(level, f.x(), f.z(), f.sx(), f.sz(), false)) {
                why[0]++;
                continue;
            }
            int[] at = world(f.x(), f.z(), width, depth, f.door(), h[0], h[1], h[2], h[3]);
            int y = ground(level, at[0], at[1], at[2], at[3]);
            if (y == Integer.MIN_VALUE) {
                why[1 + Math.max(0, LAST_WHY)]++;
                continue;
            }
            if (Math.abs(y - bell.getY()) > RISE) {
                why[3]++;
                continue;
            }
            return Optional.of(new Site(new BlockPos(f.x(), y, f.z()), f.door(), width, depth));
        }
        dev.b28.enterprise.Enterprise.LOGGER.info("No plot {} wide by {}'s bell: {} frontages, {} on a wall or street, {} built "
                + "on or a town's, {} too steep, {} too high or low", width, bell.toShortString(), frontages.size(), why[0],
                why[1], why[2], why[3]);
        return Optional.empty();
    }

    /** How far above or below the bell a plot's house may stand: a suburb runs down the hill a little. */
    static final int RISE = 12;

    /** How far a house's ground may rise and fall across it, to be filled and cut level by its placing (its cuts retained: `Earthworks`). */
    static final int ROLL = 4;

    /**
     * The ground a house of {@code sx} x {@code sz} with its corner here would stand on: the top of the land's own ground
     * under any trees (its placing fells them), within {@link #ROLL} of level, nothing built on it and nothing of a
     * town's (a block entity, a point of interest) within it; {@link Integer#MIN_VALUE} if not, or not loaded.
     */
    /** Why the last {@link #ground} failed: 0 built on or a town's, 1 too steep. */
    private static int LAST_WHY;

    static int ground(ServerLevel level, int x0, int z0, int sx, int sz) {
        int top = Integer.MIN_VALUE, bottom = Integer.MAX_VALUE;
        LAST_WHY = 0;
        for (int dx = 0; dx < sx; dx++) {
            for (int dz = 0; dz < sz; dz++) {
                int x = x0 + dx, z = z0 + dz;
                int y = FieldMaps.under(level, x, z);
                if (y == Integer.MIN_VALUE) {
                    return Integer.MIN_VALUE;
                }
                BlockPos at = new BlockPos(x, y, z);
                BlockState state = level.getBlockState(at);
                if (!dev.b28.enterprise.settlement.Grounds.natural(state) || state.is(Blocks.FARMLAND)
                        || !state.getFluidState().isEmpty()) {
                    return Integer.MIN_VALUE; // something built, a field, or water
                }
                for (int up = 1; up <= 3; up++) {
                    BlockPos over = at.above(up);
                    if (level.getBlockEntity(over) != null || level.getPoiManager().getType(over).isPresent()) {
                        return Integer.MIN_VALUE;
                    }
                }
                top = Math.max(top, y);
                bottom = Math.min(bottom, y);
            }
        }
        LAST_WHY = 1;
        return top - bottom <= ROLL ? top : Integer.MIN_VALUE;
    }

    /** What was laid on a plot: the house, and the yards' kits (empty where the kingdom has none for it). */
    public record Laid(Kits.Placed house, Optional<String> front, Optional<String> back, int path) {}

    /**
     * Lays a plot: {@code house} (a home kit) in it, a front and a back yard of the kingdom's for its width, and the
     * path from the street to the door. {@code corner} is the plot's in the world at the house's ground; {@code width}
     * and {@code depth} its size as authored. Empty if the house can't be laid.
     */
    public static Optional<Laid> lay(ServerLevel level, String kingdom, Kits.Meta house, BlockPos corner, int width,
            int depth, Direction door, long seed) {
        return lay(level, kingdom, house, null, corner, width, depth, door, seed);
    }

    /**
     * As {@link #lay(ServerLevel, String, Kits.Meta, BlockPos, int, int, Direction, long)}, the house given as composed
     * blocks ({@code built}: a house the generator made, `Houses`) where it isn't a kit.
     */
    public static Optional<Laid> lay(ServerLevel level, String kingdom, Kits.Meta house, Houses.Built built, BlockPos corner,
            int width, int depth, Direction door, long seed) {
        return lay(level, kingdom, house, built, corner, width, depth, door, seed, "village", null);
    }

    /**
     * As above, its yards chosen for the town's {@code tier} and the occupant's {@code trade} (a workshop's: the town
     * rules' `yards.front.by_trade` names the trade's yard, the smith's for the smiths, the baker's for the farmer).
     */
    public static Optional<Laid> lay(ServerLevel level, String kingdom, Kits.Meta house, Houses.Built built, BlockPos corner,
            int width, int depth, Direction door, long seed, String tier, String trade) {
        ResourceManager resources = level.getServer().getResourceManager();
        int[] h = house(depth, house.width(), house.depth());
        int[] at = world(corner.getX(), corner.getZ(), width, depth, door, h[0], h[1], h[2], h[3]);
        BlockPos houseCorner = new BlockPos(at[0], corner.getY(), at[1]);
        Optional<Kits.Placed> placed = built != null
                ? Optional.of(new Kits.Placed(house.id(), Kits.placeCells(level, built.cells(), built.sx(), built.sy(), built.sz(),
                        house.ground(), houseCorner, door), List.of(), List.of()))
                : Kits.place(level, house.id(), houseCorner, door);
        if (placed.isEmpty()) {
            return Optional.empty();
        }
        int doorX = h[0] + house.entrances().stream().findFirst().map(BlockPos::getX).orElse(house.width() / 2);
        // The yards: tiled from the kingdom's yard tiles to the plot's width where it has them (`Yards`: the path tile
        // at the door, lamps in reach, edges matching), else the widest whole yard of its that fits.
        List<Yards.Tile> tiles = Yards.tiles(resources, kingdom);
        Optional<String> front, back;
        int pathFrom = h[1] + house.depth();
        if (!tiles.isEmpty()) {
            String yardTrade = null;
            if (trade != null) {
                var rules = WallKit.json(resources, "town_rules/" + kingdom + ".json");
                yardTrade = rules.map(r -> r.getAsJsonObject("yards")).filter(y -> y.has("front"))
                        .map(y -> y.getAsJsonObject("front")).filter(f -> f.has("by_trade"))
                        .map(f -> f.getAsJsonObject("by_trade")).filter(b -> b.has(trade))
                        .map(b -> b.get(trade).getAsString().replace("trade:", "")).orElse(trade);
            }
            var likes = Yards.likes(tier, yardTrade);
            var f = Yards.fill(tiles, "yard:front", width, doorX, likes, seed);
            f.ifPresent(strip -> Yards.lay(level, strip, corner, width, depth, door, depth - TownPlans.FRONT_YARD));
            front = f.map(strip -> strip.stream().map(l -> l.tile().id()).toList().toString());
            if (f.isPresent()) {
                pathFrom = depth; // the path tile is the way to the door; only the doorstep beyond is cut
            }
            var b = h[1] >= 4 ? Yards.fill(tiles, "yard:back", width, -1, likes, seed + 1) : Optional.<List<Yards.Laid>>empty();
            b.ifPresent(strip -> Yards.lay(level, strip, corner, width, depth, door, h[1] - 4));
            back = b.map(strip -> strip.stream().map(l -> l.tile().id()).toList().toString());
        } else {
            front = yard(level, resources, kingdom, "yard:front", width, TownPlans.FRONT_YARD, seed, corner,
                    width, depth, door, depth - TownPlans.FRONT_YARD);
            back = h[1] <= 0 ? Optional.empty()
                    : yard(level, resources, kingdom, "yard:back", width, h[1], seed + 1, corner, width, depth, door, 0);
        }
        // The path to the door: from its doorstep across the front yard to the street.
        int path = 0;
        for (int lz = pathFrom; lz <= depth; lz++) {
            BlockPos p = world(corner, width, depth, door, doorX, lz);
            int y = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, p.getX(), p.getZ()) - 1;
            BlockPos ground = p.atY(Math.min(y, corner.getY()));
            BlockState top = level.getBlockState(ground);
            if (dev.b28.enterprise.settlement.Grounds.natural(top) || top.canBeReplaced()) {
                level.setBlock(ground, Blocks.DIRT_PATH.defaultBlockState(), 3);
                for (int up = 1; up <= 2; up++) {
                    BlockState over = level.getBlockState(ground.above(up));
                    if (!over.isAir() && (over.canBeReplaced() || over.is(net.minecraft.tags.BlockTags.FLOWERS))) {
                        level.setBlock(ground.above(up), Blocks.AIR.defaultBlockState(), 3);
                    }
                }
                path++;
            }
        }
        // Its edges retained where levelling cut the slope or built up a fill (the kingdom's earthworks).
        int[] plot = world(corner.getX(), corner.getZ(), width, depth, door, 0, 0, width, depth);
        Earthworks.retain(level, kingdom, plot[0], plot[1], plot[0] + plot[2] - 1, plot[1] + plot[3] - 1, corner.getY(), door);
        return Optional.of(new Laid(placed.get(), front, back, path));
    }

    private static Optional<String> yard(ServerLevel level, ResourceManager resources, String kingdom, String slot,
            int width, int depth, long seed, BlockPos corner, int plotWidth, int plotDepth, Direction door, int lz) {
        List<String> fit = Kits.kits(resources, kingdom, slot, width, depth);
        if (fit.isEmpty()) {
            return Optional.empty();
        }
        int widest = fit.stream().mapToInt(id -> Kits.meta(resources, id).map(Kits.Meta::width).orElse(0)).max().orElse(0);
        List<String> best = fit.stream()
                .filter(id -> Kits.meta(resources, id).map(Kits.Meta::width).orElse(0) == widest).toList();
        String id = best.get((int) Math.floorMod(seed * 0x9E3779B97F4A7C15L >>> 7, (long) best.size()));
        Kits.Meta m = Kits.meta(resources, id).orElseThrow();
        int lx = (width - m.width()) / 2;
        // A back yard against the house's back (its region runs from the plot's back edge to the house).
        int z = slot.equals("yard:back") ? lz + depth - m.depth() : lz;
        int[] at = world(corner.getX(), corner.getZ(), plotWidth, plotDepth, door, lx, z, m.width(), m.depth());
        return Kits.place(level, id, new BlockPos(at[0], corner.getY(), at[1]), door).map(p -> id);
    }
}
