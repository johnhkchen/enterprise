package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.territory.Husbandry;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class HusbandryTests {
    private static MerchantStock warehouse(GameTestHelper helper) {
        helper.setBlock(new BlockPos(3, 2, 3), Merchant.DESK.get());
        helper.setBlock(new BlockPos(4, 2, 3), Blocks.CHEST);
        return MerchantStock.at(helper.getLevel(), helper.absolutePos(new BlockPos(3, 2, 3)));
    }

    @GameTest(template = "floor")
    public static void shepherdsAndButchersBringAnimals(GameTestHelper helper) {
        Map<String, Double> herd = new HashMap<>();
        Husbandry.seed(herd, Set.of("minecraft:shepherd"));
        helper.assertTrue(herd.get("minecraft:sheep") == 3, "a Shepherd brings 3 sheep");
        Husbandry.seed(herd, Set.of("minecraft:butcher"));
        helper.assertTrue(herd.get("minecraft:pig") == 2 && herd.get("minecraft:chicken") == 2, "a Butcher, pigs and chickens");
        helper.assertTrue(Math.abs(Husbandry.feedDemand(Map.of("minecraft:sheep", 4.0)).get("minecraft:wheat") - 1) < 1e-9,
                "4 sheep eat a wheat a day: the village asks for it");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void leatherworkersKeepCowsForHides(GameTestHelper helper) {
        Map<String, Double> herd = new HashMap<>();
        Husbandry.seed(herd, Set.of("minecraft:leatherworker"));
        helper.assertTrue(herd.get("minecraft:cow") == 2, "a Leatherworker brings 2 cows");
        MerchantStock stock = warehouse(helper);
        stock.insert(new ItemStack(Items.WHEAT, 64));
        herd.put("minecraft:cow", 8.0);
        Map<String, Double> produce = new HashMap<>();
        Husbandry.tend(stock, herd, Set.of("minecraft:leatherworker"), 2, produce);
        helper.assertTrue(produce.getOrDefault("minecraft:leather", 0.0) > 2 && produce.getOrDefault("minecraft:beef", 0.0) > 4,
                "culled for leather, and beef: " + produce);
        var boots = dev.b28.enterprise.territory.Workshop.CONVERSIONS.stream()
                .filter(c -> c.output() == Items.LEATHER_BOOTS).findFirst().orElseThrow();
        stock.insert(new ItemStack(Items.LEATHER, 9));
        helper.assertTrue(dev.b28.enterprise.territory.Workshop.runForTest(boots, 2, stock) == 2,
                "9 leather make 2 pairs of boots (clothing)");
        helper.succeed();
    }

    @GameTest(template = "floor", skyAccess = true)
    public static void fishermenBuildADockWhereLandMeetsWater(GameTestHelper helper) {
        for (int x = 3; x <= 6; x++) {
            helper.setBlock(new BlockPos(x, 1, 3), Blocks.WATER);
        }
        var pier = dev.b28.enterprise.territory.Docks.findShore(helper.getLevel(), helper.absolutePos(new BlockPos(0, 2, 3)));
        helper.assertTrue(pier.isPresent() && pier.get().start().equals(helper.absolutePos(new BlockPos(3, 1, 3))),
                "the shore by the bell: " + pier);
        dev.b28.enterprise.territory.Docks.build(helper.getLevel(), pier.get());
        helper.assertTrue(helper.getLevel().getBlockState(helper.absolutePos(new BlockPos(6, 1, 3))).is(Blocks.SPRUCE_PLANKS),
                "a pier four planks out");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aFedFlockGrowsAndIsShorn(GameTestHelper helper) {
        MerchantStock stock = warehouse(helper);
        stock.insert(new ItemStack(Items.WHEAT, 64));
        Map<String, Double> herd = new HashMap<>(Map.of("minecraft:sheep", 3.0));
        Map<String, Double> produce = new HashMap<>();
        Husbandry.tend(stock, herd, Set.of("minecraft:shepherd"), 4, produce);
        helper.assertTrue(herd.get("minecraft:sheep") > 4, "fed, the flock grows: " + herd);
        helper.assertTrue(produce.getOrDefault("minecraft:white_wool", 0.0) > 5, "and gives wool: " + produce);
        helper.assertTrue(stock.count(new ItemCost(Items.WHEAT, 1)) == 61, "3 wheat eaten");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aHungryFlockDwindles(GameTestHelper helper) {
        MerchantStock stock = warehouse(helper);
        Map<String, Double> herd = new HashMap<>(Map.of("minecraft:sheep", 6.0));
        Husbandry.tend(stock, herd, Set.of(), 4, new HashMap<>());
        helper.assertTrue(herd.get("minecraft:sheep") < 6, "no wheat: fewer sheep, " + herd);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void butchersSlaughterBeyondTheBreedingStock(GameTestHelper helper) {
        MerchantStock stock = warehouse(helper);
        stock.insert(new ItemStack(Items.CARROT, 64));
        Map<String, Double> herd = new HashMap<>(Map.of("minecraft:pig", 10.0));
        Map<String, Double> produce = new HashMap<>();
        Husbandry.tend(stock, herd, Set.of("minecraft:butcher"), 2, produce);
        helper.assertTrue(herd.get("minecraft:pig") >= Husbandry.BREEDING_STOCK && herd.get("minecraft:pig") < 10,
                "pigs slaughtered down toward the breeding stock: " + herd);
        helper.assertTrue(produce.getOrDefault("minecraft:porkchop", 0.0) > 4, "porkchops: " + produce);
        helper.succeed();
    }
}
