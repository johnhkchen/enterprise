package dev.b28.enterprise.client;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.Merchant;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.bus.api.SubscribeEvent;
import net.neoforged.fml.common.EventBusSubscriber;
import net.neoforged.neoforge.client.event.RegisterColorHandlersEvent;
import net.neoforged.neoforge.client.event.RegisterMenuScreensEvent;

@EventBusSubscriber(modid = Enterprise.MOD_ID, value = Dist.CLIENT)
public final class EnterpriseClient {
    /** Steel-blue wash over the cartography table art, so the Merchant's Desk reads as its own block. */
    private static final int DESK_TINT = 0xFFA9BEDD;

    private EnterpriseClient() {}

    @SubscribeEvent
    static void tintBlocks(RegisterColorHandlersEvent.Block event) {
        event.register((state, level, pos, tintIndex) -> DESK_TINT, Merchant.DESK.get());
    }

    @SubscribeEvent
    static void tintItems(RegisterColorHandlersEvent.Item event) {
        event.register((stack, tintIndex) -> DESK_TINT, Merchant.DESK_ITEM.get());
    }

    @SubscribeEvent
    static void registerScreens(RegisterMenuScreensEvent event) {
        event.register(Merchant.MENU.get(), MerchantScreen::new);
    }
}
