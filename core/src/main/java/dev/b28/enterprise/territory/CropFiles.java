package dev.b28.enterprise.territory;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.Reader;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;

/**
 * The crops a field can grow, one file each: {@code data/enterprise/crops/<id>.json} (docs/markets-plan.md 5: "adding
 * cotton, tobacco, indigo, Farmer's Delight's crops or flowers is adding files"). The mod ships its six (wheat,
 * carrots, potatoes, beetroot, cane, berries); a datapack adds one with a file of its own, and a crop with a file and a
 * bid somewhere is a crop the towns grow for money. Read from the server's resources the first time they're asked
 * for, again after a reload.
 *
 * <p>A file: {@code item} (what working the crop yields), {@code block} (what's planted: on farmland, by water or as a
 * bush, {@code sown}), {@code order} (the crops' order: the first of equals is planted), {@code rotation} (it shares
 * farmland with the others that do, a field turned over from one to another, sold forward and rented), {@code sold_as}
 * (what a locker takes for it: paper for cane), {@code cash} (grown to sell, not eaten: a town still growing plants it
 * only where it's wanted and its land suits it), {@code fit} (0 to 1 a land, how well it suits a new field there:
 * {@code plains woods taiga desert shore hills water}, 1 where not named) and {@code made_into} (what a trade makes of it
 * alone, {@code from} of it into {@code makes} of {@code good}: what its Farmers price it as beside itself). The core
 * takes the same with the content pack ({@code binding/ModPack}).
 */
public final class CropFiles {
    /** How a crop is planted, and the blocks of water a 9 x 9 field of it takes. */
    public enum Sown {
        /** Farmland round water: wheat, roots, most modded crops. */
        FARMLAND(1),
        /** Along water channels, on sand or earth: cane. */
        BY_WATER(Fields.SIZE * 3),
        /** A bush on every block: berries. */
        BUSH(0);

        public final int water;

        Sown(int water) {
            this.water = water;
        }
    }

    /** What a trade makes of a crop alone. */
    public record Made(Item good, double from, double makes) {}

    /** A crop's file, read. */
    public record Crop(ResourceLocation id, Item item, Block block, Sown sown, int order, boolean rotation, Item soldAs,
            boolean cash, Map<String, Double> fit, List<Made> madeInto) {}

    /** The lands a fit names, in the core's order ({@code farms::LANDS}). */
    public static final List<String> LANDS = List.of("plains", "woods", "taiga", "desert", "shore", "hills", "water");

    private static volatile List<Crop> loaded;

    private CropFiles() {}

    /** Forgets the files read, for the server's resources reloading. */
    public static void forget() {
        loaded = null;
    }

    /** Every crop, in their files' order: the running server's. */
    public static List<Crop> all() {
        List<Crop> crops = loaded;
        if (crops == null) {
            MinecraftServer server = net.neoforged.neoforge.server.ServerLifecycleHooks.getCurrentServer();
            if (server == null) {
                throw new IllegalStateException("no server: the crop files are its resources");
            }
            crops = of(server.getResourceManager());
            loaded = crops;
        }
        return crops;
    }

    /**
     * The crops in {@code resources}: each file under {@code enterprise:crops/}, by its order, then its name. A file the
     * mod can't read is left out and logged, with why.
     */
    public static List<Crop> of(ResourceManager resources) {
        List<Crop> crops = new ArrayList<>();
        resources.listResources("crops", p -> p.getNamespace().equals("enterprise") && p.getPath().endsWith(".json"))
                .forEach((path, resource) -> {
                    try (Reader reader = resource.openAsReader()) {
                        crops.add(read(path, JsonParser.parseReader(reader).getAsJsonObject()));
                    } catch (Exception e) {
                        dev.b28.enterprise.Enterprise.LOGGER.warn("Crop file {} left out: {}", path, e.getMessage());
                    }
                });
        crops.sort(Comparator.comparingInt(Crop::order).thenComparing(c -> c.id().toString()));
        return List.copyOf(crops);
    }

