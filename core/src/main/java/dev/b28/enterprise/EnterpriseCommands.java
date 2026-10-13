package dev.b28.enterprise;

import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.BoolArgumentType;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.context.CommandContext;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.caravan.Caravans;
import dev.b28.enterprise.caravan.Route;
import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.locker.Locker;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.MerchantView;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.settlement.Census;
import dev.b28.enterprise.settlement.CensusTaker;
import dev.b28.enterprise.settlement.Settlements;
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
                .then(Commands.literal("mint").executes(EnterpriseCommands::mint))
                .then(Commands.literal("census")
                        .then(Commands.argument("bell", BlockPosArgument.blockPos())
                                .executes(EnterpriseCommands::census)))
                .then(Commands.literal("settlements").executes(EnterpriseCommands::settlements))
                .then(Commands.literal("hex")
                        .executes(context -> hex(context, BlockPos.containing(context.getSource().getPosition())))
                        .then(Commands.argument("pos", BlockPosArgument.blockPos())
                                .executes(context -> hex(context, BlockPosArgument.getBlockPos(context, "pos")))))
                .then(Commands.literal("route")
                        .then(Commands.literal("add")
                                .then(Commands.argument("from", BlockPosArgument.blockPos())
                                        .then(Commands.argument("to", BlockPosArgument.blockPos())
                                                .executes(EnterpriseCommands::addRoute))))
                        .then(Commands.literal("remove")
                                .then(Commands.argument("id", IntegerArgumentType.integer(1))
                                        .executes(EnterpriseCommands::removeRoute))))
                .then(Commands.literal("routes").executes(EnterpriseCommands::routes))
                .then(Commands.literal("territory")
                        .then(Commands.argument("bell", BlockPosArgument.blockPos())
                                .executes(EnterpriseCommands::territory)))
                .then(Commands.literal("autoswitch")
                        .then(Commands.argument("desk", BlockPosArgument.blockPos())
                                .then(Commands.literal("selling").executes(context -> autoswitch(context, false)))
                                .then(Commands.literal("all").executes(context -> autoswitch(context, true)))))
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
        dev.b28.enterprise.money.MoneyChanger.bankAt(source.getLevel(), desk).ifPresent(bank -> source.sendSuccess(
                () -> Component.literal("  Blaze Banker: %d spurs".formatted(bank.getBalance())), false));
        totals.forEach((item, count) -> source.sendSuccess(() -> Component.literal("  %s %d".formatted(item, count)), false));
        return totals.size();
    }

    private static int hex(CommandContext<CommandSourceStack> context, BlockPos pos) {
        CommandSourceStack source = context.getSource();
        Hex hex = Hex.at(pos);
        String owner = dev.b28.enterprise.territory.Territory.get(source.getServer())
                .owner(source.getLevel().dimension(), hex)
                .map(bell -> "held by the settlement at " + bell.pos().toShortString())
                .orElse("unclaimed");
        source.sendSuccess(() -> Component.literal("Hex %s, centre %.0f %.0f, %s".formatted(
                hex, hex.centerX(), hex.centerZ(), owner)), false);
        return 1;
    }

    /** A settlement's hexes, culture, farm plan and tolls owed. */
    private static int territory(CommandContext<CommandSourceStack> context) throws CommandSyntaxException {
        CommandSourceStack source = context.getSource();
        GlobalPos bell = GlobalPos.of(source.getLevel().dimension(), BlockPosArgument.getBlockPos(context, "bell"));
        var territory = dev.b28.enterprise.territory.Territory.get(source.getServer());
        var hexes = territory.hexes(bell);
        source.sendSuccess(() -> Component.literal("Settlement at %s holds %d hex(es), culture %.1f of %.1f for the next"
                .formatted(bell.pos().toShortString(), hexes.size(), territory.culture(bell), territory.nextHexCost(bell))), false);
        source.sendSuccess(() -> Component.literal("  hexes " + hexes), false);
        territory.farmPlan(bell).forEach((item, perDay) -> source.sendSuccess(
                () -> Component.literal("  farms %s: %.1f a day".formatted(item, perDay)), false));
        if (territory.tollsOwed(bell) > 0) {
            source.sendSuccess(() -> Component.literal("  tolls owed: %d spurs".formatted(territory.tollsOwed(bell))), false);
        }
        return hexes.size();
    }

    /** A caravan route from a hub Merchant's Desk to a settlement's Merchant's Desk, if food allows the distance. */
    private static int addRoute(CommandContext<CommandSourceStack> context) throws CommandSyntaxException {
        CommandSourceStack source = context.getSource();
        ServerLevel level = source.getLevel();
        BlockPos from = BlockPosArgument.getBlockPos(context, "from");
        BlockPos to = BlockPosArgument.getBlockPos(context, "to");
        for (BlockPos desk : List.of(from, to)) {
            if (level.isLoaded(desk) && !level.getBlockState(desk).is(Merchant.DESK.get())) {
                source.sendFailure(Component.literal("No Merchant's Desk at " + desk.toShortString()));
                return 0;
            }
        }
        double dx = from.getX() - to.getX();
        double dz = from.getZ() - to.getZ();
        int distance = (int) Math.ceil(Math.sqrt(dx * dx + dz * dz));
        if (!Caravans.inRange(distance)) {
            source.sendFailure(Component.literal(("Too far: %d blocks needs %d food each way, and a caravan carries at "
                    + "most %d. Add a settlement in between as a waystation.").formatted(
                    distance, Caravans.foodNeeded(distance), Caravans.FOOD_CAPACITY)));
            return 0;
        }
        Route route = CaravanLedger.get(source.getServer()).addRoute(
                GlobalPos.of(level.dimension(), from), GlobalPos.of(level.dimension(), to));
        source.sendSuccess(() -> Component.literal(("Route %d: %d blocks over %d hexes, %.2f days each way, "
                + "%d food each way").formatted(route.id(), distance, route.hexes(),
                Caravans.travelTicks(distance) / 24000.0, Caravans.foodNeeded(distance))), true);
        return route.id();
    }

    private static int removeRoute(CommandContext<CommandSourceStack> context) {
        int id = IntegerArgumentType.getInteger(context, "id");
        boolean removed = CaravanLedger.get(context.getSource().getServer()).removeRoute(id);
        context.getSource().sendSuccess(() -> Component.literal(removed ? "Removed route " + id : "No route " + id), true);
        return removed ? 1 : 0;
    }

    /** Every route with its totals and where its caravan is. */
    private static int routes(CommandContext<CommandSourceStack> context) {
        CommandSourceStack source = context.getSource();
        CaravanLedger ledger = CaravanLedger.get(source.getServer());
        source.sendSuccess(() -> Component.literal("%d route(s)".formatted(ledger.routes().size())), false);
        for (Route route : ledger.routes()) {
            String caravan = ledger.caravan(route.id())
                    .map(c -> " | caravan %s, %d items, %d spurs".formatted(
                            c.leg().name().toLowerCase().replace('_', ' '), c.items(), c.spurs()))
                    .orElse("");
            source.sendSuccess(() -> Component.literal(("  #%d %s -> %s: %d blocks, %d trips, %d items delivered, "
                    + "%d spurs collected, %d tolls paid, %d food eaten, %s%s").formatted(route.id(),
                    route.from().pos().toShortString(), route.to().pos().toShortString(), route.distance(), route.trips(),
                    route.itemsDelivered(), route.spursCollected(), route.tollsPaid(), route.foodEaten(), route.status(),
                    caravan)), false);
        }
        return ledger.routes().size();
    }

    /** Takes a census around a bell right now and prints it. */
    private static int census(CommandContext<CommandSourceStack> context) throws CommandSyntaxException {
        CommandSourceStack source = context.getSource();
        BlockPos bell = BlockPosArgument.getLoadedBlockPos(context, "bell");
        Census result = CensusTaker.countNow(source.getLevel(), bell, CensusTaker.RADIUS);
        source.sendSuccess(() -> Component.literal(describe(bell, result)), false);
        return result.isSettlement() ? 1 : 0;
    }

    /** Every settlement with a census on record. */
    private static int settlements(CommandContext<CommandSourceStack> context) {
        CommandSourceStack source = context.getSource();
        var all = Settlements.get(source.getServer()).all();
        source.sendSuccess(() -> Component.literal("%d bell(s) counted".formatted(all.size())), false);
        all.forEach((bell, result) -> source.sendSuccess(() -> Component.literal("  " + describe(bell.pos(), result)), false));
        return all.size();
    }

    private static String describe(BlockPos bell, Census result) {
        return "%s at %s: %d villagers (%d free), %d beds, %d workstations, value %.1f".formatted(
                result.isSettlement() ? "Settlement" : "Not a settlement", bell.toShortString(), result.villagers(),
                result.free(), result.beds(), result.workstations(), result.value());
    }

    /** The default faction's emerald rate and selling pressure. */
    private static int mint(CommandContext<CommandSourceStack> context) {
        CommandSourceStack source = context.getSource();
        Mint mint = Mint.get(source.getServer());
        long now = source.getLevel().getGameTime();
        source.sendSuccess(() -> Component.literal("%s: %.2f spurs per emerald, selling pressure %.0f emeralds"
                .formatted(Mint.DEFAULT_FACTION, mint.rate(Mint.DEFAULT_FACTION, now),
                        mint.pressure(Mint.DEFAULT_FACTION, now))), false);
        return 1;
    }

    /**
     * Switches on, for every locker in reach of a desk (managed or waiting) that has nothing switched on yet, the
     * trades that earn emeralds, or every trade. Lockers someone has already set up are left alone. For test worlds
     * and servers setting up a market.
     */
    private static int autoswitch(CommandContext<CommandSourceStack> context, boolean all) throws CommandSyntaxException {
        CommandSourceStack source = context.getSource();
        ServerLevel level = source.getLevel();
        BlockPos desk = BlockPosArgument.getLoadedBlockPos(context, "desk");
        GlobalPos deskPos = GlobalPos.of(level.dimension(), desk);
        MerchantBook book = MerchantBook.get(source.getServer());
        int switched = 0;
        for (MerchantView.LockerView locker : MerchantView.build(level, desk, Integer.MAX_VALUE).lockers()) {
            if (!book.enabledTrades(deskPos, locker.workstation()).isEmpty()) {
                continue;
            }
            for (MerchantView.TradeView trade : locker.trades()) {
                if (!trade.enabled() && (all || trade.key().endsWith(">minecraft:emerald"))) {
                    book.setEnabled(deskPos, locker.workstation(), trade.key(), true);
                    switched++;
                }
            }
        }
        int count = switched;
        source.sendSuccess(() -> Component.literal("Switched on %d trade(s)".formatted(count)), true);
        return switched;
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
