# Quiet, predictable world for testing. Daylight stays on: villagers restock on the clock.
gamerule doMobSpawning false
gamerule doPatrolSpawning false
gamerule doTraderSpawning false
gamerule doWeatherCycle false
gamerule doInsomnia false
gamerule keepInventory true
weather clear
difficulty peaceful
tellraw @a [{"text":"Testbench loaded. ","color":"gray"},{"text":"/function testbench:help","color":"aqua","clickEvent":{"action":"suggest_command","value":"/function testbench:help"}}]
