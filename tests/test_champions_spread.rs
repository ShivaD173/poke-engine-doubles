//! Regression cases checked against Showdown's `champions` mod.
use poke_engine::choices::{Choices, MOVES};
use poke_engine::engine::abilities::Abilities;
use poke_engine::engine::generate_instructions::generate_instructions_from_move_pair;
use poke_engine::engine::items::Items;
use poke_engine::engine::state::{MoveChoice, PokemonVolatileStatus};
use poke_engine::instruction::{Instruction, StateInstructions};
use poke_engine::state::{
    LastUsedMove, PokemonMoveIndex as M, SideReference as S, SlotReference as L, State,
};

fn attack(state: &mut State, mv: Choices) -> Vec<StateInstructions> {
    state.sides[0].pokemon.pkmn[0].replace_move(M::M0, mv);
    let before = state.serialize();
    let results = generate_instructions_from_move_pair(
        state,
        &MoveChoice::Move(L::SlotA, S::SideTwo, M::M0),
        &MoveChoice::None,
        &MoveChoice::None,
        &MoveChoice::None,
        false,
    );
    assert_eq!(
        before,
        state.serialize(),
        "Search must restore the input state"
    );
    assert!((results.iter().map(|r| r.percentage).sum::<f32>() - 100.0).abs() < 0.001);
    results
}

fn target_was_damaged(r: &StateInstructions) -> bool {
    r.instruction_list.iter().any(|i| {
        matches!(i,
        Instruction::Damage(d) if d.side_ref == S::SideTwo && d.damage_amount > 0)
    })
}

#[test]
fn champions_make_it_rain_keeps_its_nerfed_definition() {
    let choice = MOVES.get(&Choices::MAKEITRAIN).unwrap();
    assert_eq!(choice.accuracy, 95.0);
    assert_eq!(choice.boost.as_ref().unwrap().boosts.special_attack, -2);
}

#[test]
fn make_it_rain_drops_once_on_every_branch_that_hits_either_target() {
    let mut state = State::default();
    for result in attack(&mut state, Choices::MAKEITRAIN) {
        state.apply_instructions(&result.instruction_list);
        assert_eq!(
            state.sides[0].slot_a.special_attack_boost,
            if target_was_damaged(&result) { -2 } else { 0 }
        );
        state.reverse_instructions(&result.instruction_list);
    }
}

#[test]
fn make_it_rain_drop_survives_either_targets_protect() {
    for protected in [L::SlotA, L::SlotB] {
        let mut state = State::default();
        state.sides[1]
            .get_slot(&protected)
            .volatile_statuses
            .insert(PokemonVolatileStatus::PROTECT);
        for result in attack(&mut state, Choices::MAKEITRAIN) {
            state.apply_instructions(&result.instruction_list);
            assert_eq!(
                state.sides[0].slot_a.special_attack_boost,
                if target_was_damaged(&result) { -2 } else { 0 }
            );
            state.reverse_instructions(&result.instruction_list);
        }
    }
}

#[test]
fn make_it_rain_does_not_drop_when_both_targets_protect() {
    let mut state = State::default();
    for slot in [L::SlotA, L::SlotB] {
        state.sides[1]
            .get_slot(&slot)
            .volatile_statuses
            .insert(PokemonVolatileStatus::PROTECT);
    }
    for result in attack(&mut state, Choices::MAKEITRAIN) {
        state.apply_instructions(&result.instruction_list);
        assert_eq!(state.sides[0].slot_a.special_attack_boost, 0);
        state.reverse_instructions(&result.instruction_list);
    }
}

#[test]
fn wide_guard_blocks_spread_moves_with_only_one_target_left() {
    for mv in [Choices::DAZZLINGGLEAM, Choices::EARTHQUAKE] {
        for survivor in [0, 1] {
            let mut state = State::default();
            state.sides[0].pokemon.pkmn[1].hp = 0;
            state.sides[1].pokemon.pkmn[1 - survivor].hp = 0;
            state.sides[1].side_conditions.wide_guard = 1;
            for result in attack(&mut state, mv) {
                assert!(!target_was_damaged(&result));
            }
        }
    }
}

