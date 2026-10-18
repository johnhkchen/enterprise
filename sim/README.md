# The Enterprise core and its fast host

`core/` is the society (towns, goods, money, wagons, the crown) with no game in it; `host/` runs it fast for studies
and sweeps. See `docs/architecture.md` for the whole plan and `docs/worktrees.md` for how it's built.

```sh
cargo test --release                              # every test, seconds to a minute
cargo run --release -p enterprise-sim -- 10000    # 10,000 days of the network study, the books checked daily
```

## Money is counted in thousandths

Every amount of money in the core is a `Fixed` (`core/src/fixed.rs`): a whole number of thousandths of an emerald.
Balances, every posting in the cash book, debts, budgets and tallies are all `Fixed`, so they add up exactly and
`check.rs` balances the books to the thousandth with no tolerance. Rates stay `f64` (a good's worth an item, shares,
trust, mood); where a rate times an amount becomes money it is rounded once, the way the mod rounds that kind of sum:

| Sum | Rounded | As the mod does |
|---|---|---|
| What a payer owes: a price for goods, food, a debt, a fee, a cost | up | `Treasury.withdraw`: `Math.ceil(rest * rate)` spurs |
| A share paid out: wages, a commission kept, a toll, a discount, a loan, aid | down | `Households.payIn`, tolls: `Math.floor` |
| A figure from a host or the outside, a valuation | to the nearest | |

and never more than the payer holds. A claim that grows by fractions every look (wages owed, guards' pay owed,
deposits' interest) is kept as `f64` and paid in whole emeralds or rounded once when it's paid.

## Replays agree on every host

`+ - * /` and `sqrt` are correctly rounded on every IEEE host, so `f64` arithmetic gives the same bits natively and in
WebAssembly (and in Java running the WebAssembly). The curves aren't: `exp`, `ln`, `powf` and `hypot` come from the
platform's libm and can differ in the last bit. The core computes them itself from `+ - * /` alone (`fixed::exp`,
`exp2`, `ln`, `pow`, `halved` for the half-lives, `powi`, `hypot`), so nothing in a run depends on the host.

`core/tests/replay.rs` plays a script of 200 days (the random worlds' generator for four seeds, every slice in it) and
digests what a host sees: each day's messages, every town and route, the mint's rate, the save at the end, and the
curves over a grid, bit for bit. The digest is committed in `core/tests/replay_digest.txt`; the test checks it.

A change to the rules moves the digest. Write the new one natively and commit it with the change:

```sh
cd sim/core && REPLAY_DIGEST=write cargo test --release --test replay
```

### The same script in WebAssembly

The same test, built for `wasm32-wasip1`, must give the same digest. Once: `rustup target add wasm32-wasip1`. Then
from `sim/`, with Node (its built-in WASI, through `wasi-run.mjs`; no dependency):

```sh
CARGO_TARGET_WASM32_WASIP1_RUNNER="node $PWD/wasi-run.mjs" \
    cargo test --release -p enterprise-core --target wasm32-wasip1 --test replay
```

or with wasmtime, if it's installed:

```sh
CARGO_TARGET_WASM32_WASIP1_RUNNER=wasmtime \
    cargo test --release -p enterprise-core --target wasm32-wasip1 --test replay
```

`just core-wasm-replay` runs the Node one. It passes when the WebAssembly build's digest is byte for byte the native
one's (about 7 s). A failure prints both digests, the curves' and each seed's on lines of their own. Only the replay
is meant for WebAssembly: the other tests run their seeds on threads, which `wasm32-wasip1` hasn't got.
