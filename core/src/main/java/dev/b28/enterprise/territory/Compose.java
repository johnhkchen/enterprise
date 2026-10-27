package dev.b28.enterprise.territory;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtAccounter;
import net.minecraft.nbt.NbtIo;
import net.minecraft.nbt.NbtUtils;
import net.minecraft.nbt.Tag;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Mirror;
import net.minecraft.world.level.block.Rotation;
import net.minecraft.world.level.block.state.BlockState;

/**
 * A house composed from a module set's parts (the studio's `timber_relief`, thread 0004; its reference composer is
 * scripts/assets/lordaeron-ref/compose.mjs, which this follows step for step): a layout (its body, storeys, party walls,
 * roof, stair core, the shell's modules by face and place, the plan's cores and fit-out) laid out as blocks. Each part
 * is a structure file of the set (`structure/<kingdom>/part/<set>/<id>.nbt`), pasted mirrored east-west first if asked,
 * then turned a quarter at a time clockwise about its own footprint, as the reference's `Grid.paste` does.
 *
 * <p>The blocks come out as a map from the house's own positions (as a template's: its north-west corner at its lowest
 * layer, its front to the south) to states, to be checked against a golden house or laid in the world.
 */
public final class Compose {
    private Compose() {}

    /** A storey's height, floor included (the set's grid). */
    static final int H = 5;
    static final int PITCH = 2;

    /** A part: its size, and its blocks by position (air included: it clears). */
    record Part(int w, int h, int d, Map<BlockPos, BlockState> cells) {}

    private static final Map<String, Optional<Part>> PARTS = new java.util.concurrent.ConcurrentHashMap<>();

    /** The part {@code id} of a set ({@code enterprise:lordaeron/part/timber_relief/post_ground}), read from the data packs. */
    static Optional<Part> part(ResourceManager resources, HolderGetter<Block> blocks, String id) {
        Optional<Part> known = PARTS.get(id);
        if (known != null) {
            return known;
        }
        ResourceLocation t = ResourceLocation.parse(id);
        ResourceLocation file = ResourceLocation.fromNamespaceAndPath(t.getNamespace(), "structure/" + t.getPath() + ".nbt");
        Optional<Part> read = resources.getResource(file).map(r -> {
            try (InputStream in = r.open()) {
                return read(NbtIo.readCompressed(in, NbtAccounter.unlimitedHeap()), blocks);
            } catch (Exception e) {
                throw new IllegalStateException("part " + id + ": " + e, e);
            }
        });
        PARTS.put(id, read);
        return read;
    }

    /** A structure file's blocks. */
    static Part read(CompoundTag tag, HolderGetter<Block> blocks) {
        ListTag size = tag.getList("size", Tag.TAG_INT);
        ListTag palette = tag.contains("palette") ? tag.getList("palette", Tag.TAG_COMPOUND)
                : tag.getList("palettes", Tag.TAG_LIST).getList(0);
        List<BlockState> states = new ArrayList<>();
        for (int i = 0; i < palette.size(); i++) {
            states.add(NbtUtils.readBlockState(blocks, palette.getCompound(i)));
        }
        Map<BlockPos, BlockState> cells = new LinkedHashMap<>();
        ListTag list = tag.getList("blocks", Tag.TAG_COMPOUND);
        for (int i = 0; i < list.size(); i++) {
            CompoundTag b = list.getCompound(i);
            ListTag pos = b.getList("pos", Tag.TAG_INT);
            cells.put(new BlockPos(pos.getInt(0), pos.getInt(1), pos.getInt(2)), states.get(b.getInt("state")));
        }
        return new Part(size.getInt(0), size.getInt(1), size.getInt(2), cells);
    }

    /** A whole structure's blocks by position (a golden house to check a composition against). */
    public static Map<BlockPos, BlockState> golden(ResourceManager resources, HolderGetter<Block> blocks, String id) {
        return part(resources, blocks, id).map(Part::cells).orElseThrow(() -> new IllegalStateException("no " + id));
    }

