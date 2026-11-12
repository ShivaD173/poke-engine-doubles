# MCTS iteration throughput

Target: `[Gen 9 Champions] VGC 2026 Reg M-C`, built with `mega`.

The objective here is **more MCTS iterations within the search budget**.
Baseline commit `132430e` already contains the previous sampler and diagnostics
changes. The earlier [latency report](performance-latency-2026-11-12.md) measured
complete return times; those figures do not establish faster search iterations.

## Changes inside the search

The CLI and Python-extension entry points now use **mimalloc for Rust
allocations**, replacing the system allocator. This targets the numerous
allocations and frees performed while generating branches and growing the tree.
The Rust library itself does not declare a global allocator, so applications
embedding it retain control of their allocator. Python's own allocator is
unchanged. A `system_allocator` build feature provides an opt-out for comparison.

No production selection, expansion, rollout, or backpropagation logic changes
are included. A shared result-construction helper supports the new profiling
entry point. Other code-level trials were inconclusive and are not included.

Search depths, visit caps, sample counts, evaluation weights, damage branching,
move-order rules, and battle mechanics remain unchanged. The client continues to
use its existing process parallelism and one native search thread per sample.

## Measurements

The shared dependencies remain locked; optimized entry points add mimalloc
0.1.52. Baseline and optimized binaries use the
same synthetic fixtures in `examples/benchmark_mcts.rs`. The baseline receives
only the [profiling-helper patch](benchmarks/throughput-baseline.patch), without
any of the runtime optimizations above.

Machine: AMD EPYC 9V74, Linux x86-64, two-CPU quota, Rust 1.99.1. Both binaries
are warmed up before measurement. Benchmark processes are pinned to the same
available CPU, run serially, and alternate baseline/optimized order across
three repetitions.

Two different comparisons are kept separate:

1. **Fixed work:** 500,000 opening iterations or 150,000 chance-heavy iterations,
   using seeds 42, 777, and 2026. The timer covers only the iteration loop,
   excluding initialization, result construction, and tree destruction.
2. **Fixed budget:** normal stochastic searches with a 10,000 ms budget. The
   reported metric is their iteration count, not iterations divided by complete
   return time. The existing 1,000-iteration deadline-check batches remain.

The fixed-work mode uses `StdRng` to make each comparison repeatable. Normal
searches still use the advancing `ThreadRng` stream. Enabling the benchmark
feature alone does not change the RNG used by normal searches.

Medians from three normal **10-second search budgets**, one native thread:

| Fixture | Before: iterations | After: iterations | Increase |
| --- | ---: | ---: | ---: |
| opening | 1,100,000 | 1,359,000 | +23.5% |
| chance-heavy | 325,000 | 382,000 | +17.5% |

These percentages compare iteration counts inside the configured budget.
No post-search diagnostic or destruction time is used to calculate them.

Fixed-work kernel throughput gains, paired by seed:

| Fixture | Iterations per seed | Median throughput gain | Range across seeds |
| --- | ---: | ---: | ---: |
| opening | 500,000 | +27.0% | +14.2% to +30.8% |
| chance-heavy | 150,000 | +17.8% | +5.8% to +22.0% |

For fixed work, throughput gain is `before_search_seconds / after_search_seconds - 1`.
Every fixed-work pair produced the same root-policy fingerprint.

More iterations grow larger trees. Peak resident memory during the budgeted
searches also increased:

| Fixture | Before: peak MiB | After: peak MiB |
| --- | ---: | ---: |
| opening | 902 | 1076 |
| chance-heavy | 502 | 594 |

These peaks include the complete process and cleanup and were measured with
Linux `wait4`. They compare the trees actually built within ten seconds, not
an equal number of allocated nodes. Allocator caches can remain resident in
reused workers. [All 24 per-run measurements](benchmarks/2026-11-12-throughput.jsonl)
include the complete return time, CPU usage, and peak RSS for auditing.

The fixed-work comparisons verify an identical fingerprint of every root
policy's action, visit count, and exact floating-point score bits. Each call also
verifies that the root state is restored. This checks matching policy statistics for the controlled workloads. The
production search algorithm itself is unchanged.

These are small synthetic benchmarks, not legal teams or battle replays. The
opening fixture has two active Pokémon and two reserves per side, with 47/61
root actions. The chance-heavy fixture uses Rock Slide, Heat Wave, Blizzard,
and Discharge, with 34/34 root actions. Position, random trajectory, hardware,
and compiler version affect iteration counts. Three repetitions do not establish
a universal speedup or a playing-strength improvement.

## Reproduce

Build in this checkout:

```sh
cargo build --locked --release --no-default-features --features mega,benchmark_mcts --example benchmark_mcts
```

Create a baseline checkout and add only the same measurement helper:

```sh
git worktree add --detach ../poke-engine-before 132430e
git -C ../poke-engine-before apply --unidiff-zero "$PWD/docs/benchmarks/throughput-baseline.patch"
cargo build --manifest-path ../poke-engine-before/Cargo.toml --locked --release --no-default-features --features mega,benchmark_mcts --example benchmark_mcts
```

Fixed-work invocation, shown for this checkout:

```sh
./target/release/examples/benchmark_mcts opening 1 10000 500000 42
./target/release/examples/benchmark_mcts chance-heavy 1 10000 150000 42
```

The fourth argument selects a fixed iteration count and the fifth selects its
seed. The duration argument is ignored in that mode. Compare `search_seconds`
and verify that both binaries return the same `policy` fingerprint. Repeat with
seeds 777 and 2026, alternating binary order. Run each binary's command separately;
benchmarking them concurrently introduces CPU and memory competition.

Normal 10-second searches omit the iteration and seed arguments:

```sh
./target/release/examples/benchmark_mcts opening 1 10000
./target/release/examples/benchmark_mcts chance-heavy 1 10000
```

Compare their `iterations` fields across repeated runs. The separate `total_seconds`
field includes post-search cleanup and must not be used as the denominator for
iterations within the configured budget. Fixed-work profiling supports one
native thread; normal budgeted benchmarking also supports shared-tree searches.

## Installation and remaining opportunities

With both repositories updated and the client's virtualenv active:

```sh
# From foul-play-doubles, with the engine checked out beside it:
make poke_engine_local GEN=mega
```

The Python API and bot invocation are unchanged. The mimalloc dependency adds a
C compilation step. To compare the Python extension with the system allocator,
add `system_allocator` to its build features:
`--features poke-engine/mega,system_allocator --no-default-features`.
For the CLI/benchmark use `--features mega,benchmark_mcts,system_allocator`. `mcts_stats` still enables
optional full-tree diagnostics; `benchmark_allocations` still enables the example's
allocation counter. Neither is required for normal play.

Instruction-prefix copying, state apply/reverse work, tree destruction, and
shared-tree synchronization remain candidates for deeper profiling. Reusing
whole trees across turns would require matching observed state and sampled
hidden information. Pokémon Showdown was not modified.
