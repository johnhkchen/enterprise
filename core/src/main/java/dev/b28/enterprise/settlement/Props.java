package dev.b28.enterprise.settlement;

import dev.b28.enterprise.Enterprise;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.function.Supplier;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.util.StringRepresentable;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.DyeColor;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.StateDefinition;
import net.minecraft.world.level.block.state.properties.BlockStateProperties;
import net.minecraft.world.level.block.state.properties.DirectionProperty;
import net.minecraft.world.level.block.state.properties.EnumProperty;
import net.minecraft.world.phys.shapes.CollisionContext;
import net.minecraft.world.phys.shapes.VoxelShape;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.neoforge.registries.DeferredBlock;
import net.neoforged.neoforge.registries.DeferredRegister;

/**
 * Decorative twins of the vanilla blocks villagers take as job sites (`docs/assets/README.md`: the studio's builds
 * dress a street with barrels, dye vats, lecterns, a smith's yard): each looks and stands as its vanilla block but is
 * no point of interest, so no villager is hired into a trade by a prop, and no chest beside one becomes a shop. A
 * build uses the vanilla block only where its metadata declares a workstation.
 *
 * <p>{@code enterprise:decor_<vanilla id>}: barrel (facing six ways), cauldron (empty, water, or water dyed any colour:
 * a dye vat), composter, brewing stand, the tables (smithing, fletching, cartography), and lectern, smoker, blast
 * furnace, loom, stonecutter and grindstone (facing four ways).
 */
public final class Props {
    private static final DeferredRegister.Blocks BLOCKS = DeferredRegister.createBlocks(Enterprise.MOD_ID);
    private static final DeferredRegister.Items ITEMS = DeferredRegister.createItems(Enterprise.MOD_ID);

    /** Each twin by its vanilla block's id. */
    public static final Map<String, DeferredBlock<? extends Block>> TWINS = new LinkedHashMap<>();

    public static final DeferredBlock<Cask> BARREL = twin("barrel", () -> Blocks.BARREL, Cask::new);
    public static final DeferredBlock<Vat> CAULDRON = twin("cauldron", () -> Blocks.CAULDRON, Vat::new);

    static {
        for (String id : new String[] {"composter", "brewing_stand", "smithing_table", "fletching_table", "cartography_table"}) {
            Supplier<Block> vanilla = () -> net.minecraft.core.registries.BuiltInRegistries.BLOCK
                    .get(net.minecraft.resources.ResourceLocation.withDefaultNamespace(id));
            twin(id, vanilla, p -> new Prop(p, vanilla));
        }
        for (String id : new String[] {"lectern", "smoker", "blast_furnace", "loom", "stonecutter", "grindstone"}) {
            Supplier<Block> vanilla = () -> net.minecraft.core.registries.BuiltInRegistries.BLOCK
                    .get(net.minecraft.resources.ResourceLocation.withDefaultNamespace(id));
            twin(id, vanilla, p -> new Facing(p, vanilla));
        }
    }

    private Props() {}

    /**
     * The block at {@code at} made its decorative twin if it's a job site with one, facing as it faced: a building's
     * declared workstation shown, not worked. True if it was.
     */
    public static boolean twinOf(net.minecraft.world.level.Level level, BlockPos at) {
        BlockState state = level.getBlockState(at);
        String id = net.minecraft.core.registries.BuiltInRegistries.BLOCK.getKey(state.getBlock()).getPath();
        DeferredBlock<? extends Block> twin = TWINS.get(id.equals("water_cauldron") ? "cauldron" : id);
        if (twin == null) {
            return false;
        }
        BlockState t = twin.get().defaultBlockState();
        if (state.hasProperty(BlockStateProperties.HORIZONTAL_FACING) && t.hasProperty(Facing.FACING)) {
            t = t.setValue(Facing.FACING, state.getValue(BlockStateProperties.HORIZONTAL_FACING));
        } else if (state.hasProperty(BlockStateProperties.FACING) && t.hasProperty(Cask.FACING)) {
            t = t.setValue(Cask.FACING, state.getValue(BlockStateProperties.FACING));
        }
        level.setBlock(at, t, 3);
        return true;
    }

    public static void register(IEventBus modBus) {
        BLOCKS.register(modBus);
        ITEMS.register(modBus);
    }

    private static <B extends Block> DeferredBlock<B> twin(String id, Supplier<Block> vanilla,
            java.util.function.Function<BlockBehaviour.Properties, B> make) {
        DeferredBlock<B> block = BLOCKS.register("decor_" + id,
                () -> make.apply(BlockBehaviour.Properties.ofFullCopy(vanilla.get()).lightLevel(state -> 0)));
        ITEMS.register("decor_" + id, () -> new BlockItem(block.get(), new net.minecraft.world.item.Item.Properties()));
        TWINS.put(id, block);
        return block;
    }

    /** A twin with no facing: it stands as its vanilla block does. */
    public static class Prop extends Block {
        private final Supplier<Block> vanilla;