    /** A block state written as the game writes one ({@code minecraft:lectern[facing=south]}). */
    static BlockState parse(HolderGetter<Block> blocks, String state) {
        try {
            return net.minecraft.commands.arguments.blocks.BlockStateParser.parseForBlock(
                    net.minecraft.core.registries.BuiltInRegistries.BLOCK.asLookup(), state, false).blockState();
        } catch (com.mojang.brigadier.exceptions.CommandSyntaxException e) {
            throw new IllegalArgumentException("not a block state: " + state, e);
        }
    }

    /** Forgets the parts read, for the server's resources reloading. */
    public static void forget() {
        PARTS.clear();
    }

    /** The blocks being composed, by position, and the house's size. */
    public static final class Grid {
        public final Map<BlockPos, BlockState> cells = new HashMap<>();
        public final int sx, sy, sz;

        Grid(int sx, int sy, int sz) {
            this.sx = sx;
            this.sy = sy;
            this.sz = sz;
        }

        /** A part pasted at (ox, oy, oz): mirrored east-west first if asked, then turned q quarters clockwise. */
        void paste(Part p, int ox, int oy, int oz, int q, boolean mirror) {
            int turns = Math.floorMod(q, 4);
            Rotation rotation = switch (turns) {
                case 1 -> Rotation.CLOCKWISE_90;
                case 2 -> Rotation.CLOCKWISE_180;
                case 3 -> Rotation.COUNTERCLOCKWISE_90;
                default -> Rotation.NONE;
            };
            for (var e : p.cells().entrySet()) {
                int x = e.getKey().getX(), y = e.getKey().getY(), z = e.getKey().getZ();
                BlockState s = e.getValue();
                if (mirror) {
                    x = p.w() - 1 - x;
                    s = s.mirror(Mirror.FRONT_BACK);
                }
                int px = x, pz = z;
                for (int i = 0; i < turns; i++) {
                    int nx = (i % 2 == 1 ? p.w() : p.d()) - 1 - pz;
                    pz = px;
                    px = nx;
                }
                cells.put(new BlockPos(ox + px, oy + y, oz + pz), s.rotate(rotation));
            }
        }
    }

    /** A layout, as the set's `golden.layouts[].layout` or the generator's (see compose.mjs). */
    public record Layout(int w, int d, int storeys, boolean partyWest, boolean partyEast, String roof, JsonArray shell,
            JsonArray plan, int[] chimney) {
        public static Layout of(JsonObject o) {
            JsonArray body = o.getAsJsonArray("body");
            JsonArray party = o.has("party") ? o.getAsJsonArray("party") : new JsonArray();
            int[] chimney = null;
            if (o.has("chimney") && o.get("chimney").isJsonArray()) {
                JsonArray c = o.getAsJsonArray("chimney");
                chimney = new int[] {c.get(0).getAsInt(), c.get(1).getAsInt()};
            }
            return new Layout(body.get(0).getAsInt(), body.get(1).getAsInt(), o.get("storeys").getAsInt(),
                    party.size() > 0 && party.get(0).getAsBoolean(), party.size() > 1 && party.get(1).getAsBoolean(),
                    o.get("roof").getAsString(), o.getAsJsonArray("shell"),
                    o.has("plan") ? o.getAsJsonArray("plan") : new JsonArray(), chimney);
        }
    }

    static int roofLayers(int span) {
        return (span - 1) / 2 * PITCH + 1;
    }

