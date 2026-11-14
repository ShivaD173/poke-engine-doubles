use super::*;
use rand::rngs::StdRng;

fn options(count: usize) -> Vec<MoveNode> {
    (0..count)
        .map(|_| MoveNode {
            move_choice: (MoveChoice::None, MoveChoice::None),
            total_score: 0.0,
            visits: 0,
        })
        .collect()
}

#[test]
fn explores_each_action_once_then_uses_its_value() {
    let mut options = options(5);
    let mut rng = StdRng::seed_from_u64(42);
    for visits in 0..5 {
        let index = Node::select_move_index(&options, visits, &mut rng);
        assert_eq!(options[index].visits, 0);
        options[index].visits = 1;
        options[index].total_score = if index == 3 { 1.0 } else { 0.0 };
    }
    assert!(options.iter().all(|option| option.visits == 1));
    // Value-based selection starts after five visits, not 5 * 50 visits.
    assert_eq!(Node::select_move_index(&options, 5, &mut rng), 3);
}

#[test]
fn sides_explore_independently_instead_of_repeating_diagonal_pairs() {
    let mut s1 = options(12);
    let mut s2 = options(12);
    let mut rng = StdRng::seed_from_u64(42);
    let mut pairs = Vec::new();
    for visits in 0..12 {
        let a = Node::select_move_index(&s1, visits, &mut rng);
        let b = Node::select_move_index(&s2, visits, &mut rng);
        assert_eq!(s1[a].visits, 0);
        assert_eq!(s2[b].visits, 0);
        s1[a].visits += 1;
        s2[b].visits += 1;
        pairs.push((a, b));
    }
    assert!(pairs.iter().any(|(a, b)| a != b));
    assert!(s1.iter().chain(s2.iter()).all(|option| option.visits == 1));
}

#[test]
fn ucb_uses_all_actions_and_breaks_equal_scores_randomly() {
    let mut options = options(4);
    for option in &mut options {
        option.visits = 10;
        option.total_score = 5.0;
    }
    let mut rng = StdRng::seed_from_u64(42);
    let mut counts = [0; 4];
    for _ in 0..20_000 {
        counts[Node::select_move_index(&options, 40, &mut rng)] += 1;
    }
    assert!(counts.iter().all(|&count| (4_600..5_400).contains(&count)));
    options[2].total_score = 10.0;
    assert_eq!(Node::select_move_index(&options, 40, &mut rng), 2);
    // Selection does not shuffle/reorder the action array under path indices.
    assert_eq!(options[2].total_score, 10.0);

    // UCB still explores an uncertain action even with a lower mean.
    options[0].visits = 100;
    options[0].total_score = 95.0;
    options[1].visits = 2;
    options[1].total_score = 0.2;
    options[2].visits = 100;
    options[2].total_score = 80.0;
    options[3].visits = 100;
    options[3].total_score = 80.0;
    assert_eq!(Node::select_move_index(&options, 302, &mut rng), 1);
}

#[test]
fn backpropagation_credits_actual_actions_at_every_edge_after_visit_50() {
    use crate::instruction::{DamageInstruction, Instruction};
    use crate::state::{PokemonIndex, SideReference};
    let mut root = Box::new(Node::new(0));
    root.root = true;
    root.times_visited = 75;
    root.s1_options = Some(options(3));
    root.s2_options = Some(options(5));
    let mut middle = Box::new(Node::new(1));
    middle.parent = &mut *root;
    middle.times_visited = 51;
    middle.s1_options = Some(options(20));
    middle.s2_options = Some(options(7));
    for node in [&mut *root, &mut *middle] {
        for side in [&mut node.s1_options, &mut node.s2_options] {
            let first = &mut side.as_mut().unwrap()[0];
            first.visits = node.times_visited;
            first.total_score = node.times_visited as f32 * 0.5;
        }
    }
    let mut leaf = Box::new(Node::new(2));
    leaf.parent = &mut *middle;
    middle
        .instructions
        .instruction_list
        .push(Instruction::Damage(DamageInstruction {
            side_ref: SideReference::SideOne,
            pokemon_index: PokemonIndex::P0,
            damage_amount: 7,
        }));
    leaf.instructions
        .instruction_list
        .push(Instruction::Damage(DamageInstruction {
            side_ref: SideReference::SideTwo,
            pokemon_index: PokemonIndex::P1,
            damage_amount: 11,
        }));
    let mut state = State::default();
    let before = state.serialize();
    state.apply_instructions(&middle.instructions.instruction_list);
    state.apply_instructions(&leaf.instructions.instruction_list);
    let path = vec![
        PathStep {
            parent: &mut *root,
            child: &mut *middle,
            s1_index: 2,
            s2_index: 4,
        },
        // This was previously incorrectly credited to index zero at visit 51.
        PathStep {
            parent: &mut *middle,
            child: &mut *leaf,
            s1_index: 11,
            s2_index: 2,
        },
    ];
    unsafe {
        leaf.backpropagate(0.75, &mut state, &path);
    }
    assert_eq!(state.serialize(), before);
    assert_eq!(root.times_visited, 76);
    assert_eq!(middle.times_visited, 52);
    assert_eq!(leaf.times_visited, 1);
    for (node, a, b, prior) in [(&*root, 2, 4, 75), (&*middle, 11, 2, 51)] {
        let s1 = node.s1_options.as_ref().unwrap();
        let s2 = node.s2_options.as_ref().unwrap();
        assert_eq!(s1[a].visits, 1);
        assert_eq!(s1[a].total_score, 0.75);
        assert_eq!(s2[b].visits, 1);
        assert_eq!(s2[b].total_score, 0.25);
        assert_eq!(s1[0].visits, prior);
        assert_eq!(s2[0].visits, prior);
        assert_eq!(
            s1.iter().map(|option| option.visits).sum::<u32>(),
            node.times_visited
        );
        assert_eq!(
            s2.iter().map(|option| option.visits).sum::<u32>(),
            node.times_visited
        );
    }
}

