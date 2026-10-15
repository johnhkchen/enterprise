package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.caravan.Caravan;
import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.caravan.Caravans;
import dev.b28.enterprise.caravan.Route;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.MoneyChanger;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.entity.EntityType;
import java.util.List;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/** A hub and a destination 4 blocks apart: 80 ticks each way, 1 food each way. Each test keeps its own ledger. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class CaravanTests {
    private static final BlockPos HUB = new BlockPos(1, 2, 3);
    private static final BlockPos HUB_CHEST = new BlockPos(0, 2, 3);
    private static final BlockPos DEST = new BlockPos(5, 2, 3);
    private static final BlockPos DEST_CHEST = new BlockPos(6, 2, 3);
    private static final ItemCost PAPER = new ItemCost(Items.PAPER, 1);

    private record Network(GameTestHelper helper, CaravanLedger ledger, Route route,
            dev.b28.enterprise.territory.Territory territory) {
        MerchantStock hub() {
            return MerchantStock.at(helper.getLevel(), helper.absolutePos(HUB));
        }

        MerchantStock dest() {
            return MerchantStock.at(helper.getLevel(), helper.absolutePos(DEST));
        }

        void poll(long at) {
            // Its own territory: other tests' censuses claim real hexes, and their tolls would leak in.
            Caravans.poll(helper.getLevel().getServer(), ledger, territory, at);
        }
    }

    private static Network network(GameTestHelper helper, int hubBread) {
        return network(helper, hubBread, 5, 192);
    }

    /** As below, with a squad of one Librarian at the hub. */
    private static Network network(GameTestHelper helper, int hubBread, int hubEmeralds, int destSpurs) {
        return network(helper, hubBread, hubEmeralds, destSpurs, List.of(VillagerProfession.LIBRARIAN));
    }

    /** A trained villager of {@code trade} by the hub, ready to go with a caravan. */
    private static Villager squadMember(GameTestHelper helper, VillagerProfession trade, BlockPos at) {
        Villager member = helper.spawn(EntityType.VILLAGER, at);
        member.setVillagerData(member.getVillagerData().setProfession(trade).setLevel(2));
        member.setVillagerXp(1); // trained: keeps its trade without a workstation
        return member;
    }

    /**
     * A hub with paper, bread, {@code hubEmeralds} and a squad of {@code squad}; a destination holding
     * {@code destSpurs} in coins and bread.
     */
    private static Network network(GameTestHelper helper, int hubBread, int hubEmeralds, int destSpurs,
            List<VillagerProfession> squad) {
        helper.setBlock(HUB, Merchant.DESK.get());
        helper.setBlock(HUB_CHEST, Blocks.CHEST);
        helper.setBlock(DEST, Merchant.DESK.get());
        helper.setBlock(DEST_CHEST, Blocks.CHEST);
        Villager merchant = helper.spawn(EntityType.VILLAGER, new BlockPos(1, 2, 4));
        merchant.setVillagerData(merchant.getVillagerData().setProfession(Merchant.PROFESSION.get()));
        LockerTradingTests.claim(helper, merchant, HUB);
        for (int i = 0; i < squad.size(); i++) {
            squadMember(helper, squad.get(i), new BlockPos(0, 2, 5 + i % 2).east(i / 2));
        }

        var dimension = helper.getLevel().dimension();
        GlobalPos hub = GlobalPos.of(dimension, helper.absolutePos(HUB));
        GlobalPos dest = GlobalPos.of(dimension, helper.absolutePos(DEST));
        // The destination's librarian buys paper.
        MerchantBook.get(helper.getLevel().getServer()).setEnabled(
                dest, helper.absolutePos(new BlockPos(5, 2, 6)), "minecraft:paper+>minecraft:emerald", true);
        CaravanLedger ledger = new CaravanLedger();
        Network network = new Network(helper, ledger, ledger.addRoute(hub, dest), new dev.b28.enterprise.territory.Territory());
        network.hub().insert(new ItemStack(Items.PAPER, 64));
        network.hub().insert(new ItemStack(Items.PAPER, 64));
        if (hubBread > 0) {
            network.hub().insert(new ItemStack(Items.BREAD, hubBread));
        }
        if (hubEmeralds > 0) {
            network.hub().insert(new ItemStack(Items.EMERALD, hubEmeralds)); // funds the caravan's purse
        }
        MoneyChanger.coins(destSpurs).forEach(network.dest()::insert); // the destination Merchant's money
        network.dest().insert(new ItemStack(Items.BREAD, 2));
        return network;
    }

    @GameTest(template = "floor")
    public static void caravansAreCommissionedAtTheDesk(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        var server = helper.getLevel().getServer();
        GlobalPos hub = net.route().from();
        GlobalPos dest = net.route().to();
        var view = dev.b28.enterprise.merchant.MerchantView.build(helper.getLevel(), hub.pos(), 4);
        helper.assertTrue(view.caravans().towns().stream().anyMatch(t -> t.desk().equals(dest.pos())),
                "the desk knows its neighbour by word of mouth");
        helper.assertTrue(view.caravans().squad().equals(List.of("minecraft:librarian")), "a Librarian at home to send");
        var ledger = CaravanLedger.get(server);
        helper.assertTrue(dev.b28.enterprise.merchant.CommissionPayload.commission(server, hub, view, dest.pos(), true)
                && ledger.route(hub, dest).isPresent(), "Send caravans: a route");
        helper.assertTrue(dev.b28.enterprise.merchant.CommissionPayload.commission(server, hub, view, dest.pos(), false)
                && ledger.route(hub, dest).isEmpty(), "Stop caravans: gone");
        helper.assertFalse(dev.b28.enterprise.merchant.CommissionPayload.commission(server, hub, view,
                dest.pos().offset(500, 0, 0), true), "only markets the screen showed");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aTownShortOfComfortsCommissionsItsOwnCaravan(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        var server = helper.getLevel().getServer();
        var territory = net.territory();
        BlockPos bell = helper.absolutePos(new BlockPos(6, 2, 4)); // the destination's town
        territory.afterCensus(helper.getLevel(), bell, new dev.b28.enterprise.settlement.Census(5, 5, 5, 5, 0), 1, List.of());
        GlobalPos town = net.route().to();
        GlobalPos hub = net.route().from();
        long t = helper.getLevel().getGameTime();
        net.dest().insert(new ItemStack(Items.EMERALD, 30));
        territory.touch(helper.getLevel(), town.pos(), net.dest(), t);
        territory.touch(helper.getLevel(), town.pos(), net.dest(), t + 24000); // a day without comforts
        dev.b28.enterprise.merchant.Warehouse.get(server).recordStocktake(hub, java.util.Map.of("minecraft:cookie", 64), t);
        dev.b28.enterprise.caravan.Atlas.get(server).learn(town, hub, t); // its caravans have been there
        var route = dev.b28.enterprise.caravan.Commissions.consider(helper.getLevel(), town, net.dest(),
                GlobalPos.of(town.dimension(), bell), territory, t + 24000);
        helper.assertTrue(route.isPresent() && route.get().to().equals(hub),
                "it sends to the market it knows has treats: " + route.map(r -> r.to().pos().toShortString()));
        CaravanLedger.get(server).removeRoute(route.get().id());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aTownWithARouteTrainsItsSquad(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192, List.of());
        var server = helper.getLevel().getServer();
        BlockPos bell = helper.absolutePos(new BlockPos(0, 2, 1)); // the hub's own town
        net.territory().afterCensus(helper.getLevel(), bell, new dev.b28.enterprise.settlement.Census(5, 5, 5, 5, 0), 1, List.of());
        net.hub().insert(new ItemStack(Items.EMERALD, 20));
        Villager pupil = helper.spawn(EntityType.VILLAGER, new BlockPos(0, 2, 2));
        var route = CaravanLedger.get(server).addRoute(net.route().from(), net.route().to());
        var trained = dev.b28.enterprise.caravan.Commissions.train(helper.getLevel(), net.route().from(), bell, net.hub(),
                net.territory());
        CaravanLedger.get(server).removeRoute(route.id());
        helper.assertTrue(trained.isPresent() && trained.get() == pupil, "the unemployed adult is trained");
        helper.assertTrue(pupil.getVillagerData().getProfession() == Merchant.PROFESSION.get() && pupil.getVillagerXp() > 0,
                "as a Merchant first, and keeps the trade");
        pupil.discard(); // a trained Merchant without a desk would go looking for a neighbouring test's
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aTownsOwnCaravanShopsForHome(GameTestHelper helper) {
        Network net = network(helper, 2, 20, 192);
        BlockPos bell = helper.absolutePos(new BlockPos(0, 2, 1)); // the hub is a town of five, with no comforts
        net.territory().afterCensus(helper.getLevel(), bell, new dev.b28.enterprise.settlement.Census(5, 5, 5, 5, 0), 1, List.of());
        net.dest().insert(new ItemStack(Items.COOKIE, 64)); // the market it sends to has treats to spare
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        net.poll(t + 80);
        net.poll(t + 160);
        int cookies = net.hub().count(new ItemCost(Items.COOKIE, 1));
        // Its par of treats is 5 x 0.1 x 4 = 2 emeralds' worth: 36 cookies.
        helper.assertTrue(cookies == 36, "it brought home its par of treats: " + cookies);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void noSquadNoCaravan(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192, List.of());
        net.poll(helper.getLevel().getGameTime());
        helper.assertTrue(net.ledger().caravan(net.route().id()).isEmpty(), "nobody to send, nobody goes");
        helper.assertTrue(net.route().status().contains("squad"), "the route says so: " + net.route().status());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aTownShortOfCrewTakesOnAHand(GameTestHelper helper) {
        Network net = network(helper, 2, 8, 192, List.of());
        Villager hand = helper.spawn(EntityType.VILLAGER, new BlockPos(0, 2, 5)); // an adult waiting for work
        java.util.UUID id = hand.getUUID();
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        helper.assertTrue(hand.getVillagerData().getProfession() == Merchant.PROFESSION.get() && hand.getVillagerXp() > 0,
                "the Merchant takes the hand on as a travelling Merchant");
        helper.assertTrue(net.ledger().caravan(net.route().id()).isPresent(), "who leads the caravan: " + net.route().status());
        helper.assertTrue(helper.getLevel().getEntity(id) == null, "off the world while on the road");
        net.poll(t + 80);
        net.poll(t + 160);
        helper.assertTrue(helper.getLevel().getEntity(id) instanceof Villager back
                        && back.getVillagerData().getProfession() == Merchant.PROFESSION.get(),
                "home again, still a travelling Merchant");
        helper.getLevel().getEntity(id).discard(); // it would go looking for a neighbouring test's desk
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void theSquadLeavesTheWorldAndComesBack(GameTestHelper helper) {
        // 70 bread (350 food points): food to spare past the caravans' 320-point par, for a second member.
        Network net = network(helper, 70, 5, 192, List.of(VillagerProfession.CARTOGRAPHER, VillagerProfession.LIBRARIAN));
        var squad = helper.getLevel().getEntitiesOfClass(Villager.class,
                new net.minecraft.world.phys.AABB(helper.absolutePos(HUB)).inflate(8),
                v -> v.getVillagerData().getProfession() != Merchant.PROFESSION.get());
        helper.assertTrue(squad.size() == 2, "two to send: " + squad.size());
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        Caravan out = net.ledger().caravan(net.route().id()).orElseThrow();
        helper.assertTrue(out.crew().size() == 2, "both go: no books on that town yet, its road unmapped");
        helper.assertTrue(squad.stream().allMatch(v -> helper.getLevel().getEntity(v.getUUID()) == null),
                "both are off the world while on the road");
        helper.assertTrue(net.hub().foodNutrition() == 345, "both eat: 2 food points, one bread");
        net.poll(t + 80);
        net.poll(t + 160);
        helper.assertTrue(net.ledger().caravan(net.route().id()).isEmpty(), "home");
        helper.assertTrue(squad.stream().allMatch(v -> helper.getLevel().getEntity(v.getUUID()) instanceof Villager back
                        && back.getVillagerData().getProfession() == v.getVillagerData().getProfession()),
                "both are back, still in their trades");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void extraMembersGoOnlyWhereTheyHelp(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192, List.of(VillagerProfession.CARTOGRAPHER, VillagerProfession.SHEPHERD));
        var desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(HUB));
        var noNeed = new dev.b28.enterprise.caravan.Squad.Needs(false, false, false);
        helper.assertTrue(dev.b28.enterprise.caravan.Squad.muster(helper.getLevel(), desk, 16, noNeed, 3).size() == 1,
                "nothing to map, no animals: the lead goes alone");
        var animals = new dev.b28.enterprise.caravan.Squad.Needs(false, false, true);
        var squad = dev.b28.enterprise.caravan.Squad.muster(helper.getLevel(), desk, 16, animals, 3);
        helper.assertTrue(squad.size() == 2 && dev.b28.enterprise.caravan.Squad.trade(squad.get(1)).equals("minecraft:shepherd"),
                "animals to drive: the Shepherd comes too");
        helper.assertTrue(dev.b28.enterprise.caravan.Squad.muster(helper.getLevel(), desk, 16, animals, 0).size() == 1,
                "unless she's another route's lead");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aClericGoesAlongButNeverLeads(GameTestHelper helper) {
        network(helper, 2, 5, 192, List.of(VillagerProfession.CLERIC));
        var desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(HUB));
        helper.assertTrue(dev.b28.enterprise.caravan.Squad.muster(helper.getLevel(), desk, 16,
                dev.b28.enterprise.caravan.Squad.Needs.ALL, 3).isEmpty(), "a Cleric alone doesn't drive the wagon");
        helper.getLevel().getEntitiesOfClass(Villager.class, new net.minecraft.world.phys.AABB(helper.absolutePos(HUB)).inflate(8),
                v -> v.getVillagerData().getProfession() == VillagerProfession.CLERIC).forEach(Villager::discard);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aVisitingClericBlessesATownWithoutOne(GameTestHelper helper) {
        Network net = network(helper, 70, 5, 192, List.of(VillagerProfession.LIBRARIAN, VillagerProfession.CLERIC));
        BlockPos bellPos = helper.absolutePos(new BlockPos(6, 2, 4)); // the destination's town
        net.territory().afterCensus(helper.getLevel(), bellPos, new dev.b28.enterprise.settlement.Census(5, 5, 5, 5, 0), 1,
                List.of());
        GlobalPos bell = GlobalPos.of(helper.getLevel().dimension(), bellPos);
        net.dest().insert(new ItemStack(Items.EMERALD, 10)); // enough to pay for a blessing
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        Caravan out = net.ledger().caravan(net.route().id()).orElseThrow();
        helper.assertTrue(out.crew().stream().anyMatch(m -> dev.b28.enterprise.caravan.Squad.trade(m).equals("minecraft:cleric")),
                "a town without a Cleric: the Cleric goes too");
        net.poll(t + 80);
        helper.assertTrue(net.territory().blessedUntil(bell) > t + 24000, "the town is blessed for days");
        helper.assertTrue(net.route().serviceSpurs() > 0, "and paid for it: " + net.route().serviceSpurs());
        net.poll(t + 160);
        helper.assertTrue(net.ledger().caravan(net.route().id()).isEmpty(), "home");
        helper.getLevel().getEntitiesOfClass(Villager.class, new net.minecraft.world.phys.AABB(helper.absolutePos(HUB)).inflate(8),
                v -> v.getVillagerData().getProfession() == VillagerProfession.CLERIC).forEach(Villager::discard);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void onlyTheDesksCommunityIsMustered(GameTestHelper helper) {
        Villager cartographer = squadMember(helper, VillagerProfession.CARTOGRAPHER, new BlockPos(3, 2, 3));
        var dimension = helper.getLevel().dimension();
        var farDesk = GlobalPos.of(dimension, helper.absolutePos(new BlockPos(3, 2, 3)).offset(30, 0, 0));
        helper.assertFalse(dev.b28.enterprise.caravan.Squad.connected(cartographer, farDesk, java.util.Set.of(),
                java.util.Set.of()), "a stranger to that desk's Merchant: not its squad");
        helper.assertTrue(dev.b28.enterprise.caravan.Squad.connected(cartographer, farDesk,
                java.util.Set.of(cartographer.getUUID()), java.util.Set.of()), "a resident of its town: yes");
        helper.assertTrue(dev.b28.enterprise.caravan.Squad.connected(cartographer,
                GlobalPos.of(dimension, helper.absolutePos(new BlockPos(1, 2, 3))), java.util.Set.of(), java.util.Set.of()),
                "standing in its market: yes");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aLeatherworkersPacksCarryMore(GameTestHelper helper) {
        Network net = network(helper, 70, 5, 192, List.of(VillagerProfession.LIBRARIAN, VillagerProfession.LEATHERWORKER));
        helper.setBlock(HUB.north(), Blocks.CHEST); // a second warehouse chest: room for the paper
        for (int i = 0; i < 30; i++) {
            net.hub().insert(new ItemStack(Items.PAPER, 64)); // 32 stacks of paper for a town that buys paper
        }
        net.poll(helper.getLevel().getGameTime());
        Caravan out = net.ledger().caravan(net.route().id()).orElseThrow();
        helper.assertTrue(out.crew().size() == 2 && out.capacity() == 36, "with packs: 36 stacks, " + out.capacity());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aPorterCarriesOutAndWalksHome(GameTestHelper helper) {
        Network net = network(helper, 70, 5, 192, List.of(VillagerProfession.LIBRARIAN, VillagerProfession.NITWIT));
        helper.setBlock(HUB.north(), Blocks.CHEST); // a second warehouse chest: room for the paper
        for (int i = 0; i < 30; i++) {
            net.hub().insert(new ItemStack(Items.PAPER, 64)); // 32 stacks of paper for a town that buys paper
        }
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        Caravan out = net.ledger().caravan(net.route().id()).orElseThrow();
        helper.assertTrue(out.crew().size() == 2 && out.capacity() == 36, "a porter: 36 stacks, " + out.capacity());
        net.poll(t + 80);
        helper.assertTrue(out.crew().size() == 1 && net.ledger().walkers().size() == 1,
                "the load delivered, the porter walks home: " + out.crew().size());
        net.poll(t + 160);
        helper.assertTrue(net.ledger().walkers().isEmpty(), "and is home");
        helper.assertTrue(!helper.getLevel().getEntitiesOfClass(Villager.class,
                new net.minecraft.world.phys.AABB(helper.absolutePos(HUB)).inflate(8),
                v -> v.getVillagerData().getProfession() == VillagerProfession.NITWIT).isEmpty(), "by the desk");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aSaddledCaravanRidesFaster(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        net.hub().insert(new ItemStack(Items.SADDLE, 1));
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        Caravan out = net.ledger().caravan(net.route().id()).orElseThrow();
        helper.assertTrue(out.arrives() == t + 60, "a third faster: there at " + (out.arrives() - t) + " ticks, not 80");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aShopsOrderAtTheFarEndRidesOut(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        net.hub().insert(new ItemStack(Items.APPLE, 64));
        var exchange = dev.b28.enterprise.market.Exchange.get(helper.getLevel().getServer());
        exchange.setOrdersForTest(net.route().to(), java.util.Map.of(Items.APPLE,
                new dev.b28.enterprise.market.Exchange.Order(Items.APPLE, 32, 4, BlockPos.ZERO)));
        net.poll(helper.getLevel().getGameTime());
        Caravan out = net.ledger().caravan(net.route().id()).orElseThrow();
        helper.assertTrue(out.cargo().stream().anyMatch(stack -> stack.is(Items.APPLE)),
                "a player's shop there ordered apples: the caravan brings some");
        exchange.setOrdersForTest(net.route().to(), java.util.Map.of());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void exWorksTheBuyersCaravanCollects(GameTestHelper helper) {
        Network net = network(helper, 2, 20, 192);
        net.dest().insert(new ItemStack(Items.FLINT, 32));
        var book = dev.b28.enterprise.contract.ContractBook.get(helper.getLevel().getServer());
        long t = helper.getLevel().getGameTime();
        var contract = book.draft(net.route().to(), net.route().from(),
                new dev.b28.enterprise.contract.Delivery(Items.FLINT, 16, 0.25), dev.b28.enterprise.contract.Incoterm.EXW,
                t, 24000, 1, t, net.route().to(), true);
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(net.dest().count(new ItemCost(Items.FLINT, 1)) == 16, "the hub's caravan took its 16 flint: "
                + net.route().status() + ", " + net.ledger().caravan(net.route().id()).map(c -> c.spurs() + " spurs")
                .orElse("no caravan") + ", " + contract.history());
        net.poll(t + 160);
        helper.assertTrue(net.hub().count(new ItemCost(Items.FLINT, 1)) == 16, "and brought them home");
        helper.assertTrue(contract.state() == dev.b28.enterprise.contract.Contract.State.COMPLETED
                && contract.paid() >= 3.9, "paid for at the locked price: " + contract.paid());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aContractsCopyRidesWithTheCaravan(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        var book = dev.b28.enterprise.contract.ContractBook.get(helper.getLevel().getServer());
        long t = helper.getLevel().getGameTime();
        var contract = book.draft(net.route().from(), net.route().to(),
                new dev.b28.enterprise.contract.Delivery(Items.PAPER, 16, 0.05), dev.b28.enterprise.contract.Incoterm.DAP,
                t, 24000, 3, t, net.route().from(), false);
        helper.assertFalse(contract.inForce(), "drafted at the hub: not yet in force");
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(contract.inForce() && contract.knownBy(net.route().to()), "the caravan carried its copy there");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void contractedGoodsShipInCratesWithTheirPapers(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        var book = dev.b28.enterprise.contract.ContractBook.get(helper.getLevel().getServer());
        long t = helper.getLevel().getGameTime();
        var contract = book.draft(net.route().from(), net.route().to(),
                new dev.b28.enterprise.contract.Delivery(Items.PAPER, 64, 0.05), dev.b28.enterprise.contract.Incoterm.DDP,
                t, 24000, 1, t, net.route().from(), true);
        net.poll(t);
        Caravan out = net.ledger().caravan(net.route().id()).orElseThrow();
        var crate = out.cargo().stream().filter(dev.b28.enterprise.cargo.Crates::isCrate).findFirst();
        helper.assertTrue(crate.isPresent(), "the paper rides in a crate");
        var papers = dev.b28.enterprise.cargo.Crates.papers(crate.get());
        helper.assertTrue(papers.consignee().filter(net.route().to()::equals).isPresent()
                        && papers.contract().filter(id -> id == contract.id()).isPresent(),
                "its bill of lading names the town and the contract: " + papers.describe());
        net.poll(t + 80);
        helper.assertTrue(contract.delivered() == 64, "opened and delivered under the contract: " + contract.delivered());
        helper.assertTrue(net.dest().count(new ItemCost(dev.b28.enterprise.cargo.Cargo.CRATE_ITEM.get(), 1)) >= 1,
                "the empty crate stays, to ship with");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void whatAPlayerBroughtIsPaidForByTheNextCaravan(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        var server = helper.getLevel().getServer();
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        long t = helper.getLevel().getGameTime();
        var contract = book.draft(net.route().from(), net.route().to(),
                new dev.b28.enterprise.contract.Delivery(Items.FLINT, 16, 0.25), dev.b28.enterprise.contract.Incoterm.DAP,
                t, 24000, 1, t, net.route().from(), true);
        // A player carried a crate from the hub and put it in the town's warehouse.
        var goods = List.of(new ItemStack(Items.FLINT, 16));
        net.dest().insert(dev.b28.enterprise.cargo.Crates.of(goods, dev.b28.enterprise.cargo.Crates.shipping(contract,
                net.route().from(), net.route().to(), goods, 0, t, false)));
        dev.b28.enterprise.cargo.Crates.receive(server, helper.getLevel(), net.route().to(), net.dest(), t);
        helper.assertTrue(contract.delivered() == 16 && net.dest().count(new ItemCost(Items.FLINT, 1)) == 16,
                "the Merchant opened it and took delivery");
        helper.assertTrue(Math.abs(contract.owing() - 4) < 1e-9, "4 emeralds owed to the hub: " + contract.owing());
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(contract.owing() < 0.01 && contract.paid() >= 3.9, "the hub's caravan collected it: "
                + contract.history());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void shortOfCoinTheBuyerPaysWithABill(GameTestHelper helper) {
        Network net = network(helper, 2, 1, 192);
        net.dest().insert(new ItemStack(Items.FLINT, 32));
        var server = helper.getLevel().getServer();
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        long t = helper.getLevel().getGameTime();
        var contract = book.draft(net.route().to(), net.route().from(),
                new dev.b28.enterprise.contract.Delivery(Items.FLINT, 32, 0.5), dev.b28.enterprise.contract.Incoterm.EXW,
                t, 24000, 1, t, net.route().to(), true);
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(contract.delivered() == 32, "collected in full: " + contract.history());
        var bills = dev.b28.enterprise.contract.Bills.get(server).all().stream()
                .filter(b -> b.drawee().equals(net.route().from()) && b.payee().equals(net.route().to())).toList();
        helper.assertTrue(!bills.isEmpty(), "the rest on a bill drawn on the hub, left with the town");
        net.poll(t + 160);
        net.hub().insert(new ItemStack(Items.EMERALD, 30)); // the hub has money again
        long later = t + dev.b28.enterprise.contract.Bills.TERM_TICKS + 24000;
        net.poll(later);
        net.poll(later + 80);
        helper.assertTrue(bills.stream().allMatch(b -> b.state() == dev.b28.enterprise.contract.Bills.State.PAID),
                "the hub's next caravan, once it was due, paid it");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aSquadIsNeverMoreThanFour(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192, List.of(VillagerProfession.CARTOGRAPHER, VillagerProfession.LIBRARIAN,
                VillagerProfession.SHEPHERD, VillagerProfession.LEATHERWORKER, VillagerProfession.FISHERMAN,
                VillagerProfession.FISHERMAN));
        var desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(HUB));
        var squad = dev.b28.enterprise.caravan.Squad.muster(helper.getLevel(), desk, 16,
                dev.b28.enterprise.caravan.Squad.Needs.ALL, 9);
        helper.assertTrue(squad.size() == dev.b28.enterprise.caravan.Squad.MAX, "four at most: " + squad.size());
        helper.succeed();
    }

    /**
     * A month of the destination's lockers selling 64 paper a day, and 8 more stacks of paper at the hub (enough to
     * promise); returns the time after it.
     */
    private static long aMonthOfPaperSales(GameTestHelper helper, Network net) {
        helper.setBlock(HUB.north(), Blocks.CHEST);
        for (int i = 0; i < 8; i++) {
            net.hub().insert(new ItemStack(Items.PAPER, 64));
        }
        long start = helper.getLevel().getGameTime();
        var warehouse = dev.b28.enterprise.merchant.Warehouse.get(helper.getLevel().getServer());
        for (int day = 0; day <= 30; day++) {
            warehouse.recordUse(net.route().to(), Items.PAPER, 64, start + day * 24000L);
        }
        return start + 30 * 24000L + 100;
    }

    private static List<dev.b28.enterprise.contract.Contract> contractsBetween(GameTestHelper helper, Network net) {
        return dev.b28.enterprise.contract.ContractBook.get(helper.getLevel().getServer()).of(net.route().from()).stream()
                .filter(c -> c.buyer().equals(net.route().to())).toList();
    }

    @GameTest(template = "floor")
    public static void aTravellingMerchantDraftsAStandingOrder(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192, List.of(Merchant.PROFESSION.get()));
        long t = aMonthOfPaperSales(helper, net);
        net.poll(t);
        net.poll(t + 80);
        var drafted = contractsBetween(helper, net);
        helper.assertTrue(drafted.size() == 1 && drafted.get(0).delivery().orElseThrow().item() == Items.PAPER
                && drafted.get(0).delivery().get().quantity() == 64, "a standing order for the 64 paper it bought");
        helper.assertTrue(drafted.get(0).state() == dev.b28.enterprise.contract.Contract.State.DRAFTED
                && drafted.get(0).knownBy(net.route().to()) && !drafted.get(0).knownBy(net.route().from()),
                "the town holds its copy; the hub hasn't heard yet");
        net.poll(t + 160);
        helper.assertTrue(contractsBetween(helper, net).get(0).inForce(), "the copy rode home: in force");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aTownPromisesOnlyWhatItSells(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192, List.of(Merchant.PROFESSION.get()));
        long t = helper.getLevel().getGameTime(); // no record of selling paper
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(contractsBetween(helper, net).isEmpty(), "it bought paper, but sells none yet: no promise");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void contractedGoodsGoFirstAndSettleAtTheLockedPrice(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192, List.of(Merchant.PROFESSION.get()));
        net.hub().insert(new ItemStack(Items.PAPER, 64));
        net.hub().insert(new ItemStack(Items.BREAD, 2));
        long t = aMonthOfPaperSales(helper, net);
        net.poll(t);
        net.poll(t + 80);
        net.poll(t + 160);
        var contract = contractsBetween(helper, net).get(0);
        double price = contract.delivery().orElseThrow().unitPrice();
        // A day on, the next caravan: the town holds its par of paper already, but the contract delivers anyway.
        long next = t + 24000;
        net.poll(next);
        net.poll(next + 80);
        helper.assertTrue(contract.delivered() == 64, "64 paper delivered under contract: " + contract.delivered());
        helper.assertTrue(Math.abs(contract.paid() - 64 * price) < 0.1, "paid at the locked price: " + contract.paid());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void missedInstallmentsCostTrustAndBreakTheContract(GameTestHelper helper) {
        var book = dev.b28.enterprise.contract.ContractBook.get(helper.getLevel().getServer());
        var dimension = helper.getLevel().dimension();
        GlobalPos seller = GlobalPos.of(dimension, helper.absolutePos(new BlockPos(1, 2, 1)));
        GlobalPos buyer = GlobalPos.of(dimension, helper.absolutePos(new BlockPos(5, 2, 5)));
        long t = helper.getLevel().getGameTime();
        var contract = book.draftDelivery(seller, buyer, Items.PAPER, 16, 0.04, t, 24000, 10, t, buyer);
        book.deliverCopy(contract.id(), seller);
        helper.assertTrue(contract.inForce(), "both hold a copy: in force");
        double before = book.trust(seller, buyer);
        book.review(t + 5 * 24000L);
        helper.assertTrue(contract.missed() >= 3 && contract.state() == dev.b28.enterprise.contract.Contract.State.BROKEN,
                "nothing delivered: missed and broken, " + contract.missed());
        helper.assertTrue(book.trust(seller, buyer) < before, "and trust falls");
        var registries = helper.getLevel().registryAccess();
        var reloaded = dev.b28.enterprise.contract.ContractBook.load(book.save(new net.minecraft.nbt.CompoundTag(), registries),
                registries).contract(contract.id()).orElseThrow();
        helper.assertTrue(reloaded.delivery().orElseThrow().item() == Items.PAPER && reloaded.missed() == contract.missed()
                && reloaded.state() == contract.state(), "saved and loaded whole");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void babiesAndTheDesksMerchantStayHome(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192, List.of());
        Villager baby = squadMember(helper, VillagerProfession.CARTOGRAPHER, new BlockPos(0, 2, 5));
        baby.setAge(-24000);
        Villager untrained = helper.spawn(EntityType.VILLAGER, new BlockPos(0, 2, 6));
        var desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(HUB));
        helper.assertFalse(dev.b28.enterprise.caravan.Squad.eligible(baby, desk), "no babies on the road");
        helper.assertTrue(dev.b28.enterprise.caravan.Squad.muster(helper.getLevel(), desk, 16,
                new dev.b28.enterprise.caravan.Squad.Needs(java.util.Set.of(), false, 2), 3).isEmpty(),
                "the untrained go only as porters, and never alone");
        helper.assertTrue(dev.b28.enterprise.caravan.Squad.available(helper.getLevel(), desk, 16).stream()
                        .noneMatch(v -> v.getVillagerData().getProfession() == Merchant.PROFESSION.get()),
                "and not the Merchant who works the desk");
        untrained.discard(); // an unemployed adult would go looking for a neighbouring test's workstation
        baby.discard();
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aShepherdSellsSheepWhereThePastureIsShort(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192, List.of(VillagerProfession.SHEPHERD));
        net.territory().afterCensus(helper.getLevel(), helper.absolutePos(new BlockPos(6, 2, 4)),
                new dev.b28.enterprise.settlement.Census(5, 5, 5, 5, 0), 1, List.of());
        for (int i = 0; i < 3; i++) {
            helper.spawn(EntityType.SHEEP, new BlockPos(2, 2, 1 + i));
        }
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        helper.assertTrue(net.ledger().caravan(net.route().id()).orElseThrow().herd().size() == 3, "she drives 3 sheep");
        net.poll(t + 80);
        helper.assertTrue(net.route().animalsSold() == 3, "the town buys all three: " + net.route().animalsSold());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aCartographerBringsHomeAMapAndTheTownsOnTheWay(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192, List.of(VillagerProfession.CARTOGRAPHER));
        var server = helper.getLevel().getServer();
        // A settlement a hex off the road, known to the ledgers but not yet to the hub.
        BlockPos farBell = helper.absolutePos(new BlockPos(3, 2, 3)).offset(0, 0, 80);
        BlockPos farDesk = farBell.offset(2, 0, 0);
        dev.b28.enterprise.settlement.Settlements.get(server).record(GlobalPos.of(helper.getLevel().dimension(), farBell),
                new dev.b28.enterprise.settlement.Census(5, 5, 5, 5, helper.getLevel().getGameTime()));
        net.territory().setDeskForTest(GlobalPos.of(helper.getLevel().dimension(), farBell),
                GlobalPos.of(helper.getLevel().dimension(), farDesk));
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        net.poll(t + 80);
        net.poll(t + 160);
        helper.assertTrue(net.hub().count(new ItemCost(Items.FILLED_MAP, 1)) == 1, "a map of the road in the warehouse");
        helper.assertTrue(dev.b28.enterprise.caravan.Atlas.get(server).known(server, net.route().from()).contains(
                GlobalPos.of(helper.getLevel().dimension(), farDesk)), "and the hub knows the town it saw");
        helper.assertTrue(net.route().mappedAt() >= 0, "the road is mapped");
        dev.b28.enterprise.settlement.Settlements.get(server).forget(GlobalPos.of(helper.getLevel().dimension(), farBell));
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aLibrarianBringsBackTheBooks(GameTestHelper helper) {
        Network net = network(helper, 70, 5, 192, List.of(VillagerProfession.CARTOGRAPHER));
        var warehouse = dev.b28.enterprise.merchant.Warehouse.get(helper.getLevel().getServer());
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(warehouse.stocktakeTime(net.route().to()) < 0, "no Librarian: nobody counted their shelves");
        net.poll(t + 160);
        Villager librarian = squadMember(helper, VillagerProfession.LIBRARIAN, new BlockPos(0, 2, 5));
        net.poll(t + 160 + 24000);
        net.poll(t + 240 + 24000);
        helper.assertTrue(warehouse.stocktakeTime(net.route().to()) >= 0, "with one, the books come home");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void caravanSellsItsCargoAndBuysItsFood(GameTestHelper helper) {
        Network net = network(helper, 2);
        long t = helper.getLevel().getGameTime();

        net.poll(t);
        Caravan out = net.ledger().caravan(net.route().id()).orElseThrow();
        helper.assertTrue(out.leg() == Caravan.Leg.OUTBOUND && out.items() == 64,
                "leaves with the 64 paper the far end will take (its par, one stack): " + out.items());
        helper.assertTrue(out.spurs() > 0, "and a purse for the road");
        helper.assertTrue(net.hub().count(PAPER) == 64 && net.hub().foodNutrition() == 5,
                "the rest of the paper stays home, one bread eaten");
        int purse = out.spurs();

        int destMoney = MoneyChanger.value(net.dest().looseStacks());
        net.poll(t + 80);
        helper.assertTrue(net.dest().count(PAPER) == dev.b28.enterprise.merchant.Warehouse.MIN_PAR,
                "the destination bought paper up to its par of one stack, bought " + net.dest().count(PAPER));
        Caravan back = net.ledger().caravan(net.route().id()).orElseThrow();
        helper.assertTrue(back.leg() == Caravan.Leg.RETURNING, "heads home");
        int destSpent = destMoney - MoneyChanger.value(net.dest().looseStacks());
        // It paid the bargained price for 64 paper and was paid back for a bread of road food.
        helper.assertTrue(net.route().salesSpurs() > 0 && net.route().foodSpurs() > 0, "a sale and a food purchase");
        helper.assertTrue(destSpent == net.route().salesSpurs() - net.route().foodSpurs(),
                "the destination's money moved by sales less food: " + destSpent);
        helper.assertTrue(back.spurs() == purse + net.route().salesSpurs() - net.route().foodSpurs(),
                "the caravan carries its purse plus sales less food");

        int carried = back.spurs();
        net.hub().insert(new ItemStack(Items.PAPER, 1)); // a sheet for the day's page of its books: no scribe to pay
        int hubBefore = MoneyChanger.value(net.hub().looseStacks()); // includes change from funding the purse
        net.poll(t + 160);
        helper.assertTrue(net.ledger().caravan(net.route().id()).isEmpty(), "the caravan is home");
        helper.assertTrue(MoneyChanger.value(net.hub().looseStacks()) - hubBefore == carried, "the hub banks what it carried");
        helper.assertTrue(net.route().trips() == 1 && net.route().itemsDelivered() == 64, "one trip, 64 sold");
        helper.assertTrue(net.hub().count(PAPER) == 64, "the paper it didn't send is still in the hub's warehouse");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void townsBuyComfortsUpToTheirPar(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        // The destination is a town of five: it wants 4 days of treats, 5 x 0.1 x 4 = 2 emeralds' worth (36 cookies).
        net.territory().afterCensus(helper.getLevel(), helper.absolutePos(new BlockPos(6, 2, 4)),
                new dev.b28.enterprise.settlement.Census(5, 5, 5, 5, 0), 1, java.util.List.of());
        net.hub().insert(new ItemStack(Items.COOKIE, 64));
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        int aboard = net.ledger().caravan(net.route().id()).orElseThrow().cargo().stream()
                .filter(stack -> stack.is(Items.COOKIE)).mapToInt(ItemStack::getCount).sum();
        helper.assertTrue(aboard == 54, "the caravan takes half again what the town wants: " + aboard);
        net.poll(t + 80);
        helper.assertTrue(net.dest().count(new ItemCost(Items.COOKIE, 1)) == 36,
                "the town buys 36 cookies: " + net.dest().count(new ItemCost(Items.COOKIE, 1)));
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void caravansAllowForComfortsUsedSinceTheStocktake(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        var territory = net.territory();
        territory.afterCensus(helper.getLevel(), helper.absolutePos(new BlockPos(6, 2, 4)),
                new dev.b28.enterprise.settlement.Census(5, 5, 5, 5, 0), 1, java.util.List.of());
        var dest = net.route().to();
        long t = helper.getLevel().getGameTime();
        // At the last visit the town was full: 36 cookies, its whole 2-emerald par of treats.
        dev.b28.enterprise.merchant.Warehouse.get(helper.getLevel().getServer())
                .recordStocktake(dest, java.util.Map.of("minecraft:cookie", 36), t);
        var now = Caravans.comfortWantsForTest(helper.getLevel().getServer(), territory, dest, t);
        helper.assertTrue(!now.containsKey("treats"), "just after: it wants no treats " + now);
        var later = Caravans.comfortWantsForTest(helper.getLevel().getServer(), territory, dest, t + 2 * 24000L);
        // Two days on, 5 villagers have eaten 1 emerald's worth: it wants that back.
        helper.assertTrue(Math.abs(later.getOrDefault("treats", 0.0) - 1.0) < 1e-6, "two days on: " + later);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void townsKeepTheirTradingCapital(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192); // the town holds 192 spurs, 12 emeralds
        net.territory().afterCensus(helper.getLevel(), helper.absolutePos(new BlockPos(6, 2, 4)),
                new dev.b28.enterprise.settlement.Census(5, 5, 5, 5, 0), 1, java.util.List.of());
        // Its lockers sell 240 paper a day: two days' stock is 480 paper, 20 emeralds, more than it has.
        long start = helper.getLevel().getGameTime();
        var warehouse = dev.b28.enterprise.merchant.Warehouse.get(helper.getLevel().getServer());
        for (int day = 0; day <= 30; day++) {
            warehouse.recordUse(net.route().to(), Items.PAPER, 240, start + day * 24000L);
        }
        long t = start + 30 * 24000L + 100; // a month of trade later
        net.hub().insert(new ItemStack(Items.COOKIE, 64));
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(net.dest().count(new ItemCost(Items.COOKIE, 1)) == 0, "no cookies: the money is for paper");
        helper.assertTrue(net.dest().count(PAPER) > 0, "and it bought paper");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aTownShortOfClothesBuysAPairOfBoots(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        net.territory().afterCensus(helper.getLevel(), helper.absolutePos(new BlockPos(6, 2, 4)),
                new dev.b28.enterprise.settlement.Census(5, 5, 5, 5, 0), 1, java.util.List.of());
        for (int i = 0; i < 3; i++) {
            net.hub().insert(new ItemStack(Items.LEATHER_BOOTS)); // 4 emeralds each: more than the 2-emerald par
        }
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(net.dest().count(new ItemCost(Items.LEATHER_BOOTS, 1)) == 1, "one pair bought");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void caravansBuyATownsSurplusForTheTripHome(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        // Another town on the hub's routes buys carrots: the travelling Merchant has somewhere to sell them.
        var dimension = helper.getLevel().dimension();
        GlobalPos elsewhere = GlobalPos.of(dimension, helper.absolutePos(new BlockPos(3, 2, 6)));
        net.ledger().addRoute(GlobalPos.of(dimension, helper.absolutePos(HUB)), elsewhere);
        MerchantBook.get(helper.getLevel().getServer()).setEnabled(
                elsewhere, helper.absolutePos(new BlockPos(3, 2, 0)), "minecraft:carrot+>minecraft:emerald", true);
        for (int i = 0; i < 4; i++) {
            net.dest().insert(new ItemStack(Items.CARROT, 64)); // nobody there wants carrots: all surplus past food par
        }
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        int destMoney = MoneyChanger.value(net.dest().looseStacks());
        net.poll(t + 80);
        helper.assertTrue(net.route().itemsBought() > 0, "the caravan bought carrots");
        int carrotsAboard = net.ledger().caravan(net.route().id()).orElseThrow().cargo().stream()
                .filter(stack -> stack.is(Items.CARROT)).mapToInt(ItemStack::getCount).sum();
        helper.assertTrue(carrotsAboard > 0 && carrotsAboard <= net.route().itemsBought(),
                "carrots ride home (with any other surplus bought): " + carrotsAboard);
        helper.assertTrue(MoneyChanger.value(net.dest().looseStacks()) > destMoney - net.route().salesSpurs(),
                "the town was paid for its surplus");
        helper.assertTrue(net.dest().foodNutrition() >= dev.b28.enterprise.merchant.Warehouse.FOOD_PAR_POINTS,
                "the town keeps its food par: " + net.dest().foodNutrition());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void caravansDontBuySurplusNobodyWants(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        for (int i = 0; i < 4; i++) {
            net.dest().insert(new ItemStack(Items.CARROT, 64));
        }
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(net.route().itemsBought() == 0, "no town on the hub's routes wants carrots: none bought");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aDestinationThatCantPayGetsNothing(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 0);
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(net.dest().count(PAPER) == 0, "no money, no paper");
        helper.assertTrue(dev.b28.enterprise.cargo.Crates.contents(net.ledger().caravan(net.route().id()).orElseThrow().cargo())
                .stream().filter(stack -> stack.is(Items.PAPER)).mapToInt(ItemStack::getCount).sum() == 64,
                "the paper rides home, crated");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aHubThatCantFundThePurseStaysHome(GameTestHelper helper) {
        Network net = network(helper, 2, 0, 192);
        net.poll(helper.getLevel().getGameTime());
        helper.assertTrue(net.ledger().caravan(net.route().id()).isEmpty(), "no purse, no caravan");
        helper.assertTrue(net.route().status().contains("fund"), "the route says the hub can't fund it");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void noFoodNoCaravan(GameTestHelper helper) {
        Network net = network(helper, 0);
        net.poll(helper.getLevel().getGameTime());
        helper.assertTrue(net.ledger().caravan(net.route().id()).isEmpty(), "without food, nobody leaves");
        helper.assertTrue(net.hub().count(PAPER) == 128, "the goods stay at the hub");
        helper.assertTrue(net.route().status().contains("food"), "the route says it's waiting for food");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void foodCapsALegAt3200Blocks(GameTestHelper helper) {
        helper.assertTrue(Caravans.inRange(3200), "3,200 blocks takes a full stack of bread");
        helper.assertFalse(Caravans.inRange(3210), "a little further is out of range");
        helper.assertTrue(Caravans.travelTicks(1200) == 24000, "1,200 blocks is a day on the road");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void theAdvanceRidesWithTheCopyAndTheInputsFollow(GameTestHelper helper) {
        Network net = network(helper, 8, 20, 192);
        var server = helper.getLevel().getServer();
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        GlobalPos hub = net.route().from();
        GlobalPos town = net.route().to();
        net.hub().insert(new ItemStack(Items.IRON_INGOT, 64));
        long t = helper.getLevel().getGameTime();
        // The hub orders eight iron swords made, two a day from the third day, a third paid up front.
        var contract = book.draft(town, hub, List.of(new dev.b28.enterprise.contract.Delivery(Items.IRON_SWORD, 2, 1.0),
                new dev.b28.enterprise.contract.Advance(1 / 3.0)), dev.b28.enterprise.contract.Incoterm.EXW,
                t + 3 * 24000L, 24000, 4, t, hub, false);
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(contract.inForce(), "the caravan brought the copy");
        helper.assertTrue(Math.abs(contract.advanced() - 8 / 3.0) < 0.01, "and the advance: " + contract.advanced());
        helper.assertTrue(net.route().advanceSpurs() > 0, "in coin, from its purse: " + net.route().advanceSpurs());
        net.poll(t + 160);
        net.poll(t + 24000);
        net.poll(t + 24080);
        int iron = net.dest().count(new ItemCost(Items.IRON_INGOT, 1));
        helper.assertTrue(iron >= 16, "the next caravan brings the iron for the swords: " + iron);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aCaravanDeliversToAMarketOffTheMap(GameTestHelper helper) {
        Network net = network(helper, 8, 5, 192);
        var server = helper.getLevel().getServer();
        GlobalPos hub = net.route().from();
        GlobalPos front = GlobalPos.of(hub.dimension(), hub.pos().offset(64, 0, 0)); // no chest, no villager there
        java.util.UUID money = java.util.UUID.randomUUID();
        dev.ithundxr.createnumismatics.Numismatics.BANK.getOrCreateAccount(money,
                dev.ithundxr.createnumismatics.content.backend.BankAccount.Type.PLAYER).deposit(1000);
        dev.b28.enterprise.merchant.OffMap.get(server).open(front, "the front", money, false);
        var lines = MerchantStock.at(helper.getLevel(), front.pos());
        int before = lines.count(PAPER);
        lines.insert(new ItemStack(Items.BREAD, 20)); // the wagons' food home
        var route = net.ledger().addRoute(hub, front);
        var contract = dev.b28.enterprise.contract.ContractBook.get(server).draft(hub, front,
                new dev.b28.enterprise.contract.Delivery(Items.PAPER, 32, 0.1), dev.b28.enterprise.contract.Incoterm.DAP,
                helper.getLevel().getGameTime(), 24000, 1, helper.getLevel().getGameTime(), hub, true);
        long t = helper.getLevel().getGameTime();
        for (long at = t; at <= t + 4000; at += 100) {
            net.poll(at);
        }
        helper.assertTrue(contract.delivered() == 32 && lines.count(PAPER) - before == 32,
                "delivered into its warehouse on the ledger: " + contract.history());
        helper.assertTrue(dev.ithundxr.createnumismatics.Numismatics.BANK.getAccount(money).getBalance() < 1000,
                "and paid for from its account");
        net.ledger().removeRoute(route.id());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aCaravanGoesHomeOnShortRationsRatherThanWaitForDays(GameTestHelper helper) {
        Network net = network(helper, 8);
        net.dest().take(new ItemCost(Items.BREAD, 1), 64); // the far end has no food to sell
        long t = helper.getLevel().getGameTime();
        for (long at = t; at <= t + 2000; at += 100) {
            net.poll(at);
        }
        var waiting = net.ledger().caravan(net.route().id());
        helper.assertTrue(waiting.isPresent() && waiting.get().leg() == Caravan.Leg.WAITING_FOR_FOOD,
                "waits a while: " + net.route().status());
        for (long at = t + 2000; at <= t + 2000 + Caravans.MAX_WAIT_TICKS + 200; at += 100) {
            net.poll(at);
        }
        helper.assertTrue(net.route().shortRations() == 1, "then goes home hungry: " + net.route().status());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aForwarderTakesCratesBoundOnAndTheirRunLoadsThemFirst(GameTestHelper helper) {
        Network net = network(helper, 2);
        var server = helper.getLevel().getServer();
        GlobalPos shipper = net.route().from();
        GlobalPos hub = net.route().to();
        GlobalPos buyer = GlobalPos.of(hub.dimension(), hub.pos().offset(200, 0, 0)); // further on
        var depot = dev.b28.enterprise.merchant.Depots.get(server).add(helper.getLevel(), hub,
                dev.b28.enterprise.merchant.Depots.Kind.STOREHOUSE, hub.pos().above(3), hub.pos().above(3));
        var onward = CaravanLedger.get(server).addRoute(hub, buyer);
        var freight = dev.b28.enterprise.market.Freight.get(server);
        var bill = dev.b28.enterprise.cargo.Papers.billOfLading(0, shipper, buyer, "64 paper", "DAP", 0, 0);
        List<ItemStack> cargo = new java.util.ArrayList<>(List.of(dev.b28.enterprise.cargo.Crates.of(
                List.of(new ItemStack(Items.PAPER, 64)), dev.b28.enterprise.cargo.Papers.NONE.with(bill))));
        int[] purse = {1000};
        helper.assertTrue(freight.handOver(helper.getLevel(), hub, net.dest(), cargo, purse) == 0 && cargo.size() == 1
                && purse[0] == 1000, "no draymen to cart it in: it stays aboard, nothing paid");
        var leg = new dev.b28.enterprise.market.Freight.Leg(onward, true);
        net.dest().insert(dev.b28.enterprise.market.Freight.book(cargo.get(0), leg, 1));
        var load = freight.loadBooked(hub, net.dest(), leg, 27);
        helper.assertTrue(load.size() == 1 && dev.b28.enterprise.market.Freight.booked(load.get(0), leg) == 1
                && dev.b28.enterprise.cargo.Crates.consignedTo(load.get(0), buyer), "booked, it loads for its run");
        CaravanLedger.get(server).removeRoute(onward.id());
        dev.b28.enterprise.merchant.Depots.get(server).remove(depot.id());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aRequisitionTakesTheBestOfferForTheWholeLot(GameTestHelper helper) {
        Network net = network(helper, 2);
        var server = helper.getLevel().getServer();
        GlobalPos hub = net.route().from();
        GlobalPos dest = net.route().to();
        long t = helper.getLevel().getGameTime();
        for (int i = 0; i < 4; i++) {
            net.dest().insert(new ItemStack(Items.BREAD, 64)); // beyond what its own caravans eat
        }
        dev.b28.enterprise.merchant.Warehouse.get(server).recordStocktake(dest, net.dest().totals(), t);
        var road = CaravanLedger.get(server).addRoute(hub, dest);
        var requisitions = dev.b28.enterprise.market.Requisitions.get(server);
        var wanted = requisitions.post(helper.getLevel(), hub, List.of(Items.BREAD), false, 40, 1.5, null, t);
        helper.assertTrue(requisitions.read(helper.getLevel(), dest, hub, t) == 1, "the town's Merchant bids");
        helper.assertTrue(requisitions.read(helper.getLevel(), dest, hub, t) == 0, "once");
        var bid = wanted.bids().get(0);
        helper.assertTrue(bid.each() < wanted.ceiling(Items.BREAD), "under the crown's price: " + bid.each());
        helper.assertTrue(requisitions.award(helper.getLevel(), hub, dev.b28.enterprise.army.Army.Theatre.FACTION, t) == 0,
                "not while bids are taken");
        long closed = t + dev.b28.enterprise.market.Requisitions.BID_DAYS * 24000L;
        helper.assertTrue(requisitions.award(helper.getLevel(), hub, dev.b28.enterprise.army.Army.Theatre.FACTION, closed) == 1,
                "the best offer taken");
        var contract = dev.b28.enterprise.contract.ContractBook.get(server)
                .open(dest, hub, new dev.b28.enterprise.contract.Delivery(Items.BREAD, 1, 0)).get(0);
        helper.assertTrue(contract.installments() == 1 && contract.delivery().get().quantity() == bid.count()
                && wanted.open() == 40 - bid.count(), "one contract for the lot: " + bid.count() + ", open " + wanted.open());
        CaravanLedger.get(server).removeRoute(road.id());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aCaravanBringsMercenariesHiredOverTheSea(GameTestHelper helper) {
        Network net = network(helper, 8, 5, 192);
        var server = helper.getLevel().getServer();
        GlobalPos hub = net.route().from();
        GlobalPos sea = GlobalPos.of(hub.dimension(), hub.pos().offset(-64, 0, 0));
        java.util.UUID money = java.util.UUID.randomUUID();
        dev.ithundxr.createnumismatics.Numismatics.BANK.getOrCreateAccount(money,
                dev.ithundxr.createnumismatics.content.backend.BankAccount.Type.PLAYER);
        dev.b28.enterprise.merchant.OffMap.get(server).open(sea, "the market across the sea", money, true);
        MerchantStock.at(helper.getLevel(), sea.pos()).insert(new ItemStack(Items.BREAD, 20)); // the wagons' food home
        var route = net.ledger().addRoute(hub, sea);
        long t = helper.getLevel().getGameTime();
        var hire = dev.b28.enterprise.contract.ContractBook.get(server).draft(sea, hub,
                List.of(new dev.b28.enterprise.contract.Levy(3, 1.0)), dev.b28.enterprise.contract.Incoterm.EXW, t,
                24000, 1, t, hub, true);
        var army = dev.b28.enterprise.army.Army.get(server);
        int before = army.waiting();
        for (long at = t; at <= t + 8000; at += 100) {
            net.poll(at);
        }
        helper.assertTrue(hire.delivered() == 3, "three hired: " + hire.history() + " " + route.status() + " "
                + net.ledger().caravan(route.id()).map(c -> c.toString()).orElse("home"));
        helper.assertTrue(army.waiting() - before >= 3, "and waiting in the barracks: " + (army.waiting() - before));
        army.embark(3);
        net.ledger().removeRoute(route.id());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void bothMerchantsKeepBooksOfATrip(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 640);
        var server = helper.getLevel().getServer();
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        net.poll(t + 80);
        net.poll(t + 160);
        var books = dev.b28.enterprise.merchant.Books.get(server);
        var home = books.of(net.route().from());
        var there = books.of(net.route().to());
        helper.assertTrue(home.stream().anyMatch(e -> e.kind() == dev.b28.enterprise.merchant.Books.Kind.SENT
                && e.item() == Items.PAPER), "home's books: paper sent out " + home);
        helper.assertTrue(home.stream().anyMatch(e -> e.kind() == dev.b28.enterprise.merchant.Books.Kind.SOLD
                && e.item() == Items.PAPER && e.spurs() > 0 && net.route().to().equals(e.with())),
                "and, from the caravan's log, paper sold there: " + home);
        helper.assertTrue(there.stream().anyMatch(e -> e.kind() == dev.b28.enterprise.merchant.Books.Kind.BOUGHT
                && e.item() == Items.PAPER && e.spurs() > 0), "the far Merchant's: paper bought " + there);
        helper.assertTrue(books.lastPrice(net.route().from(), net.route().to(), Items.PAPER).isPresent(),
                "home knows what paper fetched there");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aMerchantBuysToSellOnOnlyAtAMargin(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        var outlet = new dev.b28.enterprise.caravan.Desires.Outlet(net.route().to(), 0.5, 40);
        helper.assertTrue(dev.b28.enterprise.caravan.Desires.worthBuying(0.40, outlet), "0.40 against 0.50 there: worth it");
        helper.assertFalse(dev.b28.enterprise.caravan.Desires.worthBuying(0.45, outlet),
                "0.45 leaves less than the 15% a Merchant wants: no");
        double haul = dev.b28.enterprise.caravan.Desires.haulEach(net.route(), Items.COOKIE);
        helper.assertTrue(haul > 0 && haul < 0.01, "the road costs a cookie a fraction of a spur's worth: " + haul);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aForeignRealmSendsItsOwnTraders(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 640);
        var server = helper.getLevel().getServer();
        GlobalPos dest = net.route().to();
        // A locker of the town's buys leather: the March has it.
        MerchantBook.get(server).setEnabled(dest, helper.absolutePos(new BlockPos(5, 2, 6)),
                "minecraft:leather+>minecraft:emerald", true);
        var realms = dev.b28.enterprise.trade.Realms.get(server);
        var realm = realms.open(helper.getLevel(), "march", dest.pos().offset(64, 0, 0), 2000, List.of(dest));
        var ledger = CaravanLedger.get(server);
        var road = ledger.route(realm.home(), dest).orElseThrow();
        long t = helper.getLevel().getGameTime();
        for (long at = t; at <= t + 6000 && road.trips() == 0; at += 100) {
            Caravans.poll(server, ledger, net.territory(), at);
        }
        helper.assertTrue(net.dest().count(new ItemCost(Items.LEATHER, 1)) > 0, "the March's caravan sold its leather here");
        helper.assertTrue(road.trips() >= 1, "and went home: " + road.trips() + " trips, " + road.status());
        helper.assertTrue(road.salesSpurs() > 0, "paid for: " + road.salesSpurs() + " spurs");
        ledger.removeRoute(road.id());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aCaravanFetchesHomeCratesMadeOutToIt(GameTestHelper helper) {
        Network net = network(helper, 4, 5, 192);
        GlobalPos hub = net.route().from();
        GlobalPos town = net.route().to();
        var bill = dev.b28.enterprise.cargo.Papers.billOfLading(0, town, hub, "9 iron_axe", "EXW", 0, 0);
        bill.data().putBoolean("settled", true);
        List<ItemStack> axes = new java.util.ArrayList<>();
        for (int i = 0; i < 9; i++) {
            axes.add(new ItemStack(Items.IRON_AXE));
        }
        net.dest().insert(dev.b28.enterprise.cargo.Crates.of(axes, dev.b28.enterprise.cargo.Papers.NONE.with(bill)));
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        net.poll(t + 80);
        net.poll(t + 160);
        helper.assertTrue(net.hub().count(new ItemCost(Items.IRON_AXE, 1)) == 9, "home and opened: "
                + net.hub().count(new ItemCost(Items.IRON_AXE, 1)));
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void goodsForSaleTravelCratedAndAreStoredAsTheyCome(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 640);
        // The destination's Librarian takes plenty of paper: it wants the whole lot.
        dev.b28.enterprise.merchant.Warehouse.get(helper.getLevel().getServer()).recordCapacity(net.route().to(),
                helper.absolutePos(new BlockPos(5, 2, 6)).asLong(), java.util.Map.of(Items.PAPER, 200));
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        var out = net.ledger().caravan(net.route().id()).orElseThrow();
        helper.assertTrue(out.cargo().stream().anyMatch(s -> dev.b28.enterprise.cargo.Crates.lot(s)
                        .filter(g -> g.is(Items.PAPER) && g.getCount() == 128).isPresent()),
                "the paper goes in a crate of its own");
        net.poll(t + 80);
        helper.assertTrue(net.dest().count(PAPER) == 128, "sold, all of it: " + net.dest().count(PAPER));
        helper.assertTrue(net.dest().looseStacks().stream().anyMatch(dev.b28.enterprise.cargo.Crates::isCrate)
                        && net.dest().looseStacks().stream().noneMatch(s -> s.is(Items.PAPER)),
                "stored as it came, in its crate: one slot, not two");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aBankBuysATownsBillForCoinNow(GameTestHelper helper) {
        Network net = network(helper, 2, 40, 0);
        var server = helper.getLevel().getServer();
        GlobalPos hub = net.route().from();
        GlobalPos town = net.route().to();
        var bank = dev.b28.enterprise.market.Bank.get(server);
        bank.open(hub);
        long t = helper.getLevel().getGameTime();
        var bills = dev.b28.enterprise.contract.Bills.get(server);
        var paper = bills.draw(hub, town, 10, t); // the hub owes the town ten emeralds, in four days
        dev.b28.enterprise.contract.Bills.file(net.dest(), paper);
        int id = paper.data().getInt("bill");
        net.poll(t);
        net.poll(t + 80);
        var bill = bills.bill(id).orElseThrow();
        bank.close(hub);
        helper.assertTrue(bill.state() == dev.b28.enterprise.contract.Bills.State.PAID, "the hub's bank bought it back early");
        int coins = dev.b28.enterprise.money.MoneyChanger.value(net.dest().looseStacks());
        helper.assertTrue(coins > 0, "and the town has coin now, not in four days: " + coins);
        helper.assertTrue(bank.book(hub).billsPaid() < 10 * 1.02, "less the discount: " + bank.book(hub).billsPaid());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aBankLendsAgainstAContractAndIsRepaidFromThePayment(GameTestHelper helper) {
        Network net = network(helper, 8, 40, 0); // a town with no money
        var server = helper.getLevel().getServer();
        GlobalPos hub = net.route().from();
        GlobalPos town = net.route().to();
        var bank = dev.b28.enterprise.market.Bank.get(server);
        bank.open(hub);
        long t = helper.getLevel().getGameTime();
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        // The hub buys 64 books from the town, collected in two days; the town has the paper, and nothing to pay with.
        var contract = book.draft(town, hub, new dev.b28.enterprise.contract.Delivery(Items.BOOK, 64, 0.5),
                dev.b28.enterprise.contract.Incoterm.EXW, t + 2 * 24000L, 24000, 1, t, town, true);
        net.poll(t);
        net.poll(t + 80);
        double lent = bank.owedOn(contract.id());
        helper.assertTrue(lent > 5, "lent against the books to come: " + lent);
        net.poll(t + 160);
        net.dest().insert(new ItemStack(Items.BOOK, 64)); // made, with the money
        net.poll(t + 2 * 24000L);
        net.poll(t + 2 * 24000L + 80);
        double owed = bank.owedOn(contract.id());
        bank.close(hub);
        helper.assertTrue(contract.delivered() == 64, "collected: " + contract.history());
        helper.assertTrue(owed < 0.01 && bank.book(hub).repaid() > 5, "the loan came out of the payment: " + owed);
        helper.succeed();
    }
}
