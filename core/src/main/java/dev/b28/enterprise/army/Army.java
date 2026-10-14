package dev.b28.enterprise.army;

import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.caravan.Caravans;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Territory;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.util.RandomSource;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * The sovereign's army, on the ledger: the soldiers its levies raised (villagers off the world while they serve), the
 * kit they draw from the front's warehouse (its quartermaster's, supplied from the capital by contract and caravan),
 * what campaigning wears out, who falls, and what each town gave. Recruits wait at the capital, in its barracks, for a
 * caravan to the front. Each campaign day a soldier draws whatever kit it lacks, the best there is (a weapon: diamond,
 * iron, stone or wooden sword, or a bow with arrows; a helmet, chestplate, leggings and boots; a shield), eats its
 * ration (an army eats a lot; unfed, a soldier
 * runs more risk and may desert), wears its kit, and may fall: less often the better its kit,
 * and its enchantments suited to the enemy. The fallen are gone from their town for good, and it grieves. At peace
 * the rest walk home. Enchantments don't change what kit is; they change how it serves, and the tally counts that.
 */
public final class Army extends SavedData {
    private static final String NAME = "enterprise_army";

    /** Who the war is against: which enchantments tell. */
    public enum Theatre {
        FACTION("minecraft:sharpness", "minecraft:protection"),
        UNDEAD("minecraft:smite", "minecraft:protection"),
        NETHER("minecraft:fire_aspect", "minecraft:fire_protection");

        public final String weaponEnchant;
        public final String armourEnchant;

        Theatre(String weaponEnchant, String armourEnchant) {
            this.weaponEnchant = weaponEnchant;
            this.armourEnchant = armourEnchant;
        }
    }

    /** A campaign day's chance of falling, before kit. */
    public static final double DEATH_BASE = 0.02;
    /** An army eats: food points a soldier a day, from the capital's warehouse. */
    public static final int RATION = 8;
    /** Unfed, a soldier runs half again the risk, and may desert (walk home) at this chance a day. */
    public static final double HUNGRY_RISK = 1.5;
    public static final double DESERTION = 0.1;

    /**
     * Good food is good morale: a food's heartiness is how far its saturation per point of food is above plain bread's
     * (golden carrots are the heartiest), from 0 to 1, and meat of any kind (cooked, or in a stew) is at least as hearty
     * as a pork chop. A meal's is its foods', by points. A hearty army fights a quarter harder, falls 15% less, and
     * deserts half as readily when short.
     */
    public static final double BREAD_SATURATION = 1.2;
    public static final double HEARTY_STRENGTH = 0.25;
    public static final double HEARTY_RISK = 0.15;
    /** Meat, any kind the army can be sent: what a sovereign stockpiles for hearty meals. */
    public static final List<Item> MEATS = List.of(Items.COOKED_PORKCHOP, Items.COOKED_BEEF, Items.COOKED_MUTTON,
            Items.COOKED_CHICKEN, Items.COOKED_RABBIT, Items.RABBIT_STEW);
    public static final double MEAT_HEARTINESS = 1.0 / 3;

    public static double heartiness(double saturationPerPoint) {
        double above = saturationPerPoint - BREAD_SATURATION;
        return above < 1e-4 ? 0 : Math.min(1, above / BREAD_SATURATION); // (a float's rounding is no feast)
    }

    /** How hearty a food is: by its saturation, and meat at least as hearty as a pork chop. */
    public static double heartiness(Item food) {
        int points = nutrition(food);
        double bySaturation = points == 0 ? 0 : heartiness(saturation(food) / points);
        return MEATS.contains(food) ? Math.max(MEAT_HEARTINESS, bySaturation) : bySaturation;
    }

    /** Arrows an archer shoots a day. */
    public static final int ARROWS_PER_DAY = 8;

