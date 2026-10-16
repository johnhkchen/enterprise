package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.contract.Bills;
import dev.b28.enterprise.market.Post;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/** The crown's post: letters on the post days, calls a hub books, bills paid by post. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class PostTests {
    private static MerchantStock market(GameTestHelper helper, BlockPos desk, int emeralds) {
        helper.setBlock(desk, Merchant.DESK.get());
        helper.setBlock(desk.east(), Blocks.CHEST);
        MerchantStock stock = MerchantStock.at(helper.getLevel(), helper.absolutePos(desk));
        stock.insert(new ItemStack(Items.EMERALD, emeralds));
        return stock;
    }

    @GameTest(template = "floor")
    public static void aLetterLeavesOnThePostDayAndWaitsAtTheDesk(GameTestHelper helper) {
        var level = helper.getLevel();
        MerchantStock stock = market(helper, new BlockPos(1, 2, 1), 5);
        GlobalPos from = GlobalPos.of(level.dimension(), helper.absolutePos(new BlockPos(1, 2, 1)));
        GlobalPos to = GlobalPos.of(level.dimension(), from.pos().offset(4000, 0, 0));
        long t = level.getGameTime();
        Post post = Post.get(level.getServer());
        post.sendForTest(level, stock, Post.Kind.REPLY, from, to, 0, t);
        helper.assertTrue(post.onTheRoad(to, Post.Kind.REPLY) == 1, "on the road");
        helper.assertTrue(stock.count(new ItemCost(Items.EMERALD, 1)) == 4, "the postage paid");
        Post.atDesk(level, to, stock, t);
        helper.assertTrue(post.onTheRoad(to, Post.Kind.REPLY) == 1, "4000 blocks at 2000 a day: not there yet");
        Post.atDesk(level, to, stock, t + 9 * 24000);
        helper.assertTrue(post.onTheRoad(to, Post.Kind.REPLY) == 0, "read at the desk once it's there");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aHubBooksACallOneADayAndDeclinesOneThatDoesntPay(GameTestHelper helper) {
        var level = helper.getLevel();
        MerchantStock stock = market(helper, new BlockPos(1, 2, 1), 20);
        GlobalPos hub = GlobalPos.of(level.dimension(), helper.absolutePos(new BlockPos(1, 2, 1)));
        GlobalPos near = GlobalPos.of(level.dimension(), hub.pos().offset(400, 0, 0));
        GlobalPos other = GlobalPos.of(level.dimension(), hub.pos().offset(0, 0, 400));
        GlobalPos poor = GlobalPos.of(level.dimension(), hub.pos().offset(-400, 0, 0));
        long t = level.getGameTime();
        Post post = Post.get(level.getServer());
        post.sendForTest(level, stock, Post.Kind.CALL, near, hub, 200, t);
        post.sendForTest(level, stock, Post.Kind.CALL, other, hub, 200, t);
        post.sendForTest(level, stock, Post.Kind.CALL, poor, hub, 1, t);
        post.deliverNowForTest(t);
        Post.atDesk(level, hub, stock, t);
        var calls = post.calls(hub);
        helper.assertTrue(calls.size() == 2, "two calls booked, the poor one declined: " + calls);
        helper.assertTrue(calls.get(0).day() != calls.get(1).day(), "on different days: one wagon a day, " + calls);
        CaravanLedger ledger = CaravanLedger.get(level.getServer());
        var run = ledger.route(hub, near);
        helper.assertTrue(run.isPresent() && run.get().onCall(), "a run there, on call");
        helper.assertTrue(ledger.route(hub, poor).isEmpty(), "and none to the town that didn't pay");
        long day = calls.get(0).day();
        helper.assertTrue(!post.due(hub, calls.get(0).town(), (day - 1) * 24000) && post.due(hub, calls.get(0).town(), day * 24000),
                "the run's due on its booked day, not before");
        ledger.route(hub, near).ifPresent(r -> ledger.removeRoute(r.id()));
        ledger.route(hub, other).ifPresent(r -> ledger.removeRoute(r.id()));
        post.made(hub, near, (day + 5) * 24000);
        post.made(hub, other, (day + 5) * 24000);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aBillGoesByPostAndTheCoinComesBack(GameTestHelper helper) {
        var level = helper.getLevel();
        MerchantStock holderStock = market(helper, new BlockPos(1, 2, 1), 2);
        MerchantStock draweeStock = market(helper, new BlockPos(4, 2, 4), 20);
        GlobalPos holder = GlobalPos.of(level.dimension(), helper.absolutePos(new BlockPos(1, 2, 1)));
        GlobalPos drawee = GlobalPos.of(level.dimension(), helper.absolutePos(new BlockPos(4, 2, 4)));
        long t = level.getGameTime();
        Bills ledger = Bills.get(level.getServer());
        var doc = ledger.draw(drawee, holder, 5, t, -1, 1); // due now: no caravan of either goes to the other
        Bills.file(holderStock, doc);
        var bill = ledger.bill(doc.data().getInt("bill")).orElseThrow();
        Post post = Post.get(level.getServer());
        Post.atDesk(level, holder, holderStock, t + 1);
        helper.assertTrue(post.onTheRoad(drawee, Post.Kind.BILLS) == 1, "the bill sent by post");
        post.deliverNowForTest(t + 1);
        Post.atDesk(level, drawee, draweeStock, t + 1);
        helper.assertTrue(bill.state() == Bills.State.PAID, "paid when the drawee read it: " + bill.state());
        post.deliverNowForTest(t + 1);
        double before = dev.b28.enterprise.money.Treasury.available(level, holder.pos(), holderStock, 16);
        Post.atDesk(level, holder, holderStock, t + 2);
        double after = dev.b28.enterprise.money.Treasury.available(level, holder.pos(), holderStock,
                dev.b28.enterprise.money.Mint.get(level.getServer()).rate(dev.b28.enterprise.money.Mint.DEFAULT_FACTION, t));
        helper.assertTrue(after > before + 4, "the coin back by post: " + before + " -> " + after);
        helper.succeed();
    }
}
