# How the Power Grid Really Works
### A visual storybook for consumers wondering where their money goes

> **Format note:** This document is a script and content guide for an interactive
> visual story in five pages. Each page is a self-contained chapter designed as a
> full-screen panel with animations or interactive elements. The narrative builds
> from physical reality up to markets, trading, and the green transition -- giving
> the reader enough understanding to form their own opinion on whether energy
> trading benefits them.

---

## Page 1: The Balancing Act

**Visual:** A tightrope walker on a wire labeled "50 Hz". Below, a city glows
on one side and power plants hum on the other. The wire wobbles gently.

### The one rule of electricity

The grid has a single non-negotiable constraint: **supply must equal demand,
every second of every day.** There is (almost) no warehouse for electricity.
The moment you flip a light switch, a generator somewhere spins a tiny bit
harder.

This balance is measured as frequency -- in Europe, exactly 50 Hz. If demand
exceeds supply, frequency drops. If supply exceeds demand, frequency rises.
Drift too far from 50 Hz and equipment starts failing, protection systems
trip, and blackouts cascade.

**Key insight:** Everything that follows -- markets, trading, pricing -- exists
to solve this one problem: keeping the wire balanced.

> **Animation idea:** A frequency meter hovering at 50.00 Hz. The reader drags a
> slider labeled "Demand" up. The frequency dips. Then generators visually "spin
> up" to compensate, restoring balance. Reverse when demand drops.

---

## Page 2: The Merit Order — Who Gets to Sell?

**Visual:** A bar chart filling from left to right. Each bar is a generator type,
ordered by cost. A horizontal "demand line" slides across, and everything to its
left is switched on.

### The auction that sets your price

Every day (and intraday), electricity is auctioned on power exchanges. Generators
submit offers: "I can produce X megawatts at Y euros per megawatt-hour." These
offers are stacked from cheapest to most expensive. This stack is called the
**merit order**.

Typical merit order (left to right, cheapest to most expensive):

| Position | Source | Marginal cost | Why |
|----------|--------|--------------|-----|
| 1 | Wind & Solar | ~€0/MWh | No fuel cost -- the wind is free |
| 2 | Nuclear | ~€5-15/MWh | Fuel is cheap, plants run continuously |
| 3 | Hydro (run-of-river) | ~€5-20/MWh | Water is free, but capacity is limited |
| 4 | Coal/Lignite | ~€30-60/MWh | Fuel + carbon permits |
| 5 | Natural gas (CCGT) | ~€50-90/MWh | Gas price + carbon permits |
| 6 | Natural gas (peaker) | ~€80-150/MWh | Less efficient, started only at peak |
| 7 | Oil (emergency) | ~€150+/MWh | Last resort |

The demand line slides across this stack. Every generator to the left of the line
is "dispatched" (turned on). **The most expensive generator that is still needed
sets the price for everyone.** This is called **marginal pricing**.

### Why marginal pricing?

This feels unfair at first -- why should a wind farm that produces at €0 get paid
€70 because some gas plant was also needed? Two reasons:

1. **Investment signal.** If wind farms only got paid €0, nobody would build them.
   The spread between their cost and the marginal price is what repays the
   investment in turbines, panels, and grid connections.
2. **Dispatch efficiency.** Marginal pricing ensures the cheapest generators always
   run first. If each generator were paid its own bid, gaming would be rampant.

> **Interactive panel:** The reader controls four sliders:
> - ☀️ **Solar influx** (0-100%)
> - 💨 **Wind strength** (0-100%)
> - 🌡️ **Temperature** (affects demand via heating/cooling)
> - 📈 **Base demand** (industrial activity)
>
> As sliders move, the merit order bar chart adjusts in real time. The demand line
> shifts. Generators light up or go dark. The spot price updates. The reader
> *feels* how a sunny windy day pushes gas off the stack and crashes the price,
> while a cold windless evening forces expensive peakers online and the price
> spikes.

---

## Page 3: Grid Pressures — When Physics Gets in the Way

**Visual:** A map of northern Europe with animated power flows. Arrows pulse
between regions. Bottlenecks glow red.

### Scenario 1: The wind dies down

It's Tuesday afternoon in the North Sea. Wind forecasts predicted 15 GW of
offshore wind, but actual output drops to 8 GW over two hours.

**What happens:**
1. Frequency starts to dip across the synchronous grid
2. Automatic reserves kick in (batteries, hydro) within seconds
3. Gas turbines receive dispatch orders -- they can ramp up in 10-30 minutes
4. In Norway, dam operators open turbines to export hydropower south
5. Prices on the intraday market spike as traders scramble for replacement power

> **Animation:** The wind turbines on the map slow down. Red "deficit" zones
> appear. Then gas plants light up, Norwegian hydro arrows grow thicker, and
> balance is restored -- but the price ticker in the corner jumps.

