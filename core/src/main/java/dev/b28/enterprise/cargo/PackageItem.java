package dev.b28.enterprise.cargo;

import java.util.List;
import net.minecraft.ChatFormatting;
import net.minecraft.network.chat.Component;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.TooltipFlag;

/**
 * A package of a kind of packing other than the crate (a barrel, a bundle: {@code binding/ModPack.PACKAGES}), as a
 * pallet's package is shown: its goods inside, its tag on it ({@link PackageTag}). Carried whole, never opened by hand:
 * what's in it is the core's to take in when its pallet is received.
 */
public final class PackageItem extends Item {
    public PackageItem(Properties properties) {
        super(properties);
    }

    @Override
    public boolean canFitInsideContainerItems() {
        return false;
    }

    @Override
    public void appendHoverText(ItemStack stack, TooltipContext context, List<Component> tooltip, TooltipFlag flag) {
        int count = Crates.goods(stack).stream().mapToInt(ItemStack::getCount).sum();
        tooltip.add(Component.translatable(count == 0 ? "item.enterprise.crate.empty" : "item.enterprise.crate.goods", count,
                Crates.goods(stack).size()).withStyle(ChatFormatting.GRAY));
        PackageTag tag = stack.get(Cargo.PACKAGE.get());
        if (tag != null) {
            tooltip.addAll(tag.describe());
        }
    }
}
