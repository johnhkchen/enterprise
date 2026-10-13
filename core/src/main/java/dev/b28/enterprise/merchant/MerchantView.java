package dev.b28.enterprise.merchant;

import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.MoneyChanger;
import dev.ithundxr.createnumismatics.content.backend.BankAccount;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.ComponentSerialization;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.item.ItemStack;

/**
 * What the Merchant screen shows: the lockers a Merchant manages and each one's trades, the emerald rate, and the
 * settlement bank's balance ({@link #NO_BANK} without a Blaze Banker). Sent to the client once.
 */
public record MerchantView(BlockPos desk, int limit, List<LockerView> lockers, double rate, int bankSpurs) {
    public static final int NO_BANK = -1;

    public static final StreamCodec<RegistryFriendlyByteBuf, MerchantView> STREAM_CODEC = StreamCodec.composite(
            BlockPos.STREAM_CODEC, MerchantView::desk,
            ByteBufCodecs.VAR_INT, MerchantView::limit,
            LockerView.STREAM_CODEC.apply(ByteBufCodecs.list()), MerchantView::lockers,
            ByteBufCodecs.DOUBLE, MerchantView::rate,
            ByteBufCodecs.VAR_INT, MerchantView::bankSpurs,
            MerchantView::new);

    public record LockerView(BlockPos workstation, Component worker, List<TradeView> trades) {
        public static final StreamCodec<RegistryFriendlyByteBuf, LockerView> STREAM_CODEC = StreamCodec.composite(
                BlockPos.STREAM_CODEC, LockerView::workstation,
                ComponentSerialization.TRUSTED_STREAM_CODEC, LockerView::worker,
                TradeView.STREAM_CODEC.apply(ByteBufCodecs.list()), LockerView::trades,
                LockerView::new);
    }

    public record TradeView(ItemStack costA, ItemStack costB, ItemStack result, String key, boolean enabled) {
        public static final StreamCodec<RegistryFriendlyByteBuf, TradeView> STREAM_CODEC = StreamCodec.composite(
                ItemStack.OPTIONAL_STREAM_CODEC, TradeView::costA,
                ItemStack.OPTIONAL_STREAM_CODEC, TradeView::costB,
                ItemStack.OPTIONAL_STREAM_CODEC, TradeView::result,
                ByteBufCodecs.STRING_UTF8, TradeView::key,
                ByteBufCodecs.BOOL, TradeView::enabled,
                TradeView::new);
    }

    /** Builds the view for the Merchant at {@code desk}, managing up to {@code limit} lockers. */
    public static MerchantView build(ServerLevel level, BlockPos desk, int limit) {
        return build(level, desk, limit, Lockers.MERCHANT_RANGE);
    }

    /** As {@link #build(ServerLevel, BlockPos, int)}, searching {@code range} blocks around the desk. */
    public static MerchantView build(ServerLevel level, BlockPos desk, int limit, int range) {
        MerchantBook book = MerchantBook.get(level.getServer());
        GlobalPos deskPos = GlobalPos.of(level.dimension(), desk);
        List<LockerView> lockers = Lockers.near(level, desk, range, limit).stream()
                .map(locker -> {
                    Optional<Villager> worker = Merchant.workingAt(level, locker.workstation(), range);
                    List<TradeView> trades = worker.map(villager -> villager.getOffers().stream()
                                    .map(offer -> {
                                        String key = TradeKeys.of(offer);
                                        return new TradeView(
                                                offer.getBaseCostA().copy(),
                                                offer.getCostB().copy(),
                                                offer.getResult().copy(),
                                                key,
                                                book.isEnabled(deskPos, locker.workstation(), key));
                                    })
                                    .toList())
                            .orElse(List.of());
                    Component label = worker.<Component>map(Villager::getName)
                            .orElse(Component.translatable("screen.enterprise.merchant.unclaimed"));
                    return new LockerView(locker.workstation(), label, trades);
                })
                .toList();
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        int bankSpurs = MoneyChanger.bankAt(level, desk).map(BankAccount::getBalance).orElse(NO_BANK);
        return new MerchantView(desk, limit, lockers, rate, bankSpurs);
    }
}
