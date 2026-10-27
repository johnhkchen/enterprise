package dev.b28.enterprise.territory;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.AgeableMob;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.MobSpawnType;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.entity.ai.village.poi.PoiRecord;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerType;
import net.minecraft.world.level.block.BedBlock;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.DoorBlock;
import net.minecraft.world.level.block.Rotation;
import net.minecraft.world.level.block.SlabBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.BedPart;
import net.minecraft.world.level.block.state.properties.DoubleBlockHalf;
import net.minecraft.world.level.block.state.properties.SlabType;
import net.minecraft.world.level.levelgen.Heightmap;
import net.neoforged.neoforge.event.entity.EntityJoinLevelEvent;

/**
 * How a content town grows: it builds homes when every bed is taken, and has children when a bed is free. Both are
 * paid for from the town's warehouse and happen when the town is counted while loaded; the ledger keeps the progress
 * between visits. A home is a small cottage with a bed, put up on flat open ground near the bell. A child is a baby
 * villager; it grows up on the clock, even while its town is unloaded.
 */
public final class Growth {
    /** A town builds homes (two beds each) at this mood or better, about one every {@link #DAYS_PER_HOME} days, for 10 emeralds. */
    public static final double HOME_MOOD = 0.5;
    public static final double DAYS_PER_HOME = 4;
    public static final double HOME_EMERALDS = 10;

    /** A town has children at this mood or better, about one every {@link #DAYS_PER_BIRTH} days. */
    public static final double BIRTH_MOOD = 1;
    public static final double DAYS_PER_BIRTH = 4;
    public static final double BIRTH_EMERALDS = 3;
    public static final int BIRTH_FOOD_POINTS = 36; // vanilla parents need 12 each, and the child eats

    /** Cottages are 5 x 5, with a block of open ground around, between these distances from the bell. */
    public static final int HOME_SIZE = 5;
    private static final int NEAREST = 8;
    private static final int FARTHEST = 32;

    private static final String BORN = "enterprise_born";

    private Growth() {}

    /** The nearest corner (ground level, lowest x and z) where a cottage fits, if any within reach of the bell. */
    public static Optional<BlockPos> homeSite(ServerLevel level, BlockPos bell) {
        // Fronting a lane or an avenue inside the town's inner wall, where it has a plan (John: "lanes").
        Optional<TownPlans.Plan> plan = TownPlans.at(level, bell);
        if (plan.isPresent() && plan.get().lanes()) {
            List<BlockPos> corners = new ArrayList<>();
            for (TownPlans.Frontage f : TownPlans.frontages(plan.get(), HOME_SIZE)) {
                corners.add(new BlockPos(f.x(), bell.getY(), f.z()));
            }
            Optional<BlockPos> fronting = firstFit(level, bell, HOME_SIZE, RISE, corners);
            if (fronting.isPresent()) {
                return fronting;
            }
        }
        return site(level, bell, HOME_SIZE, NEAREST);
    }

    /** The nearest corner where a {@code size} x {@code size} building fits, at least {@code nearest} from the bell. */
    public static Optional<BlockPos> site(ServerLevel level, BlockPos bell, int size, int nearest) {
        return site(level, bell, size, nearest, FARTHEST, RISE, corner -> true);
    }

    /** Ground kept clear round each home, workstation and the bell: no field, pasture or pit within it. */
    public static final int YARD = 3;
    /** Homes and workstations this near a field are its neighbours (see {@link #farmSite}). */
    private static final int NEIGHBOURHOOD = 12;

