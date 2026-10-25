package dev.b28.enterprise.territory;

import java.util.List;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.DoorBlock;
import net.minecraft.world.level.block.LadderBlock;
import net.minecraft.world.level.block.SlabBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.DoubleBlockHalf;
import net.minecraft.world.level.block.state.properties.SlabType;

/**
 * What a town's Mason can build for it, beyond walls and bridges: a public square, and halls for its trades. Each
 * needs a Mason of so much skill at home, and most a master of the trade it serves (a temple a Cleric, a library a
 * Librarian); each costs its materials up front (logs in the warehouse stand in for some of the timber) and the
 * masons' wages while they build. What each does is read where it matters (mood, culture, the catch, the mine).
 */
public final class Buildings {
    /**
     * A kind of building: its name, its footprint (square) and height, the Mason's level it needs, the trades it
     * serves (one of them at {@code partnerLevel} must live in the town; none for a square), its materials in
     * emeralds, and what it does.
     */
    public record Kind(String id, int size, int height, int masonLevel, Set<String> partners, int partnerLevel,
            double emeralds, boolean everyPartner, String does) {
        /** A hall for one of its trades: any one of them at home will do. */
        public Kind(String id, int size, int height, int masonLevel, Set<String> partners, int partnerLevel,
                double emeralds, String does) {
            this(id, size, height, masonLevel, partners, partnerLevel, emeralds, false, does);
        }

        public int blocks() {
            return size * size * height;
        }

        /**
         * Does the town have the trades it serves, at the level: one of them, or (a hall two trades share) every one?
         * {@code skills} are the residents' best levels by trade; {@code merchant}, the town Merchant's level.
         */
        public boolean partnered(java.util.Map<String, Integer> skills, int merchant) {
            java.util.function.Predicate<String> has = trade -> trade.equals("enterprise:merchant")
                    ? merchant >= partnerLevel : skills.getOrDefault(trade, 0) >= partnerLevel;
            return partners.isEmpty() || (everyPartner ? partners.stream().allMatch(has) : partners.stream().anyMatch(has));
        }
    }

    public static final Kind SQUARE = new Kind("square", 9, 1, 2, Set.of(), 0, 8,
            "a paved square by the bell, where the town gathers: happier");
    /** Wheat a granary holds, as hay bales: 64 stacks of bales. */
    public static final int GRANARY_WHEAT = 64 * 64 * 9;
    public static final Kind GRANARY = new Kind("granary", 7, 4, 2, Set.of("minecraft:farmer"), 2, 12,
            "banks the town's wheat as hay bales, up to " + GRANARY_WHEAT + ": what the warehouse can't hold, drawn "
                    + "on as the town wants it");
    /** Roots a root cellar keeps: sixteen barrels of them. */
    public static final int ROOT_CELLAR_ROOTS = 16 * 27 * 64;
    public static final Kind ROOT_CELLAR = new Kind("root_cellar", 5, 3, 2, Set.of("minecraft:farmer"), 2, 8,
            "keeps the town's carrots, potatoes and beetroot cool in barrels, up to " + ROOT_CELLAR_ROOTS
                    + ": what the warehouse can't hold, drawn on as the town wants them");

    /**
     * A hall that banks a crop: what it keeps and how much in all. A town builds one only where that crop piles up
     * (see {@link Territory#wantsStore}); the warehouse keeps its par loose, and the store tops it back up.
     */
    public record Store(Kind kind, Set<Item> items, int holds) {}

    public static final List<Store> STORES = List.of(new Store(GRANARY, Set.of(Items.WHEAT), GRANARY_WHEAT),
            new Store(ROOT_CELLAR, Set.of(Items.CARROT, Items.POTATO, Items.BEETROOT), ROOT_CELLAR_ROOTS));

    public static Optional<Store> store(Kind kind) {
        return STORES.stream().filter(s -> s.kind() == kind).findFirst();
    }

