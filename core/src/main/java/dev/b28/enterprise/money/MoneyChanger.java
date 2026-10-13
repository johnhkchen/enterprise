package dev.b28.enterprise.money;

import dev.b28.enterprise.merchant.MerchantBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.ithundxr.createnumismatics.Numismatics;
import dev.ithundxr.createnumismatics.content.backend.BankAccount;
import dev.ithundxr.createnumismatics.content.backend.Coin;
import dev.ithundxr.createnumismatics.content.bank.blaze_banker.BlazeBankerBlockEntity;
import dev.ithundxr.createnumismatics.content.coins.CoinItem;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.GlobalPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;

/**
 * The Merchant as money changer: emeralds above a working reserve are sold to its faction's mint for Numismatics
 * coins. A Blaze Banker touching the desk is the settlement's bank account and takes the coins as a deposit;
 * otherwise they land as coins in the Merchant's stock, if they fit.
 */
public final class MoneyChanger {
    /** Emeralds the Merchant keeps when any of its lockers has a trade switched on that costs emeralds. */
    public static final int EMERALD_RESERVE = 64;

    private static final ItemCost EMERALD = new ItemCost(Items.EMERALD, 1);

    private MoneyChanger() {}

    /** What happened in one settlement: emeralds sold and spurs paid, and whether they went to a bank account. */
    public record Settlement(int emeralds, int spurs, boolean deposited) {
        public static final Settlement NONE = new Settlement(0, 0, false);
    }

    public static Settlement settle(ServerLevel level, BlockPos desk, MerchantStock stock) {
        return settle(level, desk, stock, Mint.get(level.getServer()), Mint.DEFAULT_FACTION);
    }

    public static Settlement settle(ServerLevel level, BlockPos desk, MerchantStock stock, Mint mint, String faction) {
        int surplus = stock.count(EMERALD) - reserve(level, desk);
        if (surplus <= 0) {
            return Settlement.NONE;
        }
        long now = level.getGameTime();
        Optional<BankAccount> bank = bankAt(level, desk);
        if (bank.isPresent()) {
            stock.take(EMERALD, surplus);
            int spurs = mint.sell(faction, surplus, now);
            bank.get().deposit(spurs);
            return new Settlement(surplus, spurs, true);
        }
        // Pay out in coins, re-minting the loose coins already in stock with them so they stay a few stacks.
        List<ItemStack> held = stock.removeLoose(stack -> stack.getItem() instanceof CoinItem);
        int heldSpurs = value(held);
        if (!stock.fits(coins(heldSpurs + mint.quote(faction, surplus, now)))) {
            held.forEach(stock::insert);
            return Settlement.NONE; // no room for the coins: keep the emeralds for now
        }
        stock.take(EMERALD, surplus);
        int spurs = mint.sell(faction, surplus, now);
        coins(heldSpurs + spurs).forEach(stock::insert);
        return new Settlement(surplus, spurs, false);
    }

    /** The working reserve: {@link #EMERALD_RESERVE} if a switched-on trade costs emeralds, otherwise none. */
    public static int reserve(ServerLevel level, BlockPos desk) {
        MerchantBook book = MerchantBook.get(level.getServer());
        GlobalPos at = GlobalPos.of(level.dimension(), desk);
        boolean spendsEmeralds = book.workstations(at).stream()
                .flatMap(workstation -> book.enabledTrades(at, workstation).stream())
                .anyMatch(key -> key.startsWith("minecraft:emerald+") || key.contains("+minecraft:emerald>"));
        return spendsEmeralds ? EMERALD_RESERVE : 0;
    }

    /** The bank account of a Blaze Banker touching the desk. Opens one for a Banker placed without an owner. */
    public static Optional<BankAccount> bankAt(ServerLevel level, BlockPos desk) {
        for (Direction dir : Direction.values()) {
            if (level.getBlockEntity(desk.relative(dir)) instanceof BlazeBankerBlockEntity banker
                    && banker.getAccountId() != null) {
                return Optional.of(banker.hasAccount()
                        ? banker.getAccount()
                        : Numismatics.BANK.getOrCreateAccount(banker.getAccountId(), BankAccount.Type.BLAZE_BANKER));
            }
        }
        return Optional.empty();
    }

    /** {@code spurs} as the fewest coins, largest first. */
    public static List<ItemStack> coins(int spurs) {
        List<ItemStack> stacks = new ArrayList<>();
        for (Map.Entry<Coin, Integer> entry : Coin.getCoinsFromSpurAmount(spurs)) {
            int count = entry.getValue();
            while (count > 0) {
                ItemStack stack = entry.getKey().asStack(count);
                int size = Math.min(count, stack.getMaxStackSize());
                stack.setCount(size);
                stacks.add(stack);
                count -= size;
            }
        }
        return stacks;
    }

    /** Total value in spurs of the Numismatics coins in a list of stacks. */
    public static int value(Iterable<ItemStack> stacks) {
        int spurs = 0;
        for (ItemStack stack : stacks) {
            if (stack.getItem() instanceof CoinItem coin) {
                spurs += coin.coin.toSpurs(stack.getCount());
            }
        }
        return spurs;
    }
}
