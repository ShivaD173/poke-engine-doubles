//! Synthetic fixed-position benchmark. See docs/performance.md for usage.
use poke_engine::choices::Choices as C;
use poke_engine::engine::abilities::Abilities as A;
use poke_engine::engine::items::Items as I;
use poke_engine::engine::state::MoveOptions;
use poke_engine::mcts::perform_mcts;
use poke_engine::mcts_threaded::perform_mcts_shared_tree;
use poke_engine::pokemon::PokemonName as P;
use poke_engine::state::{Pokemon, PokemonMoveIndex as M, PokemonType as T, State};
use std::time::{Duration, Instant};

#[cfg(feature = "benchmark_allocations")]
mod allocations {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicU64, Ordering};

    pub static COUNT: AtomicU64 = AtomicU64::new(0);
    pub struct CountingAllocator;
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            COUNT.fetch_add(1, Ordering::Relaxed);
            System.alloc(layout)
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            System.dealloc(ptr, layout)
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            COUNT.fetch_add(1, Ordering::Relaxed);
            System.realloc(ptr, layout, size)
        }
    }
}

#[cfg(feature = "benchmark_allocations")]
#[global_allocator]
static ALLOCATOR: allocations::CountingAllocator = allocations::CountingAllocator;
fn mon(id: P, types: (T, T), ability: A, item: I, stats: [i16; 6], moves: [C; 4]) -> Pokemon {
    let mut p = Pokemon::default();
    p.id = id;
    p.types = types;
    p.base_types = types;
    p.ability = ability;
    p.base_ability = ability;
    p.item = item;
    p.hp = stats[0];
    p.maxhp = stats[0];
    p.attack = stats[1];
    p.defense = stats[2];
    p.special_attack = stats[3];
    p.special_defense = stats[4];
    p.speed = stats[5];
    for (index, mv) in [M::M0, M::M1, M::M2, M::M3].iter().copied().zip(moves) {
        p.replace_move(index, mv);
    }
    p
}
fn position(name: &str) -> State {
    let mut s = State::default();
    for side in &mut s.sides {
        for p in &mut side.pokemon.pkmn {
            p.hp = 0;
        }
    }
    let ghold = mon(
        P::GHOLDENGO,
        (T::STEEL, T::GHOST),
        A::GOODASGOLD,
        I::LIFEORB,
        [181, 72, 115, 203, 111, 136],
        [C::MAKEITRAIN, C::SHADOWBALL, C::NASTYPLOT, C::PROTECT],
    );
    let chomp = mon(
        P::GARCHOMP,
        (T::DRAGON, T::GROUND),
        A::ROUGHSKIN,
        I::CLEARAMULET,
        [184, 182, 115, 90, 105, 169],
        [C::EARTHQUAKE, C::DRAGONCLAW, C::ROCKSLIDE, C::PROTECT],
    );
    let amoonguss = mon(
        P::AMOONGUSS,
        (T::GRASS, T::POISON),
        A::REGENERATOR,
        I::ROCKYHELMET,
        [221, 81, 114, 105, 110, 31],
        [C::SPORE, C::POLLENPUFF, C::RAGEPOWDER, C::PROTECT],
    );
    let arcanine = mon(
        P::ARCANINE,
        (T::FIRE, T::TYPELESS),
        A::INTIMIDATE,
        I::SITRUSBERRY,
        [197, 145, 110, 108, 110, 137],
        [C::FLAREBLITZ, C::SNARL, C::EXTREMESPEED, C::PROTECT],
    );
    s.sides[0].pokemon.pkmn[0] = ghold.clone();
    s.sides[0].pokemon.pkmn[1] = chomp.clone();
    s.sides[0].pokemon.pkmn[2] = amoonguss.clone();
    s.sides[0].pokemon.pkmn[3] = arcanine.clone();
    s.sides[1].pokemon.pkmn[0] = chomp;
    s.sides[1].pokemon.pkmn[1] = amoonguss;
    s.sides[1].pokemon.pkmn[2] = ghold;
    s.sides[1].pokemon.pkmn[3] = arcanine;
    if name == "chance-heavy" {
        for side in &mut s.sides {
            side.pokemon.pkmn[0].replace_move(M::M0, C::ROCKSLIDE);
            side.pokemon.pkmn[0].replace_move(M::M1, C::HEATWAVE);
            side.pokemon.pkmn[1].replace_move(M::M0, C::BLIZZARD);
            side.pokemon.pkmn[1].replace_move(M::M1, C::DISCHARGE);
        }
    }
    if name == "endgame" {
        for side in &mut s.sides {
            for p in &mut side.pokemon.pkmn[1..] {
                p.hp = 0;
            }
            side.pokemon.pkmn[0].hp = 70;
        }
    }
    s
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let name = args.get(1).map(String::as_str).unwrap_or("opening");
    assert!(["opening", "chance-heavy", "endgame"].contains(&name));
    let threads = args
        .get(2)
        .map(|v| v.parse::<usize>().unwrap())
        .unwrap_or(1);
    let millis = args
        .get(3)
        .map(|v| v.parse::<u64>().unwrap())
        .unwrap_or(3000);
    assert!(
        threads >= 1 && millis >= 1,
        "Threads and budget must be positive"
    );
    let mut s = position(name);
    let before = s.serialize();
    let mut options = MoveOptions::new();
    s.get_all_options(&mut options);
    let n1 = options.side_one_combined_options.len();
    let n2 = options.side_two_combined_options.len();
    #[cfg(feature = "benchmark_allocations")]
    allocations::COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
    let started = Instant::now();
    let result = if threads == 1 {
        perform_mcts(
            &mut s,
            options.side_one_combined_options,
            options.side_two_combined_options,
            Duration::from_millis(millis),
        )
    } else {
        perform_mcts_shared_tree(
            &mut s,
            options.side_one_combined_options,
            options.side_two_combined_options,
            Duration::from_millis(millis),
            threads,
        )
    };
    let elapsed = started.elapsed().as_secs_f64();
    assert_eq!(before, s.serialize());
    #[cfg(feature = "benchmark_allocations")]
    {
        let count = allocations::COUNT.load(std::sync::atomic::Ordering::Relaxed);
        println!(
            "ALLOCATIONS total={} per_iteration={:.3}",
            count,
            count as f64 / result.iteration_count as f64
        );
    }
    println!("BENCH name={} threads={} joint_options={}/{} iterations={} seconds={:.6} iterations_per_second={:.0}",name,threads,n1,n2,result.iteration_count,elapsed,result.iteration_count as f64/elapsed);
}
