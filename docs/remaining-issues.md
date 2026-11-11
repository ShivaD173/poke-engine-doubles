# Remaining Champions issues

Target format: **[Gen 9 Champions] VGC 2026 Reg M-C**
(`gen9championsvgc2026regmc`, mod `champions`).

Reference: [Pokémon Showdown at 965a535](https://github.com/smogon/pokemon-showdown/tree/965a535ef892d13cd68cf75c076db26f9f32842a).
Engine reviewed before fixes: `70a9b0384613bb90c76a8b746553e0753ae8b52d`.
The reference repository was not modified.

This document records the issues left after the selected spread-move fixes.
Numbers preserve the original review IDs. **Reproduced** means an isolated
scenario demonstrated the behavior; **source finding** means inspection found
the mismatch without a complete differential regression. Availability refers
to the resolved Champions data, not a full species/learnset legality check.

## Remaining mechanics issues

### 6 — Telepathy exclusions ignore suppression and status moves

**Reproduced for suppression; source finding for status targeting.**

`generate_instructions_from_move` excludes a Telepathy ally from `AllOther`
targets by its stored ability alone. Neutralizing Gas can therefore leave an
ally incorrectly immune. The exclusion also applies to status moves, although
Showdown's Telepathy only blocks damaging attacks. Teeter Dance is available
in Champions and should affect a Telepathy ally.

The spread damage penalty/target-counting problem from ID 5 is fixed; this is
the separate remaining question of whether the ally is hit.

Source: [target enumeration](../src/genx/generate_instructions.rs).

### 10 — KO boosts strengthen later hits of the same spread move

**Reproduced.**

The engine applies Moxie and related offensive boosts after damaging one
target, before calculating damage against subsequent targets. In Showdown,
the spread hit's damage calculations precede those KO boosts. An Earthquake
that KOs the first processed foe can incorrectly hit the second foe and ally
harder. Fixing Friend Guard in ID 11 did not redesign other mid-hit effects.

Sources: [ability_after_damage_hit](../src/genx/abilities.rs),
[spread execution](../src/genx/generate_instructions.rs).

### 12 — Attacker fainting can interrupt a spread hit

**Reproduced.**

Explosion can damage only the first processed target because the user faints
before the engine processes the others. A low-HP Breaking Swipe user fainting
to Rocky Helmet can produce the same early stop. Showdown processes the
spread hit so the other targets still take their damage.

Source: [spread execution and attacker-HP checks](../src/genx/generate_instructions.rs).

### 13 — Damp is not treated as field-wide prevention

**Reproduced with an allied Damp holder.**

An allied Damp Pokémon should prevent Explosion entirely. The relevant engine
path checks the current defender instead, allowing the user to faint and the
first target to take damage. Audit the full field, suppression, and applicable
ability bypasses when implementing a fix.

Sources: [defensive ability modifiers](../src/genx/abilities.rs),
[self-destruction effects](../src/genx/choice_effects.rs).

### 14 — Substitute / Infiltrator damage routing is inverted

**Reproduced.**

The Substitute condition requires `attacker_has_infiltrator` where ordinary
attacks should damage the Substitute. As a result, ordinary Dazzling Gleam
bypasses it while an Infiltrator user's attack damages it. Sound moves have a
separate bypass condition that should be preserved.

Source: [generate_instructions_from_damage](../src/genx/generate_instructions.rs).

### 15 — Blizzard and Air Cutter are missing relevant flags

**Wind Rider interaction reproduced; slicing is a source finding.**

Both moves lack their wind flag, so Wind Rider fails to grant immunity and
its Attack boost. Air Cutter also lacks its slicing flag, affecting Sharpness.
Champions inherits these flags from Showdown's base move data.

Source: [move definitions](../src/choices.rs).

### 16 — Expanding Force does not require a grounded user

**Reproduced with an airborne user.**

Psychic Terrain causes the engine to boost the move and change its targeting
to spread even if the user is airborne. Showdown requires the user to be
grounded for both changes. The original Tera-based example is not the target
format's mechanic; the grounding condition itself is still required.

Sources: [choice modifiers](../src/genx/choice_effects.rs),
[modify_choice_before_move](../src/genx/generate_instructions.rs).

### 17 — Magic Bounce does not reconstruct reflected spread status moves

**Original Growl scenario reproduced; Champions applicability checked in source.**

The engine changes the individual target's effects to affect the attacker,
rather than executing the reflected spread move. The original user's partner
is not affected, and target ordering can also make the reflection miss the
original user. Growl is unavailable in Champions, but String Shot and Cotton
Spore are available reflectable spread status examples. These alternative
moves have not each received a complete differential test.

Source: [Magic Bounce handling](../src/genx/abilities.rs).

### 18 — PP consumption and Pressure are incomplete

**Source finding.**

The execution-path PP decrement code is commented out. Search therefore does
not correctly model move depletion. Re-enabling the existing block directly
would risk consuming PP repeatedly per spread target. A fix needs a once-per-
move base cost plus correct additional Pressure costs for applicable targets.

Source: [run_move](../src/genx/generate_instructions.rs).

### 20 — Accuracy/evasion stages are incompletely represented

**Engine evasion behavior reproduced; client serialization is a source finding.**

The hit-probability calculation uses the attacker's accuracy boost but omits
the target's evasion boost. Separately, the Python client's state adapter
writes zero for both accuracy and evasion stages. Fixing only the Rust
calculation would not recover stages discarded by the client.

Sources: [check_move_hit_or_miss](../src/genx/generate_instructions.rs),
[client adapter](https://github.com/ShivaD173/foul-play-doubles/blob/main/fp/search/poke_engine_helpers.py).

### 21 — Friendly-fire critical/KO branching can inspect the wrong Pokémon

**Source finding.**

For an ally hit by Earthquake or Surf, the damage calculation uses the actual
ally, but `run_move`'s later critical-rate and KO-branch logic obtains the
opposing side's corresponding slot via `get_both_sides(attacking_side)`.
It can therefore use the wrong HP or critical-hit immunity when constructing
chance branches. Regression tests must enable damage branching to expose it.

Source: [run_move damage branching](../src/genx/generate_instructions.rs).

### 22 — Shell Bell heals per target instead of after the move

**Source finding.**

Shell Bell is encoded as a drain fraction and applied during each target's
processing. Showdown heals from the move's accumulated damage afterward.
Both timing and rounding can differ. Shell Bell is available in Champions.

Sources: [item modifiers](../src/genx/items.rs),
[damage/drain execution](../src/genx/generate_instructions.rs).

## Unchanged search approximations

These are deliberate simplifications to evaluate separately from mechanics bugs.

- **23 — Damage rounding:** modifiers, including the spread multiplier, are
  multiplied with floating-point arithmetic and truncated at different points
  from Showdown's ordered intermediate rounding. Exact KO thresholds can differ.
  Source: [damage calculation](../src/genx/damage_calc.rs).
- **24 — Chance distributions:** search uses average damage in many situations,
  selectively branches on damage/criticals, and prunes/renormalizes outcomes.
  For example, a 95%-accurate two-target Make It Rain can lose its 0.25%
  double-miss outcome to pruning. Correcting its self-drop did not change that
  optimization. Source: [branch generation and pruning](../src/genx/generate_instructions.rs).

## Lower-priority incomplete mechanics previously discussed

These were explicitly deprioritized and remain unfinished in the source:

- **Wish and Future Sight:** doubles end-of-turn/source/target handling contains
  commented-out implementation and TODOs.
- **Leech Seed:** source-aware doubles residual handling is commented out.

These are source findings rather than newly validated Champions scenarios.
No priority or availability claim is implied for them.

## Findings no longer open

- **1:** Withdrawn. Champions Make It Rain is correctly 95% accurate with a
  two-stage Special Attack drop. Astral Barrage's 110 base power also matches
  Champions, where it is marked unavailable.
- **2, 3, 7, 11, 19:** Selected fixes implemented. See the
  [Champions spread audit](champions-spread-audit.md) for tests and scope.
- **4, 5:** Resolved as consequences of preserving spread targeting separately
  from the damage penalty and counting intended living targets.
- **8:** Deferred spread-move recoil also avoids the reviewed Covert Cloak
  suppression problem. The item is unavailable in Champions.
- **9:** Throat Spray's early activation remains a simulator discrepancy, but
  the item is unavailable in Champions. It is deferred for this format.

## Client integration and deployment

The separate [client known-issues document](https://github.com/ShivaD173/foul-play-doubles/blob/main/docs/known-issues.md)
records the engine dependency/build settings, omitted stages, fragile startup
message handling, and best-of-three assumptions.
Pushing the engine source does not automatically replace the client's pinned
published engine. Build/install this checkout with the `mega` feature as
explained in the [installation instructions](champions-spread-audit.md#validation).

The 10-second benchmark also measured cleanup overhead and position-dependent
iteration rates. A time budget is not a strict bound on the complete call's
elapsed time, and the client applies it separately to sampled opponent states.
