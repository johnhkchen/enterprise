package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.territory.Jobs;
import java.util.Map;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class JobsTests {
    @GameTest(template = "floor")
    public static void theTradeThatBuysMostOfTheSupply(GameTestHelper helper) {
        Map<String, Double> paper = Map.of("minecraft:paper", 3.0, "minecraft:carrot", 1.0);
        helper.assertTrue(Jobs.best(paper, Set.of()).orElseThrow().profession() == VillagerProfession.LIBRARIAN,
                "paper: a Librarian");
        helper.assertTrue(Jobs.best(paper, Set.of(VillagerProfession.LIBRARIAN)).orElseThrow().profession()
                == VillagerProfession.CARTOGRAPHER, "with a Librarian already, a Cartographer");
        helper.assertTrue(Jobs.best(Map.of("minecraft:carrot", 2.0), Set.of()).orElseThrow().profession()
                == VillagerProfession.FARMER, "carrots: a Farmer");
        helper.assertTrue(Jobs.best(Map.of(), Set.of()).isEmpty(), "nothing to sell: no stall");
        helper.succeed();
    }

    @GameTest(template = "floor", skyAccess = true)
    public static void fieldsGrowWhatTheMarketBuys(GameTestHelper helper) {
        var desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(new BlockPos(3, 2, 3)));
        var warehouse = dev.b28.enterprise.merchant.Warehouse.get(helper.getLevel().getServer());
        helper.setBlock(new BlockPos(3, 2, 3), dev.b28.enterprise.merchant.Merchant.DESK.get());
        helper.setBlock(new BlockPos(4, 2, 3), Blocks.CHEST);
        var stock = dev.b28.enterprise.merchant.MerchantStock.at(helper.getLevel(), helper.absolutePos(new BlockPos(3, 2, 3)));
        stock.insert(new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.BREAD, 128)); // fed
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(warehouse, desk, stock).isEmpty(),
                "no market, enough food: no field");
        warehouse.recordCapacity(desk, 1L, Map.of(net.minecraft.world.item.Items.PAPER, 48));
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(warehouse, desk, stock)
                .filter(net.minecraft.world.item.Items.SUGAR_CANE::equals).isPresent(), "lockers buy paper: sugar cane");
        warehouse.recordCapacity(desk, 2L, Map.of(net.minecraft.world.item.Items.CARROT, 220));
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(warehouse, desk, stock)
                .filter(net.minecraft.world.item.Items.CARROT::equals).isPresent(), "they'd take more in carrots: carrots");
        helper.succeed();
    }

    @GameTest(template = "floor", skyAccess = true)
    public static void caneIsPressedIntoPaper(GameTestHelper helper) {
        var desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(new BlockPos(3, 2, 3)));
        helper.setBlock(new BlockPos(3, 2, 3), dev.b28.enterprise.merchant.Merchant.DESK.get());
        helper.setBlock(new BlockPos(4, 2, 3), Blocks.CHEST);
        var warehouse = dev.b28.enterprise.merchant.Warehouse.get(helper.getLevel().getServer());
        warehouse.recordCapacity(desk, 1L, Map.of(net.minecraft.world.item.Items.PAPER, 24)); // par: 64 paper
        var stock = dev.b28.enterprise.merchant.MerchantStock.at(helper.getLevel(), helper.absolutePos(new BlockPos(3, 2, 3)));
        stock.insert(new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.SUGAR_CANE, 40));
        int made = dev.b28.enterprise.territory.Fields.press(warehouse, desk, stock, helper.getLevel().getGameTime());
        helper.assertTrue(made == 40 && stock.count(new net.minecraft.world.item.trading.ItemCost(
                net.minecraft.world.item.Items.PAPER, 1)) == 40, "40 cane, 40 paper: " + made);
        helper.succeed();
    }

    @GameTest(template = "floor", skyAccess = true)
    public static void aStallIsALockerWithAWorker(GameTestHelper helper) {
        Villager idle = helper.spawn(EntityType.VILLAGER, new BlockPos(5, 2, 1));
        var trade = Jobs.TRADES.stream().filter(t -> t.profession() == VillagerProfession.LIBRARIAN).findFirst().orElseThrow();
        BlockPos workstation = Jobs.buildStall(helper.getLevel(), helper.absolutePos(new BlockPos(0, 1, 0)), trade);
        helper.assertTrue(helper.getLevel().getBlockState(workstation).is(Blocks.LECTERN), "a lectern");
        helper.assertFalse(Lockers.touchingContainers(helper.getLevel(), workstation).isEmpty(), "with a chest: a locker");
        var desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(new BlockPos(5, 2, 5)));
        Jobs.hire(helper.getLevel(), idle, workstation, trade, desk);
        helper.assertTrue(idle.getVillagerData().getProfession() == VillagerProfession.LIBRARIAN, "the idle villager is a Librarian");
        helper.assertTrue(idle.getBrain().getMemory(MemoryModuleType.JOB_SITE).map(GlobalPos::pos).filter(workstation::equals)
                .isPresent(), "working the stall");
        idle.discard();
        helper.succeed();
    }
}
