package dev.b28.enterprise.client;

import dev.b28.enterprise.merchant.MerchantMenu;
import dev.b28.enterprise.merchant.MerchantView;
import dev.b28.enterprise.merchant.MerchantView.LockerView;
import dev.b28.enterprise.merchant.MerchantView.TradeView;
import dev.b28.enterprise.merchant.ToggleTradePayload;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.core.BlockPos;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.item.ItemStack;
import net.neoforged.neoforge.network.PacketDistributor;

/** Lockers on the left; the selected locker's trades on the right, each with an On/Off switch. */
public final class MerchantScreen extends AbstractContainerScreen<MerchantMenu> {
    private static final int LIST_X = 8;
    private static final int LIST_Y = 32;
    private static final int LIST_W = 96;
    private static final int LIST_ROW = 16;
    private static final int TRADES_X = 112;
    private static final int TRADES_Y = 46;
    private static final int TRADE_ROW = 20;
    private static final int TRADE_ROWS = 7;
    private static final int SWITCH_X = 222;
    private static final int TEXT = 0x404040;

    private final Map<BlockPos, Set<String>> enabled = new HashMap<>();
    private int selected;
    private int scroll;

    public MerchantScreen(MerchantMenu menu, Inventory inventory, Component title) {
        super(menu, inventory, title);
        imageWidth = 276;
        imageHeight = 202;
        for (LockerView locker : menu.view().lockers()) {
            Set<String> on = new HashSet<>();
            locker.trades().stream().filter(TradeView::enabled).forEach(trade -> on.add(trade.key()));
            enabled.put(locker.workstation(), on);
        }
    }

    @Override
    protected void init() {
        super.init();
        rebuildSwitches();
    }

    private List<LockerView> lockers() {
        return menu.view().lockers();
    }

    private LockerView current() {
        return selected < lockers().size() ? lockers().get(selected) : null;
    }

    private void rebuildSwitches() {
        clearWidgets();
        LockerView locker = current();
        if (locker == null) {
            return;
        }
        for (int row = 0; row < TRADE_ROWS && scroll + row < locker.trades().size(); row++) {
            TradeView trade = locker.trades().get(scroll + row);
            boolean on = enabled.get(locker.workstation()).contains(trade.key());
            addRenderableWidget(Button.builder(
                            Component.translatable(on ? "screen.enterprise.merchant.on" : "screen.enterprise.merchant.off"),
                            button -> toggle(locker, trade))
                    .bounds(leftPos + SWITCH_X, topPos + TRADES_Y + row * TRADE_ROW, 46, 18)
                    .build());
        }
    }

    private void toggle(LockerView locker, TradeView trade) {
        Set<String> on = enabled.get(locker.workstation());
        boolean now = !on.contains(trade.key());
        if (now) {
            on.add(trade.key());
        } else {
            on.remove(trade.key());
        }
        PacketDistributor.sendToServer(new ToggleTradePayload(locker.workstation(), trade.key(), now));
        rebuildSwitches();
    }

    @Override
    protected void renderBg(GuiGraphics graphics, float partialTick, int mouseX, int mouseY) {
        int x = leftPos;
        int y = topPos;
        graphics.fill(x, y, x + imageWidth, y + imageHeight, 0xFF373737);
        graphics.fill(x + 1, y + 1, x + imageWidth - 1, y + imageHeight - 1, 0xFFC6C6C6);
        well(graphics, x + LIST_X, y + LIST_Y, LIST_W, imageHeight - LIST_Y - 8);
        well(graphics, x + TRADES_X - 4, y + LIST_Y, imageWidth - TRADES_X - 4, imageHeight - LIST_Y - 8);
    }

    /** A recessed panel: dark edge top-left, light edge bottom-right, like vanilla slots. */
    private static void well(GuiGraphics graphics, int x, int y, int w, int h) {
        graphics.fill(x - 1, y - 1, x + w + 1, y + h + 1, 0xFFFFFFFF);
        graphics.fill(x - 1, y - 1, x + w, y + h, 0xFF373737);
        graphics.fill(x, y, x + w, y + h, 0xFF8B8B8B);
    }