    /**
     * Where works that dig up the ground (a field and its water, a pasture's fences, a pit) may go: as {@link #site},
     * but zoned off from where people live and work. Nothing of the town's (a bed, a workstation, the bell) within
     * {@link #YARD} of it, and none of its neighbours farther out from the bell than it is, so a field never stands
     * between a home and the bell: the town's farms ring its houses, and never cut anyone off from bed, work or market.
     */
    public static Optional<BlockPos> farmSite(ServerLevel level, BlockPos bell, int size, int nearest) {
        List<BlockPos> town = level.getPoiManager()
                .getInRange(type -> true, bell, TownPlans.farmReach(level, bell, FIELD_FARTHEST) + NEIGHBOURHOOD + size, PoiManager.Occupancy.ANY)
                .map(PoiRecord::getPos)
                .toList();
        return site(level, bell, size, nearest, TownPlans.farmReach(level, bell, FIELD_FARTHEST), FIELD_RISE,
                corner -> zoned(bell, corner, size, town) && TownPlans.clear(level, corner.getX(), corner.getZ(), size, size, true));
    }

    /**
     * How far from the bell a field (or a pasture, a pit) may go: past a generated village's last house, which a
     * field must be beyond to ring it ({@link #zoned}); a village spreads past {@link #FARTHEST}, and its town's fields
     * were never laid.
     */
    static final int FIELD_FARTHEST = 56;
    /**
     * How far above or below the bell a field's ground may be (a home's within {@link #RISE}): a hilltop village's flat
     * ground is down the slope, and nobody lives in a field.
     */
    static final int FIELD_RISE = 10;
    /** How far above or below the bell a home or a hall's ground may be. */
    static final int RISE = 4;

    /** Is a {@code size} x {@code size} footprint with this corner clear of the town's places, and outside them? */
    public static boolean zoned(BlockPos bell, BlockPos corner, int size, List<BlockPos> town) {
        int x0 = corner.getX();
        int z0 = corner.getZ();
        int x1 = x0 + size - 1;
        int z1 = z0 + size - 1;
        double fromBell = Math.sqrt(squaredToBox(bell.getX(), bell.getZ(), x0, z0, x1, z1));
        for (BlockPos place : town) {
            double near = Math.sqrt(squaredToBox(place.getX(), place.getZ(), x0, z0, x1, z1));
            if (near <= YARD) {
                return false;
            }
            if (near <= NEIGHBOURHOOD && Math.hypot(place.getX() - bell.getX(), place.getZ() - bell.getZ()) > fromBell) {
                return false; // a home or workstation beyond it: the field would stand in its way to the bell
            }
        }
        return true;
    }

    private static double squaredToBox(int x, int z, int x0, int z0, int x1, int z1) {
        int dx = Math.max(0, Math.max(x0 - x, x - x1));
        int dz = Math.max(0, Math.max(z0 - z, z - z1));
        return (double) dx * dx + (double) dz * dz;
    }

    private static Optional<BlockPos> site(ServerLevel level, BlockPos bell, int size, int nearest, int farthest, int rise,
            java.util.function.Predicate<BlockPos> allowed) {
        List<BlockPos> corners = new ArrayList<>();
        for (int dx = -farthest; dx <= farthest; dx++) {
            for (int dz = -farthest; dz <= farthest; dz++) {
                double fromBell = Math.hypot(dx + size / 2.0, dz + size / 2.0);
                if (fromBell >= nearest && fromBell <= farthest) {
                    corners.add(bell.offset(dx, 0, dz));
                }
            }
        }
        corners.sort(Comparator.comparingDouble((BlockPos c) -> c.distSqr(bell)).thenComparing(c -> c));
        // (Where it's allowed first: a footprint's place among the town's is cheaper to ask than its ground.)
        corners.removeIf(c -> !allowed.test(c));
        return firstFit(level, bell, size, rise, corners);
    }

