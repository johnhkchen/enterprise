package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantBook;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.NonNullList;
import net.minecraft.core.component.DataComponents;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.item.trading.MerchantOffer;
import net.minecraft.world.item.component.ItemContainerContents;
import net.minecraft.world.item.trading.MerchantOffers;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;
import net.neoforged.neoforge.items.IItemHandler;
import net.neoforged.neoforge.items.ItemHandlerHelper;

/**
 * A librarian with a locker chest, a Merchant at a desk, and one trade: 24 paper for 1 emerald, 16 uses, 2 XP,
 * price multiplier 0.05. Restocks are called directly.
 */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class LockerTradingTests {
    private static final BlockPos DESK = new BlockPos(3, 2, 3);
    private static final BlockPos LECTERN = new BlockPos(1, 2, 1);
    private static final BlockPos CHEST = new BlockPos(0, 2, 1);
    private static final String PAPER_TRADE = "minecraft:paper+>minecraft:emerald";

    private record Market(GameTestHelper helper, Villager librarian, Optional<Villager> merchant) {
        boolean ready() {
            return claimed(librarian, LECTERN) && merchant.map(m -> claimed(m, DESK)).orElse(true);
        }

        private boolean claimed(Villager villager, BlockPos site) {
            return villager.getBrain()
                    .getMemory(MemoryModuleType.JOB_SITE)
                    .map(GlobalPos::pos)
                    .filter(helper.absolutePos(site)::equals)
                    .isPresent();
        }

        MerchantOffer givePaperTrade(int demand) {
            MerchantOffer offer = new MerchantOffer(
                    new ItemCost(Items.PAPER, 24), Optional.empty(), new ItemStack(Items.EMERALD), 0, 16, 2, 0.05F, demand);
            MerchantOffers offers = new MerchantOffers();
            offers.add(offer);
            librarian.setOffers(offers);
            return offer;
        }

        /** Also switches on a trade that costs emeralds, so the Merchant keeps them rather than selling them. */
        void keepEmeralds() {
            enable("minecraft:emerald+>minecraft:bookshelf");
        }

        void enable(String key) {
            GlobalPos desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(DESK));
            MerchantBook.get(helper.getLevel().getServer()).setEnabled(desk, helper.absolutePos(LECTERN), key, true);
        }

        IItemHandler locker() {
            return Lockers.inventory(helper.getLevel(), List.of(helper.absolutePos(CHEST)));
        }

        void stock(Item item, int count) {
            ItemHandlerHelper.insertItemStacked(locker(), new ItemStack(item, count), false);
        }

        int count(Item item) {
            int total = 0;
            for (int slot = 0; slot < locker().getSlots(); slot++) {
                ItemStack stack = locker().getStackInSlot(slot);
                if (stack.is(item)) {
                    total += stack.getCount();
                }
            }
            return total;
        }

        void assertLocker(int paper, int emeralds) {
            helper.assertTrue(count(Items.PAPER) == paper, "expected " + paper + " paper, found " + count(Items.PAPER));
            helper.assertTrue(
                    count(Items.EMERALD) == emeralds, "expected " + emeralds + " emeralds, found " + count(Items.EMERALD));
        }
    }

    private static Market market(GameTestHelper helper, boolean withMerchant) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(LECTERN, Blocks.LECTERN);
        helper.setBlock(CHEST, Blocks.CHEST);
        Villager librarian = helper.spawn(EntityType.VILLAGER, new BlockPos(1, 2, 2));
        librarian.setVillagerData(librarian.getVillagerData().setProfession(VillagerProfession.LIBRARIAN));
        claim(helper, librarian, LECTERN);
        Optional<Villager> merchant = Optional.empty();
        if (withMerchant) {
            Villager m = helper.spawn(EntityType.VILLAGER, new BlockPos(4, 2, 4));
            m.setVillagerData(m.getVillagerData().setProfession(Merchant.PROFESSION.get()));
            claim(helper, m, DESK);
            merchant = Optional.of(m);
        }
        return new Market(helper, librarian, merchant);
    }

    /**
     * A restock as the Java rules hook it ({@code VillagerMixin}: demand floored, then the locker's trades). In core mode
     * the hook runs no locker trades (the core runs the lockers: CoreBindingTests' locker tests), so these tests of the
     * Java rules, which stay until delete-java-rules, run them under the Java rules there.
     */
    static void restock(Villager villager) {
        dev.b28.enterprise.locker.LockerTrading.underJavaRules(() -> {
            villager.restock();
            return null;
        });
    }

    /**
     * Gives the villager this job site at once. Waiting for a natural claim is flaky: tests run side by side, and a
     * villager may claim a neighbouring test's workstation.
     */
    static void claim(GameTestHelper helper, Villager villager, BlockPos site) {
        BlockPos pos = helper.absolutePos(site);
        helper.getLevel().getPoiManager().take(type -> true, (type, at) -> at.equals(pos), pos, 1);
        villager.getBrain().setMemory(MemoryModuleType.JOB_SITE, GlobalPos.of(helper.getLevel().dimension(), pos));
    }

    @GameTest(template = "floor", timeoutTicks = 400)
    public static void enabledTradeRunsAtRestock(GameTestHelper helper) {
        Market market = market(helper, true);
        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    MerchantOffer offer = market.givePaperTrade(0);
                    market.enable(PAPER_TRADE);
                    market.stock(Items.PAPER, 64);
                    restock(market.librarian());

                    market.assertLocker(16, 2);
                    helper.assertTrue(offer.getUses() == 2, "two trades used, found " + offer.getUses());
                    helper.assertTrue(market.librarian().getVillagerXp() == 4, "librarian earns trade XP");
                    int merchantXp = market.merchant().orElseThrow().getVillagerXp();
                    helper.assertTrue(merchantXp >= 6 && merchantXp <= 12,
                            "Merchant earns 3 to 6 XP per trade, like a player; found " + merchantXp);
                })
                .thenSucceed();
    }

    @GameTest(template = "floor", timeoutTicks = 400)
    public static void aVillagerLevelsUpAtOnceFromLockerTrades(GameTestHelper helper) {
        Market market = market(helper, true);
        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    market.givePaperTrade(0);
                    market.enable(PAPER_TRADE);
                    market.stock(Items.PAPER, 64);
                    market.stock(Items.PAPER, 64);
                    restock(market.librarian()); // five trades, ten XP: an Apprentice
                    helper.assertTrue(market.librarian().getVillagerData().getLevel() == 2,
                            "levelled up there and then, not on its own later tick: "
                                    + market.librarian().getVillagerData().getLevel());
                })
                .thenSucceed();
    }

    @GameTest(template = "floor", timeoutTicks = 400)
    public static void theMerchantTrainsASmithOnlyUnderATrainingContract(GameTestHelper helper) {
        Market market = market(helper, true);
        helper.setBlock(DESK.east(), Blocks.CHEST); // the Merchant's stock
        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    Villager smith = market.librarian();
                    smith.setVillagerData(smith.getVillagerData().setProfession(VillagerProfession.WEAPONSMITH));
                    MerchantOffers offers = new MerchantOffers();
                    offers.add(new MerchantOffer(new ItemCost(Items.COAL, 15), Optional.empty(),
                            new ItemStack(Items.EMERALD), 0, 16, 2, 0.05F, 0));
                    offers.add(new MerchantOffer(new ItemCost(Items.EMERALD, 1), Optional.empty(),
                            new ItemStack(Items.IRON_AXE), 0, 12, 1, 0.05F, 0));
                    smith.setOffers(offers);
                    var server = helper.getLevel().getServer();
                    GlobalPos desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(DESK));
                    MerchantBook book = MerchantBook.get(server);
                    var stock = dev.b28.enterprise.merchant.MerchantStock.at(helper.getLevel(), helper.absolutePos(DESK));
                    stock.insert(new ItemStack(Items.EMERALD, 20));
                    restock(smith);
                    String coal = "minecraft:coal+>minecraft:emerald";
                    String axe = "minecraft:emerald+>minecraft:iron_axe";
                    helper.assertFalse(book.isEnabled(desk, helper.absolutePos(LECTERN), coal),
                            "no contract to train it: the Merchant leaves it be");
                    dev.b28.enterprise.contract.ContractBook.get(server).draft(desk,
                            GlobalPos.of(helper.getLevel().dimension(), new BlockPos(9_400_000, 64, 9_000_000)),
                            List.of(new dev.b28.enterprise.contract.Training("minecraft:weaponsmith", 5, 24)),
                            dev.b28.enterprise.contract.Incoterm.EXW, helper.getLevel().getGameTime() + 240000, 24000, 1,
                            helper.getLevel().getGameTime(), desk, true);
                    restock(smith);
                    helper.assertTrue(book.isTraining(desk, helper.absolutePos(LECTERN), coal),
                            "under contract: what it learns on goes on");
                    helper.assertTrue(book.isTraining(desk, helper.absolutePos(LECTERN), axe),
                            "and with no coal in the warehouse, its work is bought instead");
                    helper.assertTrue(smith.getVillagerXp() > 0, "it learns");
                    smith.setVillagerData(smith.getVillagerData().setLevel(5));
                    restock(smith);
                    helper.assertFalse(book.isEnabled(desk, helper.absolutePos(LECTERN), coal)
                            || book.isEnabled(desk, helper.absolutePos(LECTERN), axe), "a master: the training stops");
                })
                .thenSucceed();
    }

    @GameTest(template = "floor", timeoutTicks = 400)
    public static void underAnOfftakeThePupilsWorkIsCratedForTheBuyerWithABill(GameTestHelper helper) {
        Market market = market(helper, true);
        helper.setBlock(DESK.east(), Blocks.CHEST); // the Merchant's stock
        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    Villager smith = market.librarian();
                    smith.setVillagerData(smith.getVillagerData().setProfession(VillagerProfession.WEAPONSMITH));
                    MerchantOffers offers = new MerchantOffers(); // a Novice who only sells
                    offers.add(new MerchantOffer(new ItemCost(Items.EMERALD, 1), Optional.empty(),
                            new ItemStack(Items.IRON_AXE), 0, 12, 1, 0.05F, 0));
                    smith.setOffers(offers);
                    var server = helper.getLevel().getServer();
                    GlobalPos desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(DESK));
                    GlobalPos buyer = GlobalPos.of(helper.getLevel().dimension(), new BlockPos(9_700_000, 64, 9_000_000));
                    var stock = dev.b28.enterprise.merchant.MerchantStock.at(helper.getLevel(), helper.absolutePos(DESK));
                    stock.insert(new ItemStack(Items.EMERALD, 40));
                    dev.b28.enterprise.contract.ContractBook.get(server).draft(desk, buyer,
                            List.of(new dev.b28.enterprise.contract.Training("minecraft:weaponsmith", 5, 24, 1.0)),
                            dev.b28.enterprise.contract.Incoterm.EXW, helper.getLevel().getGameTime() + 240000, 24000, 1,
                            helper.getLevel().getGameTime(), desk, true);
                    // The buyer's caravans call here: it will collect the work, and pay for it.
                    var ledger = dev.b28.enterprise.caravan.CaravanLedger.get(server);
                    var route = ledger.addRoute(buyer, desk);
                    restock(smith);
                    ledger.removeRoute(route.id());
                    helper.assertTrue(stock.count(new ItemCost(Items.IRON_AXE, 1)) == 0, "no axes left loose to clog it");
                    long crates = stock.looseStacks().stream()
                            .filter(s -> dev.b28.enterprise.cargo.Crates.consignedTo(s, buyer)).count();
                    helper.assertTrue(crates == 2, "twelve axes crated for the buyer: " + crates);
                    double owed = dev.b28.enterprise.contract.Bills.get(server).owedBy(buyer);
                    helper.assertTrue(owed >= 12 / 3.0, "and a bill drawn on the buyer for them: " + owed);
                })
                .thenSucceed();
    }

    @GameTest(template = "floor", timeoutTicks = 400)
    public static void atItsRestockTheMerchantBuysWhatItsContractsWantOfAVillager(GameTestHelper helper) {
        Market market = market(helper, true);
        helper.setBlock(DESK.east(), Blocks.CHEST); // the Merchant's stock
        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    Villager smith = market.librarian();
                    smith.setVillagerData(smith.getVillagerData().setProfession(VillagerProfession.WEAPONSMITH).setLevel(5));
                    MerchantOffers offers = new MerchantOffers(); // a master's sword
                    offers.add(new MerchantOffer(new ItemCost(Items.EMERALD, 20), Optional.empty(),
                            new ItemStack(Items.DIAMOND_SWORD), 0, 3, 30, 0.2F, 0));
                    smith.setOffers(offers);
                    var server = helper.getLevel().getServer();
                    GlobalPos desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(DESK));
                    var stock = dev.b28.enterprise.merchant.MerchantStock.at(helper.getLevel(), helper.absolutePos(DESK));
                    stock.insert(new ItemStack(Items.EMERALD, 64));
                    dev.b28.enterprise.contract.ContractBook.get(server).draft(desk,
                            GlobalPos.of(helper.getLevel().dimension(), new BlockPos(9_950_000, 64, 9_000_000)),
                            new dev.b28.enterprise.contract.Delivery(Items.DIAMOND_SWORD, 1, 30),
                            dev.b28.enterprise.contract.Incoterm.EXW, helper.getLevel().getGameTime(), 24000, 4,
                            helper.getLevel().getGameTime(), desk, true);
                    restock(smith);
                    helper.assertTrue(MerchantBook.get(server).isEnabled(desk, helper.absolutePos(LECTERN),
                            "minecraft:emerald+>minecraft:diamond_sword"), "its sword trade switched on, at the restock");
                    helper.assertTrue(stock.count(new ItemCost(Items.DIAMOND_SWORD, 1)) >= 1, "and swords bought for the contract");
                })
                .thenSucceed();
    }

    @GameTest(template = "floor", timeoutTicks = 400)
    public static void switchedOffTradeDoesNotRun(GameTestHelper helper) {
        Market market = market(helper, true);
        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    market.givePaperTrade(0);
                    market.stock(Items.PAPER, 64);
                    restock(market.librarian());
                    market.assertLocker(64, 0);
                })
                .thenSucceed();
    }

    @GameTest(template = "floor", timeoutTicks = 400)
    public static void noMerchantNoTrading(GameTestHelper helper) {
        Market market = market(helper, false);
        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    market.givePaperTrade(0);
                    market.enable(PAPER_TRADE);
                    market.stock(Items.PAPER, 64);
                    restock(market.librarian());
                    market.assertLocker(64, 0);
                })
                .thenSucceed();
    }

    @GameTest(template = "floor", timeoutTicks = 400)
    public static void inflatedPriceIsSkipped(GameTestHelper helper) {
        Market market = market(helper, true);
        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    // Restock cools demand by 16 when unused; 100 stays well above zero.
                    market.givePaperTrade(100);
                    market.enable(PAPER_TRADE);
                    market.stock(Items.PAPER, 64);
                    restock(market.librarian());
                    market.assertLocker(64, 0);
                })
                .thenSucceed();
    }

    @GameTest(template = "floor", timeoutTicks = 400)
    public static void masterMerchantGetsADiscount(GameTestHelper helper) {
        Market market = market(helper, true);
        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    Villager merchant = market.merchant().orElseThrow();
                    merchant.setVillagerData(merchant.getVillagerData().setLevel(5));
                    market.givePaperTrade(0);
                    market.enable(PAPER_TRADE);
                    market.stock(Items.PAPER, 64);
                    restock(market.librarian());
                    // Reputation 110 x 0.05 = 5 off: 19 paper per emerald, so 3 trades from 64.
                    market.assertLocker(7, 3);
                })
                .thenSucceed();
    }

    @GameTest(template = "floor", timeoutTicks = 400)
    public static void merchantFillsLockerFromShulkerAndCollects(GameTestHelper helper) {
        BlockPos stockChest = new BlockPos(4, 2, 3); // touches the desk
        Market market = market(helper, true);
        helper.setBlock(stockChest, Blocks.CHEST);
        NonNullList<ItemStack> boxContents = NonNullList.withSize(27, ItemStack.EMPTY);
        boxContents.set(0, new ItemStack(Items.PAPER, 64));
        boxContents.set(1, new ItemStack(Items.PAPER, 64));
        ItemStack box = new ItemStack(Items.SHULKER_BOX);
        box.set(DataComponents.CONTAINER, ItemContainerContents.fromItems(boxContents));
        IItemHandler stock = Lockers.inventory(helper.getLevel(), List.of(helper.absolutePos(stockChest)));
        stock.insertItem(0, box, false);

        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    market.givePaperTrade(0);
                    market.enable(PAPER_TRADE);
                    market.keepEmeralds();
                    restock(market.librarian());

                    // 128 paper moved into the locker: 5 trades use 120, 8 stay behind; emeralds go to stock.
                    market.assertLocker(8, 0);
                    ItemStack emeralds = stock.getStackInSlot(1);
                    helper.assertTrue(emeralds.is(Items.EMERALD) && emeralds.getCount() == 5,
                            "5 emeralds back in the Merchant's chest, found " + emeralds);
                    ItemContainerContents left = stock.getStackInSlot(0).get(DataComponents.CONTAINER);
                    helper.assertTrue(left != null && left.nonEmptyItems().iterator().hasNext() == false,
                            "the shulker box is empty");
                })
                .thenSucceed();
    }

    @GameTest(template = "floor", timeoutTicks = 400)
    public static void lockerRestocksDoNotSinkDemand(GameTestHelper helper) {
        Market market = market(helper, false);
        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    MerchantOffer offer = market.givePaperTrade(10);
                    restock(market.librarian()); // unused: vanilla would take demand to 10 - 16 = -6
                    helper.assertTrue(offer.getDemand() == 0, "cools to zero, found " + offer.getDemand());
                    restock(market.librarian());
                    helper.assertTrue(offer.getDemand() == 0, "stays at zero, found " + offer.getDemand());
                })
                .thenSucceed();
    }

    @GameTest(template = "floor")
    public static void merchantLevelsMatchPlayerLevels(GameTestHelper helper) {
        helper.assertTrue(dev.b28.enterprise.locker.LockerTrading.merchantXpForNextLevel(1) == 315, "Apprentice at 315");
        helper.assertTrue(dev.b28.enterprise.locker.LockerTrading.merchantXpForNextLevel(4) == 6895, "Master at 6895");
        helper.assertTrue(dev.b28.enterprise.locker.LockerTrading.merchantXpForNextLevel(5) == 0, "nothing past Master");
        helper.succeed();
    }

    @GameTest(template = "floor", timeoutTicks = 400)
    public static void unloadedDaysAreReplayedOnLoad(GameTestHelper helper) {
        BlockPos stockChest = new BlockPos(4, 2, 3); // touches the desk
        Market market = market(helper, true);
        helper.setBlock(stockChest, Blocks.CHEST);
        NonNullList<ItemStack> paper = NonNullList.withSize(27, ItemStack.EMPTY);
        for (int i = 0; i < 27; i++) {
            paper.set(i, new ItemStack(Items.PAPER, 64));
        }
        ItemStack box = new ItemStack(Items.SHULKER_BOX);
        box.set(DataComponents.CONTAINER, ItemContainerContents.fromItems(paper));
        Lockers.inventory(helper.getLevel(), List.of(helper.absolutePos(stockChest))).insertItem(0, box, false);

        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    market.givePaperTrade(0);
                    market.enable(PAPER_TRADE);
                    market.keepEmeralds();
                    Villager librarian = market.librarian();
                    long lastRestock = helper.getLevel().getGameTime() - 3 * 24000L - 100;
                    librarian.lastRestockGameTime = lastRestock;

                    int replayed = dev.b28.enterprise.locker.LockerTrading.underJavaRules(
                            () -> dev.b28.enterprise.locker.LockerCatchUp.catchUp(librarian));
                    helper.assertTrue(replayed == 6, "3 missed days replay 6 restocks, replayed " + replayed);
                    helper.assertTrue(librarian.lastRestockGameTime == lastRestock + 3 * 24000L,
                            "the clock moves by the 3 days replayed, keeping the 100 leftover ticks");
                    // Restocks alternate: trade 16, inflated (skip), trade 16, skip, trade 16, skip.
                    int emeralds = dev.b28.enterprise.merchant.MerchantStock.at(helper.getLevel(), helper.absolutePos(DESK))
                            .count(new ItemCost(Items.EMERALD, 1));
                    helper.assertTrue(emeralds == 48, "48 emeralds back in stock, found " + emeralds);
                })
                .thenSucceed();
    }

    @GameTest(template = "floor", timeoutTicks = 400)
    public static void catchUpWaitsForItsMerchant(GameTestHelper helper) {
        Market market = market(helper, false); // the Merchant is out of sight: its chunk isn't loaded, say
        long[] lastRestock = new long[1];
        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    market.givePaperTrade(0);
                    market.librarian().setNoAi(true); // like a villager in a chunk loaded only at the edge: not ticking
                    lastRestock[0] = helper.getLevel().getGameTime() - 3 * 24000L;
                    market.librarian().lastRestockGameTime = lastRestock[0];
                    dev.b28.enterprise.locker.LockerCatchUp.enqueueNear(helper.getLevel(), helper.absolutePos(LECTERN), 2);
                })
                .thenIdle(45) // past the catch-up's wait for things to load
                .thenExecute(() -> {
                    dev.b28.enterprise.locker.LockerCatchUp.runJobs(helper.getLevel().getServer());
                    helper.assertTrue(market.librarian().lastRestockGameTime == lastRestock[0],
                            "no Merchant to trade through: the missed days wait, unspent");
                })
                .thenSucceed();
    }

    @GameTest(template = "floor", timeoutTicks = 400)
    public static void neverRestockedOwesNothing(GameTestHelper helper) {
        Market market = market(helper, false);
        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    market.librarian().lastRestockGameTime = 0;
                    helper.assertTrue(dev.b28.enterprise.locker.LockerCatchUp.missedRestocks(market.librarian()) == 0,
                            "a villager that never restocked owes nothing");
                    dev.b28.enterprise.locker.LockerTrading.underJavaRules(
                            () -> dev.b28.enterprise.locker.LockerCatchUp.catchUp(market.librarian()));
                    helper.assertTrue(market.librarian().lastRestockGameTime == helper.getLevel().getGameTime(),
                            "but with a locker, its restock clock starts now, so later days count");
                })
                .thenSucceed();
    }

    /**
     * In Java mode a villager with a locker asks to restock on schedule, unused, so its locker trades. In core mode the
     * core restocks the hand's locker by its own clock (loaded or not), so the villager's own restocks are vanilla's:
     * none while its trades are unused, locker or not.
     */
    @GameTest(template = "floor", timeoutTicks = 400)
    public static void villagerWithLockerAsksToRestock(GameTestHelper helper) {
        Market market = market(helper, false);
        boolean core = dev.b28.enterprise.binding.Society.isCore();
        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    market.givePaperTrade(0);
                    helper.assertTrue(market.librarian().shouldRestock() != core, core
                            ? "in core mode an unused villager with a locker doesn't ask to restock (the core restocks its locker)"
                            : "an unused villager with a locker restocks");
                    helper.setBlock(CHEST, Blocks.AIR);
                    helper.assertFalse(market.librarian().shouldRestock(), "without a locker, vanilla rules apply");
                })
                .thenSucceed();
    }
}
