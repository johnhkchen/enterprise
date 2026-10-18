# Decisions

Decisions made after the brief (`docs/brief.md`). Where this file and the brief disagree, this file wins. Newest changes are dated.

## Name (2026-10-13)

- **The modpack is called Enterprise**, not Staples. The brief still uses the old name.
- Core mod: `mod_id=enterprise`, package `dev.b28.enterprise`.

## Platform (2026-10-13)

- **Minecraft 1.21.1 on NeoForge, Java 21.** Create, Numismatics and BankSystem all top out at 1.21.1. Create's 26.1 port is in progress with no date, so expect a port later.
- **Each quality tier is its own item** (`cotton_bale_fine`), not a component on one item. Create recipes then work unchanged, and a build-time generator writes the per-tier recipes.
- **Content modules are JSON manifests turned into code at build time.** Datapacks cannot register items or blocks, so "adding tea is data, not code" means "write JSON, rebuild", not "drop in a datapack".

## The logistics rule (2026-10-13)

**Nothing crosses distance without a carrier.** Goods, coins and price news all travel. This is why StockMarket is out (its trades are instant across the map), and why banks are per settlement.

## Dependencies and money (2026-10-13)

- **Numismatics is a core dependency** (LGPL-3.0). It is the early-era money: physical coins, vendors, depositors.
- **StockMarket is dropped.** It has no license file, so it cannot be forked; we write our own exchange from scratch.
- **BankSystem is not a dependency.** We write our own bank on Numismatics and lift pieces of BankSystem (MIT) with credit where useful.
- **Ledger / Ledger Databases are not dependencies.** They are Fabric-only grief loggers. Their append-only event log on H2/MySQL/Postgres is a reference for our own ledger storage.
- **Eras of money:** Numismatics coins first, our bank as an unlock, our futures exchange on top of that.

## Coinage (2026-10-13)

- **Banks are per settlement.** No account is reachable from everywhere. Money moves between towns by courier or by bill of exchange.
- **Each faction mints its own coinage.** A coin's value is its metal content (a gold/silver standard) times trust in the issuer, so exchange rates differ slightly between factions.
- **The Merchant is the money changer.** It converts between coinages and keeps the spread.
- Faction coins are one coin item per denomination with a "mint" component naming the faction. Item variety stays bounded by factions × denominations.
- Numismatics coins act as the neutral standard (bullion, or the trade coin everyone accepts).
- **Coinage is the first graded good.** A coinage is a graded batch of metal: its fineness is its grade. Assaying is inspection, and clipping or debasement is adulteration. The grade, inspection and reputation grammar from the brief starts with money, before any crop.

## Lockers (2026-10-13, revised 2026-10-14)

- A **locker** is a vanilla chest, double chest or barrel touching a workstation. Its villager is whoever has claimed that workstation.
- **Lockers only work when a Merchant manages them.** No Merchant in the village, no locker trading.
- **Trades run right after the villager restocks** (vanilla: at most twice a day). Each enabled trade runs until its uses run out, the inputs run out, or the locker is full.
- **Which trades run is set on the Merchant's screen**, per locker.
- **No trading at inflated prices.** If demand has pushed a trade's price above its base, the locker skips that trade until demand cools off. Automation can't run a price into the ground.
- **Price:** the standard price, with a discount that grows with the Merchant's level. A top-level Merchant gets close to the discount a cured villager gives.
- **XP:** each locker trade gives XP to both the villager and the Merchant.
- **Locked-up villagers get lockers too.** Confinement affects settlement value, not lockers.
- **Trades run while unloaded.** One pure settlement function takes (contents snapshot, cached trades, elapsed time) and returns the new state. First it runs on load, replaying what was missed. Later the ledger runs the same function while the village is unloaded, and the result is written back when the village loads.
- **Catch-up on load (built 2026-10-14):** when a villager with a locker loads back in, it waits 2 seconds for the Merchant's desk and chests nearby to load, then replays the restocks it missed, two per whole day, through the normal locker trade (supply, trade, collect, sell). Measured: a market unloaded for 3 days replayed exactly the 48 trades it makes in 3 loaded days.
- **Catch-up cap: 60 days** of missed restocks, as a guard on load time. The locker's size, trade uses per restock and the inflated-price rule already bound what accumulates well before that.
- **Rounds (built 2026-10-14):** a market whose villagers aren't running (unloaded, or loaded but idle like spawn chunks with no player near) gets a short visit when the server has room. Every 10 seconds, if the average tick is under 30 ms, the most overdue desk (no round for an in-game day) has its footprint loaded on a ticket that expires after 15 seconds: the desk, its locker workstations and their neighbouring chunks, at most 25. Villagers loading in, or already loaded but idle, get catch-up. One round at a time.
- **Catch-up is budgeted:** at most 8 replayed restocks per tick across all villagers, none while ticks run long. A villager's restock clock advances by exactly the days replayed, so part-days carry over to the next round instead of being lost. Measured: a market left with nobody near keeps its loaded pace, one day settled per daily round.
- **Still to come:** settlement without loading anything (pure ledger), so remote views see results between rounds.
- **Write-back safety:** a snapshot records the locker's contents at unload. If the chest doesn't match the snapshot on load (rollback, hand edit), the ledger's result is discarded and a fresh snapshot is taken. No duplication either way.

## Merchant (2026-10-13)

- New villager profession with its own job-site block.
- **Its job site is the Merchant's Desk.**
- **It is the village's trading desk.** It is the counterparty for players, villagers and arriving caravans.
- **Its stock is the chest(s) touching its desk**, the same rule as lockers, so hoppers and Create work with it. It can reach into shulker boxes kept in those chests.
- **Reach:** until settlements exist, it manages lockers within 48 blocks of its desk. Its level sets how many, from 2 at Novice to 10 at Master.
- **It manages the town's lockers.** It fills them best deal first; a higher level manages more lockers. Its screen lists the lockers it manages and which trades each may run.
- It gains XP from every locker trade it manages, and its level drives the locker discount.
- **Merchant XP mirrors a trading player's (2026-10-14).** Each locker trade gives the Merchant what a player would pick up: 3 to 6 XP, plus 5 when the villager levels up. It levels at the points a player has at levels 15, 30, 45 and 55 (315, 1395, 4020, 6895). Tuned by experiment (`dev/experiments/merchant_ramp.py`): a pen of four villagers fed 18 shulkers of goods is fully managed by about day 2 and has a Master Merchant in about 7 to 9 in-game days, depending on which trades the villagers roll. Thresholds at player levels 5 to 20 took about 1 day, which skipped the "earn your partnerships" arc.
- **Right-clicking a Merchant opens its screen** (it has no trades of its own): its lockers, and per locker the villager's trades as on/off switches. Every trade starts off; the player opts in.
- **A trade is identified by its items** (cost A, cost B, result), not its position in the list, so a choice survives level-ups and price changes. Two trades with the same items (two enchanted books) share one switch for now.

## Merchant money (2026-10-14)

- **First loop:** the player fills the Merchant's stock chest with shulker boxes of raw goods. The Merchant feeds lockers from them, villagers trade, the Merchant collects the emeralds, then converts them to Numismatics coins.
- **The Merchant converts emeralds at its faction's floating rate.** The rate starts at a base (spurs per emerald), drops as emeralds pour in, and recovers over time toward the base. Computed on touch: a pressure number that decays with elapsed game time. Until factions exist, every Merchant uses one default faction.
- **A Blaze Banker touching the Merchant's Desk is the settlement's bank account.** Converted coins are deposited into it. Later, wages and purchases are paid out of it.
- **Collecting earnings (2026-10-14): in person, at the settlement's Blaze Banker.** The Merchant deposits into it; the owner and their trust list withdraw through the Banker's own menu. A Banker placed with no owner belongs to the first player who uses it. A Bank Card link (money arriving anywhere instantly) was considered and left out: it breaks "nothing crosses distance without a carrier".
- **No Banker: coins land as physical coins in the stock chest.** Each payout re-mints the loose coins already there together with the new ones as the fewest coins, so they stay a handful of stacks. If even that doesn't fit, the Merchant keeps the emeralds for later.
- **Rate numbers (2026-10-14):** base 16 spurs (one sprocket) per emerald; a full shulker of recent selling (1,728 emeralds) halves the rate; selling pressure halves every 3 in-game days. A batch is priced along the falling curve, not at its opening rate. Measured: a stocked four-villager pen pushes the rate from 16 to about 12 over a week and earns about 17,700 spurs in 10 days.
- **Working reserve:** the Merchant keeps 64 emeralds unsold for trades that cost emeralds. With every trade switched on, it spends what it earns and sells little.
- **Player shops:** Numismatics vendors only trade with a clicking player, so the Merchant deals with them by reading the vendor's mode, price and stock and moving goods and coins itself.
- **It surveys routes and runs caravans.**
- Wages are paid in coins.

## Settlements (2026-10-13)

- **A settlement is anchored by a bell plus at least 3 beds and 3 workstations** within 48 blocks (built 2026-10-14). The Merchant's Desk counts as a workstation.
- A census reads the villagers, beds (home points), workstations (job points) and the bell (meeting point). The result stays in the ledger.
- **The census is polled, never constant.** Every 10 seconds, when the server has headroom, the bell within 64 blocks of a player that has gone longest without a census (at most once per in-game day each) is counted, with path checks spread over ticks (two villagers per tick). `/enterprise census <bell>` counts on the spot. Results live in world save data. Still to come: census on rounds, after big block changes, and path checks off the main thread on a snapshot.
- **Free villagers count; prisoners barely do.** A villager is free when it can walk to its own bed, its workstation (if it has a job) and the bell. Settlement value counts free villagers fully and confined ones at 0.1. The path check uses its own path finder under villager rules (doors open), so a villager that is jumping, sleeping or just spawned is judged the same as one standing still. Goal: a thriving village is worth more than a trading hall with villagers locked in cells. The bell check stops a cell that just has a bed and workstation inside it from counting as free.
- Farmers produce food. Caravans eat food per day of travel, so villages are the natural resupply stops along a route.

## Territory: a hex grid (2026-10-13)

- **Civilization-style.** A settlement earns hexes with money or culture growth, and its population works them.
- **The hex is the ledger's unit of place.** Ownership, security, detection and tolls are per hex. A route is a list of hexes.
- **Size: a hex is about 80% of a naturally generated village's core**, roughly 80 blocks across to start, set in config. A normal village covers its own hex plus up to about 2 rings around it (up to 19 hexes). We'll tune this by experiment.
- **Working a hex:** in player towns, the real farms, mines and buildings on the hex count. Simulated towns get abstract yields.
- **Culture comes from villagers of certain professions working a hex that has an improvement**, as in Civilization. Improvements cost money to build, which makes them a money sink.
- **A garrison on a hex lets its owner levy tolls**, given the right kind of unit. Garrisons make blockades and sieges possible, and chokepoints worth fighting over.
- **Garrison units:** a militia of villagers, where each profession gives its own boost (like caravan squads), plus a new raider-style unit for settlements (knights and soldiers), the village's answer to pillagers. Players' role in garrisons is decided later.

## Hex grid (built 2026-10-14)

- Pointy-top axial hexes, 80 blocks across the flats (centre to corner 46.2). `Hex.at(pos)`, distance, neighbours, and the straight line of hexes between two points. A route reports its length in hexes; tolls, detection and encounters will be per hex. `/enterprise hex` shows the hex you stand in.

## Territory, farms and tolls (built 2026-10-14)