    /** Weapons, best first: attack power, and how much a soldier carrying one is at risk (none: twice). */
    public static final List<Gear> WEAPONS = List.of(
            new Gear(Items.DIAMOND_SWORD, 7, 0.8, 12), new Gear(Items.IRON_SWORD, 6, 1.0, 10),
            new Gear(Items.DIAMOND_AXE, 6, 0.95, 12), new Gear(Items.IRON_AXE, 5, 1.15, 10), // axes: lesser weapons
            new Gear(Items.STONE_SWORD, 5, 1.25, 6), new Gear(Items.STONE_AXE, 4, 1.35, 6),
            new Gear(Items.WOODEN_SWORD, 4, 1.4, 4), new Gear(Items.BOW, 6, 1.1, 10));

    /** Weapons of any kind held in a stock: what a quartermaster counts toward a weapon each. */
    public static int weapons(MerchantStock stock) {
        return WEAPONS.stream().filter(g -> g.item() != Items.BOW)
                .mapToInt(g -> stock.count(new ItemCost(g.item(), 1))).sum();
    }
    /**
     * Armour, a piece for each of four places, best first, with its armour points (a full diamond set is 20, iron 15,
     * chainmail 12, leather 7) and the days it lasts. Protection is the points worn, all pieces together.
     */
    public static final List<List<Gear>> ARMOUR = List.of(
            List.of(new Gear(Items.DIAMOND_HELMET, 3, 1, 30), new Gear(Items.IRON_HELMET, 2, 1, 20),
                    new Gear(Items.CHAINMAIL_HELMET, 2, 1, 16), new Gear(Items.LEATHER_HELMET, 1, 1, 10)),
            List.of(new Gear(Items.DIAMOND_CHESTPLATE, 8, 1, 30), new Gear(Items.IRON_CHESTPLATE, 6, 1, 20),
                    new Gear(Items.CHAINMAIL_CHESTPLATE, 5, 1, 16), new Gear(Items.LEATHER_CHESTPLATE, 3, 1, 10)),
            List.of(new Gear(Items.DIAMOND_LEGGINGS, 6, 1, 30), new Gear(Items.IRON_LEGGINGS, 5, 1, 20),
                    new Gear(Items.CHAINMAIL_LEGGINGS, 4, 1, 16), new Gear(Items.LEATHER_LEGGINGS, 2, 1, 10)),
            List.of(new Gear(Items.DIAMOND_BOOTS, 3, 1, 30), new Gear(Items.IRON_BOOTS, 2, 1, 20),
                    new Gear(Items.CHAINMAIL_BOOTS, 1, 1, 16), new Gear(Items.LEATHER_BOOTS, 1, 1, 10)));
    public static final Gear SHIELD = new Gear(Items.SHIELD, 0, 0.85, 12);
    public static final double UNARMED_RISK = 2.0;
    /** Unarmoured, a soldier runs this much more risk; a full diamond set takes it down to 0.5, a full iron one ~0.77. */
    public static final double UNARMOURED_RISK = 1.6;
    public static final double FULL_SET_POINTS = 20;

    /** The risk factor of wearing so many armour points. */
    static double armourRisk(int points) {
        return UNARMOURED_RISK * (1 - (1 - 0.5 / UNARMOURED_RISK) * Math.min(1, points / FULL_SET_POINTS));
    }

    /** Every piece of armour a soldier can wear, any place. */
    public static List<Gear> allArmour() {
        return ARMOUR.stream().flatMap(List::stream).toList();
    }

    /** A piece of kit: its power (attack, or armour points), the risk factor it carries, and the days it lasts. */
    public record Gear(Item item, int power, double risk, int days) {}

    /** A serving soldier. */
    static final class Soldier {
        CompoundTag villager;
        GlobalPos home;
        long enlisted;
        ItemStack weapon = ItemStack.EMPTY;
        final ItemStack[] armour = {ItemStack.EMPTY, ItemStack.EMPTY, ItemStack.EMPTY, ItemStack.EMPTY};
        ItemStack shield = ItemStack.EMPTY;
        double wornWeapon;
        final double[] wornArmour = new double[4];
        double wornShield;
        int arrows;
    }

