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
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerType;
import net.minecraft.world.level.block.BedBlock;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.DoorBlock;
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
    /** A town builds homes at this mood or better, about one every {@link #DAYS_PER_HOME} days, for 8 emeralds. */
    public static final double HOME_MOOD = 0.5;
    public static final double DAYS_PER_HOME = 4;
    public static final double HOME_EMERALDS = 8;

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
        List<BlockPos> corners = new ArrayList<>();
        for (int dx = -FARTHEST; dx <= FARTHEST; dx++) {
            for (int dz = -FARTHEST; dz <= FARTHEST; dz++) {
                double fromBell = Math.hypot(dx + HOME_SIZE / 2.0, dz + HOME_SIZE / 2.0);
                if (fromBell >= NEAREST && fromBell <= FARTHEST) {
                    corners.add(bell.offset(dx, 0, dz));
                }
            }
        }
        corners.sort(Comparator.comparingDouble((BlockPos c) -> c.distSqr(bell)).thenComparing(c -> c));
        for (BlockPos corner : corners) {
            int ground = surface(level, corner.getX(), corner.getZ());
            if (ground != Integer.MIN_VALUE && Math.abs(ground - bell.getY()) <= 4) {
                BlockPos site = new BlockPos(corner.getX(), ground, corner.getZ());
                if (fits(level, site)) {
                    return Optional.of(site);
                }
            }
        }
        return Optional.empty();
    }

    /**
     * Does a cottage fit with its corner here? The 5 x 5 footprint and a block around it must be loaded, level with the
     * corner, solid underfoot (no fields, no water) and open for four blocks up.
     */
    public static boolean fits(ServerLevel level, BlockPos corner) {
        int ground = corner.getY();
        for (int dx = -1; dx <= HOME_SIZE; dx++) {
            for (int dz = -1; dz <= HOME_SIZE; dz++) {
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
     * A cottage: cobblestone floor, plank walls with log corners, a door facing south, two windows, a slab roof, a bed
     * and a lantern. Returns the bed's head, where villagers claim it.
     */
    public static BlockPos buildHome(ServerLevel level, BlockPos corner) {
        int g = corner.getY();
        for (int dx = 0; dx < HOME_SIZE; dx++) {
            for (int dz = 0; dz < HOME_SIZE; dz++) {
                BlockPos column = new BlockPos(corner.getX() + dx, g, corner.getZ() + dz);
                level.setBlock(column, Blocks.COBBLESTONE.defaultBlockState(), 3);
                boolean edgeX = dx == 0 || dx == HOME_SIZE - 1;
                boolean edgeZ = dz == 0 || dz == HOME_SIZE - 1;
                for (int up = 1; up <= 2; up++) {
                    BlockState wall = edgeX && edgeZ ? Blocks.OAK_LOG.defaultBlockState()
                            : edgeX || edgeZ ? Blocks.OAK_PLANKS.defaultBlockState()
                            : Blocks.AIR.defaultBlockState();
                    level.setBlock(column.above(up), wall, 3);
                }
                level.setBlock(column.above(3),
                        Blocks.OAK_SLAB.defaultBlockState().setValue(SlabBlock.TYPE, SlabType.BOTTOM), 3);
            }
        }
        BlockPos door = corner.offset(2, 1, HOME_SIZE - 1);
        BlockState doorState = Blocks.OAK_DOOR.defaultBlockState().setValue(DoorBlock.FACING, Direction.SOUTH);
        level.setBlock(door, doorState.setValue(DoorBlock.HALF, DoubleBlockHalf.LOWER), 3);
        level.setBlock(door.above(), doorState.setValue(DoorBlock.HALF, DoubleBlockHalf.UPPER), 3);
        level.setBlock(corner.offset(0, 2, 2), Blocks.GLASS_PANE.defaultBlockState(), 3);
        level.setBlock(corner.offset(HOME_SIZE - 1, 2, 2), Blocks.GLASS_PANE.defaultBlockState(), 3);
        BlockPos foot = corner.offset(1, 1, 3);
        BlockPos head = foot.north();
        BlockState bed = Blocks.WHITE_BED.defaultBlockState().setValue(BedBlock.FACING, Direction.NORTH);
        level.setBlock(foot, bed.setValue(BedBlock.PART, BedPart.FOOT), 3);
        level.setBlock(head, bed.setValue(BedBlock.PART, BedPart.HEAD), 3);
        level.setBlock(corner.offset(3, 1, 1), Blocks.LANTERN.defaultBlockState(), 3);
        return head;
    }

    /** A baby villager by the bell, of the local kind, marked with its birth time so it can grow up while away. */
    public static Villager bear(ServerLevel level, BlockPos bell) {
        BlockPos at = besideBell(level, bell);
        Villager child = EntityType.VILLAGER.create(level);
        child.moveTo(at.getX() + 0.5, at.getY(), at.getZ() + 0.5, level.random.nextFloat() * 360, 0);
        child.finalizeSpawn(level, level.getCurrentDifficultyAt(at), MobSpawnType.BREEDING, null);
        child.setVillagerData(child.getVillagerData().setType(VillagerType.byBiome(level.getBiome(at))));
        child.setAge(AgeableMob.BABY_START_AGE);
        child.getPersistentData().putLong(BORN, level.getGameTime());
        level.addFreshEntity(child);
        return child;
    }

    /** Open ground next to the bell (two blocks of air over something solid), or on top of it. */
    private static BlockPos besideBell(ServerLevel level, BlockPos bell) {
        for (Direction side : Direction.Plane.HORIZONTAL) {
            BlockPos spot = bell.relative(side);
            if (level.getBlockState(spot).isAir() && level.getBlockState(spot.above()).isAir()
                    && level.getBlockState(spot.below()).isFaceSturdy(level, spot.below(), Direction.UP)) {
                return spot;
            }
        }
        return bell.above();
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
