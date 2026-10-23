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
                                .executes(LedgerCommand::ledger))));
    }

    private static int ledger(CommandContext<CommandSourceStack> context) throws CommandSyntaxException {
        CommandSourceStack source = context.getSource();
        ServerLevel level = source.getLevel();
        BlockPos desk = BlockPosArgument.getLoadedBlockPos(context, "desk");
        CoreHost host = CoreHost.of(source.getServer());
        if (host == null || host.pausedWhy() != null) {
            source.sendFailure(Component.literal(host == null ? "The core didn't start; the world runs without it" :
                    "The society is paused: " + host.pausedWhy()));
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