    @Override
    protected void renderLabels(GuiGraphics graphics, int mouseX, int mouseY) {
        graphics.drawString(font, title, 8, 7, TEXT, false);
        Component count = Component.translatable(
                "screen.enterprise.merchant.lockers", lockers().size(), menu.view().limit());
        graphics.drawString(font, count, imageWidth - 8 - font.width(count), 7, TEXT, false);

        Component rate = Component.translatable(
                "screen.enterprise.merchant.rate", String.format("%.1f", menu.view().rate()));
        Component bank = menu.view().bankSpurs() == MerchantView.NO_BANK
                ? Component.translatable("screen.enterprise.merchant.no_bank")
                : Component.translatable("screen.enterprise.merchant.bank", String.format("%,d", menu.view().bankSpurs()));
        graphics.drawString(font, rate, 8, 19, TEXT, false);
        graphics.drawString(font, bank, imageWidth - 8 - font.width(bank), 19, TEXT, false);
    }

    @Override
    public void render(GuiGraphics graphics, int mouseX, int mouseY, float partialTick) {
        super.render(graphics, mouseX, mouseY, partialTick);
        renderLockerList(graphics);
        renderTrades(graphics, mouseX, mouseY);
    }

    private void renderLockerList(GuiGraphics graphics) {
        for (int i = 0; i < lockers().size(); i++) {
            int x = leftPos + LIST_X;
            int y = topPos + LIST_Y + i * LIST_ROW;
            if (i == selected) {
                graphics.fill(x, y, x + LIST_W, y + LIST_ROW, 0xFFC6C6C6);
            }
            String name = font.plainSubstrByWidth(lockers().get(i).worker().getString(), LIST_W - 8);
            graphics.drawString(font, name, x + 4, y + 4, i == selected ? TEXT : 0xFFFFFF, i != selected);
        }
    }

    private void renderTrades(GuiGraphics graphics, int mouseX, int mouseY) {
        int x = leftPos + TRADES_X;
        LockerView locker = current();
        if (locker == null) {
            graphics.drawWordWrap(font, Component.translatable("screen.enterprise.merchant.no_lockers"),
                    x, topPos + LIST_Y + 4, imageWidth - TRADES_X - 12, 0xFFFFFF);
            return;
        }
        graphics.drawString(font, locker.worker(), x, topPos + LIST_Y + 4, 0xFFFFFF, true);
        if (locker.trades().isEmpty()) {
            graphics.drawWordWrap(font, Component.translatable("screen.enterprise.merchant.no_trades"),
                    x, topPos + TRADES_Y + 4, imageWidth - TRADES_X - 12, 0xFFFFFF);
            return;
        }
        ItemStack hovered = ItemStack.EMPTY;
        for (int row = 0; row < TRADE_ROWS && scroll + row < locker.trades().size(); row++) {
            TradeView trade = locker.trades().get(scroll + row);
            int y = topPos + TRADES_Y + row * TRADE_ROW + 1;
            hovered = item(graphics, trade.costA(), x, y, mouseX, mouseY, hovered);
            hovered = item(graphics, trade.costB(), x + 20, y, mouseX, mouseY, hovered);
            graphics.drawString(font, "→", x + 44, y + 4, 0xFFFFFF, true);
            hovered = item(graphics, trade.result(), x + 58, y, mouseX, mouseY, hovered);
        }
        if (!hovered.isEmpty()) {
            graphics.renderTooltip(font, hovered, mouseX, mouseY);
        }
    }

    private ItemStack item(GuiGraphics graphics, ItemStack stack, int x, int y, int mouseX, int mouseY, ItemStack hovered) {
        if (stack.isEmpty()) {
            return hovered;
        }
        graphics.renderItem(stack, x, y);
        graphics.renderItemDecorations(font, stack, x, y);
        boolean over = mouseX >= x && mouseX < x + 16 && mouseY >= y && mouseY < y + 16;
        return over ? stack : hovered;
    }

    @Override
    public boolean mouseClicked(double mouseX, double mouseY, int button) {
        int x = leftPos + LIST_X;
        int y = topPos + LIST_Y;
        if (mouseX >= x && mouseX < x + LIST_W && mouseY >= y) {
            int row = (int) ((mouseY - y) / LIST_ROW);
            if (row < lockers().size()) {
                selected = row;
                scroll = 0;
                rebuildSwitches();
                return true;
            }
        }
        return super.mouseClicked(mouseX, mouseY, button);
    }

    @Override
    public boolean mouseScrolled(double mouseX, double mouseY, double scrollX, double scrollY) {
        LockerView locker = current();
        if (locker != null) {
            int max = Math.max(0, locker.trades().size() - TRADE_ROWS);
            int next = Math.clamp(scroll - (int) Math.signum(scrollY), 0, max);
            if (next != scroll) {
                scroll = next;
                rebuildSwitches();
                return true;
            }
        }
        return super.mouseScrolled(mouseX, mouseY, scrollX, scrollY);
    }
}
