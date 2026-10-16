package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.cargo.Paperwork;
import dev.b28.enterprise.contract.Bills;
import dev.b28.enterprise.market.Discounts;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Buildings;
import dev.b28.enterprise.territory.Territory;
import dev.b28.enterprise.territory.Works;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/** A hub's counting house and its discount desk. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class CountingHouseTests {
    @GameTest(template = "floor")
    public static void aCountingHouseNeedsBothItsTrades(GameTestHelper helper) {
        var house = Buildings.COUNTING_HOUSE;
        helper.assertTrue(house.partnered(Map.of("minecraft:librarian", 3), 3), "a Librarian and a Merchant, both at 3");
        helper.assertFalse(house.partnered(Map.of("minecraft:librarian", 3), 1), "a Novice Merchant isn't enough");
        helper.assertFalse(house.partnered(Map.of(), 5), "nor a master Merchant with no Librarian");
        helper.assertTrue(Buildings.SMITHY.partnered(Map.of("minecraft:armorer", 3), 1), "a smithy: any one of its smiths");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aCountingHousesClerksDrawUpPapersCheaply(GameTestHelper helper) {
        var level = helper.getLevel();
        Territory territory = Territory.get(level.getServer());
        GlobalPos bell = LabourTests.town(helper, territory);
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(new BlockPos(5, 2, 5)));
        var stock = LabourTests.stock(helper);
        stock.insert(new ItemStack(Items.EMERALD, 10));
        var paperwork = Paperwork.get(level.getServer());
        Paperwork.drawn(desk, 10, Paperwork.LADING);
        paperwork.settle(level, desk, stock, level.getGameTime());
        double scribes = paperwork.book(desk)[4];
        helper.assertTrue(Math.abs(scribes - 1.0) < 1e-9, "no stationery: a scribe at 0.1 a document, " + scribes);
        Works works = Works.get(level.getServer());
        works.finish(works.startBuilding(bell, Buildings.COUNTING_HOUSE, helper.absolutePos(new BlockPos(0, 1, 0))));
        Paperwork.drawn(desk, 10, Paperwork.LADING);
        paperwork.settle(level, desk, stock, level.getGameTime());
        double clerks = paperwork.book(desk)[4] - scribes;
        helper.assertTrue(Math.abs(clerks - 0.2) < 1e-9, "with a counting house: its clerks at 0.02, " + clerks);
        Settlements.get(level.getServer()).setRoster(bell, Map.of());
        works.forgetForTest(bell);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void theCashBookTalliesMoneyByWhatItWasFor(GameTestHelper helper) {
        var level = helper.getLevel();
        BlockPos at = helper.absolutePos(new BlockPos(1, 2, 1));
        helper.setBlock(new BlockPos(1, 2, 1), dev.b28.enterprise.merchant.Merchant.DESK.get());
        helper.setBlock(new BlockPos(2, 2, 1), net.minecraft.world.level.block.Blocks.CHEST);
        var stock = dev.b28.enterprise.merchant.MerchantStock.at(level, at);
        stock.insert(new ItemStack(Items.EMERALD, 10));
        GlobalPos desk = GlobalPos.of(level.dimension(), at);
        var mint = dev.b28.enterprise.money.Mint.get(level.getServer());
        dev.b28.enterprise.money.Treasury.withdraw(level, at, stock, 4, mint, level.getGameTime());
        dev.b28.enterprise.money.CashBook.note(desk, "lockers: sold wheat", 2);
        long today = level.getGameTime() / 24000;
        var totals = dev.b28.enterprise.money.CashBook.get(level.getServer()).totals(desk, today, today);
        helper.assertTrue(Math.abs(totals.getOrDefault("CountingHouseTests.theCashBookTalliesMoneyByWhatItWasFor", 0.0)
                + 4) < 1e-9, "4 out, booked to what took it: " + totals);
        helper.assertTrue(Math.abs(totals.getOrDefault("lockers: sold wheat", 0.0) - 2) < 1e-9, "2 in from wheat");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void theAccountantKeepsTwoSpendersOffTheSameMoney(GameTestHelper helper) {
        var level = helper.getLevel();
        BlockPos at = helper.absolutePos(new BlockPos(1, 2, 1));
        helper.setBlock(new BlockPos(1, 2, 1), dev.b28.enterprise.merchant.Merchant.DESK.get());
        helper.setBlock(new BlockPos(2, 2, 1), net.minecraft.world.level.block.Blocks.CHEST);
        var stock = dev.b28.enterprise.merchant.MerchantStock.at(level, at);
        stock.insert(new ItemStack(Items.EMERALD, 40));
        GlobalPos desk = GlobalPos.of(level.dimension(), at);
        long t = level.getGameTime();
        dev.b28.enterprise.money.Accountant.forgetForTest(desk);
        var dials = dev.b28.enterprise.Dials.get(level.getServer());
        dials.set(dev.b28.enterprise.Dials.BUDGET_SHARE, 0.5); // half of the 40 free, 20, today
        helper.assertTrue(dev.b28.enterprise.money.Accountant.approve(level, desk, stock, "a field", 12, t),
                "40 free: so much of it today; a field for 12, yes");
        helper.assertFalse(dev.b28.enterprise.money.Accountant.approve(level, desk, stock, "a shop", 12, t),
                "and a shop for 12 more, not today: 8 left");
        double shopping = dev.b28.enterprise.money.Accountant.allow(level, desk, stock, "caravan shopping", 20, t);
        helper.assertTrue(Math.abs(shopping - 8) < 1e-9, "the caravan gets what's left, 8: " + shopping);
        dials.set(dev.b28.enterprise.Dials.BUDGET_SHARE, null);
        dev.b28.enterprise.money.Accountant.forgetForTest(desk);
        GlobalPos lender = GlobalPos.of(level.dimension(), new BlockPos(7_920_000, 64, 7_000_000));
        var bill = Bills.get(level.getServer()).draw(desk, lender, 30, t, -1, 2 * 24000L); // due in 2 days: kept for
        double free = dev.b28.enterprise.money.Accountant.free(level, desk, stock, t);
        helper.assertTrue(free < 10 + 1e-9, "a bill of 30 falling due: only 10 of the 40 free, " + free);
        Bills.get(level.getServer()).bill(bill.data().getInt("bill")).ifPresent(b -> Bills.get(level.getServer()).redeem(b));
        dev.b28.enterprise.money.Accountant.forgetForTest(desk);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void theDiscountDeskPricesByGoodsAndCapsItsBook(GameTestHelper helper) {
        var server = helper.getLevel().getServer();
        var dim = helper.getLevel().dimension();
        GlobalPos hub = GlobalPos.of(dim, new BlockPos(7_910_000, 64, 7_000_000));
        GlobalPos farm = GlobalPos.of(dim, new BlockPos(7_910_400, 64, 7_000_000));
        GlobalPos other = GlobalPos.of(dim, new BlockPos(7_910_800, 64, 7_000_000));
        Bills ledger = Bills.get(server);
        long t = helper.getLevel().getGameTime();
        var held = ledger.bill(ledger.draw(farm, hub, 30, t).data().getInt("bill")).orElseThrow(); // already in its book
        var more = ledger.bill(ledger.draw(farm, other, 10, t).data().getInt("bill")).orElseThrow();
        var elsewhere = ledger.bill(ledger.draw(other, farm, 10, t).data().getInt("bill")).orElseThrow();
        double means = 100;
        var onFarm = Discounts.quote(server, hub, more, false, 0, 0, means, t);
        helper.assertFalse(onFarm.taken(), "past a third of its book on one market: " + onFarm);
        var fresh = Discounts.quote(server, hub, elsewhere, false, 0, 0, means, t);
        var backed = Discounts.quote(server, hub, elsewhere, true, 0, 0, means, t);
        helper.assertTrue(fresh.taken() && backed.share() - fresh.share() > 0.039,
                "goods it can see: a narrower discount, " + fresh.share() + " vs " + backed.share());
        var full = Discounts.quote(server, hub, elsewhere, true, 0, 1, means, t);
        helper.assertTrue(backed.share() - full.share() > 0.029, "a full warehouse: dearer, " + full.share());
        var booked = Discounts.quote(server, hub, elsewhere, true, 65, 0, means, t);
        helper.assertFalse(booked.taken(), "its book at its limit: " + booked);
        ledger.redeem(held);
        ledger.redeem(more);
        ledger.redeem(elsewhere);
        helper.succeed();
    }
}
