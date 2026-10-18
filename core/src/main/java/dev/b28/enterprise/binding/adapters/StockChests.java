package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.money.MoneyChanger;
import dev.ithundxr.createnumismatics.content.coins.CoinItem;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.core.NonNullList;
import net.minecraft.core.component.DataComponents;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.component.ItemContainerContents;
import net.minecraft.world.level.block.ShulkerBoxBlock;
import net.neoforged.neoforge.capabilities.Capabilities;
import net.neoforged.neoforge.items.IItemHandler;
import net.neoforged.neoforge.items.IItemHandlerModifiable;
import net.neoforged.neoforge.items.ItemHandlerHelper;

/**
 * A trade point's chests in the world, the Minecraft half of the stock (docs/binding-plan.md 2, concern 3): the
 * containers touching a desk, or the one container a depot is; what they hold, loose and in the shulker boxes kept in
 * them; how many of their slots stand empty; items put in and taken out. No rules: what moves is the core's ({@code
 * Moved}); where in the chests is Minecraft's, as the mod's {@code merchant/MerchantStock} has it (loose stacks first,
 * merging into the good's stacks, then shulker boxes with room; taken loose first, then out of boxes).
 *
 * <p>Crates (the mod's cargo items) aren't opened: a crate is a handling unit, one of the core's pallets, which {@code
 * pallet-views} draws from the core; its goods aren't the chests' loose stock. Items that aren't goods (dirt, coins,
 * crates) are left alone and take their slots.
 */
final class StockChests {
    private static final int BOX_SLOTS = 27;

    private final List<IItemHandlerModifiable> handlers;

    private StockChests(List<IItemHandlerModifiable> handlers) {
        this.handlers = handlers;
    }

    /**
     * The chests of a point of {@code kind} at {@code pos}: a desk's touching containers, a locker's (a workstation's)
     * the same, a depot's own container.
     */
    static StockChests at(ServerLevel level, PointsAdapter.Kind kind, BlockPos pos) {
        List<BlockPos> containers = switch (kind) {
            case DESK, LOCKER -> Lockers.touchingContainers(level, pos);
            case DEPOT -> isContainer(level, pos) ? List.of(Lockers.canonicalContainer(pos, level.getBlockState(pos))) : List.of();
            default -> List.of();
        };
        List<IItemHandlerModifiable> handlers = new ArrayList<>();
        for (BlockPos at : containers) {
            IItemHandler handler = level.getCapability(Capabilities.ItemHandler.BLOCK, at, null);
            if (handler instanceof IItemHandlerModifiable modifiable) {
                handlers.add(modifiable);
            }
        }
        return new StockChests(handlers);
    }

    /** A container a depot can be (a chest, a barrel, a shulker box...: the lockers' containers). */
    static boolean isContainer(ServerLevel level, BlockPos pos) {
        return level.getBlockState(pos).is(Lockers.CONTAINERS);
    }

    /** No container at all (a desk with no chest touching it, a depot's broken). */
    boolean none() {
        return handlers.isEmpty();
    }

    /** Every item they hold, loose and in shulker boxes, by item, in the order first found. Boxes themselves aren't counted. */
    Map<Item, Long> held() {
        Map<Item, Long> held = new LinkedHashMap<>();
        for (IItemHandlerModifiable h : handlers) {
            for (int slot = 0; slot < h.getSlots(); slot++) {
                ItemStack stack = h.getStackInSlot(slot);
                if (isBox(stack)) {
                    for (ItemStack inner : contents(stack)) {
                        add(held, inner);
                    }
                } else {
                    add(held, stack);
                }
            }
        }
        return held;
    }

    private static void add(Map<Item, Long> held, ItemStack stack) {
        if (!stack.isEmpty()) {
            held.merge(stack.getItem(), (long) stack.getCount(), Long::sum);
        }
    }

    /** Empty loose slots: room for whole stacks. */
    int free() {
        int free = 0;
        for (IItemHandlerModifiable h : handlers) {
            for (int slot = 0; slot < h.getSlots(); slot++) {
                if (h.getStackInSlot(slot).isEmpty()) {
                    free++;
                }
            }
        }
        return free;
    }

    /** Puts {@code count} of {@code item} in: loose first (merging, then empty slots), then into boxes. Returns what didn't fit. */
    long put(Item item, long count) {
        int max = Math.max(1, item.getDefaultMaxStackSize());
        long left = count;
        while (left > 0) {
            int piece = (int) Math.min(left, max);
            ItemStack rest = new ItemStack(item, piece);
            for (IItemHandlerModifiable h : handlers) {
                if (rest.isEmpty()) {
                    break;
                }
                rest = ItemHandlerHelper.insertItemStacked(h, rest, false);
            }
            if (!rest.isEmpty()) {
                rest = intoBoxes(rest);
            }
            left -= piece - rest.getCount();
            if (!rest.isEmpty()) {
                break; // no room for the rest
            }
        }
        return left;
    }

