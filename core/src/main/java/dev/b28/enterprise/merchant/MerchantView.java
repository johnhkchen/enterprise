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
 * What the Merchant screen shows: the lockers a Merchant manages and each one's trades, the emerald rate, the
 * settlement bank's balance ({@link #NO_BANK} without a Blaze Banker), and the caravans: the markets this one knows
 * and its squad at home. Sent to the client once.
 */
public record MerchantView(BlockPos desk, int limit, List<LockerView> lockers, double rate, int bankSpurs,
        CaravanView caravans) {
    public static final int NO_BANK = -1;

    public static final StreamCodec<RegistryFriendlyByteBuf, MerchantView> STREAM_CODEC = StreamCodec.composite(
            BlockPos.STREAM_CODEC, MerchantView::desk,
            ByteBufCodecs.VAR_INT, MerchantView::limit,
            LockerView.STREAM_CODEC.apply(ByteBufCodecs.list()), MerchantView::lockers,
            ByteBufCodecs.DOUBLE, MerchantView::rate,
            ByteBufCodecs.VAR_INT, MerchantView::bankSpurs,
            CaravanView.STREAM_CODEC, MerchantView::caravans,
            MerchantView::new);

    /** The markets this desk knows, and the trades of its squad members at home (e.g. "minecraft:librarian"). */
    public record CaravanView(List<TownView> towns, List<String> squad) {
        public static final StreamCodec<RegistryFriendlyByteBuf, CaravanView> STREAM_CODEC = StreamCodec.composite(
                TownView.STREAM_CODEC.apply(ByteBufCodecs.list()), CaravanView::towns,
                ByteBufCodecs.STRING_UTF8.apply(ByteBufCodecs.list()), CaravanView::squad,
                CaravanView::new);
    }

    /**
     * A market this desk knows: its desk, blocks away and which way (N, NE, ...), whether it's a settlement, whether
     * this desk sends caravans there, and the route's status.
     */
    public record TownView(BlockPos desk, int distance, String direction, boolean settlement, boolean routed,
            String status) {
        /** Can a caravan carry the food to get there? */
        public boolean inRange() {
            return dev.b28.enterprise.caravan.Caravans.inRange(distance);
        }

        public static final StreamCodec<RegistryFriendlyByteBuf, TownView> STREAM_CODEC = StreamCodec.composite(
                BlockPos.STREAM_CODEC, TownView::desk,
                ByteBufCodecs.VAR_INT, TownView::distance,
                ByteBufCodecs.STRING_UTF8, TownView::direction,
                ByteBufCodecs.BOOL, TownView::settlement,
                ByteBufCodecs.BOOL, TownView::routed,
                ByteBufCodecs.STRING_UTF8, TownView::status,
                TownView::new);
    }

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
        return new MerchantView(desk, limit, lockers, rate, bankSpurs, caravans(level, deskPos, range));
    }

    /** The markets this desk knows, nearest first, with its routes to them; and its squad at home. */
    public static CaravanView caravans(ServerLevel level, GlobalPos desk, int range) {
        var server = level.getServer();
        var ledger = dev.b28.enterprise.caravan.CaravanLedger.get(server);
        var territory = dev.b28.enterprise.territory.Territory.get(server);
        List<TownView> towns = dev.b28.enterprise.caravan.Atlas.get(server).known(server, desk).stream()
                .map(other -> {
                    int dx = other.pos().getX() - desk.pos().getX();
                    int dz = other.pos().getZ() - desk.pos().getZ();
                    int distance = (int) Math.ceil(Math.sqrt((double) dx * dx + (double) dz * dz));
                    var route = ledger.route(desk, other);
                    return new TownView(other.pos(), distance, direction(dx, dz), territory.bellOf(other).isPresent(),
                            route.isPresent(), route.map(dev.b28.enterprise.caravan.Route::status).orElse(""));
                })
                .toList();
        List<String> squad = dev.b28.enterprise.caravan.Squad.available(level, desk, Math.min(range, dev.b28.enterprise.caravan.Squad.reach())).stream()
                .map(dev.b28.enterprise.caravan.Squad::trade)
                .toList();
        return new CaravanView(towns, squad);
    }

    /** Which way, on the eight points of the compass (north is -z). */
    static String direction(int dx, int dz) {
        String[] points = {"E", "SE", "S", "SW", "W", "NW", "N", "NE"};
        double angle = Math.toDegrees(Math.atan2(dz, dx));
        return points[(int) Math.floorMod(Math.round(angle / 45), 8)];
    }
}
