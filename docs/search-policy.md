# Single-threaded MCTS exploration and depth

Target: `[Gen 9 Champions] VGC 2026 Reg M-C`, built with `mega`.
These changes apply to the single-threaded engine used by the client. The
client still samples four opponent states; process parallelism does not select
the separate shared-tree search implementation.

## Action credit

Every traversed edge records its parent, child, and the actual action index for
each side in a scratch path reused across iterations. Backpropagation updates
those actions and reverses the corresponding instructions in reverse order.
Every visited node is counted once per iteration. Action arrays retain their
generated order, keeping the recorded indices valid.

Previously selection cycled for `50 * action_count` visits, but backpropagation
assumed index zero after 50 total visits. For 20 actions at visit 51, action 11
could be selected while action zero received its reward. This mismatch is
removed; credit no longer depends on a reconstructed visit-counter index.

Both newly expanded and previously cached branches record their selected
edges. If expansion stops at a terminal state or the depth limit, no unexecuted
action is credited at that leaf; the traversed ancestor actions still receive
the leaf reward.

## Exploration

An action here is the joint pair of commands for one side's active Pokémon.
Each side independently samples uniformly from its currently unvisited actions.
After each action has one visit, selection uses UCB1 over every available action:

```text
Q(action) + sqrt(2 * ln(parent_visits) / action_visits)
```

Equal UCB scores are broken randomly. This replaces the compulsory 50 visits
per action and the synchronized round-robin schedule. For example, a side with
40 actions can begin value-based selection after 40 action visits instead of
2,000. The two sides can enter this stage at different times when their action
counts differ.

The implementation scans current UCB values instead of maintaining an
implicitly reordered action list. No heuristic priors, PUCT, progressive
widening, or new action pruning are enabled. Every action supplied by the
existing generator remains available for exploration.

## Six completed turns

Single-threaded search now stops expanding at six completed turns, matching
the existing shared-tree depth cap. Depth advances only when a transition marks
`end_of_turn_triggered`; forced replacement phases do not consume a completed
turn. At the limit the state is evaluated without generating another unused
set of actions.

Rollout remains the existing static, root-relative evaluation. There is no
additional move simulation after the reached leaf. A six-turn cap is a maximum,
not a guarantee that every iteration reaches six turns.

The separate shared-tree implementation retains its existing selection policy,
heuristics, and widening rules. Its six-turn limit is unchanged.

## Verification and practical effects

Regression tests cover:

- Actual-action credit after visit 50, unequal side action counts, and multiple
  traversed edges, including the opponent's complementary reward.
- Independent initial exploration, immediate value-based selection after one
  visit per action, UCB uncertainty bonuses, and randomized ties.
- Cached-branch reuse, exactly reaching turns five and six, stopping before
  seven, and forced replacements retaining their existing turn depth.
- Choosing an immediate guaranteed KO over an action that guarantees defeat.
- Root-state restoration, visit totals, and repeatability with a fixed seed.

Chosen policies and fixed-seed fingerprints are expected to differ from earlier
versions. Repeated searches with the same seed within this version remain
repeatable. Normal play still uses the advancing ThreadRng stream.

Search time budgets, the 25-million iteration cap, battle mechanics, damage
branching, and evaluation weights are unchanged. The larger horizon and changed
exploration can change iteration throughput and memory use; higher playing
strength has not been established by these regression tests. The earlier
[allocator benchmarks](performance.md) describe the four-turn, old-exploration
version and should not be treated as current throughput predictions.

Rebuild the installed native extension after updating the sibling engine:

```sh
# From foul-play-doubles, with its virtualenv active:
make poke_engine_local GEN=mega
```
