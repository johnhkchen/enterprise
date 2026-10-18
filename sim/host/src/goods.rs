//! The host's content pack: the Minecraft goods the network study's world deals in, at the mod's prices
//! (`money/Prices.java`: items to the emerald, 32 for anything unlisted), vanilla's stack sizes and food (nutrition
//! and saturation), the mod's comfort tags (`data/enterprise/tags/item/comforts/`) and its crops (`Crops.YIELDS`).

use enterprise_core::{Content, Good};

/// Ticks in a Minecraft day.
pub const DAY: u64 = 24_000;

/// A good: its name, items to the emerald, its stack, its food (nutrition, saturation, meat), its kind of comfort, and
/// whether farmers grow it.
struct Item {
    name: &'static str,
    per_emerald: f64,
    stack: u32,
    food: (f64, f64, bool),
    comfort: Option<&'static str>,
    crop: bool,
}

const fn plain(name: &'static str, per_emerald: f64, stack: u32) -> Item {
    Item { name, per_emerald, stack, food: (0.0, 0.0, false), comfort: None, crop: false }
}

const fn crop(name: &'static str, per_emerald: f64, food: (f64, f64)) -> Item {
    Item { name, per_emerald, stack: 64, food: (food.0, food.1, false), comfort: None, crop: true }
}

const fn food(name: &'static str, per_emerald: f64, nutrition: f64, saturation: f64, meat: bool) -> Item {
    Item { name, per_emerald, stack: 64, food: (nutrition, saturation, meat), comfort: None, crop: false }
}

const fn comfort(name: &'static str, per_emerald: f64, stack: u32, kind: &'static str) -> Item {
    Item { name, per_emerald, stack, food: (0.0, 0.0, false), comfort: Some(kind), crop: false }
}

/// Everything the study's towns hold, make, trade or eat, and the goods the core's slices name (kit, herds, the
/// workshop's conversions, the realms' wares, the army's rations).
const ITEMS: &[Item] = &[
    // Crops (`Crops.YIELDS`), at what villagers pay.
    crop("wheat", 20.0, (0.0, 0.0)),
    crop("carrot", 22.0, (3.0, 3.6)),
    crop("potato", 26.0, (1.0, 0.6)),
    crop("beetroot", 15.0, (1.0, 1.2)),
    crop("pumpkin", 6.0, (0.0, 0.0)),
    crop("melon", 4.0, (0.0, 0.0)),
    crop("sugar_cane", 24.0, (0.0, 0.0)),
    crop("sweet_berries", 10.0, (2.0, 0.4)),
    crop("nether_wart", 22.0, (0.0, 0.0)),
    crop("cocoa_beans", 20.0, (0.0, 0.0)),
    // Goods villagers buy.
    plain("paper", 24.0, 64),
    plain("ink_sac", 5.0, 64),
    plain("stick", 32.0, 64),
    plain("flint", 26.0, 64),
    plain("string", 14.0, 64),
    plain("feather", 24.0, 64),
    plain("tripwire_hook", 8.0, 64),
    plain("glass_pane", 11.0, 64),
    plain("compass", 1.0, 64),
    plain("coal", 15.0, 64),
    plain("raw_iron", 5.0, 64),
    plain("iron_ingot", 4.0, 64),
    plain("raw_gold", 3.0, 64),
    plain("gold_ingot", 3.0, 64),
    plain("diamond", 1.0, 64),
    plain("clay_ball", 10.0, 64),
    plain("leather", 6.0, 64),
    plain("rabbit_hide", 9.0, 64),
    plain("white_wool", 32.0, 64),
    plain("hay_block", 20.0 / 9.0, 64),
    plain("rotten_flesh", 32.0, 64),
    plain("glass_bottle", 32.0, 64),
    plain("lapis_lazuli", 1.0, 64),
    plain("stone", 32.0, 64),
    plain("brick", 10.0, 64),
    plain("tobacco_leaf", 8.0, 64),
    plain("wheat_seeds", 32.0, 64),
    // What hands gather.
    plain("oak_log", 8.0, 64),
    plain("spruce_log", 8.0, 64),
    plain("sand", 32.0, 64),
    plain("cobblestone", 32.0, 64),
    plain("cactus", 16.0, 64),
    plain("dandelion", 16.0, 64),
    plain("poppy", 16.0, 64),
    // Food villagers sell, and game and meat.
    food("bread", 6.0, 5.0, 6.0, false),
    food("apple", 4.0, 4.0, 2.4, false),
    food("baked_potato", 8.0, 5.0, 6.0, false),
    food("cod", 32.0, 2.0, 0.4, false),
    food("cooked_cod", 32.0, 5.0, 6.0, false),
    food("salmon", 32.0, 2.0, 0.4, false),
    food("cooked_salmon", 32.0, 6.0, 9.6, false),
    food("melon_slice", 32.0, 2.0, 1.2, false),
    food("rabbit", 8.0, 3.0, 1.8, false),
    food("beef", 8.0, 3.0, 1.8, false),
    food("porkchop", 7.0, 3.0, 1.8, false),
    food("chicken", 14.0, 2.0, 1.2, false),
    food("mutton", 7.0, 2.0, 1.2, false),
    food("cooked_porkchop", 5.0, 8.0, 12.8, true),
    food("cooked_chicken", 8.0, 6.0, 7.2, true),
    food("cooked_beef", 5.0, 8.0, 12.8, true),
    food("cooked_mutton", 5.0, 6.0, 9.6, true),
    food("cooked_rabbit", 5.0, 5.0, 6.0, true),
    // Comforts (`data/enterprise/tags/item/comforts/`).
    comfort("cookie", 18.0, 64, "treats"),
    comfort("cake", 1.0, 1, "treats"),
    comfort("pumpkin_pie", 4.0, 64, "treats"),
    comfort("honey_bottle", 3.0, 16, "treats"),
    comfort("book", 4.0, 64, "reading"),
    comfort("writable_book", 2.0, 1, "reading"),
    comfort("leather_helmet", 0.2, 1, "clothing"),
    comfort("leather_chestplate", 1.0 / 7.0, 1, "clothing"),
    comfort("leather_leggings", 1.0 / 3.0, 1, "clothing"),
    comfort("leather_boots", 0.25, 1, "clothing"),
    comfort("white_carpet", 4.0, 64, "furnishings"),
    comfort("red_carpet", 4.0, 64, "furnishings"),
    comfort("yellow_carpet", 4.0, 64, "furnishings"),
    comfort("blue_carpet", 4.0, 64, "furnishings"),
    comfort("candle", 3.0, 64, "furnishings"),
    comfort("painting", 0.5, 64, "furnishings"),
    comfort("flower_pot", 4.0, 64, "furnishings"),
    comfort("lantern", 1.0, 64, "furnishings"),
    comfort("white_banner", 1.0 / 3.0, 16, "furnishings"),
    comfort("pipe_tobacco", 4.0, 64, "leisure"),
    // Smiths' goods: tools, weapons, armour.
    plain("wooden_sword", 2.0, 1),
    plain("stone_sword", 1.0, 1),
    plain("stone_axe", 1.0, 1),
    plain("stone_shovel", 1.0, 1),
    plain("stone_pickaxe", 1.0, 1),
    plain("iron_sword", 2.0 / 3.0, 1),
    plain("iron_axe", 1.0 / 3.0, 1),
    plain("iron_shovel", 0.5, 1),
    plain("iron_pickaxe", 2.0 / 3.0, 1),
    plain("iron_hoe", 1.0, 1),
    plain("iron_helmet", 0.2, 1),
    plain("iron_chestplate", 1.0 / 9.0, 1),
    plain("iron_leggings", 1.0 / 7.0, 1),
    plain("iron_boots", 0.25, 1),
    plain("chainmail_helmet", 1.0, 1),
    plain("chainmail_chestplate", 0.25, 1),
    plain("chainmail_leggings", 1.0 / 3.0, 1),
    plain("chainmail_boots", 1.0, 1),
    plain("diamond_sword", 1.0 / 22.0, 1),
    plain("diamond_axe", 1.0 / 18.0, 1),
    plain("diamond_helmet", 1.0 / 13.0, 1),
    plain("diamond_chestplate", 1.0 / 21.0, 1),
    plain("diamond_leggings", 1.0 / 19.0, 1),
    plain("diamond_boots", 1.0 / 13.0, 1),
    plain("shield", 0.2, 1),
    plain("bow", 0.5, 1),
    plain("arrow", 16.0, 64),
    plain("saddle", 1.0 / 6.0, 1),
];