### Scenario 2: The cold snap

A polar vortex pushes into central Europe. Germany's electricity demand jumps
15% as heat pumps and old resistive heaters work overtime. France, normally a net
exporter, is also cold -- its electric heating load is enormous.

**What happens:**
1. Germany needs imports, but France has none to spare
2. Nordic hydro is drawn south harder, but transmission lines hit capacity
3. Pricing areas that normally track each other **decouple** -- Nordic prices
   stay moderate while German prices surge
4. Coal and gas plants that were on standby come online at high cost
5. Consumer spot prices that evening can hit €200-400/MWh (vs. a normal €50-80)

> **Animation:** Temperature drops on the map. Demand bubbles swell in Germany
> and France. Transmission lines between pricing zones pulse and then some turn
> red (congested). Price tags appear above each zone, diverging sharply.

### Scenario 3: The Nordic dry spell

A warm, dry summer means Scandinavian reservoirs are below normal. Norway and
Sweden have less hydro to export. This is felt across the continent.

**What happens:**
1. Nordic hydro producers conserve water -- they bid higher in auctions
2. Less cheap hydro flows south to Germany and the Netherlands
3. These countries must run more gas, pushing up the merit order price
4. The effect persists for *weeks or months*, not hours

> **Animation:** Reservoir levels visually drop. The hydro bars in the merit order
> shrink. The gas bars grow. The price baseline drifts upward over a calendar
> that advances week by week.

### Scenario 4: The French nuclear surprise

One of France's 56 nuclear reactors discovers a stress corrosion issue. It's
taken offline for inspection. 1.3 GW of baseload generation vanishes.

**What happens:**
1. France goes from comfortable to tight within a day
2. French prices jump immediately -- forward contracts for the next months adjust
3. Neighboring countries feel the pull: Germany, Belgium, Spain export more to
   France
4. Their own prices rise in sympathy
5. The forward price curve for the entire winter shifts upward

This actually happened at scale in 2022 when France took ~30 reactors offline
for corrosion checks. French wholesale prices hit €1,000/MWh on some days, and
the ripple was felt continent-wide.

> **Animation:** A nuclear plant icon blinks red and goes dark. An immediate
> ripple of price increases spreads outward across the map like a shockwave,
> strongest nearby and attenuating with distance (but not disappearing).

---

## Page 4: Enter the Traders

**Visual:** The same map, but now with a new layer: trading desks overlaid as
nodes, with buy/sell arrows flowing between them and the exchanges.

### What energy traders actually do

Energy trading companies sit between producers and consumers. They operate on
the exchanges (spot and futures) and in bilateral (over-the-counter) markets.
Their activities fall into a few categories:

**1. Hedging and portfolio management**

Utilities that serve consumers need stable costs. A gas plant operator needs
to lock in fuel prices. Trading desks assemble portfolios of contracts that
reduce risk for these physical players. This is genuinely useful -- it's
insurance.

**2. Spatial arbitrage**

If electricity is cheap in Norway (lots of hydro) and expensive in Germany
(cold, no wind), a trader buys Norwegian power and sells it into Germany.
This *does* move power to where it's needed. It also narrows the price gap
between regions.

**But:** the trader captures the remaining spread as profit. The question is
whether the spread would be even wider without them (probably yes) or whether
their activity sometimes amplifies dislocations (occasionally also yes).

**3. Temporal arbitrage**

Buying power when it's cheap (summer, windy night) and selling when it's
expensive (winter peak). With physical assets like batteries or pumped hydro,
this directly helps grid balance. With purely financial contracts, it provides
liquidity and price discovery -- but the value to consumers is more indirect.

**4. Speculation**

Taking directional bets on where prices will go. If a trader expects a cold
winter, they buy winter futures. If winter is indeed cold, they profit. This
adds liquidity to the market and helps price discovery, but it can also
amplify price moves.

### Where does the money come from?

This is the key consumer question. Trading profits come from:

- **Bid-ask spreads:** A small cut on each transaction. This is the "cost of
  liquidity" -- like a market maker in stocks.
- **Information advantages:** Traders invest heavily in weather models, grid
  flow analysis, and satellite data. When they predict a wind shortfall before
  the market does, they profit. This money comes from less-informed market
  participants who are buying or selling at stale prices.
- **Speed:** Algorithmic trading captures small mispricings faster than others.
- **Risk premiums:** When a utility hedges its winter exposure, it pays a premium
  for certainty. The trader earns that premium by bearing the risk.