    /** What a town gave the war. */
    public static final class Record {
        int raised;
        int fallen;
        int days;
        double strength; // soldier-days weighted by kit and its fitness for the enemy
        int hungry; // soldier-days unfed
        int deserted;
        public int hungry() { return hungry; }
        public int deserted() { return deserted; }
        public int raised() { return raised; }
        public int fallen() { return fallen; }
        public int days() { return days; }
        public double strength() { return strength; }
    }

    private final List<Soldier> soldiers = new ArrayList<>();
    private final List<CompoundTag> barracks = new ArrayList<>(); // recruits at the capital, waiting for the front
    private GlobalPos front; // where the war is fought
    private final Map<GlobalPos, Record> records = new LinkedHashMap<>();
    private final Map<String, Integer> issued = new java.util.TreeMap<>(); // kit issued, by item (and if enchanted)
    private final Map<String, Integer> enchantLevels = new java.util.TreeMap<>(); // levels of each enchantment issued
    private Theatre theatre = Theatre.FACTION;
    private long eaten; // food points the army has eaten
    private double heartySum; // heartiness by soldier-day, summed
    private long mealDays;
    private long lastDay = -1;

    public static Army get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Army::new, Army::load, null), NAME);
    }

    public int size() {
        return soldiers.size();
    }

    public Theatre theatre() {
        return theatre;
    }

    public void setTheatre(Theatre theatre) {
        this.theatre = theatre;
        setDirty();
    }

    public Map<GlobalPos, Record> records() {
        return Map.copyOf(records);
    }

    public long eaten() {
        return eaten;
    }

    /** How hearty its meals were, on average over soldier-days: 0 plain bread, 1 the best. */
    public double heartiness() {
        return mealDays == 0 ? 0 : heartySum / mealDays;
    }

    /**
     * Eats {@code points} of food from the depot, the heartiest first. Returns the points eaten and their heartiness,
     * summed by point.
     */
    static double[] mess(MerchantStock depot, int points) {
        double[] meal = {0, 0};
        if (points <= 0) {
            return meal;
        }
        for (Item food : heartiestFirst(depot)) {
            int left = points - (int) meal[0];
            if (left <= 0) {
                break;
            }
            int n = (left + nutrition(food) - 1) / nutrition(food);
            int taken = depot.take(new ItemCost(food, 1), n).stream().mapToInt(ItemStack::getCount).sum();
            meal[0] += taken * nutrition(food);
            meal[1] += taken * nutrition(food) * heartiness(food);
        }
        return meal;
    }

    public static double[] messForTest(MerchantStock depot, int points) {
        return mess(depot, points);
    }

    public static float saturation(Item item) {
        var food = item.getDefaultInstance().get(net.minecraft.core.component.DataComponents.FOOD);
        return food == null ? 0 : food.saturation();
    }

    public Map<String, Integer> issued() {
        return Map.copyOf(issued);
    }

    public Map<String, Integer> enchantLevels() {
        return Map.copyOf(enchantLevels);
    }

    /** Soldiers serving from a town. */
    public int servingFrom(GlobalPos home) {
        return (int) soldiers.stream().filter(s -> s.home.equals(home)).count();
    }

    public java.util.Optional<GlobalPos> front() {
        return java.util.Optional.ofNullable(front);
    }

    public void setFront(GlobalPos at) {
        front = at;
        setDirty();
    }

    /** Recruits waiting at the capital for a caravan to the front. */
    public int waiting() {
        return barracks.size();
    }

    /** A levied recruit arrives at the capital and waits in its barracks (its home on it) for a caravan to the front. */
    public void muster(CompoundTag recruit) {
        barracks.add(recruit);
        setDirty();
    }

    /** Up to {@code n} waiting recruits board a caravan to the front. */
    public List<CompoundTag> embark(int n) {
        List<CompoundTag> going = new ArrayList<>(barracks.subList(0, Math.min(n, barracks.size())));
        barracks.subList(0, going.size()).clear();
        setDirty();
        return going;
    }

    /** A recruit joins: off the world, its town's to count. */
    public void enlist(CompoundTag villager, GlobalPos home, long now) {
        Soldier soldier = new Soldier();
        soldier.villager = villager;
        soldier.home = home;
        soldier.enlisted = now;
        soldiers.add(soldier);
        records.computeIfAbsent(home, h -> new Record()).raised++;
        setDirty();
    }

    /** What kit the army will want on hand: a set for each soldier and half again, and arrows for its archers. */
    public Map<Item, Integer> wants() {
        return wants(4);
    }

    /**
     * As above, with {@code rationDays} of food for each soldier and each recruit waiting to join (counted as bread: the
     * quartermaster sends heartier food in its place where the capital has it).
     */
    public Map<Item, Integer> wants(int rationDays) {
        Map<Item, Integer> wants = new LinkedHashMap<>();
        int men = soldiers.size();
        wants.put(Items.IRON_SWORD, men);
        for (List<Gear> place : ARMOUR) {
            wants.put(place.get(1).item(), men); // an iron set each
        }
        wants.put(Items.SHIELD, men / 2);
        wants.put(Items.ARROW, men * ARROWS_PER_DAY * 2);
        int mouths = men + barracks.size();
        wants.put(Items.BREAD, (mouths * RATION * rationDays + nutrition(Items.BREAD) - 1) / nutrition(Items.BREAD));
        return wants;
    }

    /** Spare days of rations the front keeps beyond the time a delivery takes. */
    public static final int RATION_SPARE_DAYS = 2;

    /**
     * Days of rations the front keeps on hand, {@code distance} blocks from the capital: what it eats while the next
     * order goes and its delivery comes (the order rides back with a caravan, one sets out with the goods, and a caravan
     * may be on the road meanwhile: three round trips), and spare.
     */
    public static int rationDays(int distance) {
        int roundTrip = (int) Math.ceil(2.0 * Caravans.travelTicks(distance) / 24000);
        return 3 * roundTrip + RATION_SPARE_DAYS;
    }

    /** Food points held in a stock, of every food. */
    public static int foodPoints(MerchantStock stock) {
        return stock.totals().entrySet().stream().mapToInt(e -> e.getValue()
                * nutrition(BuiltInRegistries.ITEM.get(net.minecraft.resources.ResourceLocation.parse(e.getKey()))))
                .sum();
    }

    /** Food a stock holds, the heartiest first, bread and the like last. */
    public static List<Item> heartiestFirst(MerchantStock stock) {
        return stock.totals().keySet().stream()
                .map(id -> BuiltInRegistries.ITEM.get(net.minecraft.resources.ResourceLocation.parse(id)))
                .filter(item -> nutrition(item) > 0)
                .sorted(java.util.Comparator.comparingDouble((Item item) -> -heartiness(item))
                        .thenComparingDouble(item -> -saturation(item) / nutrition(item)))
                .toList();
    }

    /** A campaign day's chance of falling, in this kit, against this enemy. */
    public static double risk(ItemStack weapon, ItemStack armour, ItemStack shield, Theatre theatre) {
        return risk(weapon, List.of(armour), shield, theatre);
    }

    /** As above, in all the armour it wears: protection from the points of every piece, and their enchantments. */
    public static double risk(ItemStack weapon, List<ItemStack> armour, ItemStack shield, Theatre theatre) {
        Optional<Gear> w = gear(WEAPONS, weapon);
        int edge = w.map(g -> level(weapon, g.item() == Items.BOW ? "minecraft:power" : theatre.weaponEnchant)).orElse(0);
        int guard = armour.stream().mapToInt(piece -> level(piece, theatre.armourEnchant)).sum();
        return DEATH_BASE
                * w.map(Gear::risk).orElse(UNARMED_RISK) * Math.max(0.6, 1 - 0.05 * edge)
                * armourRisk(points(armour)) * Math.max(0.5, 1 - 0.06 * guard)
                * (shield.isEmpty() ? 1 : SHIELD.risk());
    }

    /** Armour points worn. */
    static int points(List<ItemStack> armour) {
        return armour.stream().mapToInt(piece -> gear(allArmour(), piece).map(Gear::power).orElse(0)).sum();
    }

    public static int nutrition(Item item) {
        var food = item.getDefaultInstance().get(net.minecraft.core.component.DataComponents.FOOD);
        return food == null ? 0 : food.nutrition();
    }

    static int level(ItemStack stack, String enchantment) {
        int level = 0;
        for (var entry : stack.getEnchantments().entrySet()) {
            if (entry.getKey().unwrapKey().map(k -> k.location().toString().equals(enchantment)).orElse(false)) {
                level = entry.getIntValue();
            }
        }
        return level;
    }

    static Optional<Gear> gear(List<Gear> kinds, ItemStack stack) {
        return kinds.stream().filter(g -> stack.is(g.item())).findFirst();
    }

    /** Draws the best of these kinds the depot has, enchanted ones first. */
    private ItemStack draw(MerchantStock depot, List<Gear> kinds) {
        for (Gear gear : kinds) {
            List<ItemStack> there = depot.copies(new ItemCost(gear.item(), 1));
            if (there.isEmpty()) {
                continue;
            }
            ItemStack best = there.stream().max(java.util.Comparator.comparingInt(s -> s.getEnchantments().size()))
                    .orElseThrow();
            var taken = depot.take(new ItemCost(gear.item().builtInRegistryHolder(), 1,
                    net.minecraft.core.component.DataComponentPredicate.allOf(best.getComponents())), 1);
            ItemStack piece = taken.isEmpty() ? depot.take(new ItemCost(gear.item(), 1), 1).stream().findFirst()
                    .orElse(ItemStack.EMPTY) : taken.get(0);
            if (!piece.isEmpty()) {
                String name = BuiltInRegistries.ITEM.getKey(piece.getItem()).getPath()
                        + (piece.isEnchanted() ? " (enchanted)" : "");
                issued.merge(name, 1, Integer::sum);
                piece.getEnchantments().entrySet().forEach(e -> e.getKey().unwrapKey().ifPresent(k ->
                        enchantLevels.merge(k.location().getPath(), e.getIntValue(), Integer::sum)));
                return piece;
            }
        }
        return ItemStack.EMPTY;
    }

    /**
     * The campaign's days since the last: each soldier draws what it lacks from the depot, wears it, serves (its
     * town's pay owed under its levy), and may fall.
     */
    public void campaign(ServerLevel level, MerchantStock depot, GlobalPos capital, long now) {
        if (lastDay < 0) {
            lastDay = now;
            setDirty();
            return;
        }
        int days = (int) ((now - lastDay) / 24000);
        if (days <= 0) {
            return;
        }
        lastDay += days * 24000L;
        MinecraftServer server = level.getServer();
        ContractBook book = ContractBook.get(server);
        Territory territory = Territory.get(server);
        RandomSource random = level.random;
        CaravanLedger ledger = CaravanLedger.get(server);
        for (int day = 0; day < days; day++) {
            // The day's rations, from the depot, its best food first: as far as they go, the army is fed, and the
            // heartier the meal, the better it fights and the less it falls or deserts.
            int wanted = soldiers.size() * RATION;
            double[] meal = mess(depot, wanted);
            int got = (int) meal[0];
            eaten += got;
            double fed = wanted == 0 ? 1 : Math.min(1, (double) got / wanted);
            double hearty = got == 0 ? 0 : meal[1] / got;
            heartySum += hearty * soldiers.size();
            mealDays += soldiers.size();
            int fedSoldiers = (int) Math.floor(soldiers.size() * fed);
            int index = 0;
            for (var it = soldiers.iterator(); it.hasNext(); index++) {
                Soldier soldier = it.next();
                boolean hungry = index >= fedSoldiers;
                if (hungry && random.nextDouble() < DESERTION * (1 - 0.5 * hearty)) {
                    // Deserted: it walks home, its kit with it lost to the army.
                    records.computeIfAbsent(soldier.home, h -> new Record()).deserted++;
                    it.remove();
                    GlobalPos from = front == null ? capital : front;
                    int distance = (int) Math.sqrt(soldier.home.pos().distSqr(from.pos()));
                    ledger.walkHome(soldier.home, soldier.villager, now + Caravans.travelTicks(distance));
                    continue;
                }
                if (soldier.weapon.isEmpty() || soldier.weapon.is(Items.BOW) && soldier.arrows <= 0) {
                    if (soldier.weapon.is(Items.BOW)) {
                        soldier.arrows += depot.take(new ItemCost(Items.ARROW, 1), ARROWS_PER_DAY * 4).stream()
                                .mapToInt(ItemStack::getCount).sum();
                    }
                    if (soldier.weapon.isEmpty()) {
                        soldier.weapon = draw(depot, WEAPONS);
                        soldier.wornWeapon = 0;
                        if (soldier.weapon.is(Items.BOW)) {
                            soldier.arrows += depot.take(new ItemCost(Items.ARROW, 1), ARROWS_PER_DAY * 4).stream()
                                    .mapToInt(ItemStack::getCount).sum();
                        }
                    }
                }
                for (int place = 0; place < ARMOUR.size(); place++) {
                    if (soldier.armour[place].isEmpty()) {
                        soldier.armour[place] = draw(depot, ARMOUR.get(place));
                        soldier.wornArmour[place] = 0;
                    }
                }
                if (soldier.shield.isEmpty() && !soldier.weapon.is(Items.BOW)) {
                    soldier.shield = draw(depot, List.of(SHIELD));
                    soldier.wornShield = 0;
                }
                // How it serves today, and the risk it runs.
                Optional<Gear> weapon = gear(WEAPONS, soldier.weapon)
                        .filter(g -> g.item() != Items.BOW || soldier.arrows > 0);
                List<ItemStack> worn = List.of(soldier.armour);
                int edge = weapon.map(g -> level(soldier.weapon, g.item() == Items.BOW ? "minecraft:power"
                        : theatre.weaponEnchant)).orElse(0);
                double risk = risk(weapon.isPresent() ? soldier.weapon : ItemStack.EMPTY, worn, soldier.shield,
                        theatre) * (hungry ? HUNGRY_RISK : 1 - HEARTY_RISK * hearty);
                Record record = records.computeIfAbsent(soldier.home, h -> new Record());
                record.days++;
                if (hungry) {
                    record.hungry++;
                }
                record.strength += weapon.map(Gear::power).orElse(1) * (1 + 0.1 * edge)
                        * (points(worn) * 0.4 + 4) / 10.0
                        * (hungry ? 0.5 : 1 + HEARTY_STRENGTH * hearty);
                // Its pay, under the levy (or hire) it came by: once, though its home has had more than one.
                book.of(soldier.home).stream()
                        .filter(c -> c.inForce() && c.buyer().equals(capital) && c.seller().equals(soldier.home)
                                && c.levy().isPresent())
                        .findFirst().ifPresent(c -> book.owe(c, c.levy().get().payPerDay()));
                // Wear: unbreaking makes it last.
                if (weapon.isPresent()) {
                    if (soldier.weapon.is(Items.BOW)) {
                        soldier.arrows = Math.max(0, soldier.arrows - ARROWS_PER_DAY);
                    }
                    soldier.wornWeapon += 1.0 / (weapon.get().days() * (1 + level(soldier.weapon, "minecraft:unbreaking")));
                    if (soldier.wornWeapon >= 1) {
                        soldier.weapon = ItemStack.EMPTY;
                    }
                }
                for (int place = 0; place < ARMOUR.size(); place++) {
                    ItemStack piece = soldier.armour[place];
                    Optional<Gear> gear = gear(ARMOUR.get(place), piece);
                    if (gear.isPresent()) {
                        soldier.wornArmour[place] += 1.0 / (gear.get().days() * (1 + level(piece, "minecraft:unbreaking")));
                        if (soldier.wornArmour[place] >= 1) {
                            soldier.armour[place] = ItemStack.EMPTY;
                        }
                    }
                }
                if (!soldier.shield.isEmpty()) {
                    soldier.wornShield += 1.0 / (SHIELD.days() * (1 + level(soldier.shield, "minecraft:unbreaking")));
                    if (soldier.wornShield >= 1) {
                        soldier.shield = ItemStack.EMPTY;
                    }
                }
                if (random.nextDouble() < risk) {
                    // Fallen: gone from its town for good, which grieves.
                    record.fallen++;
                    it.remove();
                    java.util.UUID id = soldier.villager.getUUID("UUID");
                    territory.bellOf(soldier.home).ifPresent(bell -> {
                        Settlements.get(server).remove(bell, id);
                        territory.mourn(bell, now);
                    });
                    dev.b28.enterprise.Enterprise.LOGGER.debug("A soldier from {} fell", soldier.home.pos().toShortString());
                }
            }
        }
        setDirty();
    }

    /** Peace: the soldiers walk home, as long as the road takes; their kit stays with the army. */
    public int demobilise(MinecraftServer server, GlobalPos capital, long now) {
        CaravanLedger ledger = CaravanLedger.get(server);
        int home = 0;
        GlobalPos from = front == null ? capital : front;
        for (Soldier soldier : soldiers) {
            int distance = (int) Math.sqrt(soldier.home.pos().distSqr(from.pos()));
            ledger.walkHome(soldier.home, soldier.villager, now + Caravans.travelTicks(distance));
            home++;
        }
        for (CompoundTag recruit : barracks) { // and those still waiting at the capital
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, recruit.get("enterprise_home")).result().ifPresent(h -> ledger.walkHome(
                    h, recruit, now + Caravans.travelTicks((int) Math.sqrt(h.pos().distSqr(capital.pos())))));
            home++;
        }
        barracks.clear();
        soldiers.clear();
        lastDay = -1;
        setDirty();
        return home;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        for (Soldier s : soldiers) {
            CompoundTag entry = new CompoundTag();
            entry.put("villager", s.villager);
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, s.home).ifSuccess(t -> entry.put("home", t));
            entry.putLong("enlisted", s.enlisted);
            if (!s.weapon.isEmpty()) entry.put("weapon", s.weapon.save(registries));
            ListTag set = new ListTag();
            for (int place = 0; place < 4; place++) {
                CompoundTag piece = new CompoundTag();
                if (!s.armour[place].isEmpty()) piece.put("item", s.armour[place].save(registries));
                piece.putDouble("worn", s.wornArmour[place]);
                set.add(piece);
            }
            entry.put("armourSet", set);
            if (!s.shield.isEmpty()) entry.put("shield", s.shield.save(registries));
            entry.putDouble("wornWeapon", s.wornWeapon);
            entry.putDouble("wornShield", s.wornShield);
            entry.putInt("arrows", s.arrows);
            list.add(entry);
        }
        tag.put("soldiers", list);
        ListTag waiting = new ListTag();
        waiting.addAll(barracks);
        tag.put("barracks", waiting);
        if (front != null) {
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, front).ifSuccess(t -> tag.put("front", t));
        }
        ListTag recs = new ListTag();
        records.forEach((home, r) -> {
            CompoundTag entry = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, home).ifSuccess(t -> entry.put("home", t));
            entry.putInt("raised", r.raised);
            entry.putInt("fallen", r.fallen);
            entry.putInt("days", r.days);
            entry.putDouble("strength", r.strength);
            entry.putInt("hungry", r.hungry);
            entry.putInt("deserted", r.deserted);
            recs.add(entry);
        });
        tag.put("records", recs);
        CompoundTag kit = new CompoundTag();
        issued.forEach(kit::putInt);
        tag.put("issued", kit);
        CompoundTag ench = new CompoundTag();
        enchantLevels.forEach(ench::putInt);
        tag.put("enchantLevels", ench);
        tag.putString("theatre", theatre.name());
        tag.putLong("lastDay", lastDay);
        tag.putLong("eaten", eaten);
        tag.putDouble("heartySum", heartySum);
        tag.putLong("mealDays", mealDays);
        return tag;
    }

    public static Army load(CompoundTag tag, HolderLookup.Provider registries) {
        Army army = new Army();
        for (Tag t : tag.getList("soldiers", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            Soldier s = new Soldier();
            s.villager = entry.getCompound("villager");
            s.home = GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("home")).result().orElse(null);
            if (s.home == null) {
                continue;
            }
            s.enlisted = entry.getLong("enlisted");
            s.weapon = entry.contains("weapon") ? ItemStack.parse(registries, entry.get("weapon")).orElse(ItemStack.EMPTY) : ItemStack.EMPTY;
            ListTag set = entry.getList("armourSet", Tag.TAG_COMPOUND);
            for (int place = 0; place < Math.min(4, set.size()); place++) {
                CompoundTag piece = set.getCompound(place);
                s.armour[place] = piece.contains("item")
                        ? ItemStack.parse(registries, piece.get("item")).orElse(ItemStack.EMPTY) : ItemStack.EMPTY;
                s.wornArmour[place] = piece.getDouble("worn");
            }
            s.shield = entry.contains("shield") ? ItemStack.parse(registries, entry.get("shield")).orElse(ItemStack.EMPTY) : ItemStack.EMPTY;
            s.wornWeapon = entry.getDouble("wornWeapon");
            s.wornShield = entry.getDouble("wornShield");
            s.arrows = entry.getInt("arrows");
            army.soldiers.add(s);
        }
        tag.getList("barracks", Tag.TAG_COMPOUND).forEach(t -> army.barracks.add((CompoundTag) t));
        army.front = tag.contains("front") ? GlobalPos.CODEC.parse(NbtOps.INSTANCE, tag.get("front")).result().orElse(null) : null;
        for (Tag t : tag.getList("records", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("home")).ifSuccess(home -> {
                Record r = new Record();
                r.raised = entry.getInt("raised");
                r.fallen = entry.getInt("fallen");
                r.days = entry.getInt("days");
                r.strength = entry.getDouble("strength");
                r.hungry = entry.getInt("hungry");
                r.deserted = entry.getInt("deserted");
                army.records.put(home, r);
            });
        }
        CompoundTag kit = tag.getCompound("issued");
        kit.getAllKeys().forEach(k -> army.issued.put(k, kit.getInt(k)));
        CompoundTag ench = tag.getCompound("enchantLevels");
        ench.getAllKeys().forEach(k -> army.enchantLevels.put(k, ench.getInt(k)));
        army.theatre = tag.contains("theatre") ? Theatre.valueOf(tag.getString("theatre")) : Theatre.FACTION;
        army.lastDay = tag.contains("lastDay") ? tag.getLong("lastDay") : -1;
        army.eaten = tag.getLong("eaten");
        army.heartySum = tag.getDouble("heartySum");
        army.mealDays = tag.getLong("mealDays");
        return army;
    }
}
