package dev.b28.enterprise.client;

import dev.b28.enterprise.cargo.CrateMenu;
import dev.b28.enterprise.cargo.Crates;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.player.Inventory;

/**
 * A crate's screen: its nine stacks in a row, the documents slot below with its label, the inventory under that. Drawn
 * in the clay kit's colours: warm off-white panel, cream wells, ink text.
 */
public final class CrateScreen extends AbstractContainerScreen<CrateMenu> {
    private static final int PANEL = 0xFFFAF8F5;
    private static final int EDGE = 0xFFD9D2C7;
    private static final int WELL = 0xFFF2EFE9;
    private static final int SLOT = 0xFFC9C1B4;
    private static final int INK = 0xFF1C1917;
    private static final int STEEL = 0xFF44679B;

    public CrateScreen(CrateMenu menu, Inventory inventory, Component title) {
        super(menu, inventory, title);
        imageWidth = 176;
        imageHeight = 156;
        inventoryLabelY = CrateMenu.INVENTORY_Y - 11;
    }

    @Override
    protected void renderBg(GuiGraphics graphics, float partialTick, int mouseX, int mouseY) {
        int x = leftPos;
        int y = topPos;
        graphics.fill(x, y, x + imageWidth, y + imageHeight, EDGE);
        graphics.fill(x + 1, y + 1, x + imageWidth - 1, y + imageHeight - 1, PANEL);
        graphics.fill(x + 5, y + CrateMenu.GOODS_Y - 3, x + imageWidth - 5, y + CrateMenu.GOODS_Y + 19, WELL);
        for (int i = 0; i < Crates.GOODS; i++) {
            slot(graphics, x + CrateMenu.GOODS_X + i * 18, y + CrateMenu.GOODS_Y);
        }
        slot(graphics, x + CrateMenu.PAPERS_X, y + CrateMenu.PAPERS_Y);
        graphics.fill(x + 5, y + CrateMenu.INVENTORY_Y - 3, x + imageWidth - 5, y + CrateMenu.INVENTORY_Y + 77, WELL);
        for (int row = 0; row < 3; row++) {
            for (int col = 0; col < 9; col++) {
                slot(graphics, x + 8 + col * 18, y + CrateMenu.INVENTORY_Y + row * 18);
            }
        }
        for (int col = 0; col < 9; col++) {
            slot(graphics, x + 8 + col * 18, y + CrateMenu.INVENTORY_Y + 58);
        }
    }

    private static void slot(GuiGraphics graphics, int x, int y) {
        graphics.fill(x - 1, y - 1, x + 17, y + 17, SLOT);
        graphics.fill(x, y, x + 16, y + 16, 0xFFE8E2D8);
    }

    @Override
    protected void renderLabels(GuiGraphics graphics, int mouseX, int mouseY) {
        graphics.drawString(font, title, titleLabelX, titleLabelY, INK, false);
        graphics.drawString(font, Component.translatable("container.enterprise.crate.papers"), CrateMenu.PAPERS_X + 22,
                CrateMenu.PAPERS_Y + 4, STEEL, false);
        graphics.drawString(font, playerInventoryTitle, inventoryLabelX, inventoryLabelY, INK, false);
    }

    @Override
    public void render(GuiGraphics graphics, int mouseX, int mouseY, float partialTick) {
        super.render(graphics, mouseX, mouseY, partialTick);
        renderTooltip(graphics, mouseX, mouseY);
    }
}
