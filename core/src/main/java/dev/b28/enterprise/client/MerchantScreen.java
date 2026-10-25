package dev.b28.enterprise.client;

import dev.b28.enterprise.merchant.LedgerView;
import dev.b28.enterprise.merchant.MerchantMenu;
import dev.b28.enterprise.merchant.MerchantView;
import dev.b28.enterprise.merchant.MerchantView.LockerView;
import dev.b28.enterprise.merchant.MerchantView.TownView;
import dev.b28.enterprise.merchant.MerchantView.TradeView;
import dev.b28.enterprise.merchant.CharterPayload;
import dev.b28.enterprise.merchant.CommissionPayload;
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
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.item.ItemStack;
import net.neoforged.neoforge.network.PacketDistributor;

/**
 * Three tabs. Ledger: the town's books in six sections (Money, Goods, People, Wagons, Papers, Mint) listed on the left, the
 * selected section's lines on the right (scroll for more), each line naming the question it came from on hover; asked once when
 * the screen opened. Lockers: the lockers on the left, the selected locker's trades on the right, each with an On/Off switch.
 * Caravans: the markets this desk knows on the left, the selected one on the right with a button to send caravans
 * there or stop, and who is at home to go. Beside the tabs, a button to buy a town charter for a block of diamond.
 */
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
    private static final int TAB_W = 64;
    private static final int LEDGER_LIST_W = 56;
    private static final int LEDGER_X = LIST_X + LEDGER_LIST_W + 8;
    private static final int LEDGER_ROW = 10;
    /** The ledger's panes start lower: the town's heading and its till (two lines) stand above them. */
    private static final int LEDGER_Y = 50;
    private static final int LEDGER_H = 202 - LEDGER_Y - 20;

    private final Map<BlockPos, Set<String>> enabled = new HashMap<>();
    private final Set<BlockPos> routed = new HashSet<>();
    private boolean caravansTab;
    private boolean ledgerTab;
    private int section;
    private int ledgerScroll;
    /** The selected section's lines wrapped to the pane, each with the question it came from. */
    private List<Row> rows = List.of();

    private record Row(FormattedCharSequence text, String source) {}
    private int selected;
    private int selectedTown;
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
        menu.view().caravans().towns().stream().filter(TownView::routed).forEach(town -> routed.add(town.desk()));
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

    private List<TownView> towns() {
        return menu.view().caravans().towns();
    }

    private void rebuildSwitches() {
        clearWidgets();
        int ledgerW = 44;
        int caravansW = 54;
        int lockersW = 62;
        int right = leftPos + imageWidth - 8;
        addRenderableWidget(Button.builder(Component.translatable("screen.enterprise.merchant.tab.ledger"),
                        button -> showTab(false, true))
                .bounds(right - ledgerW, topPos + 3, ledgerW, 14).build()).active = !ledgerTab;
        addRenderableWidget(Button.builder(Component.translatable("screen.enterprise.merchant.tab.caravans"),
                        button -> showTab(true, false))
                .bounds(right - ledgerW - caravansW, topPos + 3, caravansW, 14).build()).active = ledgerTab || !caravansTab;
        addRenderableWidget(Button.builder(Component.translatable("screen.enterprise.merchant.tab.lockers",
                                lockers().size(), menu.view().limit()), button -> showTab(false, false))
                .bounds(right - ledgerW - caravansW - lockersW, topPos + 3, lockersW, 14).build()).active = ledgerTab || caravansTab;
        int charterW = 44;
        addRenderableWidget(Button.builder(Component.translatable("screen.enterprise.merchant.charter"),
                        button -> PacketDistributor.sendToServer(new CharterPayload()))
                .tooltip(net.minecraft.client.gui.components.Tooltip.create(
                        Component.translatable("screen.enterprise.merchant.charter.tooltip")))
                .bounds(right - ledgerW - caravansW - lockersW - 4 - charterW, topPos + 3, charterW, 14).build());
        if (ledgerTab) {
            return;
        }
        if (caravansTab) {
            TownView town = selectedTown < towns().size() ? towns().get(selectedTown) : null;
            if (town != null && (town.inRange() || routed.contains(town.desk()))) {
                boolean sending = routed.contains(town.desk());
                addRenderableWidget(Button.builder(Component.translatable(
                                        sending ? "screen.enterprise.merchant.stop" : "screen.enterprise.merchant.send"),
                                button -> commission(town, !sending))
                        .bounds(leftPos + imageWidth - 8 - 96, topPos + imageHeight - 30, 92, 18).build());
            }
            return;
        }
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

    private void showTab(boolean caravans, boolean ledger) {
        caravansTab = caravans;
        ledgerTab = ledger;
        if (ledger) {
            wrapSection();
        }
        rebuildSwitches();
    }

    private LedgerView ledger() {
        return menu.view().ledger();
    }

    /** The selected section's lines (or the ledger's one word) wrapped to the pane. */
    private void wrapSection() {
        ledgerScroll = 0;
        int width = imageWidth - LEDGER_X - 12;
        List<Row> out = new java.util.ArrayList<>();
        if (ledger().isEmpty()) {
            font.split(Component.literal(ledger().heading().isBlank()
                    ? "No town here yet: use a Town Charter on this desk to found one (Charter, above, sells one for a block of diamond)." : ledger().heading()), width)
                    .forEach(part -> out.add(new Row(part, "")));
        } else if (section < ledger().sections().size()) {
            for (LedgerView.Line line : ledger().sections().get(section).lines()) {
                font.split(Component.literal(line.text()), width).forEach(part -> out.add(new Row(part, line.source())));
            }
        }
        rows = out;
    }

    private int ledgerRows() {
        return (LEDGER_H - 6) / LEDGER_ROW;
    }

    private void commission(TownView town, boolean send) {
        if (send) {
            routed.add(town.desk());
        } else {
            routed.remove(town.desk());
        }
        PacketDistributor.sendToServer(new CommissionPayload(town.desk(), send));
        rebuildSwitches();
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
        if (ledgerTab) {
            well(graphics, x + LIST_X, y + LEDGER_Y, LEDGER_LIST_W, LEDGER_H);
            well(graphics, x + LEDGER_X - 4, y + LEDGER_Y, imageWidth - LEDGER_X - 4, LEDGER_H);
            return;
        }
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
        if (ledgerTab) {
            if (!ledger().isEmpty()) {
                graphics.drawString(font, font.plainSubstrByWidth(ledger().heading(), imageWidth - 16), 8, 19, TEXT, false);
                int line = 29;
                for (FormattedCharSequence part : font.split(Component.literal(ledger().till()), imageWidth - 16)) {
                    if (line > 38) {
                        break;
                    }
                    graphics.drawString(font, part, 8, line, TEXT, false);
                    line += 9;
                }
            }
            return;
        }

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
        if (ledgerTab) {
            renderLedger(graphics, mouseX, mouseY);
        } else if (caravansTab) {
            renderTownList(graphics);
            renderTown(graphics);
        } else {
            renderLockerList(graphics);
            renderTrades(graphics, mouseX, mouseY);
        }
    }

    private void renderLedger(GuiGraphics graphics, int mouseX, int mouseY) {
        LedgerView ledger = ledger();
        for (int i = 0; i < ledger.sections().size(); i++) {
            int x = leftPos + LIST_X;
            int y = topPos + LEDGER_Y + i * LIST_ROW;
            if (i == section) {
                graphics.fill(x, y, x + LEDGER_LIST_W, y + LIST_ROW, 0xFFC6C6C6);
            }
            graphics.drawString(font, ledger.sections().get(i).name(), x + 4, y + 4, i == section ? TEXT : 0xFFFFFF, i != section);
        }
        int x = leftPos + LEDGER_X;
        int top = topPos + LEDGER_Y + 4;
        String hovered = "";
        for (int r = 0; r < ledgerRows() && ledgerScroll + r < rows.size(); r++) {
            Row row = rows.get(ledgerScroll + r);
            int y = top + r * LEDGER_ROW;
            graphics.drawString(font, row.text(), x, y, 0xFFFFFF, true);
            if (mouseX >= x && mouseX < leftPos + imageWidth - 8 && mouseY >= y && mouseY < y + LEDGER_ROW) {
                hovered = row.source();
            }
        }
        if (!hovered.isEmpty()) {
            graphics.renderTooltip(font, Component.translatable("screen.enterprise.ledger.from", hovered), mouseX, mouseY);
        }
        if (rows.size() > ledgerRows()) {
            String more = (ledgerScroll > 0 ? "↑ " : "") + (ledgerScroll + ledgerRows() < rows.size() ? "↓" : "");
            graphics.drawString(font, more, leftPos + imageWidth - 8 - font.width(more) - 4,
                    topPos + LEDGER_Y + LEDGER_H - LEDGER_ROW, 0xE0E0E0, true);
        }
        if (!ledger.footer().isEmpty()) {
            graphics.drawString(font, font.plainSubstrByWidth(ledger.footer(), imageWidth - 16), leftPos + 8,
                    topPos + imageHeight - 13, 0x606060, false);
        }
    }

    private void renderTownList(GuiGraphics graphics) {
        for (int i = 0; i < towns().size() && i < (imageHeight - LIST_Y - 8) / LIST_ROW; i++) {
            TownView town = towns().get(i);
            int x = leftPos + LIST_X;
            int y = topPos + LIST_Y + i * LIST_ROW;
            if (i == selectedTown) {
                graphics.fill(x, y, x + LIST_W, y + LIST_ROW, 0xFFC6C6C6);
            }
            String label = (routed.contains(town.desk()) ? "» " : "") + town.distance() + " " + town.direction();
            graphics.drawString(font, label, x + 4, y + 4, i == selectedTown ? TEXT : 0xFFFFFF, i != selectedTown);
        }
    }

    private void renderTown(GuiGraphics graphics) {
        int x = leftPos + TRADES_X;
        int width = imageWidth - TRADES_X - 12;
        int y = topPos + LIST_Y + 4;
        if (towns().isEmpty()) {
            graphics.drawWordWrap(font, Component.translatable("screen.enterprise.merchant.no_towns"), x, y, width, 0xFFFFFF);
        } else {
            TownView town = towns().get(Math.min(selectedTown, towns().size() - 1));
            Component kind = Component.translatable(
                    town.settlement() ? "screen.enterprise.merchant.town" : "screen.enterprise.merchant.market");
            graphics.drawString(font, kind.copy().append(" · ").append(Component.translatable(
                    "screen.enterprise.merchant.where", town.distance(), town.direction())), x, y, 0xFFFFFF, true);
            graphics.drawString(font, town.desk().toShortString(), x, y + 12, 0xE0E0E0, false);
            Component state = !town.inRange() && !routed.contains(town.desk())
                    ? Component.translatable("screen.enterprise.merchant.too_far")
                    : routed.contains(town.desk())
                            ? Component.literal(town.status().isEmpty() ? "…" : capitalised(town.status()))
                            : Component.translatable("screen.enterprise.merchant.not_sending");
            graphics.drawWordWrap(font, state, x, y + 28, width, 0xFFFFFF);
        }
        List<String> squad = menu.view().caravans().squad();
        Component home = squad.isEmpty()
                ? Component.translatable("screen.enterprise.merchant.no_squad")
                : Component.translatable("screen.enterprise.merchant.squad", squad.stream()
                        .map(trade -> Component.translatable(professionKey(trade)).getString())
                        .collect(java.util.stream.Collectors.joining(", ")));
        graphics.drawWordWrap(font, home, x, topPos + imageHeight - 74, width, 0xFFFFFF);
    }

    private static String capitalised(String text) {
        return Character.toUpperCase(text.charAt(0)) + text.substring(1);
    }

    /** Vanilla's name key for a villager trade: entity.minecraft.villager.librarian, ...villager.enterprise.merchant. */
    static String professionKey(String trade) {
        String[] parts = trade.split(":", 2);
        return parts[0].equals("minecraft")
                ? "entity.minecraft.villager." + parts[1]
                : "entity.minecraft.villager." + parts[0] + "." + parts[1];
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
        if (ledgerTab) {
            int ly = topPos + LEDGER_Y;
            if (mouseX >= x && mouseX < x + LEDGER_LIST_W && mouseY >= ly) {
                int row = (int) ((mouseY - ly) / LIST_ROW);
                if (row < ledger().sections().size()) {
                    section = row;
                    wrapSection();
                    return true;
                }
            }
            return super.mouseClicked(mouseX, mouseY, button);
        }
        if (caravansTab) {
            if (mouseX >= x && mouseX < x + LIST_W && mouseY >= y) {
                int row = (int) ((mouseY - y) / LIST_ROW);
                if (row < towns().size()) {
                    selectedTown = row;
                    rebuildSwitches();
                    return true;
                }
            }
            return super.mouseClicked(mouseX, mouseY, button);
        }
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
        if (ledgerTab) {
            int next = Math.clamp(ledgerScroll - (int) Math.signum(scrollY), 0, Math.max(0, rows.size() - ledgerRows()));
            ledgerScroll = next;
            return true;
        }
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
