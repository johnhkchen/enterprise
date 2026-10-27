package dev.b28.enterprise.territory;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;

/**
 * Houses made procedurally from the studio's module set (`timber_relief`, thread 0004): a layout for a body
 * {@code W} x {@code D} of so many storeys, as the reference's `layoutOf` makes it (scripts/assets/lordaeron-ref/
 * compose.mjs: the shell's posts, corners and bays; the stair core; the plan layer's rooms and furniture by body width,
 * tuned with the floor-plan tool, thread 0008), but each face's bays chosen by wave-function collapse ({@link Wfc})
 * rather than by rule: a grid of a face's bays by storeys, the door, a shop front and the bays over the stair pinned
 * where the plan puts them, the rest choosing among windows, windows with a box and plain daub by the set's weights,
 * a window box only beside a plain window along the front (a rhythm), and each bay's kind the same up its column
 * (a house reads in columns). The same body and seed give the same house; another seed another face.
 */
public final class Houses {
    private Houses() {}

    /** What a house is asked for. */
    public record Params(int w, int d, int storeys, boolean partyWest, boolean partyEast, boolean shop, boolean trade,
            boolean doorLeft, long seed) {}

    /** The bays a face's free slots choose among, and how often (the set's weights). */
    static final String[] KINDS = {"window", "window_box", "plain"};
    static final double[] WEIGHTS = {4, 2, 1};

