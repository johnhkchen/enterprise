package dev.b28.enterprise.territory;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.Reader;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.tags.BlockTags;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.Mirror;
import net.minecraft.world.level.block.Rotation;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.structure.BoundingBox;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructurePlaceSettings;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplate;

/**
 * The placer for the asset studio's structures (`docs/assets/README.md`, C1): a template from
 * {@code data/<ns>/structure/<path>.nbt} and its metadata beside it ({@code <path>.json}), authored facing south with its
 * origin at the north-west corner of its lowest layer and its {@code ground} layer at the ground's top, laid in the
 * world turned to face its street: the ground under it filled up to its foundation, the trees over it cleared, and the
 * beds and workstations its metadata declares found where it said (so the census counts what the sim paid for).
 *
 * <p>A kit is chosen for a town's kingdom and a slot ({@link #choose}): the same town and slot always take the same one.
 * Where none fits, the callers lay what they always have ({@link Growth#buildHome}).
 */
public final class Kits {
    private Kits() {}

    /** A structure's metadata: what the placer needs of it (positions in its own frame, facing south). */
    public record Meta(String id, String kingdom, String kind, String slot, int width, int depth, int height, int ground,
            List<BlockPos> bedHeads, List<BlockPos> workstations, List<String> tiers, List<BlockPos> entrances, int plots) {

        /** Is it a lot whole (`plots` of 2 or more: Dalaran's courtyard house, thread 0010), its yards its own? */
        public boolean whole() {
            return plots > 1;
        }
    }

    /** What was laid: its world box, and its beds' heads and workstations where they stand now. */
    public record Placed(String id, BoundingBox box, List<BlockPos> bedHeads, List<BlockPos> workstations) {}

    private static final Map<String, Optional<Meta>> METAS = new ConcurrentHashMap<>();

    /** The structure's id as a template's resource location: {@code enterprise:lordaeron/home/cottage_9x9_b}. */
    static ResourceLocation template(String id) {
        return ResourceLocation.parse(id);
    }

    /** Its metadata, read from the data packs (kept until the server's resources reload). */
    public static Optional<Meta> meta(ResourceManager resources, String id) {
        // (Not computeIfAbsent: a variant reads its base's on the way, and the map can't be updated inside its own update.)
        Optional<Meta> known = METAS.get(id);
        if (known != null) {
            return known;
        }
        Optional<Meta> read = read(resources, id);
        METAS.put(id, read);
        return read;
    }

    private static Optional<Meta> read(ResourceManager resources, String id) {
        ResourceLocation t = template(id);
        ResourceLocation json = ResourceLocation.fromNamespaceAndPath(t.getNamespace(), "structure/" + t.getPath() + ".json");
        return resources.getResource(json).flatMap(r -> {
            try (Reader reader = r.openAsReader()) {
                JsonObject o = JsonParser.parseReader(reader).getAsJsonObject();
                // A palette variant ({"id", "base", "palette"}): everything but its blocks is its base's.
                if (o.has("base") && !o.has("footprint")) {
                    return meta(resources, o.get("base").getAsString()).map(b -> new Meta(id, b.kingdom(), b.kind(),
                            b.slot(), b.width(), b.depth(), b.height(), b.ground(), b.bedHeads(), b.workstations(),
                            b.tiers(), b.entrances(), b.plots()));
                }
                return Optional.of(parse(id, o));
            } catch (Exception e) {
                dev.b28.enterprise.Enterprise.LOGGER.warn("Kit {}: its metadata can't be read: {}", id, e.toString());
                return Optional.empty();
            }
        });
    }

    /** Forgets the metadata read, for the server's resources reloading. */
    public static void forget() {
        METAS.clear();
    }

    static Meta parse(String id, JsonObject o) {
        // [w, d]; or {at, size}: what stands on the ground, inside a template its overhang wider every side (Dalaran's
        // eaves, thread 0010): the template whole is what's laid.
        JsonArray fp;
        if (o.get("footprint").isJsonObject()) {
            JsonObject f = o.getAsJsonObject("footprint");
            JsonArray at = f.getAsJsonArray("at"), size = f.getAsJsonArray("size");
            fp = new JsonArray();
            fp.add(size.get(0).getAsInt() + 2 * at.get(0).getAsInt());
            fp.add(size.get(1).getAsInt() + 2 * at.get(1).getAsInt());
        } else {
            fp = o.getAsJsonArray("footprint");
        }
        List<BlockPos> beds = new ArrayList<>();
        for (JsonElement b : array(o, "beds")) {
            beds.add(pos(b.getAsJsonObject().getAsJsonArray("head")));
        }
        List<BlockPos> work = new ArrayList<>();
        for (JsonElement w : array(o, "workstations")) {
            work.add(pos(w.getAsJsonObject().getAsJsonArray("at")));
        }
        List<BlockPos> doors = new ArrayList<>();
        for (JsonElement e : array(o, "entrances")) {
            doors.add(pos(e.getAsJsonObject().getAsJsonArray("at")));
        }
        List<String> tiers = new ArrayList<>();
        for (JsonElement t : array(o, "tiers")) {
            tiers.add(t.getAsString());
        }
        return new Meta(id, string(o, "kingdom"), string(o, "kind"), string(o, "slot"), fp.get(0).getAsInt(),
                fp.get(1).getAsInt(), o.has("height") ? o.get("height").getAsInt() : 8,
                o.has("ground") ? o.get("ground").getAsInt() : 0, beds, work, tiers, doors,
                o.has("plots") ? o.get("plots").getAsInt() : 1);
    }

