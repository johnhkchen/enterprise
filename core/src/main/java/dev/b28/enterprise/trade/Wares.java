package dev.b28.enterprise.trade;

import dev.b28.enterprise.Enterprise;
import net.minecraft.world.item.CreativeModeTabs;
import net.minecraft.world.item.Item;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.neoforge.event.BuildCreativeModeTabContentsEvent;
import net.neoforged.neoforge.registries.DeferredItem;
import net.neoforged.neoforge.registries.DeferredRegister;

/**
 * Wares: goods the villager economy doesn't make, standing in for the crafting chains the pack will bring later. Each
 * is an ordinary item with a price, so it rides the whole framework (warehouses, caravans, contracts, comforts); the
 * chain behind it is nominal for now. Tobacco: the coast realm grows and cures the leaf; a Butcher's
 * smokehouse (or the coast's own works) turns leaf into pipe tobacco, a comfort of its own kind, leisure.
 */
public final class Wares {
    private static final DeferredRegister.Items ITEMS = DeferredRegister.createItems(Enterprise.MOD_ID);

    /** Cured tobacco leaf, by the bale: the raw ware. */
    public static final DeferredItem<Item> TOBACCO_LEAF = ITEMS.registerSimpleItem("tobacco_leaf");
    /** Pipe tobacco, cut and pressed: what villagers enjoy (comforts/leisure). */
    public static final DeferredItem<Item> PIPE_TOBACCO = ITEMS.registerSimpleItem("pipe_tobacco");

    private Wares() {}

    public static void register(IEventBus modBus) {
        ITEMS.register(modBus);
        modBus.addListener(Wares::addToCreativeTabs);
    }

    private static void addToCreativeTabs(BuildCreativeModeTabContentsEvent event) {
        if (event.getTabKey() == CreativeModeTabs.INGREDIENTS) {
            event.accept(TOBACCO_LEAF);
            event.accept(PIPE_TOBACCO);
        }
    }
}
