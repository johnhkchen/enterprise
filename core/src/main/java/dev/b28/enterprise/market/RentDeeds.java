package dev.b28.enterprise.market;

import dev.b28.enterprise.cargo.Cargo;
import dev.b28.enterprise.cargo.Papers;
import dev.b28.enterprise.contract.Rents;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Treasury;
import dev.ithundxr.createnumismatics.Numismatics;
import dev.ithundxr.createnumismatics.content.backend.BankAccount;
import dev.ithundxr.createnumismatics.content.bank.CardItem;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Block;
import net.neoforged.neoforge.event.entity.player.PlayerInteractEvent;

/**
 * Rents as paper a player can hold. A dealer (a freight hub's Merchant) quotes each rent it holds on a vendor by its
 * desk: selling the deed at its ask while the rent is its own; once a player has bought it, buying it back at its
 * bid, paid from a bank card the dealer keeps funded. The deed is the title: the dealer still collects the coupons,
 * in its keeping, and a player bringing the deed to its desk is handed the crop they've earned.
 */
public final class RentDeeds {
    public static final String DEED = "rent_deed";
    /** The row of rent shops by a dealer's desk: this far south of it, beyond the freight shops. */
    public static final int ROW = Freight.SHOP_ROW + 2;

    private RentDeeds() {}

    /** The deed to a rent: a document naming it. */
    public static ItemStack deed(Rents.Rent rent) {
        CompoundTag data = new CompoundTag();
        data.putInt("rent", rent.id());
        data.putString("summary", "%d %s a season from %s".formatted(rent.quantity(),
                BuiltInRegistries.ITEM.getKey(rent.crop()).getPath(),
                dev.b28.enterprise.settlement.Names.label(rent.farm())));
        ItemStack deed = new ItemStack(Cargo.DOCUMENTS.get());
        deed.set(Cargo.PAPERS.get(), Papers.NONE.with(new Papers.Document(DEED, data)));
        return deed;
    }

    /** The rent a stack is the deed to, if it is one. */
    public static Optional<Integer> rentOf(ItemStack stack) {
        if (!stack.is(Cargo.DOCUMENTS.get())) {
            return Optional.empty();
        }
        return Optional.ofNullable(stack.get(Cargo.PAPERS.get())).flatMap(p -> p.first(DEED))
                .map(d -> d.data().getInt("rent"));
    }

    /** Where a dealer's rent shops go: the row by its desk. */
    public static List<BlockPos> sites(BlockPos desk) {
        List<BlockPos> sites = new ArrayList<>();
        for (int i = 0; i < 12; i++) {
            sites.add(desk.offset(-6 + 2 * i, 0, ROW));
        }
        return sites;
    }

    /** As {@link #quote(ServerLevel, GlobalPos, MerchantStock, Mint, double, long, List)} in the row by its desk. */
    public static void quote(ServerLevel level, GlobalPos hub, MerchantStock stock, Mint mint, double rate, long now) {
        quote(level, hub, stock, mint, rate, now, sites(hub.pos()));
    }

    /**
     * The dealer's shops, brought up to date: a rent it holds gets a shop (on the first free site) selling its deed
     * at the ask; a shop whose deed has gone was bought by a player (its takings into the treasury), and buys it back
     * from then on at the bid; a shop holding the deed again was sold it (the rent is the dealer's own again).
     */
    public static void quote(ServerLevel level, GlobalPos hub, MerchantStock stock, Mint mint, double rate, long now,
            List<BlockPos> sites) {
        var server = level.getServer();
        Rents rents = Rents.get(server);
        for (Rents.Rent rent : rents.heldBy(hub)) {
            Optional<VendorBlockEntity> shop = rent.shop().filter(level::isLoaded)
                    .map(level::getBlockEntity).filter(VendorBlockEntity.class::isInstance).map(VendorBlockEntity.class::cast);
            if (rent.shop().isPresent() && shop.isEmpty()) {
                if (rent.shop().filter(level::isLoaded).isEmpty()) {
                    continue; // not loaded: next time
                }
                rents.setShop(rent, Optional.empty()); // broken: put up another
            }
            if (shop.isEmpty()) {
                shop = putUp(level, sites);
                if (shop.isEmpty()) {
                    return; // no room in the row
                }
                rents.setShop(rent, Optional.of(shop.get().getBlockPos()));
                Shops.offer(shop.get(), deed(rent), 0);
                Shops.stock(shop.get(), deed(rent));
            }
            VendorBlockEntity vendor = shop.get();
            double price = RentDealer.price(server, hub, rent);
            if (!rent.bearer()) {
                if (vendor.getMode() == VendorBlockEntity.Mode.SELL && Shops.lots(vendor) == 0) {
                    // A player bought the deed: the takings to the treasury, and the shop buys it back from now on.
                    int spurs = Shops.collect(vendor);
                    Treasury.deposit(level, hub.pos(), stock, spurs);
                    rents.setBearer(rent, true, spurs / rate, now);
                    buyBack(level, hub, stock, vendor, rent, price * (1 - dev.b28.enterprise.Dials.DEALER_SPREAD.get()), mint, rate, now);
                } else {
                    Shops.setPrice(vendor, (int) Math.ceil(price * (1 + dev.b28.enterprise.Dials.DEALER_SPREAD.get()) * rate));
                }
            } else {
                boolean handedBack = vendor.items.stream().anyMatch(s -> rentOf(s).filter(id -> id == rent.id()).isPresent());
                if (handedBack) {
                    // A player sold the deed back: the rent is the dealer's own again, and the shop sells it.
                    rents.setBearer(rent, false, price * (1 - dev.b28.enterprise.Dials.DEALER_SPREAD.get()), now);
                    vendor.cardContainer.setItem(0, ItemStack.EMPTY);
                    Shops.offer(vendor, deed(rent), (int) Math.ceil(price * (1 + dev.b28.enterprise.Dials.DEALER_SPREAD.get()) * rate));
                } else {
                    buyBack(level, hub, stock, vendor, rent, price * (1 - dev.b28.enterprise.Dials.DEALER_SPREAD.get()), mint, rate, now);
                }
            }
        }
    }

