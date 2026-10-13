# Staples: An Agricultural-Industrial Complex for Minecraft

Oct 13, 2026 · @John Chen

## Vision

Staples is a commodity-civilization game inside Minecraft: you build real factories, routes and depots at a few nodes, and the server turns their flows into a trading empire. Vanilla farming is a loop with no consequences, where three wheat make bread and nothing upstream or downstream cares. Here every step adds value, every step has a bottleneck, and each era exists to solve the last era's bottleneck.

Three layers carry the game. The factory is the puzzle, played at full fidelity in Create. The route is the strategy, shaped by geography, infrastructure and the cold chain. The ledger is the empire, where output becomes standing that you spend on embassies, warehouses and brands.

Cotton goes first because almost none of it is eaten, so it exists only through a chain of machines. Chocolate follows as the convergence product that no single town can make. The history is the spec and the factory is the fun: nothing is dumbed down, and every depth is optional.

## The core loop

The loop is build, flow, settle, spend. The player builds real things at a few nodes, those nodes produce and move goods, the server settles the flows on a ledger each day, and the resulting standing buys reach for the next build.

&#91;embedded content: the core loop, 5 steps and a sampled audit\]

You cannot be everywhere at once, so detail is full where you build and ledger-only everywhere else. An unloaded facility keeps producing at its last measured rate, drawing on real stock. Short, rotating chunk loads, called rounds, audit supply, security and logistics, so the ledger stays tied to the world.

Server cost stays flat as the empire grows, because a distant operation costs one ledger row, not a loaded chunk.

## Design pillars

Nine pillars hold the design together, and each one is a reason to build something.

1. **Place matters.** Climate and land decide what you grow well, so towns specialize and trade.
2. **Process matters.** Quality is made, not found: every stage can raise or ruin a grade.
3. **Scale is the progress bar.** Hand tools become workshops, workshops become mills, and mills become towns.
4. **Paper follows goods.** Receipts, bills, shares and contracts appear when the economy needs them.
5. **Risk is priced.** Every route and cargo has a measurable chance of loss, and someone sells cover against it.
6. **Infrastructure pays.** What you build, and where, is the reason: roads, harbors, rail, canals and cold stores change speed, capacity and risk.
7. **Mastery is a threshold.** Cheap paths always exist, and mastery clears thresholds that open better markets.
8. **One grammar, many crops.** Grade, bale, broker and exchange work the same for cotton, chocolate, grain and tea.
9. **Respect the player.** Real vocabulary, legible failures, and depth that is optional and never explained down.

## The player arc and the eras

The player grows from grower to maker to merchant, and each era replaces the last era's bottleneck. Selling raw fiber teaches that raw goods are cheap, building the chain teaches why mills exist, and trading the paper teaches why exchanges exist.

| Era | Make | Money and paper | Move | Cover |
| --- | --- | --- | --- | --- |
| Antiquity | Hand ginning, drop spindle, warp-weighted loom | Barter, item for item; granary-style receipts; cacao beans as early currency | Porters, pack animals, river boats; ice houses and snow cellars | Shared loss when cargo is jettisoned; sea loans repaid only if the ship arrives |
| Roman and trade | First water power for grinding; spinning wheel; treadle loom | Coinage; shared accounts as merchant partnerships; warrants | Carts on built roads, coastal ships | Merchant guilds spread risk; state-backed grain shipping |
| Early industrial | Jenny, water frame, mule, power loom, saw gin; cocoa press and conche | Companies and shares; spot market; newspaper; forwards | Canals, barges, early rail, sailing freighters; natural-ice trade, ice-cooled rail cars | Underwriting syndicates; mutual fire insurance for mills |
| Exchange age | Full mill towns, dye houses, finishing, chocolate factories | Standardized grades; futures; clearing house | Rail networks, steamships, aircraft for fine goods; powered reefers | Premiums priced from route grade, cargo and temperature record |

Each era unlocks through a broker trade tier or an advancement, not a tech click, so the town earns it. An optional medieval bridge adds the spinning wheel, bow carding and the roller gin, which suits long-staple cotton.

## Geography and settlements

Geography becomes an economic gradient instead of scenery, so every biome gets a role in a route and every world offers different opportunities. The system reads what players build and where, and the builder supplies the reason: a depot placed in a cold biome needs no simulated backstory.

| Terrain | Gift | Cost |
| --- | --- | --- |
| Jungle and warm coast | Cacao, sugar, long-staple cotton | Heat, spoilage |
| Ice and snow | Ice, cold storage | Distance, thin farmland |
| Mountains | Hydro power, minerals | Slow passes, expensive roads |
| Rivers and coasts | Cheap bulk transport | Depth limits vessel class |
| Desert | Open ground, clear sightlines | Heat, no water |

An ice biome between a jungle and a distant capital becomes a re-icing station, cold store, transshipment point and market, as the early-1800s ice trade out of New England did. Chokepoints such as passes, river mouths and crossroads gain value because traffic funnels through them, which is how real cities were placed.

