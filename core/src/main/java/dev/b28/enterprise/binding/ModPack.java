package dev.b28.enterprise.binding;

import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.wire.ContentPack;
import dev.b28.enterprise.wire.PackGood;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.food.FoodProperties;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

/**
 * The mod's content pack, as the core takes it at a new world (docs/architecture.md: "content packs are data, checked
 * by the core"): Minecraft's day, and a good for every item the mod prices ({@link Prices}), in the order of their ids,
 * each worth what the mod values it at, a stack of what the item stacks to, and its food if it's food.
 *
 * <p>The proof's pack (B2.1): the goods and their worth only. Comforts, crops, meat, weights and the dials come with
 * {@code content-pack} (B4.1). A pack is read once, when the society is made: a loaded world keeps the goods it was
 * made with, and the host reads them back from the core ({@code Query.Goods}), never from this.
 */
public final class ModPack {
    /** Minecraft's day, in ticks. */
    public static final long DAY_TICKS = 24_000;

    private ModPack() {}

    public static ContentPack of() {
        List<PackGood> goods = new ArrayList<>();
        for (String id : Prices.listed()) {
            Item item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(id));
            if (item == Items.AIR) {
                continue; // priced, but not in this game (a mod that isn't loaded)
            }
            ItemStack one = item.getDefaultInstance();
            FoodProperties food = one.get(DataComponents.FOOD);
            goods.add(new PackGood(id, 1 / Prices.itemsPerEmerald(id), one.getMaxStackSize(),
                    food == null ? 0 : food.nutrition(), food == null ? 0 : food.saturation(), Optional.empty(), List.of()));
        }
        return new ContentPack(DAY_TICKS, goods);
    }
}
