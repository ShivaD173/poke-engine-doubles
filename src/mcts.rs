use crate::engine::evaluate::evaluate;
use crate::engine::generate_instructions::generate_instructions_from_move_pair;
use crate::engine::state::{MoveChoice, MoveOptions};
use crate::instruction::StateInstructions;
use crate::state::State;
use rand::distr::weighted::WeightedIndex;
use rand::prelude::*;
use rand::rng;
use std::collections::HashMap;
use std::time::Duration;

const MCTS_MAX_DEPTH: u8 = 6;
const ROOT_INITIAL_VISITS: u32 = 20;
const NON_ROOT_INITIAL_VISITS: u32 = 4;

fn sigmoid(x: f32) -> f32 {
    // Tuned so that ~400 points is very close to 1.0
    1.0 / (1.0 + (-0.0062 * x).exp())
}

#[derive(Debug)]
pub struct Node {
    pub root: bool,
    pub depth: u8,
    pub parent: *mut Node,
    pub times_visited: u32,

    // represents the instructions that led to this node from the parent
    pub instructions: StateInstructions,

    // represents the total score and number of visits for this node
    // de-coupled for s1 and s2
    pub s1_options: Option<Vec<MoveNode>>,
    pub s2_options: Option<Vec<MoveNode>>,
}

// Owns stable node addresses and builds its chance distribution only once.
// Repeated selection does not allocate or rescan every outcome's weight.
pub struct ChanceBranch {
    nodes: Box<[Node]>,
    distribution: Option<WeightedIndex<f64>>,
}

impl ChanceBranch {
    fn new(nodes: Box<[Node]>) -> Self {
        assert!(!nodes.is_empty(), "Cannot sample an empty chance branch");
        let distribution = if nodes.len() == 1 {
            let weight = nodes[0].instructions.percentage;
            assert!(
                weight > 0.0 && weight.is_finite(),
                "Invalid chance branch weight"
            );
            None
        } else {
            Some(
                WeightedIndex::new(nodes.iter().map(|node| node.instructions.percentage as f64))
                    .unwrap(),
            )
        };
        Self {
            nodes,
            distribution,
        }
    }

    fn sample<R: Rng + ?Sized>(&mut self, rng: &mut R) -> *mut Node {
        let index = self
            .distribution
            .as_ref()
            .map_or(0, |distribution| distribution.sample(rng));
        &mut self.nodes[index]
    }
}

// Edges actually traversed in the current iteration. Option arrays keep a stable
// order; these indices never have to be inferred from a node's visit count.
struct PathStep {
    parent: *mut Node,
    child: *mut Node,
    s1_index: usize,
    s2_index: usize,
}

impl Node {
    fn new(depth: u8) -> Node {
        Node {
            root: false,
            depth,
            parent: std::ptr::null_mut(),
            instructions: StateInstructions::default(),
            times_visited: 0,
            s1_options: None,
            s2_options: None,
        }
    }
    unsafe fn populate(
        &mut self,
        s1_options: &mut Vec<(MoveChoice, MoveChoice)>,
        s2_options: &mut Vec<(MoveChoice, MoveChoice)>,
    ) {
        let s1_options_vec = s1_options
            .drain(..)
            .map(|x| MoveNode {
                move_choice: x,
                total_score: 0.0,
                visits: 0,
            })
            .collect();
        let s2_options_vec = s2_options
            .drain(..)
            .map(|x| MoveNode {
                move_choice: x,
                total_score: 0.0,
                visits: 0,
            })
            .collect();

        self.s1_options = Some(s1_options_vec);
        self.s2_options = Some(s2_options_vec);
    }

    unsafe fn should_branch_on_damage(&self) -> bool {
        if self.root {
            return true;
        }

        // if there aren't many options left, branch on damage if we're one node below the root
        if (*self.parent).root && self.s1_options.as_ref().unwrap().len() < 20
            || self.s2_options.as_ref().unwrap().len() < 20
        {
            return true;
        }
        false
    }