/// What a full stack weighs where it isn't 1: `ModPack.WEIGHTS` in the mod, kept in step by hand (the mod has jungle and
/// acacia logs and red sand too, which the study deals in none of; stone and brick are study goods the mod's pack doesn't
/// list yet). Mild: ore, raw metal, coal, bricks, ingots, logs and hay blocks 2 a stack, stone, cobblestone and sand 3,
/// so a wagon (27 stacks of weight) of iron carries 13 stacks and a wagon of wool 27.
pub const WEIGHTS: &[(&str, f64)] = &[
    ("stone", 3.0), ("cobblestone", 3.0), ("sand", 3.0),
    ("raw_iron", 2.0), ("raw_gold", 2.0), ("coal", 2.0), ("brick", 2.0), ("hay_block", 2.0),
    ("iron_ingot", 2.0), ("gold_ingot", 2.0),
    ("oak_log", 2.0), ("spruce_log", 2.0),
];

/// The study's content: its goods, the mod's dials.
pub fn content() -> Content {
    let goods: Vec<Good> = ITEMS
        .iter()
        .map(|i| {
            let mut g = Good::new(i.name, 1.0 / i.per_emerald, i.stack);
            if let Some((_, w)) = WEIGHTS.iter().find(|(n, _)| *n == i.name) {
                g = g.weight(w / i.stack.max(1) as f64);
            }
            if let Some(kind) = i.comfort {
                let k = Content::COMFORTS.iter().position(|c| *c == kind).expect("one of the mod's kinds");
                g = g.comfort(k);
            }
            if i.food.0 > 0.0 {
                g = g.food(i.food.0).saturation(i.food.1);
            }
            if i.food.2 {
                g = g.meat();
            }
            if i.crop {
                g = g.crop();
            }
            g
        })
        .collect();
    Content::new(goods, DAY)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every weighted name is a good of the study, and a stack of it weighs what the table says (the core weighs a piece).
    #[test]
    fn weights_land_on_the_goods() {
        let c = content();
        for (name, w) in WEIGHTS {
            let g = &c.goods[c.good(name).unwrap_or_else(|| panic!("{name} is not a study good"))];
            assert!((g.weight * g.stack as f64 - w).abs() < 1e-9, "{name}");
        }
        let wool = &c.goods[c.good("white_wool").unwrap()];
        assert!((wool.weight * wool.stack as f64 - 1.0).abs() < 1e-9);
    }
}
