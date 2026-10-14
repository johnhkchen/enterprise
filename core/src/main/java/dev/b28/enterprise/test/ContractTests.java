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

/** Contracts' shipping terms, and sourcing from a supplier town. Parties far from any arena. */
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
}
