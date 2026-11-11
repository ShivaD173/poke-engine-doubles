# Champions spread-move review

Target: **[Gen 9 Champions] VGC 2026 Reg M-C**, format ID
`gen9championsvgc2026regmc`, mod `champions`.

Read-only reference: smogon/pokemon-showdown commit
`965a535ef892d13cd68cf75c076db26f9f32842a`. Engine baseline:
`70a9b0384613bb90c76a8b746553e0753ae8b52d`.

The original review mistakenly used `gen9predlc`. Champions explicitly changes
Make It Rain to **95% accuracy and a two-stage Special Attack drop**. Those
engine data values are correct and are unchanged. Astral Barrage's 110 base
power also matches Champions (the move is marked unavailable in this mod).

## Selected fixes

The numbers below retain the IDs from the original review.

| ID | Correction |
| --- | --- |
| 2 | Track successful damaging hits independently for each probability branch. Apply the move's self-drop once after all targets, including when an earlier target hits but the last target misses, Protects, or is immune. |
| 3 | Preserve spread targeting for Wide Guard and redirection. Track the spread damage penalty separately, so a move with only one remaining target still has its correct full damage. |
| 7 | Apply spread-move Life Orb recoil once after the move if any target was hit, independently of the last target's Protect, miss, or immunity. |
| 11 | Snapshot each target's applicable Friend Guard support at the start of the spread hit. A KO during that hit does not remove the protection for later damage calculations. A holder fainted before the move provides no protection. |
| 19 | Advance Encore and Taunt's existing engine counters on the first target only, for both two- and three-target moves. This fixes duplicate advancement; it is not a redesign of the engine's broader duration model. |

Preserving targeting and counting intended living targets also resolves the
review's one-target redirection issue (4) and Telepathy spread-penalty issue (5).
Deferred recoil avoids the spread-move Covert Cloak interaction (8), although
Covert Cloak is marked unavailable in Champions. These are consequences of the
selected changes, not separate rewrites of those mechanics.

## Recheck of remaining findings

The shared spread pipeline in Champions inherits Showdown's Gen 9 implementation.
The new mod does not make the following simulator differences correct:

| ID | Status after these fixes |
| --- | --- |
| 1 | Withdrawn: Make It Rain's accuracy and self-drop match Champions. |
| 6 | Still present: Telepathy target exclusion ignores suppression and also excludes spread status moves, which Telepathy should not block. Teeter Dance is available in Champions. |
| 9 | Throat Spray still activates too early in simulation, but the item is marked `Past` in Champions. |
| 10 | Still present: KO-triggered offensive boosts can change damage against later targets in the same spread hit. |
| 12 | Still present: attacker fainting can stop remaining targets from being hit (Explosion or contact retaliation). |
| 13 | Still present: Damp is not treated as a field-wide prevention in this path. |
| 14 | Still present: Substitute/Infiltrator's damage-routing condition is inverted. |
| 15 | Still present: Blizzard and Air Cutter lack wind flags; Air Cutter also lacks slicing. Champions inherits these flags from base Showdown data. |
| 16 | Still present: Expanding Force's spread/power changes fail to require a grounded user. |
| 17 | Still present for available reflectable spread status moves: Magic Bounce doesn't reproduce the reflected spread attack. Growl, the original example, is unavailable; String Shot and Cotton Spore are available examples. |
| 18 | Still present: move execution does not consume PP correctly. |
| 20 | Still present: target evasion is omitted; the Python adapter also discards accuracy/evasion boosts. |
| 21 | Still present: friendly-fire critical/KO branching can consult the corresponding opposing slot. |
| 22 | Still present: Shell Bell uses per-target drain instead of post-move aggregate healing. Shell Bell is available in Champions. |
| 23 | Unchanged approximation: damage modifiers do not reproduce all of Showdown's intermediate rounding. |
| 24 | Unchanged search approximation: average damage, selective damage branching, outcome pruning and renormalization. |

Availability here means the resolved Champions data does not mark the entry
`Past`; it is not a complete team-validator/learnset audit. The remaining rows
were checked against the resolved Champions definitions and inherited pipeline;
not every combination has a complete differential regression test. In
particular, source findings 18 and 20–24 retain that distinction from reproduced
battle scenarios.

## Validation

Run the engine tests with Mega Evolution support for Champions:

```sh
cargo test --locked --no-default-features --features mega --no-fail-fast
```

`tests/test_champions_spread.rs` covers the selected rules, chance branches,
state reversal, and both single-threaded and shared-tree MCTS. The existing
Make It Rain regression now expects the Champions -2 drop on each successful
branch, including the previously missing earlier-target-only hit branch.

A separate reference check can run against a compiled, unmodified Showdown
checkout containing the Champions mod:

```sh
node tests/champions_reference.cjs /absolute/path/to/showdown/dist/sim
```

It checks the resolved format/mod and move definition, partial failures,
Wide Guard with one target, Life Orb, Friend Guard, and residual duration ticks.
The scenarios override stats, hit outcomes and abilities to isolate mechanics;
they are not tests of legal team construction or exact damage-roll parity.

To use the patched engine from the sibling client checkout rather than the
published 0.0.7 engine, activate the client's virtual environment and run:

```sh
pip install --force-reinstall --no-cache-dir ../poke-engine-doubles/poke-engine-py \
  --config-settings="build-args=--features poke-engine/mega --no-default-features"
```

The client's existing requirements still select the published engine with Tera
support. Reinstalling that requirements file afterward replaces this local
build; pin the desired engine commit when packaging/deploying these changes.