#[test]
fn one_target_spread_move_keeps_full_damage_without_wide_guard() {
    let mut state = State::default();
    state.sides[1].pokemon.pkmn[1].hp = 0;
    let result = &attack(&mut state, Choices::DAZZLINGGLEAM)[0];
    state.apply_instructions(&result.instruction_list);
    // Existing damage policy: floor(max roll) * .925, without a .75 spread penalty.
    assert_eq!(state.sides[1].pokemon.pkmn[0].hp, 66);
}

#[test]
fn life_orb_recoils_once_if_either_target_is_hit() {
    for protected in [None, Some(L::SlotA), Some(L::SlotB)] {
        let mut state = State::default();
        state.sides[0].pokemon.pkmn[0].item = Items::LIFEORB;
        if let Some(slot) = protected {
            state.sides[1]
                .get_slot(&slot)
                .volatile_statuses
                .insert(PokemonVolatileStatus::PROTECT);
        }
        for result in attack(&mut state, Choices::HEATWAVE) {
            let recoil: Vec<_> = result
                .instruction_list
                .iter()
                .filter(|i| {
                    matches!(i,
                Instruction::Heal(h) if h.side_ref == S::SideOne && h.heal_amount < 0)
                })
                .collect();
            assert_eq!(recoil.len(), usize::from(target_was_damaged(&result)));
            state.apply_instructions(&result.instruction_list);
            assert_eq!(
                state.sides[0].pokemon.pkmn[0].hp,
                if target_was_damaged(&result) { 90 } else { 100 }
            );
            state.reverse_instructions(&result.instruction_list);
        }
    }
}

#[test]
fn life_orb_does_not_recoil_into_double_protect_or_magic_guard() {
    for magic_guard in [false, true] {
        let mut state = State::default();
        state.sides[0].pokemon.pkmn[0].item = Items::LIFEORB;
        if magic_guard {
            state.sides[0].pokemon.pkmn[0].ability = Abilities::MAGICGUARD;
        } else {
            for slot in [L::SlotA, L::SlotB] {
                state.sides[1]
                    .get_slot(&slot)
                    .volatile_statuses
                    .insert(PokemonVolatileStatus::PROTECT);
            }
        }
        for result in attack(&mut state, Choices::DAZZLINGGLEAM) {
            state.apply_instructions(&result.instruction_list);
            assert_eq!(state.sides[0].pokemon.pkmn[0].hp, 100);
            state.reverse_instructions(&result.instruction_list);
        }
    }
}

#[test]
fn friend_guard_survives_partners_ko_for_the_entire_spread_hit() {
    let mut protected_damage = None;
    for partner_hp in [1, 100] {
        let mut state = State::default();
        state.sides[1].pokemon.pkmn[0].ability = Abilities::FRIENDGUARD;
        state.sides[1].pokemon.pkmn[0].hp = partner_hp;
        let result = &attack(&mut state, Choices::DAZZLINGGLEAM)[0];
        state.apply_instructions(&result.instruction_list);
        let remaining_hp = state.sides[1].pokemon.pkmn[1].hp;
        if let Some(expected) = protected_damage {
            assert_eq!(remaining_hp, expected);
        } else {
            protected_damage = Some(remaining_hp);
        }
    }
    let mut state = State::default();
    let result = &attack(&mut state, Choices::DAZZLINGGLEAM)[0];
    state.apply_instructions(&result.instruction_list);
    assert!(protected_damage.unwrap() > state.sides[1].pokemon.pkmn[1].hp);
}

#[test]
fn friend_guard_is_not_kept_after_its_holder_was_already_fainted() {
    let mut state = State::default();
    state.sides[1].pokemon.pkmn[0].ability = Abilities::FRIENDGUARD;
    state.sides[1].pokemon.pkmn[0].hp = 0;
    let result = &attack(&mut state, Choices::DAZZLINGGLEAM)[0];
    state.apply_instructions(&result.instruction_list);
    assert_eq!(state.sides[1].pokemon.pkmn[1].hp, 66);
}

