# Run from the same spot as market/build.
kill @e[type=minecraft:villager,tag=testbench]
fill ~-1 ~ ~-1 ~15 ~4 ~15 minecraft:air
tellraw @s {"text":"Test market cleared.","color":"yellow"}
