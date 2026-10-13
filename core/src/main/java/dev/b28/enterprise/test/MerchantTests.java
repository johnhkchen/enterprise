package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.Merchant;
import net.minecraft.core.BlockPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.ai.village.poi.PoiTypes;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class MerchantTests {
    // Test coordinates start at the structure block, one below the arena; the floor is y=1.
    private static final BlockPos DESK = new BlockPos(3, 2, 3);

    @GameTest(template = "floor")
    public static void deskIsMerchantJobSite(GameTestHelper helper) {
        var poi = PoiTypes.forState(Merchant.DESK.get().defaultBlockState());
        helper.assertTrue(poi.isPresent(), "Merchant's Desk is not a point of interest");
        helper.assertTrue(
                Merchant.PROFESSION.get().acquirableJobSite().test(poi.get()),
                "Merchant profession does not accept the Merchant's Desk");
        helper.succeed();
    }

    @GameTest(template = "floor", timeoutTicks = 1200)
    public static void villagerTakesDeskJob(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        Villager villager = helper.spawn(EntityType.VILLAGER, new BlockPos(1, 2, 1));
        villager.setVillagerData(villager.getVillagerData().setProfession(VillagerProfession.NONE));
        helper.succeedWhen(() -> helper.assertTrue(
                villager.getVillagerData().getProfession() == Merchant.PROFESSION.get(),
                "villager has not become a Merchant"));
    }
}
