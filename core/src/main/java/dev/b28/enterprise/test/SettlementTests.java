package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.settlement.Census;
import dev.b28.enterprise.settlement.CensusTaker;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.level.block.BedBlock;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.properties.BedPart;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

// Arena floor at y=1; blocks stand at y=2. Censuses look 4 blocks from the bell to stay inside this test's arena.
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class SettlementTests {
    private static final BlockPos BELL = new BlockPos(3, 2, 3);
    private static final int RADIUS = 4;

    /** A bed running east from {@code foot}; returns the head, where vanilla puts the bed's point of interest. */
    private static BlockPos bed(GameTestHelper helper, BlockPos foot) {
        var state = Blocks.RED_BED.defaultBlockState().setValue(BedBlock.FACING, Direction.EAST);
        helper.setBlock(foot, state.setValue(BedBlock.PART, BedPart.FOOT));
        helper.setBlock(foot.east(), state.setValue(BedBlock.PART, BedPart.HEAD));
        return foot.east();
    }

    private static void village(GameTestHelper helper, int beds) {
        helper.setBlock(BELL, Blocks.BELL);
        for (int i = 0; i < beds; i++) {
            bed(helper, new BlockPos(0, 2, 5 + (i % 2)).east(i / 2 * 3));
        }
        helper.setBlock(new BlockPos(1, 2, 0), Blocks.LECTERN);
        helper.setBlock(new BlockPos(3, 2, 0), Blocks.COMPOSTER);
        helper.setBlock(new BlockPos(5, 2, 0), Blocks.FLETCHING_TABLE);
    }

    @GameTest(template = "floor")
    public static void bellBedsAndWorkstationsMakeASettlement(GameTestHelper helper) {
        village(helper, 3);
        Census census = CensusTaker.countNow(helper.getLevel(), helper.absolutePos(BELL), RADIUS);
        helper.assertTrue(census.beds() == 3 && census.workstations() == 3,
                "3 beds and 3 workstations, found " + census.beds() + " and " + census.workstations());
        helper.assertTrue(census.isSettlement(), "a bell with 3 beds and 3 workstations is a settlement");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void theRosterCountsVillagersOutOfSight(GameTestHelper helper) {
        village(helper, 3);
        BlockPos bell = helper.absolutePos(BELL);
        GlobalPos at = GlobalPos.of(helper.getLevel().dimension(), bell);
        var settlements = dev.b28.enterprise.settlement.Settlements.get(helper.getLevel().getServer());
        long now = helper.getLevel().getGameTime();
        var faraway = new net.minecraft.world.level.ChunkPos(bell.offset(100_000, 0, 0)).toLong(); // never loaded
        var here = new net.minecraft.world.level.ChunkPos(bell).toLong(); // loaded, and nobody there
        java.util.UUID asleep = java.util.UUID.randomUUID();
        java.util.UUID gone = java.util.UUID.randomUUID();
        java.util.UUID wandered = java.util.UUID.randomUUID();
        java.util.UUID out = java.util.UUID.randomUUID();
        settlements.setRoster(at, java.util.Map.of(
                asleep, new dev.b28.enterprise.settlement.Settlements.Resident(asleep, faraway, java.util.Optional.empty(),
                        "minecraft:farmer", 2, true, false, now, false),
                gone, new dev.b28.enterprise.settlement.Settlements.Resident(gone, here, java.util.Optional.empty(),
                        "minecraft:farmer", 1, true, false, now, false),
                wandered, new dev.b28.enterprise.settlement.Settlements.Resident(wandered, here, java.util.Optional.empty(),
                        "minecraft:fletcher", 1, true, false, now, false),
                out, new dev.b28.enterprise.settlement.Settlements.Resident(out, here, java.util.Optional.empty(),
                        "minecraft:none", 1, true, false, now, true)));
        Villager baby = helper.spawn(EntityType.VILLAGER, new BlockPos(2, 2, 2));
        baby.setAge(-24000);
        settlements.died(gone);
        Census census = CensusTaker.countNow(helper.getLevel(), bell, RADIUS);
        // The baby, the villager asleep out of sight, the one out with a caravan, and the one not where it was last
        // seen (wandered off the loaded ground: sent for); not the one who died.
        helper.assertTrue(census.villagers() == 4, "4 residents: " + census.villagers());
        var roster = settlements.roster(at);
        helper.assertTrue(roster.containsKey(asleep) && roster.containsKey(out) && roster.containsKey(wandered) && !roster.containsKey(gone)
                && roster.containsKey(baby.getUUID()), "the roster keeps the unseen, drops the dead: " + roster.keySet());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void tooFewBedsIsNotASettlement(GameTestHelper helper) {
        village(helper, 2);
        Census census = CensusTaker.countNow(helper.getLevel(), helper.absolutePos(BELL), RADIUS);
        helper.assertFalse(census.isSettlement(), "2 beds is too few");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void bellsNearPlayersComeUpForCensusOncePerDay(GameTestHelper helper) {
        village(helper, 3);
        BlockPos bell = helper.absolutePos(BELL);
        var server = helper.getLevel().getServer();
        var dimension = helper.getLevel().dimension();
        // A player standing at the bell: in play the lookouts are the players' positions.
        var lookout = java.util.List.of(new CensusTaker.Bell(helper.getLevel(), bell));
        // Other tests' bells nearby may be due too; this bell must be among them until it is counted.
        helper.assertTrue(
                dev.b28.enterprise.settlement.Settlements.get(server).census(GlobalPos.of(dimension, bell)).isEmpty()
                        && CensusTaker.nextDueBell(server, lookout).isPresent(),
                "a bell near a player with no census is due");
        CensusTaker.countNow(helper.getLevel(), bell, RADIUS);
        helper.assertTrue(
                dev.b28.enterprise.settlement.Settlements.get(server).census(GlobalPos.of(dimension, bell)).isPresent(),
                "the census is recorded");
        helper.assertTrue(CensusTaker.nextDueBell(server, lookout).map(found -> !found.pos().equals(bell)).orElse(true),
                "a bell counted just now isn't due again for a day");
        helper.succeed();
    }

    /** A librarian whose bed, lectern and bell are all set; optionally walled into a glass cell where it stands. */
    private static Villager resident(GameTestHelper helper, boolean caged) {
        village(helper, 3);
        BlockPos home = helper.absolutePos(new BlockPos(1, 2, 5)); // head of the first bed
        BlockPos stand = new BlockPos(5, 2, 3);
        if (caged) {
            for (Direction side : Direction.Plane.HORIZONTAL) {
                helper.setBlock(stand.relative(side), Blocks.GLASS);
                helper.setBlock(stand.relative(side).above(), Blocks.GLASS);
            }
            helper.setBlock(stand.above(2), Blocks.GLASS);
        }
        Villager villager = helper.spawn(EntityType.VILLAGER, stand);
        villager.setVillagerData(villager.getVillagerData().setProfession(VillagerProfession.LIBRARIAN));
        var dimension = helper.getLevel().dimension();
        villager.getBrain().setMemory(MemoryModuleType.HOME, GlobalPos.of(dimension, home));
        villager.getBrain().setMemory(MemoryModuleType.JOB_SITE,
                GlobalPos.of(dimension, helper.absolutePos(new BlockPos(1, 2, 0))));
        villager.getBrain().setMemory(MemoryModuleType.MEETING_POINT,
                GlobalPos.of(dimension, helper.absolutePos(BELL)));
        return villager;
    }

    @GameTest(template = "floor")
    public static void villagerWhoCanReachEverythingIsFree(GameTestHelper helper) {
        Villager villager = resident(helper, false);
        helper.assertTrue(CensusTaker.isFree(villager, helper.absolutePos(BELL)), "an open villager is free");
        Census census = CensusTaker.countNow(helper.getLevel(), helper.absolutePos(BELL), RADIUS);
        helper.assertTrue(census.villagers() == 1 && census.free() == 1, "1 villager, free; got " + census);
        helper.assertTrue(census.value() == 1.0, "a free villager is worth 1, got " + census.value());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void cagedVillagerIsNotFree(GameTestHelper helper) {
        Villager villager = resident(helper, true);
        helper.assertFalse(CensusTaker.isFree(villager, helper.absolutePos(BELL)), "a caged villager is not free");
        Census census = CensusTaker.countNow(helper.getLevel(), helper.absolutePos(BELL), RADIUS);
        helper.assertTrue(census.villagers() == 1 && census.free() == 0, "1 villager, confined; got " + census);
        helper.assertTrue(Math.abs(census.value() - Census.CONFINED_WEIGHT) < 1e-9,
                "a confined villager is worth " + Census.CONFINED_WEIGHT + ", got " + census.value());
        helper.succeed();
    }

    /**
     * Dirt from y=2 up to {@code top} over the arena's columns x0..x1, every z, open sky over it (the framework's
     * barriers round the arena taken off, or they'd be its ground).
     */
    private static void ground(GameTestHelper helper, int x0, int x1, int top) {
        for (int x = x0; x <= x1; x++) {
            for (int z = 0; z <= 6; z++) {
                for (int y = 2; y <= 12; y++) {
                    helper.setBlock(new BlockPos(x, y, z), y <= top ? Blocks.DIRT : Blocks.AIR);
                }
            }
        }
    }

    @GameTest(template = "floor")
    public static void aTrenchByTheDeskIsFilledAndWhoeverIsInItLiftedOut(GameTestHelper helper) {
        ground(helper, 0, 2, 6);
        ground(helper, 3, 3, 2); // a trench 4 deep down x=3
        ground(helper, 4, 6, 6);
        Villager stuck = helper.spawn(EntityType.VILLAGER, new BlockPos(3, 3, 3));
        var made = dev.b28.enterprise.settlement.Grounds.make(helper.getLevel(), helper.absolutePos(new BlockPos(3, 2, 3)), 3);
        for (int z = 0; z <= 6; z++) {
            helper.assertBlockPresent(Blocks.DIRT, new BlockPos(3, 6, z));
        }
        helper.assertTrue(made.filled() == 7 && made.fenced() == 0, "the trench's 7 columns filled, no fences: " + made);
        helper.assertTrue(stuck.getY() >= helper.absolutePos(new BlockPos(3, 7, 3)).getY(),
                "the villager in the trench stands on its new ground: y " + stuck.getY());
        stuck.discard();
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aDropTooWideToFillIsFencedAtTheTop(GameTestHelper helper) {
        ground(helper, 0, 3, 6);
        ground(helper, 4, 6, 2); // 4 down, 3 wide to the arena's edge: a valley, not a pit
        var made = dev.b28.enterprise.settlement.Grounds.make(helper.getLevel(), helper.absolutePos(new BlockPos(3, 2, 3)), 3);
        for (int z = 0; z <= 6; z++) {
            helper.assertBlockPresent(Blocks.OAK_FENCE, new BlockPos(3, 7, z));
            helper.assertBlockPresent(Blocks.AIR, new BlockPos(5, 3, z));
        }
        helper.assertTrue(made.filled() == 0 && made.fenced() == 7, "the 7 columns at the edge fenced, none filled: " + made);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void anAlleyBetweenAHouseAndAHillIsFilledAndFencedOffTheRoof(GameTestHelper helper) {
        for (int x = 0; x <= 2; x++) {
            for (int z = 0; z <= 6; z++) {
                for (int y = 2; y <= 12; y++) {
                    helper.setBlock(new BlockPos(x, y, z), y <= 7 ? Blocks.OAK_PLANKS : Blocks.AIR); // a house's wall
                }
            }
        }
        ground(helper, 3, 3, 2); // the alley, 5 down from the hill
        ground(helper, 4, 6, 6);
        var made = dev.b28.enterprise.settlement.Grounds.make(helper.getLevel(), helper.absolutePos(new BlockPos(3, 2, 3)), 3);
        for (int z = 0; z <= 6; z++) {
            helper.assertBlockPresent(Blocks.DIRT, new BlockPos(3, 6, z));
            helper.assertBlockPresent(Blocks.OAK_FENCE, new BlockPos(3, 7, z));
        }
        helper.assertTrue(made.filled() == 7 && made.fenced() == 7, "the alley's 7 columns filled and fenced: " + made);
        helper.succeed();
    }
}
