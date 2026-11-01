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

    private final IItemHandlerModifiable handler; // every store, the desk's chests first
    private final List<Part> parts; // each store: the desk's chests (no kind), and each building with its kind

    /** One store of the stock: the desk's own chests (kind null), or a building's. */
    private record Part(Depots.Kind kind, IItemHandlerModifiable handler) {
        boolean general() {
            return kind == null || kind == Depots.Kind.STOREHOUSE;
        }
    }

    private MerchantStock(IItemHandlerModifiable handler) {
        this.handler = handler;
        this.parts = List.of(new Part(null, handler));
    }

    private MerchantStock(List<Part> parts) {
        this.parts = List.copyOf(parts);
        this.handler = parts.size() == 1 ? parts.get(0).handler()
                : new CombinedInvWrapper(parts.stream().map(Part::handler).toArray(IItemHandlerModifiable[]::new));
    }

    public static MerchantStock at(Level level, BlockPos desk) {
        if (level instanceof net.minecraft.server.level.ServerLevel server) {
            OffMap offMap = OffMap.get(server.getServer());
            var at = net.minecraft.core.GlobalPos.of(level.dimension(), desk);
            if (offMap.is(at)) {
                return new MerchantStock(offMap.stock(at)); // a market off the map: its warehouse is on the ledger
            }
        }
        List<Part> parts = new ArrayList<>();
        parts.add(new Part(null, (CombinedInvWrapper) Lockers.inventory(level, Lockers.touchingContainers(level, desk))));
        if (level instanceof net.minecraft.server.level.ServerLevel server) {
            // And the buildings it keeps goods in, every container in each one's floorplan.
            for (Depots.Depot depot : Depots.get(server.getServer()).of(net.minecraft.core.GlobalPos.of(level.dimension(), desk))) {
                if (depot.loaded(server)) {
                    parts.add(new Part(depot.kind(), (CombinedInvWrapper) Lockers.inventory(level, depot.containers(server))));
                }
            }
        }
        return new MerchantStock(parts);
    }

    /** Slots every warehouse keeps free for locker results and coins: deliveries and harvests stop short of them. */
    public static final int FREE_SLOTS = 5;

    /** Empty loose slots in the desk's chests and storehouses (not those kept for particular goods). */
    public int freeSlots() {
        return parts.stream().filter(Part::general).mapToInt(p -> free(p.handler())).sum();
    }

    private static int free(IItemHandlerModifiable handler) {
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
        int kept = 0; // free slots in buildings meant for these goods: no need to keep any of those free
        for (Part part : parts) {
            if (!part.general() && !part.kind().prefers(stack)) {
                continue;
            }
            for (int slot = 0; slot < part.handler().getSlots(); slot++) {
                ItemStack held = part.handler().getStackInSlot(slot);
                if (ItemStack.isSameItemSameComponents(held, stack)) {
                    merge += held.getMaxStackSize() - held.getCount();
                }
            }
            if (!part.general()) {
                kept += free(part.handler());
            }
        }
        int newStacks = (int) Math.ceil(Math.max(0, stack.getCount() - merge) / (double) stack.getMaxStackSize());
        return kept + Math.max(0, freeSlots() - FREE_SLOTS) >= newStacks;
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
            if (isBox(stack)) {
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
            } else if (isBox(stack)) {
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
            } else if (isBox(stack)) {
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
            if (!isBox(box)) {
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
        // A building meant for these goods first, then the desk's chests and the storehouses; never one meant for others.
        ItemStack rest = stack;
        for (Part part : parts) {
            if (!part.general() && part.kind().prefers(rest) && !rest.isEmpty()) {
                rest = ItemHandlerHelper.insertItemStacked(part.handler(), rest, false);
            }
        }
        for (Part part : parts) {
            if (part.general() && !rest.isEmpty()) {
                rest = ItemHandlerHelper.insertItemStacked(part.handler(), rest, false);
            }
        }
        for (int slot = 0; slot < handler.getSlots() && !rest.isEmpty(); slot++) {
            ItemStack box = handler.getStackInSlot(slot);
            if (!isBox(box) || !rest.getItem().canFitInsideContainerItems()) {
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
            if (isBox(stack)) {
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
            if (!isBox(box)) {
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
    public static int nutrition(ItemStack stack) {
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

    /**
     * A box the stock reaches into: a shulker box, or a storage crate (packed with goods and no papers: a crate with a
     * bill of lading is a shipment, someone's goods, and stays shut).
     */
    private static boolean isBox(ItemStack stack) {
        if (dev.b28.enterprise.cargo.Cargo.isPackage(stack)) { // a package of one of the core's pallets: never stock
            return false;
        }
        if (dev.b28.enterprise.cargo.Crates.isCrate(stack)) {
            return stack.getCount() == 1 && dev.b28.enterprise.cargo.Crates.papers(stack).documents().isEmpty()
                    && !dev.b28.enterprise.cargo.Crates.goods(stack).isEmpty();
        }
        return stack.getItem() instanceof BlockItem item && item.getBlock() instanceof ShulkerBoxBlock;
    }

    private static NonNullList<ItemStack> contents(ItemStack box) {
        if (dev.b28.enterprise.cargo.Crates.isCrate(box)) {
            NonNullList<ItemStack> items = NonNullList.withSize(dev.b28.enterprise.cargo.Crates.GOODS, ItemStack.EMPTY);
            List<ItemStack> goods = dev.b28.enterprise.cargo.Crates.goods(box);
            for (int i = 0; i < goods.size(); i++) {
                items.set(i, goods.get(i));
            }
            return items;
        }
        NonNullList<ItemStack> items = NonNullList.withSize(SHULKER_SLOTS, ItemStack.EMPTY);
        box.getOrDefault(DataComponents.CONTAINER, ItemContainerContents.EMPTY).copyInto(items);
        return items;
    }

    private static ItemStack withContents(ItemStack box, NonNullList<ItemStack> items) {
        if (dev.b28.enterprise.cargo.Crates.isCrate(box)) { // emptied, a crate is an empty crate again
            return dev.b28.enterprise.cargo.Crates.of(items.stream().filter(i -> !i.isEmpty()).toList(),
                    dev.b28.enterprise.cargo.Papers.NONE);
        }
        ItemStack copy = box.copy();
        copy.set(DataComponents.CONTAINER, ItemContainerContents.fromItems(items));
        return copy;
    }
}
