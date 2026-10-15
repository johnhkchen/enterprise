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
    public static void fieldsGrowWhatTheVillageWouldBuy(GameTestHelper helper) {
        helper.setBlock(new BlockPos(3, 2, 3), dev.b28.enterprise.merchant.Merchant.DESK.get());
        helper.setBlock(new BlockPos(4, 2, 3), Blocks.CHEST);
        var stock = dev.b28.enterprise.merchant.MerchantStock.at(helper.getLevel(), helper.absolutePos(new BlockPos(3, 2, 3)));
        stock.insert(new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.BREAD, 128)); // fed
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(Map.of(), Map.of(), stock).isEmpty(),
                "nobody buys a crop, enough food: no field");
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(Map.of("minecraft:paper", 48.0), Map.of(), stock)
                .filter(net.minecraft.world.item.Items.SUGAR_CANE::equals).isPresent(), "Librarians buy paper: cane");
        var both = Map.of("minecraft:paper", 48.0, "minecraft:potato", 640.0);
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(both, Map.of(), stock)
                .filter(net.minecraft.world.item.Items.POTATO::equals).isPresent(), "a Farmer buys more in potatoes");
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(both, Map.of("minecraft:potato", 640.0), stock)
                .filter(net.minecraft.world.item.Items.SUGAR_CANE::equals).isPresent(), "potatoes grown already: cane next");
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(Map.of("minecraft:sweet_berries", 20.0), Map.of(), stock)
                .filter(net.minecraft.world.item.Items.SWEET_BERRIES::equals).isPresent(), "a Master Butcher: berries");
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(both, Map.of("minecraft:potato", 640.0), stock, Map.of(),
                true).filter(net.minecraft.world.item.Items.SUGAR_CANE::equals).isEmpty(), "a town still growing: no cane yet");
        helper.assertTrue(dev.b28.enterprise.territory.Focus.of(5, 8, 2000) == dev.b28.enterprise.territory.Focus.GROW,
                "five villagers: it grows first");
        helper.assertTrue(dev.b28.enterprise.territory.Focus.of(9, 12, 100) == dev.b28.enterprise.territory.Focus.GROW,
                "short of food: it grows");
        helper.assertTrue(dev.b28.enterprise.territory.Focus.of(9, 9, 2000) == dev.b28.enterprise.territory.Focus.BUILD,
                "fed and grown, every bed taken: it builds");
        helper.assertTrue(dev.b28.enterprise.territory.Focus.of(9, 12, 2000) == dev.b28.enterprise.territory.Focus.TRADE,
                "and with room to grow: it trades");
        helper.succeed();
    }

    @GameTest(template = "floor", skyAccess = true)
    public static void caneIsPressedIntoPaper(GameTestHelper helper) {
        helper.setBlock(new BlockPos(3, 2, 3), dev.b28.enterprise.merchant.Merchant.DESK.get());
        helper.setBlock(new BlockPos(4, 2, 3), Blocks.CHEST);
        var stock = dev.b28.enterprise.merchant.MerchantStock.at(helper.getLevel(), helper.absolutePos(new BlockPos(3, 2, 3)));
        stock.insert(new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.SUGAR_CANE, 40));
        var pressing = dev.b28.enterprise.territory.Workshop.CONVERSIONS.get(0);
        int made = dev.b28.enterprise.territory.Workshop.runForTest(pressing, 64, stock);
        helper.assertTrue(made == 40 && stock.count(new net.minecraft.world.item.trading.ItemCost(
                net.minecraft.world.item.Items.PAPER, 1)) == 40, "40 cane, 40 paper: " + made);
        stock.insert(new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.WHEAT, 10));
        int bread = dev.b28.enterprise.territory.Workshop.runForTest(dev.b28.enterprise.territory.Workshop.CONVERSIONS.get(1),
                10, stock);
        helper.assertTrue(bread == 3, "10 wheat bake 3 bread, a wheat over: " + bread);
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