fn splash_duel() -> State {
    use crate::choices::Choices;
    use crate::pokemon::PokemonName;
    use crate::state::PokemonMoveIndex as M;
    let mut state = State::default();
    for (side_index, side) in state.sides.iter_mut().enumerate() {
        for (index, pkmn) in side.pokemon.pkmn.iter_mut().enumerate() {
            pkmn.hp = if index == 0 { 100 } else { 0 };
            pkmn.id = PokemonName::MAGIKARP;
            pkmn.speed = if side_index == 0 { 100 } else { 50 };
            pkmn.replace_move(M::M0, Choices::SPLASH);
            // Keep this deterministic one-action fixture the same under
            // the repository's Tera build as well as its Champions build.
            #[cfg(feature = "terastallization")]
            {
                pkmn.terastallized = true;
            }
            for index in [M::M1, M::M2, M::M3].iter() {
                pkmn.moves[index].disabled = true;
            }
        }
    }
    state
}

#[test]
fn reaches_six_completed_turns_and_reuses_edges_without_going_to_seven() {
    let mut state = splash_duel();
    let before = state.serialize();
    let (mut s1, mut s2) = state.root_get_all_options();
    assert_eq!(s1.len(), 1);
    assert_eq!(s2.len(), 1);
    let mut root = Node::new(0);
    root.root = true;
    unsafe {
        root.populate(&mut s1, &mut s2);
    }
    let root_eval = evaluate(&state);
    let mut move_options = MoveOptions::new();
    let mut children = HashMap::new();
    let mut path = Vec::new();
    let mut rng = StdRng::seed_from_u64(42);
    for iteration in 1..=12 {
        do_mcts(
            &mut root,
            &mut state,
            &root_eval,
            &mut move_options,
            &mut children,
            &mut path,
            &mut rng,
        );
        assert_eq!(state.serialize(), before);
        let depth = unsafe { (*path.last().unwrap().child).depth };
        assert_eq!(depth, iteration.min(6));
        assert_eq!(root.times_visited, iteration as u32);
    }
    assert_eq!(children.len(), 6);
    let capped = children
        .values()
        .flat_map(|branch| branch.nodes.iter())
        .find(|node| node.depth == 6)
        .unwrap();
    assert_eq!(capped.times_visited, 7);
    assert!(capped.s1_options.is_none());
    assert!(capped.s2_options.is_none());
    assert!(children
        .values()
        .flat_map(|branch| branch.nodes.iter())
        .all(|node| node.depth <= 6));
    assert_eq!(root.s1_options.as_ref().unwrap()[0].visits, 12);
    assert_eq!(root.s2_options.as_ref().unwrap()[0].visits, 12);
}