    private static JsonArray array(JsonObject o, String key) {
        return o.has(key) && o.get(key).isJsonArray() ? o.getAsJsonArray(key) : new JsonArray();
    }

    private static String string(JsonObject o, String key) {
        return o.has(key) ? o.get(key).getAsString() : "";
    }

    private static BlockPos pos(JsonArray a) {
        return new BlockPos(a.get(0).getAsInt(), a.get(1).getAsInt(), a.get(2).getAsInt());
    }

    /** The turn that brings a south-facing structure to face {@code front} (as {@link Growth#buildHome} turns a cottage). */
    static Rotation turn(Direction front) {
        return switch (front) {
            case WEST -> Rotation.CLOCKWISE_90;
            case NORTH -> Rotation.CLOCKWISE_180;
            case EAST -> Rotation.COUNTERCLOCKWISE_90;
            default -> Rotation.NONE;
        };
    }

    /**
     * The kits of a kingdom for a slot whose footprint fits {@code width} x {@code depth} (as authored, facing south),
     * by id; empty if the kingdom has none.
     */
    public static List<String> kits(ResourceManager resources, String kingdom, String slot, int width, int depth) {
        List<String> out = new ArrayList<>();
        // (Not its parts: pieces of buildings, chosen by the assembler, not by slot.)
        resources.listResources("structure/" + kingdom, rl -> rl.getPath().endsWith(".json") && !rl.getPath().contains("/part/")
                        && !rl.getPath().contains("/yard/") && !rl.getPath().contains("/earthworks/")
                        && !rl.getPath().contains("/field/"))
                .keySet().stream()
                .sorted(Comparator.comparing(ResourceLocation::toString))
                .forEach(rl -> {
                    String path = rl.getPath().substring("structure/".length(), rl.getPath().length() - ".json".length());
                    String id = rl.getNamespace() + ":" + path;
                    meta(resources, id).filter(m -> m.slot().equals(slot) && m.width() <= width && m.depth() <= depth)
                            .ifPresent(m -> out.add(id));
                });
        return out;
    }

    /**
     * A kingdom's homes that are a lot whole ({@link Meta#whole}) within {@code width} x {@code depth}, by id: the one
     * {@code n} picks (the town's how-manieth), empty if it has none.
     */
    public static Optional<Meta> wholeHome(ResourceManager resources, String kingdom, int width, int depth, long town, int n) {
        List<Meta> fit = kits(resources, kingdom, "", width, depth).stream().flatMap(id -> meta(resources, id).stream())
                .filter(m -> m.kind().equals("home") && m.whole()).toList();
        return fit.isEmpty() ? Optional.empty() : Optional.of(fit.get((int) Math.floorMod(town * 31 + n, (long) fit.size())));
    }

    /** The kit a town's slot takes, fixed by the town and the slot's place: the same one every time. */
    public static Optional<String> choose(ResourceManager resources, String kingdom, String slot, int width, int depth,
            long town, BlockPos at) {
        List<String> fit = kits(resources, kingdom, slot, width, depth);
        if (fit.isEmpty()) {
            return Optional.empty();
        }
        long h = town * 0x9E3779B97F4A7C15L ^ at.asLong() * 0xC2B2AE3D27D4EB4FL;
        h ^= h >>> 29;
        return Optional.of(fit.get((int) Math.floorMod(h, (long) fit.size())));
    }

