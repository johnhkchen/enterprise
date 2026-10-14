package dev.b28.enterprise.merchant;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.caravan.Caravans;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.server.level.ServerPlayer;
import net.neoforged.neoforge.network.handling.IPayloadContext;

/** Client to server: start or stop sending caravans from the open Merchant screen's desk to a market it knows. */
public record CommissionPayload(BlockPos destination, boolean send) implements CustomPacketPayload {
    public static final Type<CommissionPayload> TYPE = new Type<>(Enterprise.id("commission"));
    public static final StreamCodec<FriendlyByteBuf, CommissionPayload> STREAM_CODEC = StreamCodec.composite(
            BlockPos.STREAM_CODEC, CommissionPayload::destination,
            ByteBufCodecs.BOOL, CommissionPayload::send,
            CommissionPayload::new);

    @Override
    public Type<CommissionPayload> type() {
        return TYPE;
    }

    static void handle(CommissionPayload payload, IPayloadContext context) {
        context.enqueueWork(() -> {
            if (context.player() instanceof ServerPlayer player
                    && player.containerMenu instanceof MerchantMenu menu
                    && menu.stillValid(player)) {
                commission(player.server, GlobalPos.of(player.level().dimension(), menu.view().desk()), menu.view(),
                        payload.destination(), payload.send());
            }
        });
    }

    /**
     * Starts (or stops) caravans from {@code desk} to {@code destination}, if the screen showed that market and a
     * caravan can reach it. Returns true if anything changed.
     */
    public static boolean commission(net.minecraft.server.MinecraftServer server, GlobalPos desk, MerchantView view,
            BlockPos destination, boolean send) {
        var town = view.caravans().towns().stream().filter(t -> t.desk().equals(destination)).findFirst();
        if (town.isEmpty()) {
            return false; // only markets the screen was shown
        }
        GlobalPos to = GlobalPos.of(desk.dimension(), destination);
        CaravanLedger ledger = CaravanLedger.get(server);
        var route = ledger.route(desk, to);
        if (send && route.isEmpty() && Caravans.inRange(town.get().distance())) {
            ledger.addRoute(desk, to);
            return true;
        }
        if (!send && route.isPresent()) {
            return ledger.removeRoute(route.get().id());
        }
        return false;
    }
}
