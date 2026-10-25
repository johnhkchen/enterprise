package dev.b28.enterprise.merchant;

import dev.b28.enterprise.binding.CoreHost;
import java.util.List;
import net.minecraft.ChatFormatting;
import net.minecraft.network.chat.Component;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.TooltipFlag;
import net.minecraft.world.item.context.UseOnContext;

/** A town charter: used on a Merchant's Desk, founds a town there ({@link Charters}). */
public final class CharterItem extends Item {
    public CharterItem(Properties properties) {
        super(properties);
    }

    @Override
    public InteractionResult useOn(UseOnContext context) {
        if (!context.getLevel().getBlockState(context.getClickedPos()).is(Merchant.DESK.get())) {
            return InteractionResult.PASS;
        }
        if (context.getLevel() instanceof ServerLevel level && context.getPlayer() != null) {
            Component said = Charters.use(CoreHost.of(level.getServer()), level, context.getClickedPos(),
                    context.getItemInHand(), context.getPlayer().getAbilities().instabuild);
            context.getPlayer().displayClientMessage(said, true);
        }
        return InteractionResult.sidedSuccess(context.getLevel().isClientSide());
    }

    @Override
    public void appendHoverText(ItemStack stack, TooltipContext context, List<Component> tooltip, TooltipFlag flag) {
        tooltip.add(Component.translatable("item.enterprise.town_charter.hint").withStyle(ChatFormatting.GRAY));
    }
}
