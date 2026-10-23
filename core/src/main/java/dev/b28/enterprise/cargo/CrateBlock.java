package dev.b28.enterprise.cargo;

import com.mojang.serialization.MapCodec;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.core.BlockPos;
import net.minecraft.network.chat.Component;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.BaseEntityBlock;
import net.minecraft.world.level.block.RenderShape;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.BlockHitResult;

/**
 * A crate: a wooden box of nine stacks with a slot for a bundle of documents. Broken, it keeps what's in it, like a
 * shulker box (see its loot table), so goods and papers travel together.
 */
public final class CrateBlock extends BaseEntityBlock {
    public static final MapCodec<CrateBlock> CODEC = simpleCodec(CrateBlock::new);

    public CrateBlock(Properties properties) {
        super(properties);
    }

    @Override
    protected MapCodec<? extends BaseEntityBlock> codec() {
        return CODEC;
    }

    @Override
    public BlockEntity newBlockEntity(BlockPos pos, BlockState state) {
        return new CrateBlockEntity(pos, state);
    }

    @Override
    protected RenderShape getRenderShape(BlockState state) {
        return RenderShape.MODEL;
    }

    @Override
    protected InteractionResult useWithoutItem(BlockState state, Level level, BlockPos pos, Player player,
            BlockHitResult hit) {
        if (!level.isClientSide && level.getBlockEntity(pos) instanceof CrateBlockEntity crate) {
            player.openMenu(crate);
            if (dev.b28.enterprise.binding.Society.isCore() && TOLD.add(player.getUUID())) {
                player.sendSystemMessage(Component.literal(UNCOUNTED));
            }
        }
        return InteractionResult.sidedSuccess(level.isClientSide);
    }

    /**
     * In core mode a crate is plain storage (docs/alpha-plan.md 2): the core counts no packed goods, so a player is told,
     * once a server run, the first time they open one.
     */
    public static final String UNCOUNTED =
            "Crates you pack aren't counted in the alpha: keep goods loose in the desk's chests.";

    private static final Set<UUID> TOLD = ConcurrentHashMap.newKeySet();

    @Override
    protected boolean hasAnalogOutputSignal(BlockState state) {
        return true;
    }

    @Override
    protected int getAnalogOutputSignal(BlockState state, Level level, BlockPos pos) {
        return level.getBlockEntity(pos) instanceof CrateBlockEntity crate
                ? net.minecraft.world.inventory.AbstractContainerMenu.getRedstoneSignalFromContainer(crate) : 0;
    }
}
