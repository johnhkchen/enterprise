package dev.b28.enterprise.merchant;

import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.item.ItemStack;

/** The Merchant screen's menu. It has no slots: it carries the view and which Merchant it belongs to. */
public final class MerchantMenu extends AbstractContainerMenu {
    private static final double REACH_SQR = 8.0 * 8.0;

    private final MerchantView view;
    private final int merchantId;

    public MerchantMenu(int containerId, MerchantView view, int merchantId) {
        super(Merchant.MENU.get(), containerId);
        this.view = view;
        this.merchantId = merchantId;
    }

    /** Client side: rebuilt from what the server sent with the open request. */
    public MerchantMenu(int containerId, Inventory inventory, RegistryFriendlyByteBuf buf) {
        this(containerId, MerchantView.STREAM_CODEC.decode(buf), buf.readVarInt());
    }

    public MerchantView view() {
        return view;
    }

    @Override
    public ItemStack quickMoveStack(Player player, int index) {
        return ItemStack.EMPTY;
    }

    @Override
    public boolean stillValid(Player player) {
        Entity merchant = player.level().getEntity(merchantId);
        return merchant != null && merchant.isAlive() && player.distanceToSqr(merchant) <= REACH_SQR;
    }
}
