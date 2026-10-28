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
 * (its side yard either side), and its back yard (what's left of the plot's depth). The house is the town's
 * kingdom's home kit; each yard is a kit of its own ({@code yard:front}, {@code yard:back}, the studio's), chosen per
 * plot; and a path is cut from the street to the house's door wherever its door fell.
 *
 * <p>Positions in a plot are as authored: x along the street, z from the back (0) to the street ({@code depth - 1}),
 * the street to the south; {@link #world} turns them to the plot's street.
 */
public final class Plots {
    private Plots() {}

    /**
     * A house's side yard either side: two blocks, so neighbours stand four apart (John: "poor design in how crowded the
     * homes are"; one block each side read as a terrace of separate houses).
     */
    public static final int SIDE = 2;

    /** A plot's width for a house {@code width} wide: its side yard either side. */
    public static int width(int house) {
        return house + 2 * SIDE;
    }

    /** Where the house stands in its plot: its corner (x, z) as authored, its width and depth. */
    static int[] house(int plotDepth, int width, int depth) {
        return house(plotDepth, width, depth, width(width));
    }

    /** As {@link #house(int, int, int)}, in a plot {@code plotWidth} wide: the house in its middle (none either side on a terrace). */
    static int[] house(int plotDepth, int width, int depth, int plotWidth) {
        return new int[] {Math.max(0, (plotWidth - width) / 2), plotDepth - TownPlans.FRONT_YARD - depth, width, depth};
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
        int depth = plan.get().deepest();
        if ((house.whole() ? depth - house.depth() : house(depth, house.width(), house.depth())[1]) < 0) {
            return Optional.empty(); // deeper than a plot
        }
        // Why each frontage failed, for the log when none fits.
        int[] why = new int[4];
        // A lot whole as wide as it is (its compound wall its edge); a house with its side yards.
        int plotWidth = house.whole() ? house.width() : width(house.width());
        // A founded town's main street first (its first builds face each other across it).
        var main = level.getServer() == null ? java.util.Optional.<net.minecraft.core.Direction>empty()
                : TownPlans.get(level.getServer()).mainAt(bell);
        // A founded town's plots are the grade's own (its main street's pads, 14 wide: r81, a 12-wide plot packed on the
        // avenue straddled two pads where the street climbs, too steep to build by the square).
        int graded = Math.max(width(10), plotWidth);
        var frontages = TownPlans.mainFirst(plan.get(), main.isPresent() ? TownPlans.plots(plan.get(), graded)
                : TownPlans.plots(plan.get(), house.width(), plotWidth), main);
        int[] mainWhy = new int[4];
        for (TownPlans.Frontage f : frontages) {
            int entrance = house.entrances().stream().findFirst().map(BlockPos::getX).orElse(house.width() / 2);
            Optional<Site> site = fits(level, f, bell, depth, house.width(), house.depth(), entrance, house.whole());
            if (site.isPresent()) {
                if (main.isPresent() && !TownPlans.onMain(plan.get(), f, main.get())) {
                    // Off the main street: why its plots wouldn't take it (a founded town's first builds face across it).
                    dev.b28.enterprise.Enterprise.LOGGER.info("A house {} wide by {}'s bell off its main street: {} on a wall or "
                            + "street, {} built on or a town's, {} too steep, {} too high or low", house.width(),
                            bell.toShortString(), mainWhy[0], mainWhy[1], mainWhy[2], mainWhy[3]);
                }
                return site;
            }
            why[LAST_FIT]++;
            if (main.isPresent() && TownPlans.onMain(plan.get(), f, main.get())) {
                mainWhy[LAST_FIT]++;
                if (System.getenv("ENTERPRISE_GRADE_PROBE") != null) {
                    dev.b28.enterprise.Enterprise.LOGGER.info("Main street plot {} refused: {} (built on: {})", f, LAST_FIT, BLOCKED);
                }
            }
        }
        // Said when the reasons change, not at every look the town asks again (r35: 445 lines of one full town).
        String said = String.format("No plot for a house %d wide by %s's bell: %d frontages, %d on a wall or street, %d "
                + "built on or a town's, %d too steep, %d too high or low", house.width(), bell.toShortString(), frontages.size(),
                why[0], why[1], why[2], why[3]);
        String on = BLOCKED.isEmpty() ? "" : " (built on: " + BLOCKED.entrySet().stream()
                .sorted(java.util.Map.Entry.<String, Integer>comparingByValue().reversed()).limit(4)
                .map(e -> e.getValue() + " " + e.getKey()).collect(java.util.stream.Collectors.joining(", ")) + ")";
        BLOCKED.clear();
        if (!said.equals(SAID.put(bell.asLong() * 31 + house.width(), said))) {
            dev.b28.enterprise.Enterprise.LOGGER.info(said + on);
        }
        return Optional.empty();
    }

    /** The last refusal said for each bell and width. */
    private static final java.util.Map<Long, String> SAID = new java.util.concurrent.ConcurrentHashMap<>();

    /** Why the last {@link #fits} failed: 0 on a wall or street, 1 built on or a town's, 2 too steep, 3 too high or low. */
    static int LAST_FIT;

    /**
     * Does a house {@code houseW} x {@code houseD} (its door {@code entrance} along its front) fit the frontage, its plot
     * as wide as the frontage along its street and {@code depth} deep: off the plan's walls and streets, its ground level
     * enough and nothing built on it, its surroundings bankable, within reach of the bell's height? Its site if so.
     */
    public static Optional<Site> fits(ServerLevel level, TownPlans.Frontage f, BlockPos bell, int depth, int houseW,
            int houseD, int entrance) {
        return fits(level, f, bell, depth, houseW, houseD, entrance, false);
    }

    /** As {@link #fits(ServerLevel, TownPlans.Frontage, BlockPos, int, int, int, int)}; {@code whole}: a lot whole, to the plot's back from its street, no front yard but its own. */
    public static Optional<Site> fits(ServerLevel level, TownPlans.Frontage f, BlockPos bell, int depth, int houseW,
            int houseD, int entrance, boolean whole) {
        // Its width along its street: the house's alone on an avenue's terrace, with side yards elsewhere.
        int width = f.door().getAxis() == Direction.Axis.Z ? f.sx() : f.sz();
        int[] h = whole ? new int[] {Math.max(0, (width - houseW) / 2), depth - houseD, houseW, houseD}
                : house(depth, houseW, houseD, width);
        if (h[1] < 0) {
            LAST_FIT = 0;
            return Optional.empty();
        }
        if (!TownPlans.clear(level, f.x(), f.z(), f.sx(), f.sz(), false)) {
            LAST_FIT = 0;
            return Optional.empty();
        }
        int[] at = world(f.x(), f.z(), width, depth, f.door(), h[0], h[1], h[2], h[3]);
        int y = ground(level, at[0], at[1], at[2], at[3]);
        if (y == Integer.MIN_VALUE) {
            LAST_FIT = 1 + Math.max(0, LAST_WHY);
            return Optional.empty();
        }
        int doorX = h[0] + entrance;
        if (!bankable(level, world(f.x(), f.z(), width, depth, f.door(), 0, 0, width, depth), y,
                world(new BlockPos(f.x(), y, f.z()), width, depth, f.door(), doorX, depth))) {
            LAST_FIT = 2;
            return Optional.empty();
        }
        if (Math.abs(y - bell.getY()) > RISE) {
            LAST_FIT = 3;
            return Optional.empty();
        }
        return Optional.of(new Site(new BlockPos(f.x(), y, f.z()), f.door(), width, depth));
    }

    /**
     * How far above or below the bell a plot's house may stand: a suburb runs down the hill a little. Graded land stands
     * each plot at its street's level, so it's wide (r78: a founded town's main street fell 13 below its bell).
     */
    static final int RISE = 24;

    /** How far a house's ground may rise and fall across it, to be filled and cut level by its placing (its cuts retained: `Earthworks`). */
    static final int ROLL = 4;

    /**
     * The ground a house of {@code sx} x {@code sz} with its corner here would stand on: the top of the land's own ground
     * under any trees (its placing fells them), within {@link #ROLL} of level, nothing built on it and nothing of a
     * town's (a block entity, a point of interest) within it; {@link Integer#MIN_VALUE} if not, or not loaded.
     */
    /** Why the last {@link #ground} failed: 0 built on or a town's, 1 too steep. */
    private static int LAST_WHY;

    /** What stood on the plots refused as built on, by block, since the last refusal said (for the log). */
    private static final java.util.Map<String, Integer> BLOCKED = new java.util.TreeMap<>();

    static int ground(ServerLevel level, int x0, int z0, int sx, int sz) {
        return ground(level, x0, z0, sx, sz, ROLL);
    }

    /** As {@link #ground(ServerLevel, int, int, int, int)}, its ground within {@code roll} of level. */
    static int ground(ServerLevel level, int x0, int z0, int sx, int sz, int roll) {
        int top = Integer.MIN_VALUE, bottom = Integer.MAX_VALUE;
        LAST_WHY = 0;
        for (int dx = 0; dx < sx; dx++) {
            for (int dz = 0; dz < sz; dz++) {
                int x = x0 + dx, z = z0 + dz;
                int y = FieldMaps.under(level, x, z);
                if (y == Integer.MIN_VALUE) {
                    BLOCKED.merge("unloaded", 1, Integer::sum);
                    return Integer.MIN_VALUE;
                }
                BlockPos at = new BlockPos(x, y, z);
                BlockState state = level.getBlockState(at);
                if (!dev.b28.enterprise.settlement.Grounds.natural(state) || state.is(Blocks.FARMLAND)
                        || !state.getFluidState().isEmpty()) {
                    BLOCKED.merge(net.minecraft.core.registries.BuiltInRegistries.BLOCK.getKey(state.getBlock()).getPath(), 1,
                            Integer::sum);
                    return Integer.MIN_VALUE; // something built, a field, or water
                }
                for (int up = 1; up <= 3; up++) {
                    BlockPos over = at.above(up);
                    if (level.getBlockEntity(over) != null || level.getPoiManager().getType(over).isPresent()) {
                        BLOCKED.merge("a town's " + net.minecraft.core.registries.BuiltInRegistries.BLOCK
                                .getKey(level.getBlockState(over).getBlock()).getPath(), 1, Integer::sum);
                        return Integer.MIN_VALUE;
                    }
                }
                top = Math.max(top, y);
                bottom = Math.min(bottom, y);
            }
        }
        LAST_WHY = 1;
        return top - bottom <= roll ? top : Integer.MIN_VALUE;
    }

    /**
     * Levels a plot ({@code x0, z0} to {@code x1, z1}, inclusive) at {@code ground}, its walking surface: the land's own
     * ground cut down to it and filled up to it, grass on top, and its trees felled (the studio's renders: houses in
     * stepped dirt between their yards, acacias against their walls). Anything built is left alone.
     */
    public static void level(ServerLevel level, int x0, int z0, int x1, int z1, int ground) {
        BlockState dirt = Blocks.DIRT.defaultBlockState();
        for (int x = x0; x <= x1; x++) {
            for (int z = z0; z <= z1; z++) {
                int top = Earthworks.height(level, x, z);
                if (top == Integer.MIN_VALUE) {
                    continue;
                }
                for (int y = ground + 1; y <= Math.max(top, ground) + 16; y++) {
                    BlockPos p = new BlockPos(x, y, z);
                    BlockState s = level.getBlockState(p);
                    if (s.is(net.minecraft.tags.BlockTags.LOGS) || s.is(net.minecraft.tags.BlockTags.LEAVES)
                            || dev.b28.enterprise.settlement.Grounds.natural(s) || !s.isAir() && s.canBeReplaced()) {
                        level.setBlock(p, Blocks.AIR.defaultBlockState(), 2);
                    }
                }
                for (int y = top + 1; y < ground; y++) {
                    BlockPos p = new BlockPos(x, y, z);
                    BlockState s = level.getBlockState(p);
                    if (s.isAir() || s.canBeReplaced() || s.is(net.minecraft.tags.BlockTags.LOGS)) {
                        level.setBlock(p, dirt, 2);
                    }
                }
                BlockPos surface = new BlockPos(x, ground, z);
                BlockState s = level.getBlockState(surface);
                if (s.isAir() || s.canBeReplaced() || s.is(net.minecraft.tags.BlockTags.LOGS)
                        || dev.b28.enterprise.settlement.Grounds.natural(s) && !s.is(Blocks.DIRT_PATH)) {
                    level.setBlock(surface, Blocks.GRASS_BLOCK.defaultBlockState(), 2);
                }
            }
        }
    }

    /**
     * Lays a kit that fills its plot whole (a farmstead: house, barn, yard and garden in one, the studio's batch 025): the
     * plot levelled at {@code corner}'s height, the kit turned to {@code door}, its edges retained. Empty if the kit can't
     * be placed.
     */
    public static Optional<Laid> layWhole(ServerLevel level, String kingdom, Kits.Meta kit, BlockPos corner, int width,
            int depth, Direction door) {
        int[] plot = world(corner.getX(), corner.getZ(), width, depth, door, 0, 0, width, depth);
        level(level, plot[0], plot[1], plot[0] + plot[2] - 1, plot[1] + plot[3] - 1, corner.getY());
        // Its front on the plot's street, in the middle of its width.
        int[] at = world(corner.getX(), corner.getZ(), width, depth, door, Math.max(0, (width - kit.width()) / 2),
                Math.max(0, depth - kit.depth()), kit.width(), kit.depth());
        Optional<Kits.Placed> placed = Kits.place(level, kit.id(), new BlockPos(at[0], corner.getY(), at[1]), door);
        if (placed.isEmpty()) {
            return Optional.empty();
        }
        var banked = Earthworks.retain(level, kingdom, plot[0], plot[1], plot[0] + plot[2] - 1, plot[1] + plot[3] - 1,
                corner.getY(), door, null);
        return Optional.of(new Laid(placed.get(), Optional.empty(), Optional.empty(), 0, banked, false));
    }

    /**
     * Can a plot ({x, z, sx, sz} in the world) levelled at {@code ground} be banked: the land just round it within
     * {@link Earthworks#MOST} of the ground (two courses of retaining wall at most), and the street at its door within
     * a step of it (a flight of one fits the front yard).
     */
    static boolean bankable(ServerLevel level, int[] plot, int ground, BlockPos doorOut) {
        for (int x = plot[0] - 1; x <= plot[0] + plot[2]; x++) {
            for (int z = plot[1] - 1; z <= plot[1] + plot[3]; z++) {
                if (x >= plot[0] && x < plot[0] + plot[2] && z >= plot[1] && z < plot[1] + plot[3]) {
                    continue;
                }
                int y = Earthworks.height(level, x, z);
                if (y != Integer.MIN_VALUE && Math.abs(y - ground) > Earthworks.MOST) {
                    return false;
                }
            }
        }
        int street = Earthworks.height(level, doorOut.getX(), doorOut.getZ());
        return street == Integer.MIN_VALUE || Math.abs(street - ground) <= 1;
    }

    /** What was laid on a plot: the house, and the yards' kits (empty where the kingdom has none for it). */
    public record Laid(Kits.Placed house, Optional<String> front, Optional<String> back, int path, Earthworks.Banked banked,
            boolean steps) {}

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
        int[] h = house(depth, house.width(), house.depth(), width);
        int[] plot = world(corner.getX(), corner.getZ(), width, depth, door, 0, 0, width, depth);
        level(level, plot[0], plot[1], plot[0] + plot[2] - 1, plot[1] + plot[3] - 1, corner.getY());
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
        // On beyond the plot to the way it fronts, where that's a few blocks off over grass (a farmhouse's lot stands back
        // from its track): the path on till it meets a path.
        for (int lz = depth + 1; lz <= depth + 6; lz++) {
            BlockPos p = world(corner, width, depth, door, doorX, lz);
            BlockPos ground = p.atY(level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, p.getX(), p.getZ()) - 1);
            BlockState top = level.getBlockState(ground);
            if (!top.is(Blocks.GRASS_BLOCK) && !top.is(Blocks.DIRT) && !top.is(Blocks.COARSE_DIRT)
                    || Math.abs(ground.getY() - corner.getY()) > 2) {
                break;
            }
            level.setBlock(ground, Blocks.DIRT_PATH.defaultBlockState(), 3);
            BlockState over = level.getBlockState(ground.above());
            if (!over.isAir() && over.canBeReplaced()) {
                level.setBlock(ground.above(), Blocks.AIR.defaultBlockState(), 3);
            }
            path++;
        }
        // A step of one between the street and the plot at the door: the kingdom's 1-wide flight in the front yard, by
        // half steps (the studio's renders: doorsteps reached up full blocks).
        BlockPos out = world(corner, width, depth, door, doorX, depth);
        int street = Earthworks.height(level, out.getX(), out.getZ());
        boolean steps = false;
        int[] open = null;
        if (street != Integer.MIN_VALUE && Math.abs(street - corner.getY()) == 1 && doorX >= 1 && doorX + 1 < width) {
            boolean plotHigh = street < corner.getY();
            // The street's columns at the flight's foot, left open by the kerb.
            open = world(corner.getX(), corner.getZ(), width, depth, door, doorX - 1, depth, 3, 1);
            steps = Earthworks.steps(level, kingdom, world(corner.getX(), corner.getZ(), width, depth, door, doorX - 1,
                    depth - TownPlans.FRONT_YARD, 3, TownPlans.FRONT_YARD), plotHigh ? door.getOpposite() : door,
                    Math.min(street, corner.getY()));
        }
        // Its edges retained where levelling cut the slope or built up a fill (the kingdom's earthworks).
        var banked = Earthworks.retain(level, kingdom, plot[0], plot[1], plot[0] + plot[2] - 1, plot[1] + plot[3] - 1,
                corner.getY(), door, steps ? open : null);
        return Optional.of(new Laid(placed.get(), front, back, path, banked, steps));
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
