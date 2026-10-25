package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.binding.adapters.LedgerAdapter;
import dev.b28.enterprise.merchant.LedgerView;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantView;
import io.netty.buffer.Unpooled;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.stream.Stream;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.entity.ChestBlockEntity;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/**
 * The ledger (docs/alpha-plan.md 3): a town's books in short lines, from the core's answers, for the Merchant screen's Ledger
 * tab and {@code /enterprise ledger}. A host of its own on a folder of its own, as the core binding tests have.
 */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class LedgerTests {
    // Test coordinates start at the structure block, one below the arena; the floor is y=1.
    private static final BlockPos DESK = new BlockPos(3, 2, 3);

    /**
     * A desk with emeralds and goods in its chest, a look, and the ledger: its Money and Goods lines say what {@code core cash}
     * and {@code core stock} say of the same desk (the same adapters' answers), its other sections are there, each line names
     * its question, and the whole rides the Merchant screen's packet and comes out as it went in.
     */
    @GameTest(template = "floor")
    public static void theLedgerSaysWhatCoreCashAndCoreStockSay(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        var level = helper.getLevel();
        ChestBlockEntity chest = (ChestBlockEntity) level.getBlockEntity(helper.absolutePos(DESK.east()));
        chest.setItem(0, new ItemStack(Items.WHEAT, 30));
        chest.setItem(1, new ItemStack(Items.BREAD, 5));
        chest.setItem(2, new ItemStack(Items.EMERALD, 12));
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            host.stock().found(level, desk.pos(), "Testford"); // a look: the chests counted, the emeralds sold to the mint
            host.settle();
            LedgerView ledger = new LedgerAdapter(host).view(desk);

            // Money: what `core cash` says.
            var cash = host.money().cashAt(desk);
            helper.assertTrue(cash != null && cash.treasury() > 0, "the desk's till: " + cash);
            LedgerView.Section money = ledger.section("Money");
            helper.assertTrue(money != null, "no Money section in " + ledger.sections());
            has(helper, money, "Treasury %s emeralds".formatted(LedgerAdapter.fmt(cash.treasury())), "Cash");
            has(helper, money, "Reserve %d emeralds kept back".formatted(cash.reserve()), "Cash");
            has(helper, money, "Till: chests hold %d emeralds and %d spurs of coin (worth %s emeralds)".formatted(cash.emeralds(),
                    cash.coins(), LedgerAdapter.fmt(cash.coinWorth())), "Cash");
            helper.assertTrue(ledger.till().startsWith("Treasury " + LedgerAdapter.fmt(cash.treasury()) + " emeralds"),
                    "the till line: " + ledger.till());
            helper.assertTrue(ledger.heading().startsWith("Testford · day "), "the heading: " + ledger.heading());

            // Goods: what `core stock` says, largest first.
            Map<String, Double> held = host.stock().stockAt(desk);
            helper.assertTrue(held.size() == 2 && held.get("minecraft:wheat") == 30 && held.get("minecraft:bread") == 5,
                    "the town holds " + held);
            LedgerView.Section goods = ledger.section("Goods");
            helper.assertTrue(goods != null, "no Goods section in " + ledger.sections());
            for (var good : held.entrySet()) {
                has(helper, goods, "  %s %s".formatted(good.getKey().replaceFirst(".*:", ""), LedgerAdapter.fmt(good.getValue())),
                        "Stock");
            }
            has(helper, goods, "The town holds 2 kinds of goods, largest first", "Stock");
            List<String> texts = goods.lines().stream().map(LedgerView.Line::text).toList();
            helper.assertTrue(texts.indexOf("  wheat 30") < texts.indexOf("  bread 5"), "not largest first: " + texts);
            helper.assertTrue(texts.stream().anyMatch(t -> t.startsWith("Free slots in the chests: ")), "no free slots in " + texts);

            // Every section is there, every line names its question, and the command prints what the screen shows.
            helper.assertTrue(ledger.sections().stream().map(LedgerView.Section::name).toList()
                    .equals(List.of("Money", "Goods", "People", "Wagons", "Papers", "Mint")), "sections " + ledger.sections());
            ledger.sections().forEach(s -> s.lines().forEach(l -> helper.assertTrue(!l.source().isBlank() && !l.text().isBlank(),
                    "a line without a source or words in " + s.name() + ": " + l)));
            helper.assertTrue(ledger.footer().contains("asked at open") && ledger.footer().contains("schema "), ledger.footer());
            List<String> printed = ledger.toText();
            helper.assertTrue(printed.contains("== Goods ==") && printed.contains("    wheat 30"), "the command's lines: " + printed);

            // No number reads -0 or 1e-9 dust.
            helper.assertTrue(LedgerAdapter.fmt(-1e-12).equals("0") && LedgerAdapter.fmt(2.5).equals("2.5")
                    && LedgerAdapter.fmt(3.0).equals("3"), "fmt");

            // The screen's packet: the view goes through its codec whole.
            MerchantView view = new MerchantView(helper.absolutePos(DESK), 2, List.of(), 16, MerchantView.NO_BANK,
                    new MerchantView.CaravanView(List.of(), List.of()), ledger);
            var buf = new RegistryFriendlyByteBuf(Unpooled.buffer(), level.registryAccess());
            MerchantView.STREAM_CODEC.encode(buf, view);
            helper.assertTrue(MerchantView.STREAM_CODEC.decode(buf).equals(view), "the view changed through its codec");
            helper.assertTrue(buf.writerIndex() < 30000, "the packet is " + buf.writerIndex() + " bytes");

            // A desk the core keeps no town at is only a word.
            LedgerView none = new LedgerAdapter(host).view(GlobalPos.of(level.dimension(), helper.absolutePos(DESK.south(2))));
            helper.assertTrue(none.isEmpty() && none.toText().get(0).startsWith("No town here"), "no town: " + none.toText());
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * The ledger says what keeps a town's villagers from having lockers: a Librarian's lectern with no chest touching it
     * is named, with where it stands, and a Cartographer's table with one isn't (it's a locker).
     */
    @GameTest(template = "floor")
    public static void theLedgerNamesTheWorkstationsWithNoChest(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        var level = helper.getLevel();
        BlockPos lectern = new BlockPos(1, 2, 5);
        BlockPos table = new BlockPos(5, 2, 5);
        helper.setBlock(lectern, Blocks.LECTERN);
        helper.setBlock(table, Blocks.CARTOGRAPHY_TABLE);
        helper.setBlock(table.east(), Blocks.CHEST);
        var librarian = helper.spawn(net.minecraft.world.entity.EntityType.VILLAGER, new BlockPos(1, 2, 4));
        librarian.setVillagerData(librarian.getVillagerData().setProfession(net.minecraft.world.entity.npc.VillagerProfession.LIBRARIAN));
        librarian.setVillagerXp(1);
        librarian.getBrain().setMemory(net.minecraft.world.entity.ai.memory.MemoryModuleType.JOB_SITE,
                GlobalPos.of(level.dimension(), helper.absolutePos(lectern)));
        var cartographer = helper.spawn(net.minecraft.world.entity.EntityType.VILLAGER, new BlockPos(5, 2, 4));
        cartographer.setVillagerData(cartographer.getVillagerData().setProfession(net.minecraft.world.entity.npc.VillagerProfession.CARTOGRAPHER));
        cartographer.setVillagerXp(1);
        cartographer.getBrain().setMemory(net.minecraft.world.entity.ai.memory.MemoryModuleType.JOB_SITE,
                GlobalPos.of(level.dimension(), helper.absolutePos(table)));
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, desk.pos(), "Testford");
            host.hands().join(town, librarian);
            host.hands().join(town, cartographer);
            LedgerView.Section people = new LedgerAdapter(host).view(desk).section("People");
            helper.assertTrue(people != null, "no People section");
            BlockPos at = helper.absolutePos(lectern);
            has(helper, people, "    the librarian's lectern at %d %d %d: no chest touching it".formatted(at.getX(), at.getY(), at.getZ()),
                    "Lockers");
            helper.assertTrue(people.lines().stream().noneMatch(l -> l.text().contains("cartographer's")),
                    "a workstation with a chest named: " + people.lines());
        } finally {
            delete(dir);
            librarian.discard();
            cartographer.discard();
        }
        helper.succeed();
    }

    private static void has(GameTestHelper helper, LedgerView.Section section, String text, String source) {
        helper.assertTrue(section.lines().stream().anyMatch(l -> l.text().equals(text) && l.source().equals(source)),
                "no line '%s' (from %s) in %s: %s".formatted(text, source, section.name(), section.lines()));
    }

    private static Path folder() {
        try {
            return Files.createTempDirectory("enterprise-ledger-test");
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    private static void delete(Path dir) {
        try (Stream<Path> all = Files.walk(dir)) {
            all.sorted(Comparator.reverseOrder()).forEach(p -> p.toFile().delete());
        } catch (IOException e) {
            // a temporary folder left behind
        }
    }
}
