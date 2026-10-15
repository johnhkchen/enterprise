package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.settlement.Census;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Labour;
import dev.b28.enterprise.territory.Territory;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
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

/** Hands without a trade: a five-villager town with its desk and chest in the arena; its own Territory each test. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class LabourTests {
    private static final BlockPos BELL = new BlockPos(3, 2, 3);
    private static final BlockPos DESK = new BlockPos(5, 2, 5);

    static GlobalPos town(GameTestHelper helper, Territory territory, String... trades) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        GlobalPos bell = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(BELL));
        Map<UUID, Settlements.Resident> roster = new java.util.LinkedHashMap<>();
        for (String trade : trades) {
            UUID id = UUID.randomUUID();
            roster.put(id, new Settlements.Resident(id, 0, Optional.empty(), trade, 1, true, false, 0, false));
        }
        Settlements.get(helper.getLevel().getServer()).setRoster(bell, roster);
        territory.afterCensus(helper.getLevel(), helper.absolutePos(BELL), new Census(5, 5, 5, 5, 0), 1, List.of());
        return bell;
    }

    static MerchantStock stock(GameTestHelper helper) {
        return MerchantStock.at(helper.getLevel(), helper.absolutePos(DESK));
    }

    @GameTest(template = "floor")
    public static void wagesFollowTheLabourMarketAndOwnersTakeMore(GameTestHelper helper) {
        helper.assertTrue(dev.b28.enterprise.territory.Households.tightness(100, 100, 0) == 1, "every hand busy, no plot idle");
        helper.assertTrue(dev.b28.enterprise.territory.Households.tightness(100, 100, 50) == 1.5,
                "plots going untended: hands are scarce, the wage rises (up to half again)");
        helper.assertTrue(dev.b28.enterprise.territory.Households.tightness(100, 40, 0) == 0.5,
                "hands idle: the wage falls (to half at most)");
        helper.assertTrue(dev.b28.enterprise.territory.Households.share(0.25, 0) == 0.25, "hands on the estate: the wage");
        helper.assertTrue(dev.b28.enterprise.territory.Households.share(0.25, 1) == 0.75, "owners take three quarters");
        helper.assertTrue(dev.b28.enterprise.territory.Households.share(0.9, 1) == 0.9, "or the wage, if it's more");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aNitwitGathersWhatTheLandGives(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory, Labour.NITWIT);
        long t = helper.getLevel().getGameTime();
        territory.touch(helper.getLevel(), helper.absolutePos(DESK), stock(helper), t + 24000);
        var hauls = Labour.hauls(Labour.biome(helper.getLevel(), Hex.at(bell.pos())));
        var first = hauls.get(0);
        int got = stock(helper).count(new ItemCost(first.item(), 1));
        helper.assertTrue(got == (int) Math.floor(first.perDay() * Labour.NITWIT_FACTOR),
                "a day of a Nitwit's gathering: " + got + " " + first.item());
        Settlements.get(helper.getLevel().getServer()).setRoster(bell, Map.of());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void handsStandWatchForTheirFood(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory, Labour.NITWIT);
        territory.setGuards(bell, 1);
        stock(helper).insert(new ItemStack(Items.BREAD, 10));
        stock(helper).insert(new ItemStack(Items.EMERALD, 10));
        long t = helper.getLevel().getGameTime();
        territory.touch(helper.getLevel(), helper.absolutePos(DESK), stock(helper), t);
        territory.touch(helper.getLevel(), helper.absolutePos(DESK), stock(helper), t + 2 * 24000); // before a toll review
        helper.assertTrue(territory.militia(bell) == 1, "the Nitwit stands watch");
        helper.assertTrue(stock(helper).count(new ItemCost(Items.EMERALD, 1)) == 10, "unpaid: "
                + stock(helper).count(new ItemCost(Items.EMERALD, 1)));
        helper.assertTrue(stock(helper).count(new ItemCost(Items.BREAD, 1)) < 10, "but fed");
        helper.assertTrue(territory.guards(bell) == 1, "and stays");
        Settlements.get(helper.getLevel().getServer()).setRoster(bell, Map.of());
        helper.succeed();
    }
}