Whoever owns a chokepoint can charge for it, which gives rivals a reason to build around it. Minecraft has no seasons, so ice is a permanent service unless seasons are added later.

## Production: the cotton chain

A bale of cotton passes through about eight stages before it becomes dyed cloth, and each stage maps onto a Create machine and a quality effect. The waterwheel and line shaft sit at the center: wheel output depends on fall height, and each machine wants its own speed band.

| Stage | What happens | Create fit | Effect on grade |
| --- | --- | --- | --- |
| Pick | Bolls become seed cotton | Harvester, deployer | Hand picking keeps a higher grade than rough machine picking |
| Gin | Seed is separated from lint | Mechanical saw for short staple; roller gin for long staple | The wrong gin tears long staple |
| Bale | Lint is compressed for transport | Mechanical press | Cleanliness sets the starting grade |
| Open and blend | Bales are loosened and mixed | Fans, mixer | Blending averages grades |
| Card and draw | Fibers are aligned into sliver | Rotating machine, high stress | Skipping drawing leaves uneven yarn |
| Spin | Sliver is twisted into yarn | High-speed shaft | Finer yarn is slower to make |
| Weave | Warp and weft become cloth | Sequenced assembly, deployers | Plain, twill and patterned weaves unlock by era |
| Finish | Scour, bleach, dye, print | Basin, spout, mixer | Mordants and vats decide color fastness |

Byproducts keep the chain branching: cottonseed becomes oil and feed, and linters become paper. Lint fire, humidity and thread breaks can be added later as optional hazards.

## Chocolate: the convergence product

Milk chocolate is the second module because no single town can make it: cacao needs a hot, humid region, sugar comes from elsewhere, and milk is local and perishable. Trade is forced, not optional. It took centuries to converge: Mesoamerican cacao drinks, sweetened European luxury, then the cocoa press (1828), the solid bar (1847), the conche (1879) and milk chocolate in the late 1870s, once condensed milk solved preservation.

The factory is the puzzle, station by station.

| Station | What happens | Create fit | Constraint |
| --- | --- | --- | --- |
| Ferment and dry | Beans gain flavor and a grade | Drying racks, fans | Sets the bean tier |
| Roast, crack, winnow | Beans become nibs and the shell is removed | Fans, crushing wheels | Sorting purity |
| Grind | Nibs become liquid cocoa mass | Mill, heated basin | Butter is a fluid on pipes |
| Press | Mass splits into butter and powder | Mechanical press | One input, two outputs to recombine |
| Blend and conche | Sugar, milk powder and butter are mixed and refined | Mixer, heated basin | Long run time; finer is slower |
| Temper and mold | Chocolate is cooled through set temperature windows, then molded | Heat levels, spout, deployers | The wrong window gives dull, crumbly bars |
| Pack | Bars are boxed for export | Packagers | Heat and odor sensitivity |

Recipes are flow-balancing puzzles, not crafting grids: cocoa mass, butter, sugar and milk powder must arrive in proportion. A longer conche makes a better bar but ties up a machine, so factories grow by adding parallel conches. Cacao beans also fit antiquity as an early currency.

## Perishability and the cold chain

Perishability is a second cargo dimension beside value density, and it decides which carrier and equipment a cargo needs. A refrigeration module costs payload space and power, and it pays for itself only when the value lost to spoilage on a trip exceeds its cost. Short hauls stay passive, and long routes force the decision.

| Class | Example | What goes wrong | What carries it |
| --- | --- | --- | --- |
| Ambient | Raw cotton, sugar | Damp, fire | Any carrier, dry storage |
| Ventilated | Cocoa beans | Moisture, mold | Dry, ventilated holds |
| Cool | Chocolate bars | Heat, bloom, odor pickup | Insulated, steady temperature |
| Chilled | Fresh milk, cream | Spoils in days | Refrigerated and fast |
| Frozen | Ice cream | Heat shock | Continuous cold |

Heat shock is the one to simulate: ice cream that thaws and refreezes loses a tier permanently, even if it arrives cold. Costs show up as a decay clock on delay, equipment that takes cargo space, a cooling load that rises with outside heat, breaks in the chain at docks and stations, and premiums that reflect thermal risk. A temperature log on the bill of lading lowers premiums and settles claims.

Each problem gets a cheap path and a mastered path:

1. **Move the last step:** ship intermediates and finish near buyers, including liquid chocolate in heated tankers.
2. **Passive cooling:** insulated boxes with packed ice on a fast route, with the ice consumed on the way.
3. **Powered reefer:** a container with a refrigeration module extends reach and holds grade.
4. **Full cold chain:** cold stores at origin, port and destination, with loggers, which unlocks premium export contracts that demand a continuous temperature record.

The top rung is gated by buyers who require proof, so mastery earns access to better markets and is not a stat check. By era, antiquity has ice houses, the industrial era has the natural-ice trade, ice-cooled rail cars and then mechanical refrigeration, and the exchange age has powered reefers with loggers.

