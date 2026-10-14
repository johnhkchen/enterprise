package dev.b28.enterprise.cargo;

import java.util.List;
import net.minecraft.ChatFormatting;
import net.minecraft.network.chat.Component;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.TooltipFlag;

/** A bundle of documents: the papers it holds show on hover. */
public final class DocumentsItem extends Item {
    public DocumentsItem(Properties properties) {
        super(properties);
    }

    @Override
    public void appendHoverText(ItemStack stack, TooltipContext context, List<Component> tooltip, TooltipFlag flag) {
        Papers papers = stack.getOrDefault(Cargo.PAPERS.get(), Papers.NONE);
        if (papers.documents().isEmpty()) {
            tooltip.add(Component.translatable("item.enterprise.documents.blank").withStyle(ChatFormatting.GRAY));
        }
        papers.describe().forEach(line -> tooltip.add(Component.literal(line).withStyle(ChatFormatting.GRAY)));
    }

    @Override
    public boolean canFitInsideContainerItems() {
        return true;
    }
}
