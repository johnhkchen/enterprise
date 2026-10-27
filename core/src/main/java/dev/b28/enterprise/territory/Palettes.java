package dev.b28.enterprise.territory;

import com.google.gson.JsonObject;
import java.util.HashMap;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.Property;

/**
 * A kingdom's palettes (the studio's `town_kits/<kingdom>/palettes.json`): parts are authored in the base palette, every
 * role (post, beam, infill, roof...) its own block, and a palette is a map from role to block. A house of the module set
 * recoloured: each base block replaced by the palette's block for its role, a stair or slab keeping its state where the
 * new block is one too. A kingdom built on another's module set (Stormwind on Lordaeron's) names it in `base_of`, and maps
 * Lordaeron's palette names to its own in `aliases`, so the town rules work unchanged.
 */
public final class Palettes {
    private Palettes() {}

    /** The module set's authoring palette: Lordaeron's `stone`. */
    static final String SET = "lordaeron";

    /**
     * The cells recoloured to the kingdom's {@code palette} (a Lordaeron name: an alias of the kingdom's where it has
     * them); as they are if the kingdom or palette has none, or it's the base.
     */
    public static Map<BlockPos, BlockState> apply(ResourceManager resources, String kingdom, String palette,
            Map<BlockPos, BlockState> cells) {
        Optional<JsonObject> base = WallKit.json(resources, "town_kits/" + SET + "/palettes.json");
        Optional<JsonObject> own = WallKit.json(resources, "town_kits/" + kingdom + "/palettes.json");
        if (base.isEmpty() || own.isEmpty() || palette == null) {
            return cells;
        }
        String name = own.get().has("aliases") && own.get().getAsJsonObject("aliases").has(palette)
                ? own.get().getAsJsonObject("aliases").get(palette).getAsString() : palette;
        JsonObject palettes = own.get().getAsJsonObject("palettes");
        JsonObject baseRoles = base.get().getAsJsonObject("palettes").getAsJsonObject(base.get().get("base").getAsString());
        if (!palettes.has(name)) {
            return cells;
        }
        JsonObject target = palettes.getAsJsonObject(name);
        // A kingdom of its own set lists only the roles it changes (`inherit`); one on another's set lists them all.
        Map<Block, Block> swap = new HashMap<>();
        for (var role : baseRoles.entrySet()) {
            if (role.getKey().equals("note") || !target.has(role.getKey())) {
                continue;
            }
            Block from = block(role.getValue().getAsString()), to = block(target.get(role.getKey()).getAsString());
            if (from != null && to != null && from != to) {
                swap.putIfAbsent(from, to);
            }
        }
        if (swap.isEmpty()) {
            return cells;
        }
        Map<BlockPos, BlockState> out = new HashMap<>(cells.size());
        cells.forEach((p, s) -> {
            Block to = swap.get(s.getBlock());
            out.put(p, to == null ? s : carry(s, to.defaultBlockState()));
        });
        return out;
    }

    private static Block block(String id) {
        ResourceLocation rl = ResourceLocation.tryParse(id);
        return rl == null || !BuiltInRegistries.BLOCK.containsKey(rl) ? null : BuiltInRegistries.BLOCK.get(rl);
    }

    /** {@code to} with every property {@code from} has that it has too (a stair's facing and half, a slab's type). */
    @SuppressWarnings({"unchecked", "rawtypes"})
    private static BlockState carry(BlockState from, BlockState to) {
        for (Property p : from.getProperties()) {
            if (to.hasProperty(p)) {
                to = to.setValue(p, from.getValue(p));
            }
        }
        return to;
    }
}
