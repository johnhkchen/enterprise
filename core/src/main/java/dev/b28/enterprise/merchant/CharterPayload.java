package dev.b28.enterprise.merchant;

import dev.b28.enterprise.Enterprise;
import io.netty.buffer.ByteBuf;
import net.minecraft.network.chat.Component;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.server.level.ServerPlayer;
import net.neoforged.neoforge.network.handling.IPayloadContext;

/** Client to server: buy a town charter from the open Merchant screen's Merchant, for a block of diamond. */
public record CharterPayload() implements CustomPacketPayload {
    public static final Type<CharterPayload> TYPE = new Type<>(Enterprise.id("charter"));
    public static final StreamCodec<ByteBuf, CharterPayload> STREAM_CODEC = StreamCodec.unit(new CharterPayload());

    @Override
    public Type<CharterPayload> type() {
        return TYPE;
    }

    static void handle(CharterPayload payload, IPayloadContext context) {
        context.enqueueWork(() -> {
            if (context.player() instanceof ServerPlayer player
                    && player.containerMenu instanceof MerchantMenu menu
                    && menu.stillValid(player)) {
                player.displayClientMessage(Component.translatable(Charters.buy(player.getInventory())
                        ? "message.enterprise.charter.bought" : "message.enterprise.charter.no_diamonds"), true);
            }
        });
    }
}