    pub fn maximize_ucb_for_side(&self, side_map: &[MoveNode]) -> usize {
        let mut choice = 0;
        let mut best_ucb1 = f32::MIN;
        for (index, node) in side_map.iter().enumerate() {
            let this_ucb1 = node.ucb1(self.times_visited);
            if this_ucb1 > best_ucb1 {
                best_ucb1 = this_ucb1;
                choice = index;
            }
        }
        choice
    }

    fn initial_visits(&self) -> u32 {
        if self.root {
            ROOT_INITIAL_VISITS
        } else {
            NON_ROOT_INITIAL_VISITS
        }
    }

    fn select_move_index<R: Rng + ?Sized>(&self, options: &[MoveNode], rng: &mut R) -> usize {
        // Sample independently for each side until every action reaches this
        // node's minimum. Actual action visits, not parent visits, set the phase.
        let minimum = self.initial_visits();
        let under_sampled = options
            .iter()
            .filter(|option| option.visits < minimum)
            .count();
        if under_sampled > 0 {
            let rank = rng.random_range(0..under_sampled);
            return options
                .iter()
                .enumerate()
                .filter(|(_, option)| option.visits < minimum)
                .nth(rank)
                .unwrap()
                .0;
        }

        let exploration = 2.0 * (self.times_visited.max(1) as f32).ln();
        let mut best_score = f32::NEG_INFINITY;
        let mut best_index = 0;
        let mut ties = 0;
        for (index, option) in options.iter().enumerate() {
            let score = option.ucb1_with_exploration(exploration);
            if score > best_score {
                best_score = score;
                best_index = index;
                ties = 1;
            } else if score == best_score {
                // Equal scores should not align the two sides' choices by
                // their array order. Reservoir sampling breaks ties uniformly.
                ties += 1;
                if rng.random_range(0..ties) == 0 {
                    best_index = index;
                }
            }
        }
        best_index
    }

    unsafe fn selection<R: Rng + ?Sized>(
        &mut self,
        state: &mut State,
        move_options: &mut MoveOptions,
        children: &mut HashMap<(usize, u16, u16), ChanceBranch>,
        path: &mut Vec<PathStep>,
        rng: &mut R,
    ) -> (*mut Node, usize, usize) {
        let return_node = self as *mut Node;
        if self.depth >= MCTS_MAX_DEPTH {
            // These action indices are never used: expand stops at the cap.
            return (return_node, 0, 0);
        }
        if self.s1_options.is_none() {
            state.get_all_options(move_options);
            self.populate(
                &mut move_options.side_one_combined_options,
                &mut move_options.side_two_combined_options,
            );
        }

        let s1_mc_index = self.select_move_index(self.s1_options.as_ref().unwrap(), rng);
        let s2_mc_index = self.select_move_index(self.s2_options.as_ref().unwrap(), rng);

        let s1_key = self.s1_options.as_ref().unwrap()[s1_mc_index].to_u16();
        let s2_key = self.s2_options.as_ref().unwrap()[s2_mc_index].to_u16();
        let key = (return_node as usize, s1_key, s2_key);
        match children.get_mut(&key) {
            Some(branch) => {
                let chosen_child = branch.sample(rng);
                state.apply_instructions(&(*chosen_child).instructions.instruction_list);
                path.push(PathStep {
                    parent: return_node,
                    child: chosen_child,
                    s1_index: s1_mc_index,
                    s2_index: s2_mc_index,
                });
                (*chosen_child).selection(state, move_options, children, path, rng)
            }
            None => (return_node, s1_mc_index, s2_mc_index),
        }
    }

    fn should_not_expand(
        &self,
        state: &State,
        s1_move: &(MoveChoice, MoveChoice),
        s2_move: &(MoveChoice, MoveChoice),
    ) -> bool {
        if (state.battle_is_over() != 0.0 && !self.root)
            || (s1_move == &(MoveChoice::None, MoveChoice::None)
                && s2_move == &(MoveChoice::None, MoveChoice::None))
        {
            true
        } else {
            false
        }
    }

