package dev.b28.enterprise.merchant;

import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.trading.MerchantOffer;

/** Names a villager trade by its items, so a Merchant's choice survives level-ups and price changes. */
public final class TradeKeys {
    private TradeKeys() {}

    /** "costA+costB>result" as item ids; costB is empty when the trade takes one item. */
    public static String of(MerchantOffer offer) {
        return id(offer.getBaseCostA()) + "+" + id(offer.getCostB()) + ">" + id(offer.getResult());
    }

    private static String id(ItemStack stack) {
        return stack.isEmpty() ? "" : BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
    }
}