    /**
     * A counting house: the Merchant's books and the Librarian's clerks under one roof, where a busy market's paper is
     * drawn up, kept and discounted. Built where the papers pile up (see {@link #countingWanted}).
     */
    public static final Kind COUNTING_HOUSE = new Kind("counting_house", 7, 4, 3,
            Set.of("minecraft:librarian", "enterprise:merchant"), 3, 16, true,
            "its clerks draw up its papers on its own forms (half the stationery, a fifth of a scribe's fee), and it "
                    + "carries twice the paper on its discount desk, any day, at half the fee");

    public static final Kind TEMPLE = new Kind("temple", 7, 4, 3, Set.of("minecraft:cleric"), 3, 16,
            "its Clerics' blessing counts double, and children come twice as often");
    public static final Kind LIBRARY = new Kind("library", 7, 4, 3, Set.of("minecraft:librarian"), 3, 16,
            "3 culture a day");
    public static final Kind SMITHY = new Kind("smithy", 7, 4, 3,
            Set.of("minecraft:toolsmith", "minecraft:armorer", "minecraft:weaponsmith"), 3, 12,
            "its mine yields half again, and its kit lasts a quarter longer");
    public static final Kind WATCHTOWER = new Kind("watchtower", 3, 7, 3, Set.of("minecraft:fletcher"), 3, 8,
            "archers on the tower: half its guards count as armed");
    public static final Kind STONE_DOCKS = new Kind("stone_docks", 3, 1, 4, Set.of("minecraft:fisherman"), 3, 12,
            "a stone pier: its Fishermen land twice the catch");
    public static final Kind CARAVANSERAI = new Kind("caravanserai", 9, 4, 5, Set.of("enterprise:merchant"), 3, 24,
            "a waystation: markets within 800 blocks hear of the town, and 1 culture a day");

    /** In the order a town builds them. */
    public static final List<Kind> KINDS = List.of(SQUARE, GRANARY, ROOT_CELLAR, COUNTING_HOUSE, TEMPLE, LIBRARY, SMITHY, WATCHTOWER, STONE_DOCKS, CARAVANSERAI);

    /** A square brings this much mood; with a Farmer's flower beds, this much more. */
    public static final double SQUARE_MOOD = 0.5;
    public static final double FLOWER_MOOD = 0.5;
    public static final double LIBRARY_CULTURE_PER_DAY = 3;
    public static final double CARAVANSERAI_CULTURE_PER_DAY = 1;
    public static final int CARAVANSERAI_REACH = 800;

    /** Each log in the warehouse saves this much of a building's materials, in emeralds. */
    public static final double EMERALDS_PER_LOG = 0.125;

    private Buildings() {}

    /** Has the town whose market is at {@code desk} built this? */
    public static boolean has(net.minecraft.server.MinecraftServer server, net.minecraft.core.GlobalPos desk, Kind kind) {
        return Territory.get(server).bellOf(desk).map(bell -> Works.get(server).builtOf(bell).contains(kind.id()))
                .orElse(false);
    }

    /** A counting house only where the market draws up papers enough to keep clerks busy. */
    public static boolean countingWanted(net.minecraft.server.MinecraftServer server, net.minecraft.core.GlobalPos desk,
            long now) {
        return dev.b28.enterprise.cargo.Paperwork.get(server).perDay(desk, now)
                >= dev.b28.enterprise.Dials.COUNTING_DOCUMENTS.get();
    }

    public static Optional<Kind> kind(String id) {
        return KINDS.stream().filter(k -> k.id().equals(id)).findFirst();
    }

    /** Where it goes: the nearest open ground near the bell (a square close by); stone docks on the town's pier. */
    public static Optional<BlockPos> site(ServerLevel level, BlockPos bell, Kind kind, Optional<BlockPos> dock) {
        if (kind == STONE_DOCKS) {
            return dock;
        }
        return Growth.site(level, bell, kind.size(), kind == SQUARE ? 4 : 8);
    }