    /**
     * The house's layout, as compose.mjs's (body, storeys, party, roof, core, shell, plan, chimney, beds,
     * workstations, doorBay, door), or empty for a body the plan layer has no rules for: widths and depths of 3n + 1
     * from 7, a 7-wide body of one storey (thread 0008), a 10-wide one up to three.
     */
    public static Optional<JsonObject> layout(Params p, JsonObject planData) {
        int W = p.w(), D = p.d(), S = p.storeys();
        if (W < 7 || D < 7 || (W - 1) % 3 != 0 || (D - 1) % 3 != 0 || S < 1 || W < 10 && S > 1 || S > 3) {
            return Optional.empty();
        }
        // The plan layer's body class for this body (the studio's plan.json: its core, partitions and furniture).
        JsonObject bodyClass = null;
        for (var e : planData.getAsJsonObject("bodies").entrySet()) {
            JsonObject b = e.getValue().getAsJsonObject(), when = b.getAsJsonObject("when");
            JsonArray range = b.getAsJsonArray("storeys");
            if (when.get("W").getAsInt() == W && when.get("D").getAsInt() == D && range.get(0).getAsInt() <= S
                    && S <= range.get(1).getAsInt()) {
                bodyClass = b;
            }
        }
        if (bodyClass == null) {
            return Optional.empty();
        }
        // Beds a chamber: three where it's wide enough (13), else two (floor a bed 4 to 8.5).
        Map<String, Integer> vars = new HashMap<>(Map.of("W", W, "D", D, "cx", W - 4, "storeys", S, "beds", W >= 13 ? 3 : 2));
        boolean[] party = {p.partyWest(), p.partyEast()};
        JsonObject coreSpec = bodyClass.has("core") && bodyClass.get("core").isJsonObject() ? bodyClass.getAsJsonObject("core") : null;
        String core = coreSpec == null ? "none" : coreSpec.get("type").getAsString();
        int[] coreAt = coreSpec == null ? null : new int[] {eval(coreSpec.getAsJsonArray("at").get(0), vars),
                eval(coreSpec.getAsJsonArray("at").get(1), vars)};
        int nb = (W - 1) / 3, doorBay = (nb - 1) / 2;
        Set<String> coreCells = new HashSet<>();
        if (core.equals("straight")) {
            for (int z = coreAt[1]; z <= coreAt[1] + 4; z++) {
                coreCells.add(coreAt[0] + "," + z);
                coreCells.add((coreAt[0] + 1) + "," + z);
            }
        } else if (core.equals("turned")) {
            for (int x = coreAt[0]; x <= coreAt[0] + 2; x++) {
                for (int z = coreAt[1]; z <= coreAt[1] + 2; z++) {
                    coreCells.add(x + "," + z);
                }
            }
        }
        // The chimney on the west wall's post beside the hearth (the ground storey's hearth, if it stands at the wall).
        int chimneyZ = 3;
        for (var e : bodyClass.getAsJsonObject("storey").getAsJsonArray("0")) {
            JsonObject o = e.getAsJsonObject();
            if (o.get("module").getAsString().equals("hearth")) {
                int hx = eval(o.getAsJsonArray("at").get(0), vars), hz = eval(o.getAsJsonArray("at").get(1), vars);
                if (hx == 1 && hz % 3 == 0) {
                    chimneyZ = hz;
                }
            }
        }
        boolean westChimney = !party[0] && D >= 7;
        // The free bays' kinds, face by face, by wave-function collapse.
        Map<String, String> chosen = new HashMap<>();
        for (String face : new String[] {"front", "back", "west", "east"}) {
            int L = face.equals("front") || face.equals("back") ? W : D;
            int bays = (L - 1) / 3;
            boolean isParty = face.equals("west") && party[0] || face.equals("east") && party[1];
            if (isParty) {
                continue;
            }
            String[] fixed = new String[bays * S];
            for (int s = 0; s < S; s++) {
                for (int b = 0; b < bays; b++) {
                    int i = b * 3 + 1;
                    int[][] cells = switch (face) {
                        case "front" -> new int[][] {{i, D - 2}, {i + 1, D - 2}};
                        case "back" -> new int[][] {{W - 1 - i, 1}, {W - 2 - i, 1}};
                        case "west" -> new int[][] {{1, i}, {1, i + 1}};
                        default -> new int[][] {{W - 2, D - 1 - i}, {W - 2, D - 2 - i}};
                    };
                    boolean stair = false;
                    for (int[] c : cells) {
                        stair |= coreCells.contains(c[0] + "," + c[1]);
                    }
                    String pin = null;
                    if (face.equals("front") && s == 0 && b == doorBay) {
                        pin = "door";
                    } else if (stair) {
                        pin = "landing";
                    } else if (face.equals("front") && s == 0 && p.shop()) {
                        pin = "shop";
                    } else if (s == 0 && (face.equals("west") || face.equals("east")) && !core.equals("none")) {
                        pin = "landing";
                    }
                    fixed[s * bays + b] = pin;
                }
            }
            String[] kinds = solveFace(face, bays, S, fixed, p.seed() * 31 + face.hashCode());
            for (int s = 0; s < S; s++) {
                for (int b = 0; b < bays; b++) {
                    chosen.put(face + ":" + s + ":" + b, kinds[s * bays + b]);
                }
            }
        }
        JsonArray shell = new JsonArray(), plan = new JsonArray();
        for (int s = 0; s < S; s++) {
            String st = s == 0 ? "ground" : "upper";
            for (String face : new String[] {"front", "back", "west", "east"}) {
                int L = face.equals("front") || face.equals("back") ? W : D;
                boolean isParty = face.equals("west") && party[0] || face.equals("east") && party[1];
                for (int i = 0; i < L; i++) {
                    boolean corner = i == 0 || i == L - 1;
                    if (corner && (face.equals("west") || face.equals("east"))) {
                        continue;
                    }
                    String module;
                    boolean mirror = false;
                    if (corner) {
                        boolean partySide = face.equals("front") && (i == 0 && party[0] || i == L - 1 && party[1])
                                || face.equals("back") && (i == 0 && party[1] || i == L - 1 && party[0]);
                        module = (partySide ? "corner_party_" : "corner_") + st;
                    } else if (i % 3 == 0) {
                        module = face.equals("west") && westChimney && i == chimneyZ ? "chimney_" + st
                                : (isParty ? "post_party_" : "post_") + st;
                    } else if (i % 3 == 1) {
                        int b = i / 3;
                        if (isParty) {
                            module = "bay_party_" + st;
                        } else {
                            String kind = chosen.get(face + ":" + s + ":" + b);
                            module = switch (kind) {
                                case "door" -> "bay_door_ground";
                                case "shop" -> "bay_shop_ground";
                                case "landing" -> "bay_landing_" + st;
                                case "window_box" -> "bay_window_box_" + st;
                                case "plain" -> "bay_plain_" + st;
                                default -> "bay_window_" + st;
                            };
                            // (The fit-out keeps the spine from a right-hand door: a mirrored door met a pot on its step.)
                            mirror = false;
                        }
                    } else {
                        continue; // a bay's second column
                    }
                    JsonObject m = new JsonObject();
                    m.addProperty("face", face);
                    m.addProperty("i", i);
                    m.addProperty("module", module);
                    m.addProperty("storey", s);
                    if (module.startsWith("bay_") && !isParty) {
                        m.addProperty("mirror", mirror);
                    }
                    shell.add(m);
                }
            }
        }
        // The stair core, storey by storey.
        for (int s = 0; s < S && !core.equals("none"); s++) {
            String part = s == 0 ? "bottom" : s == S - 1 ? "top" : "mid";
            put(plan, s, "core_" + core + "_" + part, coreAt, 0, 0);
        }
        // The plan layer's rooms and furniture, from the data (plan.json): the storey's own list, or "upper" above the
        // ground; a placement's `if` decides it, `role` records a bed or the workstation.
        List<int[]> beds = new ArrayList<>(), work = new ArrayList<>();
        int doorX = doorBay * 3 + 2;
        JsonObject storeys = bodyClass.getAsJsonObject("storey");
        for (int s = 0; s < S; s++) {
            JsonArray list = storeys.has(String.valueOf(s)) ? storeys.getAsJsonArray(String.valueOf(s))
                    : s > 0 && storeys.has("upper") ? storeys.getAsJsonArray("upper") : new JsonArray();
            for (var e : list) {
                JsonObject o = e.getAsJsonObject();
                if (o.has("if") && eval(o.get("if"), vars) == 0) {
                    continue;
                }
                int[] at = {eval(o.getAsJsonArray("at").get(0), vars), eval(o.getAsJsonArray("at").get(1), vars)};
                int dy = o.has("dy") ? o.get("dy").getAsInt() : 1, q = o.has("q") ? o.get("q").getAsInt() : 0;
                String role = o.has("role") ? o.get("role").getAsString() : "";
                String module = o.get("module").getAsString();
                if (role.equals("work")) {
                    if (!p.trade()) {
                        if (o.has("else")) {
                            put(plan, s, o.get("else").getAsString(), at, dy, q);
                        }
                        continue;
                    }
                    work.add(new int[] {s, at[0], at[1]});
                }
                if (role.equals("bed")) {
                    beds.add(new int[] {s, at[0], at[1]});
                }
                put(plan, s, module, at, dy, q);
            }
        }
        JsonObject out = new JsonObject();
        JsonArray body = new JsonArray();
        body.add(W);
        body.add(D);
        out.add("body", body);
        out.addProperty("storeys", S);
        JsonArray pa = new JsonArray();
        pa.add(party[0]);
        pa.add(party[1]);
        out.add("party", pa);
        // Eaves to the street on a house wider than deep, the gable to it on a square one (as the golden houses).
        out.addProperty("roof", W > D ? "gable_x" : "gable_z");
        out.addProperty("core", core);
        out.add("shell", shell);
        out.add("plan", plan);
        JsonArray ch = new JsonArray();
        ch.add(0);
        ch.add(westChimney ? chimneyZ : 1);
        out.add("chimney", ch);
        out.addProperty("doorBay", doorBay);
        out.addProperty("doorX", doorX);
        JsonArray bs = new JsonArray();
        for (int[] b : beds) {
            JsonArray a = new JsonArray();
            for (int v : b) {
                a.add(v);
            }
            bs.add(a);
        }
        out.add("beds", bs);
        JsonArray ws = new JsonArray();
        for (int[] w : work) {
            JsonArray a = new JsonArray();
            for (int v : w) {
                a.add(v);
            }
            ws.add(a);
        }
        out.add("workstations", ws);
        return Optional.of(out);
    }

