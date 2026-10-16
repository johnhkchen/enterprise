package dev.b28.enterprise.money;

import dev.b28.enterprise.merchant.MerchantStock;
import dev.ithundxr.createnumismatics.content.coins.CoinItem;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;

/**
 * A market's money: emeralds in its Merchant's chest, coins there, and the settlement's Blaze Banker. Withdrawals
 * take emeralds first, then coins (with change), then the Banker; amounts are in emeralds at the region's rate.
 */
public final class Treasury {
    private static final ItemCost EMERALD = new ItemCost(Items.EMERALD, 1);

    private Treasury() {}

    /** What was taken: whole emeralds, and spurs from coins and the Banker. */
    public record Withdrawal(int emeralds, int spurs) {
        public double value(double rate) {
            return emeralds + spurs / rate;
        }
    }

    /** All of a market's money, in emeralds at {@code rate}. */
    public static double available(ServerLevel level, BlockPos desk, MerchantStock stock, double rate) {
        int bank = MoneyChanger.bankAt(level, desk).map(account -> account.getBalance()).orElse(0);
        return stock.count(EMERALD) + (MoneyChanger.value(stock.looseStacks()) + bank) / rate;
    }

    /**
     * Takes up to {@code emeralds} worth, as far as the market has it. Emeralds go whole; a part-emerald comes from
     * coins, then the Banker, and if those run short an emerald is changed at the mint and the change goes back.
     */
    public static Withdrawal withdraw(
            ServerLevel level, BlockPos desk, MerchantStock stock, double emeralds, Mint mint, long now) {
        double rate = mint.rate(Mint.DEFAULT_FACTION, now);
        int whole = Math.min((int) Math.floor(emeralds), stock.count(EMERALD));
        stock.take(EMERALD, whole);
        double rest = emeralds - whole;
        if (rest <= 1e-9) {
            CashBook.note(net.minecraft.core.GlobalPos.of(level.dimension(), desk), CashBook.caller(), -whole);
            return new Withdrawal(whole, 0);
        }
        int spursOwed = (int) Math.ceil(rest * rate);
        List<ItemStack> coins = stock.removeLoose(stack -> stack.getItem() instanceof CoinItem);
        int held = MoneyChanger.value(coins);
        int fromChest = Math.min(held, spursOwed);
        MoneyChanger.coins(held - fromChest).forEach(stock::insert); // change goes back
        int fromBank = 0;
        var bank = MoneyChanger.bankAt(level, desk);
        if (spursOwed > fromChest && bank.isPresent()) {
            fromBank = Math.min(spursOwed - fromChest, bank.get().getBalance());
            bank.get().setBalance(bank.get().getBalance() - fromBank);
        }
        int changed = 0;
        int short1 = spursOwed - fromChest - fromBank;
        if (short1 > 0 && stock.count(EMERALD) > 0) {
            int toChange = Math.min(stock.count(EMERALD), Math.max(1, (int) Math.ceil(short1 / rate)));
            stock.take(EMERALD, toChange);
            int got = mint.sell(Mint.DEFAULT_FACTION, toChange, now);
            changed = Math.min(short1, got);
            MoneyChanger.coins(got - changed).forEach(stock::insert); // change goes back
        }
        Withdrawal taken = new Withdrawal(whole, fromChest + fromBank + changed);
        CashBook.note(net.minecraft.core.GlobalPos.of(level.dimension(), desk), CashBook.caller(), -taken.value(rate));
        return taken;
    }

    /** Pays {@code spurs} into the market: its Blaze Banker if it has one, otherwise coins in the chest. */
    public static void deposit(ServerLevel level, BlockPos desk, MerchantStock stock, int spurs) {
        if (spurs <= 0) {
            return;
        }
        CashBook.note(net.minecraft.core.GlobalPos.of(level.dimension(), desk), CashBook.caller(),
                spurs / Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime()));
        var bank = MoneyChanger.bankAt(level, desk);
        if (bank.isPresent()) {
            bank.get().deposit(spurs);
        } else {
            MoneyChanger.coins(spurs).forEach(stock::insert);
        }
    }
}