    /** Lays the building with its corner (ground level, lowest x and z) at {@code corner}. */
    public static void build(ServerLevel level, Kind kind, BlockPos corner) {
        if (kind == STONE_DOCKS) {
            paveDock(level, corner);
            return;
        }
        Growth.level(level, corner, kind.size());
        if (kind == WATCHTOWER) {
            tower(level, corner, kind);
            return;
        }
        boolean timber = kind == CARAVANSERAI || kind == GRANARY;
        Block floor = timber ? Blocks.SPRUCE_PLANKS : kind == ROOT_CELLAR ? Blocks.PACKED_MUD : Blocks.STONE_BRICKS;
        Block wall = timber ? Blocks.OAK_PLANKS : kind == ROOT_CELLAR ? Blocks.COBBLESTONE : Blocks.STONE_BRICKS;
        int n = kind.size();
        for (int dx = 0; dx < n; dx++) {
            for (int dz = 0; dz < n; dz++) {
                BlockPos ground = corner.offset(dx, 0, dz);
                level.setBlock(ground, floor.defaultBlockState(), 3);
                if (kind.height() <= 1) {
                    continue;
                }
                boolean edge = dx == 0 || dz == 0 || dx == n - 1 || dz == n - 1;
                boolean door = dz == n - 1 && dx == n / 2;
                for (int up = 1; up < kind.height(); up++) {
                    BlockState state = !edge || door && up <= 2 ? Blocks.AIR.defaultBlockState()
                            : (dx == 0 || dx == n - 1) && (dz == 0 || dz == n - 1) && kind == CARAVANSERAI
                                    ? Blocks.OAK_LOG.defaultBlockState()
                                    : up == 2 && (dx == n / 2 || dz == n / 2) && !door
                                            ? Blocks.GLASS_PANE.defaultBlockState() : wall.defaultBlockState();
                    level.setBlock(ground.above(up), state, 3);
                }
                level.setBlock(ground.above(kind.height()),
                        Blocks.STONE_BRICK_SLAB.defaultBlockState().setValue(SlabBlock.TYPE, SlabType.BOTTOM), 3);
            }
        }
        if (kind == SQUARE) {
            for (BlockPos post : List.of(corner, corner.offset(n - 1, 0, 0), corner.offset(0, 0, n - 1),
                    corner.offset(n - 1, 0, n - 1))) {
                level.setBlock(post.above(), Blocks.OAK_FENCE.defaultBlockState(), 3);
                level.setBlock(post.above(2), Blocks.LANTERN.defaultBlockState(), 3);
            }
            return;
        }
        BlockPos door = corner.offset(n / 2, 1, n - 1);
        BlockState doorState = Blocks.OAK_DOOR.defaultBlockState().setValue(DoorBlock.FACING, Direction.SOUTH);
        level.setBlock(door, doorState.setValue(DoorBlock.HALF, DoubleBlockHalf.LOWER), 3);
        level.setBlock(door.above(), doorState.setValue(DoorBlock.HALF, DoubleBlockHalf.UPPER), 3);
        BlockPos middle = corner.offset(n / 2, 1, n / 2);
        level.setBlock(middle.above(kind.height() - 2), Blocks.LANTERN.defaultBlockState(), 3);
        // What's inside says what it is.
        if (kind == TEMPLE) {
            level.setBlock(middle.north(), Blocks.BREWING_STAND.defaultBlockState(), 3);
            level.setBlock(middle.north().west(), Blocks.CANDLE.defaultBlockState(), 3);
            level.setBlock(middle.north().east(), Blocks.CANDLE.defaultBlockState(), 3);
        } else if (kind == LIBRARY) {
            level.setBlock(middle, Blocks.LECTERN.defaultBlockState(), 3);
            for (int dx = 1; dx < n - 1; dx++) {
                level.setBlock(corner.offset(dx, 1, 1), Blocks.BOOKSHELF.defaultBlockState(), 3);
            }
        } else if (kind == SMITHY) {
            level.setBlock(middle.north().west(), Blocks.SMITHING_TABLE.defaultBlockState(), 3);
            level.setBlock(middle.north(), Blocks.BLAST_FURNACE.defaultBlockState(), 3);
            level.setBlock(middle.north().east(), Blocks.ANVIL.defaultBlockState(), 3);
        } else if (kind == GRANARY) {
            for (int dx = 1; dx < n - 1; dx++) {
                level.setBlock(corner.offset(dx, 1, 1), Blocks.HAY_BLOCK.defaultBlockState(), 3);
                level.setBlock(corner.offset(dx, 2, 1), Blocks.HAY_BLOCK.defaultBlockState(), 3);
            }
        } else if (kind == ROOT_CELLAR) {
            for (int dx = 1; dx < n - 1; dx++) {
                level.setBlock(corner.offset(dx, 1, 1), Blocks.BARREL.defaultBlockState(), 3);
            }
            level.setBlock(corner.offset(1, 1, 2), Blocks.BARREL.defaultBlockState(), 3);
            level.setBlock(corner.offset(n - 2, 1, 2), Blocks.BARREL.defaultBlockState(), 3);
        } else if (kind == COUNTING_HOUSE) {
            // The clerks' lectern, the books along the back wall, the files in barrels, the strongbox.
            level.setBlock(middle, Blocks.LECTERN.defaultBlockState(), 3);
            for (int dx = 1; dx < n - 1; dx++) {
                level.setBlock(corner.offset(dx, 1, 1), dx % 2 == 0 ? Blocks.BOOKSHELF.defaultBlockState()
                        : Blocks.BARREL.defaultBlockState(), 3);
            }
            level.setBlock(middle.east(2), Blocks.CHEST.defaultBlockState(), 3);
        } else if (kind == CARAVANSERAI) {
            level.setBlock(middle.north(), Blocks.HAY_BLOCK.defaultBlockState(), 3);
            level.setBlock(middle.north().west(), Blocks.BARREL.defaultBlockState(), 3);
            level.setBlock(middle.north().east(), Blocks.HAY_BLOCK.defaultBlockState(), 3);
        }
    }