## Quality, place and specialization

Quality uses four tiers in the style of Stardew Valley: Common, Fine, Superfine and Prime. Tier is set at harvest by variety, soil and care, then capped or nudged by each processing step, and blending averages lots and rounds down. For manufactured goods, tier is input grade times process care, so a Prime bar needs good beans and a long conche.

Place decides which tiers are reachable. Biome climate sets what grows well: long-staple cotton and fine cacao need hot, humid conditions, and hardy short-staple tolerates drier ground. A town's best crop is rarely the one it wants, so regions specialize and trade.

Each tier is a separate item with its own market. Colors, weaves, brands and other details ride along as tags for filters and buyer orders without changing the tier.

## Markets: global and local

Two kinds of market sit side by side. The global exchange, built on StockMarket, trades standardized grades such as bales, yarn, beans and sugar at impersonal prices. Local markets handle what does not standardize: fresh milk, boutique chocolate, dyed bolts and other luxury goods, where reputation, freshness and brand matter.

|  | Global exchange | Local market |
| --- | --- | --- |
| Goods | Standard grades | Fresh, branded, exotic |
| Price | Order book | Reputation and negotiation |
| Reach | Everywhere | One town or region |
| Needs | Paper such as warrants and bills | Presence: a stall, hall or broker office |

A local market is a block-form place, a market hall or stall built in a town, with its own demand and stock. Heavy net selling lowers a good's price range on the exchange, so flooding a market pushes players up the value chain.

Distant cities are simulated markets with their own demand, much as StockMarket's plugins simulate a player base on small servers. Standing earned from real flows buys access to them: a stall, a broker office or an embassy.

## Commerce and finance

Finance modernizes alongside manufacturing, and each instrument solves a problem the previous era created. Barter gives way to coin, coin to receipts, receipts to shares, and shares to futures.

- **Spot market:** bales, yarn, beans and sugar trade on a real order book with candlestick charts, and villagers and brokers price from it.
- **Warrant:** a shulker of one grade is stamped into a certificate backed by goods in escrow. It is the warehouse receipt, and it can trade instead of hauling.
- **Prepaid forward:** a warrant with a delivery date. The buyer pays now, the seller gets working capital, and the goods arrive later.
- **Company shares:** mills and factories incorporate, issue shares and pay dividends, which can be paid in finished goods.
- **Futures:** cash-settled contracts with margin come last, once standardized grades exist.

&#91;embedded content: goods and paper by stage, 5 stages\]

Each stage of a bale's life has its own paper, and the paper can be sold while the goods stay put.

## Logistics

The bill of lading is what turns logistics into a system: it is a receipt for goods loaded, a contract with the carrier, and a document of title. Ownership separates from location, so whoever holds the paper owns the goods, even at sea.

Vehicles earn their roles by value density, and cargo class from the cold chain decides what equipment they need.

| Carrier | Best cargo | Why |
| --- | --- | --- |
| Carts and caravans | Low-volume local trade | Cheap to start, slow, exposed |
| Barges and ships | Raw bales, grain, coal, cocoa beans | Cheap per unit, slow, need water depth |
| Trains | Bulk on land routes | Cheap per unit once track exists |
| Aircraft | Fine dyed cloth, dye stock, top long-staple lots | Fast, small loads, expensive |

Players choose between two routes through the same economy. On the paper route they contract a carrier, hold a bill of lading and trade goods without touching them. On the builder route they lay their own track, build aircraft and schedule caravans.

Building should beat contracting on margins, while contracting stays a viable floor. Long sea routes can be abstracted into a document and a delay, so players build local and regional links without building an ocean.

## Risk and insurance

Insurance turns the risk of a route into a price that falls when the route improves, which is what makes infrastructure worth building. Every cargo move carries an expected loss, and an underwriter sells cover for that loss plus a margin.

Three things set the risk:

- **Route:** distance, terrain, weather exposure, how much real infrastructure the path has, and how secure it is and who controls it.
- **Cargo:** value density, sensitivity to damp, spoilage and heat, and packaging such as sacking and ties.
- **Carrier:** vehicle class, reliability and reputation.

Three kinds of party supply the cover:

- **Underwriter companies:** players stake capital in a company account, earn premiums, and pay claims from reserves. Shares and dividends come with the company.
- **A default insurer:** it prices from route grade, so single-player always has a floor.
- **Policies:** paper like any other, a certificate tied to a cargo and its bill of lading, with claims verified by a loss adjuster's visit.

Insurance evolves with the eras. Antiquity has shared loss when cargo is jettisoned and sea loans repaid only if the ship arrives. Later eras add underwriting syndicates and, for mills, mutual fire insurance. The exchange age prices premiums directly from route grade, cargo and temperature record.

Losses fall mainly on abstracted carriage and caravans. Player-built vehicles stay predictable, with risk showing up as speed, cost and later optional hazards, so nobody's own train loses cargo at random.

