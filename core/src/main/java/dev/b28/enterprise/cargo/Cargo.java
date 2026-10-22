package dev.b28.enterprise.cargo;

import dev.b28.enterprise.Enterprise;
import net.minecraft.core.component.DataComponentType;
import net.minecraft.core.registries.Registries;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.inventory.MenuType;
import net.minecraft.world.item.CreativeModeTabs;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.block.SoundType;
import net.minecraft.world.level.block.entity.BlockEntityType;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.material.MapColor;
import java.util.LinkedHashMap;
import java.util.Map;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.neoforge.event.BuildCreativeModeTabContentsEvent;
import net.neoforged.neoforge.registries.DeferredBlock;
import net.neoforged.neoforge.registries.DeferredHolder;
import net.neoforged.neoforge.registries.DeferredItem;
import net.neoforged.neoforge.registries.DeferredRegister;

/** Crates and the papers that ride with them. */
public final class Cargo {
    private static final DeferredRegister.Blocks BLOCKS = DeferredRegister.createBlocks(Enterprise.MOD_ID);
    private static final DeferredRegister.Items ITEMS = DeferredRegister.createItems(Enterprise.MOD_ID);
    private static final DeferredRegister<BlockEntityType<?>> BLOCK_ENTITIES =
            DeferredRegister.create(Registries.BLOCK_ENTITY_TYPE, Enterprise.MOD_ID);
    private static final DeferredRegister<MenuType<?>> MENUS = DeferredRegister.create(Registries.MENU, Enterprise.MOD_ID);
    private static final DeferredRegister.DataComponents COMPONENTS =
            DeferredRegister.createDataComponents(Registries.DATA_COMPONENT_TYPE, Enterprise.MOD_ID);

    /** The papers a bundle of documents holds. */
    public static final DeferredHolder<DataComponentType<?>, DataComponentType<Papers>> PAPERS =
            COMPONENTS.registerComponentType("papers", b -> b.persistent(Papers.CODEC).networkSynchronized(Papers.STREAM_CODEC));
    /** The pallet a package belongs to (one of the core's: {@link PackageTag}). */
    public static final DeferredHolder<DataComponentType<?>, DataComponentType<PackageTag>> PACKAGE =
            COMPONENTS.registerComponentType("package", b -> b.persistent(PackageTag.CODEC).networkSynchronized(PackageTag.STREAM_CODEC));

    public static final DeferredBlock<CrateBlock> CRATE = BLOCKS.register("crate", () -> new CrateBlock(
            BlockBehaviour.Properties.of().mapColor(MapColor.WOOD).strength(2.0f).sound(SoundType.WOOD).ignitedByLava()));
    public static final DeferredItem<CrateItem> CRATE_ITEM = ITEMS.register("crate",
            () -> new CrateItem(CRATE.get(), new Item.Properties().stacksTo(Crates.EMPTIES_KEPT)));
    public static final DeferredItem<DocumentsItem> DOCUMENTS = ITEMS.register("documents",
            () -> new DocumentsItem(new Item.Properties().stacksTo(1)));

    /**
     * An item for each kind of packing the content pack lists that has none of its own here ({@code
     * binding/ModPack.PACKAGES}, the table the core checks): a barrel, a bundle. Loose stacks and the crate are shown as
     * the crate ({@link #packageItem}).
     */
    private static final Map<String, DeferredItem<PackageItem>> PACKAGE_ITEMS = new LinkedHashMap<>();

    static {
        for (var kind : dev.b28.enterprise.binding.ModPack.PACKAGES) {
            if (!kind.name().equals("loose") && !kind.name().equals("crate")) {
                PACKAGE_ITEMS.put(kind.name(), ITEMS.register(kind.name(), () -> new PackageItem(new Item.Properties().stacksTo(1))));
            }
        }
    }

    /** The item a package of the kind called {@code kind} is shown as: its own, or the crate. */
    public static Item packageItem(String kind) {
        DeferredItem<PackageItem> own = PACKAGE_ITEMS.get(kind);
        return own != null ? own.get() : CRATE_ITEM.get();
    }

    /** Whether {@code stack} is a package of one of the core's pallets. */
    public static boolean isPackage(net.minecraft.world.item.ItemStack stack) {
        return stack.has(PACKAGE.get());
    }

    public static final DeferredHolder<BlockEntityType<?>, BlockEntityType<CrateBlockEntity>> CRATE_ENTITY =
            BLOCK_ENTITIES.register("crate",
                    () -> BlockEntityType.Builder.of(CrateBlockEntity::new, CRATE.get()).build(null));
    public static final DeferredHolder<MenuType<?>, MenuType<CrateMenu>> CRATE_MENU =
            MENUS.register("crate", () -> new MenuType<>(CrateMenu::new, FeatureFlags.DEFAULT_FLAGS));

    private Cargo() {}

    public static void register(IEventBus modBus) {
        COMPONENTS.register(modBus);
        BLOCKS.register(modBus);
        ITEMS.register(modBus);
        BLOCK_ENTITIES.register(modBus);
        MENUS.register(modBus);
        modBus.addListener(Cargo::addToCreativeTabs);
    }

    private static void addToCreativeTabs(BuildCreativeModeTabContentsEvent event) {
        if (event.getTabKey() == CreativeModeTabs.FUNCTIONAL_BLOCKS) {
            event.accept(CRATE_ITEM);
        }
        if (event.getTabKey() == CreativeModeTabs.TOOLS_AND_UTILITIES) {
            event.accept(DOCUMENTS);
        }
    }
}
