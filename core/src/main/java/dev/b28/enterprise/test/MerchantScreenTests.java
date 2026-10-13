package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantBook;
import dev.b28.enterprise.merchant.MerchantView;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class MerchantScreenTests {
    private static final BlockPos DESK = new BlockPos(3, 2, 3);
    private static final BlockPos LECTERN = new BlockPos(1, 2, 1);
    // Tests run side by side; stay inside this test's arena.
    private static final int RANGE = 4;

    @GameTest(template = "floor")
    public static void tradeSwitchesSurviveSaveAndLoad(GameTestHelper helper) {
        GlobalPos desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(DESK));
        BlockPos workstation = helper.absolutePos(LECTERN);
        MerchantBook book = new MerchantBook();
        book.setEnabled(desk, workstation, "minecraft:paper+>minecraft:emerald", true);
        book.setEnabled(desk, workstation, "minecraft:book+>minecraft:emerald", true);
        book.setEnabled(desk, workstation, "minecraft:book+>minecraft:emerald", false);

        CompoundTag saved = book.save(new CompoundTag(), helper.getLevel().registryAccess());
        MerchantBook loaded = MerchantBook.load(saved, helper.getLevel().registryAccess());
        helper.assertTrue(loaded.isEnabled(desk, workstation, "minecraft:paper+>minecraft:emerald"), "paper trade stays on");
        helper.assertFalse(loaded.isEnabled(desk, workstation, "minecraft:book+>minecraft:emerald"), "book trade stays off");
        helper.succeed();
    }

    @GameTest(template = "floor", timeoutTicks = 400)
    public static void viewListsEachLockersTrades(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(LECTERN, Blocks.LECTERN);
        helper.setBlock(new BlockPos(0, 2, 1), Blocks.CHEST);
        Villager librarian = helper.spawn(EntityType.VILLAGER, new BlockPos(1, 2, 2));
        librarian.setVillagerData(librarian.getVillagerData().setProfession(VillagerProfession.LIBRARIAN));

        // Wait for the librarian to claim the lectern, then build the screen's view.
        helper.succeedWhen(() -> {
            MerchantView view = MerchantView.build(helper.getLevel(), helper.absolutePos(DESK), 2, RANGE);
            List<MerchantView.LockerView> lockers = view.lockers();
            helper.assertTrue(lockers.size() == 1, "expected 1 locker, found " + lockers.size());
            helper.assertFalse(lockers.get(0).trades().isEmpty(), "the librarian's trades should be listed");
            helper.assertTrue(
                    lockers.get(0).trades().stream().noneMatch(MerchantView.TradeView::enabled),
                    "every trade starts switched off");
        });
    }
}
