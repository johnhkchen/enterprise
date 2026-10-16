package dev.b28.enterprise.territory;

import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.Warehouse;
import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;

/**
 * A town's trades turning raw goods into the goods they deal in, in the warehouse: Librarians and Cartographers press
 * sugar cane into paper while paper is short, Farmers bake wheat into bread while food is short, and the town's own
 * comforts: biscuits (a Farmer's: wheat sweetened with cane), books (a Librarian's: paper bound in leather), carpets
 * (a Shepherd's, of wool), boots (a Leatherworker's). A conversion runs only if a villager of its trade
 * lives in the town (by its roster). Comforts are made up to the town's own par, and as much again for each run its
 * caravans make to other towns: what it makes, it can sell.
 */
public final class Workshop {
    /**
     * {@code in} of {@code input} (and {@code in2} of {@code input2}, if any) make {@code out} of {@code output}, done
     * by any of {@code trades}.
     */
    public record Conversion(Set<String> trades, Item input, int in, Item input2, int in2, Item output, int out) {
        public Conversion(Set<String> trades, Item input, int in, Item output, int out) {
            this(trades, input, in, null, 0, output, out);
        }
    }

    public static final List<Conversion> CONVERSIONS = List.of(
            new Conversion(Set.of("minecraft:librarian", "minecraft:cartographer"), Items.SUGAR_CANE, 1, Items.PAPER, 1),
            new Conversion(Set.of("minecraft:farmer"), Items.WHEAT, 3, Items.BREAD, 1),
            new Conversion(Set.of("minecraft:shepherd"), Items.WHITE_WOOL, 2, Items.WHITE_CARPET, 3),
            new Conversion(Set.of("minecraft:leatherworker"), Items.LEATHER, 4, Items.LEATHER_BOOTS, 1),
            new Conversion(Set.of("minecraft:fisherman"), Items.COD, 1, Items.COOKED_COD, 1),
            new Conversion(Set.of("minecraft:farmer"), Items.WHEAT, 2, Items.SUGAR_CANE, 1, Items.COOKIE, 4),
            new Conversion(Set.of("minecraft:librarian"), Items.PAPER, 3, Items.LEATHER, 1, Items.BOOK, 1));

    private Workshop() {}

    /** What the town's conversions could make from these inputs, by item id. */
    public static List<String> outputsOf(Set<String> inputs) {
        return CONVERSIONS.stream()
                .filter(c -> inputs.contains(BuiltInRegistries.ITEM.getKey(c.input()).toString()))
                .map(c -> BuiltInRegistries.ITEM.getKey(c.output()).toString())
                .toList();
    }

    /**
     * What one of the town's conversions could do now: {@code wanted} of its output, of which it can make {@code can}
     * from what it holds; to make the rest, it's short of so many of each input in {@code short_} (empty: not short).
     */
    public record Potential(Conversion conversion, int wanted, int can, java.util.Map<Item, Integer> short_) {}

    /** The town's potential: each conversion its trades can do, what of its output is wanted, and what it's short of. */
    public static List<Potential> potential(ServerLevel level, Warehouse warehouse, GlobalPos desk, GlobalPos bell,
            MerchantStock stock, long now) {
        Set<String> trades = tradesOf(level, bell);
        List<Potential> out = new java.util.ArrayList<>();
        for (Conversion conversion : CONVERSIONS) {
            if (conversion.trades().stream().noneMatch(trades::contains)) {
                continue;
            }
            var kind = Comforts.kindOf(conversion.output());
            int wanted = kind.isPresent()
                    ? comfortWanted(Territory.get(level.getServer()), desk, stock, Comforts.name(kind.get()), conversion.output())
                            * (1 + routesOut(level, desk))
                    : wanted(conversion.output(), warehouse, desk, stock, now);
            int batches = (wanted + conversion.out() - 1) / conversion.out();
            int have = stock.count(new ItemCost(conversion.input(), 1));
            int have2 = conversion.input2() == null ? Integer.MAX_VALUE : stock.count(new ItemCost(conversion.input2(), 1));
            int can = Math.min(batches, Math.min(have / conversion.in(),
                    conversion.input2() == null ? Integer.MAX_VALUE : have2 / conversion.in2()));
            java.util.Map<Item, Integer> short_ = new java.util.LinkedHashMap<>();
            if (batches * conversion.in() > have) {
                short_.put(conversion.input(), batches * conversion.in() - have);
            }
            if (conversion.input2() != null && batches * conversion.in2() > have2) {
                short_.put(conversion.input2(), batches * conversion.in2() - have2);
            }
            out.add(new Potential(conversion, wanted, can * conversion.out(), short_));
        }
        return out;
    }

