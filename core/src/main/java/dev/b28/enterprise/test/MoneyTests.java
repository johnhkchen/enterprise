package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.MoneyChanger;
import dev.ithundxr.createnumismatics.Numismatics;
import dev.ithundxr.createnumismatics.content.backend.BankAccount;
import dev.ithundxr.createnumismatics.content.bank.blaze_banker.BlazeBankerBlockEntity;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

// Each test sells to its own fresh Mint, so tests running side by side don't move each other's rates.
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class MoneyTests {
    private static final BlockPos DESK = new BlockPos(3, 2, 3);
    private static final BlockPos STOCK = new BlockPos(4, 2, 3);
    private static final BlockPos BANKER = new BlockPos(3, 2, 4);
    private static final String FACTION = Mint.DEFAULT_FACTION;
    private static final ItemCost EMERALD = new ItemCost(Items.EMERALD, 1);

    @GameTest(template = "floor")
    public static void rateFallsWithSellingAndRecovers(GameTestHelper helper) {
        Mint mint = new Mint();
        helper.assertTrue(mint.rate(FACTION, 0) == 16, "starts at 16 spurs per emerald");
        int paid = mint.sell(FACTION, 1728, 0);
        // A shulker's worth at once: 16 x 1728 x ln 2 = 19164, between the opening and closing rates.
        helper.assertTrue(paid == 19164, "a shulker of emeralds pays 19164 spurs, paid " + paid);
        helper.assertTrue(Math.abs(mint.rate(FACTION, 0) - 8) < 1e-9, "the rate halves after a shulker's worth");
        double later = mint.rate(FACTION, Mint.HALF_LIFE_TICKS);
        helper.assertTrue(Math.abs(later - 16 / 1.5) < 1e-9, "half the pressure is gone after 3 days, rate " + later);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void surplusEmeraldsBecomeCoinsInStock(GameTestHelper helper) {
        MerchantStock stock = stockWith(helper, 100);
        Mint mint = new Mint();
        int expected = mint.quote(FACTION, 36, helper.getLevel().getGameTime());

        var result = MoneyChanger.settle(helper.getLevel(), helper.absolutePos(DESK), stock, mint, FACTION);
        helper.assertTrue(result.emeralds() == 36 && !result.deposited(), "36 emeralds sold for coins, got " + result);
        helper.assertTrue(stock.count(EMERALD) == MoneyChanger.EMERALD_RESERVE, "64 emeralds kept in reserve");
        int coins = MoneyChanger.value(stock.looseStacks());
        helper.assertTrue(coins == expected && coins == result.spurs(),
                "coins worth " + expected + " spurs in stock, found " + coins);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void coinsAreRemintedIntoFewStacks(GameTestHelper helper) {
        MerchantStock stock = stockWith(helper, 64 + 300);
        for (int i = 0; i < 3; i++) {
            stock.insert(MoneyChanger.coins(64).get(0).copyWithCount(1)); // three loose cogs
        }
        stock.insert(MoneyChanger.coins(1).get(0).copyWithCount(40)); // forty loose spurs
        Mint mint = new Mint();
        int expected = 3 * 64 + 40 + mint.quote(FACTION, 300, helper.getLevel().getGameTime());

        MoneyChanger.settle(helper.getLevel(), helper.absolutePos(DESK), stock, mint, FACTION);
        int value = MoneyChanger.value(stock.looseStacks());
        long coinStacks = stock.looseStacks().stream()
                .filter(stack -> stack.getItem() instanceof dev.ithundxr.createnumismatics.content.coins.CoinItem)
                .count();
        helper.assertTrue(value == expected, "no value lost in re-minting: expected " + expected + ", found " + value);
        helper.assertTrue(coinStacks <= 6, "at most one stack per coin, found " + coinStacks);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void reserveIsNotSold(GameTestHelper helper) {
        MerchantStock stock = stockWith(helper, 50);
        var result = MoneyChanger.settle(helper.getLevel(), helper.absolutePos(DESK), stock, new Mint(), FACTION);
        helper.assertTrue(result.emeralds() == 0, "under the reserve, nothing is sold");
        helper.assertTrue(stock.count(EMERALD) == 50, "all 50 emeralds stay");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void blazeBankerTakesTheDeposit(GameTestHelper helper) {
        MerchantStock stock = stockWith(helper, 164);
        helper.setBlock(BANKER, BuiltInRegistries.BLOCK.get(ResourceLocation.fromNamespaceAndPath("numismatics", "blaze_banker")));
        BlazeBankerBlockEntity banker = (BlazeBankerBlockEntity) helper.getBlockEntity(BANKER);
        helper.assertTrue(banker.getAccountId() != null, "a Blaze Banker has an account id");
        BankAccount account = Numismatics.BANK.getOrCreateAccount(banker.getAccountId(), BankAccount.Type.BLAZE_BANKER);
        int before = account.getBalance();
        Mint mint = new Mint();
        int expected = mint.quote(FACTION, 100, helper.getLevel().getGameTime());

        var result = MoneyChanger.settle(helper.getLevel(), helper.absolutePos(DESK), stock, mint, FACTION);
        helper.assertTrue(result.deposited(), "coins go to the Blaze Banker's account");
        helper.assertTrue(account.getBalance() - before == expected,
                "balance grew by " + expected + ", grew by " + (account.getBalance() - before));
        helper.assertTrue(MoneyChanger.value(stock.looseStacks()) == 0, "no loose coins in stock");
        helper.succeed();
    }

    private static MerchantStock stockWith(GameTestHelper helper, int emeralds) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(STOCK, Blocks.CHEST);
        MerchantStock stock = MerchantStock.at(helper.getLevel(), helper.absolutePos(DESK));
        for (int left = emeralds; left > 0; left -= 64) {
            stock.insert(new ItemStack(Items.EMERALD, Math.min(64, left)));
        }
        return stock;
    }
}
