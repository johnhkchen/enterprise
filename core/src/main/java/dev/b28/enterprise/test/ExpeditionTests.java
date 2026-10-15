package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.caravan.Expeditions;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.BedBlock;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.properties.BedPart;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class ExpeditionTests {
    @GameTest(template = "floor", skyAccess = true)
    public static void aMerchantSettlesAVillageNobodyTradesWith(GameTestHelper helper) {
        // A village of its own: a bell, three beds along the east side, three workstations along the west.
        helper.setBlock(new BlockPos(1, 2, 1), Blocks.BELL);
        var bed = Blocks.WHITE_BED.defaultBlockState().setValue(BedBlock.FACING, Direction.SOUTH);
        for (int z : new int[] {0, 2, 4}) {
            helper.setBlock(new BlockPos(6, 2, z), bed.setValue(BedBlock.PART, BedPart.FOOT));
            helper.setBlock(new BlockPos(6, 2, z + 1), bed.setValue(BedBlock.PART, BedPart.HEAD));
        }
        helper.setBlock(new BlockPos(0, 2, 4), Blocks.LECTERN);
        helper.setBlock(new BlockPos(0, 2, 5), Blocks.COMPOSTER);
        helper.setBlock(new BlockPos(0, 2, 6), Blocks.FLETCHING_TABLE);
        Villager merchant = helper.spawn(EntityType.VILLAGER, new BlockPos(2, 2, 6));
        merchant.setVillagerData(merchant.getVillagerData().setProfession(Merchant.PROFESSION.get()));
        merchant.setVillagerXp(1);
        var id = merchant.getUUID();
        var level = helper.getLevel();
        var from = GlobalPos.of(level.dimension(), helper.absolutePos(new BlockPos(0, 2, 0)));
        String what = Expeditions.get(level.getServer()).settle(level, from, helper.absolutePos(new BlockPos(1, 2, 1)),
                Expeditions.Kind.VILLAGE, List.of(merchant), 10, 4, level.getGameTime());
        BlockPos desk = null;
        for (BlockPos at : BlockPos.betweenClosed(helper.absolutePos(new BlockPos(0, 2, 0)), helper.absolutePos(new BlockPos(6, 2, 6)))) {
            if (level.getBlockState(at).is(Merchant.DESK.get())) {
                desk = at.immutable();
            }
        }
        helper.assertTrue(desk != null, "a desk set up by the bell: " + what);
        var site = desk;
        helper.assertTrue(level.getEntity(id) instanceof Villager back && back.getBrain().getMemory(MemoryModuleType.JOB_SITE)
                .filter(at -> at.pos().equals(site)).isPresent(), "the Merchant works it");
        helper.assertTrue(MerchantStock.at(level, desk).count(new ItemCost(Items.EMERALD, 1)) == 10, "with its capital");
        level.getEntity(id).discard();
        helper.succeed();
    }
}
