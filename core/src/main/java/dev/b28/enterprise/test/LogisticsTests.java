package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.army.Army;
import dev.b28.enterprise.contract.Advance;
import dev.b28.enterprise.contract.Contract;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.contract.Incoterm;
import dev.b28.enterprise.contract.Training;
import dev.b28.enterprise.market.Tenders;
import dev.b28.enterprise.merchant.Depots;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.OffMap;
import dev.b28.enterprise.money.MoneyChanger;
import dev.b28.enterprise.settlement.Census;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Territory;
import dev.ithundxr.createnumismatics.Numismatics;
import dev.ithundxr.createnumismatics.content.backend.BankAccount;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/** Off-map markets, buildings as stores, whole armour, training contracts and tenders. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class LogisticsTests {
    private static GlobalPos far(GameTestHelper helper, int x, int z) {
        return GlobalPos.of(helper.getLevel().dimension(), new BlockPos(x, 64, z));
    }

    private static UUID account(int spurs) {
        UUID id = UUID.randomUUID();
        Numismatics.BANK.getOrCreateAccount(id, BankAccount.Type.PLAYER).deposit(spurs);
        return id;
    }

    @GameTest(template = "floor")
    public static void anOffMapMarketKeepsItsWarehouseOnTheLedger(GameTestHelper helper) {
        var level = helper.getLevel();
        GlobalPos front = far(helper, 9_000_000, 9_000_000);
        UUID money = account(500);
        OffMap.get(level.getServer()).open(front, "the front", money, false);
        MerchantStock stock = MerchantStock.at(level, front.pos());
        int before = stock.count(new ItemCost(Items.BREAD, 1)); // the test world keeps its saved data between runs
        stock.insert(new ItemStack(Items.BREAD, 40));
        helper.assertTrue(MerchantStock.at(level, front.pos()).count(new ItemCost(Items.BREAD, 1)) == before + 40,
                "its warehouse holds what it's given, with no chest there");
        helper.assertTrue(MoneyChanger.bankAt(level, front.pos()).map(BankAccount::getBalance).orElse(0) == 500,
                "and its money is in its account");
        var registries = level.registryAccess();
        var reloaded = OffMap.load(OffMap.get(level.getServer()).save(new CompoundTag(), registries), registries);
        helper.assertTrue(reloaded.is(front), "saved and loaded");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aBuildingsChestsAreOneStoreAndHoldWhatItsFor(GameTestHelper helper) {
        var level = helper.getLevel();
        BlockPos desk = new BlockPos(1, 2, 1);
        helper.setBlock(desk, Merchant.DESK.get());
        helper.setBlock(desk.east(), Blocks.CHEST);
        helper.setBlock(new BlockPos(5, 2, 5), Blocks.CHEST); // the armoury: two chests, apart
        helper.setBlock(new BlockPos(5, 2, 3), Blocks.CHEST);
        GlobalPos at = GlobalPos.of(level.dimension(), helper.absolutePos(desk));
        var depots = Depots.get(level.getServer());
        var armoury = depots.add(level, at, Depots.Kind.ARMOURY, helper.absolutePos(new BlockPos(4, 2, 3)),
                helper.absolutePos(new BlockPos(6, 3, 5)));
        helper.assertTrue(armoury.containers(level).size() == 2, "every chest in its floorplan");
        MerchantStock stock = MerchantStock.at(level, helper.absolutePos(desk));
        stock.insert(new ItemStack(Items.IRON_SWORD));
        stock.insert(new ItemStack(Items.PAPER, 10));
        var deskChest = net.minecraft.world.level.block.entity.ChestBlockEntity.class.cast(
                level.getBlockEntity(helper.absolutePos(desk.east())));
        helper.assertTrue(deskChest.countItem(Items.IRON_SWORD) == 0 && deskChest.countItem(Items.PAPER) == 10,
                "war goods to the armoury, paper to the desk's chest");
        helper.assertTrue(stock.count(new ItemCost(Items.IRON_SWORD, 1)) == 1, "and the stock counts both");
        for (int i = 0; i < 60; i++) {
            stock.insert(new ItemStack(Items.PAPER, 64)); // more paper than the desk's chest holds
        }
        var armouryChest = net.minecraft.world.level.block.entity.ChestBlockEntity.class.cast(
                level.getBlockEntity(helper.absolutePos(new BlockPos(5, 2, 5))));
        helper.assertTrue(armouryChest.countItem(Items.PAPER) == 0, "an armoury never takes goods it isn't for");
        depots.remove(armoury.id());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void soldiersAreSafestInWholeSets(GameTestHelper helper) {
        var theatre = Army.Theatre.FACTION;
        double chest = Army.risk(new ItemStack(Items.IRON_SWORD), new ItemStack(Items.IRON_CHESTPLATE), ItemStack.EMPTY,
                theatre);
        double set = Army.risk(new ItemStack(Items.IRON_SWORD), List.of(new ItemStack(Items.IRON_HELMET),
                new ItemStack(Items.IRON_CHESTPLATE), new ItemStack(Items.IRON_LEGGINGS), new ItemStack(Items.IRON_BOOTS)),
                ItemStack.EMPTY, theatre);
        double diamond = Army.risk(new ItemStack(Items.IRON_SWORD), List.of(new ItemStack(Items.DIAMOND_HELMET),
                new ItemStack(Items.DIAMOND_CHESTPLATE), new ItemStack(Items.DIAMOND_LEGGINGS),
                new ItemStack(Items.DIAMOND_BOOTS)), ItemStack.EMPTY, theatre);
        helper.assertTrue(chest > set && set > diamond, "chestplate %.4f > iron set %.4f > diamond set %.4f"
                .formatted(chest, set, diamond));
        helper.assertTrue(Math.abs(diamond / (Army.DEATH_BASE * 1.0) - 0.5) < 1e-9, "a full diamond set halves the risk");
        helper.assertTrue(new Army().wants().containsKey(Items.IRON_BOOTS), "the quartermaster wants boots too");
        helper.succeed();
    }

    /** A settlement in the arena: its desk, its bell, and a roster of these trades at these levels. */
    private static GlobalPos town(GameTestHelper helper, Map<String, Integer> trades) {
        var level = helper.getLevel();
        BlockPos desk = new BlockPos(3, 2, 3);
        helper.setBlock(desk, Merchant.DESK.get());
        helper.setBlock(desk.east(), Blocks.CHEST); // its warehouse, with money
        MerchantStock.at(level, helper.absolutePos(desk)).insert(new ItemStack(Items.EMERALD, 20));
        BlockPos bell = helper.absolutePos(new BlockPos(3, 2, 5));
        Territory.get(level.getServer()).afterCensus(level, bell, new Census(5, 5, 5, 5, 0), 1, List.of());
        Map<UUID, Settlements.Resident> roster = new java.util.LinkedHashMap<>();
        trades.forEach((trade, lvl) -> {
            UUID id = UUID.randomUUID();
            roster.put(id, new Settlements.Resident(id, 0, Optional.empty(), trade, lvl, true, false, 0, false));
        });
        Settlements.get(level.getServer()).setRoster(GlobalPos.of(level.dimension(), bell), roster);
        return GlobalPos.of(level.dimension(), helper.absolutePos(desk));
    }

    @GameTest(template = "floor")
    public static void aTrainingContractIsPaidWhenTheCensusShowsTheMaster(GameTestHelper helper) {
        var server = helper.getLevel().getServer();
        GlobalPos town = town(helper, Map.of("minecraft:weaponsmith", 3));
        GlobalPos capital = far(helper, 9_100_000, 9_000_000);
        ContractBook book = new ContractBook();
        long t = helper.getLevel().getGameTime();
        Contract c = book.draft(town, capital, List.of(new Training("minecraft:weaponsmith", 5, 24), new Advance(1 / 3.0)),
                Incoterm.EXW, t + 10 * 24000L, 24000, 1, t, town, true);
        helper.assertTrue(Math.abs(c.advanceDue() - 8) < 1e-9, "a third of the bonus up front: " + c.advanceDue());
        book.advanced(c, 8, t);
        book.review(server, t + 24000);
        helper.assertTrue(c.inForce(), "a Journeyman yet: under way");
        var bell = Territory.get(server).bellOf(town).orElseThrow();
        var roster = new java.util.LinkedHashMap<>(Settlements.get(server).roster(bell));
        roster.replaceAll((id, r) -> new Settlements.Resident(id, 0, Optional.empty(), r.profession(), 5, true, false, 0, false));
        Settlements.get(server).setRoster(bell, roster);
        book.review(server, t + 2 * 24000L);
        helper.assertTrue(c.state() == Contract.State.COMPLETED && Math.abs(c.owing() - 16) < 1e-9,
                "a master: done, the rest of the bonus owed: " + c.owing());
        Contract another = book.draft(town, capital, List.of(new Training("minecraft:weaponsmith", 5, 24)
                        .forSeller(ContractBook.at(server, town, "minecraft:weaponsmith", 5))),
                Incoterm.EXW, t + 10 * 24000L, 24000, 1, t, town, true);
        book.review(server, t + 3 * 24000L);
        helper.assertTrue(another.inForce(), "a second contract wants a second master, not the one it has");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aMerchantTakesATenderWorthItsWhile(GameTestHelper helper) {
        var level = helper.getLevel();
        GlobalPos town = town(helper, Map.of("minecraft:weaponsmith", 2));
        GlobalPos capital = far(helper, 9_200_000, 9_000_000);
        long t = level.getGameTime();
        Tenders tenders = new Tenders();
        var cheap = tenders.post(capital, capital, new Training("minecraft:weaponsmith", 5, 3), 1, t + 20 * 24000L);
        helper.assertTrue(Tenders.worth(level, town, cheap, t).map(v -> v < 0).orElse(false),
                "three emeralds won't pay for three levels of coal and iron");
        var rushed = tenders.post(capital, capital, new Training("minecraft:weaponsmith", 5, 40), 1, t + 3 * 24000L);
        helper.assertTrue(Tenders.worth(level, town, rushed, t).isEmpty(), "and three days is too soon");
        var fair = tenders.post(capital, capital, new Training("minecraft:weaponsmith", 5, 40), 1, t + 20 * 24000L);
        helper.assertTrue(Tenders.worth(level, town, fair, t).map(v -> v > 0).orElse(false), "forty in twenty days pays");
        helper.assertTrue(tenders.read(level, town, capital, t) == 1 && fair.open() == 0 && cheap.open() == 1,
                "the town's Merchant takes the one that pays, and leaves the others");
        var contract = ContractBook.get(level.getServer()).of(town).stream()
                .filter(c -> c.seller().equals(town) && c.training().isPresent()).findFirst().orElseThrow();
        helper.assertTrue(contract.buyer().equals(capital) && contract.knownBy(town) && !contract.knownBy(capital),
                "a contract with the buyer, its copy to ride home with the caravan");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aMerchantWithNoOneToTrainCarriesTheTenderOn(GameTestHelper helper) {
        var server = helper.getLevel().getServer();
        GlobalPos hub = far(helper, 9_300_000, 9_000_000); // a market with a road to a town of its own
        GlobalPos town = town(helper, Map.of("minecraft:weaponsmith", 2));
        GlobalPos capital = far(helper, 9_300_600, 9_000_000);
        var ledger = dev.b28.enterprise.caravan.CaravanLedger.get(server);
        var route = ledger.addRoute(hub, town);
        long t = helper.getLevel().getGameTime();
        Tenders tenders = new Tenders();
        var tender = tenders.post(capital, capital, new Training("minecraft:weaponsmith", 5, 40), 1, t + 20 * 24000L);
        boolean carried = tenders.read(helper.getLevel(), hub, capital, t) == 1;
        ledger.removeRoute(route.id());
        helper.assertTrue(carried && tender.open() == 0, "the hub can't train, but carries it on");
        var onward = tenders.on(hub);
        helper.assertTrue(onward.size() == 1 && onward.get(0).work().bonus() < 40 && onward.get(0).due() < tender.due(),
                "posted on its own board, for a little less, a little sooner");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void theExchangeBuysMetalFromTheNearestHolderFirst(GameTestHelper helper) {
        var level = helper.getLevel();
        var server = level.getServer();
        GlobalPos capital = far(helper, 9_500_000, 9_000_000);
        GlobalPos mine = far(helper, 9_500_300, 9_000_000); // a town with a mine, near
        GlobalPos overseas = far(helper, 9_497_600, 9_000_000); // across the sea, far
        var warehouse = dev.b28.enterprise.merchant.Warehouse.get(server);
        warehouse.recordStocktake(mine, Map.of("minecraft:iron_ingot", 40), 0);
        warehouse.recordStocktake(overseas, Map.of("minecraft:iron_ingot", 512), 0);
        var atlas = dev.b28.enterprise.caravan.Atlas.get(server);
        atlas.learn(capital, mine, 0);
        atlas.learn(capital, overseas, 0);
        var ledger = dev.b28.enterprise.caravan.CaravanLedger.get(server);
        var toMine = ledger.addRoute(capital, mine);
        var toSea = ledger.addRoute(capital, overseas);
        var exchange = new dev.b28.enterprise.market.MetalExchange();
        var iron = dev.b28.enterprise.market.MetalExchange.metal(Items.IRON_INGOT).orElseThrow();
        double premium = exchange.premium(iron, dev.b28.enterprise.market.Sovereign.Phase.PEACE);
        helper.assertTrue(Math.abs(premium - 1.5) < 1e-9, "holding none, it pays half again: " + premium);
        long t = level.getGameTime();
        dev.b28.enterprise.market.Broker.source(level, capital, new Territory(), Items.IRON_INGOT, 100, t, premium, List.of());
        var book = ContractBook.get(server);
        int fromMine = book.of(capital).stream().filter(c -> c.seller().equals(mine))
                .mapToInt(c -> c.delivery().get().quantity() * c.installments()).sum();
        int fromSea = book.of(capital).stream().filter(c -> c.seller().equals(overseas))
                .mapToInt(c -> c.delivery().get().quantity() * c.installments()).sum();
        ledger.removeRoute(toMine.id());
        ledger.removeRoute(toSea.id());
        helper.assertTrue(fromMine == 40 && fromSea == 60, "the mine's forty first, the rest from overseas: %d and %d"
                .formatted(fromMine, fromSea));
        exchange.setHeldForTest(Items.IRON_INGOT, 192);
        helper.assertTrue(Math.abs(exchange.premium(iron, dev.b28.enterprise.market.Sovereign.Phase.PEACE) - 1) < 1e-9,
                "holding its target, its worth");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aStorageCrateHoldsNineStacksInASlotAndTheStockReachesIn(GameTestHelper helper) {
        var level = helper.getLevel();
        BlockPos desk = new BlockPos(1, 2, 1);
        helper.setBlock(desk, Merchant.DESK.get());
        helper.setBlock(desk.east(), Blocks.CHEST);
        MerchantStock stock = MerchantStock.at(level, helper.absolutePos(desk));
        List<ItemStack> axes = new java.util.ArrayList<>();
        for (int i = 0; i < 9; i++) {
            axes.add(new ItemStack(Items.IRON_AXE));
        }
        stock.insert(dev.b28.enterprise.cargo.Crates.of(axes, dev.b28.enterprise.cargo.Papers.NONE));
        helper.assertTrue(stock.count(new ItemCost(Items.IRON_AXE, 1)) == 9 && stock.freeSlots() == 26,
                "nine axes, one slot: " + stock.freeSlots());
        stock.take(new ItemCost(Items.IRON_AXE, 1), 9);
        helper.assertTrue(stock.count(new ItemCost(dev.b28.enterprise.cargo.Cargo.CRATE_ITEM.get(), 1)) == 1
                && dev.b28.enterprise.cargo.Crates.empty(stock.looseStacks().stream()
                        .filter(dev.b28.enterprise.cargo.Crates::isCrate).findFirst().orElseThrow()),
                "taken out, an empty crate is left to ship with");
        var shipment = dev.b28.enterprise.cargo.Crates.of(List.of(new ItemStack(Items.IRON_AXE)),
                dev.b28.enterprise.cargo.Papers.NONE.with(dev.b28.enterprise.cargo.Papers.billOfLading(1,
                        far(helper, 9_600_000, 9_000_000), far(helper, 9_600_300, 9_000_000), "1 iron_axe", "EXW", 0, 0)));
        stock.insert(shipment);
        helper.assertTrue(stock.count(new ItemCost(Items.IRON_AXE, 1)) == 0, "a crate with a bill of lading is someone's");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void shortOfRoomTheMerchantRepacksCratesPickedAt(GameTestHelper helper) {
        var level = helper.getLevel();
        BlockPos desk = new BlockPos(1, 2, 1);
        helper.setBlock(desk, Merchant.DESK.get());
        helper.setBlock(desk.east(), Blocks.CHEST);
        GlobalPos at = GlobalPos.of(level.dimension(), helper.absolutePos(desk));
        MerchantStock stock = MerchantStock.at(level, helper.absolutePos(desk));
        // Crates packed long ago, picked at since: a poppy and a seed or two in each, a slot each.
        for (int i = 0; i < 22; i++) {
            stock.insert(dev.b28.enterprise.cargo.Crates.of(List.of(new ItemStack(Items.POPPY),
                    new ItemStack(Items.WHEAT_SEEDS, 2)), dev.b28.enterprise.cargo.Papers.NONE));
        }
        helper.assertTrue(stock.freeSlots() == 5, "the chest full of thin crates: " + stock.freeSlots());
        dev.b28.enterprise.market.ColdStore.tidy(level, at, stock, level.getGameTime());
        MerchantStock after = MerchantStock.at(level, helper.absolutePos(desk));
        helper.assertTrue(after.freeSlots() >= 20, "repacked into one crate, the empties stacked: " + after.freeSlots());
        helper.assertTrue(after.count(new ItemCost(Items.POPPY, 1)) == 22
                && after.count(new ItemCost(Items.WHEAT_SEEDS, 1)) == 44, "every poppy and seed still stock");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void shortOfRoomTheMerchantPacksIdleGoodsIntoColdStorage(GameTestHelper helper) {
        var level = helper.getLevel();
        BlockPos desk = new BlockPos(1, 2, 1);
        helper.setBlock(desk, Merchant.DESK.get());
        helper.setBlock(desk.east(), Blocks.CHEST);
        helper.setBlock(new BlockPos(5, 2, 5), Blocks.CHEST); // its cold store, already set up
        GlobalPos at = GlobalPos.of(level.dimension(), helper.absolutePos(desk));
        var cold = Depots.get(level.getServer()).add(level, at, Depots.Kind.COLD_STORE,
                helper.absolutePos(new BlockPos(5, 2, 5)), helper.absolutePos(new BlockPos(5, 2, 5)));
        MerchantStock stock = MerchantStock.at(level, helper.absolutePos(desk));
        stock.insert(new ItemStack(Items.EMERALD, 10)); // to pay for crates
        for (int i = 0; i < 24; i++) {
            stock.insert(new ItemStack(Items.IRON_AXE)); // a Novice's paid work, a slot each
        }
        int before = stock.freeSlots();
        dev.b28.enterprise.market.ColdStore.tidy(level, at, stock, level.getGameTime());
        MerchantStock after = MerchantStock.at(level, helper.absolutePos(desk));
        var coldChest = net.minecraft.world.level.block.entity.ChestBlockEntity.class.cast(
                level.getBlockEntity(helper.absolutePos(new BlockPos(5, 2, 5))));
        helper.assertTrue(after.freeSlots() > before + 15, "room again: " + before + " -> " + after.freeSlots());
        helper.assertTrue(after.count(new ItemCost(Items.IRON_AXE, 1)) == 24, "every axe still there, and still stock");
        helper.assertTrue(coldChest.countItem(dev.b28.enterprise.cargo.Cargo.CRATE_ITEM.get()) == 3,
                "in three crates, in cold storage: " + coldChest.countItem(dev.b28.enterprise.cargo.Cargo.CRATE_ITEM.get()));
        Depots.get(level.getServer()).remove(cold.id());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aMerchantShortOfLockersTendsTheOnesItsContractsNeedFirst(GameTestHelper helper) {
        var level = helper.getLevel();
        BlockPos desk = new BlockPos(3, 2, 3);
        helper.setBlock(desk, Merchant.DESK.get());
        BlockPos lectern = new BlockPos(2, 2, 3);
        BlockPos composter = new BlockPos(4, 2, 3);
        BlockPos grindstone = new BlockPos(6, 2, 6); // the Weaponsmith's, furthest away
        for (BlockPos site : List.of(lectern, composter, grindstone)) {
            helper.setBlock(site, site == lectern ? Blocks.LECTERN : site == composter ? Blocks.COMPOSTER : Blocks.GRINDSTONE);
            helper.setBlock(site.north(), Blocks.CHEST);
        }
        GlobalPos at = GlobalPos.of(level.dimension(), helper.absolutePos(desk));
        var merchants = dev.b28.enterprise.merchant.MerchantBook.get(level.getServer());
        merchants.setEnabled(at, helper.absolutePos(lectern), "minecraft:paper+>minecraft:emerald", true);
        merchants.setEnabled(at, helper.absolutePos(composter), "minecraft:wheat+>minecraft:emerald", true);
        var nearest = dev.b28.enterprise.locker.Lockers.near(level, helper.absolutePos(desk), 16, 2);
        helper.assertFalse(nearest.stream().anyMatch(l -> l.workstation().equals(helper.absolutePos(grindstone))),
                "a Novice Merchant's two lockers: the nearest with trades on");
        ContractBook.get(level.getServer()).draft(at, far(helper, 9_800_000, 9_000_000),
                List.of(new Training("minecraft:weaponsmith", 5, 24)), Incoterm.EXW, level.getGameTime() + 240000, 24000,
                1, level.getGameTime(), at, true);
        var needed = dev.b28.enterprise.locker.Lockers.near(level, helper.absolutePos(desk), 16, 2);
        helper.assertTrue(needed.stream().anyMatch(l -> l.workstation().equals(helper.absolutePos(grindstone))),
                "contracted to train a Weaponsmith: its locker comes first");
        merchants.setEnabled(at, helper.absolutePos(lectern), "minecraft:paper+>minecraft:emerald", false);
        merchants.setEnabled(at, helper.absolutePos(composter), "minecraft:wheat+>minecraft:emerald", false);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aMerchantBuysForWhatsDueSoonNotTheWholeContract(GameTestHelper helper) {
        var level = helper.getLevel();
        BlockPos desk = new BlockPos(3, 2, 3);
        helper.setBlock(desk, Merchant.DESK.get());
        helper.setBlock(desk.east(), Blocks.CHEST);
        GlobalPos at = GlobalPos.of(level.dimension(), helper.absolutePos(desk));
        long t = level.getGameTime();
        ContractBook.get(level.getServer()).draft(at, far(helper, 9_900_000, 9_000_000),
                new dev.b28.enterprise.contract.Delivery(Items.DIAMOND_SWORD, 1, 30), Incoterm.EXW, t, 24000, 10, t, at, true);
        var stock = MerchantStock.at(level, helper.absolutePos(desk));
        int soon = dev.b28.enterprise.market.Procurement.soon(level, at, stock).getOrDefault(Items.DIAMOND_SWORD, 0);
        int all = dev.b28.enterprise.market.Procurement.wanted(level, at, stock).getOrDefault(Items.DIAMOND_SWORD, 0);
        helper.assertTrue(soon == 3 && all == 10, "three due within two days, of ten in all: " + soon + " of " + all);
        helper.succeed();
    }
}