#[test]
fn forced_replacement_does_not_consume_a_completed_turn() {
    use crate::state::PokemonIndex;
    let mut state = splash_duel();
    let replacement = state.sides[0].pokemon.pkmn[0].clone();
    state.sides[0].pokemon.pkmn[0].hp = 0;
    state.sides[0].pokemon.pkmn[2] = replacement;
    state.sides[0].slot_a.force_switch = true;
    let before = state.serialize();
    let (mut s1, mut s2) = state.root_get_all_options();
    assert_eq!(s1.len(), 1);
    assert_eq!(s2.len(), 1);
    assert_eq!(s1[0].0, MoveChoice::Switch(PokemonIndex::P2));
    let mut root = Node::new(5);
    root.root = true;
    unsafe {
        root.populate(&mut s1, &mut s2);
    }
    let root_eval = evaluate(&state);
    let mut move_options = MoveOptions::new();
    let mut children = HashMap::new();
    let mut path = Vec::new();
    let mut rng = StdRng::seed_from_u64(42);
    do_mcts(
        &mut root,
        &mut state,
        &root_eval,
        &mut move_options,
        &mut children,
        &mut path,
        &mut rng,
    );
    let replacement = unsafe { &*path.last().unwrap().child };
    assert_eq!(replacement.depth, 5);
    assert!(!replacement.instructions.end_of_turn_triggered);
    assert_eq!(state.serialize(), before);
    do_mcts(
        &mut root,
        &mut state,
        &root_eval,
        &mut move_options,
        &mut children,
        &mut path,
        &mut rng,
    );
    let next_turn = unsafe { &*path.last().unwrap().child };
    assert_eq!(next_turn.depth, 6);
    assert!(next_turn.instructions.end_of_turn_triggered);
    assert_eq!(state.serialize(), before);
}

#[test]
fn concentrates_on_a_guaranteed_ko_instead_of_a_guaranteed_loss() {
    use crate::choices::Choices;
    use crate::state::{PokemonMoveIndex as M, SideReference, SlotReference};
    let mut state = splash_duel();
    for side in &mut state.sides {
        side.pokemon.pkmn[0].hp = 1;
    }
    let ours = &mut state.sides[0].pokemon.pkmn[0];
    ours.replace_move(M::M1, Choices::TACKLE);
    ours.moves.m1.disabled = false;
    state.sides[1].pokemon.pkmn[0].replace_move(M::M0, Choices::TACKLE);
    // Ours is faster: attacking KOs first; Splash instead loses to Tackle.
    let before = state.serialize();
    let (mut s1, mut s2) = state.root_get_all_options();
    assert_eq!(s1.len(), 2);
    assert_eq!(s2.len(), 1);
    let mut root = Node::new(0);
    root.root = true;
    unsafe {
        root.populate(&mut s1, &mut s2);
    }
    let root_eval = evaluate(&state);
    let mut move_options = MoveOptions::new();
    let mut children = HashMap::new();
    let mut path = Vec::new();
    let mut rng = StdRng::seed_from_u64(42);
    for _ in 0..500 {
        do_mcts(
            &mut root,
            &mut state,
            &root_eval,
            &mut move_options,
            &mut children,
            &mut path,
            &mut rng,
        );
        assert_eq!(state.serialize(), before);
    }
    let result = collect_result(&root);
    let attack = result
        .s1
        .iter()
        .find(|option| {
            option.move_choice.0
                == MoveChoice::Move(SlotReference::SlotA, SideReference::SideTwo, M::M1)
        })
        .unwrap();
    let splash = result
        .s1
        .iter()
        .find(|option| {
            option.move_choice.0
                == MoveChoice::Move(SlotReference::SlotA, SideReference::SideOne, M::M0)
        })
        .unwrap();
    assert!(attack.visits > 450);
    assert_eq!(attack.total_score, attack.visits as f32);
    assert!(splash.visits > 0);
    assert_eq!(splash.total_score, 0.0);
    assert_eq!(
        result.s1.iter().map(|option| option.visits).sum::<u32>(),
        result.iteration_count
    );
    assert_eq!(
        result.s2.iter().map(|option| option.visits).sum::<u32>(),
        result.iteration_count
    );
}

#[test]
fn chance_sampling_preserves_relative_weights_and_skips_zeroes() {
    let nodes: Box<[Node]> = [0.0, 2.0, 0.0, 6.0, 0.0]
        .iter()
        .map(|&weight| {
            let mut node = Node::new(1);
            node.instructions.percentage = weight;
            node
        })
        .collect();
    let mut branch = ChanceBranch::new(nodes);
    let mut rng = StdRng::seed_from_u64(42);
    let mut light = 0;
    for _ in 0..20_000 {
        let weight = unsafe { (*branch.sample(&mut rng)).instructions.percentage };
        assert!(weight == 2.0 || weight == 6.0);
        if weight == 2.0 {
            light += 1;
        }
    }
    assert!(
        (4_600..5_400).contains(&light),
        "light branch sampled {} times",
        light
    );
}

#[test]
fn singleton_chance_branch_returns_its_only_node() {
    let mut nodes = vec![Node::new(1)].into_boxed_slice();
    let expected = &mut nodes[0] as *mut Node;
    let mut branch = ChanceBranch::new(nodes);
    let mut rng = StdRng::seed_from_u64(42);
    assert_eq!(branch.sample(&mut rng), expected);
}