    static final int H = Compose.H;

    /** A house the generator made: its metadata as a kit's, and its blocks (authored facing south) and size. */
    public record Built(Kits.Meta meta, java.util.Map<net.minecraft.core.BlockPos, net.minecraft.world.level.block.state.BlockState> cells,
            int sx, int sy, int sz) {}

    /**
     * The house a town of {@code kingdom} builds next at its tier (`town_rules/<kingdom>.json`'s homes: requests to the
     * generator by tier, weighted), made from its module set and plan data, the request and face fixed by the seed;
     * empty if the kingdom has no rules, set or plan, or no request fits under {@code widest} x {@code deepest} (with
     * its relief).
     */
    public static Optional<Built> next(net.minecraft.server.level.ServerLevel level, String kingdom, String tier,
            int widest, int deepest, long seed) {
        var resources = level.getServer().getResourceManager();
        var rules = WallKit.json(resources, "town_rules/" + kingdom + ".json");
        var plan = WallKit.json(resources, "town_kits/" + kingdom + "/plan.json");
        if (rules.isEmpty() || plan.isEmpty() || !rules.get().has("homes")) {
            return Optional.empty();
        }
        JsonObject homes = rules.get().getAsJsonObject("homes");
        JsonArray byTier = homes.has(tier) ? homes.getAsJsonArray(tier) : homes.has("village") ? homes.getAsJsonArray("village")
                : new JsonArray();
        List<JsonObject> fit = new ArrayList<>();
        double total = 0;
        for (var e : byTier) {
            JsonObject r = e.getAsJsonObject(), solve = r.getAsJsonObject("solve");
            if (solve.has("terrace")) {
                continue; // terraces need a run of plots: not yet
            }
            if (solve.get("W").getAsInt() + 2 <= widest && solve.get("D").getAsInt() + 2 <= deepest) {
                fit.add(r);
                total += r.has("weight") ? r.get("weight").getAsDouble() : 1;
            }
        }
        if (fit.isEmpty()) {
            return Optional.empty();
        }
        double pick = new java.util.Random(seed).nextDouble() * total;
        JsonObject chosen = fit.get(0);
        for (JsonObject r : fit) {
            pick -= r.has("weight") ? r.get("weight").getAsDouble() : 1;
            if (pick <= 0) {
                chosen = r;
                break;
            }
        }
        JsonObject solve = chosen.getAsJsonObject("solve");
        return build(level, kingdom, new Params(solve.get("W").getAsInt(), solve.get("D").getAsInt(),
                solve.get("storeys").getAsInt(), false, false, false, false, false, seed), null);
    }