    /** A 3 x 3 tower of stone, a ladder up the inside, a fenced platform on top. */
    private static void tower(ServerLevel level, BlockPos corner, Kind kind) {
        int top = kind.height();
        for (int dx = 0; dx < 3; dx++) {
            for (int dz = 0; dz < 3; dz++) {
                BlockPos ground = corner.offset(dx, 0, dz);
                boolean inside = dx == 1 && dz == 1;
                for (int up = 0; up < top; up++) {
                    level.setBlock(ground.above(up), inside && up > 0 ? Blocks.AIR.defaultBlockState()
                            : Blocks.STONE_BRICKS.defaultBlockState(), 3);
                }
                level.setBlock(ground.above(top), inside ? Blocks.AIR.defaultBlockState()
                        : Blocks.OAK_FENCE.defaultBlockState(), 3);
            }
        }
        for (int up = 1; up < top; up++) {
            level.setBlock(corner.offset(1, up, 1),
                    Blocks.LADDER.defaultBlockState().setValue(LadderBlock.FACING, Direction.SOUTH), 3);
        }
        level.setBlock(corner.offset(1, 1, 2), Blocks.AIR.defaultBlockState(), 3); // the way in
        level.setBlock(corner.offset(1, 2, 2), Blocks.AIR.defaultBlockState(), 3);
    }

    /** The town's wooden pier, relaid in stone brick with a wall along it. */
    private static void paveDock(ServerLevel level, BlockPos start) {
        for (int dx = -Docks.PIER; dx <= Docks.PIER; dx++) {
            for (int dz = -Docks.PIER; dz <= Docks.PIER; dz++) {
                BlockPos at = start.offset(dx, 0, dz);
                if (level.getBlockState(at).is(Blocks.SPRUCE_PLANKS)) {
                    level.setBlock(at, Blocks.STONE_BRICKS.defaultBlockState(), 3);
                }
                if (level.getBlockState(at.above()).is(Blocks.SPRUCE_FENCE)) {
                    level.setBlock(at.above(), Blocks.STONE_BRICK_WALL.defaultBlockState(), 3);
                }
            }
        }
    }
}
