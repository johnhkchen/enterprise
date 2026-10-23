package dev.b28.enterprise;

import com.mojang.brigadier.context.CommandContext;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.binding.adapters.LedgerAdapter;
import dev.b28.enterprise.merchant.LedgerView;
import dev.b28.enterprise.merchant.Merchant;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.commands.arguments.coordinates.BlockPosArgument;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.network.chat.Component;
import net.minecraft.server.level.ServerLevel;
import net.neoforged.neoforge.event.RegisterCommandsEvent;

/**
 * {@code /enterprise ledger <desk>} (docs/alpha-plan.md 3): the lines of the Merchant screen's Ledger tab, for a desk with no
 * Merchant to open it and for a check over RCON, which reads exactly what a player sees. Registered here, not in {@link
 * EnterpriseCommands}: Brigadier merges this {@code enterprise} node into that one.
 */
final class LedgerCommand {
    private LedgerCommand() {}

    static void register(RegisterCommandsEvent event) {
        event.getDispatcher().register(Commands.literal("enterprise")
                .requires(source -> source.hasPermission(2))
                .then(Commands.literal("ledger")
                        .then(Commands.argument("desk", BlockPosArgument.blockPos())
                                .executes(LedgerCommand::ledger)))
                .then(Commands.literal("core")
                        .then(Commands.literal("version").executes(LedgerCommand::version))
                        .then(Commands.literal("report").executes(LedgerCommand::report))
                        .then(Commands.literal("journal").executes(LedgerCommand::journal))));
    }

    /**
     * {@code /enterprise core version} (docs/alpha-plan.md 4): the mod's version, the core's commit and the save format it
     * writes (as a save made now says), the wire's schema and the society this game runs.
     */
    private static int version(CommandContext<CommandSourceStack> context) {
        CommandSourceStack source = context.getSource();
        source.sendSuccess(() -> Component.literal(versionLine(CoreHost.of(source.getServer()))), false);
        return 1;
    }

    static String versionLine(CoreHost host) {
        String mod = net.neoforged.fml.ModList.get().getModContainerById(Enterprise.MOD_ID)
                .map(c -> c.getModInfo().getVersion().toString()).orElse("?");
        var save = host == null ? null : host.saveNow();
        String core = save == null ? "core ? (not running)" : "core %s · format %d".formatted(save.core(), save.format());
        return "enterprise %s · %s · schema %016x · society %s".formatted(mod, core, dev.b28.enterprise.wire.Wire.SCHEMA,
                dev.b28.enterprise.binding.Society.mode().name().toLowerCase(java.util.Locale.ROOT));
    }

    /**
     * {@code /enterprise core report} (docs/alpha-plan.md 4): a bug report's folder, {@code
     * world/enterprise-reports/day<N>-<time>/}, with the version line and why the society is paused (if it is), every
     * town's ledger, its cash book for 30 days and its whys, a copy of the core's save, and this run's journal so far
     * (`enterprise-sim trace` replays it). The core is deterministic, so the save and the steps are a reproduction.
     */
    private static int report(CommandContext<CommandSourceStack> context) {
        CommandSourceStack source = context.getSource();
        var server = source.getServer();
        CoreHost host = CoreHost.of(server);
        java.nio.file.Path world = server.getWorldPath(net.minecraft.world.level.storage.LevelResource.ROOT)
                .toAbsolutePath().normalize();
        String stamp = "day%d-%s".formatted(server.overworld().getDayTime() / 24000,
                java.time.LocalTime.now().format(java.time.format.DateTimeFormatter.ofPattern("HHmmss")));
        java.nio.file.Path dir = world.resolve("enterprise-reports").resolve(stamp);
        java.util.List<String> out = new java.util.ArrayList<>();
        out.add(versionLine(host)); // a save made now: the copy below is this moment's
        String why = dev.b28.enterprise.binding.PausedNotice.why(host);
        out.add(why == null ? "running" : why);
        if (why == null) {
            var books = host.books();
            host.points().of(dev.b28.enterprise.binding.adapters.PointsAdapter.Kind.DESK).forEach((desk, town) -> {
                out.add("");
                out.add("######## town " + town + " at " + desk.pos().toShortString());
                out.addAll(new LedgerAdapter(host).view(desk).toText());
                out.add("== Cash book, 30 days ==");
                out.addAll(books.cashBook(town, 30));
                out.add("== Whys ==");
                out.addAll(books.whys(town));
            });
        }
        try {
            java.nio.file.Files.createDirectories(dir);
            java.nio.file.Files.write(dir.resolve("report.txt"), out);
            java.nio.file.Path save = world.resolve(dev.b28.enterprise.core.SaveStore.FILE);
            if (java.nio.file.Files.isRegularFile(save)) {
                java.nio.file.Files.copy(save, dir.resolve(save.getFileName()));
            }
            java.nio.file.Path journal = host == null ? null : host.journal();
            if (journal != null && java.nio.file.Files.isRegularFile(journal)) {
                java.nio.file.Files.copy(journal, dir.resolve(journal.getFileName()));
            }
        } catch (java.io.IOException e) {
            source.sendFailure(Component.literal("Couldn't write the report to " + dir + ": " + e));
            return 0;
        }
        source.sendSuccess(() -> Component.literal("A bug report is in " + dir
                + ". Zip that folder with logs/latest.log and send both."), false);
        return 1;
    }

    /** {@code /enterprise core journal}: where this run's journal is, and how to read it. */
    private static int journal(CommandContext<CommandSourceStack> context) {
        CommandSourceStack source = context.getSource();
        CoreHost host = CoreHost.of(source.getServer());
        java.nio.file.Path journal = host == null ? null : host.journal();
        if (journal == null) {
            source.sendFailure(Component.literal("No journal this run (journal=off, or it failed: see logs/latest.log)"));
            return 0;
        }
        source.sendSuccess(() -> Component.literal("The core's journal: " + journal.toAbsolutePath().normalize()
                + ". Read it with: cd sim && cargo run --release -p enterprise-sim -- --trace <that file> run.db"), false);
        return 1;
    }

    private static int ledger(CommandContext<CommandSourceStack> context) throws CommandSyntaxException {
        CommandSourceStack source = context.getSource();
        ServerLevel level = source.getLevel();
        BlockPos desk = BlockPosArgument.getLoadedBlockPos(context, "desk");
        CoreHost host = CoreHost.of(source.getServer());
        String why = dev.b28.enterprise.binding.PausedNotice.why(host);
        if (why != null) {
            source.sendFailure(Component.literal(why));
            return 0;
        }
        GlobalPos at = GlobalPos.of(level.dimension(), desk);
        if (host.stock().townAt(at) == null) {
            if (!level.getBlockState(desk).is(Merchant.DESK.get())) {
                source.sendFailure(Component.literal("No Merchant's Desk at " + desk.toShortString()));
                return 0;
            }
            // A desk already standing when the server started isn't the core's yet: a town at it, as `core stock` does.
            host.stock().found(level, desk, dev.b28.enterprise.settlement.Names.of(source.getServer(), at));
        }
        LedgerView view = new LedgerAdapter(host).view(at);
        source.sendSuccess(() -> Component.literal(String.join("\n", view.toText())), false);
        return view.sections().size();
    }
}
