package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.service.Services;
import dev.b28.enterprise.service.Visit;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Buildings;
import dev.b28.enterprise.territory.Smithing;
import dev.b28.enterprise.territory.Territory;
import dev.b28.enterprise.territory.Works;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/** Masons' buildings and Fletchers' hunting; the town of {@link LabourTests}. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class BuildingTests {
    private static final BlockPos DESK = new BlockPos(5, 2, 5);

    private static GlobalPos desk(GameTestHelper helper) {
        return GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(DESK));
    }

    private static void forget(GameTestHelper helper, GlobalPos bell) {
        Settlements.get(helper.getLevel().getServer()).setRoster(bell, Map.of());
        Works.get(helper.getLevel().getServer()).forgetForTest(bell);
    }

    @GameTest(template = "floor")
    public static void aWatchtowerHasALadderUp(GameTestHelper helper) {
        BlockPos corner = helper.absolutePos(new BlockPos(1, 1, 1));
        Buildings.build(helper.getLevel(), Buildings.WATCHTOWER, corner);
        helper.assertTrue(helper.getLevel().getBlockState(corner.offset(1, 3, 1)).is(Blocks.LADDER), "a ladder inside");
        helper.assertTrue(helper.getLevel().getBlockState(corner.offset(0, Buildings.WATCHTOWER.height(), 0))
                .is(Blocks.OAK_FENCE), "a fenced platform on top");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aSquareMakesItsTownHappier(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = LabourTests.town(helper, territory);
        double before = territory.moodTargetForTest(bell);
        Works works = Works.get(helper.getLevel().getServer());
        works.finish(works.startBuilding(bell, Buildings.SQUARE, helper.absolutePos(new BlockPos(1, 1, 1))));
        territory.touch(helper.getLevel(), desk(helper).pos(), LabourTests.stock(helper), helper.getLevel().getGameTime());
        double after = territory.moodTargetForTest(bell);
        helper.assertTrue(Math.abs(after - before - Buildings.SQUARE_MOOD) < 1e-9, "a square: +0.5, " + before + " -> " + after);
        forget(helper, bell);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aVisitingMasonLaysADaysWork(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = LabourTests.town(helper, territory);
        Works works = Works.get(helper.getLevel().getServer());
        var temple = works.startBuilding(bell, Buildings.TEMPLE, helper.absolutePos(new BlockPos(1, 1, 1)));
        GlobalPos home = GlobalPos.of(bell.dimension(), bell.pos().offset(500, 0, 0));
        var masonry = Services.get("enterprise:masonry_abroad").orElseThrow();
        Visit visit = Visit.abroad(helper.getLevel(), territory, home, desk(helper), 2, LabourTests.stock(helper), 5,
                null, null, Map.of(), helper.getLevel().getGameTime());
        helper.assertTrue(masonry.wanted().test(visit), "a temple under way and no Mason: wanted");
        double fee = masonry.perform().applyAsDouble(visit);
        helper.assertTrue(Math.abs(temple.progress() - 64.0 / Buildings.TEMPLE.blocks()) < 1e-9 && fee == 2,
                "a level-2 Mason lays 64 blocks for 2 emeralds: " + temple.progress() + ", " + fee);
        forget(helper, bell);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aFletcherHuntsWithABow(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = LabourTests.town(helper, territory, Territory.FLETCHER);
        LabourTests.stock(helper).insert(new ItemStack(Items.BOW, 1));
        long t = helper.getLevel().getGameTime();
        territory.touch(helper.getLevel(), desk(helper).pos(), LabourTests.stock(helper), t);
        territory.touch(helper.getLevel(), desk(helper).pos(), LabourTests.stock(helper), t + 24000);
        int feathers = LabourTests.stock(helper).count(new ItemCost(Items.FEATHER, 1));
        helper.assertTrue(territory.hunting(bell) == 1.0 && feathers >= 1, "a day's hunt with a bow: " + feathers + " feathers");
        forget(helper, bell);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void sparesBowsArmTheGuards(GameTestHelper helper) {
        helper.setBlock(DESK, dev.b28.enterprise.merchant.Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        LabourTests.stock(helper).insert(new ItemStack(Items.BOW, 2));
        LabourTests.stock(helper).insert(new ItemStack(Items.IRON_SWORD, 1));
        helper.assertTrue(Smithing.armed(LabourTests.stock(helper), 2, 1, 0) == 1.0,
                "a sword and the bow the hunter doesn't need arm both guards");
        helper.assertTrue(Smithing.armed(LabourTests.stock(helper), 4, 1, 0.5) == 0.5, "or half of four");
        helper.succeed();
    }
}
