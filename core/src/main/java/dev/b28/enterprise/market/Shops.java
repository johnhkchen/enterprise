package dev.b28.enterprise.market;

import dev.b28.enterprise.mixin.VendorAccess;
import dev.ithundxr.createnumismatics.Numismatics;
import dev.ithundxr.createnumismatics.content.backend.BankAccount;
import dev.ithundxr.createnumismatics.content.backend.Coin;
import dev.ithundxr.createnumismatics.content.backend.ReasonHolder;
import dev.ithundxr.createnumismatics.content.bank.CardItem;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.component.DataComponents;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.enchantment.ItemEnchantments;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.block.entity.BlockEntity;

/**
 * Numismatics vendors (shops) near a Merchant's Desk, and trading with them as a customer would. A vendor buying
 * goods is a standing order: it pays the price set on it, from the bank card in its card slot, for each lot (the
 * filter's count) handed to it. A vendor selling goods takes the price, into its coins or its deposit account, for
 * each lot taken. A vendor touching the desk is the town's own shop: its Merchant sets what it sells and collects its
 * takings.
 */
public final class Shops {
    /** Vendors this close to the desk are its market's shops; touching it, the town's own. */
    public static final int REACH = 16;
    public static final int OWN = 2;

    private Shops() {}

    /** The loaded vendors within {@link #REACH} of the desk, nearest first. */
    public static List<VendorBlockEntity> near(ServerLevel level, BlockPos desk) {
        List<VendorBlockEntity> found = new ArrayList<>();
        ChunkPos from = new ChunkPos(desk.offset(-REACH, 0, -REACH));
        ChunkPos to = new ChunkPos(desk.offset(REACH, 0, REACH));
        for (int cx = from.x; cx <= to.x; cx++) {
            for (int cz = from.z; cz <= to.z; cz++) {
                if (!level.hasChunk(cx, cz)) {
                    continue;
                }
                for (BlockEntity entity : level.getChunk(cx, cz).getBlockEntities().values()) {
                    if (entity instanceof VendorBlockEntity vendor && vendor.getBlockPos().closerThan(desk, REACH)
                            && !vendor.isCreativeVendor()) {
                        found.add(vendor);
                    }
                }
            }
        }
        found.sort(Comparator.comparingDouble(v -> v.getBlockPos().distSqr(desk)));
        return found;
    }

    /** Is this vendor the town's own (touching its desk)? */
    public static boolean own(VendorBlockEntity vendor, BlockPos desk) {
        return vendor.getBlockPos().closerThan(desk, OWN + 0.5);
    }

    /** Does the vendor's filter ask for enchantments (a commission, not a plain order)? */
    public static boolean enchanted(ItemStack filter) {
        return !filter.getOrDefault(DataComponents.ENCHANTMENTS, ItemEnchantments.EMPTY).isEmpty()
                || !filter.getOrDefault(DataComponents.STORED_ENCHANTMENTS, ItemEnchantments.EMPTY).isEmpty();
    }

    /** The bank account a buying vendor pays from: the one its card is bound to. */
    public static Optional<BankAccount> purse(VendorBlockEntity vendor) {
        ItemStack card = vendor.cardContainer.getItem(0);
        if (card.isEmpty() || !CardItem.isBound(card)) {
            return Optional.empty();
        }
        return Optional.ofNullable(Numismatics.BANK.getAccount(CardItem.get(card)));
    }

    /** How many more lots a buying vendor has room for. */
    public static int roomFor(VendorBlockEntity vendor) {
        ItemStack filter = vendor.getFilterItem();
        if (filter.isEmpty()) {
            return 0;
        }
        int room = 0;
        for (ItemStack slot : vendor.items) {
            if (slot.isEmpty()) {
                room += filter.getMaxStackSize();
            } else if (vendor.matchesFilterItem(slot)) {
                room += slot.getMaxStackSize() - slot.getCount();
            }
        }
        return room / filter.getCount();
    }

    /**
     * Hands a lot ({@code goods}, the filter's count) to a buying vendor and takes its price from the vendor's purse.
     * Returns the spurs paid, or 0 (and nothing handed over) if it can't pay or hasn't room.
     */
    public static int sellTo(VendorBlockEntity vendor, ItemStack goods) {
        int price = vendor.getTotalPrice();
        Optional<BankAccount> purse = purse(vendor);
        if (price <= 0 || purse.isEmpty() || roomFor(vendor) < 1 || !vendor.matchesFilterItem(goods)
                || !purse.get().deduct(price, new ReasonHolder())) {
            return 0;
        }
        ItemStack left = goods.copy();
        for (int i = 0; i < vendor.items.size() && !left.isEmpty(); i++) {
            ItemStack slot = vendor.items.get(i);
            if (slot.isEmpty()) {
                vendor.items.set(i, left.split(left.getMaxStackSize()));
            } else if (vendor.matchesFilterItem(slot) && ItemStack.isSameItemSameComponents(slot, left)) {
                int moved = Math.min(left.getCount(), slot.getMaxStackSize() - slot.getCount());
                slot.grow(moved);
                left.shrink(moved);
            }
        }
        vendor.setChanged();
        vendor.notifyUpdate();
        return price;
    }

