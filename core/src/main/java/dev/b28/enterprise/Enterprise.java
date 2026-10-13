package dev.b28.enterprise;

import com.mojang.logging.LogUtils;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.fml.common.Mod;
import org.slf4j.Logger;

@Mod(Enterprise.MOD_ID)
public final class Enterprise {
    public static final String MOD_ID = "enterprise";
    public static final Logger LOGGER = LogUtils.getLogger();

    public Enterprise(IEventBus modBus) {
        LOGGER.info("Enterprise core loaded");
    }
}
