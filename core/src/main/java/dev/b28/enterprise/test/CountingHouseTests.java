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