    /**
     * A house of the kingdom's module set and plan data for {@code p} (its body, storeys, party walls, shop, seed),
     * recoloured to {@code palette} (`Palettes`; null for the set's own). A kingdom built on another's module set (no
     * plan data of its own) takes that set's, in its own palettes.
     */
    public static Optional<Built> build(net.minecraft.server.level.ServerLevel level, String kingdom, Params p, String palette) {
        var resources = level.getServer().getResourceManager();
        String set = WallKit.json(resources, "town_kits/" + kingdom + "/plan.json").isPresent() ? kingdom : Palettes.SET;
        var plan = WallKit.json(resources, "town_kits/" + set + "/plan.json");
        if (plan.isEmpty()) {
            return Optional.empty();
        }
        var layout = layout(p, plan.get());
        if (layout.isEmpty()) {
            return Optional.empty();
        }
        var grid = Compose.compose(resources, Compose.blocks(level), "enterprise:" + set + "/part/timber_relief",
                Compose.Layout.of(layout.get()));
        var cells = Palettes.apply(resources, kingdom, palette, grid.cells);
        String id = "enterprise:generated/" + kingdom + "_" + p.w() + "x" + p.d() + "_s" + p.storeys()
                + (p.partyWest() || p.partyEast() ? "_t" + (p.partyWest() ? "w" : "") + (p.partyEast() ? "e" : "") : "")
                + (palette == null ? "" : "_" + palette) + "_" + Long.toHexString(p.seed());
        var meta = Kits.parse(id, meta(id, layout.get(), grid.sx, grid.sy, grid.sz));
        return Optional.of(new Built(meta, cells, grid.sx, grid.sy, grid.sz));
    }

