package dev.b28.enterprise.merchant;

import com.google.common.collect.ImmutableSet;
import dev.b28.enterprise.Enterprise;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.chat.Component;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.sounds.SoundEvents;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.SimpleMenuProvider;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.ai.village.poi.PoiType;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.inventory.MenuType;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.CreativeModeTabs;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.SoundType;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.material.MapColor;
import net.minecraft.world.phys.AABB;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.neoforge.common.NeoForge;
import net.neoforged.neoforge.common.extensions.IMenuTypeExtension;
import net.neoforged.neoforge.event.BuildCreativeModeTabContentsEvent;
import net.neoforged.neoforge.event.entity.player.PlayerInteractEvent;
import net.neoforged.neoforge.network.event.RegisterPayloadHandlersEvent;
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
    private static final DeferredRegister<MenuType<?>> MENUS = DeferredRegister.create(Registries.MENU, Enterprise.MOD_ID);

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

    public static final DeferredHolder<MenuType<?>, MenuType<MerchantMenu>> MENU =
            MENUS.register("merchant", () -> IMenuTypeExtension.create(MerchantMenu::new));

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
        MENUS.register(modBus);
        modBus.addListener(Merchant::addToCreativeTabs);
        modBus.addListener(Merchant::registerPayloads);
        NeoForge.EVENT_BUS.addListener(Merchant::openScreen);
    }

    public static boolean isMerchant(Villager villager) {
        return villager.getVillagerData().getProfession() == PROFESSION.get();
    }

    /** Right-clicking a Merchant opens its screen; it has no trades of its own to show. */
    private static void openScreen(PlayerInteractEvent.EntityInteract event) {
        if (!(event.getTarget() instanceof Villager villager) || !isMerchant(villager) || villager.isBaby()) {
            return;
        }
        event.setCanceled(true);
        event.setCancellationResult(InteractionResult.SUCCESS);
        if (event.getHand() != InteractionHand.MAIN_HAND || !(event.getEntity() instanceof ServerPlayer player)) {
            return;
        }
        ServerLevel level = player.serverLevel();
        BlockPos desk = villager.getBrain()
                .getMemory(MemoryModuleType.JOB_SITE)
                .filter(site -> site.dimension() == level.dimension())
                .map(GlobalPos::pos)
                .orElse(null);
        if (desk == null) {
            player.displayClientMessage(Component.translatable("message.enterprise.merchant.no_desk"), true);
            return;
        }
        MerchantView view = MerchantView.build(level, desk, lockerLimit(villager.getVillagerData().getLevel()));
        int merchantId = villager.getId();
        player.openMenu(
                new SimpleMenuProvider(
                        (id, inventory, p) -> new MerchantMenu(id, view, merchantId), villager.getName()),
                buf -> {
                    MerchantView.STREAM_CODEC.encode(buf, view);
                    buf.writeVarInt(merchantId);
                });
    }

    private static void registerPayloads(RegisterPayloadHandlersEvent event) {
        event.registrar("1").playToServer(
                ToggleTradePayload.TYPE, ToggleTradePayload.STREAM_CODEC, ToggleTradePayload::handle);
    }

    private static void addToCreativeTabs(BuildCreativeModeTabContentsEvent event) {
        if (event.getTabKey() == CreativeModeTabs.FUNCTIONAL_BLOCKS) {
            event.accept(DESK_ITEM);
        }
    }
}
