package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.service.Services;
import dev.b28.enterprise.service.Visit;
import dev.b28.enterprise.settlement.Census;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Mines;
import dev.b28.enterprise.territory.Smithing;
import dev.b28.enterprise.territory.Territory;
import java.util.HashMap;
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

/** The metal chain: a five-villager town with its desk and chest in the arena; each test keeps its own Territory. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class MetalTests {
    private static final BlockPos BELL = new BlockPos(3, 2, 3);
    private static final BlockPos DESK = new BlockPos(5, 2, 5);

    /** The town, with residents of these trades on its roster; no money, so it digs no mine of its own. */
    private static GlobalPos town(GameTestHelper helper, Territory territory, String... trades) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        GlobalPos bell = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(BELL));
        Map<UUID, Settlements.Resident> roster = new java.util.LinkedHashMap<>();
        for (String trade : trades) {
            UUID id = UUID.randomUUID();
            roster.put(id, new Settlements.Resident(id, 0, Optional.empty(), trade, 2, true, false, 0, false));
        }
        Settlements.get(helper.getLevel().getServer()).setRoster(bell, roster);
        territory.afterCensus(helper.getLevel(), helper.absolutePos(BELL), new Census(5, 5, 5, 5, 0), 1, List.of());
        return bell;
    }

    private static GlobalPos desk(GameTestHelper helper) {
        return GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(DESK));
    }

    private static MerchantStock stock(GameTestHelper helper) {
        return MerchantStock.at(helper.getLevel(), helper.absolutePos(DESK));
    }

    private static int count(GameTestHelper helper, net.minecraft.world.item.Item item) {
        return stock(helper).count(new ItemCost(item, 1));
    }

    private static void forget(GameTestHelper helper, GlobalPos bell) {
        Settlements.get(helper.getLevel().getServer()).setRoster(bell, Map.of());
    }

    @GameTest(template = "floor")
    public static void aMineHeadHasALadderDown(GameTestHelper helper) {
        BlockPos shaft = Mines.build(helper.getLevel(), helper.absolutePos(new BlockPos(1, 1, 1)));
        helper.assertTrue(helper.getLevel().getBlockState(shaft).is(Blocks.LADDER), "a ladder at the top of the shaft");
        helper.assertTrue(helper.getLevel().getBlockState(shaft.below()).is(Blocks.LADDER), "and down it");
        helper.assertTrue(helper.getLevel().getBlockState(shaft.east()).is(Blocks.COBBLESTONE), "in a cobblestone collar");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aToolsmithsMineBringsUpOre(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory, Territory.TOOLSMITH);
        territory.setMineForTest(bell, helper.absolutePos(new BlockPos(1, 1, 1)));
        long t = helper.getLevel().getGameTime();
        territory.touch(helper.getLevel(), desk(helper).pos(), stock(helper), t);
        territory.touch(helper.getLevel(), desk(helper).pos(), stock(helper), t + 24000);
        helper.assertTrue(count(helper, Items.RAW_IRON) == 4, "a level-2 Toolsmith brings up 4 raw iron a day: "
                + count(helper, Items.RAW_IRON));
        helper.assertTrue(count(helper, Items.COAL) == 1, "and a coal");
        forget(helper, bell);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void anArmorerSmeltsAndAToolsmithForgesHoes(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory, Territory.TOOLSMITH, "minecraft:armorer", "minecraft:farmer",
                "minecraft:farmer");
        stock(helper).insert(new ItemStack(Items.RAW_IRON, 16));
        stock(helper).insert(new ItemStack(Items.COAL, 2));
        territory.touch(helper.getLevel(), desk(helper).pos(), stock(helper), helper.getLevel().getGameTime());
        helper.assertTrue(count(helper, Items.IRON_HOE) == 2, "a hoe for each farmer: " + count(helper, Items.IRON_HOE));
        helper.assertTrue(count(helper, Items.IRON_INGOT) == 12, "of the 16 iron smelted: " + count(helper, Items.IRON_INGOT));
        helper.assertTrue(count(helper, Items.IRON_PICKAXE) == 0, "no mine, no miners: no pickaxes");
        helper.assertTrue(territory.kitShare(bell).get(Smithing.HOE.id()) == 1.0, "every farmer has tools");
        forget(helper, bell);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void toolsWearOutSlowerWhenMended(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        stock(helper).insert(new ItemStack(Items.IRON_HOE, 4));
        int used = Smithing.wear(stock(helper), new HashMap<>(), 16, 2, 0, 0, false);
        helper.assertTrue(used == 2, "two farmers wear out a hoe each in 16 days: " + used);
        used = Smithing.wear(stock(helper), new HashMap<>(), 16, 2, 0, 0, true);
        helper.assertTrue(used == 1, "half that while mended: " + used);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aVisitingArmorerSmeltsForATownWithoutOne(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory);
        stock(helper).insert(new ItemStack(Items.RAW_IRON, 16));
        stock(helper).insert(new ItemStack(Items.COAL, 2));
        GlobalPos home = GlobalPos.of(bell.dimension(), bell.pos().offset(500, 0, 0));
        long t = helper.getLevel().getGameTime();
        var smelting = Services.get("enterprise:smelting_abroad").orElseThrow();
        Visit visit = Visit.abroad(helper.getLevel(), territory, home, desk(helper), 1, stock(helper), 5, null, null,
                Map.of(), t);
        helper.assertTrue(smelting.wanted().test(visit), "ore and no Armorer: wanted");
        double fee = smelting.perform().applyAsDouble(visit);
        helper.assertTrue(count(helper, Items.IRON_INGOT) == 16, "all 16 smelted: " + count(helper, Items.IRON_INGOT));
        helper.assertTrue(Math.abs(fee - 0.32) < 1e-9, "for 0.02 an ingot: " + fee);
        forget(helper, bell);
        helper.succeed();
    }
}