- **Territory grows by culture, Civilization-style.** A settlement claims its bell's hex at its first census. Each census adds culture: settlement value times days since the last census. When culture covers the next hex (5 x hexes held), the border grows into the unclaimed neighbouring hex nearest the bell, out to two rings (19 hexes). A held hex stays with its owner. `/enterprise territory <bell>`; `/enterprise hex` shows a hex's owner.
- **Farmers work hexes.** At each census, the settlement's farmers are assigned its hexes, most-planted first; a farmer tends 32 plots per level. The crops actually growing set the produce: half an item per plot per day (wheat, carrots, potatoes, beetroot, pumpkins and melons from stems, sugar cane, cocoa, berries, nether wart). Real farms are only counted, never harvested. Produce accrues on the ledger (up to 60 days) and lands in the settlement Merchant's chest whenever the market is touched: a locker session, a caravan stop, a census. A farming waystation feeds caravans and supplies trade goods.
- **Fields are tracked per chunk.** Each hex remembers, chunk by chunk, the crops last counted there. A census recounts the chunks that are loaded and keeps the last count for the rest; nothing reads as zero just for being unloaded. Rounds load a settlement's known field chunks, so its census reads them fresh.
- **Farmers draw wages (2026-10-14).** A farmer's wage is a share of the value of what it grows (starting at half), at the crop's local buying price (carrots 22 to the emerald, wheat 20, and so on: what villagers pay). Wages accrue daily and are paid whenever the market is touched: emeralds from the Merchant's chest first, then its coins at the region's current spurs-per-emerald, then the settlement's Blaze Banker. Wages leave the economy. Unpaid days are unworked: the harvest is delivered only for the share of wages paid; unpaid wages are forgiven, not carried as debt. The Merchant keeps 3 days of wages in emeralds instead of selling them to the mint.
- **Wages absorb farm profit.** The farmers' share floats between 25% and 100% of the harvest's value, toward the fraction of the harvest that actually sells for emeralds (crop sales are counted from locker trades turning crops into emeralds, averaged over 3 days; the share closes half its gap every 2 days). A farm whose produce all sells ends up paying its whole sale value in wages: passive farming pays for itself, not for you. Unsold produce costs the 25% floor, so a harvest is cheap material for caravans, waystations and factories, never free. Profit has to come from processing and trade.
- **Measured** (`dev/experiments/farm_wages.py`, 480 plots, about 240 carrots a day, 32 emeralds of capital, 20 days each in about 20 seconds of tick warp): with one buyer, the farmers' share climbed from 50% to about 95% and the town's money levelled off near 75 emeralds by day 12, gaining nothing more; with no buyer, the share sank to the 25% floor, the capital lasted 9 days, then the farm stopped.
- **Why farms can't print emeralds:** emeralds only enter through villagers buying, capped per villager per day (inflated prices are skipped); every item grown costs half its value in wages whether it sells or not; and a settlement with no income can't pay, so its farms stop. A farm's best size is what local buyers take; growing past that loses emeralds, and caravans can carry surplus to other towns' buyers.
- **Tolls are a cut of coins in transit.** A caravan bringing coins home pays each other settlement whose territory its path crosses that town's toll rate on what it carries (once per settlement, one after another); the route's own two settlements charge nothing. The toll lands in the owner's Blaze Banker or Merchant's chest at its next touch.
- **Tolls are collected at posts (2026-10-14, replacing tolls on guarded shares of land).** A town collects its toll only from caravans that pass one of its toll posts: a hex of its land with a guard on it. The toll is the town's rate (0% to 25%) on the coins carried, once per town. Caravans plan around posts: passing one costs its town's toll on what the route usually carries home, so a caravan steps around a post on open ground for a hex's food and pays only where going around costs more. Each guard eats 6 food points a day from the town's warehouse and takes 0.5 emerald a day; guards who go unfed or unpaid desert in proportion to what was missing, the last posted first. Guards are a ledger count for now; militia professions, knights and blockades come later.
- **Towns work out whether they hold a chokepoint (2026-10-14, replacing the step-a-point toll search).** Each caravan crossing a town's land shows the town what guarding that land would do. The town tries a post on each of its hexes on the caravan's path and keeps the one that makes going around dearest; on a tie, the narrowest (most impassable neighbours), then the nearest home. Then it tries again on the cheapest path around the posts so far, until the path keeps off its land, there is no way at all, or 6 posts. Each try records the detour's cost in road food and other towns' tolls. Every 3 days the town picks the toll and posts that bring in the most after the guards' pay. It asks at most 80% of a route's detour, so caravans keep coming, and at most 25%. It drops posts it no longer wants at once and hires one guard a review for a new one. Routes unseen for 4 reviews are forgotten. `/enterprise territory <bell>` shows each route's posts and what going around them costs.
- **Water makes chokepoints (2026-10-14).** Each chunk is surveyed when it loads and unloads: of 16 surface columns, how many are water or another fluid. Chunks never loaded are guessed from their biome: ocean is water, a river is half water. A hex counts the chunks whose middle lies in it. A hex more than half water can't be crossed by a caravan; less, and it costs up to twice the road food (1 + 2 x the water share). Only surveys that differ from the biome's guess are saved. `/enterprise hex` shows a hex's water.
- **Measured** (`network_study.py --river`, 15 days): a river across the toll town's row of hexes, with the town's two hexes left dry as bridges. By day 4 the town had a guard on one bridge at 25%, and by day 7 a guard on each, collecting about 107 spurs a day for about 21 spurs of guards. Without the river (open ground) it posts no guard: a single post is stepped around for one hex's food. The road to S2 now runs 1,000 blocks over the bridge instead of 600 straight.
- **Bridges (2026-10-14): one of the oldest chokepoints, and a town improvement.** A town can bridge a hex it holds that is too wet to cross; a bridged hex is open ground to caravans. At each toll review, a town with no bridge under way looks at each impassable hex it holds and asks what a bridge would bring in: on each known route it would carry (not the town's own), the lesser of the most toll it may ask and 80% of what the bridge saves over the cheapest way without it, with the town's own tolls left out (a route with no way at all would save its whole trade), one trip a day. It builds the bridge that repays its masons' wages soonest, if within 30 days and it can pay half the wages now. Leaving out its own tolls stops a town bridging beside its own bridge just to let caravans dodge its post.
- **Masons build bridges for wages, with stone from the town's land.** Two masons at 1 emerald a day each lay 32 blocks a day each, but the town's land supplies only 16 blocks of stone a day per hex it holds, so a small town builds slowly. Unpaid masons don't work and aren't owed. A bridge's deck runs from the hex's edge facing one bank, through its centre, to the edge facing the other, and 6 blocks past each, 3 wide with a railing either side, about 450 blocks. Once finished, the deck is laid on every wet column at the water's surface the next time its chunks can be loaded: stone bricks with stone brick walls, spruce in forests, sandstone in deserts and on beaches. The ledger is the bridge; the blocks are its view. `/enterprise territory` lists a town's bridges, `/enterprise hex` says whether a hex is bridged.
- **Measured** (`network_study.py --river-bridge`, 25 days): a river through the row south of the toll town with no crossing at all, so the hub's road to S3 had no way through and S3 made no trades. On day 8 the toll town started a bridge on its river hex (repaid in about 2 days, by its estimate), finished it for 15 emeralds of wages, and laid 301 wet columns of deck. It then posted a guard there at 25%: neither route it carries has any way around. It collected about 42 spurs a day, and S3 traded again from about day 12. Before the fix above, the town also bridged the next hex, which halved its own chokepoint.
- **Route distance** is the straight distance plus a hex's width for each hex a detour adds over the straight line of hexes.
- **Warehouses keep 5 slots free** for locker results and coins: farm produce and caravan deliveries stop short of them, and a good's par is capped so wanted goods fill at most 60% of the warehouse. A full chest of unsold produce had stopped towns trading after about 9 days.
- **Rounds hold their chunks until the work is done.** A round loads its footprint on a ticket with no timeout, waits until the chunks and their villagers have loaded, then runs catch-up and the settlement's census, holds 200 ticks, and releases the chunks; it gives up after 30 seconds of real time. A tick-based timeout failed under `/tick sprint`, where 300 ticks pass in milliseconds while chunks take real time to load.
- **Rounds load the market, not just the desk:** chunks within 16 blocks of the desk and of the bell (where the Merchant and villagers spend their time), the locker workstations, and the settlement's field chunks, at most 25. A round that loaded only the desk's chunk missed a Merchant standing in the next chunk, and its lockers went unmanaged that day.
- **Censuses guard against half-loaded villages:** a round-loaded settlement is counted only once entities have loaded around its bell, and a census finding under half the villagers of one from the last 3 days is treated as incomplete and retried.
- **The census keeps a roster (2026-10-14).** Each census records every resident: where it was seen, its bed, its trade and level, whether it could walk freely, whether a baby. A resident not seen this time still counts if its chunk isn't loaded (asleep in a cottage past the market) or it's out with a caravan; one whose chunk is loaded and isn't there has gone; one unseen for 10 days is dropped. Babies count as residents (they need beds), not as free workers. Rounds load where residents sleep and were last seen as well as the market, up to 36 chunks. Measured over 25 days: no town's count dipped, and every count matched its births; before, a town of 6 was sometimes counted as 3 or 4.
- **A Merchant with few slots manages lockers with trades switched on first**, then the nearest.
- **Measured** (`caravan_network.py`, two runs): with five settlements left unloaded for 8 days, every farm produced 16 carrots a day, territories grew to 3-4 hexes, the hub banked 3,580-3,975 spurs, and the route crossing the toll town paid it 27-31 spurs.

## Mood and comforts (built 2026-10-14)

- **Comforts are what towns spend spare money on.** A kind of comfort is an item tag under `enterprise:comforts/`: to start, treats (cookies, cake, pumpkin pie, honey), clothing (leather armour), furnishings (carpets, candles, paintings, flower pots, lanterns) and reading (books). A pack adds a kind (tea, cigarettes, another mod's finished clothes) by adding a tag.
- **Villagers use them up.** Each villager uses a tenth of an emerald's worth of each kind a day from the town's warehouse, at local prices, cheapest goods first and whole items only (a cake covers several days). A ration that can't be met is gone without, not owed.
- **Mood is Civilization's happiness.** Each kind fully supplied adds 1. Every 8 villagers take 1 away (bigger towns want more), each villager without a bed takes 0.5, and wholly unpaid wages take 2. Mood closes half its gap to that every 3 days. A content town's farms yield a tenth more per point of mood (between half and one and a half times) and it gathers a quarter more culture per point (between a quarter and double), so it grows its territory faster. `/enterprise territory <bell>` shows mood, what's met and the effects.
- **Towns buy comforts by caravan.** A town aims to hold 4 days of each kind (villagers x 0.1 x 4 emeralds' worth), buying whole items, at least one when short. It pays only from money beyond its wage float and 3 days of guards' pay. A caravan loads comforts from the hub for what the town wanted at its last stocktake (taken at each caravan visit), half again over, before it fills the rest of its cargo.
- **Towns keep their trading capital (2026-10-14).** Before spending on comforts, homes, children or works, a town keeps back its wage float, 3 days of guards' pay and 2 days of what its lockers actually sell (averaged, at local value). A town that spent it couldn't buy the goods that earn its money and stayed poor. Caravans judge what comforts a town wants from its last stocktake less what its villagers will have used by arrival. Comforts are never eaten as road food or rations.
- **Measured over 40 days with everything on:** see `docs/experiments/full-study-2026-10-14.md`.
- **Measured** (`network_study.py --comforts`, 20 days, the hub stocking one shulker of comforts and its books): the three supplied towns reached mood +3.3 (all four kinds met; farms x1.33, culture x1.83). The two towns without a route sank to -0.6. Town money stopped climbing: S1 peaked at 125 emeralds on day 10, when the hub ran out of trade goods, and fell to 96 by day 20 while it kept buying comforts.

## Homes and children (built 2026-10-14)

- **Content towns grow.** When a town is counted while loaded, it works toward a home if every bed is taken and its mood is at least 0.5, and toward a child if a bed is free and its mood is at least 1: about one of each every 4 days. The ledger keeps the progress between counts.
- **A home is a real cottage**, 8 emeralds from money beyond the town's reserve: 5 x 5, cobblestone floor, plank walls with log corners, a south door, two windows, a slab roof, a bed and a lantern. It goes on the nearest flat, open, dry ground 8 to 32 blocks from the bell (inside the census's 48), with a block of clearance and never on a field.
- **A child costs 36 food points and 3 emeralds** (food beyond the caravans' par, money beyond the reserve) and is born as a baby villager by the bell. Babies only age while their chunk ticks, so a child loaded back in is aged by the time it was away.
- **Mood caps growth.** Every 8 villagers take 1 from mood and a child needs mood 1, so a town supplied with all four kinds of comfort stops growing around 24 villagers. A pack that adds kinds of comfort lets towns grow bigger.
- **Measured** (`network_study.py --comforts`, 20 days): the three supplied towns built 1-2 cottages each and had 1-2 children, growing from 5 villagers to 6-7; the first child had grown up by day 20.

## Work for idle hands, and fields (built 2026-10-14)

- **Babies never work; trained adults crew caravans first.** When a town is counted while loaded, it trains its squad first (see Caravans); only when its squad isn't short does it find work for an unemployed adult.
- **Work stalls.** An unemployed adult on the town's roster gets a stall: the workstation of the trade, not already in the town (by its roster), whose villagers would buy most of what the town has to sell (4 days of its farms' output and its warehouse's stock, at local value), with a chest against it so it's a locker. 6 emeralds beyond the reserve, on a 3 x 3 patch of open ground at least 6 blocks from the bell. The villager takes the job at once (waiting for it to walk to the workstation is chancy in an unloaded town), and its trades that buy goods for emeralds are switched on. Trades: farmer (crops), librarian and cartographer (paper and books), fletcher (sticks, flint, string, feathers), fisherman, shepherd (wool), cleric, butcher, leatherworker, mason, armourer, toolsmith, weaponsmith. Nothing to sell, no stall.
- **Fields for farmers with hands to spare.** After a census while loaded, if the town's farmers could work 54 more plots than they do (32 a level each), it plants a 9 x 9 field on open ground in its home hex, for 4 emeralds beyond the reserve, of what its market wants: the crop its lockers can take the most of a day by value, counting sugar cane as the paper it's pressed into; carrots if nothing sells but its food is under twice the caravans' par. Crops grow on farmland round a water block in the middle; cane along three water channels.
- **The warehouse presses cane into paper,** one for one as at a crafting table, at each touch, while paper is under its par and some locker buys paper. So a town with Librarians or Cartographers and a cane field sells its own paper.
- **No town is stranded (2026-10-14).** Each census notes the village's demand: every resident's trade that buys one good for emeralds, switched on or not (its cost times its uses, twice a day). Fields grow the crop with the most unmet demand by value (cane counting as paper); a crop the village wants and doesn't grow at all gets a field even when the farmers are busy (a farmer always plants for its own trades), up to two fields a farmer level. Farmers tend the crops the village buys first, the most valuable demand first, then the rest. The Merchant mediates: every locker trade in the village that buys a crop the town grows (or what its trades make of one) is switched on. Sweet berry bushes are a crop, for Master Butchers.
- **Trades convert raw goods (see `Workshop`).** Librarians and Cartographers press sugar cane into paper (1 for 1) while paper is under par and some locker buys it; Farmers bake wheat into bread (3 for 1) while food is under twice the caravans' par. A conversion runs only if a villager of that trade lives in the town.
- **Measured** (`standalone_town.py`, 20 days: one settlement, no caravans, nothing switched on by hand): the village's demand was paper (its Librarian and Cartographer), so it planted sugar cane, its Novice farmer worked the cane first (16 a day, pressed into paper), and it traded on its own, 10 trades and +7 emeralds in 20 days. Before, the same town grew only carrots nobody in it bought and never traded. With no comforts it stays at mood -0.6 and doesn't grow: stagnant, not stranded.
- **Measured** (S1 staged with four spare farmers, an unemployed resident, and string and coal no one in town bought, 6 days): it put up a Fisherman stall (fishermen buy string and coal) and planted a 9 x 9 sugar cane field for the paper its Librarian and Cartographer buy; its farms then grew 27 cane a day beside 40 carrots. In the plain network study no town needed either: caravans bring only what a town's lockers take, and one Novice farmer is busy with the starting field.

## Herds (built 2026-10-14)

- **Towns keep herds on the ledger:** sheep, cows, pigs and chickens, counted per town (fractions carried) and tended at every touch. A town's first Shepherd brings 3 sheep and its first Butcher 2 pigs and 2 chickens, once only.
- **Animals eat from the warehouse:** sheep and cows a quarter of a wheat a day, pigs a quarter of a carrot, potato or beetroot, chickens a tenth of a wheat. Their feed joins the village's demand, and comes first: a field for feed the town doesn't grow enough of is planted before anything that sells, and farmers tend feed before other crops. Fed (90% or more), a herd grows by 15% a day up to 16 animals a pasture; hungry, it dwindles by up to 10% a day.
- **What herds give.** A Shepherd shears half a wool a sheep a day; a Butcher keeps 4 of each kind to breed from and slaughters half the rest a day for meat (1.5 mutton, 2 beef, 2 pork, 1 chicken each) and leather from cows. So a Butcher's chain needs farmers for feed and a Shepherd's flock for mutton; a Master Butcher's sweet-berry trade makes berries a crop the village asks its farmers for. Shepherds weave wool into carpets (2 for 3) while the town is short of furnishings, a comfort.
- **The pasture is real:** a fenced 9 x 9 with a gate on open ground in the home hex (4 emeralds), fenced once the town keeps animals. Its animals are a view of the ledger: when the town is counted while loaded, missing animals are spawned in it and ones the ledger no longer has (slaughtered, starved) are removed.
- **Measured** (`standalone_town.py --herders`, 20 days, no caravans): the town fenced a pasture and planted wheat first; its herds, which had started to starve, recovered (sheep 1.5 to 4, chickens 1 to 4); the Butcher held pigs near the breeding stock; and the Shepherd's carpets met the town's furnishings in full with no caravan. Before feed came first, the town planted two cane fields and the flock starved.

## Leatherworkers and Fishermen (built 2026-10-14)

- **Leatherworkers** bring 2 cows (once, like Shepherds' sheep), cull the cows beyond the breeding stock for hides (and beef) as a Butcher would, and make leather boots (4 leather a pair) while the town is short of clothing: a town can now dress itself. On a caravan they carry **packs**: 9 more stacks, taken along when the destination wants more than a wagon holds.
- **Fishermen are boat captains.** A caravan with a Fisherman aboard can sail water, each water hex costing 0.8 of a hex of open ground; the squad plans both ways and takes the boat when it's the only way or the cheaper one. Boats slip past bridges and their toll posts. **A fishing fleet:** on water, every Fisherman to spare goes (up to a full squad of 4), and each catches 8 cod on each water hex sailed, carried as cargo. **Towns buy food when short:** under twice the caravans' food par, a town buys food from a caravan to get there, so a fleet sells its catch.
- **Docks.** A town with a Fisherman builds a pier where its land meets water (four planks out, a lantern at the end, 4 emeralds), and its Fishermen land 4 cod a day per level, which they smoke into cooked cod while food is short. Water is no longer land a town can't use.
- **A squad is never more than 4,** however many trades would help. **A town's own shopping trip goes only if the shopping is worth twice its road food:** the toll town had spent 393 spurs of road food to buy 68 spurs of comforts.
- **Measured** (`network_study.py --river-bridge --fleet`, 12 days, three trained Fishermen and a Leatherworker added to the hub's squad): before any bridge existed, a Cartographer and three Fishermen sailed the river to S3, which traded from day 3 (it made none until the bridge before); that route caught 312 cod, the road to S2 152.

## Walls (built 2026-10-14)

- **A wall turns a town's land and its masons' wages into an improvement.** A content town (mood at least 0.5) with nothing that pays better to build (no bridge worth it, see Bridges) and 12 emeralds to spare beyond its reserve starts a wall. Masons build it as they build bridges: two at 1 emerald a day, with stone quarried from the town's land at 16 blocks a day per hex, one project at a time.
- **It's a real wall:** a ring 40 blocks from the bell (past the homes, inside the census), 3 high, with gates 3 wide at the four points of the compass. It's of what the land offers: a log palisade on grass and dirt (spruce in taiga and snow), sandstone on sand, cobblestone on stone. Where the ground is a field, a path, water or something built, or the air above isn't clear, it leaves a gap, so villagers' paths become gates. It goes up in the world a quarter at a time as the masons lay it, loading its chunks to do so.
- **A walled town feels safe:** up to +1 mood and 2 culture a day when the wall is whole, in proportion to how much stands. Defence against raids comes with raiding.
- **Measured** (`network_study.py --comforts`, 25 days): the three content towns each started a wall about day 8 and finished it about day 20 for 24 emeralds of wages, raising all 252 columns on the flat test world. With the wall's culture and good mood their land grew to 10-11 hexes (7-8 without walls). Their money fell to 28-70 emeralds as walls, homes and comforts took it.

## Contracts (built 2026-10-14: standing orders, shipping terms, shop contracts, sourcing)

The foundation for contracts and, later, futures. Built to grow:

- **A contract is a list of promises between parties, on a schedule, at a locked price.** Parties are Merchant's Desks for now (players and factions later). Each promise is a term of a registered kind (`Term.TYPES`), so later eras add kinds without changing the book: delivery (built), and later passage through a toll town for a fee, escort, villagers lent, insurance against a raid. The schedule is a start, an interval and a count of installments. Contracts save their terms by kind; one that can't be read is skipped, not fatal.
- **News travels with carriers.** A party acts on a contract only once it holds a copy. Drafted at the destination, the destination holds one at once; the home desk gets its copy when the caravan that drafted it is home, and only then is it in force. Later carriers (telegraph, trains) will bring copies faster; nothing else needs to change.
- **Standing orders (built).** A travelling Merchant (a Merchant in the squad) that sold a good well drafts one with the resident Merchant: the destination takes, each round trip (at least a day), what its lockers actually sell in one, in sixteens, at this visit's bargained price. Ten installments, or as many as the seller can see it can cover (what's aboard and what its home warehouse held), at least three. One contract per good per pair. The resident won't promise more than it sells; the travelling Merchant won't promise more than it has.
- **Caravans settle them.** Goods owed under contracts in force go first into cargo. At the destination they're delivered before any other selling, whatever the warehouse's par (but keeping its free slots), and paid at the locked price whatever the market says now: a forward contract. A buyer that can't pay 90% loses trust.
- **Trust between each pair of parties** starts at a half: each installment settled in full by its grace (2 days) raises it by 0.05, each missed lowers it by 0.2, three misses break the contract, and below 0.3 the two draft nothing new. `/enterprise contracts [desk]` shows each contract's terms, copies, state, record and the parties' trust.
- **Measured** (`network_study.py --comforts --inject 15`, 30 days): the spring's first paper order to S1 ran its ten installments in full (trust to 1.0) and its later orders grew with the towns' sales (272, then 384 paper a delivery); the hub's early order broke when its finite stock ran out (trust 0.10, nothing new drafted). Towns stayed healthy (S1 130, S2 150 emeralds by day 30). Before orders were sized to what the buyer sells, and kept off the warehouse's free slots, they flooded S1's warehouse and it nearly went broke.
- **Next, planned:** contracts as paper you can hold and trade (assignable: a futures market, priced by the gap between the locked and the market price); deposits held in a Blaze Banker as escrow when trust is low; passage agreements with toll towns; contracts drafted by players at the desk; a Contracts tab on the desk screen.

## Services (built 2026-10-14)

What each trade does for a town: its own, or the one its caravan reaches. Built to grow, like contract terms:

- **A service is registered by name** (`Services`): the trade that does it, where (at home, abroad, or on the road), what it does in plain words, whether the town served wants it now, and doing it. Later eras add theirs (a Toolsmith mending the wagon, an Armorer fitting guards) without changing who runs them. Each is done by the most skilled of its trade who is there: at home, a resident not away; abroad, a squad member. Skill (level) sets how much.
- **Abroad, the town served pays** from its money beyond its reserve, into the caravan's purse, and each paid service counts a tenth of a kept installment toward the two towns' trust (`Services.TRUST_PER_SERVICE`). **Who goes is decided by the services:** before a trip, each trade at home is asked whether its services would be wanted there, from the ledgers alone. Clerics and Farmers now go along, but never lead (towns don't train them for squads).
- **Built:** a Merchant's deals (haggling, standing orders); a Shepherd's droving; a Cartographer's survey of the road and **map swap** (each town learns of the markets the other knows; the one that learns more pays 0.25 an extra market; ours rides home with the squad); a Librarian's copy of the books and **bookbinding** (3 paper and a leather make a book while the town is short of reading, at home, or abroad for a town without a Librarian at 0.05 a book); a **Cleric's blessing** (+0.5 to where the mood is heading: a visiting Cleric blesses a town without one for 2 days and a day a level at 2 emeralds, a resident blesses its own town while it stays); a **Farmer's help with the harvest** (a day's harvest from fields no one tends, as many plots as a Farmer of its level, for the town's farmhands' share of its worth; only a Farmer its own fields can spare goes); a Cartographer at home **charts** the country, learning of markets within 800 blocks every 10 days. On the road: a Leatherworker's packs, a Fisherman's boat and catch.
- **The census keeps what untended plots would yield** (fields beyond what the farmers can work), which is what a visiting Farmer brings in.
- `/enterprise services` lists every trade's services; `/enterprise services <desk>` shows what its town's trades do, what's been done for and by it, and the latest. `/enterprise routes` shows what each route earned from services.
- **Measured** (`network_study.py --comforts --services`, 20 days, a Cleric and a Farmer added to the hub's squad, against the same run without): the Farmer helped S1 with its harvest 19 days running for 22.8 emeralds, the Cleric blessed S1 and S2 five times each and S3 once (2 emeralds a time), and two map swaps were paid for. On day 20 the served towns were happier (S1 +4.3 against +3.2, S2 +4.5 against +4.1, S3 +4.1 against +3.8); S1 and S3 were richer (70 and 34 emeralds against 20 and 6: the extra harvest sold), S2 poorer (25 against 46: it paid for its blessings). Runs differ by a few emeralds by chance, so read these as directions.
- **Not yet:** services on the desk screen; a Cleric's shrine, potions and healing; Farmers sowing new fields abroad; Librarians copying enchanted books.

## The metal chain (built 2026-10-14)

Mine, smelter, smiths, and what the iron is for. All of it is services (see Services), so each trade also does its work abroad for a town without that trade, for a fee an item.

- **A Toolsmith digs a mine.** A town with a Toolsmith digs a mine head near its bell (a 3 x 3 cobblestone collar, a laddered shaft, a lantern; 6 emeralds beyond its reserve). Each Toolsmith at home brings up 2 raw iron and half a coal a day per level, half again with pickaxes.
- **An Armorer smelts:** 8 raw iron and a coal make 8 iron ingots (abroad, 0.02 an ingot, for a town with ore and no Armorer).
- **Smiths make kit from the iron,** for whoever lacks it: a Toolsmith hoes for the farmers and pickaxes for the miners, a Weaponsmith swords for the guards, an Armorer chestplates for them. Iron a locker wants is kept to its par. Kit wears out: a hoe or pickaxe a worker every 16 days (twice as long while a Toolsmith mends them, at home or for a few days after a visit), a sword every 20, a chestplate every 40.
- **What kit does:** with hoes for all, farmers tend half again as many plots; with pickaxes, miners bring up half again as much. **Armed guards** let a town ask up to 95% of what going around its posts would cost (from 80%); **armoured guards** let its toll go up to 35% (from 25%). In proportion to the share of guards equipped. Raids will test the same kit later.
- `/enterprise territory` shows the mine and each kit's share; `network_study.py --metal` sets the chain up.
- **Measured** (`network_study.py --river --comforts --metal`, 20 days, against the same without): S1's Toolsmith dug a mine, visiting Armorers (the hub's and T's) smelted its ore 9 times, and it forged a hoe and a pickaxe from the iron; S1 brought in 693 items against 403 (ore, and a farmer tending half again as much). T, with the whole chain, armed and armoured both bridge guards, set its toll at 35% and collected 692 spurs of tolls against 458. T's three extra smiths crowded its beds (8 villagers, 5 beds), so its mood was worse: a real cost of taking on trades without homes.
- **Not yet:** ore depending on the land (stone and hills richer than plains); an escort of armed crew on caravans (with raids); iron tools and armour as goods towns buy from each other.

## Labour, Masons, Fletchers and masters (built 2026-10-14)

The rest of the trades, as services (see Services), and a master's service for each.

- **Hands without a trade** (Nitwits, and adults waiting for work) each work one of the town's hexes and gather what its biome gives: logs, sticks, apples and string in woods; spruce and berries in taiga; sand and cactus in deserts; clay, cane and ink by water; stone and flint in hills; seeds, flowers, sticks and flint on open ground. These are the goods no trade makes. A Nitwit gathers half again as much. Gathering stops at 16 of a good the town doesn't use (64 of food, comforts and what its lockers buy), and while the warehouse is short of free slots: it must not crowd out the caravans' goods (it did, in a one-chest town, until this rule).
- **Militia:** hands stand watch at the town's posts first, fed but unpaid. **Porters:** when a caravan's cargo is more than the wagon and a Leatherworker's packs hold, hands go along carrying 9 stacks each, and walk home once it's delivered.
- **Fletchers hunt** the town's land by biome (rabbits and hides in desert, snow and taiga; beef, pork, fowl, feathers and leather elsewhere), a quarter as much without a bow, and **make bows** of sticks and string, for themselves and then for the guards: spare bows arm guards as swords do. Kit for the town's own hands comes before what its lockers sell.
- **Masons build** (`Buildings`), a money sink with something to show: a square (+0.5 mood), then halls as its own skill and the trade's allow: a temple (Cleric 3: blessings count double, children twice as often), a library (Librarian 3: 3 culture a day), a smithy (a smith 3: mine +50%, kit lasts a quarter longer), a watchtower (Fletcher 3: half the guards count as armed), stone docks (Mason 4, Fisherman 3: twice the catch), a caravanserai (master Mason, Merchant 3: word of the town carries 800 blocks, 1 culture a day). Materials up front (logs in the warehouse stand in for timber), wages while building. A town builds only when content and holding twice the whole cost beyond its reserve: at first a town built itself out of its comforts. A visiting Mason lays a day's work on a town's works for a day's wage.
- **Masters,** most with another trade's help:
  - Cleric: enchants the town's kit (10 levels as Journeyman, 20 Expert, 30 Master), laying on an enchanted book's enchantment, or at the table with lapis (from a deep mine) and an emerald of bottles o' enchanting per 10 levels. Enchanted kit works better (each level +15%, up to half again) and Unbreaking makes it last (1 + level) times as long.
  - Librarian: copies an enchanted book with a book and a lapis, while kit wants enchanting.
  - Toolsmith: sinks the mine deep, with a Mason to shore it: lapis too.
  - Armorer: builds a blast furnace, with a Mason: twice the iron per coal.
  - Weaponsmith: keeps an armoury, with an Armorer: guards' kit lasts twice as long.
  - Farmer: plants flower beds on the Mason's square (8 flowers, which the hands gather): +0.5 more mood.
  - Shepherd: weaves banners (6 wool and a stick furnish like twelve carpets).
  - Butcher: smokes meat with coal while food is short.
  - Leatherworker: saddles of leather and iron; a saddled caravan rides a third faster.
  - Cartographer: charts twice as far. Fisherman: half again the fleet's catch. Fletcher: a hunting party, twice the game. Mason: the caravanserai and stone docks.
- `/enterprise territory` shows hands, militia, buildings, hunters with bows and guards armed.
- **Measured** (`network_study.py --river --comforts --trades`, 20 days, against the same without): S1, given two Nitwits, a Mason, a Cleric, a Fletcher and a master Farmer (with beds), built a square, got flower beds and its Cleric's blessing daily, grew to 11 villagers (5 without) at a better mood (+3.3 against +3.1) and brought in 2,242 items against 403 (gathering and hunting); it had 20 emeralds to the other's 43, and a temple waits on its savings. Its one chest filled, so its hands stopped gathering, and its Fletcher never got the string for a bow. T's two Nitwits stood watch at its bridges unpaid and its spare bows armed both guards; it kept 60 emeralds to 44.
- **Not yet:** a master Farmer sowing fields abroad; arrows; dyes for banners; buildings on the desk screen.

## The master Merchant and players' shops (built 2026-10-14)

A master Merchant (level 5) at a desk deals with the Numismatics vendors (shops) within 16 blocks of it, each minute its market is loaded, as agent and market maker. Players use Numismatics' own shops; nothing new to learn.

- **A player's buying shop is a standing order:** its filter is the lot, its price what it pays, its bank card the purse. The Merchant fills it from what the warehouse can spare beyond its par, at the shop's price (if that's at least 90% of the goods' worth), and banks the coins. What it can't fill stays an **open order**, and the region works on it: caravans bound for this desk bring it (their cargo now covers orders there as well as lockers), and this desk's own caravans buy it up as surplus on their way home.
- **An order for enchanted goods is a commission,** worked by the town's master Librarian and a Cleric (3 or better), a few steps each minute: for each enchantment the order wants, a book at its level, made by copying a book (a book and a lapis), combining two at the same level into one a level up (an emerald of bottles o' enchanting), or, if no book has it, the Cleric enchanting books at the table (lapis and bottles, at 10 to 30 levels by its skill) to see what comes. Then the books are laid on the plain good (an emerald each) and it's handed over. A level beyond the enchantment's maximum can't be made, and the readout says so.
- **A player's selling shop is supply:** the Merchant buys from it what its orders and its lockers want, at up to the goods' worth, and up to half again over worth for goods it has posted as wanted.
- **Signals:** what the desk wants (open orders, its lockers' shortfalls, commissions waiting on a base good) that no market it knows has at its last stocktake, and no player's shop here sells, is posted with the premium it will pay: work for players to bring from another region or make.
- **A shop touching the desk is the town's own:** the Merchant sells its biggest surplus there (no food, no money, nothing ordered) in lots of 16 at 10% over worth, keeps a stack on the shelf, and banks the takings.
- `/enterprise market <desk>` has the Merchant look now, then shows its trade with the shops, open orders, commissions and signals. `network_study.py --master` makes the hub's Merchant a master.
- **Verified:** game tests on real vendor blocks and the Numismatics bank (an order filled, an order it can't fill posted to the region and as a signal, a stall bought from to fill an order at a profit, an Efficiency II pickaxe made from one Efficiency I book by copying, combining and laying on, the town's shop set up and its takings banked, an order riding out with a caravan). Live on the headless server: a vendor placed by the hub's master Merchant was set to sell its compasses within the minute. Not seen: a player using a shop the Merchant runs, through Numismatics' screen.
- **Not yet:** the desk's screen showing orders and signals; a player drafting a contract at the desk (the Contracts tab); the town's own shop buying signals (Numismatics pays buy orders from a bank card, which a town would need).

## Shipping terms, shop contracts and supply chains (built 2026-10-14)

- **Every contract has shipping terms** (`Incoterm`, after the trade's own; tolls stand for duties):
  - **EXW, Ex Works:** the buyer collects at the seller's. Its caravans to the seller carry the money and load what's owed, paying the locked price there. An installment the seller had in its warehouse (at its last stocktake) but nobody came for is the buyer's miss.
  - **CPT, Carriage Paid To:** the seller's caravans carry it, but it's the buyer's once on the road: it counts as delivered when it leaves. The buyer pays the tolls on top.
  - **DAP, Delivered At Place:** the seller carries it and bears the road: it counts only once it's there. The buyer pays the tolls on top: what the posts on the road will take of the payment going home.
  - **DDP, Delivered Duty Paid:** the seller carries it, bears the road and pays the tolls. The travelling Merchants' standing orders are DDP, as they always were in effect.
  - A price is set at the seller's, then carriage is added where the seller carries, and tolls where it pays them. FCA, CIF and CIP wait on rail and on raids (insurance).
- **Misses are the seller's or the buyer's:** goods not sent or not had are the seller's; goods set aside and not collected, or not paid for, the buyer's. Three by either break the contract, and both cost the pair's trust.
- **Copies ride with any caravan between the parties,** each way: a contract drafted at one desk is in force once a caravan between the two has carried it to the other. Before, only the drafting caravan carried them.
- **Goods made to order:** a delivery can name enchantments, and only goods enchanted just so count.
- **A player's buying shop now gets a real contract** with the master Merchant's desk, drafted in person at the shop (so in force at once), Ex Works (the shop is at the seller's market). It runs as many installments as the shop's purse would pay for, up to 10, one a day (commissions every two). The price is the shop's, locked. Each installment is delivered as it falls due and paid from the shop's card at the locked price; if the card can't pay, that's the buyer's miss. The commissions of the earlier round are worked under these contracts.
- **Supply chains, back to back:** when a desk can't fill what a contract wants (a player's order, a commission's base tool, lapis or books), its Merchant contracts a supplier: the nearest market it knows that held the goods at its last stocktake, with a road between them. The terms are Ex Works if its own caravans go there (they collect), else Delivered At Place (the supplier's caravans bring it). The price is the worth plus 10%, then carriage and tolls by the terms. A town sells at most half its kit. One open contract per good. With no road, the want stays a signal: a gap a player could fill.
- `/enterprise contracts` shows each contract's terms, goods, what's on the road, and whose misses.
- **Verified** in game tests: Ex Works goods collected and paid for by the buyer's caravan, a contract's copy carried to the far end putting it in force, Carriage Paid To goods on the road not a miss where Delivered At Place ones are, uncollected Ex Works goods the buyer's miss, prices by terms, a shop's contract delivering a lot a day and completing, sourcing a supplier Ex Works. Live, on a 10-day network, the spring's and hub's standing orders ran as before (three contracts, none missed).
- **Next:** contracts drafted by players at the desk (the Contracts tab), so a player can buy Ex Works and carry it themselves; escrow when trust is low; passage agreements with toll towns; contracts as paper to trade.

## Crates and bills of lading (built 2026-10-14)

- **The crate** (`enterprise:crate`) is a wooden box of nine stacks with a tenth slot for one bundle of documents: a common shulker box. It's crafted from 4 planks and 4 sticks (planks at the corners, sticks between, the middle empty). Broken, it keeps its goods and papers. It doesn't go inside another container. A packed crate is one of a kind (stacks to 1); empty ones stack to 16.
- **Documents** (`enterprise:documents`, from 2 paper and a string) hold papers, each a kind and its particulars, so later eras add kinds freely:
  - the **bill of lading**: shipper, consignee, goods, contract, terms, route, day issued; the receipt for the goods and the title to them;
  - a **copy of the contract**;
  - bills of exchange to come.
- **Shipments under contracts go in crates with their papers:** a bill of lading and a copy of the contract in each crate.
  - The shipper uses its own empty crates first, and pays an eighth of an emerald for each new one (conjured for now; later a carpenter's, with nails and wood).
  - With no crate and no money, goods go loose, without papers.
  - A crate takes the cargo space of what's in it (at least one stack).
- **The consignee's Merchant opens crates consigned to it,** whoever brought them:
  - It takes the contract copy as its own.
  - It delivers the goods under the contract the bill of lading names.
  - It keeps a stack of the empty crates for its own shipping, and hands the rest back to the caravan that brought them, as returnable crates.
- Ex Works goods are crated by the seller, made out to the buyer, and opened when the buyer's caravan is home.
- **A player can carry a crate:** put it in the consignee's warehouse chest and the Merchant opens it and takes delivery. The payment is owed to the seller until a caravan between the two carries it, whichever way it runs.
- **Measured live** (`network_study.py --comforts --inject 3`, 8 days): contracted goods shipped in crates and were delivered under their contracts. At first the empty crates filled S1's one-chest warehouse, a slot each: 80 sticks rode back and forth undelivered and that contract broke, and S1 traded 12 times. With empties stacking and returning, S1 traded 44 times and its paper contract delivered every installment.
- **Not seen:** the crate's screen and textures in a client.

## Bills of exchange (built 2026-10-14)

- **A buyer short of coin pays with paper:** a bill of exchange drawn on its own treasury, for the debt and 2% for the wait, due in 4 days. It happens when a caravan collects Ex Works without the money, or a buyer can't pay on delivery, where the two trust each other.
- The bill is a document, filed with the payee's other bills on the same drawee (one bundle per drawee).
- **It's paid only when presented, by a caravan between the two:** the drawee's caravan arriving where bills on it lie pays those due from its purse (it carries the money for them). The holder's caravan carries bills on its destination there and brings the coin home.
- Bills not yet due go back. Unpaid 4 days past due, a bill is dishonoured, and the pair's trust falls.

## War footing (built 2026-10-14)

A well-funded sovereign shifting the region onto a war economy, through the region's own institutions rather than conjured demand.

- **The capital and the treasury:** the sovereign's capital is a desk whose Blaze Banker is the royal treasury. It mints coin into it each day. Minting debases the spur: a shulker of emeralds' worth minted (27,648 spurs) doubles the spurs an emerald fetches, working its way out with a 30-day half-life.
- **Mobilising:** it builds a stockpile of kit in the capital's warehouse:
  - swords (wood, stone, iron, diamond), chestplates (leather, chainmail, iron, diamond), shields, bows, arrows;
  - the capital's master Merchant contracts towns for them at 1.5 times their worth (2.5 times at war);
  - it buys gold through a Numismatics shop by the capital at 3 times its worth.
- **Contracts name the tier, not the enchantments.** Any iron sword counts. Bonus clauses pay for quality without requiring it: Unbreaking 1 and Sharpness 2 or better earns 25% more on a sword; Protection 2 earns 25% more on armour.
- **What no town can supply is posted as wanted, and towns respond:**
  - they put idle hands to the trades the war calls for (a Weaponsmith for swords; iron goods want a Toolsmith's mine and an Armorer first);
  - they make what they've contracted to sell;
  - their Merchants switch on their own villagers' trades to buy it.
  - Smiths trade their way up the levels at their stalls. Diamond gear is a master smith's, sold for emeralds.
- **At war, levies:** a levy contract with each town the capital's caravans reach raises soldiers: its idle hands and a fifth of the rest, one to four. Idle hands go first, then a trade it has others of, then the least skilled; never a child, nor its Merchant. The recruits ride to the capital with its caravan.
- **Campaigning:** each day a soldier draws what kit it lacks from the capital's warehouse, the best there is, and wears it out (enchanted with Unbreaking, slower). It is paid 0.25 emeralds a day, owed to its town and carried there by the capital's caravans.
- **The fallen:** a soldier may fall, 2% a day before kit. A weapon, armour and shield cut it, better tiers more, and enchantments suited to the enemy more again: Sharpness and Protection against a faction, Smite against the undead, fire against the Nether. The fallen are gone from their town for good, and it grieves: half a point of mood each, for 10 days.
- **At peace** the soldiers walk home and the minting stops.
- **An army eats:** 8 food points a soldier a day, from the capital's warehouse. At war the sovereign keeps four days' rations stocked, sourced like the kit; Farmers sell bread, and towns supply what food they have beyond their caravans' needs. An unfed soldier runs half again the risk, and deserts at 10% a day (walking home, its kit lost). **Good food is good morale:** the army eats its heartiest food first, and a meal's heartiness (its saturation per point of food above plain bread's: cooked meat, stews, golden carrots) makes it fight up to a quarter harder, fall 15% less, and desert half as readily when short. The stockpile holds cooked beef and pork as well as bread; Butchers sell them.
- **The tally** (`/enterprise war tally`): by town, soldiers raised, fallen and deserted, soldier-days (and hungry ones), strength (soldier-days weighted by kit and its fitness for the enemy), kit delivered and emeralds earned; in all, the kit issued by kind and enchantment levels, food eaten, gold bought, coin minted, the spur's rate.
- **Measured** (`network_study.py --comforts --war 1,31,71`, 90 days: mobilising 30, at war 40, peace 20; three towns on the capital's roads):
  - **The region barely moved onto a war footing.** One town, S2, answered: it grew from 5 to 13 villagers and took up Leatherworker, Toolsmith and Weaponsmith stalls for the war's wants. Its Fletcher, through its own villager trades, supplied all 280 arrows delivered.
  - **Nothing else was made:** swords, armour, shields, bows and gold went unmet all 90 days.
  - **The army was raised but never armed:** 4 soldiers, 3 fallen, no kit issued.
  - **The treasury piled up 67,600 spurs it couldn't spend,** while minting took the spur from 16 to 29 an emerald.
  - **Why:**
    1. Trust froze contracting: small towns missed early installments (0.2 trust each), and by the war the capital's trust with S2 was 0.12 and with S1 0.00, below the 0.3 needed to draft. S2's Weaponsmith never got a sword contract.
    2. Towns of five or six have no hands to spare.
    3. No town completed the metal chain (mine, Armorer), so no iron, no gold.
    4. The sovereign's money had no way to reach a town before it could deliver.
  - **What's missing:** advances against contracts (money up front to hire, build and train), patience for a supplier ramping up (lead time, a softer first miss), and a way for coin to become people at work. That is the war finance to come: loans, bonds, taxes.
- **The long runs got 10 times faster** (2 seconds a game day; 90 days in 3 minutes): the study now keeps one RCON connection open, and reads the debug log from where it left off. The tick warp itself was never the slow part.

## Trust, advances and the capital's port (built 2026-10-14)

What run 1 of the war lacked: patience with new suppliers, money before production, and metal to train smiths on.

- **Trust is a track record**, not a meter. Each pair keeps installments kept and missed; trust is `(kept + 2) / (kept + missed + 4)`, a half for strangers.
  - **Consistency builds it:** each kept installment adds less as the record grows, and one bad week doesn't undo a long record (20 kept, 2 missed: still 0.79).
  - **Disruptions are forgiven:** a miss's weight halves every 20 days; kept installments every 120.
  - **New suppliers ramp up gently:** until a pair has kept 3 installments, a miss counts a quarter.
  - **Part deliveries count in part:** 27 of 30 is 0.9 kept. Only an installment short by half or more counts toward breaking the contract (3 such break it).
  - **Trust never freezes trade:** below 0.3 the two still deal, on trial terms (a lot of 4, nothing up front), so a record can be rebuilt. A bill paid when presented counts half a kept installment, a dishonoured one a whole miss; a paid service abroad a tenth.
  - `/enterprise trust` lists every pair's record.
- **Money before the goods: advances.** A contract to make goods carries an advance term: a third of its worth paid up front, carried by the buyer's caravan with the copy (coin from its purse, the rest a bill of exchange on the buyer). The seller works it off at the same share of each delivery's price. If the contract breaks, what isn't worked off is owed back and collected by the next caravan, as far as the seller can spare.
- **Work to make comes in small installments:** four, a day apart, after a three-day lead, rather than one lot of 16 due in two days. Steady deliveries build the record.
- **Inputs follow contracts.** A town contracted to make swords wants the iron for them: caravans carry a contract's inputs first, and the town buys up to what it needs. (With more wanted goods than cargo slots, a caravan used to carry nothing at all; it now carries a stack each, inputs first.)
- **The capital and its port** (`/enterprise war capital <desk> <spurs a day>`): a capital can be named in peacetime. Its treasury takes a peacetime revenue (minted for now; taxes will replace it), and its port opens a **metal exchange**:
  - it aims to hold iron, coal and flint (192/192/96) and diamonds (12), twice that while mobilising, three times at war;
  - ships bring what it lacks, three days at sea, paid from the treasury at 1.5 times worth;
  - it posts a bid and an ask a tenth either side of a middle price that rises (up to half again) as its stock runs short. The capital's caravans sell its metal to towns at the ask and buy towns' metal (gold too: the war chest) at the bid. Its bids stand as signals, so towns take up mines;
  - `/enterprise metals` shows its book.
- **Training smiths.** A villager the Merchant puts to work is its apprentice: as it levels, its new buying trades are switched on too (coal, iron, flint, diamonds). A smith with nothing to buy in the warehouse (a Novice Weaponsmith may buy nothing) has its cheapest work bought instead, while the town can spare it. At master its buying trades go off: its metal is kept for its contracts.
- **Fixes the runs turned up:**
  - Villagers in a town loaded only for a round never levelled up. Vanilla waits 40 of the villager's own ticks to apply a level, and a round may not give it those. Locker trades now level a villager at once.
  - Rounds skipped a market whose desk is loaded, so a villager that had wandered just past the loaded chunks stood frozen and never traded. A loaded market's stranded villagers now catch up once a day where they stand, and Procurement finds a villager anywhere in the market's reach.
  - Squads no longer take villagers who work a locker: the court's smiths kept leaving on caravans.
  - The Merchant changes coin for emeralds at the mint (5% over the rate, from money beyond its reserve) to pay its villagers. Caravan purses had drained the capital's emeralds, so it couldn't buy from its own smiths.
- **Measured** (`network_study.py --comforts --war 31,61,101`, 120 days: the capital named on day 1, mobilising from day 31, war from 61, peace from 101):
  - **The court's three smiths were masters by day 10**, trained on imported coal, iron, flint and diamonds. S3's Armorer reached master; S1's Level 2.
  - **251 pieces of kit delivered** (44 arrows in run 1): S2 179 for 833 emeralds, S1 72. The army was issued 3 enchanted diamond chestplates (Fire Protection, Unbreaking), 6 iron chestplates and 2 enchanted iron swords.
  - **Trust held:** 0.62, 0.42 and 0.81 at the end (0.12 and 0.00 in run 1), with 1,053 emeralds paid up front.
  - **Still short:**
    1. No diamond swords: the master Weaponsmith offers them, but none were bought.
    2. S2 sat on 814 emeralds, money it couldn't turn into more hands; S1 took 267 up front and earned 34.
    3. The army's meals were still 0% hearty: no meat reached the depot.
    4. The treasury ended with 76,000 spurs unspent, and the spur fell from 16 to 34 an emerald.

## Why training broke, and where the swords went (2026-10-14)

Run 16 trained three masters, broke eight training contracts, and issued five diamond swords. From its contracts and
locker logs:

- **Five of nine smith stalls never traded once.** A Novice Merchant manages two lockers, and town Merchants stayed
  Novices: a Merchant's level-up waited on its own tick timer, which a town loaded only for a round never runs (the
  smiths had the same bug, fixed earlier). Merchants now level up at once. After the fix the "beyond its locker
  limit" message never appeared.
- **A contract for a second master had no one to train.** A town with a master Weaponsmith that took a contract for
  another never hired a second smith: a trade was taken up only when the town lacked it. Now it is taken up whenever
  every villager of it is already at the level.
- **A master's goods were never bought.** The Merchant switched on its villagers' selling trades (diamond armour
  from a master Armorer) only when a caravan came, and then only for villagers loaded at that moment. S2's stalls
  stood in other chunks than its desk, so S2's master Armorer and Weaponsmith (masters by day 40) sold nothing: 30
  contracts for 48 diamond swords and 208 diamond armour pieces broke. The Merchant now does this at each villager's
  own restock, when that villager is certainly there.
- **The stockpile asked for little.** It wanted 4 diamond swords, for an army of about six. It now equips a planned
  army of 24: 16 diamond swords and 8 iron, 8 of each diamond armour piece and 16 of each iron one, shields for half.
- **Run 17** (the first three fixes in): 5 of 6 training contracts finished; 8 masters by day 120, S2's Armorer,
  Toolsmith and Weaponsmith all by day 40. **Run 18** (and buying at the restock): S2 delivered **148 pieces of
  diamond gear** (26 swords, 25 chestplates, 25 helmets, 36 leggings, 36 boots) and earned 9,250 emeralds; every
  soldier at the front was issued a full enchanted diamond set and a diamond sword. 5 training contracts finished,
  3 broke.
- **Now the army is the limit:** five to seven soldiers (small towns' levies), and half of them deserted hungry.

## Feeding the front (built 2026-10-14)

Soldiers at the front went hungry on 27 of 47 soldier-days (run 18) while the capital held 10,000 loaves.
- **Rations were ordered too late, too small.** The quartermaster kept 4 days' bread for the soldiers already there,
  and a delivery takes 2 to 5 days (drafting, a free caravan, 1,500 blocks of road). It now keeps a caravan's round
  trip twice over plus 2 spare days (8 days at 1,500 blocks), for the soldiers and the recruits waiting to join.
- **Heartier food first, counted as contracted.** Food heartier than bread that the capital holds is sent first;
  bread makes up the rest. Only what's actually under contract counts: a first try counted carrots on the stock sheet
  that the contract couldn't get, and ordered no bread at all (run 19).
- **Run 20:** hungry on 13 of 65 soldier-days (20%, from 57%), all in the first days before the first delivery.
- **No meat reaches the front, because none is made.** No town has a Butcher; the flat world's plains give hunters
  only feathers; and a Butcher smokes meat only while its own town is short of food, never for sale. The capital
  posts cooked beef as wanted, but no town has idle hands to take it up. Fixed below.

## Mercenaries, meat, and the front's supply (built 2026-10-14)

- **Mercenaries.** Short of its planned army of 24 after the levies, the crown hires mercenaries at the market across
  the sea by a levy-like contract (1 emerald a day, four times a levied soldier's pay, owed to that market). Its
  caravan fetches them to the barracks; they ride to the front like any recruit and go back over the sea when done.
  One hire open at a time, counting contracts still being drafted (else it hired 8 a day). Levy pay is owed once per
  soldier, whatever contracts its home has.
- **Meat, any kind.** Any cooked meat or stew is hearty (at least as a pork chop). The crown tenders to train
  Butchers to level 2 (a Butcher sells porkchop, chicken or stew for emeralds), counts a town as having one only if
  its villagers actually offer meat (offers are dealt at random; towns now record what each villager sells at its
  restock), and keeps the realm's only Butcher out of a levy. A pupil's lessons aren't capped by procurement's pacing.
  The smokehouse also smokes meat its town has contracted to sell. Food is made to order by the stack.
- **The run-up.** While mobilising the crown puts by 20 days of food for the planned army: half bread, half meat
  (by food points, any kind).
- **The front's orders.** Rations cover three caravan round trips plus 2 days (11 days at 1,500 blocks). A delivery to
  the buyer's is due once the seller's next caravan can be there. The crown's front orders from its capital day by
  day while earlier orders are on the way, isn't put on trial, and pays by bill on the treasury when short of coin.
  Standing orders (daily installments) were tried and dropped: a caravan every three days missed them, and three
  misses break a contract.
- **Runs:** hungry soldier-days 57% (run 18, 7 soldiers) → 35% (run 25, meat first reaches the front) → 18% (run 27,
  461 soldier-days, 41 raised). Meat is still thin: Butchers' training varies by seed (run 27: level 1 until war day
  25), and each town is contracted separately in lots of a stack.

## The crown's procurement agent: requisitions (built 2026-10-14)

The crown buys the way a procurement office would, not town by town.
- **Each day** its agent at the capital works out what the stockpile and the army still need for the coming period
  (5 days), beyond what's under contract and already requisitioned, and posts a **requisition** per good: so many, by
  a day, at no more than the crown's price (worth, margin and carriage, times the phase's premium). Meat is one
  requisition of a kind, counted in food points: any meat will do.
- **Numismatics shops** go up with it, in a row by the capital's desk: a buying vendor for each good, at the crown's
  price, paid from the treasury's card. Anyone (players above all) can sell into it; the agent sweeps what's sold into
  the depot daily, and it counts toward the requisition. They come down when it closes.
- **Bids:** the board's news rides with caravans. A town's Merchant who reads it bids what its town can supply by the
  day (spare stock, and what its villagers make meanwhile, from what they offer a restock) at its landed cost (a
  quarter more for what's made) and half the way to the crown's price.
- **Award:** after 2 days of bids, the agent takes the cheapest offers until covered, each a single contract for the
  whole lot, a third paid up front, with the crown's bonus clauses; the papers go back with the next caravan.
- **Run 28:** the meat stockpile filled (1,920 food points by mobilising day 15, against 0 to 292 before), the front's
  meals were 26% hearty (0 to 5% before), and 121 contracts were awarded. Hungry soldier-days 28% of 501 (run 27: 18% of
  461; runs vary that much).
- **Next:** caravans swap the crown's bills for coin on the way back (the bank discounts them), and a clearinghouse
  town that aggregates its region's lots for one delivery.

## Caravans don't sit for days at a hungry stop (built 2026-10-14)

S3 (1851,1851) was the end of three routes and ran out of food to sell them for the road home: in run 28 all three
ended "waiting for food at the destination".
- A caravan judging whether the far end can feed it counts what caravans already on their way there will buy first;
  short, it carries its own food home.
- Stuck anyway, it buys food at the shops round the far end's desk (players' and the town's own), and after half a
  day sets off home on short rations, at half pace.
- **Run 29:** 26 trips home on short rations; trips into S3 up (capital 37 → 45, S2 28 → 46); hungry soldier-days 9%
  of 527, 4 deserters (run 28: 28%, 14).
- **Bills for coin:** the crown's bank already discounts its bills where its caravans call (45 bought in run 28); the
  limit is coin: the treasury is at 0 to 1,000 spurs from mobilising day 10, and about 4,300 emeralds of crown bills
  were dishonoured. War finance is still to do.
- **Logistics specialists haven't emerged:** the capital's caravans make most trips (4 routes, ~280 trips), towns'
  own routes carry only their own trade, and no caravan carries goods for a third market for a fee.

## Freight: carriers, forwarders, drayage (built 2026-10-14)

Not everyone hauls to the capital. A market with a storehouse (a warehouse it owns and runs) is a **freight hub**.
- **Carriers sell space.** Every route touching a hub is a carrier's run through it (the hub's own caravans out, or
  another market's caravans calling and going home). Each has a Numismatics shop by the hub's desk selling **freight
  tickets**, a stack of space each, restocked daily to half a wagon less what's booked, at the road's food cost a stack
  and a quarter. What a shop takes is its carrier's (another market's caravan collects it when it calls).
- **Forwarders buy it.** The hub's Merchant forwards: a caravan arriving with crates bound further on hands them over
  for a fee (freight, drayage, a fifth on top) and heads home; the forwarder buys just the tickets each crate needs on
  the soonest run its way and puts them in the crate's papers; booked crates load first. The hub's own caravans also
  collect at the towns they call at (a town that can't send caravans still ships through the hub). The forwarder buys
  the shippers' bills of exchange on the buyers for coin, less a discount, and its caravans present them.
- **Drayage.** Crates don't teleport from the quay to the warehouse: the hub's draymen (its idle hands and one more,
  16 stacks a day each for half an emerald) cart them, sold by the cart (4 stacks) at a drayage shop. The forwarder
  buys a crate's carting in and out up front and bundles it into its fee; with no draymen free, a crate waits.
- **Shippers through hubs.** A supplier with no road to a buyer bids and delivers through a hub: DAP, landed price
  with forwarding, crates consigned to the buyer and paid by a bill drawn on it. Contract copies pass through hubs.
  Requisitions are posted at hubs that run freight to their buyer.
- **Runs (study `--freight`: S2 the hub, S3 its spoke, no capital road to S3):** run 30 forwarded 20 stacks and bought 5
  bills; runs 31 and 32 forwarded none: S3 (one level-1 Fletcher) won no bids, and in run 31 couldn't send caravans
  at all (no Merchant, Cartographer, Librarian or Shepherd). Run 32: hungry soldier-days 1% of 622, no deserters, the
  capital's caravans freed from the 1,200-block road to S3.

## Mountains, and hubs that emerge (built 2026-10-14)

- **Steep ground.** The terrain survey also records each chunk's lowest and highest surface. A hex's relief (its
  highest less its lowest) makes it harder going, as hard again every 8 blocks; 32 blocks or more (a cliff, a
  mountain's flank) can't be crossed. Water still works as before; the two multiply.
- **Hubs aren't assigned.** A market becomes a freight hub on its own: other markets' routes pass through its land
  (within a hex), at least three pass or end there (or two pass), its Merchant is in, it can feed callers, and it can
  afford the storehouse (4 emeralds): it builds two double chests by its desk and its shops follow.
- **Study `--ridge`:** a 40-block stone ridge across the land between the capital's side and S3's, 38 hexes, with
  the pass at T. Both routes to S3 (the capital's, S2's) find their way through the hexes beside T, and T became a hub
  on its own (2 routes passing, 1 calling).
- **Stops on the way.** A route whose path crosses a hub's land calls there, out and home (its arrival set so far
  along the road, the rest kept for after). At the stop the caravan buys food for the road home if the far end may
  not feed it, collects its run's fares, hands the forwarder crates bound for markets the hub serves and sells it its
  bills, loads crates booked on its run, carries the hub's contract copies on, and reads the hub's board. A run
  calling at a hub is a carrier from there too: it has freight shops at the hub, both ways. A town such a run calls at
  hands it goods for markets the hub serves (prepaying the fare to the hub and the forwarding); the carrier buys the
  town's bills on the buyers and sells them to the forwarder at the hub (endorsed on, each taking a discount).
- **Run 33 (`--ridge`, war):** the capital's runs to S3 called at T 71 times, S1's 85, S2's 16; trips home on short
  rations fell to 0 or 1 on those routes; hungry soldier-days 9% of 500. No freight was forwarded: every buyer runs
  its own caravans straight through the pass, so a transfer at T would only add the margin and drayage. Forwarding
  pays where a shipper has no road to its buyer, or the hub's legs are cheaper than the direct road; neither holds on
  this map. Routes that reorganise around hubs by price are the next step.

## Routes reorganise by price; backhauls; llamas (built 2026-10-14)

- **Reorganising.** Each day a hub opens a run of its own to a far end of an established route that calls there (5
  trips), one a day; and the owner of a long run calling at a hub, where the hub runs on, compares its run's food with
  a short run to the hub plus freight for its usual load (carted, with the margin), and winds the long run down when
  that costs at most three quarters. Shippers choose direct or by a hub by landed price, good by good.
- **Backhaul pricing.** Space on a run going home sells at a quarter of the price: the caravan goes home anyway.
- **Space priced by capacity.** A carrier's price a stack is its squad's road food over what its last caravan could
  carry: packs, porters and llamas make space cheaper. Llamas in a town's pen go with its caravans as pack animals
  (9 stacks each, at most 4), and come home with them.
- **Runs 35 and 36 (`--ridge`, T given two Leatherworkers, a Shepherd, three idle hands, four llamas and 64 emeralds):**
  T's own caravans carried up to 63 stacks and made 88 to 111 trips, but T ended broke both times (its runs were its
  own shopping, earning nothing from the traffic through its pass), so it never could afford the storehouse: no hub,
  no forwarding. The pass needs an income: a toll per caravan.

## Tolls a caravan, and the network reorganising (built 2026-10-14)

- **A handling fee a caravan.** At its posts a town charges each caravan through a flat fee besides its share of the
  coins carried: what's left of the share of the detour, at most 3 emeralds where there's no way round. Routes count it
  in their path, and an owner weighing its long run against a hub counts the tolls the run pays on its way.
- **Run 37 (`--ridge`, the pass one hex wide at T):** the network reorganised itself. T (now Haygate, between cliffs)
  became a hub and opened runs of its own to the capital, S3 and S2; S2 (Wheatfell) became a hub (2 routes passing, 3
  calling); the capital wound down its long run to S3 ("by Wheatfell 13.2 a trip, not 29.0"). Wheatfell forwarded 68
  stacks for 560 spurs, bought 178 spurs of freight, carried 59 stacks, bought 18 bills (275 emeralds face) and 41 carts
  of drayage; Haygate forwarded 6 stacks and ended with 150 emeralds (broke in runs 35-36). Haygate posted no guard:
  a second gap beside the town (the study's ridge skirts town land) lets caravans round its hex for 3 spurs.

## The ridge study made honest, and a hub that emerges at the pass (built 2026-10-15)

- **Terrain known when it's made.** Chunks were surveyed only as they loaded or unloaded, so routes were planned through
  "impassable" ridges on stale ground. `/enterprise survey` records ground at once; the study surveys each build.
- **Bridges span water.** A bridged hex is open ground only if it's mostly water: towns had bridged mountain cliffs.
- **By water, a Fisherman leads.** A squad going by boat takes a Fisherman as its lead, its captain.
- **Guards on credit.** Unpaid guards wait up to 3 days' pay where their posts took tolls, or are planned to.
- **The study's geography** (`--ridge`): the capital a port, with a sea of 91 hexes west of it and Fishermen in its
  loaded town; a ridge raised chunk by chunk (only a hex's own chunks, one left low so the hex reads rugged, not a
  plateau); the pass at T's own hex a ravine (87% water) crossed by one bridge, the town's. Earlier runs built the pass
  in the hex next door (T's desk is equidistant from two centres), walled its mouth with spilled stone, or left the
  fills unloaded: each made routes cross or miss the pass.
- **Run 46:** T (Haygate, between cliffs) became a hub, opened runs to the capital, S3 and S2; the capital wound down
  its run to S3 ("by Haygate 12.9 a trip, not 24.9"); every road across the ridge now ends at Haygate (8 routes to or
  from it). Haygate forwarded 22 stacks, bought 13 carts of drayage (651 spurs to its draymen) and took 2,774 spurs of
  landing fees. Its transit toll (25% and a 100-spur fee) finds nobody passing through: the stop fee won.

## Towns grow before they trade (built 2026-10-15)

- **Focus.** Each census sets what a town puts first, as an early city in Civilization grows before it builds and
  builds before it trades. *Growing* (under 8 villagers, or food under twice the caravans' par): fields of food only.
  *Building* (fed and peopled, every bed taken): food and feed while it builds homes. *Trading*: fields for what sells,
  cane for paper among it. Planting cane in a five-villager town starved its growth.
- **Zoning.** Fields, pastures and pits keep 3 blocks clear of every bed, workstation and the bell, and never sit
  between a home and the bell: farms ring the houses, and no one is cut off from bed, work or market.
- **Starter homes.** A cottage is a family's: two beds, a chest, a lantern (10 emeralds). Study settlements start with
  one, and a hand waiting for work living in it.
- **Crew.** A route with no one to lead it: the desk's Merchant signs on a hand of the town waiting for work as a
  travelling Merchant (2 emeralds), one lead a route at most.
- **The census keeps who it can't see.** A villager in a chunk nobody loaded is where it was; one not found where it
  was last seen is sent for (its chunks loaded). Only death, or days of being looked for and not found, takes one off
  the roster. Before, the eastern towns lost half their people to the census (not to death) and couldn't grow.
- Measured: a 20-day ridge run, every town keeps its villagers; only the trading pass town plants cane.

## Realms beyond the border, wares, and Merchants who value what towns want (built 2026-10-15)

- **Wares** stand in for crafting chains to come: ordinary items with a price (tobacco leaf, pipe tobacco), so they
  ride every warehouse, caravan and contract. Pipe tobacco is a comfort of its own kind, leisure. None of our trades
  makes it: it's imported. Production goes only to trades that plausibly make a good (Farmers bake biscuits from
  wheat and cane, Librarians bind books from paper and leather, Shepherds weave carpets, Leatherworkers make boots).
- **Realms** are off the map, each a market with its own account, making what our villagers can't and wanting some
  of what they make. The Coast (over the sea west of the capital) makes pipe tobacco, leaf, cocoa and bread, and wants
  wheat, paper, iron and books; the March (overland, east) makes leather, wool and bread, and wants grain and iron.
  Their caravans and fleets run on our routes like ours (the dispatcher skips the Merchant and squad it can't see).
  The crown takes 10% customs on what they sell inside the realm.
- **Desires.** A town short of a comfort pays up to twice its worth the less of that kind it has had, as far as its
  households and spare money can pay; one with its fill takes more only at half. A Merchant supplying other towns buys
  comforts for them, from caravans that come to it and at the far end of its own runs, where it can sell on for 15%
  more than it pays after the road (its share of a stack's road food).
- Measured (ridge, 120 days from the era's day 0, each against the last): with the realms, the March's 71-76 trips
  revived the east (Lindenfield 0 to 303-332 emeralds, 1 to 315-373 trades). With desires too, the capital bought the
  Coast's pipe tobacco to sell on: leisure met in Meadowworth, Wheatfield and Barleyby; every town but Greenwick
  content (Meadowworth +2.5, Hollowwick -2.0 to +0.5); no wages unpaid anywhere; 26 children born. Still unmet
  everywhere: treats and furnishings.

## The town's shop, the Merchant's books, and its reasons (built 2026-10-15)

- **The town's shop.** Up to three Numismatics vendors by the Merchant's desk, built when the town can pay. A comfort
  the town lacks, or a good its lockers take that it holds none of, is listed out of stock at what the town would pay
  now (the price floats with how short it is and what its people can afford). The listing is honest: only while the
  Merchant is procuring the good, with a road for it, and has money for a lot. The shelf takes only true surplus,
  comforts first. **The listings are news:** a caravan calling there reads them, and other Merchants value the good at
  the price their caravans last saw.
- **Books.** Each market's Merchant keeps books: what it bought and sold, what its caravans carried out and brought
  home, what its shop sold, with whom, and at what price. A caravan keeps its own log at the far end and hands it in at
  home: the books know of a far market only what a caravan brought back. A page a day takes stationery. They go back
  10 days with a Merchant alone, 60 with a Librarian in town, 365 with a library. A Merchant values a good at a market
  by the last price its books show there.
- **Hauling only what sells.** A caravan exports a comfort only beyond its own town's par, and loads of each good at
  most the far end's par less what it held when last seen. Realms use 64 a day of each good they want, so they keep
  buying.
- **Reasons are the trading system's debug messages.** The books keep a tally (how often, how many, last day) of why
  each good was sent or not ("they hold their fill", "we have none", "no room aboard"), why the far end wouldn't buy
  ("they hold their fill", "can't pay", "too dear to sell on", "no use for it"), why no caravan left, and the
  workshop's potential: what it could make and which input it's short of. Reasons cost no stationery.
  `/enterprise whys <desk>`.
- **Potential becomes demand.** A town tracks what its trades could make if they had the inputs. The workshop's draw of
  inputs for its people's comforts (books back through paper to cane; biscuits to wheat and cane) joins the census
  demand when farmers plan plots and fields. A town growing or building still farms food first, but plants cane for
  its own workshop once its food is in hand. Without this, nobody grew cane: the capability couldn't start without the
  raw material, and no Merchant carried a raw material nobody could yet use.
- Measured (three lanes per change, 120 days from the era's day 0): the reasons showed the Coast turning away 3,483 of
  the capital's 3,611 books ("they hold their fill of reading": a realm had no comfort par), the March stalling from
  day 29 ("waiting for food": its people ate the bread its caravans needed), and every town's paper, books and
  biscuits short of cane, with bread and biscuits short of wheat even in Wheatfield.
- With these fixes (three lanes each, mean [range] at day 120): the March ran 74-81 trips (from 24); Lindenfield 115
  to 321 emeralds; moods up in seven of eight towns (Haygate -0.65 to +1.41, Barleyby -0.03 to +0.81, Hollowwick -0.13
  to +0.32); the Coast stopped turning books away after day 43; 5-10 cane fields planted per run, all pressed into
  paper. Treats still 0% everywhere: the Librarians' paper takes the cane first, and the biscuits are short of wheat
  too. `dev/experiments/compare_lanes.py` compares lanes.

## Greenwick: a town with no road makes its own comforts (built 2026-10-16)

- **The trap (traced).** Greenwick has no caravan road (in every lane). No clothing, furnishings or leisure ever reach
  it, so its mood held near +0.3 for 120 days: under the 0.5 a full town needs to build a family a home, and the 1.0
  a town needs for a child. It stayed at six villagers, every adult employed (a Cartographer and a Fletcher among them,
  serving caravans that don't exist), so no idle hand ever drew a new trade; and the job draw weighed trades by what
  they'd buy of the town's goods, not by the comforts it lacked. Its households saved 317 to 717 emeralds of wages
  and spent about 70: there was nothing to buy but its own biscuits.
- **Comforts in the job draw.** Each comfort a town lacks puts marbles in the draw for the trade whose workshop makes
  it (carpets a Shepherd's, boots a Leatherworker's, books a Librarian's, biscuits a Farmer's), if the town has none
  of that trade: a share (0.2, dial) of its households' savings, times the share of the ration unmet.
- **A trade with nothing to do gives way, where there are no roads.** In a town no caravan comes to or goes from, a
  villager whose locker's trades did nothing for the town at the Merchant's last 5 reviews (dial) takes up the trade the
  draw picks, if the town hasn't it: its stall taken down, its trades forgotten. A new hand has twice that before it's
  judged (a Leatherworker's cows must breed past four before it has hides). Farmers, Masons and Clerics never: their
  work isn't in their lockers. (Retraining everywhere cost towns with roads up to a point of mood: there a trade's
  work may be its workshop's, a service, or goods carried away.)
- **Deposits fund the start.** A stall, and a first pasture, are paid for with the households' savings where the
  treasury can't (see "Citizens' deposits").
- **New families as a dial.** A town needs mood 0.5 (dial `birth_mood`, was 1.0) for a child, 0.5 (`home_mood`) to
  build a full town a home.
- Measured (forked from day 120, 20 days, three lanes, against the same build with all of it off): Greenwick +0.29 to
  +0.55 (to +0.87 where it had a child; with retraining in every town it reached +0.96, but Meadowworth fell to
  +0.82); the other towns within the noise. Not yet measured over a whole run with the lower birth mood.

## Stranded paper: a sound town ruined by a closed road (built 2026-10-16)

**The failure (traced).** Hollowwick, rich on day 20 (163 emeralds, mood +3.4), took the capital's training contract:
train an Armorer, and sell the capital the pupil's work Ex Works, paid by bill when its caravan next called. Each
lesson its Merchant paid its own Armorer for iron leggings, crated them for the capital and drew a bill on it: 19
bills, 1,628 emeralds. Meanwhile freight wound down the capital's direct run to Hollowwick (cheaper by the Haygate
hub). Then no one could settle any of it: the capital never came to collect or pay; Hollowwick had no road to the
capital; the hub's carriers took only goods whose fare was prepaid; and a bill only lapsed when presented, so they
stood outstanding for ever. The contract broke (the pupil untrained); its crates, someone else's shipment, filled the
warehouse, and from day 40 the harvest had no room. With nothing to sell, its wages went on credit, its caravans had
no coin for road food, it could take on no Farmer (1, against 3 in a healthy lane), and it sank to -0.26 by day 90.
Its wealth was real but illiquid and out of reach: correspondent risk, the way the 1763 Amsterdam bill crisis spread.

**The remedies, as merchants found them:**
- **The hub buys stranded paper.** A bill a town holds on a market whose caravans don't call (or past its day), that a
  freight hub runs to, is bought at its face less a fee (2%, and 10% more past its day) less the forwarding of the
  goods it paid for, which go with it to the hub's forwarder. Paid in coin as far as the hub trusts the drawee and can
  spare; the rest in the hub's own bills, in up to six installments a bill term apart, each at most half what its
  treasury could pay; beyond that it doesn't buy. Bad credit and value in hand are worth more than nothing. Either
  way: the hub's caravan calling at the town, or the town's caravan bringing its paper to the hub. Never a bill on the
  hub itself: the town presents that there, for its face.
- **Freight on credit.** The hub's own caravan carries a town's goods for markets the hub serves even when it can't pay
  the fare: what it can't pay is its bill to the hub, due in 30 days (dial), as far as the goods' stack worth covers
  the fare at the lien's share (half, dial).
- **No offtake without a collector.** The pupil's work is crated and billed to the buyer only where a caravan of the
  buyer's calls, or the town's caravans reach a hub that runs to the buyer; else it stays the town's own.
- **Dead paper goes.** A crate shipped under a contract no longer in force goes back on the shelf, unless it was paid
  for by a bill still to be collected (then it's the buyer's, and goes with the bill). A bill presented gets its grace
  from its first presenting, not from its day; one no one can carry (no road between the two, no hub to buy it) lapses
  12 days past due; paper no longer the desk's to collect is thrown out of its warehouse.
- Measured (forked from the day-120 checkpoint where Hollowwick was broke, 20 days, three lanes): Hollowwick 0 emeralds
  and -0.26 in every lane before; after, 755-1,265 emeralds, mood +1.25 to +1.98, the harvest moving again. Haygate,
  carrying the risk now, 990 to 11-971 emeralds, mood +2.17 to +1.65: the capital paid its bills in full in one lane,
  and dishonoured 17 (about 1,400 emeralds) in two, unable to find the coin within the grace.
- **Grace by the size of the debt.** A drawee has 8 days (dial) from a bill's first presenting, and a day more for
  every 100 emeralds (dial) it owes the same holder: a capital owing a hub 1,600 emeralds has about three weeks to find
  them. Measured from the same checkpoint: Hayford's bills to Haygate dishonoured, 17 (about 1,400 emeralds) at a
  fixed 4 days; 4 (343) at 4 days scaled; none at 8 (49 paid, 2,064 emeralds) or 16 days scaled. Hollowwick ends at
  900 to 1,150 emeralds, mood about +1.9.
- **Citizens' deposits.** A Merchant short of coin to honour a bill put to it, or to buy stranded paper, borrows its
  households' savings, up to half of all they have (dial), into its chest; it owes them that with interest (0.2% a day,
  dial) and repays from what it can spare beyond its reserve and a keep of 10 emeralds. (Not called on in the
  Hollowwick fork: those households spend their wages, and the capital keeps none; it's for towns like Greenwick, whose
  purses hold hundreds.)
- **Next:** a dealer pricing a drawee's record into its discount; a Merchant lending against goods in its warehouse
  (a Lombard loan); deposits where the savings are.

## Forward crop sales, rents and a dealer in them (built 2026-10-16)

Medieval farms sold their crops ahead: abbeys sold years of wool to Italian merchants for money up front, merchants
bought grain standing in the field, and towns and widows bought rents (a *census*, a *rente*): a capital paid once
for so much grain a year, for good, until the farm bought it back. (Futures proper came later: Osaka's rice market,
1730; Chicago, 1848.) Here, the same paper:

- **A forward crop sale** is a contract with a **harvest term**. A market whose lockers buy a crop its fields don't grow
  enough of offers the farm town whose caravan has come so much a round trip, ten times, at today's price, a third paid
  up front (into the caravan's purse). No more than the farm could add (its idle plots, its spare hands, half what it
  grows of the crop now, less what it has promised already); the first installment falls due after 4 days to sow.
- **The harvest term** judges each installment 4 days after its day, not 2; a short one rolls over to the next,
  costing a quarter of a miss in trust; only arrears past two installments, or what's owed at the last, are misses.
- **Farmers sow for it first**, after the herds' feed, at its locked price; rotation turns up to two fields a census
  over to a crop owed, fed or not.
- **A rent** (`Rents`): the holder pays the farm a capital of eight seasons' crop at today's price; the farm owes so much
  a season (10 days) for good. Each season issues a **coupon**, a one-installment forward contract already paid for;
  a season's shortfall rides on the next coupon, and arrears past two seasons are a default. Sold where the farm is
  short of money and the buyer has twice the capital to put by; bought back at the capital when the farm can spare
  twice that (its caravan carries it). Paid in kind, a rent keeps its worth when the mint debases.
- **A dealer.** A freight hub's Merchant buys rents from holders short of coin, or for a quarter's profit on what they
  paid, a tenth under its price, and sells to towns with savings a tenth over; a rent changes hands at most once a
  season and never straight back. Its price is its capital times how short the hub's markets are of the crop (twice
  with none bought ahead, half with all of it). A rent the dealer holds is collected Ex Works: its caravans call at
  the farm for each coupon, so a remote farm without a caravan of its own keeps its rent. A sale endorses the season's
  coupon over; the season runs on.
- **Deeds for players.** The dealer quotes each rent it holds on a vendor by its desk, selling the deed (a document
  naming the rent) at the ask; once a player has bought it, the vendor buys it back at the bid, from a bank card the
  dealer keeps funded. The deed is the title: the dealer collects the coupons in its keeping, and a player bringing the
  deed to its desk is handed the crop collected for it.
- **Owed crop is set aside.** What falls due under harvest contracts by the end of the next window, not yet delivered
  or on the road, is kept from the town's spot sales and its workshop. (Traced: Cloverby's own caravan sold the wheat it
  owed Haygate on the spot, and its bakers took the rest; a coupon of 57 got 1.)
- **Wants by use, promises by hands.** A buyer buys forward what its lockers use (at least a lot a round trip, to start a
  trade), not all its villagers could buy (Haygate's treasury fell from 617 to 11 emeralds); a farm promises what its
  spare hands, half its crop and a quarter of its other field crops could grow, not its untended plots.
- **Rotation on the books.** A field turned over where its chunks aren't loaded (most of the time) is kept in the save,
  and counts as its new crop; and a crop owed takes its field from the crop with most plots lying untended (the
  census's demand counted every field as needed: Wheatfield owed potatoes, grew none, and had 140 wheat plots idle).
- **A dealer takes delivery where it can**, collecting at the farm only where the farm's caravans don't go, and sells a
  rent on only to a town the farm can deliver to or that collects. A rent changes hands at most once a season, never
  straight back; a holder sells for a quarter's profit on what it paid.
- Measured (three lanes, 120 days from the era's day 0, against the last commit): forward sales 1 to 6 a lane, half
  delivered or running (before the trace, 22 to 25, three in four broken); rents 9 to 11 a lane, most bought back, 1 to 3
  defaulted. Moods: Wheatfield +2.17 to +3.13, Cloverby +1.63 to +2.37 (236 emeralds, from 84), Lindenfield +2.24 to
  +2.50, Meadowworth +1.81 to +2.00; Hollowwick +1.59 to +0.22 (broke in two lanes; a 16-beetroot rent to Haygate went
  undelivered for 42 days though it has a road there: not yet traced), Haygate +1.54 to +0.98. Sum of the eight 14.86
  against 15.08: within the noise.
- **Earmarked in crates.** As soon as its crop is in, a farm crates what falls due under its forward sales and coupons
  by the end of the next harvest window: a bill of lading made out to the buyer and a copy of the contract in each
  crate. A crate with papers is a shipment, out of the stock's reach (lockers, bakers, spot sales); its caravan to the
  buyer loads it first, and a buyer collecting Ex Works takes it first. A crate whose contract has closed, or gone to
  another buyer, goes back on the shelf. A bought-back rent's coupon closes (traced: one stood owed for ever, keeping
  48 potatoes set aside for no one).
- Measured (forked from a day-120 checkpoint with the paper in play, 20 days, the harvest window at 4, 6 and 8 days):
  no forward sale rolled over or broke, farms delivered ahead of their days, coupons 56 to 100% delivered; the window
  made no difference once deliveries weren't late. Hollowwick stayed broke in every lane: its caravans wait for road
  food (its bakers short of wheat, 167 potato plots a day untended for want of hands), not for paper.
- **Next:** Hollowwick's food and hands; coupons sold on their own; forwarding coupons for holders whose farm can't pay
  the fare; a rent's price moving with local prices.

## The Merchant tells its Farmers what sells; lockers by worth; root cellars (built 2026-10-16)

- **Why Cloverby starved.** It grew carrots no one bought: a town planted for its own villagers' buying trades only,
  counted even where the villager had no locker the Merchant served (its Farmer waited on a higher Merchant level);
  its neighbours' wants (Haygate's lockers take potatoes and beetroot) never reached its fields; and its Farmers worked
  every plot, wanted or not, for wages. Its caravans had nothing to sell, it went broke in a week, and could never pay
  a trip's road food again.
- **Demand from where the town sells.** The census counts a villager's buying trades only at a locker its Merchant
  manages. The farm plan adds what the markets its caravans go to would take: a town's demand for a crop beyond what it
  grows itself (a realm's: what its people use), at the crop's worth less the road's cost there and back, for goods the
  far Merchant buys. New fields are chosen with it too.
- **Fallow and rotation.** Once the town has its food (twice its par), plots of a crop no one wants lie fallow: not
  worked, not paid for, no marbles for another Farmer. And at each census a field's worth of plots (81) of a crop grown
  beyond all its uses is turned over to the farmland crop whose unmet use earns most.
- **Lockers by worth, with hot-swap.** Each day at the census the Merchant values every locker in reach by what its
  villager's trades would do for the town a restock (its buying trades, as far as the warehouse holds the goods now; half
  what its selling trades charge for goods the town wants), or by what it did since the last review if more. Its
  clients keep their places; the weakest is let go (its trades switched off) for a waiting locker worth half again
  as much and half an emerald more, one a day, and a new client has two days before it's judged. Its trades that buy
  goods the town holds are switched on: taking it on is serving it. Contract lockers are never let go.
- **Root cellars, and stores only where the crop piles up.** A root cellar (5 x 5, a Farmer of level 2, 8 emeralds)
  banks carrots, potatoes and beetroot in barrels, up to 27,648, as the granary banks wheat. A town builds either only
  where that crop piles up: twice what the warehouse keeps loose, or a stack of harvest waiting for room.
- **Masons from the work wanted.** A Mason's marbles are the halls a town wants and could pay for now (materials and
  builders' wages), not the wages it has paid hired builders.
- **A poor town packs its own road food.** A town that can't pay the far end for the road home's food, but has food to
  spare beyond its par, carries it. (Every town packing its own took the far ends' road-food income: moods fell in four
  towns, so only the poor do.)
- **Empty crates go home in stacks of 16:** a caravan carrying more in one stack crashed the save.
- Measured (lanes, 120 days from the era's day 0, against the granary run; lane 2 crashed on the crate bug at day 112):
  every town's mood rose. Cloverby -0.76 to +1.04 (365 trades, from 0), Lindenfield -0.74 to +2.54, Greenwick -1.98 to
  +0.28, Hollowwick +0.22 to +1.54, Wheatfield +2.67 to +3.65 (2,042 emeralds), Haygate +1.16 to +1.95, Meadowworth
  +1.73 to +2.46, Barleyby +1.86 to +2.38. Masons came (two a lane), a root cellar went up in Meadowworth, and no
  granary was built where no wheat piled up.
- **Next:** forward crop contracts with a harvest window (shortfalls roll over), rents bought once that pay a coupon of
  crop each season, Haygate dealing in both (and calling on remote towns on request, forwarding their crop on its
  backhaul), then the paper as items its vendors quote to players.

## Sharecroppers, hired builders, a granary and a bag of marbles (built 2026-10-15)

- **Sharecroppers.** Past its credit, a town that can't pay its Farmers no longer loses the harvest: they take their
  wage in kind, their share of the crop, and the town gets the rest. Before, a broke town had nothing to sell and
  stayed broke (Hollowwick, frozen from day 20 in one lane in three).
- **The payroll is untouchable.** Goods a caravan sells a town, and what its lockers pay its own villagers for
  procured goods (bread and armour for a crown contract), come only from money beyond its wage float and guards' pay.
  Meadowworth's coal took Hollowwick's wages on day 16; a contract's bread filled another town's warehouse.
- **Food beyond twice its par is idle**, so the warehouse crates it like any idle good, and a glut of bread no longer
  shuts out the harvest.
- **Hired builders.** A town without a Mason hires builders for its halls, as it does masons for its walls: materials
  up front, wages as they build, halls up to a Mason's level 3. Stone docks and a caravanserai still need a Mason.
  Before, no town ever built a hall: none had a Mason.
- **A granary** (after the square; a Farmer of level 2) banks wheat the warehouse needn't keep loose (its par, at least
  two stacks) as hay bales, up to 36,864, outside the warehouse, and tops the warehouse up as it's drawn on.
- **A bag of marbles.** An idle hand draws its trade: each trade's marbles are the emeralds of demand it would meet
  (what its villagers would buy of the town's supply, shared with those of the trade already there; goods markets it
  knows have posted as wanted; four days of what idle plots would yield, a Farmer's; what the town has paid hired
  builders, a Mason's until it has one). A contract's trade still comes first.
- Measured (three lanes, 120 days from the era's day 0, against sharecroppers alone): halls went up for the first time
  (10 granaries, 8 squares, smithies, watchtowers, libraries); moods rose in Wheatfield (+1.40 to +2.67), Barleyby
  (+0.70 to +1.86) and Meadowworth (+1.35 to +1.73). But every granary held nothing: no town grows wheat it doesn't
  use (Wheatfield grows 50 a day and holds 1; the east grows roots). No town took on a Mason: hall wages (a few
  emeralds) are small marbles beside four days of supply. Cloverby still holds about 1,700 bread in every lane and
  never trades (the bread is its larder from the era's setup; why it never trades: see the next section).

## Where the next item earns most; Farmers who learn and multiply (built 2026-10-15)

- **Marginal value, not market size.** The workshop gives an input first to the conversion that earns most per emerald
  of inputs (comforts at what the town would pay): a cane goes to biscuits (about 1.6) before paper (1.0). Farmers'
  plots go use by use, dearest item first, as many plots as each use takes: the Librarians' huge cheap paper market
  no longer takes every hand from the cane and wheat for the town's own biscuits.
- **Farmers learn by farming.** The harvest earns its Farmers 3 experience per emerald of crops; they rise as for a
  trade. Before, only the locker's trades levelled them, and half the towns' Farmers stayed Novices.
- **Another Farmer when the land lies idle.** An idle adult gets a Farmer's stall, though the town has one, when its
  untended plots would yield more in 4 days than the best new buyer would take (and at least half an emerald a day).
  Before, stalls opened only trades a town lacked: every town kept its one Farmer.
- Measured (three lanes each, 120 days from the era's day 0, mean): marginal value got biscuits made for the first
  time (800-1,800 a run) and wheat grown in Wheatfield (32-48 a day, from 0). Farmers who learn and multiply then
  raised farm output three to tenfold (Barleyby: 5-8 Farmers, 368-576 items a day, 395 emeralds in its treasury, mood
  +1.56; Wheatfield +2.14; Haygate +1.97). But Greenwick (-1.41) and Lindenfield (-0.71) fell: their households held
  430-830 emeralds of wages with nothing to buy (comforts 0%; Greenwick has no caravan road). Prosperity now waits on
  comforts reaching the towns that can pay for them.

## Wages stay in the world: households (built 2026-10-15)

- A town's farm pay is its households' money, not a sink: wages go into their purse, and they spend it at the market,
  paying the Merchant for the comforts the town uses, the food for a child, and (first, before the town's money) for
  family homes and children. Wages are demand.
- **Credit.** A Merchant short of coin pays its farmers in credit, up to 10 days of a customary wage (half the crop's
  worth), and the harvest keeps coming. Beyond that, the farmers take their wage in kind, their share of the crop, and
  the town gets the rest (see "Sharecroppers"; before, unpaid days went unworked).
- **Freeholds.** A household with 8 emeralds in purse and credit, where farmers have hands for a third of a field more,
  buys a field of its own (credit first, then money paid to the market). On its own plots it takes at least three
  quarters of the crop's worth.
- **The wage follows the labour market.** The share moves toward the share of the harvest that sells, times how scarce
  hands are: up to half again with plots going untended, down to half with hands idle.
- Measured (ridge, forked from the era's day 0, 120 days, against the old system): 999 emeralds of wages, 516 spent
  back at the markets and 483 still in purses (old: 715, gone); moods up in six of eight towns (Cloverby -0.75 to
  +0.33, Meadowworth +0.97 to +1.63); unpaid wages 35 (old 41). No freehold bought: hands were scarce everywhere
  (plots untended), so the east's limit is people, not land. Barleyby's farmers sat on 282 emeralds with little to
  buy: nothing in the world makes comforts.

## Settlers (built 2026-10-15)

- A town grown past growing (8 villagers or more, not short of food) raises a party of settlers: while it does, its
  Merchant keeps hands waiting for work back from new jobs. Once every 8 days at most.
- **A village nobody trades with** (a naturally grown one, no Merchant's Desk, 4 hexes or more from any market, within
  8 of home): one hand signs on as a travelling Merchant and goes alone with 10 emeralds of capital and bread
  (14 emeralds in all). It sets up a desk and chest by the village's bell and works it; the old town opens a route
  to it.
- **Founding a town** where no village is in reach: a Merchant and a settler go to the best open ground the town knows
  (4 to 8 hexes out, unclaimed, flat and dry by the survey, fresh water a plus) with 16 emeralds of capital (34 in
  all). They build a bell, a desk and chest, a composter, a lectern, a cartography table and two family cottages.
- The trip is one way with no market at the far end yet, so it isn't a caravan route: it's planned over the same
  hexes, takes as long as a caravan would, and its people are off the world on the road like a squad.
- Measured (40-day ridge run): Meadowworth settled Greenley and Wheatfield settled Elmford, villages 260 to 320
  blocks out; caravans ran to both, and Greenley opened a route of its own. Not yet seen: a town founded on open
  ground; and a settled village claiming its land.

## Names (built 2026-10-14)

Every market has a name, given from the lie of the land once the hexes round it are surveyed, and kept: a first part
from its country (Barley, Oak, Pine, Sand, Frost, Fen...) and an ending for what lies about it (-gate between cliffs,
-fell in the hills, -haven on the coast, -ford or -mere by water, else a farming word: -field, -ton, -stead...).
Off-map markets keep theirs (the front, the market across the sea). Reports and logs use names; `/enterprise name`
lists them, and `/enterprise name <desk> <name>` renames one. In the ridge study: the capital Haywick, S1
Meadowworth, S2 Wheatfell, S3 Hollowwick, T (the pass town) Hayfell.

## Pacing and the crown's bank (built 2026-10-14)

Once room stopped blocking them, Merchants were blocked by cash. S2's Merchant tried to buy 405 emeralds' worth of
diamond gear from its master smiths at once, holding about 17.

- **Pacing:** a Merchant buys from its villagers for what its contracts want within two days (and its open orders,
  and the sovereign's shortfall there), not the whole of a long contract at once. A pupil's lessons still run a
  restock's worth.
- **Banks** (`Bank`, `/enterprise bank`; the capital opens one when it's named, and `/enterprise bank open <desk>`
  opens one anywhere, a barracks town say). A bank deals only through its own caravans, from its market's treasury,
  so money still goes only where a caravan carries it. Where its caravan calls:
  - **Discounting:** it buys the town's bills not yet due for coin now, less 1% and 1% a day to their due day. A bill
    drawn on the bank's own market is redeemed early; any other is endorsed to the bank and carried home to present.
  - **Lending against contracts:** a town with a contract in force and less money than half the goods still to come
    are worth is lent up to that half (less what it was paid up front and lent already), at 3%, secured on the
    contract. When the bank's caravan pays for those goods, or pays a training bonus, the loan comes out first. A loan
    left on a contract that's done or broken is collected from what the town can spare.
  - A bank's caravan carries up to 48 emeralds more for its dealings.
  So a bill in hand, or a contract with goods coming, is money today.
- **Run 16:** the crown's bank bought 51 bills (642 emeralds paid for paper worth 665 when due) and made 33 loans:
  389 emeralds lent, 396 repaid, 5 still owed at the end. Emerald purchases refused for want of money fell from 446
  to 38. Five enchanted diamond swords (Sharpness) were issued to the army. Training finished three masters (S1's
  Weaponsmith by day 40, S2's Armorer by day 60, S1's Armorer by day 80), with 8 training contracts broken against
  2 in run 15: single runs vary too much to judge a change by one.

## Inventory waste, cold storage and the apprentices' work (built 2026-10-14)

Training a smith by paying for its work makes things nobody wants: a Novice Weaponsmith's iron axes, one to a slot.
In the run before this, S1's one-chest warehouse was 25 slots of 27 full (18 of them axes), so its Merchant couldn't
take in the emeralds to pay for the next lesson, and its Armorer sat at Novice with 0 XP for 70 days. This is
vendoring greys, and the Merchant now does what a shopkeeper would.

- **Crates are storage.** A packed crate with no papers is part of the warehouse, nine stacks in one slot; the stock
  reaches into it like a shulker box. A crate with a bill of lading is someone's shipment and stays shut.
- **Storekeeping** (`ColdStore`), whenever a town is touched:
  1. hands working its land without a tool get one (an axe first): they gather half again as much, and tools wear
     out in 16 days. So axes have a use in the populace, and towns whose hands lack tools want them;
  2. short of room, the Merchant packs goods it isn't using into crates (those that don't stack first), and whole
     stacks of what it uses beyond a stack over its par (a standing order that brings more paper than its Librarian
     takes);
  3. the crates go to **cold storage**, a double chest it sets up by the desk the first time (1 emerald, the
     cheapest warehouse there is): a store that holds only crates;
  4. still short, it gives idle tools to its hands;
  5. last, it throws out greys (goods worth under 0.1 emerald each), least worth first, counted as waste. Anything
     better is kept, room or no.
- **The sovereign buys the apprentices' work** (an offtake). Its training tenders carry an offtake: the buyer takes
  what the town pays its pupil to make, at its worth. Under one, the pupil's paid work is crated at once, made out to
  the buyer with a bill of lading (title passes), and the town draws a bill of exchange on the buyer for it. The
  buyer's next caravan pays the bill and takes the crates home. Any caravan now fetches crates made out to its home.
- **Axes are lesser weapons** (iron axe: power 5, a little more risk than an iron sword). The front's quartermaster
  counts any weapon toward a weapon each, and any piece of armour toward each place. It contracts for the best the
  capital holds first: diamond from master smiths, then iron, then the apprentices' axes.
- **The Merchant trains smarter:**
  - lockers its contracts need come first in its locker list. A Novice Merchant manages two lockers, and a new
    smith's stall used to fall outside them, so it never traded at all;
  - it pays for a pupil's work only if the work has somewhere to go (an offtake, or the town wants it, or there's
    room to keep a restock's worth);
  - a training contract is for a new master. It records how many the town already had, so one master can't collect
    every tender. Before this, S1's single master Weaponsmith "completed" eight;
  - a Merchant takes a tender only with the money (beyond its reserve, with the advance) and the room (or a cold
    store) for it. It carries a tender on only to towns that can't already see the original board, and the
    sovereign counts tenders carried on as its own, so it doesn't post again for them.
- **Fixes from the analysis:** helmets, leggings, boots and axes were missing from the price table (worth 1/32
  emerald by default, so towns were contracted for iron helmets at 0.05). A town now counts as able to make goods
  only if a villager of it sells them, or a trade of it makes them from inputs it, or its buyer, holds. No more gold
  contracts without a mine.
- **Measured** (three 120-day runs as the fixes went in; capital named day 1, mobilising 31, war 61, peace 101):
  - **Masters, at last:** with the Merchant tending contract lockers first, run 10 trained S1's Armorer, Toolsmith
    and Weaponsmith and S2's Armorer to master. Run 11, after the new-master rule, trained four genuine masters: S2's
    Toolsmith and Weaponsmith and S1's Armorer by day 40, in peacetime. Diamond swords and chestplates (4 each)
    reached the capital from day 60. Run 12 trained only one (S3's Weaponsmith, by day 40), with 9 training
    contracts broken: results swing a lot between runs (which trades villagers roll, when caravans arrive).
  - **Waste:** run 11 threw out 384 raw iron at S2 and 192 coal at the capital. With greys-only and the exchange's
    metal counted as in use, run 12 threw out nothing; every town set up cold storage, and S2 gave its hands 4 axes.
  - **The army** was issued iron swords, chestplates, boots and helmets. No diamond gear reached the front in
    run 12: it had no master selling diamond swords by the war.
  - **Why most training still broke, from run 12's contracts and locker logs:**
    1. **Cash, not will.** S3's master Weaponsmith had its diamond-sword trade switched on, but a restock's three
       swords cost about 75 emeralds and S3 had 0 to 4 to spare. Its four diamond-sword contracts broke with none
       delivered, though each paid 26 emeralds up front: the advance sat in the treasury behind the town's reserve.
       And they were priced below cost: 19.8 a sword, where a master asks about 26 (the table had 12).
    2. **Room.** S2's warehouse is one 27-slot chest, and 13 slots were paper: its Librarian takes 384 a day, so two
       days' par is 768. Cold storage had packed only what was beyond par, so the Merchant still had no room to take
       in emeralds.
  - **So:** money paid up front and not yet worked off counts as money the Merchant can spend on its contracts'
    work. It pays for a lesson as soon as it can afford one, not a whole restock. Diamond gear is priced at what
    masters ask (sword 22, chestplate 21, leggings 19, axe 18). Short of room, it packs full stacks of anything it
    holds (one loose stack of each kept to hand), since crates are still stock.
  - **Run 13:** three masters (S2's Toolsmith and S3's Armorer by day 60, S1's Armorer by day 80), and the first
    diamond gear at the front: enchanted diamond leggings and boots. Weaponsmiths still stalled, short of room.
  - **Run 14,** with packing for room: **five masters, one training contract broken.** S2's Toolsmith by day 40, its
    Weaponsmith by day 60 and its Armorer by day 100; S1's Weaponsmith and S3's Armorer by day 60, mostly in
    peacetime. **Five enchanted diamond swords** (Sharpness, Fire Aspect) were issued to the army, and diamond boots
    reached the depot. Trust held between 0.52 and 0.83, and towns threw out under 5 emeralds' worth each.
  - **Still open:** soldiers went hungry 21 of 45 days, and meals are never hearty: no meat reaches the front.
- **Goods stay crated from shipper to buyer.** Shipments used to be opened on arrival and poured loose into the
  warehouse, then packed into crates again once it filled up. Now:
  - a crate made out to a Merchant goes into store as it comes, its papers filed;
  - a caravan packs what it carries for sale in crates of one good each (the hub's empties first, else new ones);
  - a buyer takes a whole crate when it wants the whole lot, and otherwise takes what it wants out, leaving the rest
    crated aboard;
  - goods come out of a crate only when used, and the emptied crate stays to ship with.
  Counts of what's aboard a caravan look inside crates.
  - **Run 15:** S2's main chest held 6 of 27 slots and its cold store 16 crates, against 24 of 27 slots, half of it
    paper, in run 13. Five masters again (two training contracts broken); diamond helmets and leggings were issued
    to soldiers, and 3 diamond swords waited in the depot at the end.
  - But towns packed for room about as often as in run 14 (36 to 69 times each over 120 days): most of the clutter is
    what they make and gather themselves. What still blocks a Merchant from paying its smiths is cash, not room: S2's
    Merchant tried to buy 405 emeralds' worth for its master smiths' diamond-gear trades at once, holding about 17.

## Everything runs on the logistics framework (built 2026-10-14)

No special routes: goods, money, soldiers and work reach where they're needed by contract, caravan and warehouse, as
everything else does. Only what happens at the ends is special.

- **Markets off the map** (`OffMap`): a place the region trades with that no one builds or visits. It sits at a real
  position, so roads to it are planned like any other, but its warehouse is kept in save data, its money in a bank
  account, and its Merchant is never seen. Caravans, contracts, bills and the treasury treat it like any desk.
  - **The front** opens when war is declared, 1,500 blocks north of the capital, with the treasury's account. Its
    quartermaster contracts the capital for what the army will want (whole kit sets, arrows, four days' rations),
    delivered by the capital's caravans. Levied recruits wait at the capital and ride out with those caravans. The army
    draws, wears out, eats and dies off the front's own stock: the end of the line. The front sells nothing; caravans
    going there carry their own food home.
  - **The market across the sea**, 2,400 blocks west, is a source: each day its stock of metal (and bread for
    travellers) is topped up.
- **Warehouses** (`Depots`, `/enterprise depot add <desk> <kind> <from> <to>`): a building whose chests, every one in
  its floorplan, are one store, part of its market's stock. What it's for decides what it holds: an **armoury** takes
  war goods, a **barracks** war goods and rations, a **storehouse** anything. Goods go to a building meant for them
  first, then to the desk's chests. A barracks' beds let its town put that many more men under arms. The capital has
  an armoury, and S2 has been promoted with a barracks.
- **The metal exchange buys by contract**, nearest holder first: a town with a mine, else the market across the sea,
  collected by the capital's caravans. It pays up to half again over worth as it runs short, and has buying shops at
  the port for players. The ships, and the price overrides in caravan buying and selling, are gone.
- **Training is a contract, and the market distributes it.** A training term ("train a Weaponsmith to level 5 for 24
  emeralds") is judged by the seller's census and paid when it shows the master, a third up front. The sovereign posts
  what it wants (2 master Weaponsmiths, 2 Armorers, 1 Toolsmith) as **tenders** on its board. Caravans carry each
  market's board both ways. A Merchant that reads a tender weighs it for its own town: the price, against a stall for
  a trade it lacks and coal and iron for each level to go, and whether there's time. It takes the tender if that pays.
  A Merchant whose town can't do it, but which has roads to towns of its own, carries the tender on, posting it on its
  own board for 15% less. Its contract with the first buyer is done when its town's is. A tender nobody takes lapses
  and is posted again for a quarter more.
- **The Merchant trains only under contract** (`Apprentices`): for each training contract, its pupil is the town's
  villager of that trade furthest along. The Merchant switches on what the pupil buys, pays for its work while the
  warehouse has none of what it buys (what's wanted first), and switches all of it off at the level. Jobs takes up a
  trade a training contract calls for.
- **Contract inputs are ordinary wants:** what a town needs to make what it has contracted to sell raises its
  warehouse's par (the stock it aims to keep), so caravans carry and sell it as they would anything it uses.
- **Armour comes in sets.** A soldier wears a helmet, chestplate, leggings and boots. Protection is the armour points
  worn: a full iron set is what a chestplate alone gave before, a full diamond set halves the risk. The stockpile and
  the quartermaster ask for every piece; Armorers sell them.
- **Game tests start from a fresh world:** `just gametest` wipes it first. Saved data (the mint's debasement, trust)
  carried over between runs and broke two market tests.
- **Measured** (120 days; capital named on day 1, mobilising from day 31, war from 61, peace from 101):
  - **The flows work:** the market across the sea delivered 3,625 metal by contract. The front received 313 pieces
    of kit and food, and issued 5 iron chestplates and 7 iron swords. War goods gathered in the armoury.
  - **Towns took the tenders and hired for them:** S1, S2 and S3 took up Armorer, Weaponsmith and Toolsmith stalls,
    and S1 carried a tender on to S2. **But no smith got past Level 2:** 13 training contracts broke.
  - **Why** (still to fix):
    1. S1 took 336 emeralds up front, broke contracts, and paid the advances back. Broke, it can't buy coal, so it
       can't train, so it breaks more.
    2. Its Merchant couldn't change coin for emeralds to pay its pupil, with 60 to 89 emeralds to spare; most likely
       its warehouse is too full to take them.
    3. Tenders are taken on a rough cost (2 emeralds a level, 2 days a level), not on whether coal and iron will
       actually reach the town in time.
  - Soldiers went hungry 34 of 87 days, and 3 deserted: the front's rations ran short between caravans.

## Travel cost between hexes (2026-10-13)

Travel cost must react to real building. A long wall, a tunnel, a bridge or a wide canal should each change routes and costs.

- **Two levels, like hierarchical pathfinding (HPA\*).** Inside a hex, a block-level 3D walk grid. Between hexes, a small graph.
- **Each hex side is split into entrances**, the open stretches of its border. A wall along a border with one gate leaves one entrance.
- **Per hex, we cache the cost from each entrance to every other**, per mode: on foot, cart, boat. Routes are A\* over the entrances graph, which is small and cheap.
- **A hex's costs are rebuilt on touch**: marked dirty when its blocks change, and rebuilt on the next round or census, off the main thread on a chunk snapshot.
- **Modes read different things:** foot needs 2 blocks of headroom and a 1-block step; carts need a road surface and width; boats need water depth and width (which is how a wide canal opens freighter routes).
- **Light sets cost underground.** Dark caves are slow and dangerous, so natural caves don't count as free tunnels, while a lit, built tunnel is cheap.

## Caravans (2026-10-13)

**Built so far (2026-10-14): caravans as ledger records, no entities yet.**
- **A route runs from a hub Merchant's desk to a settlement's Merchant's desk** (`/enterprise route add <from> <to>`). One caravan per route; at most one departure per route per in-game day; the hub's Merchant must be at its desk.
- **Outbound:** the hub loads up to 27 stacks of what the destination's switched-on trades take, from its stock and shulkers, plus a purse for the return leg's road food (1.5x what bread would cost). No purse, no caravan.
- **Caravans trade, they don't take (2026-10-14).** The travelling Merchant deals only with the resident Merchant, whose chest is the town's warehouse. It sells cargo into the warehouse up to par, buys the town's surplus for the trip home, and buys the return leg's road food at the local price. A town's coins are no longer swept up. **Home:** the caravan banks what it carries, after tolls, in the hub's Blaze Banker (or its chest); leftover and bought cargo goes into the hub's warehouse.
- **Warehouse pars.** Each locker session records what its switched-on trades can take in a day (price times uses per restock). A town's par for a good is two days of that capacity across its lockers, at least a stack for any good a switched-on trade takes. Capacity, not past use: par learned from use kept a short town short. Below par the resident buys; above par is surplus for sale. It also keeps 320 food points for caravans' road food and its emerald float.
- **Caravans are staffed by squads (2026-10-14).** A caravan takes one to four trained villagers from its home town: Merchants (not the one working the desk), Cartographers, Librarians and Shepherds, trained meaning they keep their trade without a workstation. Babies and untrained villagers stay home. One member is enough to drive the wagon (27 stacks, whatever the squad's size); each more goes only when its trade helps this trip and the town can spare its road food beyond the caravans' par, and every other route waiting at that desk keeps a lead. Squad members leave the world while on the road (their entity data rides in the ledger) and come back with the caravan; the census counts them as residents while away.
- **A squad comes from the desk's community (2026-10-14).** Only villagers its Merchant knows are mustered or trained: residents of its town (its census roster), workers at the lockers it manages, villagers who have gone out on its caravans before, and anyone standing in its market (within 8 blocks of the desk).
- **Any town sends caravans (2026-10-14).** The Merchant's Desk screen has a Caravans tab: the markets this desk knows, nearest first, with distance and direction, a Send caravans / Stop caravans button for each within a leg's food, the route's status, and who is at home to send. A route stopped while its caravan is out comes off the books when the caravan is home, so no squad or cargo is stranded.
- **News travels with carriers: each desk keeps an atlas.** A market knows those within 400 blocks by word of mouth; both ends of a route know each other once a caravan has run it; a town whose land a caravan crosses learns both its ends; Cartographers will map more.
- **Towns commission their own caravans.** At a toll review, a town with no route of its own, short of some kind of comfort, with 8 emeralds to spare beyond its reserve, starts a route to the nearest market it knows (within a leg's food) whose last known stock has that kind. Its caravan carries a shopping purse (the comforts it lacks, at full value, from money beyond its reserve) and buys them from the destination's stock beyond the destination's own par, bargained up to full value since they're for home. Leftover money comes home.
- **Towns train their squads.** A town with routes of its own trains one villager per count while loaded, up to a lead per route and one spare (at most 4): an unemployed adult of its community learns the trade the squad lacks (Merchant, Cartographer, Librarian, Shepherd, in that order), for 4 emeralds beyond its reserve. Trained, it keeps its trade without a workstation.
- **Catch-up waits for the Merchant (2026-10-14).** A caravan stop loads only the desk's chunk, and a locker villager loaded there used to replay its missed restocks while its Merchant, a few blocks over in an unloaded chunk, was missing: those days' trades were lost, and a town could go a whole run without trading (seen at S1, then S2). Now a villager waits to replay until its Merchant is loaded and managing its locker, looking again every 100 ticks; a round loads the whole market.
- **Cartographers map the road (2026-10-14).** A squad takes a Cartographer when its road hasn't been mapped in 10 days. At the destination it notes every settlement within 2 hexes of the road, which the home desk now knows of, and draws a map of the destination's country (512 blocks across, from what grows where, like an explorer map: nothing is loaded), the destination marked with an X and the towns seen marked as villages, named "Map of the road to x, z". The map rides home into the warehouse. The Caravans tab notes a mapped road and how many towns were seen; `/enterprise atlas <desk>` lists the markets a desk knows. Measured over 12 days: the hub's three roads were mapped once each, and the hub learned of the toll town beside the road to S3.
- **Measured** (`network_study.py --comforts`, 25 days): the toll town, which no route served, heard of the hub from caravans crossing its land, commissioned a caravan there for all four kinds of comfort, and its mood rose from -0.6 to +3.2; with no income of its own it spent its savings doing so. S1 commissioned one too, to S3, for the clothing S3 had bought from the hub. After the catch-up fix every town traded, and no locker went unmanaged for want of a loaded Merchant.
- **What each trade brings:** a Merchant haggles at its own level (without one, the squad haggles as a Novice); a Librarian brings back the books, the destination's stocktake, which the next caravan uses to know what to bring (taken along when the books are over 3 days old); a Cartographer walks the road (taken when it hasn't been mapped in 10 days; its maps come with the atlas); a Shepherd drives up to 8 grown farm animals from the market pen (within 8 blocks of the desk) and sells them where a town's pasture is short (2 animals a villager within 32 blocks of its bell), turning them out by its bell. The Shepherd is the first test of the fourth slot.
- **Measured:** sharing one squad of four among a hub's three routes, a squad took the whole of it and two routes sat idle; with one lead per route and extras only where they help, all three ran, with squads of one or two. Tying cargo to squad size (9 stacks a member) cut a town's trade from about 160 to 10 trades in 12 days, so cargo is per caravan.
- **Merchants bargain by level, not a fixed wholesale rate.** The seller won't go below half the goods' local value; the buyer will pay up to the full value with an empty shelf, scaling down to half at par (no deal at or above par). The price lands in that gap by level: the seller takes its level / (both levels) of it. The travelling Merchant's level is the hub Merchant's.
- **Travelling Merchants buy surplus only to fill a want.** A town's surplus is bought only for goods another town on the hub's routes wants, up to the sum of their pars less what the hub holds (from a stocktake the hub takes as each caravan leaves), at a ceiling of 75% of local value since it has to resell.
- **Prices** live in one table (`Prices`): what villagers pay for goods and crops, and what they charge for food, in items per emerald. Farm wages, wholesale and road food all use it.
- **Money moves through one treasury helper:** withdrawals take emeralds, then coins with change, then the Banker, and break an emerald at the mint when a part-emerald is owed.
- **Speed:** 1,200 blocks per in-game day. Arrivals load the desk's chunks on a short self-expiring ticket if nobody is there.
- **Food limits travel.** A caravan eats 1 food point (nutrition) per 10 blocks and carries at most 320 (a stack of bread), so a leg is at most 3,200 blocks; longer routes need a settlement in between as a waystation. **Each settlement's Merchant's chest is its food supplier:** a caravan eats the outbound leg's food at the hub and the return leg's at the destination. No food: it doesn't leave, or it waits at the destination.
- **Measured** (`dev/experiments/caravan_network.py`, pars and bargaining, each town starting with 64 emeralds): in 8 days the hub sold 7,916 items for 3,168 spurs and banked 3,404; its towns made 79-123 villager trades each and grew their money to 90-101 emeralds. A 4,000-block route was refused.
- **Working reserve refined:** a Merchant keeps 64 emeralds only if one of its switched-on trades costs emeralds; a selling-only settlement converts every emerald, so caravans have coins to collect.
- **Still to come:** encounters (spawning caravans near players, raiding), stealth and detection, tolls per hex, routes chained through waystations, surveying.

**Planned:**
- **A caravan is a cloud, not a position.** A route has traffic, and no one's location is simulated.
- When a player is in a route's corridor, an encounter roll runs, per hex:
  `chance = traffic × hex detection × (1 − merchant stealth)`.
- A hit spawns a real caravan with real cargo, checked out from its record. The record owns the cargo and the spawned caravan is a view of it, so it can never be duplicated. If the caravan crashes or gets stuck, the record wins.
- Merchant level adds stealth. Territories can raise a hex's detection. At full detection every caravan must show itself, so a guarded chokepoint enforces its own tolls.
- Raiding works on multiplayer: kill the caravan, loot the cargo, and the loss goes on the record.

## Build order (2026-10-13)

1. Lockers
2. Settlements (census, free-villager check)
3. Hex grid and territory
4. Route network (settlements as stops, food as travel supply)
5. Merchant (lockers, stock, counterparty, wages)
6. Caravans (encounters, stealth vs. detection, tolls)
7. Coinage and per-settlement banks

The cotton chain, chocolate, the cold chain and the exchange follow later.

## Open questions

- **Hex size tuning.** Measure real generated villages to confirm the starting size.
- **Improvements in player towns.** Is an improvement a recognized block structure (a farm plot, a mine entrance), something bought, or both?
- **Toll units.** Which units count as "the right type" to levy tolls?
- **Debasement.** Can a faction cut the metal in its coin to fund a war, at the cost of its exchange rate?
- **Guard mods.** Check whether an existing guard-villager mod for 1.21.1 NeoForge could supply the militia, or whether we write our own.
- **Scope.** Hexes, factions and garrisons push this toward a 4X game. The build order keeps each step playable on its own.

## Pallets and the binding (2026-10-18)

- **Goods have weight.** A pallet and each carrier have weight caps as well as slots (`docs/pallets.md`). The base
  content keeps 1 a stack, so nothing changes there; the Minecraft content makes ore, ingots, stone and blocks
  heavier, mildly at first, so a wagon of iron carries fewer stacks than a wagon of wool.
- **The weights** (`content-weights`; `ModPack.WEIGHTS`, mirrored by hand in `sim/host/src/goods.rs`). What a
  full stack weighs, in the unit where a plain stack is 1 (a pallet holds 9, a wagon 27): ore and raw metal, coal,
  bricks, ingots, logs and hay blocks 2; stone, cobblestone and sand 3; everything else 1 (wool, crops, paper, food,
  tools and armour, which stack to one and weigh one). Mild on purpose, 2 to 3 times: a wagon of iron ingots carries 13
  stacks within its weight and one of wool 27, so heavy trade costs a wagon something without ending it. The weights are
  data in the content pack the mod sends, not Java rules, and the base (toy) content keeps 1 a stack. Measured: the
  network study (365 days, with `--river --services --metal --trades --freight`) and the 300-seed sweep come out the
  same with and without them, since wagons there leave with about 2 stacks and never near a weight cap; the weights
  bite only on a full wagon of heavy goods, which is the loads the handling-units work will make. Hay blocks weigh 2
  against wheat's 1 though a block is nine wheat: baling is what makes fodder worth hauling (a wagon's team eats it).
  The tables differ where the goods do: the mod's weighs jungle and acacia logs and red sand, which the study doesn't
  deal in, and stone and brick, which the mod's pack doesn't list yet (nothing it prices or a recipe uses), so in the
  game they wait for that content. Nothing checks the two tables match: the host can't read the Java.
- **Pallets in the core only.** The core implements pallets; the mod draws them from it through the binding (crate
  items tagged from the core's pallet messages, a chest asking the core whether a pallet fits), with no pallet rules
  in Java. The mod's old bills-on-crates stay as they are until the binding retires them. Weights are content-pack
  data the mod sends the core, not Java rules.
- **The binding** (`docs/binding-plan.md`): chests stay physical and are synced with the core each look; saves survive
  core changes from the first release on (until then dev worlds are rebuilt); Chicory, serde-reflection (for the
  generator only) and postcard with our own Java generator, Chicory run through plain Gradle `JavaExec`; game tests
  that only call a Java rule retire with their class where a Rust twin stands in; a `java | core` switch until the
  Java rules are deleted; most Java-server studies retire in favour of `sim/host`, one in-game study kept as a check.
- **Pallets for crates, as landed (pallets-for-crates).** Every crate shipped under a bill of lading is a pallet in
  the core's registry, one for one, with no outcome moved. Kept for now so the numbers hold: the pupil's work is
  `Payment::Paid` though its bill names it (as `ByBill` the stale sweep and stranded paper would call it unpaid);
  stranded paper still also takes a drawee's paid-for pallets no bought bill names (by id alone, the design's rule,
  moves 2 of the 300 worlds: the hub's forwarder would carry a refused bill's pallet for a fee); `Discounts::backed`
  still reads the contract (reading pallets would back training bills). Each is a later task's to move.
- **A pallet is counted in one plan (pallet-claims; divergence 2 of `docs/pallets.md`).** The mod, and the core
  before, cloned a town's crates into every route's plan that look, so two routes to one buyer both counted (and
  pressed for) a crate only one could carry. Now a plan claims the pallets it counts, in route order (the first route
  wins), and later routes' plans go without them and plan no loose goods for what they cover. A route whose last try
  stayed for want of money, a Merchant, a squad or range plans after the others; a wagon that stays lets its claims go
  in `decide` and one that leaves in `ship`, so no claim outlives a look (and a pallet waits on a stuck wagon at most
  its first look). One look's delay is kept: a route that claims a pallet and then waits for a fuller load leaves the
  other routes without those goods that look. `ship` asserts every pallet on a manifest is home's and claimed by its
  route: the old "another wagon took it" skip is gone. In the 300 worlds the rule alone moves almost nothing (trips
  139846 to 139843, broken contracts 12708 to 12719); the random worlds' new claims slice adds its own contracts.
- **Pallets within their caps, loads within their carrier's (pallet-caps; divergence 1 of `docs/pallets.md`).**
  Packing splits a shipment at the pallet kind's caps (nine stacks and nine weight: one crate a pallet in the base
  content), a bill of lading each; a bill of exchange for crated goods names all its pallets, and the forwarding a town
  prepays is each pallet's (a little more drayage than one big crate group). Every carrier is loaded within its slots,
  weight and pallets: the plan clips the sale last-in first-out and then the last owed; a wagon is full at `wagon_full`
  of its slots or its weight; porters are sized by the larger of slots and weight beyond the wagon. Where this departs
  from the mod: a porter run carries one pallet within its pack's weight, so two contracts owed go by wagon (`porterRun`
  counts stacks only); `stranded::load` and a hub's buying at a town take a pallet only if it fits (`loadStranded` loads
  while any room is left, `buyStrandedAtTown` whatever its room), and a hub buys no bill there whose pallets it left for
  want of room; shopping at the far end buys no more of a good than the room holds (the mod checks room before each
  good, not within it); the plan counts a good owed apart from the same good for sale (on its own pallets), so it may
  take a slot more than the mod's merged count. `plan_owed` and the Ex Works collection now limit by room, as the mod
  does (the core's ports had left it out). A host hands in its pallets as it made them (`deliver_pallet`,
  `forward_pallet`, `book`, `hand_over` neither split nor refuse one over its kind's caps): the binding must keep a
  player's pallet within a pallet's worth, or a later task refuses it at the boundary. In the 300 worlds (a third with a
  good weighing 4 a stack): trips 144084 to 143529, items carried 17.66M to 17.07M (most of it the heavy good's worlds;
  without it items fall 0.4%), documents 80964 to 84071, broken contracts 12890 to 13060.
- **Packing by the grouping key (packing; divergences 3 and 4 of `docs/pallets.md`).** Goods are packed in packages of
  their own kind (crate, barrel, bundle, or none) and made up into pallets by the key (shipper, consignee, contract,
  payment, handling), goods of one key sharing a pallet to its caps; goods that need care never share a pallet with
  goods that don't. Where this departs from the mod: owed goods that can't be crated ride loose on their pallet under
  its bill (the mod's go loose in the cargo without papers), so a shipper with no crate and no money draws a bill a
  pallet; a farm's earmark tops up its open pallet of the key (the mod crates each lot apart, a bill each); an Ex Works
  collection takes an earmarked pallet whole, its bill now paid, where the rest due fits (the mod re-crates it), and a
  part pallet is opened at the seller's, the due part packed with a bill and the rest re-palleted with another. In the
  300 worlds by the rules alone: documents 84071 to 76253, scribes' fees 7466.0 to 6768.3, pallets 63132 to 62512,
  trips 143529 to 143114, completed contracts 18952 to 19039, broken 13060 to 13093. The mod keeps no pallets of its
  own: it will draw them from the core (`Society::pack_pallets`, `PalletPacked`/`PalletMoved`) once `pallet-views`
  gives the wire a `PackPallets` command and the goods' package kind and handling.
- **Superbills and consolidations (superbills; divergence 5 of `docs/pallets.md`).** A wagon on each leg is a
  container: its manifest has a line for each pallet loose aboard and one for each hub consolidation it carries. A
  manifest of two lines or more is drawn as a document, a superbill (kind `"superbill"`): the wagon's at its home as it
  leaves home (written once its stranded pallets are aboard too) and as it leaves the far end; calls at a hub on the way
  change the manifest without a new document. A hub's booked pallets on one run for one next stop, two or more, are made
  up as one consolidation the hub issues, its superbill drawn at the hub, loaded as one line, handed out whole at its
  stop and dissolved there, its pallets received still sealed. Where this reads `docs/pallets.md` "two pallets or more"
  as two lines or more: a consolidation is one line, so a wagon carrying only one consolidation draws no superbill of
  its own (the hub's superbill is its manifest), as one pallet's bill of lading is its own. Drops are written for booked
  pallets only, and the far end's `open` still finds what's made out to it by the registry (the same pallets the lines
  give): drops by line for the rest are `itinerary-drops`'. In the 300 worlds: documents 85129 to 90343 (5834
  superbills, 1263 consolidations), scribes' fees 7631.2 to 8102.3, trips 143891 to 143839, broken contracts 12943 to
  12884. The 365-day study's books are unchanged (no wagon there carries two lines). The mod keeps no containers of its
  own: it will see a wagon's manifest and a desk's pallets through the wire once `pallet-views` adds the query.
- **Consignment on pallets (consignment-pallets; divergence 6 of `docs/pallets.md`).** The hub's wagon at a booked
  town packs the town's goods for sale as pallets made out to the hub: shipper the town, no contract, paid (the goods
  stay the town's and nobody owes for them). They are crated at the town's cost where it can spare crates beyond its
  reserve, and go loose on their pallet where it can't. Each pallet has its bill of lading drawn at the town, is sealed
  as it goes aboard, and is a line on the wagon's manifest (a superbill for two lines or more). At home the hub opens
  them into its stock, where the forwarder's lots are sold from, as before. The lot is still told at collection, with
  each stack's fare. The mod's forwarder took the goods loose as a lot, so a consigning town now pays crates and paper on
  every collection. The crates don't come back: opened at the hub, they go to the hub's empties. In the 300 worlds:
  documents 90343 to 137907 (bills of lading 84509 to 121741, superbills 5834 to 16166), scribes' fees 8102.3 to
  12536.3, pallets packed 69141 to 107440, trips 143839 to 144359, broken contracts 12884 to 12898. The 365-day study's
  books are unchanged: no collection runs there, so it doesn't exercise the rule. Open: a consignment pallet lost on
  the road (the host's `PackageLost`) still leaves its lot counted whole, and the hub would remit for goods it never
  got. While the goods rode loose, nothing could be lost. The Java mod gets no pallet code for this: it shows the
  core's pallets through the binding (`pallet-views`).

## People and the binding (2026-10-19)

- **The census is the core's town's roster's source (people-adapter, B4.5).** In core mode the mod only counts: each
  look, the beds and villagers within 48 blocks of the bell nearest a loaded desk go to the core as `Census`, and the
  core decides who joins, who's seen and who has left (`census.rs`); villagers are views of its hands (`Born`,
  `Settled`, `Departed`, `HandGone`, `HandLevel`). Where this departs from the mod's `CensusTaker`: no path checks (no
  free and confined split: the culture input goes with the land concern); a town is counted only when every chunk of
  its reach has its entities loaded, in place of the "incomplete census" guard (fewer than half the last count within
  3 days not recorded); a villager not loaded that the adapter last saw in a chunk with nothing loaded, or never saw
  (it joined on touch, or the server restarted since), is sent as the core has it, so it isn't aged (the mod assumes
  it's where it was), but no ticket loads that chunk to find it (so one that vanished with no death, a villager turned
  witch, stays a hand); a level earned in the world is adopted, its XP below
  the next level is not (the core's `HandLevel` writes the core's XP over it); a child seen in the world joins as
  newly born. A profession taken in the world becomes the hand's trade, but never "none" (a villager that lost its
  workstation keeps the core's trade) and never the desk's Merchant's: **for John to confirm.**

## Lockers and the binding (2026-10-19)

- **The core runs the lockers (locker-adapter, B4.6).** In core mode a locker is still a chest touching a villager's
  workstation, but the mod only says where each stands: each look, for every loaded villager that is a hand of a town
  with a loaded desk, its workstation (if a container touches it) goes to the core as `Locker`, its chests a `Locker`
  point of the town's. The core decides the rest (`lockers.rs`, `locker_host.rs`): which the Merchant manages, which
  offers run, the restocks, two a day, loaded or not. So the mod's rounds (chunks forced loaded) and catch-up on load have
  nothing to do in core mode, and the villagers' own restocks are vanilla's (the restock hook runs no locker trades).
  A town founded by its host has a locker only where the host placed one; a town the core keeps as numbers (the study's)
  has every hand's, as the core always had it.
- **A locker's chests are the Merchant's shelf, not the town's store.** They're counted with the desk's at each look:
  what's in them is the town's (a player can leave goods there), what the town uses is taken from them first, and what
  they still hold the Merchant carries to the desk's and depots' chests as far as those have room. The core's trades
  run against the warehouse, so nothing is put into a locker first (the mod's `supply`) and a locker is empty after a
  look that found room for its goods at the desk; in the mod a locker kept its leftover inputs. **For John to confirm.**
- **The screen switches the core's offers.** In core mode the Merchant's screen lists the lockers the core says it
  manages, each with the core's offers at the villager's level (all of its level's: the core's simplification, not the
  two a villager rolls) and the Merchant's price; a switch flipped there is `Switch` to the core. The villager's own
  offers stay vanilla's, for players to trade with, and a player's trades don't reach the core's demand: the two lists
  can differ (a Librarian whose rolled trades don't include paper still has the core's paper offer). **Open:** sending a
  villager's rolled offers to the core (`Society::set_offers` is there for it), so the screen and the villager agree.