## Infrastructure pays

Infrastructure pays when the world is the rating table. A surveyor reads the blocks a player has built and where, and the score drives speed, capacity, loss risk and premium. A caravan on a graded, bridged road is faster and safer than one on a trail, and an insurer charges less for it.

| Build | Block signature | What it changes | Who feels it |
| --- | --- | --- | --- |
| Graded or paved road | Surface, width, bridges, lighting | Wagons replace pack animals; faster, fewer losses | Caravans, insurers |
| Caravanserai (waystation or inn) | Enclosed, supplied rooms along a route | Range per day, shelter | Caravans |
| Waterway, locks, towpath | Channel depth and width, chamber size | Vessel class: barge, coaster, freighter | Carriers, riverside towns |
| Harbor | Quay, depth, cranes, lighthouse | Draft class, loading speed, wreck risk | Shippers, underwriters |
| Rail | Track, signals, stations | Lowest cost per unit for bulk | Mills and growers |
| Airfield | Runway, hangars | Fast delivery of small, high-value cargo | Dye houses, boutiques |
| Cold store | Insulated volume, ice or power | Holds grade, resets the decay clock | Perishable shippers |
| Bonded warehouse | Enclosed, secure, dry | Less damp, fire and theft | Warrant issuers, underwriters |
| Post station | Stable with horses, a desk and a lectern | Cuts information lag on each leg of the route | Merchants, insurers, exchange traders |

Function depends on supply and power as well as shape, so a decorative ice shack with no ice does not count. The structure says what is possible, and operation says whether it is happening.

A canal is the extreme case: it rewrites geography by adding a route through a chokepoint. The Panama Canal opened in 1914, and its lock size defined the Panamax class of ship. In the game the dig is the cost, shorter routes and larger vessels are the payoff, and width is a decision: a narrow canal is cheap and passes barges, and a wide one is expensive and admits freighters.

Owners of roads, canals and rail can charge tolls and freight rates into a company account, issue shares and pay dividends, as turnpike and canal companies did. That gives builders a reason to build for others, and it explains why roads, rail and harbor towns exist.

## Security and sovereignty

Valuable nodes attract threats and claimants, so a choke point that earns from passing traffic also draws hostile mobs to its warehouses and rival states to its waterways. Both pressures give the player a reason to move goods to places they do not live in.

**Local security.** Every node has a security rating, read from what is built around it and refreshed by rounds. A village whose warehouses keep being blown up by creepers is a low-security node: stock is lost, capacity drops and premiums rise. The player can fix it by building on site, or by shipping what a garrison is made of, such as iron blocks and carved pumpkins for iron golems, and the local population does the rest. Security decays without resupply, so the shipments become a standing route.

| Level | What it looks like | Typical shipments | Effect |
| --- | --- | --- | --- |
| Open | Dark, open ground | None | Frequent losses to mobs; high premiums |
| Lit | Lit perimeter, fenced yards | Torches, lanterns, fencing | Fewer spawns near stock |
| Watched | Watch posts, cats near the stores | Planks, stone, raw fish for cats | Early warning; creepers kept off |
| Patrolled | Iron golem patrols | Iron blocks, carved pumpkins | Losses rare; premiums fall |
| Fortified | Walls, gates, a garrison | Stone bricks, iron, arrows | Safe to hold large stock |

**Sovereignty.** A region is held by a sovereign, either a simulated state or a player-run polity, that controls its waters and roads. A sovereign levies tolls on passing cargo, as Denmark did on ships through the Oresund for centuries. Control is read from a block-form customs house on the waterway, so it is visible and can be contested.

When a neighboring sovereign tolls a choke point heavily, the player has four options:

1. **Pay the toll.** It is the floor and always available, and its size depends on how the sovereign regards you.
2. **Buy favor.** An embassy lowers tariffs and opens contracts.
3. **Back a claimant.** A rival is fighting for the same choke point, and wars are decided by supply. Shipments of iron, food, arrows and other war goods tip the contest. If your side wins, tolls fall and your favor pays out. If it loses, expect retaliatory tariffs.
4. **Build around it.** A canal or overland route removes the choke point's leverage, at the cost of the dig.

No armies need to be simulated. A war is a supply contest resolved on the ledger, and the player's part is a logistics problem to solve and protect, which keeps Create and the caravans at the center. Wars and blockades raise premiums on affected routes.

**Becoming a sovereign.** Players can found a polity as a late-game goal, and it is meant to be hard. Founding takes a charter bought with standing, plus a treasury to hold. Holding a region means holding its obligations, so a player new to empire should expect trouble:

