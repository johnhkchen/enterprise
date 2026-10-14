package dev.b28.enterprise.caravan;

import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Territory;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.component.DataComponents;
import net.minecraft.network.chat.Component;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.MapItem;
import net.minecraft.world.level.saveddata.maps.MapDecorationTypes;
import net.minecraft.world.level.saveddata.maps.MapItemSavedData;

/**
 * What a Cartographer brings back from the road: the towns along it (settlements within {@link #ALONG} hexes of the
 * path, which the home desk now knows of) and a map of the destination's country, drawn from what grows where (like
 * an explorer map: nothing is loaded), with the destination marked and the towns it saw on the way.
 */
public final class Cartography {
    /** A Cartographer notes towns this many hexes either side of the road. */
    public static final int ALONG = 2;

    /** Maps are drawn at this scale: 2 is 512 blocks across. */
    public static final byte SCALE = 2;

    private Cartography() {}

    /** Desks of the settlements within {@link #ALONG} hexes of the route's path, but not its ends. */
    public static List<GlobalPos> townsAlong(ServerLevel level, Territory territory, Route route) {
        List<GlobalPos> towns = new ArrayList<>();
        List<Hex> path = route.path();
        for (GlobalPos bell : Settlements.get(level.getServer()).all().keySet()) {
            if (!bell.dimension().equals(route.from().dimension())) {
                continue;
            }
            Hex at = Hex.at(bell.pos());
            if (path.stream().anyMatch(hex -> hex.distance(at) <= ALONG)) {
                territory.desk(bell).filter(desk -> !desk.equals(route.from()) && !desk.equals(route.to()))
                        .ifPresent(towns::add);
            }
        }
        return towns;
    }

    /** A map of the destination's country, the destination marked with an X and the towns seen marked as villages. */
    public static ItemStack mapOf(ServerLevel level, Route route, List<GlobalPos> towns) {
        var to = route.to().pos();
        ItemStack map = MapItem.create(level, to.getX(), to.getZ(), SCALE, true, true);
        MapItem.renderBiomePreviewMap(level, map);
        MapItemSavedData.addTargetDecoration(map, to, "destination", MapDecorationTypes.TARGET_X);
        for (int i = 0; i < towns.size(); i++) {
            MapItemSavedData.addTargetDecoration(map, towns.get(i).pos(), "town" + i, MapDecorationTypes.PLAINS_VILLAGE);
        }
        map.set(DataComponents.ITEM_NAME, Component.translatable("item.enterprise.road_map", to.getX(), to.getZ()));
        return map;
    }
}
