package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.cargo.Cargo;
import dev.b28.enterprise.cargo.CrateBlockEntity;
import dev.b28.enterprise.cargo.Crates;
import dev.b28.enterprise.cargo.Papers;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.phys.AABB;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/** Crates and their papers. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class CargoTests {
    @GameTest(template = "floor")
    public static void aCrateHoldsNineStacksAndItsPapers(GameTestHelper helper) {
        helper.assertTrue(Crates.fits(0, new ItemStack(Items.PAPER, 64)), "goods in the nine");
        helper.assertFalse(Crates.fits(0, new ItemStack(Cargo.CRATE_ITEM.get())), "but not a crate in a crate");
        helper.assertFalse(Crates.fits(Crates.PAPERS_SLOT, new ItemStack(Items.PAPER)), "only documents in the tenth");
        helper.assertTrue(Crates.fits(Crates.PAPERS_SLOT, new ItemStack(Cargo.DOCUMENTS.get())), "documents go there");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aBrokenCrateKeepsItsGoodsAndPapers(GameTestHelper helper) {
        BlockPos at = new BlockPos(3, 2, 3);
        helper.setBlock(at, Cargo.CRATE.get());
        CrateBlockEntity crate = (CrateBlockEntity) helper.getLevel().getBlockEntity(helper.absolutePos(at));
        crate.setItem(0, new ItemStack(Items.PAPER, 64));
        crate.setItem(1, new ItemStack(Items.BOOK, 12));
        GlobalPos here = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(at));
        crate.setItem(Crates.PAPERS_SLOT, Crates.documents(Papers.NONE.with(
                Papers.billOfLading(7, here, here, "64 paper, 12 book", "EXW", 0, 0))));
        helper.getLevel().destroyBlock(helper.absolutePos(at), true);
        var dropped = helper.getLevel().getEntitiesOfClass(ItemEntity.class, new AABB(helper.absolutePos(at)).inflate(2))
                .stream().map(ItemEntity::getItem).filter(Crates::isCrate).findFirst();
        helper.assertTrue(dropped.isPresent(), "it drops as a crate");
        List<ItemStack> goods = Crates.goods(dropped.get());
        helper.assertTrue(goods.size() == 2 && goods.get(0).getCount() == 64, "with its goods: " + goods);
        helper.assertTrue(Crates.papers(dropped.get()).contract().filter(id -> id == 7).isPresent(),
                "and its bill of lading");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aCrateTakesTheSpaceOfWhatsInIt(GameTestHelper helper) {
        ItemStack crate = Crates.of(List.of(new ItemStack(Items.PAPER, 64), new ItemStack(Items.PAPER, 64),
                new ItemStack(Items.FLINT, 3)), Papers.NONE);
        helper.assertTrue(Crates.stacks(List.of(crate, new ItemStack(Items.BREAD))) == 4,
                "three stacks crated, one loose: four stacks of cargo");
        helper.assertTrue(Crates.stacks(List.of(Crates.of(List.of(), Papers.NONE))) == 1, "an empty crate takes one");
        helper.assertTrue(new ItemStack(Cargo.CRATE_ITEM.get()).getMaxStackSize() == Crates.EMPTIES_KEPT
                && crate.getMaxStackSize() == 1, "empties stack; a packed crate doesn't");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void documentsUseStationeryOrAScribe(GameTestHelper helper) {
        helper.setBlock(new BlockPos(3, 2, 3), dev.b28.enterprise.merchant.Merchant.DESK.get());
        helper.setBlock(new BlockPos(4, 2, 3), net.minecraft.world.level.block.Blocks.CHEST);
        var level = helper.getLevel();
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(new BlockPos(3, 2, 3)));
        var stock = dev.b28.enterprise.merchant.MerchantStock.at(level, desk.pos());
        stock.insert(new ItemStack(Items.PAPER, 8));
        stock.insert(new ItemStack(Items.INK_SAC, 1));
        stock.insert(new ItemStack(Items.EMERALD, 10));
        var paperwork = dev.b28.enterprise.cargo.Paperwork.get(level.getServer());
        dev.b28.enterprise.cargo.Paperwork.drawn(desk, 8, dev.b28.enterprise.cargo.Paperwork.LADING);
        paperwork.settle(level, desk, stock, level.getGameTime());
        var b = paperwork.book(desk);
        helper.assertTrue(b[1] == 8 && b[2] == 1 && b[4] == 0, "eight papers, an ink sac, no scribe: "
                + java.util.Arrays.toString(b));
        dev.b28.enterprise.cargo.Paperwork.drawn(desk, 8, dev.b28.enterprise.cargo.Paperwork.LADING);
        paperwork.settle(level, desk, stock, level.getGameTime());
        b = paperwork.book(desk);
        helper.assertTrue(b[4] > 0, "out of paper: a scribe paid " + java.util.Arrays.toString(b));
        helper.succeed();
    }
}
