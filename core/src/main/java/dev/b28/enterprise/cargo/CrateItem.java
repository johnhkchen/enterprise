package dev.b28.enterprise.cargo;

import java.util.List;
import net.minecraft.ChatFormatting;
import net.minecraft.network.chat.Component;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.TooltipFlag;
import net.minecraft.world.level.block.Block;

/** A crate in hand: it carries its goods and papers, and doesn't go inside another container. */
public final class CrateItem extends BlockItem {
    public CrateItem(Block block, Properties properties) {
        super(block, properties);
    }

    @Override
    public boolean canFitInsideContainerItems() {
        return false;
    }

    /** A crate that is a package of one of the core's pallets is carried, never set down to be opened: it isn't placed. */
    @Override
    public net.minecraft.world.InteractionResult place(net.minecraft.world.item.context.BlockPlaceContext context) {
        if (Cargo.isPackage(context.getItemInHand())) {
            return net.minecraft.world.InteractionResult.FAIL;
        }
        return super.place(context);
    }

    @Override
    public void appendHoverText(ItemStack stack, TooltipContext context, List<Component> tooltip, TooltipFlag flag) {
        var goods = Crates.goods(stack);
        int count = goods.stream().mapToInt(ItemStack::getCount).sum();
        tooltip.add(Component.translatable(count == 0 ? "item.enterprise.crate.empty" : "item.enterprise.crate.goods",
                count, goods.size()).withStyle(ChatFormatting.GRAY));
        PackageTag tag = stack.get(Cargo.PACKAGE.get());
        if (tag != null) {
            tooltip.addAll(tag.describe());
        }
        Crates.papers(stack).describe().forEach(line -> tooltip.add(Component.literal(line).withStyle(ChatFormatting.DARK_GRAY)));
    }
}