    /** The shop buys the deed back at {@code bid}, its card's account topped up to pay it from the treasury. */
    private static void buyBack(ServerLevel level, GlobalPos hub, MerchantStock stock, VendorBlockEntity vendor,
            Rents.Rent rent, double bid, Mint mint, double rate, long now) {
        int spurs = (int) Math.floor(bid * rate);
        java.util.UUID id = Rents.get(level.getServer()).dealerAccount(hub);
        BankAccount account = Numismatics.BANK.getOrCreateAccount(id, BankAccount.Type.PLAYER);
        if (account.getBalance() < spurs) {
            double short_ = (spurs - account.getBalance() + 1) / rate * 1.05; // a little over: coin rounds down
            double spare = Treasury.available(level, hub.pos(), stock, rate)
                    - dev.b28.enterprise.territory.Territory.get(level.getServer()).reserve(hub);
            if (spare >= short_) {
                Treasury.Withdrawal paid = Treasury.withdraw(level, hub.pos(), stock, short_, mint, now);
                account.deposit(paid.spurs() + mint.sell(Mint.DEFAULT_FACTION, paid.emeralds(), now));
            }
        }
        if (vendor.getMode() != VendorBlockEntity.Mode.BUY) {
            vendor.setMode(VendorBlockEntity.Mode.BUY);
            vendor.filterContainer.setItem(0, deed(rent));
            ItemStack card = new ItemStack(BuiltInRegistries.ITEM.get(ResourceLocation.parse("numismatics:red_card")));
            CardItem.set(card, id);
            vendor.cardContainer.setItem(0, card);
        }
        Shops.setPrice(vendor, spurs);
    }

    private static Optional<VendorBlockEntity> putUp(ServerLevel level, List<BlockPos> sites) {
        Block vendor = BuiltInRegistries.BLOCK.get(ResourceLocation.parse("numismatics:vendor"));
        for (BlockPos pos : sites) {
            if (level.isLoaded(pos) && level.getBlockState(pos).isAir()) {
                level.setBlock(pos, vendor.defaultBlockState(), 3);
                if (level.getBlockEntity(pos) instanceof VendorBlockEntity shop) {
                    return Optional.of(shop);
                }
            }
        }
        return Optional.empty();
    }

    /**
     * A player brings a deed to the dealer's desk: handed the crop collected for it, as much as the warehouse holds.
     * Returns how many.
     */
    public static int present(ServerLevel level, BlockPos desk, ItemStack deed, Player player) {
        GlobalPos hub = GlobalPos.of(level.dimension(), desk);
        Rents rents = Rents.get(level.getServer());
        Optional<Rents.Rent> rent = rentOf(deed).flatMap(rents::rent)
                .filter(r -> r.bearer() && r.holder().equals(hub));
        if (rent.isEmpty()) {
            return 0;
        }
        MerchantStock stock = MerchantStock.at(level, desk);
        int want = Math.min(rent.get().accrued(), stock.count(new ItemCost(rent.get().crop(), 1)));
        int handed = 0;
        for (ItemStack piece : stock.take(new ItemCost(rent.get().crop(), 1), want)) {
            handed += piece.getCount();
            if (!player.getInventory().add(piece)) {
                player.drop(piece, false);
            }
        }
        rents.takeAccrued(rent.get(), handed);
        int left = rent.get().accrued();
        player.displayClientMessage(Component.literal("%d %s handed over for your rent%s".formatted(handed,
                BuiltInRegistries.ITEM.getKey(rent.get().crop()).getPath(),
                left > 0 ? "; %d more owed, when it's in".formatted(left) : "")), false);
        return handed;
    }

    public static void onRightClick(PlayerInteractEvent.RightClickBlock event) {
        if (!(event.getLevel() instanceof ServerLevel level)
                || !level.getBlockState(event.getPos()).is(Merchant.DESK.get())
                || rentOf(event.getItemStack()).isEmpty()) {
            return;
        }
        present(level, event.getPos(), event.getItemStack(), event.getEntity());
        event.setCanceled(true);
    }
}
