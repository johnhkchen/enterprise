package dev.b28.enterprise.locker;

import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.world.entity.ai.village.poi.PoiType;

/** A workstation with at least one container touching it. Double chests appear once. */
public record Locker(BlockPos workstation, Holder<PoiType> type, List<BlockPos> containers) {}
