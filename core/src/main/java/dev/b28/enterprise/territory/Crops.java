package dev.b28.enterprise.territory;

import dev.b28.enterprise.hex.Hex;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.core.SectionPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.chunk.LevelChunkSection;

/** What grows where: counts crop plots in a hex, by the item a farmer working them produces. */
public final class Crops {
    /** Crop block -> what working it yields. Stems yield the pumpkin or melon they grow. */
    public static final Map<Block, Item> YIELDS = Map.ofEntries(
            Map.entry(Blocks.WHEAT, Items.WHEAT),
            Map.entry(Blocks.CARROTS, Items.CARROT),
            Map.entry(Blocks.POTATOES, Items.POTATO),
            Map.entry(Blocks.BEETROOTS, Items.BEETROOT),
            Map.entry(Blocks.PUMPKIN_STEM, Items.PUMPKIN),
            Map.entry(Blocks.ATTACHED_PUMPKIN_STEM, Items.PUMPKIN),
            Map.entry(Blocks.MELON_STEM, Items.MELON),
            Map.entry(Blocks.ATTACHED_MELON_STEM, Items.MELON),
            Map.entry(Blocks.SUGAR_CANE, Items.SUGAR_CANE),
            Map.entry(Blocks.COCOA, Items.COCOA_BEANS),
            Map.entry(Blocks.SWEET_BERRY_BUSH, Items.SWEET_BERRIES),
            Map.entry(Blocks.NETHER_WART, Items.NETHER_WART));

    /** Blocks above and below the settlement's bell that count as its farmland. */
    private static final int HEIGHT = 16;

    private Crops() {}

    /** The chunks a hex overlaps, as packed chunk positions. */
    public static List<Long> chunksOf(Hex hex) {
        int minX = (int) Math.floor(hex.centerX() - Hex.ACROSS / 2) - 1;
        int maxX = (int) Math.ceil(hex.centerX() + Hex.ACROSS / 2) + 1;
        int minZ = (int) Math.floor(hex.centerZ() - Hex.SIZE) - 1;
        int maxZ = (int) Math.ceil(hex.centerZ() + Hex.SIZE) + 1;
        List<Long> chunks = new ArrayList<>();
        for (int cx = SectionPos.blockToSectionCoord(minX); cx <= SectionPos.blockToSectionCoord(maxX); cx++) {
            for (int cz = SectionPos.blockToSectionCoord(minZ); cz <= SectionPos.blockToSectionCoord(maxZ); cz++) {
                chunks.add(ChunkPos.asLong(cx, cz));
            }
        }
        return chunks;
    }

    /**
     * Crop plots of {@code hex} in one chunk, within {@link #HEIGHT} blocks of {@code centerY}, or null if the chunk
     * isn't loaded. Chunk sections whose palette holds no crop are skipped without reading their blocks.
     */
    public static Map<Item, Integer> countChunk(ServerLevel level, long chunk, Hex hex, int centerY) {
        int cx = ChunkPos.getX(chunk);
        int cz = ChunkPos.getZ(chunk);
        if (!level.getChunkSource().hasChunk(cx, cz)) {
            return null;
        }
        Map<Item, Integer> plots = new HashMap<>();
        LevelChunk loaded = level.getChunk(cx, cz);
        for (int sy = SectionPos.blockToSectionCoord(centerY - HEIGHT); sy <= SectionPos.blockToSectionCoord(centerY + HEIGHT); sy++) {
            int index = loaded.getSectionIndexFromSectionY(sy);
            if (index < 0 || index >= loaded.getSectionsCount()) {
                continue;
            }
            LevelChunkSection section = loaded.getSection(index);
            if (section.hasOnlyAir() || !section.maybeHas(state -> YIELDS.containsKey(state.getBlock()))) {
                continue;
            }
            countSection(section, cx, sy, cz, hex, centerY, plots);
        }
        return plots;
    }

    /** The seedling of a field crop that grows on farmland. */
    private static final Map<Item, Block> SOWN = Map.of(Items.WHEAT, Blocks.WHEAT, Items.CARROT, Blocks.CARROTS,
            Items.POTATO, Blocks.POTATOES, Items.BEETROOT, Blocks.BEETROOTS);

    /**
     * Turns over up to {@code max} plots of {@code from} in the hexes' loaded chunks to seedlings of {@code to} (both
     * farmland crops), within {@link #HEIGHT} blocks of {@code centerY}. Returns the plots turned over.
     */
    public static int resow(ServerLevel level, List<Hex> hexes, int centerY, Item from, Item to, int max) {
        Block was = SOWN.get(from);
        Block now = SOWN.get(to);
        if (was == null || now == null || max <= 0) {
            return 0;
        }
        int turned = 0;
        for (Hex hex : hexes) {
            for (long chunk : chunksOf(hex)) {
                int cx = ChunkPos.getX(chunk);
                int cz = ChunkPos.getZ(chunk);
                if (!level.getChunkSource().hasChunk(cx, cz)) {
                    continue;
                }
                LevelChunk loaded = level.getChunk(cx, cz);
                for (int sy = SectionPos.blockToSectionCoord(centerY - HEIGHT);
                        sy <= SectionPos.blockToSectionCoord(centerY + HEIGHT); sy++) {
                    int index = loaded.getSectionIndexFromSectionY(sy);
                    if (index < 0 || index >= loaded.getSectionsCount()
                            || !loaded.getSection(index).maybeHas(state -> state.is(was))) {
                        continue;
                    }
                    LevelChunkSection section = loaded.getSection(index);
                    for (int y = 0; y < 16; y++) {
                        int worldY = (sy << 4) + y;
                        if (Math.abs(worldY - centerY) > HEIGHT) {
                            continue;
                        }
                        for (int z = 0; z < 16; z++) {
                            for (int x = 0; x < 16; x++) {
                                int wx = (cx << 4) + x;
                                int wz = (cz << 4) + z;
                                if (section.getBlockState(x, y, z).is(was) && Hex.at(wx + 0.5, wz + 0.5).equals(hex)) {
                                    level.setBlock(new net.minecraft.core.BlockPos(wx, worldY, wz),
                                            now.defaultBlockState(), 3);
                                    if (++turned >= max) {
                                        return turned;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        return turned;
    }

    private static void countSection(
            LevelChunkSection section, int cx, int sy, int cz, Hex hex, int centerY, Map<Item, Integer> plots) {
        for (int y = 0; y < 16; y++) {
            int worldY = (sy << 4) + y;
            if (Math.abs(worldY - centerY) > HEIGHT) {
                continue;
            }
            for (int z = 0; z < 16; z++) {
                for (int x = 0; x < 16; x++) {
                    Item yield = YIELDS.get(section.getBlockState(x, y, z).getBlock());
                    if (yield != null && Hex.at((cx << 4) + x + 0.5, (cz << 4) + z + 0.5).equals(hex)) {
                        plots.merge(yield, 1, Integer::sum);
                    }
                }
            }
        }
    }
}