**The honest answer:** Some of this benefits consumers (hedging, liquidity,
moving power to where it's needed). Some of it is a wealth transfer from
consumers to traders (information/speed advantages, speculation on scarcity).
The proportion is genuinely debated by economists and regulators.

> **Visual concept:** A Sankey diagram showing money flows. Consumer bills
> on the left, broken into: generation cost, grid fees, taxes, retail margin,
> and -- highlighted -- "market costs" which includes the trading layer.
> The trader's cut is real but relatively small per MWh compared to the
> generation cost. The question is whether their activity raises or lowers
> the total.

---

## Page 5: The Green Transition — New Pressures

**Visual:** The merit order from Page 2, but over time. An animation shows years
passing (2020 → 2025 → 2030 → 2035). Renewable bars grow, nuclear and coal
bars shrink, but the shape of the stack changes in unexpected ways.

### The duck curve and the baseload problem

As solar capacity grows, midday prices collapse (sometimes to zero or negative).
But evening demand remains high. The daily price shape looks like a duck:
low belly during the day, high neck in the evening.

**What this means:**
- Solar producers earn less and less per MWh (their own success depresses their
  revenue -- the "cannibalization" effect)
- Gas peakers become more important, not less, because someone needs to fill the
  evening gap
- The merit order "flattens" during sunny hours but "steepens" at sunset

> **Animation:** A 24-hour price chart. As a "solar capacity" slider increases,
> the midday price drops toward zero while the evening peak stays high or grows.
> The duck shape emerges. Gas plant icons flash on at 6 PM and off at 10 AM.

### Storage changes everything (slowly)

Batteries, pumped hydro, and eventually hydrogen can shift cheap midday solar
to expensive evening hours. But we don't have nearly enough yet.

Current scale vs. what's needed:

| Storage type | Current EU capacity | Needed by 2035 (est.) |
|-------------|--------------------|-----------------------|
| Pumped hydro | ~50 GW | ~60 GW (limited sites) |
| Grid batteries | ~5 GW | ~80-120 GW |
| Hydrogen (power-to-gas) | ~0.1 GW | ~30-50 GW |

Until storage scales up, the grid needs flexible gas capacity as backup --
and that gas capacity needs to earn enough during peak hours to justify
existing. This is the "missing money" problem: a plant that only runs 500
hours a year still needs to cover its fixed costs.

> **Interactive idea:** Same merit order panel from Page 2, but now with a
> "battery capacity" slider. As the reader adds storage, it absorbs midday
> surplus and discharges in the evening. The duck curve flattens. The gas
> peaker bar shrinks. But the reader sees that enormous battery capacity is
> needed to fully eliminate the gas.

### Balancing the grid becomes harder

With large shares of variable renewables, the grid operator's job intensifies:

- **Forecast errors** matter more (clouds, wind lulls)
- **Ramp rates** are steeper (solar drops off fast at sunset)
- **Inertia** decreases (spinning turbines naturally resist frequency changes;
  solar panels don't)
- **Grid congestion** increases (wind is in the north, demand in the south)

This is where trading *could* help: liquid intraday markets allow rapid
rebalancing when forecasts shift. But it's also where speculation can hurt:
if traders amplify price signals beyond what the physical situation warrants,
it adds cost without adding electrons.

### The question this leaves you with

By now you've seen the full picture: a grid that must balance every second
(Page 1), a pricing mechanism where the most expensive generator sets the price
(Page 2), physical pressures that can cascade across a continent (Page 3), and
traders who sit in the middle of all of it (Page 4).

The green transition makes all of this *more* volatile, not less -- more
variable generation, steeper ramps, bigger forecast errors, more congestion.
That means more opportunities for trading to be useful (moving power, smoothing
risk) *and* more opportunities for it to extract value (exploiting information
gaps, amplifying price signals).

Whether the net effect benefits you as a consumer depends on how well markets
are regulated, how fast storage scales, and how much grid infrastructure gets
built. The generation mix and the wires matter far more than the financial
layer on top. But the financial layer isn't nothing -- and now you know enough
to judge for yourself.

---

## Production Notes

### Recommended format
An interactive web-based storybook. Each page is a full viewport panel.
Scroll-driven animations trigger as the reader progresses. Interactive panels
use simple sliders and see immediate results. Works on mobile (simpler
animations) and desktop (full interactive panels).

### Tech considerations
- Framework: Could be built with Svelte/SvelteKit, Next.js, or even a
  static site with D3.js animations
- Data: Real wholesale price data is publicly available from ENTSO-E
  Transparency Platform and national exchanges (Nord Pool, EPEX SPOT)
- Maps: European grid topology data from ENTSO-E
- The merit order interactive panel is the centerpiece -- invest most
  development time here

### Tone
Informative but not preachy. The goal is to give readers enough understanding
to form their own opinion. Acknowledge that the answer to "does trading help?"
is genuinely complex and that reasonable people disagree.

### Audience
Consumers and interested non-specialists. No assumed knowledge of electricity
markets. The physical grid explanations (Pages 1, 3) build intuition before
introducing financial complexity (Pages 4-5).