    private static Set<String> tradesOf(ServerLevel level, GlobalPos bell) {
        return dev.b28.enterprise.settlement.Settlements.get(level.getServer()).roster(bell).values().stream()
                .map(dev.b28.enterprise.settlement.Settlements.Resident::profession).collect(Collectors.toSet());
    }

    /**
     * What the town's workshop would draw a day of each input to keep up the comforts its trades make, and what an item
     * of it earns there: each kind at the town's ration of it (and as much again for each town its caravans run to, as
     * {@link #convert} makes), in the inputs that make it, and back through what its own trades make of those (books from
     * paper, paper from cane). An input earns its share of the output's worth to the town, by the inputs' worth: a cane
     * in biscuits earns more than a cane pressed into paper. Its farmers grow for this as for any demand, at that worth.
     * Item id -> {items a day, emeralds each}.
     */
    public static java.util.Map<String, double[]> inputDemand(ServerLevel level, GlobalPos desk, GlobalPos bell,
            int villagers) {
        Set<String> trades = tradesOf(level, bell);
        int towns = 1 + routesOut(level, desk);
        long now = level.getGameTime();
        java.util.Map<Item, double[]> out = new java.util.LinkedHashMap<>();
        for (Conversion conversion : CONVERSIONS) {
            if (conversion.trades().stream().anyMatch(trades::contains) && Comforts.kindOf(conversion.output()).isPresent()) {
                double each = worth(level, desk, conversion.output(), now);
                draw(conversion, villagers * Comforts.RATION_EMERALDS * towns
                        / dev.b28.enterprise.money.Prices.emeraldsEach(conversion.output()), each, trades, out, 2);
            }
        }
        java.util.Map<String, double[]> byId = new java.util.LinkedHashMap<>();
        out.forEach((item, d) -> byId.put(BuiltInRegistries.ITEM.getKey(item).toString(), d));
        return byId;
    }

    /**
     * Adds what making {@code made} a day (worth {@code each} apiece) draws of each input, at its share of that worth;
     * and of what makes those, {@code depth} steps back. Where two outputs draw on one input, it keeps the higher worth.
     */
    private static void draw(Conversion conversion, double made, double each, Set<String> trades,
            java.util.Map<Item, double[]> out, int depth) {
        double batches = made / conversion.out();
        double inputsWorth = inputsWorth(conversion);
        java.util.Map<Item, Integer> inputs = new java.util.LinkedHashMap<>();
        inputs.put(conversion.input(), conversion.in());
        if (conversion.input2() != null) {
            inputs.put(conversion.input2(), conversion.in2());
        }
        inputs.forEach((input, n) -> {
            double perDay = batches * n;
            // Its share of the batch's worth, by what it's worth beside the other inputs.
            double share = conversion.out() * each * dev.b28.enterprise.money.Prices.emeraldsEach(input) / inputsWorth;
            double[] d = out.computeIfAbsent(input, i -> new double[2]);
            d[0] += perDay;
            d[1] = Math.max(d[1], share);
            if (depth > 1) {
                CONVERSIONS.stream()
                        .filter(c -> c.output() == input && c.trades().stream().anyMatch(trades::contains))
                        .findFirst().ifPresent(c -> draw(c, perDay, share, trades, out, depth - 1));
            }
        });
    }

    /** What a batch earns for each emerald of its inputs' worth, its output worth {@code each} apiece. */
    public static double gain(Conversion conversion, double each) {
        return conversion.out() * each / Math.max(1e-9, inputsWorth(conversion));
    }

    /** What a batch's inputs are worth at their prices, in emeralds. */
    static double inputsWorth(Conversion conversion) {
        return conversion.in() * dev.b28.enterprise.money.Prices.emeraldsEach(conversion.input())
                + (conversion.input2() == null ? 0
                        : conversion.in2() * dev.b28.enterprise.money.Prices.emeraldsEach(conversion.input2()));
    }

    /**
     * What one of an output is worth to the town now: a comfort, what its people would pay for it (more the shorter
     * they are, see {@link dev.b28.enterprise.caravan.Desires#reckon}); anything else, its price.
     */
    static double worth(ServerLevel level, GlobalPos desk, Item output, long now) {
        return Comforts.kindOf(output).isPresent()
                ? dev.b28.enterprise.caravan.Desires.reckon(level.getServer(), desk, output, now)
                : dev.b28.enterprise.money.Prices.emeraldsEach(output);
    }