    /** A crop file, or what's wrong with it: an item or block the game hasn't, a land it doesn't know, a fit beyond 0 to 1. */
    public static Crop read(ResourceLocation id, JsonObject o) {
        List<String> wrong = new ArrayList<>();
        Item item = item(o, "item", wrong);
        String blockName = o.has("block") ? o.get("block").getAsString() : null;
        Block block = blockName == null ? Blocks.AIR : BuiltInRegistries.BLOCK.get(ResourceLocation.parse(blockName));
        if (block == Blocks.AIR) {
            wrong.add("no block " + blockName);
        }
        Sown sown = Sown.FARMLAND;
        if (o.has("sown")) {
            try {
                sown = Sown.valueOf(o.get("sown").getAsString().toUpperCase(java.util.Locale.ROOT));
            } catch (IllegalArgumentException e) {
                wrong.add("sown " + o.get("sown").getAsString() + " (farmland, by_water or bush)");
            }
        }
        Item soldAs = o.has("sold_as") ? item(o, "sold_as", wrong) : item;
        Map<String, Double> fit = new LinkedHashMap<>();
        if (o.has("fit")) {
            for (Map.Entry<String, JsonElement> e : o.getAsJsonObject("fit").entrySet()) {
                double f = e.getValue().getAsDouble();
                if (!LANDS.contains(e.getKey())) {
                    wrong.add("no land " + e.getKey() + " (" + String.join(", ", LANDS) + ")");
                } else if (!(f >= 0 && f <= 1)) {
                    wrong.add("fits " + e.getKey() + " " + f + ", not 0 to 1");
                } else {
                    fit.put(e.getKey(), f);
                }
            }
        }
        List<Made> made = new ArrayList<>();
        if (o.has("made_into")) {
            for (JsonElement e : o.getAsJsonArray("made_into")) {
                JsonObject m = e.getAsJsonObject();
                Item good = item(m, "good", wrong);
                double from = m.get("from").getAsDouble();
                double makes = m.get("makes").getAsDouble();
                if (!(from > 0 && makes > 0)) {
                    wrong.add(from + " into " + makes + ", not numbers above nothing");
                }
                made.add(new Made(good, from, makes));
            }
        }
        if (!wrong.isEmpty()) {
            throw new IllegalArgumentException(String.join("; ", wrong));
        }
        return new Crop(id, item, block, sown, o.has("order") ? o.get("order").getAsInt() : Integer.MAX_VALUE,
                o.has("rotation") && o.get("rotation").getAsBoolean(), soldAs, o.has("cash") && o.get("cash").getAsBoolean(),
                Map.copyOf(fit), List.copyOf(made));
    }

    private static Item item(JsonObject o, String key, List<String> wrong) {
        if (!o.has(key)) {
            wrong.add("no " + key);
            return Items.AIR;
        }
        String name = o.get(key).getAsString();
        Item item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(name));
        if (item == Items.AIR) {
            wrong.add("no item " + name);
        }
        return item;
    }

    /** The crop of an item, if a file has it. */
    public static Optional<Crop> of(Item item) {
        return all().stream().filter(c -> c.item() == item).findFirst();
    }

    /** The field crops, in order. */
    public static List<Item> crops() {
        return all().stream().map(Crop::item).toList();
    }

    /** The crops that share farmland. */
    public static List<Item> rotation() {
        return all().stream().filter(Crop::rotation).map(Crop::item).toList();
    }

    /** The crop whose item is called {@code name}, without its namespace ({@code carrot}, {@code cotton}). */
    public static Optional<Crop> named(String name) {
        return all().stream().filter(c -> BuiltInRegistries.ITEM.getKey(c.item()).getPath().equals(name)).findFirst();
    }

    /** A field's name for a crop (the field sets' and the open fields' states): its item's, without the namespace; wheat for none. */
    public static String name(Item crop) {
        return of(crop).map(c -> BuiltInRegistries.ITEM.getKey(c.item()).getPath()).orElse("wheat");
    }

    /** What's sown on farmland of a crop: its file's block where it's one that ripens, else wheat. */
    public static net.minecraft.world.level.block.CropBlock farmland(Item crop) {
        return of(crop).map(Crop::block).filter(b -> b instanceof net.minecraft.world.level.block.CropBlock)
                .map(b -> (net.minecraft.world.level.block.CropBlock) b)
                .orElse((net.minecraft.world.level.block.CropBlock) Blocks.WHEAT);
    }

    /** What a locker takes for a crop: its file's, else itself. */
    public static Item soldAs(Item crop) {
        return of(crop).map(Crop::soldAs).orElse(crop);
    }
}
