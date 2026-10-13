package dev.b28.enterprise;

import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.context.CommandContext;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import dev.b28.enterprise.locker.Locker;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.merchant.Merchant;
import java.util.List;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.commands.arguments.coordinates.BlockPosArgument;
import net.minecraft.core.BlockPos;
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
                                .executes(EnterpriseCommands::lockers))));
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
        List<Locker> lockers = Lockers.near(level, desk, Lockers.MERCHANT_RANGE, limit);
        source.sendSuccess(() -> Component.literal("Merchant's Desk at %s: %d stock container(s), %d/%d locker(s)"
                .formatted(desk.toShortString(), stock.size(), lockers.size(), limit)), false);
        for (Locker locker : lockers) {
            String job = locker.type().unwrapKey().map(key -> key.location().getPath()).orElse("?");
            String worker = Merchant.workingAt(level, locker.workstation(), Lockers.MERCHANT_RANGE)
                    .map(villager -> villager.getName().getString())
                    .orElse("unclaimed");
            int slots = Lockers.inventory(level, locker.containers()).getSlots();
            source.sendSuccess(() -> Component.literal("  %s at %s, %s, %d slots"
                    .formatted(job, locker.workstation().toShortString(), worker, slots)), false);
        }
        return lockers.size();
    }
}
