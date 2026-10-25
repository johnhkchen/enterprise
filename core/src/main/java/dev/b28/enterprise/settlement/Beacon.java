package dev.b28.enterprise.settlement;

import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.effect.MobEffects;
import net.minecraft.world.entity.npc.Villager;

/**
 * A town's bell is its beacon: at each look that counts the town, every hurt villager within its reach is given
 * Regeneration for a while, so a fall or a bite heals over the next minutes instead of adding up till one dies. Not a
 * beacon block: vanilla's give their effects to players only, and one built in every village would be a nether star
 * and nine iron blocks free for the mining.
 */
public final class Beacon {
    /** How long a look's Regeneration lasts: 10 seconds, 2 hearts, about what a look's minute apart lets heal. */
    static final int TICKS = 200;

    private Beacon() {}

    /** The hurt villagers within {@code radius} of {@code bell} given Regeneration; how many. */
    public static int heal(ServerLevel level, BlockPos bell, int radius) {
        int n = 0;
        for (Villager v : CensusTaker.villagersAround(level, bell, radius)) {
            if (v.isAlive() && v.getHealth() < v.getMaxHealth()) {
                v.addEffect(new MobEffectInstance(MobEffects.REGENERATION, TICKS, 0));
                n++;
            }
        }
        return n;
    }
}