    /** Composes a layout from the set's parts ({@code set}: {@code enterprise:lordaeron/part/timber_relief}). */
    public static Grid compose(ResourceManager resources, HolderGetter<Block> blocks, String set, Layout l) {
        int W = l.w(), D = l.d(), S = l.storeys();
        int rw = l.partyWest() ? 0 : 1, re = l.partyEast() ? 0 : 1, roofBase = 2 + H * S;
        int span = l.roof().equals("gable_x") ? D + 2 : W + rw + re, layers = roofLayers(span);
        Grid g = new Grid(W + rw + re, roofBase + layers + 1, D + 2);
        java.util.function.Function<String, Part> mod = id -> part(resources, blocks, set + "/" + id)
                .orElseThrow(() -> new IllegalStateException("no part " + set + "/" + id));
        // (x, z) in the body's frame: the template's is shifted by the relief.
        interface Paste {
            void at(String id, int x, int y, int z, int q, boolean mirror);
        }
        Paste paste = (id, x, y, z, q, mirror) -> g.paste(mod.apply(id), x + rw, y, z + 1, q, mirror);
        // Foundation, floors, room air.
        for (int x = -rw; x < W + re; x++) {
            for (int z = -1; z <= D; z++) {
                paste.at("foundation", x, 0, z, 0, false);
            }
        }
        for (int s = 0; s < S; s++) {
            int gy = 2 + H * s;
            for (int x = 1; x < W - 1; x++) {
                for (int z = 1; z < D - 1; z++) {
                    paste.at("floor", x, gy, z, 0, false);
                    paste.at("room_air", x, gy + 1, z, 0, false);
                }
            }
        }
        // The shell: each module on its face, a module w wide at place i counted from the face's outside-left.
        for (JsonElement e : l.shell()) {
            JsonObject s = e.getAsJsonObject();
            String id = s.get("module").getAsString(), face = s.get("face").getAsString();
            int i = s.get("i").getAsInt(), storey = s.get("storey").getAsInt();
            boolean mirror = s.has("mirror") && s.get("mirror").getAsBoolean();
            Part m = mod.apply(id);
            int w = m.w(), gy = 2 + H * storey;
            int[] a = switch (face) {
                case "front" -> new int[] {i, D - 1, 0};
                case "back" -> new int[] {W - i - w, -1, 2};
                case "east" -> new int[] {W - 1, D - i - w, 3};
                default -> new int[] {-1, i, 1};
            };
            if (m.d() == 1) { // one layer: on the wall line
                if (face.equals("west")) {
                    a[0] = 0;
                }
                if (face.equals("back")) {
                    a[1] = 0;
                }
            }
            if (id.startsWith("corner_party")) {
                a = face.equals("front") ? new int[] {i, D - 1, 0} : new int[] {W - 1 - i, -1, 2};
            } else if (id.startsWith("corner")) {
                String key = face + ":" + i;
                if (key.equals("front:0")) {
                    a = new int[] {-1, D - 1, 0};
                } else if (key.equals("front:" + (W - 1))) {
                    a = new int[] {W - 1, D - 1, 3};
                } else if (key.equals("back:0")) {
                    a = new int[] {W - 1, -1, 2};
                } else if (key.equals("back:" + (W - 1))) {
                    a = new int[] {-1, -1, 1};
                }
            }
            paste.at(id, a[0], gy, a[1], a[2], mirror);
        }
        // The plan: cores and fit-outs (after the floors, so a core's holes win).
        for (JsonElement e : l.plan()) {
            JsonObject p = e.getAsJsonObject();
            JsonArray at = p.getAsJsonArray("at");
            int dy = p.has("dy") ? p.get("dy").getAsInt() : 0, q = p.has("q") ? p.get("q").getAsInt() : 0;
            int y = 2 + H * p.get("storey").getAsInt() + dy;
            // An exact block in place of a module (a declared vanilla workstation; "any" is an air marker).
            if (p.has("block")) {
                String state = p.get("block").getAsString();
                BlockState block = state.equals("any") ? net.minecraft.world.level.block.Blocks.AIR.defaultBlockState()
                        : parse(blocks, state);
                g.cells.put(new BlockPos(at.get(0).getAsInt() + rw, y, at.get(1).getAsInt() + 1), block);
                continue;
            }
            paste.at(p.get("module").getAsString(), at.get(0).getAsInt(), y, at.get(1).getAsInt(), q, false);
        }
        // The roof's slices over the relief.
        if (l.roof().equals("gable_x")) {
            for (int x = -rw; x < W + re; x++) {
                String kind = x < 0 || x >= W ? "verge" : x == 0 && !l.partyWest() || x == W - 1 && !l.partyEast() ? "gable" : "bay";
                paste.at("roof_" + kind + "_" + span, x, roofBase, -1, 0, x >= W); // the far verge mirrored: its boards face the gable
            }
        } else {
            for (int z = -1; z <= D; z++) {
                String kind = z < 0 || z >= D ? "verge" : z == 0 || z == D - 1 ? "gable" : "bay";
                g.paste(mod.apply("roof_" + kind + "_" + span), 0, roofBase, z + 1, 1, z >= D);
            }
        }
        // The chimney's stack over its breast, one above the ridge.
        if (l.chimney() != null) {
            int cx = l.chimney()[0], cz = l.chimney()[1], top = roofBase + layers;
            for (int y = roofBase; y <= top; y++) {
                if (l.partyWest() && cx == 0) {
                    paste.at("chimney_stack_party", 0, y, cz, 0, false);
                } else {
                    g.paste(mod.apply("chimney_stack"), cx + rw - 1, y, cz + 1, 1, false);
                }
            }
        }
        return g;
    }

