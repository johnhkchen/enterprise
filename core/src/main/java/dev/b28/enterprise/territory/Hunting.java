package dev.b28.enterprise.territory;

import java.util.List;
import net.minecraft.core.Holder;
import net.minecraft.tags.BiomeTags;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.biome.Biome;
import net.neoforged.neoforge.common.Tags;

/**
 * A town's Fletchers hunt its land: so much game a day per level, of what lives there (rabbits in deserts, snow and
 * taiga; cattle, pigs and fowl on open ground and in woods), with bows; a Fletcher without one sets snares and gets a
 * quarter. Game is food, hides for Leatherworkers, feathers for arrows.
 */
public final class Hunting {
    private Hunting() {}

    /** What a Fletcher of level 1 with a bow brings in a day on land of this biome. */
    public static List<Labour.Haul> game(Holder<Biome> biome) {
        if (biome.is(BiomeTags.IS_OCEAN) || biome.is(BiomeTags.IS_DEEP_OCEAN) || biome.is(BiomeTags.IS_RIVER)) {
            return List.of(new Labour.Haul(Items.FEATHER, 1));
        }
        if (biome.is(Tags.Biomes.IS_DESERT) || biome.is(Tags.Biomes.IS_SNOWY) || biome.is(BiomeTags.IS_TAIGA)
                || biome.is(BiomeTags.IS_BADLANDS)) {
            return List.of(new Labour.Haul(Items.RABBIT, 2), new Labour.Haul(Items.RABBIT_HIDE, 1),
                    new Labour.Haul(Items.FEATHER, 1));
        }
        return List.of(new Labour.Haul(Items.BEEF, 1), new Labour.Haul(Items.PORKCHOP, 1), new Labour.Haul(Items.CHICKEN, 1),
                new Labour.Haul(Items.FEATHER, 2), new Labour.Haul(Items.LEATHER, 0.5));
    }
}
