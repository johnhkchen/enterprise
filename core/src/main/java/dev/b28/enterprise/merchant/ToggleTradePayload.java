package dev.b28.enterprise.merchant;

import dev.b28.enterprise.Enterprise;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.server.level.ServerPlayer;
import net.neoforged.neoforge.network.handling.IPayloadContext;

/** Client to server: turn one of a locker's trades on or off on the open Merchant screen. */
public record ToggleTradePayload(BlockPos workstation, String key, boolean enabled) implements CustomPacketPayload {
    public static final Type<ToggleTradePayload> TYPE = new Type<>(Enterprise.id("toggle_trade"));
    public static final StreamCodec<FriendlyByteBuf, ToggleTradePayload> STREAM_CODEC = StreamCodec.composite(
            BlockPos.STREAM_CODEC, ToggleTradePayload::workstation,
            ByteBufCodecs.STRING_UTF8, ToggleTradePayload::key,
            ByteBufCodecs.BOOL, ToggleTradePayload::enabled,
            ToggleTradePayload::new);

    @Override
    public Type<ToggleTradePayload> type() {
        return TYPE;
    }

    static void handle(ToggleTradePayload payload, IPayloadContext context) {
        context.enqueueWork(() -> {
            if (!(context.player() instanceof ServerPlayer player)
                    || !(player.containerMenu instanceof MerchantMenu menu)
                    || !menu.stillValid(player)) {
                return;
            }
            // Only trades the screen was shown may be switched.
            boolean known = menu.view().lockers().stream()
                    .filter(locker -> locker.workstation().equals(payload.workstation()))
                    .flatMap(locker -> locker.trades().stream())
                    .anyMatch(trade -> trade.key().equals(payload.key()));
            if (!known) {
                return;
            }
            // In core mode the screen was shown the core's offers, keyed for it: the switch is the core's.
            if (payload.key().startsWith(dev.b28.enterprise.binding.adapters.LockerAdapter.KEY)) {
                var host = dev.b28.enterprise.binding.CoreHost.of(player.server);
                if (host != null && host.pausedWhy() == null && !host.lockers().flip(payload.key(), payload.enabled())) {
                    Enterprise.LOGGER.warn("The core didn't take the switch {} on the Merchant's screen", payload.key());
                }
                return;
            }
            GlobalPos desk = GlobalPos.of(player.level().dimension(), menu.view().desk());
            MerchantBook.get(player.server).setEnabled(desk, payload.workstation(), payload.key(), payload.enabled());
        });
    }
}
