package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.contract.Contract;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.contract.Delivery;
import dev.b28.enterprise.contract.Incoterm;
import dev.b28.enterprise.market.Broker;
import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.territory.Territory;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.item.Items;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/** Contracts' shipping terms, trust, advances, and sourcing from a supplier town. Parties far from any arena. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class ContractTests {
    private static GlobalPos far(GameTestHelper helper, int x, int z) {
        return GlobalPos.of(helper.getLevel().dimension(), new BlockPos(x, 64, z));
    }

    @GameTest(template = "floor")
    public static void eachIncotermPricesItsBurdens(GameTestHelper helper) {
        helper.assertTrue(Incoterm.EXW.price(1, 0.1, 0.2) == 1, "Ex Works: the price at the seller's");
        helper.assertTrue(Math.abs(Incoterm.CPT.price(1, 0.1, 0.2) - 1.1) < 1e-9, "Carriage Paid To: and the carriage");
        helper.assertTrue(Math.abs(Incoterm.DAP.price(1, 0.1, 0.2) - 1.1) < 1e-9, "Delivered At Place: the same");
        helper.assertTrue(Math.abs(Incoterm.DDP.price(1, 0.1, 0.2) - 1.3) < 1e-9, "Delivered Duty Paid: and the tolls");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void underCarriagePaidToTheRoadIsTheBuyers(GameTestHelper helper) {
        ContractBook book = new ContractBook();
        GlobalPos seller = far(helper, 7_000_000, 7_000_000);
        GlobalPos buyer = far(helper, 7_000_400, 7_000_000);
        long t = 1_000_000;
        Contract cpt = book.draft(seller, buyer, new Delivery(Items.APPLE, 10, 0.25), Incoterm.CPT, t, 24000, 1, t,
                seller, true);
        Contract dap = book.draft(seller, buyer, new Delivery(Items.APPLE, 10, 0.25), Incoterm.DAP, t, 24000, 1, t,
                seller, true);
        book.ship(cpt, 10, t);
        book.ship(dap, 10, t);
        book.review(t + Contract.GRACE_TICKS + 1); // still on the road when judged
        helper.assertTrue(cpt.missed() == 0, "CPT: on the road counts as delivered");
        helper.assertTrue(dap.missed() == 1, "DAP: not there yet, the seller's miss");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void exWorksGoodsLeftUncollectedAreTheBuyersMiss(GameTestHelper helper) {
        ContractBook book = new ContractBook();
        GlobalPos seller = far(helper, 7_100_000, 7_000_000);
        GlobalPos buyer = far(helper, 7_100_400, 7_000_000);
        Warehouse.get(helper.getLevel().getServer()).recordStocktake(seller, Map.of("minecraft:apple", 64), 0);
        long t = 1_000_000;
        Contract exw = book.draft(seller, buyer, new Delivery(Items.APPLE, 10, 0.25), Incoterm.EXW, t, 24000, 1, t,
                seller, true);
        book.review(helper.getLevel().getServer(), t + Contract.GRACE_TICKS + 1);
        helper.assertTrue(exw.buyerMissed() == 1 && exw.missed() == 0, "set aside, nobody came: the buyer's miss");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aTownShortOfGoodsSourcesThemFromASupplier(GameTestHelper helper) {
        var server = helper.getLevel().getServer();
        GlobalPos desk = far(helper, 7_200_000, 7_000_000);
        GlobalPos supplier = far(helper, 7_200_300, 7_000_000);
        Warehouse.get(server).recordStocktake(supplier, Map.of("minecraft:iron_pickaxe", 6), 0);
        dev.b28.enterprise.caravan.Atlas.get(server).learn(desk, supplier, 0);
        var ledger = dev.b28.enterprise.caravan.CaravanLedger.get(server);
        var route = ledger.addRoute(desk, supplier);
        long t = helper.getLevel().getGameTime();
        helper.assertFalse(Broker.source(helper.getLevel(), desk, new Territory(), Items.IRON_PICKAXE, 5, t),
                "three of the five: the need isn't covered yet");
        var contract = ContractBook.get(server).of(desk).stream().filter(c -> c.seller().equals(supplier)).findFirst()
                .orElseThrow();
        helper.assertTrue(contract.incoterm() == Incoterm.EXW && contract.delivery().get().quantity() == 3,
                "Ex Works, since its caravans go there; three, half the supplier's kit: " + contract.history());
        helper.assertFalse(contract.inForce(), "in force once a caravan carries the copy");
        ledger.removeRoute(route.id());
        helper.succeed();
    }
    @GameTest(template = "floor")
    public static void trustIsATrackRecordThatOneBadWeekDoesntUndo(GameTestHelper helper) {
        ContractBook book = new ContractBook();
        GlobalPos a = far(helper, 7_300_000, 7_000_000);
        GlobalPos b = far(helper, 7_300_400, 7_000_000);
        long t = 1_000_000;
        helper.assertTrue(book.trust(a, b, t) == ContractBook.START_TRUST, "strangers start at a half");
        book.kept(a, b, 1, t);
        double one = book.trust(a, b, t) - ContractBook.START_TRUST;
        for (int day = 1; day < 20; day++) {
            book.kept(a, b, 1, t + day * 24000L);
        }
        long later = t + 20 * 24000L;
        double steady = book.trust(a, b, later);
        helper.assertTrue(steady > 0.8, "twenty installments kept, one a day: " + steady);
        book.kept(a, b, 1, later);
        helper.assertTrue(book.trust(a, b, later) - steady < one, "each adds less as the record grows");
        book.missed(a, b, 1, later);
        book.missed(a, b, 1, later);
        helper.assertTrue(book.trust(a, b, later) > 0.7, "two misses don't undo it: " + book.trust(a, b, later));
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aNewSupplierIsForgivenWhileItRampsUp(GameTestHelper helper) {
        ContractBook book = new ContractBook();
        GlobalPos fresh = far(helper, 7_400_000, 7_000_000);
        GlobalPos old = far(helper, 7_400_300, 7_000_000);
        GlobalPos buyer = far(helper, 7_400_600, 7_000_000);
        long t = 1_000_000;
        for (int i = 0; i < 4; i++) {
            book.missed(fresh, buyer, 1, t);
        }
        helper.assertTrue(book.trust(fresh, buyer, t) >= ContractBook.MIN_TRUST,
                "four early misses: still trusted enough to deal, " + book.trust(fresh, buyer, t));
        for (int i = 0; i < 3; i++) {
            book.kept(old, buyer, 1, t);
        }
        double before = book.trust(old, buyer, t);
        book.missed(old, buyer, 1, t);
        helper.assertTrue(before - book.trust(old, buyer, t) > 0.05, "past its first installments, a miss counts whole");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void missesAreForgivenWithTime(GameTestHelper helper) {
        ContractBook book = new ContractBook();
        GlobalPos a = far(helper, 7_500_000, 7_000_000);
        GlobalPos b = far(helper, 7_500_400, 7_000_000);
        long t = 1_000_000;
        for (int i = 0; i < 3; i++) {
            book.kept(a, b, 1, t);
        }
        for (int i = 0; i < 10; i++) {
            book.missed(a, b, 1, t);
        }
        helper.assertTrue(book.trust(a, b, t) < ContractBook.MIN_TRUST, "a bad run: " + book.trust(a, b, t));
        long later = t + 2 * ContractBook.MISS_HALF_LIFE;
        helper.assertTrue(book.trust(a, b, later) > 0.45, "two half-lives on, mostly forgiven: " + book.trust(a, b, later));
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void anInstallmentDeliveredInPartIsKeptInPart(GameTestHelper helper) {
        ContractBook book = new ContractBook();
        GlobalPos seller = far(helper, 7_600_000, 7_000_000);
        GlobalPos buyer = far(helper, 7_600_400, 7_000_000);
        long t = 1_000_000;
        Contract c = book.draft(seller, buyer, new Delivery(Items.APPLE, 10, 0.25), Incoterm.DAP, t, 24000, 4, t,
                seller, true);
        book.settle(c, 27, 6.75, t);
        book.review(t + 2 * 24000L + Contract.GRACE_TICKS + 1); // three installments judged: 10, 20 and 30 owed
        var record = book.record(seller, buyer);
        helper.assertTrue(c.missed() == 0, "three short of thirty: not a miss toward breaking it");
        helper.assertTrue(Math.abs(record.kept() - 2.7) < 0.01, "kept two and seven tenths: " + record.kept());
        helper.assertTrue(record.missed() > 0 && record.missed() < 0.3, "the shortfall, lightly: " + record.missed());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void anAdvanceIsPaidUpFrontAndWorkedOff(GameTestHelper helper) {
        ContractBook book = new ContractBook();
        GlobalPos seller = far(helper, 7_700_000, 7_000_000);
        GlobalPos buyer = far(helper, 7_700_400, 7_000_000);
        long t = 1_000_000;
        Contract c = book.draft(seller, buyer, java.util.List.of(new Delivery(Items.IRON_SWORD, 3, 1.0),
                new dev.b28.enterprise.contract.Advance(1 / 3.0)), Incoterm.EXW, t, 24000, 4, t, seller, true);
        helper.assertTrue(Math.abs(c.advanceDue() - 4) < 1e-9, "a third of twelve emeralds up front: " + c.advanceDue());
        book.advanced(c, 4, t);
        helper.assertTrue(c.advanceDue() < 1e-9, "paid");
        double off = c.recoupFrom(3, 3);
        helper.assertTrue(Math.abs(off - 1) < 1e-9, "a third of each delivery's price is taken off: " + off);
        book.recouped(c, off);
        book.settle(c, 3, 3, t);
        book.recouped(c, c.recoupFrom(3, 3));
        book.settle(c, 3, 3, t);
        book.recouped(c, c.recoupFrom(3, 3));
        book.settle(c, 3, 3, t);
        double last = c.recoupFrom(3, 3);
        helper.assertTrue(Math.abs(last - 1) < 1e-9, "and the last delivery takes off what's left: " + last);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aDialIsSetForTheWorldAndSavedWithIt(GameTestHelper helper) {
        var server = helper.getLevel().getServer();
        var dials = dev.b28.enterprise.Dials.get(server);
        var dial = dev.b28.enterprise.Dials.FORWARD_INSTALLMENTS;
        dials.set(dial, 6.0);
        boolean set = dial.getInt() == 6;
        var registries = helper.getLevel().registryAccess();
        var saved = dials.save(new net.minecraft.nbt.CompoundTag(), registries);
        dials.set(dial, null); // back to its default, within the tick: no other test sees it
        helper.assertTrue(set && saved.getDouble("forward_installments") == 6 && dial.getInt() == 10,
                "set to 6, saved as 6, reset to 10: " + saved + ", now " + dial.get());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aThinHarvestRollsOverWhereAPlainContractMisses(GameTestHelper helper) {
        ContractBook book = new ContractBook();
        GlobalPos farm = far(helper, 7_850_000, 7_000_000);
        GlobalPos town = far(helper, 7_850_400, 7_000_000);
        GlobalPos other = far(helper, 7_850_800, 7_000_000);
        long t = 1_000_000;
        var harvest = new dev.b28.enterprise.contract.Harvest(4);
        Contract forward = book.draft(farm, town, java.util.List.of(new Delivery(Items.WHEAT, 16, 0.05), harvest),
                Incoterm.DDP, t, 24000, 5, t, farm, true);
        Contract plain = book.draft(other, town, java.util.List.of(new Delivery(Items.WHEAT, 16, 0.05)),
                Incoterm.DDP, t, 24000, 5, t, other, true);
        book.settle(forward, 8, 0.4, t);
        book.settle(plain, 8, 0.4, t);
        book.review(t + harvest.windowTicks() + 1); // the first installment judged, half delivered
        helper.assertTrue(forward.missed() == 0 && forward.inForce(), "a thin harvest rolls over: " + forward.history());
        helper.assertTrue(plain.state() == Contract.State.BROKEN,
                "a plain contract, judged on its two days' grace, has missed three and broken: " + plain.history());
        book.review(t + 2 * 24000L + harvest.windowTicks() + 1); // three judged, 40 behind: past two installments
        helper.assertTrue(forward.missed() == 1, "arrears past two installments are a miss: " + forward.history());
        book.settle(forward, 40 + 16, 2.8, t + 3 * 24000L); // caught up, and the fourth
        book.review(t + 4 * 24000L + harvest.windowTicks() + 1); // the last falls due: 16 short
        helper.assertTrue(forward.missed() == 2 && forward.inForce(),
                "what's owed at the last is a miss, not rolled: " + forward.history());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aRentPaysACouponEachSeasonAndCarriesItsArrears(GameTestHelper helper) {
        ContractBook book = new ContractBook();
        var rents = new dev.b28.enterprise.contract.Rents();
        GlobalPos farm = far(helper, 7_860_000, 7_000_000);
        GlobalPos holder = far(helper, 7_860_400, 7_000_000);
        long t = 1_000_000;
        long season = dev.b28.enterprise.contract.Rents.seasonTicks() + new dev.b28.enterprise.contract.Harvest(4).windowTicks();
        var rent = rents.sell(farm, holder, Items.POTATO, 64, dev.b28.enterprise.contract.Rents.capital(64, 0.05), book, t);
        Contract first = book.contract(rent.coupon()).orElseThrow();
        helper.assertTrue(first.inForce() && first.delivery().orElseThrow().quantity() == 64
                && first.delivery().orElseThrow().unitPrice() == 0, "a coupon at once, already paid for: " + first.history());
        helper.assertTrue(Math.abs(rent.capital() - 8 * 64 * 0.05) < 1e-9, "bought for eight seasons' crop: " + rent.capital());
        book.settle(first, 40, 0, t);
        book.review(t + season); // the coupon judged: 24 short, carried by the rent, not a miss
        helper.assertTrue(first.missed() == 0, "a coupon's shortfall isn't a miss: " + first.history());
        rents.review(book, t + season);
        Contract second = book.contract(rent.coupon()).orElseThrow();
        helper.assertTrue(first.state() == Contract.State.COMPLETED && rent.arrears() == 24
                && second.delivery().orElseThrow().quantity() == 88, "the shortfall rides on the next coupon: "
                        + rent.history());
        rents.review(book, t + 2 * season); // nothing delivered: 88 in arrears, within two seasons'
        helper.assertTrue(rent.state() == dev.b28.enterprise.contract.Rents.State.ACTIVE && rent.arrears() == 88,
                "still in force: " + rent.history());
        rents.review(book, t + 3 * season); // 152 in arrears: past two seasons' crop
        helper.assertTrue(rent.state() == dev.b28.enterprise.contract.Rents.State.DEFAULTED,
                "defaulted: " + rent.history());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aBrokenContractOwesBackItsUnearnedAdvance(GameTestHelper helper) {
        ContractBook book = new ContractBook();
        GlobalPos seller = far(helper, 7_800_000, 7_000_000);
        GlobalPos buyer = far(helper, 7_800_400, 7_000_000);
        long t = 1_000_000;
        Contract c = book.draft(seller, buyer, java.util.List.of(new Delivery(Items.IRON_SWORD, 3, 1.0),
                new dev.b28.enterprise.contract.Advance(1 / 3.0)), Incoterm.DAP, t, 24000, 4, t, seller, true);
        book.advanced(c, 4, t);
        book.review(t + 3 * 24000L + Contract.GRACE_TICKS + 1); // all four judged, none delivered
        helper.assertTrue(c.state() == Contract.State.BROKEN, "broken: " + c.history());
        helper.assertTrue(Math.abs(c.refund() - 4) < 1e-9, "the advance is owed back: " + c.refund());
        helper.assertTrue(book.refunds(seller, buyer).contains(c), "for the buyer's caravan to collect");
        var registries = helper.getLevel().registryAccess();
        var reloaded = ContractBook.load(book.save(new net.minecraft.nbt.CompoundTag(), registries), registries);
        helper.assertTrue(Math.abs(reloaded.contract(c.id()).orElseThrow().refund() - 4) < 1e-9
                && Math.abs(reloaded.trust(seller, buyer) - book.trust(seller, buyer)) < 1e-9, "saved and loaded whole");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void belowTrustTheyStillDealOnTrialTerms(GameTestHelper helper) {
        var server = helper.getLevel().getServer();
        GlobalPos desk = far(helper, 7_900_000, 7_000_000);
        GlobalPos supplier = far(helper, 7_900_300, 7_000_000);
        Warehouse.get(server).recordStocktake(supplier, Map.of("minecraft:paper", 64), 0);
        dev.b28.enterprise.caravan.Atlas.get(server).learn(desk, supplier, 0);
        var ledger = dev.b28.enterprise.caravan.CaravanLedger.get(server);
        var route = ledger.addRoute(desk, supplier);
        var book = ContractBook.get(server);
        long t = helper.getLevel().getGameTime();
        for (int i = 0; i < 3; i++) {
            book.kept(supplier, desk, 1, t);
        }
        for (int i = 0; i < 12; i++) {
            book.missed(supplier, desk, 1, t);
        }
        helper.assertTrue(book.trust(supplier, desk, t) < ContractBook.MIN_TRUST, "a supplier that let it down");
        Broker.source(helper.getLevel(), desk, new Territory(), Items.PAPER, 40, t);
        var contract = book.of(desk).stream().filter(c -> c.seller().equals(supplier)).findFirst().orElseThrow();
        helper.assertTrue(contract.delivery().get().quantity() == Broker.TRIAL_LOT && contract.advance().isEmpty(),
                "still asked, for a trial lot with nothing up front: " + contract.history());
        ledger.removeRoute(route.id());
        helper.succeed();
    }
}
