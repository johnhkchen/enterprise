package dev.b28.enterprise.merchant;

import dev.b28.enterprise.locker.Lockers;
import java.util.Map;
import java.util.TreeMap;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.BlockPos;
import net.minecraft.core.NonNullList;
import net.minecraft.core.component.DataComponents;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.component.ItemContainerContents;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.ShulkerBoxBlock;
import net.neoforged.neoforge.items.IItemHandlerModifiable;
import net.neoforged.neoforge.items.ItemHandlerHelper;
import net.neoforged.neoforge.items.wrapper.CombinedInvWrapper;

/**
 * A Merchant's stock: the containers touching its desk, including the contents of shulker boxes kept in them.
 * Loose items are used before anything inside a shulker box.
 */
public final class MerchantStock {
    private static final int SHULKER_SLOTS = 27;

    private final IItemHandlerModifiable handler;

    private MerchantStock(IItemHandlerModifiable handler) {
        this.handler = handler;
    }

    public static MerchantStock at(Level level, BlockPos desk) {
        return new MerchantStock((CombinedInvWrapper) Lockers.inventory(level, Lockers.touchingContainers(level, desk)));
    }

    public boolean isEmpty() {
        return handler.getSlots() == 0;
    }

    /** Item id to total count, loose and in shulker boxes; shulker boxes themselves are not counted. */
    public Map<String, Integer> totals() {
        Map<String, Integer> totals = new TreeMap<>();
        for (int slot = 0; slot < handler.getSlots(); slot++) {
            ItemStack stack = handler.getStackInSlot(slot);
            if (isShulkerBox(stack)) {
                contents(stack).forEach(inner -> add(totals, inner));
            } else {
                add(totals, stack);
            }
        }
        return totals;
    }

    private static void add(Map<String, Integer> totals, ItemStack stack) {
        if (!stack.isEmpty()) {
            totals.merge(BuiltInRegistries.ITEM.getKey(stack.getItem()).toString(), stack.getCount(), Integer::sum);
        }
    }

    /** How many items matching {@code cost} the stock holds, loose and in shulker boxes. */
    public int count(ItemCost cost) {
        int total = 0;
        for (int slot = 0; slot < handler.getSlots(); slot++) {
            ItemStack stack = handler.getStackInSlot(slot);
            if (cost.test(stack)) {
                total += stack.getCount();
            } else if (isShulkerBox(stack)) {
                for (ItemStack inner : contents(stack)) {
                    if (cost.test(inner)) {
                        total += inner.getCount();
                    }
                }
            }
        }
        return total;
    }

    /** Removes up to {@code amount} items matching {@code cost}; returns what was taken, in stack-sized pieces. */
    public NonNullList<ItemStack> take(ItemCost cost, int amount) {
        NonNullList<ItemStack> taken = NonNullList.create();
        for (int slot = 0; slot < handler.getSlots() && amount > 0; slot++) {
            ItemStack stack = handler.getStackInSlot(slot);
            if (cost.test(stack)) {
                ItemStack piece = handler.extractItem(slot, Math.min(amount, stack.getCount()), false);
                amount -= piece.getCount();
                taken.add(piece);
            }
        }
        for (int slot = 0; slot < handler.getSlots() && amount > 0; slot++) {
            ItemStack box = handler.getStackInSlot(slot);
            if (!isShulkerBox(box)) {
                continue;
            }
            NonNullList<ItemStack> inner = contents(box);
            for (int i = 0; i < inner.size() && amount > 0; i++) {
                if (cost.test(inner.get(i))) {
                    ItemStack piece = inner.get(i).split(amount);
                    amount -= piece.getCount();
                    taken.add(piece);
                }
            }
            handler.setStackInSlot(slot, withContents(box, inner));
        }
        return taken;
    }

    /** Puts {@code stack} into loose slots first, then into shulker boxes with room. Returns what didn't fit. */
    public ItemStack insert(ItemStack stack) {
        ItemStack rest = ItemHandlerHelper.insertItemStacked(handler, stack, false);
        for (int slot = 0; slot < handler.getSlots() && !rest.isEmpty(); slot++) {
            ItemStack box = handler.getStackInSlot(slot);
            if (!isShulkerBox(box) || !rest.getItem().canFitInsideContainerItems()) {
                continue;
            }
            NonNullList<ItemStack> inner = contents(box);
            for (int i = 0; i < inner.size() && !rest.isEmpty(); i++) {
                ItemStack existing = inner.get(i);
                if (existing.isEmpty()) {
                    inner.set(i, rest);
                    rest = ItemStack.EMPTY;
                } else if (ItemStack.isSameItemSameComponents(existing, rest)) {
                    int moved = Math.min(rest.getCount(), existing.getMaxStackSize() - existing.getCount());
                    existing.grow(moved);
                    rest.shrink(moved);
                }
            }
            handler.setStackInSlot(slot, withContents(box, inner));
        }
        return rest;
    }

    private static boolean isShulkerBox(ItemStack stack) {
        return stack.getItem() instanceof BlockItem item && item.getBlock() instanceof ShulkerBoxBlock;
    }

    private static NonNullList<ItemStack> contents(ItemStack box) {
        NonNullList<ItemStack> items = NonNullList.withSize(SHULKER_SLOTS, ItemStack.EMPTY);
        box.getOrDefault(DataComponents.CONTAINER, ItemContainerContents.EMPTY).copyInto(items);
        return items;
    }

    private static ItemStack withContents(ItemStack box, NonNullList<ItemStack> items) {
        ItemStack copy = box.copy();
        copy.set(DataComponents.CONTAINER, ItemContainerContents.fromItems(items));
        return copy;
    }
}
