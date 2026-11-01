package dev.b28.enterprise.territory;

import com.google.common.collect.ImmutableSet;
import dev.b28.enterprise.Enterprise;
import net.minecraft.core.registries.Registries;
import net.minecraft.sounds.SoundEvents;
import net.minecraft.world.entity.ai.village.poi.PoiType;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.CreativeModeTabs;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.SoundType;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.material.MapColor;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.neoforge.event.BuildCreativeModeTabContentsEvent;
import net.neoforged.neoforge.registries.DeferredBlock;
import net.neoforged.neoforge.registries.DeferredHolder;
import net.neoforged.neoforge.registries.DeferredItem;
import net.neoforged.neoforge.registries.DeferredRegister;

/**
 * The Lumberjack, the mod's first trade of its own (John: "our first added profession"; docs/lumberjack-plan.md): its
 * job site, the chopping block, and the profession. The core fells its town's woodlots (`kit::Woodlots`) and burns its
 * logs to charcoal ({@link Workshop#CONVERSIONS}); a town puts its stall up as any trade's ({@link Jobs#trades}). Its
 * look is a placeholder from the game's own textures until the studio's.
 */
public final class Lumberjack {
    private static final DeferredRegister.Blocks BLOCKS = DeferredRegister.createBlocks(Enterprise.MOD_ID);
    private static final DeferredRegister.Items ITEMS = DeferredRegister.createItems(Enterprise.MOD_ID);
    private static final DeferredRegister<PoiType> POI_TYPES =
            DeferredRegister.create(Registries.POINT_OF_INTEREST_TYPE, Enterprise.MOD_ID);
    private static final DeferredRegister<VillagerProfession> PROFESSIONS =
            DeferredRegister.create(Registries.VILLAGER_PROFESSION, Enterprise.MOD_ID);

    /** The trade's name, as the core and the census know it. */
    public static final String ID = "enterprise:lumberjack";

    public static final DeferredBlock<Block> CHOPPING_BLOCK = BLOCKS.registerSimpleBlock(
            "chopping_block",
            BlockBehaviour.Properties.of()
                    .mapColor(MapColor.WOOD)
                    .strength(2.0F)
                    .sound(SoundType.WOOD)
                    .noOcclusion() // a stump, short of a block
                    .ignitedByLava());

    public static final DeferredItem<BlockItem> CHOPPING_BLOCK_ITEM = ITEMS.registerSimpleBlockItem(CHOPPING_BLOCK);

    // One villager per block, like every vanilla workstation.
    public static final DeferredHolder<PoiType, PoiType> CHOPPING_BLOCK_POI = POI_TYPES.register(
            "chopping_block",
            () -> new PoiType(ImmutableSet.copyOf(CHOPPING_BLOCK.get().getStateDefinition().getPossibleStates()), 1, 1));

    public static final DeferredHolder<VillagerProfession, VillagerProfession> PROFESSION = PROFESSIONS.register(
            "lumberjack",
            () -> new VillagerProfession(
                    "lumberjack",
                    poi -> poi.is(CHOPPING_BLOCK_POI.getKey()),
                    poi -> poi.is(CHOPPING_BLOCK_POI.getKey()),
                    ImmutableSet.of(),
                    ImmutableSet.of(),
                    SoundEvents.VILLAGER_WORK_FLETCHER));

    private Lumberjack() {}

    public static void register(IEventBus modBus) {
        BLOCKS.register(modBus);
        ITEMS.register(modBus);
        POI_TYPES.register(modBus);
        PROFESSIONS.register(modBus);
        modBus.addListener(Lumberjack::addToCreativeTabs);
    }

    private static void addToCreativeTabs(BuildCreativeModeTabContentsEvent event) {
        if (event.getTabKey() == CreativeModeTabs.FUNCTIONAL_BLOCKS) {
            event.accept(CHOPPING_BLOCK_ITEM);
        }
    }
}