    /**
     * Several layouts composed together, each at its offset ({@code golden.layouts}: a terrace's units side by side), in
     * order: a later unit's blocks over an earlier's where they meet (their party walls).
     */
    public static Map<BlockPos, BlockState> composeAll(ResourceManager resources, HolderGetter<Block> blocks, String set,
            JsonArray layouts) {
        Map<BlockPos, BlockState> out = new HashMap<>();
        for (JsonElement e : layouts) {
            JsonObject o = e.getAsJsonObject();
            JsonArray at = o.has("at") ? o.getAsJsonArray("at") : null;
            int ax = at == null ? 0 : at.get(0).getAsInt(), ay = at == null ? 0 : at.get(1).getAsInt(),
                    az = at == null ? 0 : at.get(2).getAsInt();
            Grid g = compose(resources, blocks, set, Layout.of(o.getAsJsonObject("layout")));
            g.cells.forEach((p, s) -> out.put(p.offset(ax, ay, az), s));
        }
        return out;
    }

    /** Blocks as a vanilla structure file (DataVersion of this game), its size {@code sx} x {@code sy} x {@code sz}. */
    public static CompoundTag toNbt(Map<BlockPos, BlockState> cells, int sx, int sy, int sz) {
        CompoundTag tag = new CompoundTag();
        tag.putInt("DataVersion", net.minecraft.SharedConstants.getCurrentVersion().getDataVersion().getVersion());
        ListTag size = new ListTag();
        for (int v : new int[] {sx, sy, sz}) {
            size.add(net.minecraft.nbt.IntTag.valueOf(v));
        }
        tag.put("size", size);
        Map<BlockState, Integer> index = new LinkedHashMap<>();
        ListTag palette = new ListTag(), list = new ListTag();
        cells.entrySet().stream().sorted(java.util.Map.Entry.comparingByKey()).forEach(e -> {
            int i = index.computeIfAbsent(e.getValue(), st -> {
                palette.add(NbtUtils.writeBlockState(st));
                return index.size();
            });
            CompoundTag b = new CompoundTag();
            ListTag pos = new ListTag();
            pos.add(net.minecraft.nbt.IntTag.valueOf(e.getKey().getX()));
            pos.add(net.minecraft.nbt.IntTag.valueOf(e.getKey().getY()));
            pos.add(net.minecraft.nbt.IntTag.valueOf(e.getKey().getZ()));
            b.put("pos", pos);
            b.putInt("state", i);
            list.add(b);
        });
        tag.put("palette", palette);
        tag.put("blocks", list);
        tag.put("entities", new ListTag());
        return tag;
    }

    /** The block registry's lookup, for reading parts' states. */
    public static HolderGetter<Block> blocks(ServerLevel level) {
        return level.holderLookup(Registries.BLOCK);
    }
}
