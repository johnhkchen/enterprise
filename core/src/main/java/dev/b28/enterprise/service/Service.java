package dev.b28.enterprise.service;

import java.util.function.Predicate;
import java.util.function.ToDoubleFunction;

/**
 * Something a villager of one trade does for a town: at home, for its own town whenever the town is touched; abroad,
 * for the town its caravan reaches; or on the road, worked by the caravan itself as it travels. Each is registered by
 * name in {@link Services}, so later eras add theirs (a Toolsmith mending the wagon, an Armorer fitting the guards)
 * without changing who runs them.
 *
 * @param id      its registered name
 * @param trade   the profession that does it
 * @param where   where it's done
 * @param does    what it does, in plain words, for the readout
 * @param wanted  would the town served have a use for it now? Asked before a trip too (see {@link Visit#planning()}),
 *                from the ledgers alone: that's what sends this trade along.
 * @param perform does it; returns what the town served owes for it, in emeralds, at most {@link Visit#budget()}
 *                (nothing at home)
 */
public record Service(String id, String trade, Where where, String does, Predicate<Visit> wanted,
        ToDoubleFunction<Visit> perform) {
    public enum Where { HOME, ABROAD, ROAD }

    /** A service the caravan works itself on the road: listed for the readout, run by the caravan. */
    public static Service road(String id, String trade, String does) {
        return new Service(id, trade, Where.ROAD, does, visit -> false, visit -> 0);
    }
}
