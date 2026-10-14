package dev.b28.enterprise.merchant;

import dev.b28.enterprise.locker.Lockers;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.BlockPos;
import net.minecraft.core.NonNullList;
import net.minecraft.core.component.DataComponents;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.food.FoodProperties;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.component.ItemContainerContents;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.ShulkerBoxBlock;
import net.neoforged.neoforge.items.IItemHandlerModifiable;
import net.neoforged.neoforge.items.ItemHandlerHelper;
import net.neoforged.neoforge.items.ItemStackHandler;
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

    /** Slots every warehouse keeps free for locker results and coins: deliveries and harvests stop short of them. */
    public static final int FREE_SLOTS = 5;

    /** Empty loose slots. */
    public int freeSlots() {
        int free = 0;
        for (int slot = 0; slot < handler.getSlots(); slot++) {
            if (handler.getStackInSlot(slot).isEmpty()) {
                free++;
            }
        }
        return free;
    }

    /** Whether {@code stack} can go in without dipping into the slots kept free (merging into a stack is fine). */
    public boolean hasRoomFor(ItemStack stack) {
        int merge = 0;
        for (int slot = 0; slot < handler.getSlots(); slot++) {
            ItemStack held = handler.getStackInSlot(slot);
            if (ItemStack.isSameItemSameComponents(held, stack)) {
                merge += held.getMaxStackSize() - held.getCount();
            }
        }
        int newStacks = (int) Math.ceil(Math.max(0, stack.getCount() - merge) / (double) stack.getMaxStackSize());
        return freeSlots() - newStacks >= FREE_SLOTS;
    }

    /** Loose slots in the warehouse's chests. */
    public int slots() {
        return handler.getSlots();
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

    /** Copies of the stacks matching {@code cost}, loose and in shulker boxes: to read, not to take. */
    public List<ItemStack> copies(ItemCost cost) {
        List<ItemStack> found = new java.util.ArrayList<>();
        for (int slot = 0; slot < handler.getSlots(); slot++) {
            ItemStack stack = handler.getStackInSlot(slot);
            if (cost.test(stack)) {
                found.add(stack.copy());
            } else if (isShulkerBox(stack)) {
                contents(stack).stream().filter(cost::test).forEach(inner -> found.add(inner.copy()));
            }
        }
        return found;
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

    /** Whether all of {@code stacks} fit in loose slots, tried on a copy so nothing moves. */
    public boolean fits(List<ItemStack> stacks) {
        ItemStackHandler copy = new ItemStackHandler(handler.getSlots());
        for (int slot = 0; slot < handler.getSlots(); slot++) {
            copy.setStackInSlot(slot, handler.getStackInSlot(slot).copy());
        }
        for (ItemStack stack : stacks) {
            if (!ItemHandlerHelper.insertItemStacked(copy, stack.copy(), false).isEmpty()) {
                return false;
            }
        }
        return true;
    }

    /** Food points (nutrition) in the stock, loose and in shulker boxes. */
    public int foodNutrition() {
        int total = 0;
        for (int slot = 0; slot < handler.getSlots(); slot++) {
            ItemStack stack = handler.getStackInSlot(slot);
            if (isShulkerBox(stack)) {
                for (ItemStack inner : contents(stack)) {
                    total += nutrition(inner) * inner.getCount();
                }
            } else {
                total += nutrition(stack) * stack.getCount();
            }
        }
        return total;
    }

    /**
     * Eats at least {@code points} of nutrition from the stock, whole items at a time, loose food first. Returns what
     * was eaten, item by item; eats nothing and returns an empty map if the stock holds too little.
     */
    public Map<Item, Integer> eat(int points) {
        Map<Item, Integer> eaten = new java.util.LinkedHashMap<>();
        if (points <= 0 || foodNutrition() < points) {
            return eaten;
        }
        int got = 0;
        for (int slot = 0; slot < handler.getSlots() && got < points; slot++) {
            ItemStack stack = handler.getStackInSlot(slot);
            int each = nutrition(stack);
            if (each > 0) {
                int items = Math.min(stack.getCount(), (points - got + each - 1) / each);
                eaten.merge(stack.getItem(), items, Integer::sum);
                handler.extractItem(slot, items, false);
                got += items * each;
            }
        }
        for (int slot = 0; slot < handler.getSlots() && got < points; slot++) {
            ItemStack box = handler.getStackInSlot(slot);
            if (!isShulkerBox(box)) {
                continue;
            }
            NonNullList<ItemStack> inner = contents(box);
            for (ItemStack food : inner) {
                int each = nutrition(food);
                if (each > 0 && got < points) {
                    int items = Math.min(food.getCount(), (points - got + each - 1) / each);
                    eaten.merge(food.getItem(), items, Integer::sum);
                    food.shrink(items);
                    got += items * each;
                }
            }
            handler.setStackInSlot(slot, withContents(box, inner));
        }
        return eaten;
    }

    /** Nutrition as road food and rations. Comforts (cake, cookies, pies) are kept for the town, never eaten as food. */
    private static int nutrition(ItemStack stack) {
        FoodProperties food = stack.get(DataComponents.FOOD);
        return food == null || dev.b28.enterprise.territory.Comforts.isComfort(stack) ? 0 : food.nutrition();
    }

    /** Removes and returns every loose stack matching {@code filter} (shulker contents are left alone). */
    public List<ItemStack> removeLoose(java.util.function.Predicate<ItemStack> filter) {
        List<ItemStack> removed = new ArrayList<>();
        for (int slot = 0; slot < handler.getSlots(); slot++) {
            ItemStack stack = handler.getStackInSlot(slot);
            if (!stack.isEmpty() && filter.test(stack)) {
                removed.add(stack.copy());
                handler.setStackInSlot(slot, ItemStack.EMPTY);
            }
        }
        return removed;
    }

    /** Every loose stack in the stock, for reading (shulker boxes appear as boxes). */
    public List<ItemStack> looseStacks() {
        List<ItemStack> stacks = new ArrayList<>();
        for (int slot = 0; slot < handler.getSlots(); slot++) {
            stacks.add(handler.getStackInSlot(slot));
        }
        return stacks;
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
