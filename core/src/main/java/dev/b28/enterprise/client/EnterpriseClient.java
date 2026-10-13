package dev.b28.enterprise.client;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.Merchant;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.bus.api.SubscribeEvent;
import net.neoforged.fml.common.EventBusSubscriber;
import net.neoforged.neoforge.client.event.RegisterMenuScreensEvent;

@EventBusSubscriber(modid = Enterprise.MOD_ID, value = Dist.CLIENT)
public final class EnterpriseClient {
    private EnterpriseClient() {}

    @SubscribeEvent
    static void registerScreens(RegisterMenuScreensEvent event) {
        event.register(Merchant.MENU.get(), MerchantScreen::new);
    }
}