#[test]
fn taunt_and_encore_advance_once_for_two_and_three_target_moves() {
    for mv in [Choices::DAZZLINGGLEAM, Choices::EARTHQUAKE] {
        for volatile in [PokemonVolatileStatus::TAUNT, PokemonVolatileStatus::ENCORE] {
            for initial in 0..3 {
                let mut state = State::default();
                let slot = &mut state.sides[0].slot_a;
                slot.volatile_statuses.insert(volatile);
                slot.last_used_move = LastUsedMove::Move(M::M0);
                match volatile {
                    PokemonVolatileStatus::TAUNT => slot.volatile_status_durations.taunt = initial,
                    _ => slot.volatile_status_durations.encore = initial,
                }
                for result in attack(&mut state, mv) {
                    state.apply_instructions(&result.instruction_list);
                    let slot = &state.sides[0].slot_a;
                    let duration = match volatile {
                        PokemonVolatileStatus::TAUNT => slot.volatile_status_durations.taunt,
                        _ => slot.volatile_status_durations.encore,
                    };
                    assert_eq!(duration, if initial == 2 { 0 } else { initial + 1 });
                    assert_eq!(slot.volatile_statuses.contains(&volatile), initial != 2);
                    state.reverse_instructions(&result.instruction_list);
                }
            }
        }
    }
}

#[test]
fn mcts_searches_joint_spread_actions_and_restores_the_root() {
    use poke_engine::engine::state::MoveOptions;
    use poke_engine::mcts::perform_mcts;
    use poke_engine::mcts_threaded::perform_mcts_shared_tree;
    use poke_engine::pokemon::PokemonName;
    use std::time::Duration;

    for threads in [1, 2] {
        let mut state = State::default();
        for side in &mut state.sides {
            for (index, pkmn) in side.pokemon.pkmn.iter_mut().enumerate() {
                if index >= 2 {
                    pkmn.hp = 0;
                } else {
                    pkmn.id = PokemonName::PIKACHU;
                    pkmn.replace_move(M::M0, Choices::MAKEITRAIN);
                    pkmn.replace_move(M::M1, Choices::DAZZLINGGLEAM);
                }
            }
        }
        let before = state.serialize();
        let mut options = MoveOptions::new();
        state.get_all_options(&mut options);
        let result = if threads == 1 {
            perform_mcts(
                &mut state,
                options.side_one_combined_options,
                options.side_two_combined_options,
                Duration::from_millis(100),
            )
        } else {
            perform_mcts_shared_tree(
                &mut state,
                options.side_one_combined_options,
                options.side_two_combined_options,
                Duration::from_millis(100),
                threads,
            )
        };
        assert!(result.iteration_count > 0);
        assert!(!result.s1.is_empty());
        assert_eq!(state.serialize(), before);
    }
}

#[cfg(feature = "benchmark_mcts")]
#[test]
fn fixed_work_search_respects_its_seed_count_and_root_state() {
    use poke_engine::mcts::benchmark_mcts;

    let mut state = State::default();
    for side in &mut state.sides {
        for (index, pkmn) in side.pokemon.pkmn.iter_mut().enumerate() {
            if index >= 2 {
                pkmn.hp = 0;
            } else {
                pkmn.replace_move(M::M0, Choices::ROCKSLIDE);
                pkmn.replace_move(M::M1, Choices::PROTECT);
            }
        }
    }
    let before = state.serialize();
    let (s1, s2) = state.root_get_all_options();
    let (first, _) = benchmark_mcts(&mut state, s1.clone(), s2.clone(), 2_000, 42);
    assert_eq!(before, state.serialize());
    let (second, _) = benchmark_mcts(&mut state, s1, s2, 2_000, 42);
    assert_eq!(before, state.serialize());
    assert_eq!(first.iteration_count, 2_000);
    assert_eq!(second.iteration_count, 2_000);
    for (a, b) in [(&first.s1, &second.s1), (&first.s2, &second.s2)].iter() {
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(b.iter()) {
            assert_eq!(a.move_choice, b.move_choice);
            assert_eq!(a.visits, b.visits);
            assert_eq!(a.total_score.to_bits(), b.total_score.to_bits());
        }
    }
}
