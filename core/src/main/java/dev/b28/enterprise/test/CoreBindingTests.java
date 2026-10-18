package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.binding.ModPack;
import dev.b28.enterprise.binding.Society;
import dev.b28.enterprise.binding.adapters.DialAdapter;
import dev.b28.enterprise.core.SaveStore;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.ContentPack;
import dev.b28.enterprise.wire.GoodFlag;
import dev.b28.enterprise.wire.PackGood;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.merchant.Merchant;
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
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.entity.ChestBlockEntity;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/**
 * The Rust core in the mod, end to end (docs/binding-plan.md B2.1): a Merchant's desk and its chest in the world, a
 * town in the core, its chests counted into a stocktake, its stock answered by the core through the binding. Each test
 * runs a host of its own on a folder of its own, so the server's host (and the other tests' desks) never meet it.
 */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class CoreBindingTests {
    // Test coordinates start at the structure block, one below the arena; the floor is y=1.
    private static final BlockPos DESK = new BlockPos(3, 2, 3);

    /** A desk with a chest of wheat, bread, an emerald and dirt (neither is one of the core's goods). */
    private static ChestBlockEntity market(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        ChestBlockEntity chest = (ChestBlockEntity) helper.getLevel().getBlockEntity(helper.absolutePos(DESK.east()));
        chest.setItem(0, new ItemStack(Items.WHEAT, 30));
        chest.setItem(1, new ItemStack(Items.BREAD, 5));
        chest.setItem(2, new ItemStack(Items.EMERALD, 3));
        chest.setItem(3, new ItemStack(Items.DIRT, 64));
        return chest;
    }

    @GameTest(template = "floor")
    public static void aDesksStocktakeComesBackFromTheCore(GameTestHelper helper) {
        ChestBlockEntity chest = market(helper);
        var level = helper.getLevel();
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, desk.pos(), "Testford");
            helper.assertTrue(host.stock().townAt(desk) == town, "the desk isn't its town's point in the core");
            Map<String, Double> first = host.stock().stockAt(desk);
            helper.assertTrue(first.equals(Map.of("minecraft:wheat", 30.0, "minecraft:bread", 5.0)),
                    "the core holds " + first + " after the desk's first stocktake, not its chest's 30 wheat and 5 bread");

            // A player takes 10 wheat and puts 2 books in; the next look counts the chest again.
            chest.setItem(0, new ItemStack(Items.WHEAT, 20));
            chest.setItem(4, new ItemStack(Items.BOOK, 2));
            host.look(level.getServer(), level.getGameTime());
            Map<String, Double> after = host.stock().stockAt(desk);
            helper.assertTrue(after.equals(Map.of("minecraft:wheat", 20.0, "minecraft:bread", 5.0, "minecraft:book", 2.0)),
                    "the core holds " + after + " after a look, not the chest's 20 wheat, 5 bread and 2 books");
        } finally {
            delete(dir);
        }
        if (System.getenv("ENTERPRISE_CORE_MEASURE") != null) {
            measureALookAndASave(helper);
        }
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void theCoreSurvivesASaveAndLoad(GameTestHelper helper) {
        ChestBlockEntity chest = market(helper);
        var level = helper.getLevel();
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try {
            Map<String, Double> saved;
            long town;
            try (CoreHost host = CoreHost.open(dir, 7)) {
                town = host.stock().found(level, desk.pos(), "Testford");
                chest.setItem(4, new ItemStack(Items.BOOK, 2));
                host.look(level.getServer(), level.getGameTime());
                saved = host.stock().stockAt(desk);
                helper.assertTrue(host.save(), "the host didn't save");
            }
            helper.assertTrue(Files.isRegularFile(dir.resolve(SaveStore.FILE)), "no " + SaveStore.FILE + " in the world's folder");

            // A fresh host on the same folder (another seed: it must load, not make a society) knows the desk and its
            // goods from the core alone, and answers as the saved one did.
            try (CoreHost loaded = CoreHost.open(dir, 8)) {
                helper.assertTrue(loaded.pausedWhy() == null, "the loaded host is paused: " + loaded.pausedWhy());
                Long again = loaded.stock().townAt(desk);
                helper.assertTrue(again != null && again == town, "the loaded core doesn't keep the desk as town " + town);
                Map<String, Double> answered = loaded.stock().stockAt(desk);
                helper.assertTrue(answered.equals(saved), "the loaded core holds " + answered + ", the saved one " + saved);
                helper.assertTrue(saved.equals(Map.of("minecraft:wheat", 30.0, "minecraft:bread", 5.0, "minecraft:book", 2.0)),
                        "the saved core held " + saved);

                // It counts on from the saved stocktake: the chest unchanged books nothing, a bread taken is taken.
                chest.setItem(1, new ItemStack(Items.BREAD, 4));
                loaded.look(level.getServer(), level.getGameTime() + 1);
                Map<String, Double> on = loaded.stock().stockAt(desk);
                helper.assertTrue(on.equals(Map.of("minecraft:wheat", 30.0, "minecraft:bread", 4.0, "minecraft:book", 2.0)),
                        "after the load a look counts " + on + ", not on from the saved stocktake");
            }
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * The mod's content pack (B4.1): goods with their comforts, crops, meat and weights, the trades, recipes and herds,
     * and the dials, checked by the core when it makes a society from it. A pack the core refused would pause the host,
     * so a host that isn't paused took the whole of it; the dials it holds are the Java catalogue's, one for one but
     * the mod's own, at the mod's defaults.
     */
    @GameTest(template = "floor")
    public static void theModsContentPackIsTakenWholeByTheCore(GameTestHelper helper) {
        ContentPack pack = ModPack.of();
        Map<String, PackGood> goods = new java.util.HashMap<>();
        pack.goods().forEach(g -> goods.put(g.name(), g));
        helper.assertTrue(goods.get("minecraft:iron_ingot").weight() == 2.0 && goods.get("minecraft:cobblestone").weight() == 3.0
                && goods.get("minecraft:wheat").weight() == 1.0, "the pack's weights: iron " + goods.get("minecraft:iron_ingot"));
        helper.assertTrue(goods.get("minecraft:wheat").flags().contains(GoodFlag.Crop)
                && goods.get("minecraft:cooked_beef").flags().contains(GoodFlag.Meat), "a crop and a meat aren't flagged");
        helper.assertTrue(goods.get("minecraft:book").comfort().equals(java.util.Optional.of("reading"))
                && goods.get("enterprise:pipe_tobacco").comfort().equals(java.util.Optional.of("leisure")), "comforts aren't the tags'");
        helper.assertTrue(goods.containsKey("minecraft:white_wool") && goods.containsKey("minecraft:cod"),
                "goods the recipes and herds use aren't in the pack although the mod doesn't price them");
        helper.assertTrue(pack.comforts().equals(List.of("clothing", "furnishings", "leisure", "reading", "treats")),
                "kinds of comfort: " + pack.comforts());
        helper.assertTrue(pack.trades().size() == 16 && pack.trades().get(0).equals("merchant") && pack.trades().contains("farmer")
                && pack.trades().get(14).equals("none") && pack.trades().get(15).equals("nitwit"), "trades: " + pack.trades());
        helper.assertTrue(pack.recipes().size() == dev.b28.enterprise.territory.Workshop.CONVERSIONS.size()
                && pack.animals().size() == dev.b28.enterprise.territory.Husbandry.KINDS.size(), "recipes or animals missing");

        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            helper.assertTrue(host.pausedWhy() == null, "the core refused the mod's pack: " + host.pausedWhy());
            List<String> names = ((Answer.Goods) host.ask(List.of(new Query.Goods())).get(0)).goods();
            helper.assertTrue(names.equals(pack.goods().stream().map(PackGood::name).toList()), "the core's goods aren't the pack's");
            Map<String, DialAdapter.Dial> dials = new java.util.HashMap<>();
            DialAdapter.list(host).forEach(d -> dials.put(d.name(), d));
            for (var dial : dev.b28.enterprise.Dials.all()) {
                if (ModPack.JAVA_ONLY.contains(dial.name)) {
                    helper.assertTrue(!dials.containsKey(dial.name), "the core has the mod's own dial " + dial.name);
                    continue;
                }
                DialAdapter.Dial view = dials.get(dial.name);
                helper.assertTrue(view != null, "the core has no dial " + dial.name + " (the mod's, with no rule of its own in the core)");
                helper.assertTrue(view.value() == dial.initial && view.initial() == dial.initial,
                        "dial " + dial.name + " starts at " + view.value() + " (default " + view.initial() + "), the mod's is " + dial.initial);
            }
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /** A dial set in the core is the world's, kept by its save, and put back to the pack's own setting, not the core's. */
    @GameTest(template = "floor")
    public static void aDialSetInTheCoreIsKeptByItsSaveAndResetToThePacks(GameTestHelper helper) {
        Path dir = folder();
        try {
            try (CoreHost host = CoreHost.open(dir, 7)) {
                helper.assertTrue(dial(host, "birth_mood").value() == 0.5, "the pack's birth_mood isn't the mod's 0.5 (the core's is 1)");
                helper.assertTrue(DialAdapter.set(host, "shelf_days", 7.0) == null, "shelf_days wasn't set");
                helper.assertTrue(DialAdapter.set(host, "birth_mood", 0.9) == null, "birth_mood wasn't set");
                helper.assertTrue(dial(host, "shelf_days").value() == 7.0 && dial(host, "shelf_days").initial() == 5.0,
                        "shelf_days is " + dial(host, "shelf_days"));
                String why = DialAdapter.set(host, "no_such_dial", 1.0);
                helper.assertTrue(why != null && why.contains("no dial called"), "an unknown dial: " + why);
                why = DialAdapter.set(host, "wagon_slots", -1.0);
                helper.assertTrue(why != null && why.contains("isn't a count"), "a count below nothing: " + why);
                helper.assertTrue(dial(host, "shelf_days").value() == 7.0, "a refused dial moved another");
                helper.assertTrue(host.save(), "the host didn't save");
            }
            try (CoreHost loaded = CoreHost.open(dir, 8)) {
                helper.assertTrue(dial(loaded, "shelf_days").value() == 7.0, "the loaded core lost shelf_days: " + dial(loaded, "shelf_days"));
                helper.assertTrue(DialAdapter.set(loaded, "birth_mood", null) == null, "birth_mood wasn't reset");
                helper.assertTrue(dial(loaded, "birth_mood").value() == 0.5,
                        "birth_mood went back to " + dial(loaded, "birth_mood").value() + ", not the pack's 0.5");
                helper.assertTrue(DialAdapter.set(loaded, "shelf_days", null) == null && dial(loaded, "shelf_days").value() == 5.0,
                        "shelf_days wasn't put back");
            }
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * {@code /enterprise dial} is routed (B4.1): in core mode the core's dial moves and the Java catalogue (no rule of
     * which runs) doesn't; in Java mode the Java rules' dial moves and the core beside them follows. Either way a
     * reset puts both back, within the test, so no other test sees the change.
     */
    @GameTest(template = "floor")
    public static void theDialCommandIsRoutedToTheCoreOrTheJavaRules(GameTestHelper helper) {
        var server = helper.getLevel().getServer();
        CoreHost host = CoreHost.of(server);
        helper.assertTrue(host != null && host.pausedWhy() == null, "the server's core isn't running");
        var dial = dev.b28.enterprise.Dials.SHELF_DAYS;
        boolean core = Society.isCore();
        try {
            server.getCommands().performPrefixedCommand(server.createCommandSourceStack(), "enterprise dial shelf_days 7");
            helper.assertTrue(dial(host, "shelf_days").value() == 7.0, "the core's shelf_days is " + dial(host, "shelf_days").value());
            helper.assertTrue(dial.get() == (core ? 5.0 : 7.0), "the Java dial is " + dial.get() + " in " + (core ? "core" : "Java") + " mode");
            // A name no one has is told so; nothing moves.
            server.getCommands().performPrefixedCommand(server.createCommandSourceStack(), "enterprise dial no_such_dial 3");
            helper.assertTrue(dial(host, "shelf_days").value() == 7.0, "an unknown dial moved another");
        } finally {
            server.getCommands().performPrefixedCommand(server.createCommandSourceStack(), "enterprise dial shelf_days reset");
        }
        helper.assertTrue(dial(host, "shelf_days").value() == 5.0 && dial.get() == 5.0,
                "shelf_days isn't back to 5: core " + dial(host, "shelf_days").value() + ", Java " + dial.get());
        helper.succeed();
    }

    private static DialAdapter.Dial dial(CoreHost host, String name) {
        return DialAdapter.list(host).stream().filter(d -> d.name().equals(name)).findFirst()
                .orElseThrow(() -> new AssertionError("the core has no dial " + name));
    }

    /**
     * The proof's measurements (docs/binding-plan.md B2.1), taken at the end of {@link #aDesksStocktakeComesBackFromTheCore}
     * only with {@code ENTERPRISE_CORE_MEASURE=1 just gametest} (a test of their own would change the suite's count).
     * Four desks round the arena's corners, each with five chests full (540 stacks of 15 goods), in a host of their own:
     * what a look costs the server's tick (counting the chests and handing the core its {@code Apply}; the look itself
     * runs on the core's thread) against a tick without the core (nothing), what the core's thread spends on a look, and
     * what a save of the society costs the tick that saves the world. Written to the log.
     */
    private static void measureALookAndASave(GameTestHelper helper) {
        var level = helper.getLevel();
        var server = level.getServer();
        List<net.minecraft.world.item.Item> goods = List.of(Items.WHEAT, Items.BREAD, Items.BOOK, Items.PAPER, Items.COAL,
                Items.OAK_LOG, Items.COBBLESTONE, Items.IRON_INGOT, Items.LEATHER, Items.STRING, Items.APPLE, Items.CARROT,
                Items.POTATO, Items.FLINT, Items.FEATHER);
        List<BlockPos> desks = List.of(new BlockPos(1, 2, 1), new BlockPos(1, 2, 5), new BlockPos(5, 2, 1),
                new BlockPos(5, 2, 5));
        int stacks = 0;
        for (BlockPos desk : desks) {
            helper.setBlock(desk, Merchant.DESK.get());
            for (BlockPos at : List.of(desk.east(), desk.west(), desk.north(), desk.south(), desk.above())) {
                helper.setBlock(at, Blocks.CHEST);
                ChestBlockEntity chest = (ChestBlockEntity) level.getBlockEntity(helper.absolutePos(at));
                for (int slot = 0; slot < chest.getContainerSize(); slot++) {
                    var good = goods.get(stacks++ % goods.size());
                    chest.setItem(slot, new ItemStack(good, good.getDefaultMaxStackSize()));
                }
            }
        }
        Path dir = folder();
        StringBuilder out = new StringBuilder("core measurements:");
        try (CoreHost host = CoreHost.open(dir, 7)) {
            for (int i = 0; i < desks.size(); i++) {
                host.stock().found(level, helper.absolutePos(desks.get(i)), "Measure " + i);
            }
            long t = level.getGameTime();
            int looks = 400;
            long[] tick = new long[looks];
            long start = System.nanoTime();
            for (int i = 0; i < looks; i++) {
                // a stack moved each look, so every stocktake books something
                ChestBlockEntity chest = (ChestBlockEntity) level.getBlockEntity(helper.absolutePos(desks.get(i % desks.size()).east()));
                chest.setItem(0, new ItemStack(goods.get(0), 1 + i % 64));
                long a = System.nanoTime();
                host.look(server, t + 1200L * (i + 1));
                host.poll();
                tick[i] = System.nanoTime() - a;
            }
            host.settle();
            long all = System.nanoTime() - start;
            java.util.Arrays.sort(tick);
            double mean = java.util.Arrays.stream(tick).average().orElse(0) / 1e6;
            out.append("\n  %d desks, %d stacks: a look on the server's tick %.3f ms mean, p50 %.3f, p99 %.3f, max %.3f (without the core: 0)"
                    .formatted(desks.size(), stacks, mean, tick[looks / 2] / 1e6, tick[looks * 99 / 100] / 1e6, tick[looks - 1] / 1e6));
            out.append("\n  the core's thread: %d looks (a game minute each) in %.1f ms, %.3f ms a look"
                    .formatted(looks, all / 1e6, all / 1e6 / looks));
            long[] saves = new long[20];
            for (int i = 0; i < saves.length; i++) {
                long a = System.nanoTime();
                host.save();
                saves[i] = System.nanoTime() - a;
            }
            java.util.Arrays.sort(saves);
            long bytes;
            try {
                bytes = Files.size(dir.resolve(SaveStore.FILE));
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
            out.append("\n  a save (%d bytes): %.3f ms p50, %.3f max, on the tick that saves the world"
                    .formatted(bytes, saves[saves.length / 2] / 1e6, saves[saves.length - 1] / 1e6));
        } finally {
            delete(dir);
        }
        Enterprise.LOGGER.info(out.toString());
    }

    private static Path folder() {
        try {
            return Files.createTempDirectory("enterprise-core-test");
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