- **Revenue comes with upkeep.** Tolls and taxes pay into a treasury that also pays for customs houses, garrisons and roads. A region that stops being supplied loses security and then loses its trade.
- **Subjects have demands.** Settlements in your region expect security, supplies and stable prices, and unmet demand brings unrest and lost tax income.
- **Rivals are always claiming.** A sovereign becomes the target of the same supply contests the player once joined, and every neighbor with a grievance is a candidate claimant.
- **Overextension bites.** Each border and choke point is another garrison to feed and another customs house to keep.
- **Absence matters.** An offline ruler needs a regent or a council, because a polity cannot go unattended on a multiplayer server.

Failure should be dramatic but recoverable. A fallen sovereign loses provinces and standing, never the save, and can abdicate or be deposed and play on as a merchant or a claimant.

## Merchants and caravanserai

The Merchant is a villager profession that does what the wandering trader should: it is hired, persistent and skill-leveled, and it automates the tedious parts of trade. Manual trading stays as the cozy path, and merchants are the mastery path.

A Merchant does three jobs.

**Surveys routes.** A Merchant escorted by a cartographer villager and pack llamas walks a candidate route. The trip registers the route, scores the path the way a surveyor would, and produces a trade route map item that shows it.

**Runs standing orders.** The Merchant keeps a roster of every villager in an area, with their trade tables and restock state, and executes orders the player sets: buy at or below a price, sell at or above one, or keep a stock level. The player never trades by hand. The spread between villager margins and exchange prices is what a Merchant works against, and a better-leveled Merchant tracks more villagers, runs more orders and negotiates a tighter margin. Wages are paid through BankSystem recurring payouts.

**Staffs caravanserai.** A caravanserai is a block-form waystation on a critical route. A Merchant stationed there sends carts to the surrounding settlements to restock what it consumes.

| Supply | What it keeps running |
| --- | --- |
| Food | Crews, animals and herds on the road |
| XP bottles | Mending of equipment and vehicles |
| Redstone dust | Maintenance of facilities |
| Paper | Upkeep of the information network: maps, notices, bills |

If supplies run out, the caravanserai degrades, and the route's grade, speed and insurance premium degrade with it. That gives neighboring villages a steady buyer and gives the player a reason to keep them healthy.

A Merchant on the road is a unit at risk. Losses can be insured, and the security and sovereignty rules apply to it like any other cargo.

## Squads: villagers on the road

Any villager from the player's town can join a caravan, and each brings a perk that scales with profession and level. A villager's level rises through trading, which is the existing mechanic, so a trained villager is valuable and sending one away has a real cost.

| Profession | Perk on the road | Best on |
| --- | --- | --- |
| Librarian | Knowledge and literacy: reads and writes bills of lading and policies, and unlocks market intel and document tiers | Any route that uses formal paper |
| Cleric | Healing for crew and animals; fewer losses to injury and sickness | Long or dangerous routes |
| Farmer | Boosts food production at the settlements and caravanserai the caravan stays at | Outposts and caravanserai |
| Smith (armorer, weaponsmith, toolsmith) | Maintains equipment and vehicles; fewer breakdowns | Heavy loads and rough roads |
| Shepherd | Lets the caravan travel with herds, as livestock cargo and as a moving food supply | Drove routes, wool and meat trade |
| Fisherman | Produces food passively along water | River and sea routes |
| Mason | Repairs roads, bridges and structures along the way | Routes the player built |
| Weaver | Inspects cloth lots and trades textile goods, so fewer adulterated bolts slip through | Textile routes and mill towns |

Herds tie into the cold chain. Before refrigeration, meat traveled on the hoof, so a shepherd's herd is a self-moving food supply that needs no reefer.

Squads trade off in four ways:

- **Wages:** paid through recurring payouts.
- **Absence:** the home town loses that villager's trades and production while they are away.
- **Exposure:** a villager on the road can be lost, which insurance does not fully restore.
- **Slots:** the vehicle or animal class limits caravan size, so each squad is a choice of which perks matter on this route.

A dangerous route gets a cleric and a smith, a drove gets shepherds, and a long river run gets fishermen.

## Core mechanics: cheap to simulate

Six mechanics add depth at almost no server cost. Each one leans on something the game already does and reduces to a few numbers and simple arithmetic, evaluated when something touches the batch, node or population, never every tick. Per-batch state lives on the container or in a record, not on item stacks, because items carry only a tier or a line name.

| Mechanic | Existing mechanic it leans on | The arithmetic |
| --- | --- | --- |
| Batch clock | Furnace-style progress, Create heat | progress = progress + rate × elapsed time |
| Information lag | Horses, villagers, maps | price seen = price at (now − lag) |
| Documents against payment | BankSystem locks and timed payouts | cash today = face × (1 − rate × days to due) |
| Inspection and reputation | Villager professions and levels | catch chance = 1 − (1 − a)^s |
| Carry cost | BankSystem recurring payouts | cost = (rent + shrink + interest) × days |
| Seed lines | Crop stages, bone meal, farmers, biomes | new mean = old mean + step × farmers' pull |

Together they explain themselves: grade starts in the seed line, is shaped by the batch clock, is protected by inspection, financed by documents, priced by carry cost, and moved at the speed of information.

