package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.hex.Hex;
import java.util.List;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class HexTests {
    @GameTest(template = "floor")
    public static void hexCentresMapBackToTheirHex(GameTestHelper helper) {
        for (int q = -5; q <= 5; q++) {
            for (int r = -5; r <= 5; r++) {
                Hex hex = new Hex(q, r);
                helper.assertTrue(Hex.at(hex.centerX(), hex.centerZ()).equals(hex), "centre of " + hex + " maps back");
            }
        }
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void hexesAreEightyBlocksAcross(GameTestHelper helper) {
        Hex origin = new Hex(0, 0);
        Hex east = origin.neighbors().get(0);
        double dx = east.centerX() - origin.centerX();
        double dz = east.centerZ() - origin.centerZ();
        helper.assertTrue(Math.abs(Math.sqrt(dx * dx + dz * dz) - 80) < 1e-6, "neighbouring centres are 80 apart");
        helper.assertTrue(origin.neighbors().stream().allMatch(n -> n.distance(origin) == 1), "neighbours are 1 away");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void lineRunsHexByHex(GameTestHelper helper) {
        Hex a = new Hex(-3, 1);
        Hex b = new Hex(4, -2);
        List<Hex> line = Hex.line(a, b);
        helper.assertTrue(line.size() == a.distance(b) + 1, "a line has distance + 1 hexes");
        helper.assertTrue(line.get(0).equals(a) && line.get(line.size() - 1).equals(b), "it runs end to end");
        for (int i = 1; i < line.size(); i++) {
            helper.assertTrue(line.get(i).distance(line.get(i - 1)) == 1, "each step is to a neighbour");
        }
        helper.succeed();
    }
}
