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

# Welcome panel on first join and every rejoin (see tick).
scoreboard objectives add tb_seen dummy
scoreboard objectives add tb_left minecraft.custom:minecraft.leave_game

# The standing test market: built once near spawn, then left running.
execute unless block 27 -60 27 enterprise:merchants_desk run function testbench:fixture/build