    pub unsafe fn expand<R: Rng + ?Sized>(
        &mut self,
        state: &mut State,
        s1_move_index: usize,
        s2_move_index: usize,
        children: &mut HashMap<(usize, u16, u16), ChanceBranch>,
        rng: &mut R,
    ) -> *mut Node {
        if self.depth >= MCTS_MAX_DEPTH {
            return self as *mut Node;
        }
        let node_addr = self as *mut Node as usize;
        let s1_option = &self.s1_options.as_ref().unwrap()[s1_move_index];
        let s2_option = &self.s2_options.as_ref().unwrap()[s2_move_index];
        let s1_move = &s1_option.move_choice;
        let s2_move = &s2_option.move_choice;
        if self.should_not_expand(state, &s1_move, &s2_move) {
            return self as *mut Node;
        }
        let key = (node_addr, s1_option.to_u16(), s2_option.to_u16());
        let should_branch_on_damage = self.should_branch_on_damage();
        let mut new_instructions = generate_instructions_from_move_pair(
            state,
            &s1_move.0,
            &s1_move.1,
            &s2_move.0,
            &s2_move.1,
            should_branch_on_damage,
        );
        let mut this_pair_vec = Vec::with_capacity(new_instructions.len());
        for state_instructions in new_instructions.drain(..) {
            let new_depth = if state_instructions.end_of_turn_triggered {
                self.depth + 1
            } else {
                self.depth
            };

            let mut new_node = Node::new(new_depth);
            new_node.parent = self;
            new_node.instructions = state_instructions;

            this_pair_vec.push(new_node);
        }

        // sample a node from the new instruction list.
        // this is the node that the rollout will be done on
        let mut branch = ChanceBranch::new(this_pair_vec.into_boxed_slice());
        let new_node_ptr = branch.sample(rng);
        state.apply_instructions(&(*new_node_ptr).instructions.instruction_list);
        children.insert(key, branch);
        new_node_ptr
    }

    unsafe fn backpropagate(&mut self, score: f32, state: &mut State, path: &[PathStep]) {
        self.times_visited += 1;
        for step in path.iter().rev() {
            let parent = &mut *step.parent;
            parent.times_visited += 1;

            let s1 = &mut parent.s1_options.as_mut().unwrap()[step.s1_index];
            s1.total_score += score;
            s1.visits += 1;

            let s2 = &mut parent.s2_options.as_mut().unwrap()[step.s2_index];
            s2.total_score += 1.0 - score;
            s2.visits += 1;

            state.reverse_instructions(&(*step.child).instructions.instruction_list);
        }
    }

    pub fn rollout(&mut self, state: &mut State, root_eval: &f32) -> f32 {
        let battle_is_over = state.battle_is_over();
        if battle_is_over == 0.0 {
            let eval = evaluate(state);
            sigmoid(eval - root_eval)
        } else {
            if battle_is_over == -1.0 {
                0.0
            } else {
                battle_is_over
            }
        }
    }
}

#[derive(Debug)]
pub struct MoveNode {
    pub move_choice: (MoveChoice, MoveChoice),
    pub total_score: f32,
    pub visits: u32,
}

impl MoveNode {
    pub fn to_u16(&self) -> u16 {
        (self.move_choice.0.to_u8() as u16) << 8 | (self.move_choice.1.to_u8() as u16)
    }
    pub fn ucb1(&self, parent_visits: u32) -> f32 {
        self.ucb1_with_exploration(2.0 * (parent_visits as f32).ln())
    }

    fn ucb1_with_exploration(&self, exploration: f32) -> f32 {
        if self.visits == 0 {
            return f32::INFINITY;
        }
        let score =
            (self.total_score / self.visits as f32) + (exploration / self.visits as f32).sqrt();
        score
    }
    pub fn average_score(&self) -> f32 {
        let score = self.total_score / self.visits as f32;
        score
    }
}

