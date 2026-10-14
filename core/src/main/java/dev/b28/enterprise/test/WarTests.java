package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.cargo.Papers;
import dev.b28.enterprise.contract.Bills;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.market.Sovereign;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/** Bills of exchange and the sovereign's war shops. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class WarTests {
    private static GlobalPos far(GameTestHelper helper, int x, int z) {
        return GlobalPos.of(helper.getLevel().dimension(), new BlockPos(x, 64, z));
    }

    @GameTest(template = "floor")
    public static void aBillIsPaidWhenPresentedOnceDue(GameTestHelper helper) {
        var server = helper.getLevel().getServer();
        Bills bills = new Bills();
        GlobalPos drawee = far(helper, 8_000_000, 8_000_000);
        GlobalPos payee = far(helper, 8_000_400, 8_000_000);
        long t = 1_000_000;
        Papers.Document bill = bills.draw(drawee, payee, 10, t);
        var bundle = List.of(dev.b28.enterprise.cargo.Crates.documents(Papers.NONE.with(bill)));
        var early = bills.present(bundle, 100, t + 24000, new ContractBook());
        helper.assertTrue(early.paid() == 0 && early.returned().size() == 1, "not due yet: it goes back");
        var due = bills.present(early.returned(), 100, t + Bills.TERM_TICKS, new ContractBook());
        helper.assertTrue(Math.abs(due.paid() - 10 * (1 + Bills.INTEREST)) < 1e-9 && due.returned().isEmpty(),
                "due: paid with its interest, " + due.paid());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aBillLongUnpaidIsDishonoured(GameTestHelper helper) {
        Bills bills = new Bills();
        ContractBook book = new ContractBook();
        GlobalPos drawee = far(helper, 8_100_000, 8_000_000);
        GlobalPos payee = far(helper, 8_100_400, 8_000_000);
        long t = 1_000_000;
        var bundle = List.of(dev.b28.enterprise.cargo.Crates.documents(Papers.NONE.with(bills.draw(drawee, payee, 10, t))));
        bills.present(bundle, 0, t + Bills.TERM_TICKS + Bills.DISHONOUR_TICKS + 1, book);
        helper.assertTrue(bills.all().get(0).state() == Bills.State.DISHONOURED, "dishonoured");
        helper.assertTrue(book.trust(drawee, payee) < ContractBook.START_TRUST, "and trust falls");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void theSovereignBuysThroughShopsItFundsByMinting(GameTestHelper helper) {
        var server = helper.getLevel().getServer();
        BlockPos desk = new BlockPos(3, 2, 3);
        helper.setBlock(desk, Merchant.DESK.get());
        Sovereign sovereign = new Sovereign();
        long t = helper.getLevel().getGameTime();
        double before = Mint.get(server).debasement(Mint.DEFAULT_FACTION, t);
        int shops = sovereign.declare(helper.getLevel(), GlobalPos.of(helper.getLevel().dimension(),
                helper.absolutePos(desk)), 1000);
        helper.assertTrue(shops == Sovereign.WAR_ORDERS.size(), "a shop for each war order: " + shops);
        var first = (VendorBlockEntity) helper.getLevel().getBlockEntity(sovereign.shops().get(0));
        helper.assertTrue(first.getMode() == VendorBlockEntity.Mode.BUY && first.getFilterItem().is(Items.GOLD_INGOT)
                && first.getTotalPrice() > 0, "buying gold at a war price: " + first.getTotalPrice());
        helper.assertTrue(dev.b28.enterprise.market.Shops.purse(first).map(a -> a.getBalance() >= 1000).orElse(false),
                "paid from the treasury, a day's minting in it");
        helper.assertTrue(Mint.get(server).debasement(Mint.DEFAULT_FACTION, t) > before, "the minting debases the spur");
        sovereign.peace(helper.getLevel());
        helper.assertTrue(helper.getLevel().getBlockState(sovereign.shops().isEmpty() ? helper.absolutePos(desk).offset(-9, 0, -10)
                : sovereign.shops().get(0)).isAir(), "peace: the shops come down");
        helper.succeed();
    }
}
