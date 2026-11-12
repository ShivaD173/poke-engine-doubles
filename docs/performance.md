# MCTS performance

Target: `[Gen 9 Champions] VGC 2026 Reg M-C`, built with `mega`.

## Changes

1. Full-tree diagnostics are opt-in through the `mcts_stats` Cargo feature.
   Previously every completed search built extra maps, walked the entire tree,
   and printed its shape **after** the search deadline. This affected both
   single-threaded and shared-tree searches.
2. Single-threaded MCTS stores a chance distribution with each generated branch.
   Previously every visit built a temporary weights vector and a new
   `WeightedIndex`. Selection now reuses the distribution, and deterministic
   branches return their only node immediately. Branch weights are immutable
   throughout the search. Non-normalized weights and zero-probability outcomes
   retain their weighted-sampling behavior.
3. The random-number generator handle is obtained once per search and passed
   through selection and expansion.

The shared-tree engine already reused its random-number generators and had an
allocation-free weighted sampler; its only change is the diagnostics flag.
MCTS depth limits, evaluation, move selection, widening, damage branching,
iteration cap, and battle mechanics are unchanged. Random streams and resulting
move choices are not expected to be identical between runs.

The client separately reuses worker processes and avoids an unnecessary battle
copy. See its [performance notes](https://github.com/ShivaD173/foul-play-doubles/blob/main/docs/performance.md).

## Measurement

Baseline: engine commit `0dd4eab`, before these changes. Both versions use the
same fixed-position benchmark, checked-in dependency lockfile, release profile,
and `mega` feature. Machine: AMD EPYC 9V74, Linux x86-64, **two-CPU quota**, 8 GiB
memory limit; Rust 1.99.1. Run the binaries directly after building so compile
time is excluded. Baseline and optimized versions run serially in alternating
order, three repetitions per position and thread count, with a 10,000 ms budget.

The `opening` fixture has two active Pokémon and two living reserves per side,
with 47 versus 61 joint actions at the root. `chance-heavy` replaces attacks
with Rock Slide, Heat Wave, Blizzard, and Discharge, producing 34 versus 34
joint actions. These are **synthetic workloads**, including combinations of
species and moves that need not be legal. They test search costs, not team
strength or replay fidelity. The `endgame` fixture is also available but is not
part of the comparison table.

Elapsed time measures the complete native call, including tree destruction.
Thus a 10-second budget can take longer than 10 seconds to return. Single-thread
search also checks its deadline in batches of 1,000 iterations. Throughput below
is iterations divided by complete elapsed time. One native thread uses the
existing depth-four search; two threads use the existing depth-six shared-tree
search. Compare each row against its own baseline.

Medians from the three runs (old → optimized):

Each metric is summarized independently; throughput gains compare the median
per-run rates.

| Position | Native threads | Iterations per budget | Return time (s) | Complete-call throughput gain |
| --- | ---: | ---: | ---: | ---: |
| opening | 1 | 1,263,000 → 1,234,000 | 11.249 → 10.681 | +2.5% |
| opening | 2 | 2,028,000 → 2,140,000 | 12.708 → 11.307 | +17.9% |
| chance-heavy | 1 | 304,000 → 337,000 | 10.567 → 10.392 | +12.7% |
| chance-heavy | 2 | 1,047,000 → 1,014,000 | 12.140 → 11.029 | +8.0% |

The repeatable gain is lower return latency. In-budget iteration counts are
noisy: the opening single-thread count was slightly lower, while the chance-heavy
single-thread count was higher. Shared-tree sampling/selection code was unchanged;
its iteration-count differences reflect run variation, not a new search algorithm.
These are workload-specific measurements, not a promised speedup for every turn.

| Position | Native threads | Peak resident memory (MiB), old → optimized |
| --- | ---: | ---: |
| opening | 1 | 952 → 995 |
| opening | 2 | 1764 → 1684 |
| chance-heavy | 1 | 461 → 520 |
| chance-heavy | 2 | 1538 → 1324 |

Peak memory was measured per child process using `wait4`, including diagnostics
and tree cleanup. [All 24 raw runs](benchmarks/2026-11-12-mcts.jsonl) retain the
iteration counts, elapsed times, throughput, and peak RSS.

An instrumented two-second opening run counted **54.114 → 51.366 allocation/
reallocation calls per iteration** (about 5.1% fewer). This includes the removed
diagnostics and varies with the explored tree. The instrumented wall times are
excluded from the table.

Caching distributions retains their cumulative weights until the tree is
destroyed. This trades additional storage per chance branch for fewer temporary
allocations. Faster searches can also build larger trees within the same time
budget. Worker reuse in the client keeps idle processes and allocator caches
resident. Neither change should be treated as a memory optimization.

The tests cover weighted sampling, real single-threaded and shared-tree MCTS,
root-state restoration, and the existing mechanics regressions. The optional
diagnostics build is tested as well. Pokémon Showdown was not modified.

## Repeat locally

```sh
cargo build --locked --release --no-default-features --features mega --example benchmark_mcts
./target/release/examples/benchmark_mcts opening 1 10000
./target/release/examples/benchmark_mcts opening 2 10000
./target/release/examples/benchmark_mcts chance-heavy 1 10000
./target/release/examples/benchmark_mcts chance-heavy 2 10000
```

Arguments are position, native thread count, and budget in milliseconds. The
example also verifies that the serialized root state is unchanged by the search.
To compare an earlier commit, use a separate checkout and copy this example into
its `examples` directory before building with the same lockfile and profile.

Allocation counting is optional and adds instrumentation overhead:

```sh
cargo run --locked --release --no-default-features --features mega,benchmark_allocations --example benchmark_mcts -- opening 1 2000
```

The counter includes allocator and reallocator calls around the search and root
verification. Use it to compare counts, not to measure normal runtime. To enable
it in the old checkout, add the empty `benchmark_allocations = []` feature to
that checkout's manifest; the engine code itself needs no changes.

For full tree-shape diagnostics, build with `--features mega,mcts_stats`.
For the Python extension, use
`--features poke-engine/mega,poke-engine/mcts_stats --no-default-features`.
Normal Champions installations should use the existing `mega` build command.

## Further candidates

- Instruction generation copies instruction prefixes when creating probability
  branches. Buffer reuse, arenas, or shared prefixes may reduce the remaining
  allocation cost, but require checking state apply/reverse behavior carefully.
- Tree destruction still happens after the search deadline. Arena allocation
  could reduce this cost; simply deferring destruction to another thread can
  increase peak memory and compete with the next search.
- Shared-tree searches use a concurrent child map and atomic visit counters.
  Profiling on a machine with more available cores is needed before changing
  their synchronization.

These are source-review candidates, not measured or implemented speedups.
