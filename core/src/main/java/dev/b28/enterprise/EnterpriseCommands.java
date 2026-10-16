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
import java.util.Map;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.commands.arguments.coordinates.BlockPosArgument;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
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
                .then(Commands.literal("survey")
                        .then(Commands.argument("from", BlockPosArgument.blockPos())
                                .then(Commands.argument("to", BlockPosArgument.blockPos()).executes(context -> {
                                    // The land in a box surveyed now (water and heights), for chunks that are loaded:
                                    // otherwise it's seen only as chunks load and unload.
                                    var level = context.getSource().getLevel();
                                    BlockPos a = BlockPosArgument.getBlockPos(context, "from");
                                    BlockPos b = BlockPosArgument.getBlockPos(context, "to");
                                    var terrain = dev.b28.enterprise.territory.Terrain.get(level.getServer());
                                    int surveyed = 0;
                                    for (int cx = Math.min(a.getX(), b.getX()) >> 4; cx <= Math.max(a.getX(), b.getX()) >> 4; cx++) {
                                        for (int cz = Math.min(a.getZ(), b.getZ()) >> 4; cz <= Math.max(a.getZ(), b.getZ()) >> 4; cz++) {
                                            var chunk = level.getChunkSource().getChunkNow(cx, cz);
                                            if (chunk != null) {
                                                long pos = net.minecraft.world.level.ChunkPos.asLong(cx, cz);
                                                terrain.record(level, pos, dev.b28.enterprise.territory.Terrain.survey(chunk));
                                                terrain.recordHeights(level, pos, dev.b28.enterprise.territory.Terrain.surveyHeights(chunk));
                                                surveyed++;
                                            }
                                        }
                                    }
                                    int done = surveyed;
                                    context.getSource().sendSuccess(() -> Component.literal(done + " chunks surveyed"), false);
                                    return done;
                                }))))
                .then(Commands.literal("books")
                        .then(Commands.argument("desk", BlockPosArgument.blockPos()).executes(context -> {
                            // A market's books, as its Merchant keeps them: dealings by kind, counterparty and good.
                            var server = context.getSource().getServer();
                            var desk = net.minecraft.core.GlobalPos.of(context.getSource().getLevel().dimension(),
                                    BlockPosArgument.getBlockPos(context, "desk"));
                            var summary = dev.b28.enterprise.merchant.Books.get(server).summary(server, desk);
                            StringBuilder out = new StringBuilder("Books of %s (kept %d days): %d lines".formatted(
                                    dev.b28.enterprise.settlement.Names.label(desk),
                                    dev.b28.enterprise.merchant.Books.keeps(server, desk), summary.size()));
                            summary.forEach((key, t) -> out.append("\n  %s: %d for %d spurs".formatted(key, t[0], t[1])));
                            // And its reasons: why goods went or didn't, why its workshop stood idle.
                            var whys = dev.b28.enterprise.merchant.Books.get(server).whys(desk);
                            out.append("\nReasons (%d):".formatted(whys.size()));
                            whys.forEach((key, t) -> out.append("\n  why | %s: %d times, %d items, last day %d"
                                    .formatted(key, t[0], t[1], t[2])));
                            context.getSource().sendSuccess(() -> Component.literal(out.toString()), false);
                            return summary.size();
                        })))
                .then(Commands.literal("whys")
                        .then(Commands.argument("desk", BlockPosArgument.blockPos()).executes(context -> {
                            // A market's reasons alone: why goods went or didn't, why its workshop stood idle.
                            var server = context.getSource().getServer();
                            var desk = net.minecraft.core.GlobalPos.of(context.getSource().getLevel().dimension(),
                                    BlockPosArgument.getBlockPos(context, "desk"));
                            var whys = dev.b28.enterprise.merchant.Books.get(server).whys(desk);
                            StringBuilder out = new StringBuilder("Reasons of %s (%d):".formatted(
                                    dev.b28.enterprise.settlement.Names.label(desk), whys.size()));
                            whys.forEach((key, t) -> out.append("\n  %s: %d times, %d items, last day %d"
                                    .formatted(key, t[0], t[1], t[2])));
                            context.getSource().sendSuccess(() -> Component.literal(out.toString()), false);
                            return whys.size();
                        })))
                .then(Commands.literal("realms").executes(context -> {
                    // The realms beyond the border: their trade with ours, and the customs paid.
                    var lines = dev.b28.enterprise.trade.Realms.get(context.getSource().getServer())
                            .report(context.getSource().getServer());
                    context.getSource().sendSuccess(() -> Component.literal("Realms (%d):\n  ".formatted(lines.size())
                            + String.join("\n  ", lines)), false);
                    return lines.size();
                })
                        .then(Commands.literal("open")
                                .then(Commands.argument("realm", com.mojang.brigadier.arguments.StringArgumentType.word())
                                        .then(Commands.argument("home", BlockPosArgument.blockPos())
                                                .then(Commands.argument("partners", com.mojang.brigadier.arguments.StringArgumentType.greedyString())
                                                        .executes(context -> {
                    // Opens a realm's market off the map at {home}, trading with the desks listed ("x y z; x y z").
                    var level = context.getSource().getLevel();
                    String id = com.mojang.brigadier.arguments.StringArgumentType.getString(context, "realm");
                    var home = BlockPosArgument.getBlockPos(context, "home");
                    java.util.List<net.minecraft.core.GlobalPos> partners = new java.util.ArrayList<>();
                    for (String part : com.mojang.brigadier.arguments.StringArgumentType.getString(context, "partners").split(";")) {
                        String[] xyz = part.trim().split("\\s+");
                        partners.add(net.minecraft.core.GlobalPos.of(level.dimension(), new net.minecraft.core.BlockPos(
                                Integer.parseInt(xyz[0]), Integer.parseInt(xyz[1]), Integer.parseInt(xyz[2]))));
                    }
                    var realm = dev.b28.enterprise.trade.Realms.get(level.getServer()).open(level, id, home, 20000, partners);
                    context.getSource().sendSuccess(() -> Component.literal("%s opens trade with %d of our markets"
                            .formatted(realm.name(), partners.size())), false);
                    return 1;
                }))))))
                .then(Commands.literal("expeditions").executes(context -> {
                    // Settlers on the road, and the villages settled and towns founded.
                    var server = context.getSource().getServer();
                    var lines = dev.b28.enterprise.caravan.Expeditions.get(server).report(server.overworld().getGameTime());
                    context.getSource().sendSuccess(() -> Component.literal("Expeditions (%d):\n  ".formatted(lines.size())
                            + String.join("\n  ", lines)), false);
                    return lines.size();
                }))
                .then(Commands.literal("cottage")
                        .then(Commands.argument("corner", BlockPosArgument.blockPos()).executes(context -> {
                            // A family's cottage (two beds) with its corner at {corner}: a settlement's starter home.
                            var level = context.getSource().getLevel();
                            var corner = BlockPosArgument.getBlockPos(context, "corner");
                            var head = dev.b28.enterprise.territory.Growth.buildHome(level, corner);
                            context.getSource().sendSuccess(() -> Component.literal(
                                    "A cottage at " + corner.toShortString() + ", beds from " + head.toShortString()), false);
                            return 1;
                        })))
                .then(Commands.literal("bridge")
                        .then(Commands.argument("at", BlockPosArgument.blockPos())
                                .then(Commands.argument("desk", BlockPosArgument.blockPos()).executes(context -> {
                                    // A finished bridge on the hex at {at}, its town's (the market at {desk}): a deck
                                    // across the hex, between the neighbours either side of the way through.
                                    var level = context.getSource().getLevel();
                                    var hex = dev.b28.enterprise.hex.Hex.at(BlockPosArgument.getBlockPos(context, "at"));
                                    var desk = net.minecraft.core.GlobalPos.of(level.dimension(),
                                            BlockPosArgument.getBlockPos(context, "desk"));
                                    var territory = dev.b28.enterprise.territory.Territory.get(level.getServer());
                                    var owner = territory.bellOf(desk).orElse(desk);
                                    var near = hex.neighbors();
                                    var works = dev.b28.enterprise.territory.Works.get(level.getServer());
                                    // (across the hex north-west to south-east: from one side of a ridge to the other)
                                    var bridge = works.start(owner, level.dimension(), hex, near.get(2), near.get(5));
                                    works.finish(bridge);
                                    context.getSource().sendSuccess(() -> Component.literal("A bridge on " + hex + ", "
                                            + dev.b28.enterprise.settlement.Names.label(desk) + "'s"), true);
                                    return 1;
                                }))))
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
                .then(Commands.literal("dial")
                        .executes(context -> dials(context.getSource()))
                        .then(Commands.argument("name", com.mojang.brigadier.arguments.StringArgumentType.word())
                                .suggests((context, builder) -> net.minecraft.commands.SharedSuggestionProvider.suggest(
                                        Dials.all().stream().map(d -> d.name), builder))
                                .then(Commands.literal("reset").executes(context -> setDial(context.getSource(),
                                        com.mojang.brigadier.arguments.StringArgumentType.getString(context, "name"), null)))
                                .then(Commands.argument("value", com.mojang.brigadier.arguments.DoubleArgumentType.doubleArg())
                                        .executes(context -> setDial(context.getSource(),
                                                com.mojang.brigadier.arguments.StringArgumentType.getString(context, "name"),
                                                com.mojang.brigadier.arguments.DoubleArgumentType.getDouble(context, "value"))))))
                .then(Commands.literal("contracts")
                        .executes(context -> contracts(context, null))
                        .then(Commands.argument("desk", BlockPosArgument.blockPos())
                                .executes(context -> contracts(context, BlockPosArgument.getBlockPos(context, "desk")))))
                .then(Commands.literal("war")
                        .executes(EnterpriseCommands::war)
                        .then(Commands.literal("capital")
                                .then(Commands.argument("desk", BlockPosArgument.blockPos())
                                        .then(Commands.argument("spursPerDay", IntegerArgumentType.integer(0))
                                                .executes(context -> {
                                                    var source = context.getSource();
                                                    dev.b28.enterprise.market.Sovereign.get(source.getServer()).establish(
                                                            source.getLevel(), GlobalPos.of(source.getLevel().dimension(),
                                                                    BlockPosArgument.getBlockPos(context, "desk")),
                                                            IntegerArgumentType.getInteger(context, "spursPerDay"));
                                                    source.sendSuccess(() -> Component.literal(
                                                            "The capital: its treasury takes the crown's revenue, its port's "
                                                                    + "metal exchange is open"), true);
                                                    return 1;
                                                }))))
                        .then(Commands.literal("mobilise")
                                .then(Commands.argument("desk", BlockPosArgument.blockPos())
                                        .then(Commands.argument("spursPerDay", IntegerArgumentType.integer(0))
                                                .executes(context -> {
                                                    var source = context.getSource();
                                                    dev.b28.enterprise.market.Sovereign.get(source.getServer()).mobilise(
                                                            source.getLevel(), GlobalPos.of(source.getLevel().dimension(),
                                                                    BlockPosArgument.getBlockPos(context, "desk")),
                                                            IntegerArgumentType.getInteger(context, "spursPerDay"));
                                                    source.sendSuccess(() -> Component.literal(
                                                            "Mobilising: the treasury mints, the stockpile is ordered"), true);
                                                    return 1;
                                                }))))
                        .then(Commands.literal("declare")
                                .then(Commands.argument("theatre", com.mojang.brigadier.arguments.StringArgumentType.word())
                                        .executes(context -> {
                                            var source = context.getSource();
                                            var theatre = dev.b28.enterprise.army.Army.Theatre.valueOf(
                                                    com.mojang.brigadier.arguments.StringArgumentType.getString(context, "theatre")
                                                            .toUpperCase(java.util.Locale.ROOT));
                                            int levies = dev.b28.enterprise.market.Sovereign.get(source.getServer())
                                                    .declare(source.getLevel(), theatre, source.getLevel().getGameTime());
                                            source.sendSuccess(() -> Component.literal(
                                                    "War against %s: %d towns levied".formatted(theatre, levies)), true);
                                            return levies;
                                        })))
                        .then(Commands.literal("peace").executes(context -> {
                            int home = dev.b28.enterprise.market.Sovereign.get(context.getSource().getServer())
                                    .peace(context.getSource().getLevel());
                            context.getSource().sendSuccess(() -> Component.literal(
                                    "Peace: %d soldiers walk home".formatted(home)), true);
                            return home;
                        }))
                        .then(Commands.literal("tally").executes(EnterpriseCommands::warTally)))
                .then(Commands.literal("market")
                        .then(Commands.argument("desk", BlockPosArgument.blockPos())
                                .executes(EnterpriseCommands::market)))
                .then(Commands.literal("metals").executes(EnterpriseCommands::metals))
                .then(Commands.literal("name")
                        .executes(context -> {
                            var server = context.getSource().getServer();
                            StringBuilder out = new StringBuilder("Markets:");
                            for (var desk : dev.b28.enterprise.caravan.Atlas.markets(server)) {
                                out.append("\n  ").append(dev.b28.enterprise.settlement.Names.of(server, desk))
                                        .append(" at ").append(desk.pos().toShortString());
                            }
                            context.getSource().sendSuccess(() -> Component.literal(out.toString()), false);
                            return 1;
                        })
                        .then(Commands.argument("desk", BlockPosArgument.blockPos())
                                .then(Commands.argument("name", com.mojang.brigadier.arguments.StringArgumentType.greedyString())
                                        .executes(context -> {
                                            var server = context.getSource().getServer();
                                            var desk = net.minecraft.core.GlobalPos.of(context.getSource().getLevel().dimension(),
                                                    BlockPosArgument.getLoadedBlockPos(context, "desk"));
                                            String name = com.mojang.brigadier.arguments.StringArgumentType.getString(context, "name");
                                            dev.b28.enterprise.settlement.Names.get(server).rename(desk, name);
                                            context.getSource().sendSuccess(() -> Component.literal("The market at "
                                                    + desk.pos().toShortString() + " is " + name + " now"), true);
                                            return 1;
                                        }))))
                .then(Commands.literal("cashbook").then(Commands.argument("desk", BlockPosArgument.blockPos())
                        .executes(context -> cashbook(context, 10))
                        .then(Commands.argument("days", IntegerArgumentType.integer(1, 60))
                                .executes(context -> cashbook(context, IntegerArgumentType.getInteger(context, "days"))))))
                .then(Commands.literal("post").executes(context -> {
                    var server = context.getSource().getServer();
                    String out = String.join("\n", dev.b28.enterprise.market.Post.get(server).report(server));
                    context.getSource().sendSuccess(() -> Component.literal(out), false);
                    return 1;
                }))
                .then(Commands.literal("paperwork").executes(context -> {
                    var server = context.getSource().getServer();
                    StringBuilder out = new StringBuilder("Paperwork (documents, paper, ink, quills used; scribes paid):");
                    dev.b28.enterprise.cargo.Paperwork.get(server).books().forEach((desk, b) -> out.append(
                            "\n  %s: %.0f documents, %.0f paper, %.0f ink, %.0f quills, %.1f emeralds to scribes".formatted(
                                    dev.b28.enterprise.settlement.Names.label(desk), b[0], b[1], b[2], b[3], b[4])
                            + " " + dev.b28.enterprise.cargo.Paperwork.get(server).byKind(desk)));
                    context.getSource().sendSuccess(() -> Component.literal(out.toString()), false);
                    return 1;
                }))
                .then(Commands.literal("freight").executes(context -> {
                    var server = context.getSource().getServer();
                    var freight = dev.b28.enterprise.market.Freight.get(server);
                    StringBuilder out = new StringBuilder("Freight hubs:");
                    freight.books().forEach((hub, b) -> out.append("\n  %s: %.0f stacks forwarded for %.0f spurs, %.0f spurs of freight bought, %.0f stacks carried on its runs, %.0f bills bought (%.1f emeralds face), %.0f carts of drayage (%.0f spurs of wages), %.0f spurs of landing fees"
                            .formatted(dev.b28.enterprise.settlement.Names.label(hub), b[0], b[1], b[5], b[4], b[2], b[3], b[7], b[6], b[8])));
                    for (var depot : dev.b28.enterprise.merchant.Depots.get(server).all()) {
                        if (depot.kind() != dev.b28.enterprise.merchant.Depots.Kind.STOREHOUSE) {
                            continue;
                        }
                        for (var leg : dev.b28.enterprise.market.Freight.legsFrom(server, depot.desk())) {
                            freight.shop(leg).ifPresent(pos -> out.append("\n  shop at %s: %s -> %s, %.3f emeralds a stack"
                                    .formatted(pos.toShortString(), dev.b28.enterprise.settlement.Names.label(leg.from()),
                                            dev.b28.enterprise.settlement.Names.label(leg.to()),
                                            dev.b28.enterprise.market.Freight.stackPrice(leg.route()))
                                    + (leg.passing() ? " (a run calling on its way)" : "")));
                        }
                    }
                    context.getSource().sendSuccess(() -> Component.literal(out.toString()), false);
                    return 1;
                }))
                .then(Commands.literal("depot")
                        .executes(EnterpriseCommands::depots)
                        .then(Commands.literal("add")
                                .then(Commands.argument("desk", BlockPosArgument.blockPos())
                                        .then(Commands.argument("kind", com.mojang.brigadier.arguments.StringArgumentType.word())
                                                .then(Commands.argument("from", BlockPosArgument.blockPos())
                                                        .then(Commands.argument("to", BlockPosArgument.blockPos())
                                                                .executes(EnterpriseCommands::addDepot))))))
                        .then(Commands.literal("remove")
                                .then(Commands.argument("id", IntegerArgumentType.integer(1)).executes(context -> {
                                    boolean removed = dev.b28.enterprise.merchant.Depots.get(context.getSource().getServer())
                                            .remove(IntegerArgumentType.getInteger(context, "id"));
                                    context.getSource().sendSuccess(() -> Component.literal(removed ? "Removed" : "No such building"),
                                            true);
                                    return removed ? 1 : 0;
                                }))))
                .then(Commands.literal("trust").executes(EnterpriseCommands::trust))
                .then(Commands.literal("bank")
                        .executes(EnterpriseCommands::bank)
                        .then(Commands.literal("open")
                                .then(Commands.argument("desk", BlockPosArgument.blockPos()).executes(context -> {
                                    var source = context.getSource();
                                    GlobalPos desk = GlobalPos.of(source.getLevel().dimension(),
                                            BlockPosArgument.getBlockPos(context, "desk"));
                                    dev.b28.enterprise.market.Bank.get(source.getServer()).open(desk);
                                    source.sendSuccess(() -> Component.literal("A bank at " + dev.b28.enterprise.settlement.Names.label(desk)
                                            + ": its caravans buy bills and lend against contracts"), true);
                                    return 1;
                                }))))
                .then(Commands.literal("tenders").executes(context -> {
                    var source = context.getSource();
                    var all = dev.b28.enterprise.market.Tenders.get(source.getServer()).all();
                    long now = source.getLevel().getGameTime();
                    source.sendSuccess(() -> Component.literal("%d tender(s)".formatted(all.size())), false);
                    all.forEach(t -> source.sendSuccess(() -> Component.literal("  on %s's board: %d wanted, %s, by day %d"
                            .formatted(dev.b28.enterprise.settlement.Names.label(t.board()), t.open(), t.work().describe(), t.due() / 24000)),
                            false));
                    return all.size();
                }))
                .then(Commands.literal("services")
                        .executes(context -> services(context, null))
                        .then(Commands.argument("desk", BlockPosArgument.blockPos())
                                .executes(context -> services(context, BlockPosArgument.getBlockPos(context, "desk")))))
                .then(Commands.literal("atlas")
                        .then(Commands.argument("desk", BlockPosArgument.blockPos())
                                .executes(EnterpriseCommands::atlas)))
                .then(Commands.literal("territory")
                        .then(Commands.argument("bell", BlockPosArgument.blockPos())
                                .executes(EnterpriseCommands::territory)))
                .then(Commands.literal("toll")
                        .then(Commands.argument("bell", BlockPosArgument.blockPos())
                                .then(Commands.argument("percent", com.mojang.brigadier.arguments.DoubleArgumentType.doubleArg(0, 25))
                                        .executes(context -> {
                                            GlobalPos bell = GlobalPos.of(context.getSource().getLevel().dimension(),
                                                    BlockPosArgument.getBlockPos(context, "bell"));
                                            double percent = com.mojang.brigadier.arguments.DoubleArgumentType.getDouble(context, "percent");
                                            dev.b28.enterprise.territory.Territory.get(context.getSource().getServer())
                                                    .setTollRate(bell, percent / 100);
                                            context.getSource().sendSuccess(() -> Component.literal(
                                                    "Toll set to %.0f%%".formatted(percent)), true);
                                            return 1;
                                        }))))
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
                .map(bell -> "held by " + dev.b28.enterprise.settlement.Names.label(bell))
                .orElse("unclaimed");
        var terrain = dev.b28.enterprise.territory.Terrain.get(source.getServer());
        double water = terrain.waterShare(source.getLevel(), hex);
        boolean bridged = dev.b28.enterprise.territory.Works.get(source.getServer()).bridged(source.getLevel().dimension(), hex);
        double factor = bridged ? 1 : terrain.crossingFactor(source.getLevel(), hex);
        source.sendSuccess(() -> Component.literal("Hex %s, centre %.0f %.0f, %s; %.0f%% water, %s".formatted(
                hex, hex.centerX(), hex.centerZ(), owner, water * 100,
                Double.isInfinite(factor) ? "no way across for caravans"
                        : (bridged ? "bridged, " : "") + "%.1fx the road food to cross".formatted(factor))),
                false);
        return 1;
    }

    /** A settlement's hexes, culture, farm plan and tolls owed. */
    private static int territory(CommandContext<CommandSourceStack> context) throws CommandSyntaxException {
        CommandSourceStack source = context.getSource();
        GlobalPos bell = GlobalPos.of(source.getLevel().dimension(), BlockPosArgument.getBlockPos(context, "bell"));
        var territory = dev.b28.enterprise.territory.Territory.get(source.getServer());
        var hexes = territory.hexes(bell);
        source.sendSuccess(() -> Component.literal("Settlement at %s holds %d hex(es), culture %.1f of %.1f for the next"
                .formatted(dev.b28.enterprise.settlement.Names.label(bell), hexes.size(), territory.culture(bell), territory.nextHexCost(bell))), false);
        source.sendSuccess(() -> Component.literal("  hexes " + hexes), false);
        territory.farmPlan(bell).forEach((item, perDay) -> source.sendSuccess(
                () -> Component.literal("  farms %s: %.1f a day".formatted(item, perDay)), false));
        territory.idlePlots(bell).forEach((item, perDay) -> source.sendSuccess(
                () -> Component.literal("  idle %s: %.1f a day untended".formatted(item, perDay)), false));
        territory.desk(bell).ifPresent(desk -> dev.b28.enterprise.caravan.ForwardSales.sold(source.getServer(), desk)
                .forEach((item, sold) -> source.sendSuccess(() -> Component.literal(
                        "  sold forward %s: %.1f a day at %.3f".formatted(item, sold[0], sold[1])), false)));
        if (territory.wagePerDay(bell) > 0) {
            source.sendSuccess(() -> Component.literal(("  farm wages %.2f emeralds a day (%.0f%% of the harvest's value; crops "
                    + "sell %.2f a day); %.1f paid, %.1f unpaid (harvest lost); %d items harvested").formatted(
                    territory.wagePerDay(bell), territory.laborShare(bell) * 100, territory.cropSalesPerDay(bell),
                    territory.wagesPaid(bell), territory.wagesUnpaid(bell), territory.harvested(bell))), false);
        }
        if (territory.deposits(bell) > 0.01) {
            source.sendSuccess(() -> Component.literal("  deposits: %.1f emeralds borrowed of its households' savings"
                    .formatted(territory.deposits(bell))), false);
        }
        source.sendSuccess(() -> Component.literal("  focus: " + territory.focus(bell).label()), false);
        double[] homes = territory.households(bell);
        source.sendSuccess(() -> Component.literal(("  households: purse %.1f emeralds, owed %.1f in credit, %d freehold "
                + "plots, %.1f spent at the market; hands %s (wage x%.2f)").formatted(homes[0], homes[1], (int) homes[2],
                homes[3], homes[4] > 1.01 ? "scarce" : homes[4] < 0.99 ? "idle" : "matched", homes[4])), false);
        source.sendSuccess(() -> Component.literal("  treasury %.1f emeralds at the market's last touch, %.1f kept back"
                .formatted(territory.treasury(bell), territory.desk(bell).map(territory::reserve).orElse(0.0))), false);
        double mood = territory.mood(bell);
        String met = territory.comfortsMet(bell).entrySet().stream().sorted(Map.Entry.comparingByKey())
                .map(e -> "%s %.0f%%".formatted(e.getKey(), e.getValue() * 100))
                .collect(java.util.stream.Collectors.joining(", "));
        source.sendSuccess(() -> Component.literal(("  mood %+.2f (%s): %d villagers, %d beds; comforts met: %s; "
                + "farms x%.2f, culture x%.2f").formatted(mood, mood >= 0.5 ? "content" : mood > -0.5 ? "so-so" : "unhappy",
                territory.villagers(bell), territory.beds(bell), met.isEmpty() ? "none yet" : met,
                dev.b28.enterprise.territory.Territory.yieldFactor(mood),
                dev.b28.enterprise.territory.Territory.cultureFactor(mood))), false);
        dev.b28.enterprise.territory.Works.get(source.getServer()).bridgesOf(bell).forEach(b -> source.sendSuccess(
                () -> Component.literal("  bridge on %s: %s, %.0f emeralds of masons' wages paid".formatted(b.hex(),
                        b.placed() ? "open, deck laid" : b.done() ? "open, deck to be laid" : "%.0f%% built".formatted(
                                b.progress() * 100), b.wagesPaid())), false));
        dev.b28.enterprise.territory.Works.get(source.getServer()).wall(bell).ifPresent(w -> source.sendSuccess(
                () -> Component.literal("  wall %.0f%% built, %d columns raised, %.0f emeralds of masons' wages paid"
                        .formatted(w.progress() * 100, w.columnsPlaced(), w.wagesPaid())), false));
        var hands = dev.b28.enterprise.territory.Labour.hands(
                dev.b28.enterprise.settlement.Settlements.get(source.getServer()).roster(bell));
        if (!hands.isEmpty()) {
            source.sendSuccess(() -> Component.literal("  hands without a trade: %d (%d standing watch), gathering on %s"
                    .formatted(hands.size(), territory.militia(bell), territory.hexes(bell).stream()
                            .limit(Math.max(0, hands.size() - territory.militia(bell)))
                            .map(hex -> dev.b28.enterprise.territory.Labour.biome(source.getLevel(), hex).unwrapKey()
                                    .map(k -> k.location().getPath()).orElse("?"))
                            .collect(java.util.stream.Collectors.joining(", ")))), false);
        }
        var works = dev.b28.enterprise.territory.Works.get(source.getServer()).buildingsOf(bell);
        if (!works.isEmpty() || territory.hunting(bell) > 0 || territory.armed(bell) > 0) {
            source.sendSuccess(() -> Component.literal("  buildings: %s; hunters with bows %.0f%%, guards armed %.0f%%%s"
                    .formatted(works.isEmpty() ? "none" : works.values().stream()
                                    .map(b -> b.kind().id() + (b.done() ? "" : " (%.0f%% built)".formatted(b.progress() * 100)))
                                    .collect(java.util.stream.Collectors.joining(", ")),
                            territory.hunting(bell) * 100, territory.armed(bell) * 100,
                            territory.flowerBeds(bell) ? "; flower beds on the square" : "")), false);
        }
        var kit = territory.kitShare(bell);
        if (territory.mine(bell).isPresent() || kit.values().stream().anyMatch(v -> v > 0)) {
            source.sendSuccess(() -> Component.literal("  mine %s; kit: %s%s".formatted(
                    territory.mine(bell).map(BlockPos::toShortString).orElse("not dug"),
                    dev.b28.enterprise.territory.Smithing.KITS.stream().map(k -> "%s for %.0f%%".formatted(
                            k.id().replace("minecraft:iron_", ""), kit.getOrDefault(k.id(), 0.0) * 100))
                            .collect(java.util.stream.Collectors.joining(", ")),
                    territory.mendedUntil(bell) > source.getLevel().getGameTime() ? "; tools mended" : "")), false);
        }
        var herd = territory.herd(bell);
        if (!herd.isEmpty()) {
            source.sendSuccess(() -> Component.literal("  herd: %s; pasture %s".formatted(
                    herd.entrySet().stream().map(e -> "%s %.1f".formatted(e.getKey().replace("minecraft:", ""), e.getValue()))
                            .collect(java.util.stream.Collectors.joining(", ")),
                    territory.pasture(bell).map(BlockPos::toShortString).orElse("not fenced yet"))), false);
        }
        source.sendSuccess(() -> Component.literal("  %d home(s) built, %d child(ren) born".formatted(
                territory.homesBuilt(bell), territory.born(bell))), false);
        source.sendSuccess(() -> Component.literal(("  toll %.1f%% at its posts and a handling fee of %.0f spurs a caravan "
                + "(%.0f spurs collected last review), %d guards").formatted(
                territory.tollRate(bell) * 100, territory.passageFee(bell), Math.max(0, territory.tollIncomeYesterday(bell)),
                territory.guards(bell))), false);
        source.sendSuccess(() -> Component.literal(("  %d guard(s) posted at %s; traffic %.0f spurs a day; %d deserted")
                .formatted(territory.guards(bell), territory.posts(bell), territory.trafficPerDay(bell),
                        territory.deserted(bell))), false);
        territory.crossings(bell).forEach((route, crossing) -> source.sendSuccess(() -> Component.literal(
                "  route %d: %.1f trips a day carrying %.0f spurs; going around posts at %s".formatted(route,
                        crossing.tripsPerDay(), crossing.value(), describeOptions(crossing.options()))), false));
        if (territory.tollsOwed(bell) > 0) {
            source.sendSuccess(() -> Component.literal("  tolls owed: %d spurs".formatted(territory.tollsOwed(bell))), false);
        }
        return hexes.size();
    }

    private static String describeOptions(List<dev.b28.enterprise.territory.Chokepoints.Option> options) {
        return options.stream()
                .map(o -> o.posts().get(o.posts().size() - 1) + (o.posts().size() > 1 ? " too" : "") + ": "
                        + (Double.isInfinite(o.detour()) ? "no way" : "%.0f spurs".formatted(o.detour())))
                .collect(java.util.stream.Collectors.joining("; "));
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

    /** Every contract (or a desk's), with its terms, state and record. */
    /** Every dial: its value now, its default, and what it does. */
    private static int dials(CommandSourceStack source) {
        for (Dials.Dial dial : Dials.all()) {
            source.sendSuccess(() -> Component.literal("%s = %s%s: %s".formatted(dial.name, trim(dial.get()),
                    dial.get() == dial.initial ? "" : " (default " + trim(dial.initial) + ")", dial.does)), false);
        }
        return Dials.all().size();
    }

    private static String trim(double value) {
        return value == Math.rint(value) ? Long.toString((long) value) : "%.3f".formatted(value);
    }

    /** Sets a dial for this world, or back to its default ({@code value} null). */
    private static int setDial(CommandSourceStack source, String name, Double value) {
        var dial = Dials.named(name);
        if (dial.isEmpty()) {
            source.sendFailure(Component.literal("No dial called " + name + "; /enterprise dial lists them"));
            return 0;
        }
        Dials.get(source.getServer()).set(dial.get(), value);
        source.sendSuccess(() -> Component.literal("%s = %s".formatted(name, trim(dial.get().get()))), true);
        return 1;
    }

    private static int contracts(CommandContext<CommandSourceStack> context, BlockPos desk) {
        CommandSourceStack source = context.getSource();
        var book = dev.b28.enterprise.contract.ContractBook.get(source.getServer());
        var list = desk == null ? book.all() : book.of(GlobalPos.of(source.getLevel().dimension(), desk));
        source.sendSuccess(() -> Component.literal("%d contract(s)".formatted(list.size())), false);
        for (var contract : list) {
            String buyer = (source.getLevel().getBlockEntity(contract.buyer().pos())
                    instanceof dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity ? "the shop at " : "")
                    + dev.b28.enterprise.settlement.Names.label(contract.buyer());
            source.sendSuccess(() -> Component.literal(("  #%d %s -> %s, %s, %s (copies at %d of 2): %s; %d delivered, "
                    + "%d on the road, %.1f emeralds paid (%.1f up front), %d missed by the seller, %d by the buyer; trust %.2f").formatted(
                    contract.id(), dev.b28.enterprise.settlement.Names.label(contract.seller()), buyer, contract.state().name().toLowerCase(),
                    contract.incoterm().name(),
                    (contract.knownBy(contract.seller()) ? 1 : 0) + (contract.knownBy(contract.buyer()) ? 1 : 0),
                    contract.delivery().map(d -> d.quantity() + " " + d.describe())
                            .orElse(contract.levy().map(l -> "a levy of " + l.soldiers() + " soldiers").orElse("?")),
                    contract.delivered(), contract.shipped(), contract.paid(), contract.advanced(), contract.missed(),
                    contract.buyerMissed(),
                    book.trust(contract.seller(), contract.buyer()))), false);
            contract.history().forEach(line -> source.sendSuccess(() -> Component.literal("    " + line), false));
        }
        var rents = dev.b28.enterprise.contract.Rents.get(source.getServer()).all().stream()
                .filter(r -> desk == null || r.farm().pos().equals(desk) || r.holder().pos().equals(desk)).toList();
        source.sendSuccess(() -> Component.literal("%d rent(s)".formatted(rents.size())), false);
        for (var rent : rents) {
            source.sendSuccess(() -> Component.literal(("  rent %d: %s owes %s %d %s a season, bought for %.1f "
                    + "emeralds; %s, %d coupons, %d delivered, %d in arrears").formatted(rent.id(),
                    dev.b28.enterprise.settlement.Names.label(rent.farm()),
                    dev.b28.enterprise.settlement.Names.label(rent.holder()), rent.quantity(),
                    net.minecraft.core.registries.BuiltInRegistries.ITEM.getKey(rent.crop()).getPath(), rent.capital(),
                    rent.state().name().toLowerCase(), rent.coupons(), rent.delivered(), rent.arrears())), false);
            rent.history().forEach(line -> source.sendSuccess(() -> Component.literal("    " + line), false));
        }
        return list.size();
    }

    /**
     * What each trade does, at home, abroad and on the road; with a desk, only the trades its town has, and what
     * services it has done or had done, with the latest.
     */
    private static int services(CommandContext<CommandSourceStack> context, BlockPos deskPos) {
        CommandSourceStack source = context.getSource();
        var all = dev.b28.enterprise.service.Services.all();
        if (deskPos == null) {
            all.stream().map(dev.b28.enterprise.service.Service::trade).distinct().forEach(trade -> {
                source.sendSuccess(() -> Component.literal(trade.replaceFirst(".*:", "")), false);
                dev.b28.enterprise.service.Services.of(trade).forEach(s -> source.sendSuccess(() -> Component.literal(
                        "  %s: %s".formatted(s.where().name().toLowerCase(), s.does())), false));
            });
            return all.size();
        }
        var server = source.getServer();
        GlobalPos desk = GlobalPos.of(source.getLevel().dimension(), deskPos);
        var territory = dev.b28.enterprise.territory.Territory.get(server);
        var trades = territory.bellOf(desk)
                .map(bell -> dev.b28.enterprise.service.Services.residents(source.getLevel(), bell).keySet())
                .orElse(java.util.Set.of());
        source.sendSuccess(() -> Component.literal("Services at %s%s".formatted(deskPos.toShortString(),
                territory.bellOf(desk).map(bell -> territory.blessedUntil(bell) > source.getLevel().getGameTime()
                        ? ", blessed for %.1f more days".formatted(
                                (territory.blessedUntil(bell) - source.getLevel().getGameTime()) / 24000.0) : "")
                        .orElse(""))), false);
        all.stream().filter(s -> trades.contains(s.trade())).forEach(s -> source.sendSuccess(() -> Component.literal(
                "  its %s %s: %s".formatted(s.trade().replaceFirst(".*:", ""), s.where().name().toLowerCase(), s.does())),
                false));
        var ledger = dev.b28.enterprise.service.ServiceLedger.get(server);
        ledger.tallies(desk).forEach((key, tally) -> source.sendSuccess(() -> Component.literal(
                "  %s: %s for %s, %d times, %.2f emeralds paid".formatted(key.service().replaceFirst(".*:", ""),
                        key.from().equals(desk) ? "this town" : dev.b28.enterprise.settlement.Names.label(key.from()),
                        key.at().equals(desk) ? "this town" : dev.b28.enterprise.settlement.Names.label(key.at()), tally.times(),
                        tally.emeralds())), false));
        ledger.recent(desk).stream().limit(8).forEach(e -> source.sendSuccess(() -> Component.literal(
                "  day %d: %s, %s -> %s%s".formatted(e.time() / 24000, e.key().service().replaceFirst(".*:", ""),
                        dev.b28.enterprise.settlement.Names.label(e.key().from()), dev.b28.enterprise.settlement.Names.label(e.key().at()),
                        e.emeralds() > 0 ? ", %.2f emeralds".formatted(e.emeralds()) : "")), false));
        return ledger.tallies(desk).size();
    }

    /**
     * The war effort in a line: the sovereign's treasury and minting, what its shops have bought, the contracts that
     * source them and who supplies them, bills of exchange outstanding, and the spur's rate.
     */
    private static int war(CommandContext<CommandSourceStack> context) {
        CommandSourceStack source = context.getSource();
        var server = source.getServer();
        var sovereign = dev.b28.enterprise.market.Sovereign.get(server);
        var army = dev.b28.enterprise.army.Army.get(server);
        long now = source.getLevel().getGameTime();
        var mint = dev.b28.enterprise.money.Mint.get(server);
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        var desk = sovereign.desk();
        int contracts = 0;
        int delivered = 0;
        for (var c : book.all()) {
            if (desk.isPresent() && c.buyer().equals(desk.get()) && c.delivery().isPresent()) {
                contracts++;
                delivered += c.delivered();
            }
        }
        int fallen = army.records().values().stream().mapToInt(dev.b28.enterprise.army.Army.Record::fallen).sum();
        int raised = army.records().values().stream().mapToInt(dev.b28.enterprise.army.Army.Record::raised).sum();
        double bills = dev.b28.enterprise.contract.Bills.get(server).all().stream()
                .filter(b -> b.state() != dev.b28.enterprise.contract.Bills.State.PAID)
                .mapToDouble(dev.b28.enterprise.contract.Bills.Bill::emeralds).sum();
        String depot = desk.filter(d -> source.getLevel().isLoaded(d.pos())).map(d -> {
            var stock = dev.b28.enterprise.merchant.MerchantStock.at(source.getLevel(), d.pos());
            return dev.b28.enterprise.market.Sovereign.STOCKPILE.keySet().stream()
                    .map(i -> BuiltInRegistries.ITEM.getKey(i).getPath() + "=" + stock.count(
                            new net.minecraft.world.item.trading.ItemCost(i, 1)))
                    .collect(java.util.stream.Collectors.joining(" ")) + " meat=" + dev.b28.enterprise.army.Army.MEATS
                    .stream().mapToInt(i -> stock.count(new net.minecraft.world.item.trading.ItemCost(i, 1))
                            * dev.b28.enterprise.army.Army.nutrition(i)).sum() + "pts";
        }).orElse("?");
        String line = ("war: %s %.0f days | treasury %d spurs, minted %d | contracts %d, delivered %d | depot %s | "
                + "army %d serving, %d raised, %d fallen | gold %s | unmet %s | bills %.1f | rate %.2f, debasement %.3f")
                .formatted(sovereign.phase().name().toLowerCase(), sovereign.daysInPhase(now),
                        sovereign.treasury().map(a -> a.getBalance()).orElse(0),
                        mint.mintedInAll(dev.b28.enterprise.money.Mint.DEFAULT_FACTION), contracts, delivered, depot,
                        army.size(), raised, fallen, sovereign.received(),
                        desk.map(d -> sovereign.unmet(d).keySet().stream().map(i -> BuiltInRegistries.ITEM.getKey(i).getPath())
                                .toList().toString()).orElse("[]"), bills,
                        mint.rate(dev.b28.enterprise.money.Mint.DEFAULT_FACTION, now),
                        mint.debasement(dev.b28.enterprise.money.Mint.DEFAULT_FACTION, now));
        StringBuilder trades = new StringBuilder(" | trades by town:");
        var settlements = dev.b28.enterprise.settlement.Settlements.get(server);
        for (var bell : settlements.all().keySet()) {
            var count = new java.util.TreeMap<String, String>();
            settlements.roster(bell).values().forEach(r -> {
                if (java.util.Set.of("minecraft:weaponsmith", "minecraft:armorer", "minecraft:fletcher",
                        "minecraft:toolsmith", "minecraft:leatherworker", "minecraft:butcher").contains(r.profession())) {
                    count.merge(r.profession().replace("minecraft:", ""), "L" + r.level(), (a, b) -> a + b);
                }
            });
            trades.append(" ").append(bell.pos().getX()).append(",").append(bell.pos().getZ()).append(count);
        }
        var metals = dev.b28.enterprise.market.MetalExchange.get(server);
        String held = metals.desk().isEmpty() ? "" : " | metals " + dev.b28.enterprise.market.MetalExchange.METALS.stream()
                .map(m -> BuiltInRegistries.ITEM.getKey(m.item()).getPath() + "=" + metals.held(m.item()))
                .collect(java.util.stream.Collectors.joining(" "));
        var requisitions = dev.b28.enterprise.market.Requisitions.get(server);
        String reqs = " | requisitions %d open (%s), %d awarded, %d bought at its shops".formatted(
                requisitions.all().size(), requisitions.all().stream()
                        .map(r -> BuiltInRegistries.ITEM.getKey(r.goods().get(0)).getPath() + ":" + r.open() + "/"
                                + r.wanted() + (r.awarded() ? "a" : "") + "b" + r.bids().size())
                        .collect(java.util.stream.Collectors.joining(" ")),
                requisitions.contracts(), requisitions.boughtAtShops());
        String full = line + held + reqs + trades;
        source.sendSuccess(() -> Component.literal(full), false);
        return army.size();
    }

    /** Registers a building (an armoury, barracks or storehouse) as one of a market's stores. */
    private static int addDepot(CommandContext<CommandSourceStack> context) throws CommandSyntaxException {
        var source = context.getSource();
        var kind = dev.b28.enterprise.merchant.Depots.Kind.valueOf(
                com.mojang.brigadier.arguments.StringArgumentType.getString(context, "kind").toUpperCase(java.util.Locale.ROOT));
        GlobalPos desk = GlobalPos.of(source.getLevel().dimension(), BlockPosArgument.getBlockPos(context, "desk"));
        var depot = dev.b28.enterprise.merchant.Depots.get(source.getServer()).add(source.getLevel(), desk, kind,
                BlockPosArgument.getBlockPos(context, "from"), BlockPosArgument.getBlockPos(context, "to"));
        int chests = depot.containers(source.getLevel()).size();
        source.sendSuccess(() -> Component.literal("Building %d: the %s of the market at %s, %d container(s), %d bed(s)"
                .formatted(depot.id(), kind.label(), dev.b28.enterprise.settlement.Names.label(desk), chests, depot.beds(source.getLevel()))), true);
        return depot.id();
    }

    /** Every market's buildings, what each holds and how full it is. */
    private static int depots(CommandContext<CommandSourceStack> context) {
        var source = context.getSource();
        var all = dev.b28.enterprise.merchant.Depots.get(source.getServer()).all();
        source.sendSuccess(() -> Component.literal("%d building(s)".formatted(all.size())), false);
        for (var depot : all) {
            var level = source.getServer().getLevel(depot.desk().dimension());
            String what = level == null || !depot.loaded(level) ? "not loaded" : "%d container(s), %d bed(s), %s".formatted(
                    depot.containers(level).size(), depot.beds(level), contents(level, depot));
            source.sendSuccess(() -> Component.literal("  #%d %s of %s, %s to %s: %s".formatted(depot.id(),
                    depot.kind().label(), dev.b28.enterprise.settlement.Names.label(depot.desk()), depot.min().toShortString(),
                    depot.max().toShortString(), what)), false);
        }
        return all.size();
    }

    private static String contents(ServerLevel level, dev.b28.enterprise.merchant.Depots.Depot depot) {
        java.util.Map<String, Integer> totals = new java.util.TreeMap<>();
        for (BlockPos pos : depot.containers(level)) {
            var handler = level.getCapability(net.neoforged.neoforge.capabilities.Capabilities.ItemHandler.BLOCK, pos, null);
            if (handler == null) {
                continue;
            }
            for (int i = 0; i < handler.getSlots(); i++) {
                var stack = handler.getStackInSlot(i);
                if (!stack.isEmpty()) {
                    totals.merge(BuiltInRegistries.ITEM.getKey(stack.getItem()).getPath(), stack.getCount(), Integer::sum);
                }
            }
        }
        return totals.isEmpty() ? "empty" : totals.toString();
    }

    /** The capital's metal exchange: each metal's stock, its prices, and what it has imported, bought and sold. */
    private static int metals(CommandContext<CommandSourceStack> context) {
        CommandSourceStack source = context.getSource();
        var server = source.getServer();
        var exchange = dev.b28.enterprise.market.MetalExchange.get(server);
        if (exchange.desk().isEmpty()) {
            source.sendFailure(Component.literal("No metal exchange: name a capital with /enterprise war capital"));
            return 0;
        }
        GlobalPos desk = exchange.desk().get();
        var phase = dev.b28.enterprise.market.Sovereign.get(server).phase();
        source.sendSuccess(() -> Component.literal("Metal exchange at %s (%s)".formatted(dev.b28.enterprise.settlement.Names.label(desk),
                phase.name().toLowerCase())), false);
        for (var metal : dev.b28.enterprise.market.MetalExchange.METALS) {
            source.sendSuccess(() -> Component.literal(("  %s: holds %d of %d, %d coming under contract; pays %.2f times "
                    + "its worth; %d bought from players at the port").formatted(
                    BuiltInRegistries.ITEM.getKey(metal.item()).getPath(), exchange.held(metal.item()),
                    dev.b28.enterprise.market.MetalExchange.target(metal, phase), exchange.coming(server, metal.item()),
                    exchange.premium(metal, phase), exchange.fromPlayers(metal.item()))), false);
        }
        return 1;
    }

    /** Each bank: bills it bought, loans it made and what's been repaid, and the loans still owed. */
    private static int bank(CommandContext<CommandSourceStack> context) {
        CommandSourceStack source = context.getSource();
        var bank = dev.b28.enterprise.market.Bank.get(source.getServer());
        for (GlobalPos at : bank.banks()) {
            var b = bank.book(at);
            source.sendSuccess(() -> Component.literal(("Bank at %s: %d bills bought for %.1f emeralds (worth %.1f when due); "
                    + "%d loans, %.1f lent, %.1f repaid").formatted(dev.b28.enterprise.settlement.Names.label(at), b.billsBought(), b.billsPaid(),
                    b.billsFace(), b.loansMade(), b.lent(), b.repaid())), false);
        }
        bank.loans().forEach(l -> source.sendSuccess(() -> Component.literal("  %s owes %.1f on contract #%d (lent %.1f)"
                .formatted(dev.b28.enterprise.settlement.Names.label(l.borrower()), l.owed(), l.contract(), l.principal())), false));
        return bank.banks().size();
    }

    /** Trust between each pair of parties that have dealt: their record of installments kept and missed. */
    private static int trust(CommandContext<CommandSourceStack> context) {
        CommandSourceStack source = context.getSource();
        var book = dev.b28.enterprise.contract.ContractBook.get(source.getServer());
        var records = book.records();
        source.sendSuccess(() -> Component.literal("%d record(s)".formatted(records.size())), false);
        records.forEach((pair, r) -> source.sendSuccess(() -> Component.literal("  %s: trust %.2f, %.1f kept, %.1f missed"
                .formatted(r.parties(), r.trust(), r.kept(), r.missed())), false));
        return records.size();
    }

    /** What went into the war: by town, and in all. */
    private static int warTally(CommandContext<CommandSourceStack> context) {
        CommandSourceStack source = context.getSource();
        var server = source.getServer();
        var army = dev.b28.enterprise.army.Army.get(server);
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        var sovereign = dev.b28.enterprise.market.Sovereign.get(server);
        var desk = sovereign.desk().orElse(null);
        source.sendSuccess(() -> Component.literal("War tally, against %s".formatted(army.theatre())), false);
        java.util.Set<GlobalPos> towns = new java.util.LinkedHashSet<>(army.records().keySet());
        java.util.Map<GlobalPos, int[]> kit = new java.util.LinkedHashMap<>();
        java.util.Map<GlobalPos, Double> paid = new java.util.LinkedHashMap<>();
        java.util.Map<GlobalPos, Double> upFront = new java.util.LinkedHashMap<>();
        for (var c : book.all()) {
            if (desk != null && c.buyer().equals(desk) && c.delivery().isPresent()) {
                towns.add(c.seller());
                kit.computeIfAbsent(c.seller(), k -> new int[1])[0] += c.delivered();
                paid.merge(c.seller(), c.paid(), Double::sum);
                upFront.merge(c.seller(), c.advanced(), Double::sum);
            }
            if (desk != null && c.buyer().equals(desk) && c.levy().isPresent()) {
                paid.merge(c.seller(), c.paid() + c.owing(), Double::sum);
            }
        }
        for (GlobalPos town : towns) {
            var r = army.records().getOrDefault(town, null);
            source.sendSuccess(() -> Component.literal(("  %s: %d raised, %d fallen, %d deserted, %d soldier-days (%d hungry), "
                    + "strength %.0f; %d pieces of kit delivered; %.1f emeralds earned, %.1f of it paid up front; trust %.2f")
                    .formatted(dev.b28.enterprise.settlement.Names.label(town),
                    r == null ? 0 : r.raised(), r == null ? 0 : r.fallen(), r == null ? 0 : r.deserted(),
                    r == null ? 0 : r.days(), r == null ? 0 : r.hungry(),
                    r == null ? 0 : r.strength(), kit.getOrDefault(town, new int[1])[0], paid.getOrDefault(town, 0.0),
                    upFront.getOrDefault(town, 0.0), desk == null ? 0 : book.trust(town, desk))),
                    false);
        }
        java.util.Map<String, int[]> training = new java.util.TreeMap<>(); // town: done, open, broken
        for (var c : book.all()) {
            if (desk != null && c.buyer().equals(desk) && c.training().isPresent()) {
                int[] t = training.computeIfAbsent(dev.b28.enterprise.settlement.Names.label(c.seller()) + " "
                        + c.training().get().profession().replace("minecraft:", ""), k -> new int[3]);
                t[c.state() == dev.b28.enterprise.contract.Contract.State.COMPLETED ? 0
                        : c.state() == dev.b28.enterprise.contract.Contract.State.BROKEN ? 2 : 1]++;
            }
        }
        training.forEach((who, t) -> source.sendSuccess(() -> Component.literal("  training %s: %d trained, %d under way, %d broken"
                .formatted(who, t[0], t[1], t[2])), false));
        var warehouse = dev.b28.enterprise.merchant.Warehouse.get(server);
        var territory = dev.b28.enterprise.territory.Territory.get(server);
        for (GlobalPos town : towns) {
            double[] w = warehouse.waste(town);
            double tools = territory.bellOf(town).map(territory::handTools).orElse(0.0);
            boolean cold = dev.b28.enterprise.merchant.Depots.get(server).has(town, dev.b28.enterprise.merchant.Depots.Kind.COLD_STORE);
            source.sendSuccess(() -> Component.literal(("  storekeeping %s: %.0f thrown out (%.1f emeralds), %.0f tools given "
                    + "away; %.1f tools in its hands' use; %s").formatted(dev.b28.enterprise.settlement.Names.label(town), w[0], w[1], w[2], tools,
                    cold ? "cold storage set up" : "no cold storage")), false);
        }
        if (desk != null && dev.b28.enterprise.market.Bank.get(server).is(desk)) {
            var b = dev.b28.enterprise.market.Bank.get(server).book(desk);
            source.sendSuccess(() -> Component.literal(("  the crown's bank: %d bills bought for %.1f (worth %.1f when due); "
                    + "%d loans, %.1f lent, %.1f repaid").formatted(b.billsBought(), b.billsPaid(), b.billsFace(),
                    b.loansMade(), b.lent(), b.repaid())), false);
        }
        source.sendSuccess(() -> Component.literal("  kit issued: " + army.issued()), false);
        source.sendSuccess(() -> Component.literal("  enchantment levels issued: " + army.enchantLevels()), false);
        source.sendSuccess(() -> Component.literal("  food eaten: %d points, its meals %.0f%% hearty".formatted(army.eaten(),
                army.heartiness() * 100)), false);
        source.sendSuccess(() -> Component.literal("  gold bought: " + sovereign.received()), false);
        var mint = dev.b28.enterprise.money.Mint.get(server);
        source.sendSuccess(() -> Component.literal("  minted %d spurs; rate now %.2f spurs an emerald".formatted(
                mint.mintedInAll(dev.b28.enterprise.money.Mint.DEFAULT_FACTION),
                mint.rate(dev.b28.enterprise.money.Mint.DEFAULT_FACTION, source.getLevel().getGameTime()))), false);
        return towns.size();
    }

    /** A master Merchant's dealings with the players' shops round its desk. */
    private static int market(CommandContext<CommandSourceStack> context) {
        CommandSourceStack source = context.getSource();
        GlobalPos desk = GlobalPos.of(source.getLevel().dimension(), BlockPosArgument.getBlockPos(context, "desk"));
        var exchange = dev.b28.enterprise.market.Exchange.get(source.getServer());
        if (source.getLevel().isLoaded(desk.pos())) {
            int shops = dev.b28.enterprise.market.Shops.near(source.getLevel(), desk.pos()).size();
            boolean master = dev.b28.enterprise.market.Broker.broker(source.getLevel(), desk,
                    dev.b28.enterprise.territory.Territory.get(source.getServer()), source.getLevel().getGameTime());
            source.sendSuccess(() -> Component.literal(master ? "Its master Merchant looks over %d shop(s) now".formatted(shops)
                    : "No master Merchant works this desk: it doesn't deal with players' shops (%d nearby)".formatted(shops)),
                    false);
        }
        var tally = exchange.tally(desk);
        source.sendSuccess(() -> Component.literal(("Market at %s: %d lots sold to players' shops for %d spurs, %d bought "
                + "from them for %d, %d commissions, its own shops took %d").formatted(dev.b28.enterprise.settlement.Names.label(desk),
                tally.lotsSold(), tally.spursFromSales(), tally.lotsBought(), tally.spursOnPurchases(), tally.commissions(),
                tally.spursFromOwnShops())), false);
        exchange.orders(desk).values().forEach(o -> source.sendSuccess(() -> Component.literal(
                "  ordered: %d %s at %.1f spurs each (shop at %s): the region's caravans bring it".formatted(
                        o.outstanding(), BuiltInRegistries.ITEM.getKey(o.item()).getPath(), o.spursEach(),
                        o.shop().toShortString())), false));
        exchange.jobs(desk).forEach((shop, job) -> source.sendSuccess(() -> Component.literal(
                "  commission for the shop at %s: %s (%d steps)".formatted(shop.toShortString(), job.step(), job.steps())),
                false));
        exchange.signals(desk).forEach(s -> source.sendSuccess(() -> Component.literal(
                "  wanted, and no market it knows has it: %d %s, paying %d spurs each (for %s)".formatted(s.count(),
                        BuiltInRegistries.ITEM.getKey(s.item()).getPath(), s.spursEach(), s.why())), false));
        exchange.recent(desk).stream().limit(8).forEach(line -> source.sendSuccess(() -> Component.literal("  " + line), false));
        return exchange.orders(desk).size() + exchange.signals(desk).size();
    }

    /** The markets a desk knows, nearest first. */
    private static int atlas(CommandContext<CommandSourceStack> context) {
        CommandSourceStack source = context.getSource();
        GlobalPos desk = GlobalPos.of(source.getLevel().dimension(), BlockPosArgument.getBlockPos(context, "desk"));
        var known = dev.b28.enterprise.caravan.Atlas.get(source.getServer()).known(source.getServer(), desk);
        source.sendSuccess(() -> Component.literal("The desk at %s knows %d market(s)".formatted(
                dev.b28.enterprise.settlement.Names.label(desk), known.size())), false);
        known.forEach(other -> source.sendSuccess(() -> Component.literal("  %s, %d blocks".formatted(
                dev.b28.enterprise.settlement.Names.label(other), (int) Math.sqrt(other.pos().distSqr(desk.pos())))), false));
        return known.size();
    }

    /** Every route with its totals and where its caravan is. */
    private static int routes(CommandContext<CommandSourceStack> context) {
        CommandSourceStack source = context.getSource();
        CaravanLedger ledger = CaravanLedger.get(source.getServer());
        source.sendSuccess(() -> Component.literal("%d route(s)".formatted(ledger.routes().size())), false);
        for (Route route : ledger.routes()) {
            String caravan = ledger.caravan(route.id())
                    .map(c -> " | caravan %s, %d items, %d spurs, squad: %s%s".formatted(
                            c.leg().name().toLowerCase().replace('_', ' '), c.items(), c.spurs(),
                            c.crew().stream().map(m -> dev.b28.enterprise.caravan.Squad.trade(m).replaceFirst(".*:", ""))
                                    .collect(java.util.stream.Collectors.joining(", ")),
                            c.herd().isEmpty() ? "" : ", driving %d animals".formatted(c.herd().size())))
                    .map(text -> route.fishCaught() > 0 ? text + " | %d fish caught in all".formatted(route.fishCaught()) : text)
                    .map(text -> route.portersSent() > 0 ? text + " | %d porters walked home".formatted(route.portersSent()) : text)
                    .orElse("");
            source.sendSuccess(() -> Component.literal(("  #%d %s -> %s: %d blocks, %d trips, %d items sold for %d spurs, "
                    + "%d for services, %d bought for %d, %d spurs on road food, %d tolls paid, %d spurs brought home, %d food eaten, %s%s")
                    .formatted(route.id(), dev.b28.enterprise.settlement.Names.label(route.from()), dev.b28.enterprise.settlement.Names.label(route.to()),
                    route.distance(), route.trips(), route.itemsDelivered(), route.salesSpurs(), route.serviceSpurs(),
                    route.itemsBought(),
                    route.purchaseSpurs(), route.foodSpurs(), route.tollsPaid(), route.spursCollected(), route.foodEaten(),
                    route.status(), caravan)), false);
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

    /** A market's cash book: the last {@code days} days' money in and out, by what for. */
    private static int cashbook(com.mojang.brigadier.context.CommandContext<CommandSourceStack> context, int days)
            throws com.mojang.brigadier.exceptions.CommandSyntaxException {
        var server = context.getSource().getServer();
        BlockPos pos = BlockPosArgument.getBlockPos(context, "desk"); // the book's on the ledger: loaded or not
        GlobalPos desk = GlobalPos.of(context.getSource().getLevel().dimension(), pos);
        long today = context.getSource().getLevel().getGameTime() / 24000;
        StringBuilder out = new StringBuilder("Cash book of " + dev.b28.enterprise.settlement.Names.label(desk) + ":");
        dev.b28.enterprise.money.CashBook.get(server).report(desk, today - days + 1, today)
                .forEach(line -> out.append("\n").append(line));
        context.getSource().sendSuccess(() -> Component.literal(out.toString()), false);
        return 1;
    }
}
