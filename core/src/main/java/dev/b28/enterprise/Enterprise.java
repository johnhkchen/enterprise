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
        dev.b28.enterprise.settlement.Props.register(modBus);
        dev.b28.enterprise.trade.Wares.register(modBus);
        NeoForge.EVENT_BUS.addListener(EnterpriseCommands::register);
        // The studio's kits' metadata read again when the data packs reload (`/reload`).
        NeoForge.EVENT_BUS.addListener((net.neoforged.neoforge.event.AddReloadListenerEvent e) -> dev.b28.enterprise.territory.Kits.forget());
        NeoForge.EVENT_BUS.addListener(LedgerCommand::register);
        NeoForge.EVENT_BUS.addListener(Dials::onServerStarted);
        NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.market.RentDeeds::onRightClick);
        NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.territory.Growth::onJoin);
        NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.settlement.Settlements::onDeath);
        // The rules' clocks: the Java rule classes tick themselves, or (society = core) the core runs the rules and
        // CoreClock below is the only clock; the listeners that are views (joins, deaths, chunks) stay either way.
        // LockerCatchUp's join queues the work its tick drains: both are the rule, so both go in core mode.
        if (!dev.b28.enterprise.binding.Society.isCore()) {
            NeoForge.EVENT_BUS.addListener(LockerCatchUp::onJoin);
            NeoForge.EVENT_BUS.addListener(LockerCatchUp::onServerTick);
            NeoForge.EVENT_BUS.addListener(Rounds::onServerTick);
            NeoForge.EVENT_BUS.addListener(CensusTaker::onServerTick);
            NeoForge.EVENT_BUS.addListener(Caravans::onServerTick);
            NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.caravan.Expeditions::onServerTick);
            NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.trade.Realms::onServerTick);
            NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.market.Broker::onServerTick);
            NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.market.Sovereign::onServerTick);
            NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.market.Freight::onServerTick);
            NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.market.MetalExchange::onServerTick);
            NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.territory.Works::onServerTick);
        }
        NeoForge.EVENT_BUS.addListener(net.neoforged.neoforge.event.level.ChunkEvent.Load.class, Terrain::onChunk);
        NeoForge.EVENT_BUS.addListener(net.neoforged.neoforge.event.level.ChunkEvent.Unload.class, Terrain::onChunk);
        // The Rust core beside the Java rules (docs/binding-plan.md B2.1): started, saved and stopped with the server,
        // and a look handed to it each game minute.
        NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.test.GameTestReport::onServerStarting);
        dev.b28.enterprise.binding.CoreHost.register(NeoForge.EVENT_BUS);
        NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.binding.PausedNotice::onJoin);
        NeoForge.EVENT_BUS.addListener(dev.b28.enterprise.binding.CoreClock::onServerTick);
    }

    public static ResourceLocation id(String path) {
        return ResourceLocation.fromNamespaceAndPath(MOD_ID, path);
    }
}