    /**
     * The first of {@code corners} (in order) where a {@code size} x {@code size} footprint fits, off the town plans'
     * walls, avenues and lanes: on level ground within {@code rise} of the bell, else on ground that can be filled level.
     */
    static Optional<BlockPos> firstFit(ServerLevel level, BlockPos bell, int size, int rise, List<BlockPos> candidates) {
        List<BlockPos> corners = new ArrayList<>(candidates);
        // Off the town plans' walls and avenues (core mode's `TownPlans`; none in Java mode).
        corners.removeIf(c -> !TownPlans.clear(level, c.getX(), c.getZ(), size, size, false));
        for (BlockPos corner : corners) {
            int ground = surface(level, corner.getX(), corner.getZ());
            if (ground != Integer.MIN_VALUE && Math.abs(ground - bell.getY()) <= rise) {
                BlockPos site = new BlockPos(corner.getX(), ground, corner.getZ());
                if (fits(level, site, size)) {
                    return Optional.of(site);
                }
            }
        }
        // No ground level enough: the nearest natural ground within LEVELLED of it, at its top, to be filled up to it as
        // it's built ({@link #level}). A village on a hillside has no flat 7 x 7 near its bell, and the homes its town
        // paid for were never built.
        for (BlockPos corner : corners) {
            int top = levelled(level, corner, size);
            if (top != Integer.MIN_VALUE && Math.abs(top - bell.getY()) <= rise) {
                return Optional.of(new BlockPos(corner.getX(), top, corner.getZ()));
            }
        }
        return Optional.empty();
    }

    /** How much a site's natural ground may rise and fall across its footprint and a block round, to be filled level. */
    static final int LEVELLED = 2;

    /**
     * The top of the natural ground under a {@code size} x {@code size} footprint with this corner (x and z) and a block
     * round it, if it can be filled level there: loaded, within {@link #LEVELLED} of its top everywhere, every column's
     * top natural ground (no village's or player's blocks, no water, no field) and open for four blocks over the top;
     * else {@link Integer#MIN_VALUE}. Changes nothing.
     */
    public static int levelled(ServerLevel level, BlockPos corner, int size) {
        int top = Integer.MIN_VALUE;
        int bottom = Integer.MAX_VALUE;
        for (int dx = -1; dx <= size; dx++) {
            for (int dz = -1; dz <= size; dz++) {
                int ground = surface(level, corner.getX() + dx, corner.getZ() + dz);
                if (ground == Integer.MIN_VALUE) {
                    return Integer.MIN_VALUE;
                }
                BlockPos floor = new BlockPos(corner.getX() + dx, ground, corner.getZ() + dz);
                BlockState under = level.getBlockState(floor);
                if (!dev.b28.enterprise.settlement.Grounds.natural(under) || under.is(Blocks.FARMLAND)
                        || !under.getFluidState().isEmpty()) {
                    return Integer.MIN_VALUE;
                }
                top = Math.max(top, ground);
                bottom = Math.min(bottom, ground);
            }
        }
        if (top - bottom > LEVELLED) {
            return Integer.MIN_VALUE;
        }
        for (int dx = -1; dx <= size; dx++) {
            for (int dz = -1; dz <= size; dz++) {
                for (int up = 1; up <= 4; up++) {
                    BlockState above = level.getBlockState(new BlockPos(corner.getX() + dx, top + up, corner.getZ() + dz));
                    if (!above.canBeReplaced() || !above.getFluidState().isEmpty()) {
                        return Integer.MIN_VALUE;
                    }
                }
            }
        }
        return top;
    }

    /**
     * The natural ground under a {@code size} x {@code size} footprint at {@code corner} and a block round it filled up
     * to the corner's height, with the ground's own block (dirt under grass): a site {@link #site} found by its
     * fallback, made level for what's built on it. Ground already at the corner's height, or over it, is left.
     */
    public static void level(ServerLevel level, BlockPos corner, int size) {
        for (int dx = -1; dx <= size; dx++) {
            for (int dz = -1; dz <= size; dz++) {
                int x = corner.getX() + dx;
                int z = corner.getZ() + dz;
                int ground = surface(level, x, z);
                if (ground == Integer.MIN_VALUE || ground >= corner.getY()) {
                    continue;
                }
                BlockState top = level.getBlockState(new BlockPos(x, ground, z));
                BlockState fill = top.is(Blocks.GRASS_BLOCK) || top.is(Blocks.DIRT_PATH) ? Blocks.DIRT.defaultBlockState() : top;
                for (int y = ground; y <= corner.getY(); y++) {
                    BlockPos at = new BlockPos(x, y, z);
                    if (y > ground && !level.getBlockState(at).canBeReplaced()) {
                        break;
                    }
                    level.setBlock(at, y == corner.getY() ? top : fill, net.minecraft.world.level.block.Block.UPDATE_ALL);
                }
            }
        }
    }

