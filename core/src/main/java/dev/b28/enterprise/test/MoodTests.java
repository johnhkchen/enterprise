package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.settlement.Census;
import dev.b28.enterprise.territory.Comforts;
import dev.b28.enterprise.territory.Territory;
import java.util.List;
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

/** A five-villager town with its Merchant's desk and chest in the arena; each test keeps its own Territory. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class MoodTests {
    private static final BlockPos BELL = new BlockPos(3, 2, 3);
    private static final BlockPos DESK = new BlockPos(5, 2, 5);
    private static final Census FIVE = new Census(5, 5, 5, 5, 0);

    private static GlobalPos town(GameTestHelper helper, Territory territory) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        territory.afterCensus(helper.getLevel(), helper.absolutePos(BELL), FIVE, 1, List.of());
        return GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(BELL));
    }

    private static MerchantStock stock(GameTestHelper helper) {
        return MerchantStock.at(helper.getLevel(), helper.absolutePos(DESK));
    }

    private static void touch(GameTestHelper helper, Territory territory, long at) {
        territory.touch(helper.getLevel(), helper.absolutePos(DESK), stock(helper), at);
    }

    @GameTest(template = "floor")
    public static void comfortsAreTags(GameTestHelper helper) {
        List<String> kinds = Comforts.kinds().stream().map(Comforts::name).toList();
        helper.assertTrue(kinds.equals(List.of("clothing", "furnishings", "reading", "treats")), "four kinds: " + kinds);
        helper.assertTrue(Comforts.kindOf(Items.RED_CARPET).map(Comforts::name).orElse("").equals("furnishings"),
                "a carpet is a furnishing");
        helper.assertTrue(Comforts.kindOf(Items.PAPER).isEmpty(), "paper is no comfort");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void treatsAreNotRoadFood(GameTestHelper helper) {
        Territory territory = new Territory();
        town(helper, territory);
        stock(helper).insert(new ItemStack(Items.COOKIE, 64));
        stock(helper).insert(new ItemStack(Items.PUMPKIN_PIE, 16));
        helper.assertTrue(stock(helper).foodNutrition() == 0, "no road food here: " + stock(helper).foodNutrition());
        helper.assertTrue(stock(helper).eat(10).isEmpty(), "caravans can't eat the treats");
        stock(helper).insert(new ItemStack(Items.BREAD, 4));
        helper.assertTrue(stock(helper).foodNutrition() == 20, "bread they can");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void villagersUseTheirRations(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory);
        stock(helper).insert(new ItemStack(Items.COOKIE, 64));
        long t = helper.getLevel().getGameTime();
        touch(helper, territory, t);
        touch(helper, territory, t + 2 * 24000L);
        // 5 villagers x 0.1 emerald x 2 days = 1 emerald of treats: 18 cookies.
        int left = stock(helper).count(new ItemCost(Items.COOKIE, 1));
        helper.assertTrue(left == 46 || left == 45, "18 cookies eaten: " + (64 - left));
        helper.assertTrue(territory.comfortsMet(bell).get("treats") == 1.0, "treats fully met");
        helper.assertTrue(territory.comfortsMet(bell).get("clothing") == 0.0, "no clothing");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void comfortsLiftTheMood(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory);
        long t = helper.getLevel().getGameTime();
        touch(helper, territory, t);
        for (int day = 1; day <= 6; day++) {
            touch(helper, territory, t + day * 24000L);
        }
        double without = territory.mood(bell);
        helper.assertTrue(without < 0, "no comforts: unhappy, " + without);
        for (var item : List.of(Items.COOKIE, Items.WHITE_CARPET, Items.BOOK)) {
            stock(helper).insert(new ItemStack(item, 64));
        }
        stock(helper).insert(new ItemStack(Items.LEATHER_BOOTS));
        stock(helper).insert(new ItemStack(Items.LEATHER_HELMET));
        for (int day = 7; day <= 15; day++) {
            touch(helper, territory, t + day * 24000L);
        }
        double with = territory.mood(bell);
        helper.assertTrue(with > 2, "all four kinds: content, " + with);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void contentFarmersGrowMore(GameTestHelper helper) {
        helper.assertTrue(Territory.yieldFactor(0) == 1 && Territory.yieldFactor(3) > 1.29 && Territory.yieldFactor(-3) < 0.71,
                "a tenth more or less a point");
        helper.assertTrue(Territory.yieldFactor(20) == 1.5 && Territory.yieldFactor(-20) == 0.5, "within half either way");
        helper.assertTrue(Territory.cultureFactor(2) == 1.5, "a quarter more culture a point");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void crowdingAndUnpaidWagesSour(GameTestHelper helper) {
        Territory territory = new Territory();
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        territory.afterCensus(helper.getLevel(), helper.absolutePos(BELL), new Census(8, 8, 3, 5, 0), 1, List.of());
        GlobalPos bell = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(BELL));
        long t = helper.getLevel().getGameTime();
        touch(helper, territory, t);
        for (int day = 1; day <= 12; day++) {
            touch(helper, territory, t + day * 24000L);
        }
        // Target: -8/8 for size, -0.5 x 5 without beds = -3.5.
        helper.assertTrue(territory.mood(bell) < -3, "eight villagers in three beds: " + territory.mood(bell));
        helper.succeed();
    }
}