### The batch clock

Fermentation, drying, curing, aging, tempering and spoilage are one mechanic: a progress bar with a window, the same shape as smelting, where heat advances a bar until the item is done.

Each batch stores a progress value and the time it was last updated. When anything touches the batch, the game applies one line: progress = progress + rate × (now − last update), where rate = base rate × container factor × temperature factor. A window then turns progress into a tier. Inside the sweet spot the batch gains a tier, short of it the goods stay raw, and past it they lose a tier per step, down to spoiled.

- **Fermentation and curing:** a sweet spot in the middle, such as cacao beans in a ferment box.
- **Spoilage:** the window starts at zero and its end is the shelf life, so warmth shortens the life and cold storage stretches it.
- **Aging and tempering:** windows that reward patience or a held temperature.

Containers set the factors. A cold store has a rate factor below one, a drying rack with fans sits near one, and a heated basin sits above it. Heat shock is a one-way drop in tier when temperature crosses a threshold. Everything is one multiplication per touch.

### Information lag

A distant market's price reaches you at the speed of the fastest channel on the route, so what you see is the price as of an earlier time. The arithmetic is price seen = price at (now − lag), where lag is the sum of the legs along the route and each leg is distance divided by the speed of its channel.

Channels run from runners and pack animals through horse relays, ships and rail, to a telegraph with almost no lag. Every document carries an as-of date, and acting on stale news is a gamble, because the market moves while the letter travels. A bill that arrives before the cargo lets you sell goods that are still at sea.

Lag turns settlements into an information network:

- **Post stations:** a stable with horses, a desk and a lectern every stretch along a road. Each station replaces a slow leg with a fast one.
- **Horses:** a courier leg's time comes from the horse's speed stat, so breeding fast horses shortens lag across the whole network.
- **Merchants and librarians:** spread along the settlement network, merchants keep local price records and librarians read and write the documents, which makes bills, warrants and policies effective.
- **Paper:** caravanserai paper stocks keep the network running.

### Documents against payment

Credit lets a player be rich in goods and short of cash. A bill of exchange, issued against a bill of lading, promises payment on a due date, and three rules cover it:

- **Documents against payment:** the buyer's money is locked at purchase and released when the bill of lading is presented at the destination, using BankSystem's lock and transfer-from-locked calls.
- **Discounting:** the holder can cash a bill early for face value minus interest, so cash today = face × (1 − rate × days to due).
- **Default:** an unpaid bill forfeits the locked collateral and costs reputation.

Maturity can ride on BankSystem's one-time delayed payouts, so the bank does the waiting. The interest rate here is the same one that prices the carrying of goods.

### Grading, adulteration and inspection

Every lot declares a grade, and a seller can adulterate it by mixing in lower-grade goods for margin. Detection is sampling, and the arithmetic is simple: if a fraction a of a lot is adulterated and an inspector samples s units, the chance of catching it is 1 − (1 − a)^s. With 10% adulterated, 10 samples catch it about 65% of the time and 30 samples about 96%.

| Domain | Inspector | Existing mechanic used |
| --- | --- | --- |
| Crops | Farmer | Villager profession and level |
| Meat | Butcher | Villager profession and level |
| Fish | Fisherman | Villager profession and level |
| Cloth | Weaver | New profession, below |
| Any lot, higher throughput | Merchant | Boosts samples taken per day |

- **Sample size:** s scales with the inspector's level, so a master samples more units per lot than a novice. A Merchant at the inspection hall raises throughput, the number of lots sampled per day.
- **Reputation:** each seller has a reputation number. Caught adulteration cuts it by the grade gap times a penalty, clean deliveries raise it slowly, and high reputation lowers how often your lots are sampled.
- **Expected value:** cheating pays only when the gain, the adulterated share times the units times the price gap, beats the catch chance times the penalty. The player can run the sum, which is the point.
- **Inspection halls:** capacity is infrastructure, a block-form hall at a port or market with inspectors stationed, so busier trade needs more of them.

**The Weaver.** A new villager profession with its own job site, since the loom already belongs to the Shepherd. Weavers trade in the cotton industry, buying lint and yarn and selling thread, cloth and dyed bolts, and they inspect cloth lots for low-grade or adulterated bolts. At higher levels they handle patterned weaves and designs.

### Carry cost

Holding goods costs money, and one formula covers warehouses, warrants and futures: carry cost = (rent + shrink + interest) × days. Rent is the warehouse's price per slot, shrink is the loss rate for the cargo class, and interest is the going rate times the unit's value.

- **Warehouses become a business:** rent can be paid through BankSystem recurring payouts.
- **Warrants** carry the cost of what they hold.
- **Futures** sit above spot by the cost of carrying to the delivery date: future price = spot price + carry cost per day × days to delivery.
- **Spoilage links in:** a cargo that decays on the batch clock has higher shrink, so cold storage pays for itself against it.

### Seed lines

