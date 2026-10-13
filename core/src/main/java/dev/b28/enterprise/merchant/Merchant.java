package dev.b28.enterprise.merchant;

import com.google.common.collect.ImmutableSet;
import dev.b28.enterprise.Enterprise;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.sounds.SoundEvents;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.ai.village.poi.PoiType;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.CreativeModeTabs;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.SoundType;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.material.MapColor;
import net.minecraft.world.phys.AABB;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.neoforge.event.BuildCreativeModeTabContentsEvent;
import net.neoforged.neoforge.registries.DeferredBlock;
import net.neoforged.neoforge.registries.DeferredHolder;
import net.neoforged.neoforge.registries.DeferredItem;
import net.neoforged.neoforge.registries.DeferredRegister;

/** The Merchant villager profession and its job site, the Merchant's Desk. */
public final class Merchant {
    private static final DeferredRegister.Blocks BLOCKS = DeferredRegister.createBlocks(Enterprise.MOD_ID);
    private static final DeferredRegister.Items ITEMS = DeferredRegister.createItems(Enterprise.MOD_ID);
    private static final DeferredRegister<PoiType> POI_TYPES =
            DeferredRegister.create(Registries.POINT_OF_INTEREST_TYPE, Enterprise.MOD_ID);
    private static final DeferredRegister<VillagerProfession> PROFESSIONS =
            DeferredRegister.create(Registries.VILLAGER_PROFESSION, Enterprise.MOD_ID);

    public static final DeferredBlock<Block> DESK = BLOCKS.registerSimpleBlock(
            "merchants_desk",
            BlockBehaviour.Properties.of()
                    .mapColor(MapColor.WOOD)
                    .strength(2.5F)
                    .sound(SoundType.WOOD)
                    .ignitedByLava());

    public static final DeferredItem<BlockItem> DESK_ITEM = ITEMS.registerSimpleBlockItem(DESK);

    // One villager per desk, like every vanilla workstation.
    public static final DeferredHolder<PoiType, PoiType> DESK_POI = POI_TYPES.register(
            "merchants_desk",
            () -> new PoiType(ImmutableSet.copyOf(DESK.get().getStateDefinition().getPossibleStates()), 1, 1));

    public static final DeferredHolder<VillagerProfession, VillagerProfession> PROFESSION = PROFESSIONS.register(
            "merchant",
            () -> new VillagerProfession(
                    "merchant",
                    poi -> poi.is(DESK_POI.getKey()),
                    poi -> poi.is(DESK_POI.getKey()),
                    ImmutableSet.of(),
                    ImmutableSet.of(),
                    SoundEvents.VILLAGER_WORK_CARTOGRAPHER));

    private Merchant() {}

    /** How many lockers a Merchant of this villager level manages: 2 at Novice, up to 10 at Master. */
    public static int lockerLimit(int villagerLevel) {
        return 2 * Math.clamp(villagerLevel, 1, 5);
    }

    /** The villager whose job site is the workstation at {@code pos}, if one is within {@code range}. */
    public static Optional<Villager> workingAt(ServerLevel level, BlockPos pos, int range) {
        GlobalPos site = GlobalPos.of(level.dimension(), pos);
        return level.getEntitiesOfClass(Villager.class, new AABB(pos).inflate(range), villager -> villager
                        .getBrain()
                        .getMemory(MemoryModuleType.JOB_SITE)
                        .filter(site::equals)
                        .isPresent())
                .stream()
                .findFirst();
    }

    public static void register(IEventBus modBus) {
        BLOCKS.register(modBus);
        ITEMS.register(modBus);
        POI_TYPES.register(modBus);
        PROFESSIONS.register(modBus);
        modBus.addListener(Merchant::addToCreativeTabs);
    }

    private static void addToCreativeTabs(BuildCreativeModeTabContentsEvent event) {
        if (event.getTabKey() == CreativeModeTabs.FUNCTIONAL_BLOCKS) {
            event.accept(DESK_ITEM);
        }
    }
}
