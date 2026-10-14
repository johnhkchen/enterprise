package dev.b28.enterprise.cargo;

import net.minecraft.world.Container;
import net.minecraft.world.SimpleContainer;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.inventory.Slot;
import net.minecraft.world.item.ItemStack;

/** A crate's nine stacks in a row, its documents slot beside them, and the player's inventory below. */
public final class CrateMenu extends AbstractContainerMenu {
    public static final int GOODS_X = 8;
    public static final int GOODS_Y = 18;
    public static final int PAPERS_X = 8;
    public static final int PAPERS_Y = 42;
    public static final int INVENTORY_Y = 74;

    private final Container crate;

    public CrateMenu(int id, Inventory inventory) {
        this(id, inventory, new SimpleContainer(Crates.SLOTS));
    }

    public CrateMenu(int id, Inventory inventory, Container crate) {
        super(Cargo.CRATE_MENU.get(), id);
        checkContainerSize(crate, Crates.SLOTS);
        this.crate = crate;
        crate.startOpen(inventory.player);
        for (int i = 0; i < Crates.GOODS; i++) {
            addSlot(new Slot(crate, i, GOODS_X + i * 18, GOODS_Y) {
                @Override
                public boolean mayPlace(ItemStack stack) {
                    return Crates.fits(getContainerSlot(), stack);
                }
            });
        }
        addSlot(new Slot(crate, Crates.PAPERS_SLOT, PAPERS_X, PAPERS_Y) {
            @Override
            public boolean mayPlace(ItemStack stack) {
                return Crates.fits(getContainerSlot(), stack);
            }

            @Override
            public int getMaxStackSize() {
                return 1;
            }
        });
        for (int row = 0; row < 3; row++) {
            for (int col = 0; col < 9; col++) {
                addSlot(new Slot(inventory, col + row * 9 + 9, 8 + col * 18, INVENTORY_Y + row * 18));
            }
        }
        for (int col = 0; col < 9; col++) {
            addSlot(new Slot(inventory, col, 8 + col * 18, INVENTORY_Y + 58));
        }
    }

    @Override
    public ItemStack quickMoveStack(Player player, int index) {
        Slot slot = slots.get(index);
        if (!slot.hasItem()) {
            return ItemStack.EMPTY;
        }
        ItemStack stack = slot.getItem();
        ItemStack copy = stack.copy();
        if (index < Crates.SLOTS) {
            if (!moveItemStackTo(stack, Crates.SLOTS, slots.size(), true)) {
                return ItemStack.EMPTY;
            }
        } else if (stack.is(Cargo.DOCUMENTS.get())) {
            if (!moveItemStackTo(stack, Crates.PAPERS_SLOT, Crates.SLOTS, false)) {
                return ItemStack.EMPTY;
            }
        } else if (!Crates.fits(0, stack) || !moveItemStackTo(stack, 0, Crates.GOODS, false)) {
            return ItemStack.EMPTY;
        }
        if (stack.isEmpty()) {
            slot.setByPlayer(ItemStack.EMPTY);
        } else {
            slot.setChanged();
        }
        return copy;
    }

    @Override
    public boolean stillValid(Player player) {
        return crate.stillValid(player);
    }

    @Override
    public void removed(Player player) {
        super.removed(player);
        crate.stopOpen(player);
    }
}