#[derive(Clone)]
pub struct MctsSideResult {
    pub move_choice: (MoveChoice, MoveChoice),
    pub total_score: f32,
    pub visits: u32,
}

impl MctsSideResult {
    pub fn average_score(&self) -> f32 {
        if self.visits == 0 {
            return 0.0;
        }
        let score = self.total_score / self.visits as f32;
        score
    }
}

pub struct MctsResult {
    pub s1: Vec<MctsSideResult>,
    pub s2: Vec<MctsSideResult>,
    pub iteration_count: u32,
}

fn do_mcts<R: Rng + ?Sized>(
    root_node: &mut Node,
    state: &mut State,
    root_eval: &f32,
    move_options: &mut MoveOptions,
    children: &mut HashMap<(usize, u16, u16), ChanceBranch>,
    path: &mut Vec<PathStep>,
    rng: &mut R,
) {
    path.clear();
    let (leaf, s1_move, s2_move) =
        unsafe { root_node.selection(state, move_options, children, path, rng) };
    let new_node = unsafe { (*leaf).expand(state, s1_move, s2_move, children, rng) };
    if new_node != leaf {
        path.push(PathStep {
            parent: leaf,
            child: new_node,
            s1_index: s1_move,
            s2_index: s2_move,
        });
    }
    let rollout_result = unsafe { (*new_node).rollout(state, root_eval) };
    unsafe { (*new_node).backpropagate(rollout_result, state, path) }
}

pub fn perform_mcts(
    state: &mut State,
    mut side_one_options: Vec<(MoveChoice, MoveChoice)>,
    mut side_two_options: Vec<(MoveChoice, MoveChoice)>,
    max_time: Duration,
) -> MctsResult {
    let mut root_node = Node::new(0);
    unsafe {
        root_node.populate(&mut side_one_options, &mut side_two_options);
    }
    root_node.root = true;

    let mut combined_options = MoveOptions::new();
    let mut children: HashMap<(usize, u16, u16), ChanceBranch> = HashMap::new();
    let root_eval = evaluate(state);
    let mut path = Vec::with_capacity(16);
    let mut rng = rng();
    let start_time = std::time::Instant::now();
    while start_time.elapsed() < max_time {
        for _ in 0..1000 {
            do_mcts(
                &mut root_node,
                state,
                &root_eval,
                &mut combined_options,
                &mut children,
                &mut path,
                &mut rng,
            );
        }

        /*
        Cut off after 25 million iterations

        Under normal circumstances the bot will only run for 2.5-3.5 million iterations
        however towards the end of a battle the bot may perform tens of millions of iterations

        Beyond about 30 million iterations some floating point nonsense happens where
        MoveNode.total_score stops updating because f32 does not have enough precision

        I can push the problem farther out by using f64 but if the bot is running for this long
        then it almost certainly sees a forced win
        */
        if root_node.times_visited == 25_000_000 {
            break;
        }
    }

    #[cfg(feature = "mcts_stats")]
    print_tree_stats(&root_node, &children);

    collect_result(&root_node)
}

fn collect_result(root: &Node) -> MctsResult {
    MctsResult {
        s1: root
            .s1_options
            .as_ref()
            .unwrap()
            .iter()
            .map(|v| MctsSideResult {
                move_choice: v.move_choice.clone(),
                total_score: v.total_score,
                visits: v.visits,
            })
            .collect(),
        s2: root
            .s2_options
            .as_ref()
            .unwrap()
            .iter()
            .map(|v| MctsSideResult {
                move_choice: v.move_choice.clone(),
                total_score: v.total_score,
                visits: v.visits,
            })
            .collect(),
        iteration_count: root.times_visited,
    }
}