    /** Lots a selling vendor has in stock. */
    public static int lots(VendorBlockEntity vendor) {
        ItemStack filter = vendor.getFilterItem();
        if (filter.isEmpty()) {
            return 0;
        }
        return vendor.items.stream().filter(vendor::matchesFilterItem).mapToInt(ItemStack::getCount).sum()
                / filter.getCount();
    }

    /** Takes a lot from a selling vendor, paying its price into the vendor. The caller pays from its treasury. */
    public static ItemStack buyFrom(VendorBlockEntity vendor) {
        ItemStack filter = vendor.getFilterItem();
        if (lots(vendor) < 1) {
            return ItemStack.EMPTY;
        }
        int wanted = filter.getCount();
        ItemStack taken = ItemStack.EMPTY;
        for (int i = 0; i < vendor.items.size() && wanted > 0; i++) {
            ItemStack slot = vendor.items.get(i);
            if (vendor.matchesFilterItem(slot)) {
                ItemStack piece = slot.split(Math.min(wanted, slot.getCount()));
                wanted -= piece.getCount();
                if (taken.isEmpty()) {
                    taken = piece;
                } else {
                    taken.grow(piece.getCount());
                }
            }
        }
        pay(vendor, vendor.getTotalPrice());
        vendor.setChanged();
        vendor.notifyUpdate();
        return taken;
    }

    /** Pays {@code spurs} into a vendor, in the largest coins that fit. */
    static void pay(VendorBlockEntity vendor, int spurs) {
        Coin[] coins = Coin.values();
        for (int i = coins.length - 1; i >= 0 && spurs > 0; i--) {
            int count = spurs / coins[i].value;
            if (count > 0) {
                vendor.addCoin(coins[i], count);
                spurs -= count * coins[i].value;
            }
        }
    }

    /** Sets a vendor (the town's own) to sell {@code lot} for {@code spurs}, in the largest coins that make it. */
    public static void offer(VendorBlockEntity vendor, ItemStack lot, int spurs) {
        vendor.setMode(VendorBlockEntity.Mode.SELL);
        vendor.filterContainer.setItem(0, lot.copy());
        setPrice(vendor, spurs);
    }

    /** Sets a vendor's price to {@code spurs}, in the largest coins that make it. */
    public static void setPrice(VendorBlockEntity vendor, int spurs) {
        Coin[] coins = Coin.values();
        for (int i = coins.length - 1; i >= 0; i--) {
            int count = spurs / coins[i].value;
            vendor.setPrice(coins[i], count);
            spurs -= count * coins[i].value;
        }
        vendor.setChanged();
        vendor.notifyUpdate();
    }

    /** Puts goods in a vendor's stock (the town's own shop), as many as fit; returns the rest. */
    public static ItemStack stock(VendorBlockEntity vendor, ItemStack goods) {
        ItemStack left = goods.copy();
        for (int i = 0; i < vendor.items.size() && !left.isEmpty(); i++) {
            ItemStack slot = vendor.items.get(i);
            if (slot.isEmpty()) {
                vendor.items.set(i, left.split(left.getMaxStackSize()));
            } else if (ItemStack.isSameItemSameComponents(slot, left)) {
                int moved = Math.min(left.getCount(), slot.getMaxStackSize() - slot.getCount());
                slot.grow(moved);
                left.shrink(moved);
            }
        }
        vendor.setChanged();
        return left;
    }

    /** Takes everything a vendor holds that isn't what it now sells (the town's own shop, after a change of goods). */
    public static List<ItemStack> clearOthers(VendorBlockEntity vendor) {
        List<ItemStack> out = new ArrayList<>();
        for (int i = 0; i < vendor.items.size(); i++) {
            ItemStack slot = vendor.items.get(i);
            if (!slot.isEmpty() && !vendor.matchesFilterItem(slot)) {
                out.add(slot);
                vendor.items.set(i, ItemStack.EMPTY);
            }
        }
        return out;
    }

    /** The coins a vendor took from its sales, emptied out, in spurs. */
    public static int collect(VendorBlockEntity vendor) {
        var coins = ((VendorAccess) vendor).enterprise$coins();
        int spurs = coins.getValue();
        coins.clear();
        vendor.setChanged();
        return spurs;
    }
}
