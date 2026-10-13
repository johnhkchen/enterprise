# Tear down and rebuild the standing test market from scratch.
kill @e[type=minecraft:villager,tag=testbench]
fill 19 -60 19 35 -56 35 minecraft:air
function testbench:fixture/build
