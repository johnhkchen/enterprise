package dev.b28.enterprise.territory;

import dev.b28.enterprise.merchant.Warehouse;

/**
 * What a town puts first, from how it stands at its census, as a new city in a game of Civilization grows before it
 * builds and builds before it trades. Growing: short of food (under twice the caravans' par) or of people (under
 * {@link #GROWN}), it lays out fields of food alone. Building: fed and peopled but with every bed taken, it still farms
 * for food and feed, while it builds homes. Trading: fed, peopled, with room to grow, it farms for what sells (cane for
 * paper among it).
 */
public enum Focus {
    GROW, BUILD, TRADE;

    /** Villagers a town grows to before it farms for trade. */
    public static final int GROWN = 8;

    public static Focus of(int villagers, int beds, double foodPoints) {
        if (foodPoints < 2 * Warehouse.FOOD_PAR_POINTS || villagers < GROWN) {
            return GROW;
        }
        return villagers >= beds ? BUILD : TRADE;
    }

    /** Does this focus farm only for food (and the herds' feed)? */
    public boolean foodOnly() {
        return this != TRADE;
    }

    public String label() {
        return switch (this) {
            case GROW -> "growing (food first)";
            case BUILD -> "building (homes first)";
            case TRADE -> "trading";
        };
    }
}
