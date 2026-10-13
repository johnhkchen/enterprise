package dev.b28.enterprise;

import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.BoolArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.context.CommandContext;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import dev.b28.enterprise.locker.Locker;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.MerchantView;
import java.util.List;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.commands.arguments.coordinates.BlockPosArgument;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.network.chat.Component;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.npc.Villager;
import net.neoforged.neoforge.event.RegisterCommandsEvent;

/** Operator commands for inspecting Enterprise state in a running world. */
final class EnterpriseCommands {
    private EnterpriseCommands() {}

    static void register(RegisterCommandsEvent event) {
        CommandDispatcher<CommandSourceStack> dispatcher = event.getDispatcher();
        dispatcher.register(Commands.literal("enterprise")
                .requires(source -> source.hasPermission(2))
                .then(Commands.literal("lockers")
                        .then(Commands.argument("desk", BlockPosArgument.blockPos())
                                .executes(EnterpriseCommands::lockers)))
                .then(Commands.literal("stock")
                        .then(Commands.argument("desk", BlockPosArgument.blockPos())
                                .executes(EnterpriseCommands::stock)))
                .then(Commands.literal("switch")
                        .then(Commands.argument("desk", BlockPosArgument.blockPos())
                                .then(Commands.argument("workstation", BlockPosArgument.blockPos())
                                        .then(Commands.argument("trade", StringArgumentType.string())
                                                .then(Commands.argument("on", BoolArgumentType.bool())
                                                        .executes(EnterpriseCommands::switchTrade)))))));
    }

    /** Lists what the Merchant at a desk sees: its stock containers and the lockers it manages. */
    private static int lockers(CommandContext<CommandSourceStack> context) throws CommandSyntaxException {
        CommandSourceStack source = context.getSource();
        ServerLevel level = source.getLevel();
        BlockPos desk = BlockPosArgument.getLoadedBlockPos(context, "desk");
        if (!level.getBlockState(desk).is(Merchant.DESK.get())) {
            source.sendFailure(Component.literal("No Merchant's Desk at " + desk.toShortString()));
            return 0;
        }

        Villager merchant = Merchant.workingAt(level, desk, Lockers.MERCHANT_RANGE).orElse(null);
        if (merchant == null) {
            source.sendFailure(Component.literal("No Merchant works at this desk, so its lockers are idle"));
            return 0;
        }

        int limit = Merchant.lockerLimit(merchant.getVillagerData().getLevel());
        List<BlockPos> stock = Lockers.touchingContainers(level, desk);
        // Every locker in reach, including those beyond this Merchant's level limit (marked as waiting).
        List<Locker> lockers = Lockers.near(level, desk, Lockers.MERCHANT_RANGE, Integer.MAX_VALUE);
        MerchantView view = MerchantView.build(level, desk, Integer.MAX_VALUE);
        int managed = Math.min(limit, lockers.size());
        source.sendSuccess(() -> Component.literal("Merchant's Desk at %s: %d stock container(s), %d/%d locker(s), %d waiting"
                .formatted(desk.toShortString(), stock.size(), managed, limit, lockers.size() - managed)), false);
        for (int i = 0; i < lockers.size(); i++) {
            Locker locker = lockers.get(i);
            String status = i < limit ? "" : " (waiting for a higher Merchant level)";
            String job = locker.type().unwrapKey().map(key -> key.location().getPath()).orElse("?");
            String worker = Merchant.workingAt(level, locker.workstation(), Lockers.MERCHANT_RANGE)
                    .map(villager -> villager.getName().getString())
                    .orElse("unclaimed");
            int slots = Lockers.inventory(level, locker.containers()).getSlots();
            source.sendSuccess(() -> Component.literal("  %s at %s, %s, %d slots%s"
                    .formatted(job, locker.workstation().toShortString(), worker, slots, status)), false);
            view.lockers().stream()
                    .filter(entry -> entry.workstation().equals(locker.workstation()))
                    .flatMap(entry -> entry.trades().stream())
                    .forEach(trade -> source.sendSuccess(() -> Component.literal("    [%s] %s"
                            .formatted(trade.enabled() ? "on" : "off", trade.key())), false));
        }
        return managed;
    }

    /** Totals of everything in a Merchant's stock, shulker contents included. */
    private static int stock(CommandContext<CommandSourceStack> context) throws CommandSyntaxException {
        CommandSourceStack source = context.getSource();
        BlockPos desk = BlockPosArgument.getLoadedBlockPos(context, "desk");
        var totals = MerchantStock.at(source.getLevel(), desk).totals();
        source.sendSuccess(() -> Component.literal("Stock at %s: %d kinds".formatted(desk.toShortString(), totals.size())), false);
        totals.forEach((item, count) -> source.sendSuccess(() -> Component.literal("  %s %d".formatted(item, count)), false));
        return totals.size();
    }

    /** Switches one locker trade on or off without the screen (for servers and tests). */
    private static int switchTrade(CommandContext<CommandSourceStack> context) throws CommandSyntaxException {
        CommandSourceStack source = context.getSource();
        BlockPos desk = BlockPosArgument.getLoadedBlockPos(context, "desk");
        BlockPos workstation = BlockPosArgument.getLoadedBlockPos(context, "workstation");
        String trade = StringArgumentType.getString(context, "trade");
        boolean on = BoolArgumentType.getBool(context, "on");
        MerchantBook.get(source.getServer())
                .setEnabled(GlobalPos.of(source.getLevel().dimension(), desk), workstation, trade, on);
        source.sendSuccess(() -> Component.literal("%s %s at %s"
                .formatted(on ? "Enabled" : "Disabled", trade, workstation.toShortString())), true);
        return 1;
    }
}