        public Prop(BlockBehaviour.Properties properties, Supplier<Block> vanilla) {
            super(properties);
            this.vanilla = vanilla;
        }

        /** The vanilla block's state this one stands as. */
        protected BlockState as(BlockState state) {
            return vanilla.get().defaultBlockState();
        }

        @Override
        protected VoxelShape getShape(BlockState state, BlockGetter level, BlockPos pos, CollisionContext context) {
            return as(state).getShape(level, pos, context);
        }

        @Override
        protected VoxelShape getCollisionShape(BlockState state, BlockGetter level, BlockPos pos, CollisionContext context) {
            return as(state).getCollisionShape(level, pos, context);
        }
    }

    /** A twin that faces one of four ways, as a lectern, a loom or a smoker does (toward whoever placed it). */
    public static class Facing extends Prop {
        public static final DirectionProperty FACING = BlockStateProperties.HORIZONTAL_FACING;

        public Facing(BlockBehaviour.Properties properties, Supplier<Block> vanilla) {
            super(properties, vanilla);
            registerDefaultState(stateDefinition.any().setValue(FACING, Direction.NORTH));
        }

        @Override
        protected void createBlockStateDefinition(StateDefinition.Builder<Block, BlockState> builder) {
            builder.add(FACING);
        }

        @Override
        public BlockState getStateForPlacement(BlockPlaceContext context) {
            return defaultBlockState().setValue(FACING, context.getHorizontalDirection().getOpposite());
        }

        @Override
        protected BlockState as(BlockState state) {
            BlockState v = super.as(state);
            if (v.hasProperty(BlockStateProperties.ATTACH_FACE)) {
                v = v.setValue(BlockStateProperties.ATTACH_FACE, net.minecraft.world.level.block.state.properties.AttachFace.FLOOR);
            }
            return v.hasProperty(BlockStateProperties.HORIZONTAL_FACING)
                    ? v.setValue(BlockStateProperties.HORIZONTAL_FACING, state.getValue(FACING)) : v;
        }

        @Override
        protected BlockState rotate(BlockState state, net.minecraft.world.level.block.Rotation rotation) {
            return state.setValue(FACING, rotation.rotate(state.getValue(FACING)));
        }

        @Override
        protected BlockState mirror(BlockState state, net.minecraft.world.level.block.Mirror mirror) {
            return state.rotate(mirror.getRotation(state.getValue(FACING)));
        }
    }

    /** A barrel that's only a barrel: stores nothing, faces any of six ways. */
    public static class Cask extends Block {
        public static final DirectionProperty FACING = BlockStateProperties.FACING;

        public Cask(BlockBehaviour.Properties properties) {
            super(properties);
            registerDefaultState(stateDefinition.any().setValue(FACING, Direction.UP));
        }

        @Override
        protected void createBlockStateDefinition(StateDefinition.Builder<Block, BlockState> builder) {
            builder.add(FACING);
        }

        @Override
        public BlockState getStateForPlacement(BlockPlaceContext context) {
            return defaultBlockState().setValue(FACING, context.getNearestLookingDirection().getOpposite());
        }

        @Override
        protected BlockState rotate(BlockState state, net.minecraft.world.level.block.Rotation rotation) {
            return state.setValue(FACING, rotation.rotate(state.getValue(FACING)));
        }

        @Override
        protected BlockState mirror(BlockState state, net.minecraft.world.level.block.Mirror mirror) {
            return state.rotate(mirror.getRotation(state.getValue(FACING)));
        }
    }

    /** What a vat holds: nothing, water, or water dyed one of the sixteen colours. */
    public enum Contents implements StringRepresentable {
        EMPTY, WATER, WHITE, ORANGE, MAGENTA, LIGHT_BLUE, YELLOW, LIME, PINK, GRAY, LIGHT_GRAY, CYAN, PURPLE, BLUE, BROWN,
        GREEN, RED, BLACK;

        @Override
        public String getSerializedName() {
            return name().toLowerCase(java.util.Locale.ROOT);
        }

        /** The colour its water shows (ARGB), for the client's tint; water's own blue for plain water. */
        public int tint() {
            return switch (this) {
                case EMPTY, WATER -> 0xFF3F76E4;
                default -> DyeColor.valueOf(name()).getTextureDiffuseColor() | 0xFF000000;
            };
        }
    }

    /** A cauldron that's only a cauldron: empty, full of water, or a dye vat of any colour. */
    public static class Vat extends Prop {
        public static final EnumProperty<Contents> CONTENTS = EnumProperty.create("contents", Contents.class);

        public Vat(BlockBehaviour.Properties properties) {
            super(properties, () -> Blocks.CAULDRON);
            registerDefaultState(stateDefinition.any().setValue(CONTENTS, Contents.EMPTY));
        }

        @Override
        protected void createBlockStateDefinition(StateDefinition.Builder<Block, BlockState> builder) {
            builder.add(CONTENTS);
        }
    }
}
