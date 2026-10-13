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
                    market.librarian().restock();

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
    public static void switchedOffTradeDoesNotRun(GameTestHelper helper) {
        Market market = market(helper, true);
        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    market.givePaperTrade(0);
                    market.stock(Items.PAPER, 64);
                    market.librarian().restock();
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
                    market.librarian().restock();
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
                    market.librarian().restock();
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
                    market.librarian().restock();
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
                    market.librarian().restock();

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
                    market.librarian().restock(); // unused: vanilla would take demand to 10 - 16 = -6
                    helper.assertTrue(offer.getDemand() == 0, "cools to zero, found " + offer.getDemand());
                    market.librarian().restock();
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
    public static void villagerWithLockerAsksToRestock(GameTestHelper helper) {
        Market market = market(helper, false);
        helper.startSequence().thenWaitUntil(() -> helper.assertTrue(market.ready(), "waiting for job sites"))
                .thenExecute(() -> {
                    market.givePaperTrade(0);
                    helper.assertTrue(market.librarian().shouldRestock(), "an unused villager with a locker restocks");
                    helper.setBlock(CHEST, Blocks.AIR);
                    helper.assertFalse(market.librarian().shouldRestock(), "without a locker, vanilla rules apply");
                })
                .thenSucceed();
    }
}