A variety is a seed line, a named record of about six traits, and a deliberately thin genetic model lets players breed them.

| Trait | What it changes |
| --- | --- |
| Yield | Drops per harvest |
| Grade chance | Probability of each tier at harvest |
| Seeds per harvest | How fast the line spreads |
| Bone meal response | Growth chance per bone meal |
| Stage count | Growth stages, and so time to ripen |
| Biome affinity | Multiplier for each biome |

Each region has a seed population with a mean value per trait, and its farmers shape it. Every farmer villager favors one trait, and their level sets how hard they pull. When a harvest completes, each trait mean moves a small step toward what the farmers select: new mean = old mean + step × the farmers' pull. That is one addition per trait per harvest for the whole region.

Hybridizing uses the same arithmetic. A player brings a line from elsewhere and plants it on a town farm, and the seeds it produces take trait values equal to the average of the imported line and the local population, plus a small random variation. Repeating the cycle blends the two, and the local farmers pull the result toward their preferences. A hybrid that stabilizes is registered as a named line.

Two rules keep it balanced and cheap:

- **Trait budget:** traits share a budget, so raising one lowers the others. A line can be high-yield or high-grade, but never the best at everything.
- **Coarse items:** seeds carry only the line's name. Trait records live in the region and the line, so banking and markets see a bounded number of items.

Trade routes carry seeds. Merchants and caravans bring distant lines home, and a good survey finds lines worth importing.

## The ledger and the sampled world

The ledger runs the economy, and rounds audit the world against it. Each registered facility has a capacity and each route a schedule, and once per in-game day the server moves goods and money between accounts, applies prices, spoilage and insurance, and records the result. No chunks need to be loaded for that.

Offline production works by sampled rate. While a factory runs loaded, the system measures its real output over a window. While unloaded, it keeps producing at that measured rate, drawing on real input stock and stopping when the stock runs out. Four guardrails keep it honest:

- Rates are measured, never claimed.
- Accumulated output is capped after a set number of days.
- Facilities decay without upkeep.
- StockMarket's price response gives scale diminishing returns.

**Rounds.** A short, rotated chunk load of each facility's footprint reports on throughput, supply, logistics, security, structural integrity and route health. A failed security check raises route risk, a broken signature lowers capacity until repaired, a shortage halts offline output, and a bottleneck becomes a plain foreman's report. Visits are ranked by time since the last visit, value flowing through and past problems. Output drift, an insurance claim (a loss adjuster) or a player request can also trigger one.

Paying a foreman raises visit frequency, so wages are a real cost of scale. Server cost stays bounded by a hard budget, footprint-only loads and randomized timing inside a visible window.

**Standing.** Real flows earn a score that is spent on ledger entries:

| Sink | What it buys | Historical echo |
| --- | --- | --- |
| Embassy or consulate | Lower tariffs, regional contracts | Consuls, trade treaties |
| Warehouse lot in a distant city | Storage and cold-storage capacity | Hanseatic and Venetian trading posts |
| Market stall or broker office | A local sales channel | Merchant quarters |
| Charter | The right to run a route or issue policies | Royal trading charters |
| Brand registration | Reputation that lifts a product's price | Guild marks, trademarks |

A warehouse in the capital is a bank-account slot tied to that city, so goods can be deposited, traded and sold there without loading it. Few-player servers feel populated because distant cities are simulated counterparties. Whether Create machines reach steady state quickly after a short load needs testing; if they do not, rounds fall back to flagging drift from the last measured rate.

## Roles and multiplayer

Roles emerge from the economy, and each can be a company: grower, miller, chocolatier, shipper, broker, banker, insurer and infrastructure owner. In multiplayer they depend on one another. In single-player or few-player servers, NPC caravans, a Broker villager, a default insurer and simulated distant cities fill the roles the player skips, so the full economy is always present.

The Broker profession is the early gateway. It buys bales first, posts cloth orders later, and unlocks the next era through trade tiers.

The Merchant is the persistent, skill-leveled counterpart to the Broker: it runs routes and standing orders for the player. Sovereigns can be simulated states or player-run polities, a town's villagers can join caravans as squads, and the Weaver handles the cotton trade and inspects cloth.

## Project shape: modpack, shared core and modules

Staples ships as a modpack built on one shared core mod, with every crop and good as a data-driven module on top. Four layers keep it maintainable:

1. **Shared core mod (code):** the batch clock, tiers, documents (warrant, bill of lading, bill of exchange, policy), route registry and surveyor, ledger and rounds, inspection and reputation, seed lines, the Merchant and Weaver professions, and squads.
2. **Content modules (data):** each crop or good is a datapack of varieties and trait ranges, recipes, batch-clock windows, cargo classes and inspector domains. Adding tea is data, not code. Cotton and chocolate come first, then dye crops, grain milling, sugar, tea and salt.
3. **Adapters:** thin bridges to Create, BankSystem, StockMarket, Numismatics and Tobacconist. Each is optional where possible, so the core stands alone.
4. **The pack:** configs, advancements for the era unlocks, and a curated set of dependency mods and versions.