// debug helper: walks the built tree from `root` via the children map and
// prints per-depth counts of nodes, leaves, and visits. a leaf is a node
// that has no entry in the children map (either depth-capped, terminal, or
// simply never re-selected after being created by its parent's expand).
//
// useful for sanity-checking search shape -- a deep tree (visits concentrated
// at large depth values) suggests good exploitation; a shallow, wide tree
// (many leaves at depth 0-1) suggests the search is fanning out at the root
// without descending.
#[cfg(feature = "mcts_stats")]
fn print_tree_stats(root: &Node, children: &HashMap<(usize, u16, u16), ChanceBranch>) {
    let mut by_parent: HashMap<usize, Vec<&Node>> = HashMap::new();
    for (key, kids) in children.iter() {
        let bucket = by_parent.entry(key.0).or_default();
        for child in kids.nodes.iter() {
            bucket.push(child);
        }
    }

    let mut nodes_by_depth: HashMap<u8, u64> = HashMap::new();
    let mut leaves_by_depth: HashMap<u8, u64> = HashMap::new();
    let mut visits_by_depth: HashMap<u8, u64> = HashMap::new();
    let mut total_nodes = 0u64;

    let mut stack: Vec<&Node> = vec![root];
    while let Some(node) = stack.pop() {
        let depth = node.depth;
        let visits = node.times_visited as u64;

        *nodes_by_depth.entry(depth).or_insert(0) += 1;
        *visits_by_depth.entry(depth).or_insert(0) += visits;
        total_nodes += 1;

        let addr = node as *const Node as usize;
        let mut had_children = false;
        if let Some(kids) = by_parent.get(&addr) {
            for child in kids {
                had_children = true;
                stack.push(*child);
            }
        }
        if !had_children {
            *leaves_by_depth.entry(depth).or_insert(0) += 1;
        }
    }

    let mut depths: Vec<u8> = nodes_by_depth.keys().copied().collect();
    depths.sort();
    let total_leaves: u64 = leaves_by_depth.values().sum();

    println!();
    println!("== Tree shape ==");
    println!("Distinct nodes: {}", total_nodes);
    println!("Leaf nodes:     {}", total_leaves);
    println!("Branch entries: {}", children.len());
    println!();
    println!(
        "{:>5}  {:>12}  {:>12}  {:>14}  {:>12}",
        "depth", "nodes", "leaves", "total_visits", "avg_visits"
    );
    for d in depths {
        let n = nodes_by_depth[&d];
        let l = leaves_by_depth.get(&d).copied().unwrap_or(0);
        let v = visits_by_depth[&d];
        let avg = if n > 0 { v as f64 / n as f64 } else { 0.0 };
        println!("{:>5}  {:>12}  {:>12}  {:>14}  {:>12.1}", d, n, l, v, avg);
    }
}

#[cfg(test)]
mod tests;

/// Fixed-work, seeded profiling entry point. Production searches use ThreadRng.
/// The measured duration covers only iterations, excluding setup and destruction.
#[cfg(feature = "benchmark_mcts")]
pub fn benchmark_mcts(
    state: &mut State,
    mut side_one_options: Vec<(MoveChoice, MoveChoice)>,
    mut side_two_options: Vec<(MoveChoice, MoveChoice)>,
    iterations: u32,
    seed: u64,
) -> (MctsResult, Duration) {
    assert!(iterations > 0 && iterations <= 25_000_000);
    let mut root = Node::new(0);
    unsafe {
        root.populate(&mut side_one_options, &mut side_two_options);
    }
    root.root = true;
    let mut options = MoveOptions::new();
    let mut children = HashMap::new();
    let root_eval = evaluate(state);
    let mut path = Vec::with_capacity(16);
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let started = std::time::Instant::now();
    for _ in 0..iterations {
        do_mcts(
            &mut root,
            state,
            &root_eval,
            &mut options,
            &mut children,
            &mut path,
            &mut rng,
        );
    }
    let elapsed = started.elapsed();
    (collect_result(&root), elapsed)
}
