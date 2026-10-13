package dev.b28.enterprise;

import com.mojang.logging.LogUtils;
import dev.b28.enterprise.locker.LockerCatchUp;
import dev.b28.enterprise.locker.Rounds;
import dev.b28.enterprise.merchant.Merchant;
import net.minecraft.resources.ResourceLocation;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.fml.common.Mod;
import net.neoforged.neoforge.common.NeoForge;
import org.slf4j.Logger;

@Mod(Enterprise.MOD_ID)
public final class Enterprise {
    public static final String MOD_ID = "enterprise";
    public static final Logger LOGGER = LogUtils.getLogger();

    public Enterprise(IEventBus modBus) {
        Merchant.register(modBus);
        NeoForge.EVENT_BUS.addListener(EnterpriseCommands::register);
        NeoForge.EVENT_BUS.addListener(LockerCatchUp::onJoin);
        NeoForge.EVENT_BUS.addListener(LockerCatchUp::onServerTick);
        NeoForge.EVENT_BUS.addListener(Rounds::onServerTick);
    }

    public static ResourceLocation id(String path) {
        return ResourceLocation.fromNamespaceAndPath(MOD_ID, path);
    }
}