    /**
     * Lays the structure {@code id} with its footprint's north-west corner (in the world, after turning) at
     * {@code corner}, whose y is the ground's top where its {@code ground} layer goes, its front to {@code front}.
     * Empty if its template or metadata can't be found, or its ground isn't loaded.
     */
    public static Optional<Placed> place(ServerLevel level, String id, BlockPos corner, Direction front) {
        Optional<StructureTemplate> template = level.getStructureManager().get(template(id));
        Optional<Meta> meta = meta(level.getServer().getResourceManager(), id);
        if (template.isEmpty() || meta.isEmpty()) {
            dev.b28.enterprise.Enterprise.LOGGER.warn("Kit {} not found ({} template, {} metadata)", id,
                    template.isPresent() ? "a" : "no", meta.isPresent() ? "its" : "no");
            return Optional.empty();
        }
        Meta m = meta.get();
        StructurePlaceSettings settings = new StructurePlaceSettings().setRotation(turn(front)).setMirror(Mirror.NONE)
                .setIgnoreEntities(true);
        // Where its origin goes so its turned footprint's north-west corner is the corner, its ground layer at its y.
        BoundingBox turned = template.get().getBoundingBox(settings, BlockPos.ZERO);
        BlockPos origin = new BlockPos(corner.getX() - turned.minX(), corner.getY() - m.ground(), corner.getZ() - turned.minZ());
        BoundingBox box = template.get().getBoundingBox(settings, origin);
        for (int x = box.minX(); x <= box.maxX(); x++) {
            for (int z = box.minZ(); z <= box.maxZ(); z++) {
                if (!level.isLoaded(new BlockPos(x, 0, z))) {
                    return Optional.empty();
                }
            }
        }
        prepare(level, box);
        template.get().placeInWorld(level, origin, origin, settings, RandomSource.create(origin.asLong()), 2 | 16);
        List<BlockPos> beds = m.bedHeads().stream()
                .map(p -> StructureTemplate.calculateRelativePosition(settings, p).offset(origin)).toList();
        List<BlockPos> work = m.workstations().stream()
                .map(p -> StructureTemplate.calculateRelativePosition(settings, p).offset(origin)).toList();
        return Optional.of(new Placed(id, box, beds, work));
    }

    /**
     * Lays composed blocks (a house the generator made: `Houses`, `Compose`) as {@link #place} lays a kit: authored
     * facing south, {@code sx} x {@code sz} with its {@code ground} layer at {@code corner}'s y, turned to face
     * {@code front} with its footprint's north-west corner at {@code corner}. Returns its box in the world.
     */
    public static BoundingBox placeCells(ServerLevel level, java.util.Map<BlockPos, BlockState> cells, int sx, int sy, int sz,
            int ground, BlockPos corner, Direction front) {
        Compose.Grid g = new Compose.Grid(sx, sy, sz);
        g.paste(new Compose.Part(sx, sy, sz, cells), 0, 0, 0, Yards.quarter(front), false);
        boolean turned = front.getAxis() == Direction.Axis.X;
        int wx = turned ? sz : sx, wz = turned ? sx : sz;
        int oy = corner.getY() - ground;
        BoundingBox box = BoundingBox.fromCorners(new BlockPos(corner.getX(), oy, corner.getZ()),
                new BlockPos(corner.getX() + wx - 1, oy + sy - 1, corner.getZ() + wz - 1));
        prepare(level, box);
        g.cells.forEach((p, state) -> level.setBlock(new BlockPos(corner.getX() + p.getX(), oy + p.getY(), corner.getZ() + p.getZ()),
                state, 2 | 16));
        return box;
    }

    /**
     * Its ground made ready: under its footprint, natural ground filled up to its lowest layer (the ground's own block,
     * dirt under grass), and over it, to its top and a few blocks more, the trees and plants cleared.
     */
    static void prepare(ServerLevel level, BoundingBox box) {
        for (int x = box.minX(); x <= box.maxX(); x++) {
            for (int z = box.minZ(); z <= box.maxZ(); z++) {
                int top = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z) - 1;
                BlockPos.MutableBlockPos at = new BlockPos.MutableBlockPos(x, top, z);
                BlockState ground = level.getBlockState(at);
                while (ground.is(BlockTags.LOGS) && at.getY() > level.getMinBuildHeight()) {
                    at.move(Direction.DOWN);
                    ground = level.getBlockState(at);
                }
                BlockState fill = ground.is(Blocks.GRASS_BLOCK) || ground.is(Blocks.DIRT_PATH) || ground.isAir()
                        || !ground.getFluidState().isEmpty() ? Blocks.DIRT.defaultBlockState() : ground;
                for (int y = at.getY() + 1; y < box.minY(); y++) {
                    level.setBlock(new BlockPos(x, y, z), fill, 2);
                }
                for (int y = Math.max(box.minY(), at.getY() + 1); y <= box.maxY() + 6; y++) {
                    BlockPos p = new BlockPos(x, y, z);
                    BlockState s = level.getBlockState(p);
                    if (s.is(BlockTags.LOGS) || s.is(BlockTags.LEAVES) || !s.isAir() && s.canBeReplaced()) {
                        level.setBlock(p, Blocks.AIR.defaultBlockState(), 2);
                    }
                }
            }
        }
    }
}