    /**
     * Does a cottage fit with its corner here? The 5 x 5 footprint and a block around it must be loaded, level with the
     * corner, solid underfoot (no fields, no water) and open for four blocks up.
     */
    public static boolean fits(ServerLevel level, BlockPos corner) {
        return fits(level, corner, HOME_SIZE);
    }

    /** As {@link #fits(ServerLevel, BlockPos)}, for a {@code size} x {@code size} footprint. */
    public static boolean fits(ServerLevel level, BlockPos corner, int size) {
        int ground = corner.getY();
        for (int dx = -1; dx <= size; dx++) {
            for (int dz = -1; dz <= size; dz++) {
                int x = corner.getX() + dx;
                int z = corner.getZ() + dz;
                if (surface(level, x, z) != ground) {
                    return false;
                }
                BlockPos floor = new BlockPos(x, ground, z);
                BlockState under = level.getBlockState(floor);
                if (!under.isFaceSturdy(level, floor, Direction.UP) || under.is(Blocks.FARMLAND)
                        || !under.getFluidState().isEmpty()) {
                    return false;
                }
                for (int up = 1; up <= 4; up++) {
                    BlockState above = level.getBlockState(floor.above(up));
                    if (!above.canBeReplaced() || !above.getFluidState().isEmpty()) {
                        return false;
                    }
                }
            }
        }
        return true;
    }