    /**
     * Runs each conversion the town's trades can do, as far as its output is wanted. Its Merchant notes in the books why
     * each stood idle or fell short: its potential, and what it lacks. Returns items made.
     */
    public static int convert(ServerLevel level, Warehouse warehouse, GlobalPos desk, GlobalPos bell, MerchantStock stock,
            long now) {
        var books = dev.b28.enterprise.merchant.Books.get(level.getServer());
        int made = 0;
        // Inputs go first where they earn most: the conversion whose output is worth most beside its inputs' worth
        // (biscuits before paper, while the town is short of treats). Ties keep the list's order.
        List<Potential> ranked = new java.util.ArrayList<>(potential(level, warehouse, desk, bell, stock, now));
        java.util.Map<Conversion, Double> gain = new java.util.HashMap<>();
        ranked.forEach(p -> gain.put(p.conversion(), gain(p.conversion(), worth(level, desk, p.conversion().output(), now))));
        ranked.sort(java.util.Comparator.comparingDouble((Potential p) -> -gain.get(p.conversion())));
        for (Potential p : ranked) {
            // Not the crop set aside for its forward sales: the bakers leave the wheat it owes.
            int kept = dev.b28.enterprise.caravan.ForwardSales.setAside(level.getServer(), desk, p.conversion().input(),
                    stock, now);
            int free = stock.count(new ItemCost(p.conversion().input(), 1)) - kept;
            int wanted = kept <= 0 ? p.wanted() : Math.min(p.wanted(),
                    Math.max(0, free) / p.conversion().in() * p.conversion().out());
            int done = run(p.conversion(), wanted, stock);
            made += done;
            if (done > 0) {
                books.why(desk, now, null, p.conversion().output(), "made: wanted here or by its caravans' towns", done);
            }
        }
        // What it still wants and lacks, once the inputs went where they earn most: its potential, unmet.
        for (Potential p : potential(level, warehouse, desk, bell, stock, now)) {
            if (p.wanted() > 0) {
                p.short_().forEach((input, by) -> books.whyNow(desk, now, null, p.conversion().output(),
                        "could make more: short of " + BuiltInRegistries.ITEM.getKey(input).getPath(), by));
            }
        }
        return made;
    }

    /** Runs the town's caravans make to other markets: each is somewhere its comforts can be sold. */
    static int routesOut(ServerLevel level, GlobalPos desk) {
        return (int) dev.b28.enterprise.caravan.CaravanLedger.get(level.getServer()).routes().stream()
                .filter(r -> r.from().equals(desk)).count();
    }

    /**
     * Comforts a trade makes while the town is short of that kind, up to its par: carpets for furnishings, leather
     * boots for clothing.
     */
    public static int comfortWanted(Territory territory, GlobalPos desk, MerchantStock stock, String kind, Item output) {
        double short_ = territory.comfortPar(desk) - Comforts.onHand(stock).getOrDefault(kind, 0.0);
        return short_ <= 0 ? 0 : (int) Math.ceil(short_ * dev.b28.enterprise.money.Prices.itemsPerEmerald(output));
    }

    /** How many more of {@code output} the town wants: paper up to its par when a locker buys it; bread for food. */
    static int wanted(Item output, Warehouse warehouse, GlobalPos desk, MerchantStock stock, long now) {
        if (output == Items.BREAD || output == Items.COOKED_COD) {
            return Math.max(0, (2 * Warehouse.FOOD_PAR_POINTS - stock.foodNutrition()) / 5);
        }
        if (warehouse.capacity(desk, output) == 0) {
            return 0;
        }
        return Math.max(0, warehouse.par(desk, output, true, now) - stock.count(new ItemCost(output, 1)));
    }

    public static int runForTest(Conversion conversion, int wanted, MerchantStock stock) {
        return run(conversion, wanted, stock);
    }

    /** Converts as many whole batches as cover {@code wanted}, as far as inputs and room allow. */
    static int run(Conversion conversion, int wanted, MerchantStock stock) {
        int batches = Math.min((wanted + conversion.out() - 1) / conversion.out(),
                stock.count(new ItemCost(conversion.input(), 1)) / conversion.in());
        if (conversion.input2() != null) {
            batches = Math.min(batches, stock.count(new ItemCost(conversion.input2(), 1)) / conversion.in2());
        }
        if (batches <= 0) {
            return 0;
        }
        int taken = stock.take(new ItemCost(conversion.input(), 1), batches * conversion.in()).stream()
                .mapToInt(ItemStack::getCount).sum();
        if (conversion.input2() != null) {
            stock.take(new ItemCost(conversion.input2(), 1), taken / conversion.in() * conversion.in2());
        }
        int made = taken / conversion.in() * conversion.out();
        int left = made;
        while (left > 0) {
            int batch = Math.min(conversion.output().getDefaultMaxStackSize(), left);
            ItemStack rest = stock.insert(new ItemStack(conversion.output(), batch));
            left -= batch - rest.getCount();
            if (!rest.isEmpty()) {
                // No room: put back the input for what couldn't be stored.
                stock.insert(new ItemStack(conversion.input(), rest.getCount() / conversion.out() * conversion.in()));
                made -= rest.getCount();
                break;
            }
        }
        return made;
    }
}