    /**
     * A position or a condition of the plan data: an integer, or an expression over its variables (W, D, cx, storeys,
     * beds) of + and -, and one comparison (>=, <=, ==, >, <) giving 1 or 0.
     */
    static int eval(com.google.gson.JsonElement e, Map<String, Integer> vars) {
        if (e.isJsonPrimitive() && e.getAsJsonPrimitive().isNumber()) {
            return e.getAsInt();
        }
        String x = e.getAsString().replace(" ", "");
        for (String op : new String[] {">=", "<=", "==", ">", "<"}) {
            int k = x.indexOf(op);
            if (k > 0) {
                int a = sum(x.substring(0, k), vars), b = sum(x.substring(k + op.length()), vars);
                boolean t = switch (op) {
                    case ">=" -> a >= b;
                    case "<=" -> a <= b;
                    case "==" -> a == b;
                    case ">" -> a > b;
                    default -> a < b;
                };
                return t ? 1 : 0;
            }
        }
        return sum(x, vars);
    }

    private static int sum(String x, Map<String, Integer> vars) {
        int total = 0, sign = 1, i = 0;
        while (i < x.length()) {
            char c = x.charAt(i);
            if (c == '+' || c == '-') {
                sign = c == '-' ? -1 : 1;
                i++;
                continue;
            }
            int j = i;
            while (j < x.length() && x.charAt(j) != '+' && x.charAt(j) != '-') {
                j++;
            }
            String term = x.substring(i, j);
            Integer v = term.chars().allMatch(Character::isDigit) ? Integer.valueOf(term) : vars.get(term);
            if (v == null) {
                throw new IllegalArgumentException("plan data: no variable " + term + " in " + x);
            }
            total += sign * v;
            sign = 1;
            i = j;
        }
        return total;
    }

    /**
     * A generated house's metadata as the contract has it (`docs/assets/README.md`: entrances, beds, workstations, in the
     * template's own frame, its front to the south), from its layout (as compose.mjs's `metaOf`).
     */
    public static JsonObject meta(String id, JsonObject layout, int sx, int sy, int sz) {
        boolean partyWest = layout.getAsJsonArray("party").get(0).getAsBoolean();
        int rw = partyWest ? 0 : 1, D = layout.getAsJsonArray("body").get(1).getAsInt();
        JsonObject m = new JsonObject();
        m.addProperty("id", id);
        m.addProperty("rev", 1);
        m.addProperty("kingdom", "lordaeron");
        m.addProperty("kind", "home");
        m.addProperty("slot", "home");
        JsonArray fp = new JsonArray();
        fp.add(sx);
        fp.add(sz);
        m.add("footprint", fp);
        m.addProperty("height", sy);
        m.addProperty("ground", 2);
        JsonArray doors = new JsonArray();
        JsonObject door = new JsonObject();
        door.add("at", arr(layout.get("doorX").getAsInt() + rw, 3, D - 1 + 1));
        door.addProperty("facing", "south");
        door.addProperty("width", 1);
        doors.add(door);
        m.add("entrances", doors);
        JsonArray beds = new JsonArray();
        for (var b : layout.getAsJsonArray("beds")) {
            JsonArray a = b.getAsJsonArray();
            int s = a.get(0).getAsInt(), x = a.get(1).getAsInt(), z = a.get(2).getAsInt();
            JsonObject o = new JsonObject();
            o.add("head", arr(x + rw, 3 + H * s, z + 1));
            o.add("foot", arr(x + rw, 3 + H * s, z + 2));
            beds.add(o);
        }
        m.add("beds", beds);
        JsonArray work = new JsonArray();
        for (var w : layout.getAsJsonArray("workstations")) {
            JsonArray a = w.getAsJsonArray();
            JsonObject o = new JsonObject();
            o.add("at", arr(a.get(1).getAsInt() + rw, 3, a.get(2).getAsInt() + 1));
            o.addProperty("block", "any");
            work.add(o);
        }
        m.add("workstations", work);
        for (String k : new String[] {"lockers", "markers", "pilings", "entities"}) {
            m.add(k, new JsonArray());
        }
        m.addProperty("notes", "generated by Houses (the module set and wave-function collapse), not authored");
        return m;
    }