    /** The top solid block's y at a column, or {@link Integer#MIN_VALUE} if the column isn't loaded. */
    private static int surface(ServerLevel level, int x, int z) {
        if (!level.isLoaded(new BlockPos(x, 0, z))) {
            return Integer.MIN_VALUE;
        }
        return level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z) - 1;
    }

    /**
     * A cottage for a family: cobblestone floor, plank walls with log corners, a door facing south, two windows, a slab
     * roof, two beds either side of the way in, a chest and a lantern. Returns the first bed's head, where villagers
     * claim it.
     */
    public static BlockPos buildHome(ServerLevel level, BlockPos corner) {
        return buildHome(level, corner, Direction.SOUTH);
    }

    /** As {@link #buildHome(ServerLevel, BlockPos)}, its door facing {@code door} (a lane it fronts). */
    public static BlockPos buildHome(ServerLevel level, BlockPos corner, Direction door) {
        level(level, corner, HOME_SIZE);
        Rotation turn = switch (door) {
            case WEST -> Rotation.CLOCKWISE_90;
            case NORTH -> Rotation.CLOCKWISE_180;
            case EAST -> Rotation.COUNTERCLOCKWISE_90;
            default -> Rotation.NONE;
        };
        // A block of the south-facing cottage (its corner's x and z, up from its floor) where it stands turned.
        java.util.function.BiConsumer<int[], BlockState> put = (at, state) -> {
            int n = HOME_SIZE - 1, dx = at[0], dz = at[2];
            int[] turned = switch (turn) {
                case CLOCKWISE_90 -> new int[] {n - dz, dx};
                case CLOCKWISE_180 -> new int[] {n - dx, n - dz};
                case COUNTERCLOCKWISE_90 -> new int[] {dz, n - dx};
                default -> new int[] {dx, dz};
            };
            level.setBlock(corner.offset(turned[0], at[1], turned[1]), state.rotate(turn), 3);
        };
        for (int dx = 0; dx < HOME_SIZE; dx++) {
            for (int dz = 0; dz < HOME_SIZE; dz++) {
                put.accept(new int[] {dx, 0, dz}, Blocks.COBBLESTONE.defaultBlockState());
                boolean edgeX = dx == 0 || dx == HOME_SIZE - 1;
                boolean edgeZ = dz == 0 || dz == HOME_SIZE - 1;
                for (int up = 1; up <= 2; up++) {
                    BlockState wall = edgeX && edgeZ ? Blocks.OAK_LOG.defaultBlockState()
                            : edgeX || edgeZ ? Blocks.OAK_PLANKS.defaultBlockState()
                            : Blocks.AIR.defaultBlockState();
                    put.accept(new int[] {dx, up, dz}, wall);
                }
                put.accept(new int[] {dx, 3, dz}, Blocks.OAK_SLAB.defaultBlockState().setValue(SlabBlock.TYPE, SlabType.BOTTOM));
            }
        }
        BlockState doorState = Blocks.OAK_DOOR.defaultBlockState().setValue(DoorBlock.FACING, Direction.SOUTH);
        put.accept(new int[] {2, 1, HOME_SIZE - 1}, doorState.setValue(DoorBlock.HALF, DoubleBlockHalf.LOWER));
        put.accept(new int[] {2, 2, HOME_SIZE - 1}, doorState.setValue(DoorBlock.HALF, DoubleBlockHalf.UPPER));
        put.accept(new int[] {0, 2, 2}, Blocks.GLASS_PANE.defaultBlockState());
        put.accept(new int[] {HOME_SIZE - 1, 2, 2}, Blocks.GLASS_PANE.defaultBlockState());
        BlockState bed = Blocks.WHITE_BED.defaultBlockState().setValue(BedBlock.FACING, Direction.NORTH);
        for (int side : new int[] {1, 3}) {
            put.accept(new int[] {side, 1, 3}, bed.setValue(BedBlock.PART, BedPart.FOOT));
            put.accept(new int[] {side, 1, 2}, bed.setValue(BedBlock.PART, BedPart.HEAD));
        }
        put.accept(new int[] {1, 1, 1}, Blocks.CHEST.defaultBlockState());
        put.accept(new int[] {3, 1, 1}, Blocks.LANTERN.defaultBlockState());
        // The first bed's head, where it stands turned.
        int n = HOME_SIZE - 1;
        int[] head = switch (turn) {
            case CLOCKWISE_90 -> new int[] {n - 2, 1};
            case CLOCKWISE_180 -> new int[] {n - 1, n - 2};
            case COUNTERCLOCKWISE_90 -> new int[] {2, n - 1};
            default -> new int[] {1, 2};
        };
        return corner.offset(head[0], 1, head[1]);
    }

    /** A baby villager by the bell, of the local kind, marked with its birth time so it can grow up while away. */
    public static Villager bear(ServerLevel level, BlockPos bell) {
        BlockPos at = dev.b28.enterprise.settlement.Grounds.standing(level, bell);
        Villager child = EntityType.VILLAGER.create(level);
        child.moveTo(at.getX() + 0.5, at.getY(), at.getZ() + 0.5, level.random.nextFloat() * 360, 0);
        child.finalizeSpawn(level, level.getCurrentDifficultyAt(at), MobSpawnType.BREEDING, null);
        child.setVillagerData(child.getVillagerData().setType(VillagerType.byBiome(level.getBiome(at))));
        child.setAge(AgeableMob.BABY_START_AGE);
        child.getPersistentData().putLong(BORN, level.getGameTime());
        level.addFreshEntity(child);
        return child;
    }

    /** A child loaded back in grows by the time it was away: babies only age while their chunk ticks. */
    public static void onJoin(EntityJoinLevelEvent event) {
        if (event.loadedFromDisk() && event.getLevel() instanceof ServerLevel level
                && event.getEntity() instanceof Villager villager) {
            growUp(villager, level.getGameTime());
        }
    }

    /** Sets a child's age from its birth time; grown, it's no longer marked. */
    public static void growUp(Villager villager, long now) {
        if (!villager.isBaby() || !villager.getPersistentData().contains(BORN)) {
            return;
        }
        long age = AgeableMob.BABY_START_AGE + (now - villager.getPersistentData().getLong(BORN));
        if (age >= 0) {
            villager.setAge(0);
            villager.getPersistentData().remove(BORN);
        } else {
            villager.setAge((int) age);
        }
    }
}