Four rules shape every system:

- **Cheap to simulate:** store a few numbers per batch, node or population, and compute on touch, never per tick.
- **Existing mechanics first:** reuse smelting-style progress, villager professions and levels, horse stats, bone meal, biomes, and BankSystem locks and payouts before inventing anything.
- **Data over code:** a new product should need only a datapack.
- **Coarse items:** items carry a tier or a line name, and richer state lives in records, to stay inside BankSystem's item cap.

What the design leans on, and what has been checked:

- **Create** supplies machinery, trains and logistics.
- **BankSystem** (MIT license) supplies accounts, shared accounts, companies with shares and dividends, and balance locking for escrow and margin. Its data lives in world storage rather than in loaded blocks, which the ledger model follows. Its item identity uses full data components with a lifetime cap of 32,767 item IDs, so item variety and certificate series must stay bounded.
- **StockMarket** supplies the order book, plugins, news events and villager repricing. Its documentation folder could not be read, so whether an add-on can create markets or register plugins is unconfirmed. Its repository showed no license file.
- **Tobacconist** (All Rights Reserved) can join through an adapter on public items and tags, not bundled code.
- **Version target** looks like 1.21.1 on NeoForge, to be confirmed. Ask both authors about licensing before building on their work.

Sampled rates, rounds, the batch clock, inspection and seed lines are new systems of our own, and nothing in the dependency mods provides them.

## The first vertical slice

The first slice is one crop, one region pair, one carrier, one finance step and one ledger flow, working together end to end before anything widens.

1. **Grow and gin:** short-staple and long-staple cotton, two regions with different yields, hand and roller ginning.
2. **Bale and sell:** bales with tiers, a Broker villager, and one market per tier.
3. **Warrant:** stamp a shulker of bales into a warrant and trade it.
4. **One carrier:** a cart or barge route with a surveyed route grade and a default insurer.
5. **First mill step:** a waterwheel-driven carding and spinning line that makes yarn worth more than raw bales.
6. **One ledger flow:** a registered node settles daily at its sampled rate, and one round audits it.
7. **Two cheap mechanics:** bale conditioning on the batch clock, and one lot inspected by a Weaver so adulteration has a price.

If that slice is fun, the rest is expansion: weaving and dyeing, shares and forwards, then chocolate and the cold chain, rail and ships, embassies and local markets, and finally futures.

## Open questions and risks

- **Scope:** this is a very large content set, so the discipline is the vertical slice above.
- **Market API:** can an add-on create StockMarket markets and register plugin types? The answer decides how futures get built.
- **Item identity:** certificates must be fungible series, not unique items, and perishability and seed lines must stay coarse, to remain inside BankSystem's item cap.
- **Rounds:** how quickly do Create machines reach steady state after a short chunk load? This decides how reliable audits are.
- **Offline production:** guardrails must stop unattended output from becoming an exploit or an AFK farm.
- **Era gating:** is era progress global per server, or per town?
- **Delivery dates:** are delivery periods measured in Minecraft days or real time?
- **Temperature:** how is heat represented, as block properties, a coarse item state, or only at the route level?
- **Sovereignty:** players can found polities late in the game, so how are regions bounded, and how do offline rulers and succession work?
- **War resolution:** how often do supply contests resolve, and what stops a snowball?
- **Villagers away:** how is a villager stored while on a caravan, and how do we rule out loss or duplication bugs?
- **Merchant orders:** how do standing orders interact with StockMarket's villager repricing and margins?
- **Weaver profession:** which job-site block does it use, and how is the profession added cleanly alongside vanilla and other mods?
- **Seed lines:** how fast should regional populations drift, and what caps the number of named lines per world?
- **Information lag:** which prices show as stale, and does the global exchange stay live for players at a terminal?
- **Inspection tuning:** how large are reputation penalties and sample sizes so cheating stays tempting but risky?
- **Hazards:** dust, fire, humidity and bandits stay optional toggles, designed later.
- **Licensing:** confirm terms with the Tobacconist and StockMarket authors before building on them.

## Sources

Pages opened for this draft, as of 2026-10-13. Historical points in the era, chocolate, cold chain and canal sections are from general knowledge and are not sourced here.

- [Tobacconist on CurseForge](https://www.curseforge.com/minecraft/mc-mods/tobacconist)
- [BankSystem](https://github.com/KROIA/BankSystem)
- [BankSystem API reference](https://github.com/KROIA/BankSystem/blob/main/documentation/developer-guide/API.md)
- [BankSystem configuration](https://github.com/KROIA/BankSystem/blob/main/documentation/user-guide/Configuration.md)
- [BankSystem companies](https://github.com/KROIA/BankSystem/blob/main/documentation/user-guide/Companies.md)
- [StockMarket](https://github.com/KROIA/StockMarket)