    private static JsonArray arr(int... v) {
        JsonArray a = new JsonArray();
        for (int x : v) {
            a.add(x);
        }
        return a;
    }

    /** A plan module at a storey's cell, {@code dy} over its floor layer (furniture and partitions stand on it: 1). */
    private static void put(JsonArray plan, int storey, String module, int[] at, int dy, int q) {
        JsonObject o = new JsonObject();
        o.addProperty("storey", storey);
        o.addProperty("module", module);
        JsonArray a = new JsonArray();
        a.add(at[0]);
        a.add(at[1]);
        o.add("at", a);
        if (dy != 0) {
            o.addProperty("dy", dy);
        }
        if (q != 0) {
            o.addProperty("q", q);
        }
        plan.add(o);
    }

    /**
     * A face's free bays by wave-function collapse: a grid of {@code bays} along by {@code storeys} up, cells pinned
     * where {@code fixed} names a kind (the door, a shop front, a bay over the stair), the rest a window, a window with
     * a box, or plain daub. Along a storey, a box only beside a plain window (a box, a window, a box...); up a column,
     * the same kind (a pinned cell meets anything above and below). Falls back to plain windows if it can't be solved.
     */
    static String[] solveFace(String face, int bays, int storeys, String[] fixed, long seed) {
        List<String> ids = new ArrayList<>(List.of("window", "window_box", "plain", "door", "shop", "landing"));
        double[] weight = {WEIGHTS[0], face.equals("front") ? WEIGHTS[1] : 0, WEIGHTS[2], 0, 0, 0};
        List<Wfc.Module> modules = new ArrayList<>();
        for (int k = 0; k < ids.size(); k++) {
            String id = ids.get(k);
            boolean pinnedKind = k >= 3;
            Map<Wfc.Side, String> sockets = new java.util.EnumMap<>(Wfc.Side.class);
            // Along a storey: a box beside a plain window or a pinned bay; anything else beside anything but a box.
            sockets.put(Wfc.Side.WEST, id.equals("window_box") ? "box_w" : pinnedKind ? "*" : "open");
            sockets.put(Wfc.Side.EAST, id.equals("window_box") ? "box_e" : pinnedKind ? "*" : "open");
            // Up a column: the same kind (a pinned bay meets anything).
            sockets.put(Wfc.Side.UP, pinnedKind ? "*" : "col_" + id);
            sockets.put(Wfc.Side.DOWN, pinnedKind ? "*" : "col_" + id);
            modules.add(new Wfc.Module(id, sockets, Math.max(weight[k], pinnedKind ? 1 : 0)));
        }
        // A box meets a plain window (open) or a pinned bay, never another box.
        var rules = new Wfc.Rules(modules, Set.of(List.of("box_e", "open"), List.of("open", "box_w")), Map.of());
        Map<Integer, Integer> pins = new HashMap<>();
        for (int c = 0; c < fixed.length; c++) {
            if (fixed[c] != null) {
                pins.put(Wfc.cell(c % bays, c / bays, 0, bays, 1), ids.indexOf(fixed[c]));
            }
        }
        // A pinned kind only where it's pinned; a window box only on the front.
        var grid = Wfc.solve(rules, bays, storeys, 1, (at, m) -> {
            int k = ids.indexOf(m.id());
            if (k >= 3) {
                return Integer.valueOf(k).equals(pins.get(Wfc.cell(at[0], at[1], 0, bays, 1)));
            }
            return !m.id().equals("window_box") || face.equals("front");
        }, pins, seed, 8);
        String[] out = new String[bays * storeys];
        for (int s = 0; s < storeys; s++) {
            for (int b = 0; b < bays; b++) {
                int c = s * bays + b;
                out[c] = grid.isPresent() ? ids.get(grid.get()[Wfc.cell(b, s, 0, bays, 1)])
                        : fixed[c] != null ? fixed[c] : "window";
                if (fixed[c] != null) {
                    out[c] = fixed[c]; // a pinned bay is its pin
                }
            }
        }
        return out;
    }
}
