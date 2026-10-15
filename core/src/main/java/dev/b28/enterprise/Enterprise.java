package dev.b28.enterprise;

import com.mojang.logging.LogUtils;
import dev.b28.enterprise.caravan.Caravans;
import dev.b28.enterprise.locker.LockerCatchUp;
import dev.b28.enterprise.locker.Rounds;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.settlement.CensusTaker;
import dev.b28.enterprise.territory.Terrain;
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
        dev.b28.enterprise.cargo.Cargo.register(modBus);
        NeoForge.EVENT_BUS.addListener(EnterpriseCommands::register);
        NeoForge.EVENT_BUS.addListener(LockerCatchUp::onJoin);
        NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.territory.Growth::onJoin);
        NeoForge.EVENT_BUS.addListener(LockerCatchUp::onServerTick);
        NeoForge.EVENT_BUS.addListener(Rounds::onServerTick);
        NeoForge.EVENT_BUS.addListener(CensusTaker::onServerTick);
        NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.settlement.Settlements::onDeath);
        NeoForge.EVENT_BUS.addListener(Caravans::onServerTick);
        NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.caravan.Expeditions::onServerTick);
        NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.market.Broker::onServerTick);
        NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.market.Sovereign::onServerTick);
        NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.market.Freight::onServerTick);
        NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.market.MetalExchange::onServerTick);
        NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.territory.Works::onServerTick);
        NeoForge.EVENT_BUS.addListener(net.neoforged.neoforge.event.level.ChunkEvent.Load.class, Terrain::onChunk);
        NeoForge.EVENT_BUS.addListener(net.neoforged.neoforge.event.level.ChunkEvent.Unload.class, Terrain::onChunk);
    }

    public static ResourceLocation id(String path) {
        return ResourceLocation.fromNamespaceAndPath(MOD_ID, path);
    }
}