    private ItemStack intoBoxes(ItemStack rest) {
        if (!rest.getItem().canFitInsideContainerItems()) {
            return rest;
        }
        for (IItemHandlerModifiable h : handlers) {
            for (int slot = 0; slot < h.getSlots() && !rest.isEmpty(); slot++) {
                ItemStack box = h.getStackInSlot(slot);
                if (!isBox(box)) {
                    continue;
                }
                NonNullList<ItemStack> inner = contents(box);
                boolean changed = false;
                for (int i = 0; i < inner.size() && !rest.isEmpty(); i++) {
                    ItemStack held = inner.get(i);
                    if (held.isEmpty()) {
                        inner.set(i, rest);
                        rest = ItemStack.EMPTY;
                        changed = true;
                    } else if (ItemStack.isSameItemSameComponents(held, rest) && held.getCount() < held.getMaxStackSize()) {
                        int moved = Math.min(rest.getCount(), held.getMaxStackSize() - held.getCount());
                        held.grow(moved);
                        rest.shrink(moved);
                        changed = true;
                    }
                }
                if (changed) {
                    h.setStackInSlot(slot, withContents(box, inner));
                }
            }
        }
        return rest;
    }

    /** Takes {@code count} of {@code item} out: loose first, then out of boxes. Returns what wasn't there to take. */
    long take(Item item, long count) {
        long left = count;
        for (IItemHandlerModifiable h : handlers) {
            for (int slot = 0; slot < h.getSlots() && left > 0; slot++) {
                ItemStack stack = h.getStackInSlot(slot);
                if (stack.is(item)) {
                    left -= h.extractItem(slot, (int) Math.min(left, stack.getCount()), false).getCount();
                }
            }
        }
        for (IItemHandlerModifiable h : handlers) {
            for (int slot = 0; slot < h.getSlots() && left > 0; slot++) {
                ItemStack box = h.getStackInSlot(slot);
                if (!isBox(box)) {
                    continue;
                }
                NonNullList<ItemStack> inner = contents(box);
                boolean changed = false;
                for (ItemStack held : inner) {
                    if (left > 0 && held.is(item)) {
                        int k = (int) Math.min(left, held.getCount());
                        held.shrink(k);
                        left -= k;
                        changed = true;
                    }
                }
                if (changed) {
                    h.setStackInSlot(slot, withContents(box, inner));
                }
            }
        }
        return left;
    }

    /** Kinds of Numismatics coin (spur, bevel, sprocket, cog, crown, sun): a fresh set of coin is at most this many slots. */
    private static final int COIN_KINDS = 6;

    /** Spurs of Numismatics coin loose in the chests (coin in shulker boxes isn't counted, as the mod's money changer has it). */
    long coinSpurs() {
        long spurs = 0;
        for (IItemHandlerModifiable h : handlers) {
            for (int slot = 0; slot < h.getSlots(); slot++) {
                if (h.getStackInSlot(slot).getItem() instanceof CoinItem coin) {
                    spurs += coin.coin.toSpurs(h.getStackInSlot(slot).getCount());
                }
            }
        }
        return spurs;
    }

    /** Slots coin could take: those it holds now, and the empty ones. */
    private int coinSlots() {
        int slots = 0;
        for (IItemHandlerModifiable h : handlers) {
            for (int slot = 0; slot < h.getSlots(); slot++) {
                ItemStack stack = h.getStackInSlot(slot);
                if (stack.isEmpty() || stack.getItem() instanceof CoinItem) {
                    slots++;
                }
            }
        }
        return slots;
    }

    /** Whether the chests could take a fresh set of coin, one stack of each kind. */
    boolean coinRoom() {
        return coinSlots() >= COIN_KINDS;
    }

    /**
     * Changes the coin loose in the chests by {@code delta} spurs, minting all of it again as the fewest coins so it stays
     * a few stacks. All or nothing for what fits: if the new coin needs more slots than coin and empty slots make, nothing
     * is done. Returns what wasn't done: {@code delta} itself when it didn't fit, or (below nothing) the spurs there weren't
     * to take, when the chests held less than {@code -delta}.
     */
    long mint(long delta) {
        long held = coinSpurs();
        long target = Math.max(0, held + delta);
        List<ItemStack> coins = MoneyChanger.coins((int) Math.min(Integer.MAX_VALUE, target));
        if (coins.size() > coinSlots()) {
            return delta;
        }
        for (IItemHandlerModifiable h : handlers) {
            for (int slot = 0; slot < h.getSlots(); slot++) {
                if (h.getStackInSlot(slot).getItem() instanceof CoinItem) {
                    h.setStackInSlot(slot, ItemStack.EMPTY);
                }
            }
        }
        for (ItemStack stack : coins) {
            ItemStack rest = stack;
            for (IItemHandlerModifiable h : handlers) {
                if (!rest.isEmpty()) {
                    rest = ItemHandlerHelper.insertItemStacked(h, rest, false);
                }
            }
        }
        return held + delta < 0 ? held + delta : 0;
    }

    /** A shulker box (one to a slot): the chests' goods in it are theirs. */
    static boolean isBox(ItemStack stack) {
        return stack.getCount() == 1 && stack.getItem() instanceof BlockItem item && item.getBlock() instanceof ShulkerBoxBlock;
    }

    private static NonNullList<ItemStack> contents(ItemStack box) {
        NonNullList<ItemStack> items = NonNullList.withSize(BOX_SLOTS, ItemStack.EMPTY);
        box.getOrDefault(DataComponents.CONTAINER, ItemContainerContents.EMPTY).copyInto(items);
        return items;
    }

    private static ItemStack withContents(ItemStack box, NonNullList<ItemStack> items) {
        ItemStack copy = box.copy();
        copy.set(DataComponents.CONTAINER, ItemContainerContents.fromItems(items));
        return copy;
    }
}
