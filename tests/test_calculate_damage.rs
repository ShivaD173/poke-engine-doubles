use poke_engine::choices::{Choice, Choices, MOVES};
use poke_engine::engine::abilities::Abilities;
use poke_engine::engine::generate_instructions::calculate_damage_rolls;
use poke_engine::engine::items::Items;
use poke_engine::engine::state::Weather;
use poke_engine::pokemon::PokemonName;
use poke_engine::state::{PokemonType, SideReference, SlotReference, State};

#[test]
fn test_basic_damage_calculation() {
    let mut state = State::default();

    let choice = MOVES.get(&Choices::TACKLE).unwrap().clone();

    let damage_rolls = calculate_damage_rolls(
        &mut state,
        SideReference::SideOne,
        &SlotReference::SlotA,
        SideReference::SideTwo,
        &SlotReference::SlotA,
        choice,
        &Choice::default(),
    );

    assert_eq!(damage_rolls, Some(vec![28, 42]));
}

#[test]
fn test_gives_no_damage_when_target_is_immune() {
    let mut state = State::default();
    state.sides[1].pokemon.pkmn[1].types.0 = PokemonType::GHOST;

    let choice = MOVES.get(&Choices::TACKLE).unwrap().clone();

    let damage_rolls = calculate_damage_rolls(
        &mut state,
        SideReference::SideOne,
        &SlotReference::SlotA,
        SideReference::SideTwo,
        &SlotReference::SlotB,
        choice,
        &Choice::default(),
    );

    assert_eq!(damage_rolls, Some(vec![0, 0]));
}

#[test]
fn test_seismictoss_does_static_damage() {
    let mut state = State::default();

    let choice = MOVES.get(&Choices::SEISMICTOSS).unwrap().clone();

    let damage_rolls = calculate_damage_rolls(
        &mut state,
        SideReference::SideOne,
        &SlotReference::SlotA,
        SideReference::SideTwo,
        &SlotReference::SlotA,
        choice,
        &Choice::default(),
    );

    assert_eq!(damage_rolls, Some(vec![50]));
}

#[test]
fn test_spread_move_damage_reduction() {
    let mut state = State::default();

    let choice = MOVES.get(&Choices::BREAKINGSWIPE).unwrap().clone();

    let damage_rolls = calculate_damage_rolls(
        &mut state,
        SideReference::SideOne,
        &SlotReference::SlotA,
        SideReference::SideTwo,
        &SlotReference::SlotA,
        choice,
        &Choice::default(),
    );

    assert_eq!(damage_rolls, Some(vec![21, 31]));
}

#[test]
fn test_earthquake_reduced_damage_if_our_side_alive() {
    // our side has 2 alive, but the other side has 1 alive
    // earthquake should do reduced damage because it is targetting
    // 2 alive pkmn (ours plus theirs)
    let mut state = State::default();
    state.sides[1].pokemon.pkmn[1].hp = 0;

    let choice = MOVES.get(&Choices::EARTHQUAKE).unwrap().clone();

    let damage_rolls = calculate_damage_rolls(
        &mut state,
        SideReference::SideOne,
        &SlotReference::SlotA,
        SideReference::SideTwo,
        &SlotReference::SlotA,
        choice,
        &Choice::default(),
    );

    assert_eq!(damage_rolls, Some(vec![34, 51]));
}

#[test]
fn test_earthquake_standard_damage_if_only_one_target() {
    // our side has only attacker alive, and the other side has 1 alive
    // earthquake should do full damage because it is targeting 1 alive pkmn (only theirs)
    let mut state = State::default();
    state.sides[1].pokemon.pkmn[1].hp = 0;
    state.sides[0].pokemon.pkmn[1].hp = 0;

    let choice = MOVES.get(&Choices::EARTHQUAKE).unwrap().clone();

    let damage_rolls = calculate_damage_rolls(
        &mut state,
        SideReference::SideOne,
        &SlotReference::SlotA,
        SideReference::SideTwo,
        &SlotReference::SlotA,
        choice,
        &Choice::default(),
    );

    assert_eq!(damage_rolls, Some(vec![46, 69]));
}

#[test]
fn test_spread_move_no_damage_reduction_if_single_target() {
    let mut state = State::default();
    state.sides[1].pokemon.pkmn[1].hp = 0;

    let choice = MOVES.get(&Choices::BREAKINGSWIPE).unwrap().clone();

    let damage_rolls = calculate_damage_rolls(
        &mut state,
        SideReference::SideOne,
        &SlotReference::SlotA,
        SideReference::SideTwo,
        &SlotReference::SlotA,
        choice,
        &Choice::default(),
    );

    assert_eq!(damage_rolls, Some(vec![28, 42]));
}

#[test]
fn test_snow_boosting_defense_with_crit() {
    let mut state = State::default();
    state.sides[1].pokemon.pkmn[0].types.0 = PokemonType::ICE;

    let damage_rolls_no_snow_boost = calculate_damage_rolls(
        &mut state,
        SideReference::SideOne,
        &SlotReference::SlotA,
        SideReference::SideTwo,
        &SlotReference::SlotA,
        MOVES.get(&Choices::TACKLE).unwrap().clone(),
        &Choice::default(),
    )
    .unwrap();

    state.weather.weather_type = Weather::SNOW;
    state.weather.turns_remaining = 1;

    let damage_rolls_snow_boost = calculate_damage_rolls(
        &mut state,
        SideReference::SideOne,
        &SlotReference::SlotA,
        SideReference::SideTwo,
        &SlotReference::SlotA,
        MOVES.get(&Choices::TACKLE).unwrap().clone(),
        &Choice::default(),
    )
    .unwrap();

    assert!(
        damage_rolls_no_snow_boost[0] > damage_rolls_snow_boost[0],
        "Expected snow boost damage to be less than no snow boost damage, but got {} and {}",
        damage_rolls_snow_boost[0],
        damage_rolls_no_snow_boost[0]
    );

    assert!(
        damage_rolls_no_snow_boost[1] > damage_rolls_snow_boost[1],
        "Expected snow boost crit damage to be less than no snow boost damage, but got {} and {}",
        damage_rolls_snow_boost[1],
        damage_rolls_no_snow_boost[1]
    );
}

#[test]
fn test_ivycudgel_changing_type() {
    // This was from a fuzz failure:
    // ogerponhearthflame using ivycudgel on a terastallized fire type means the damage is resisted
    let mut state = State::default();
    state.weather.weather_type = Weather::SUN;
    state.weather.turns_remaining = 3;
    state.sides[0].pokemon.pkmn[1].level = 50;
    state.sides[0].pokemon.pkmn[1].types = (PokemonType::GRASS, PokemonType::FIRE);
    state.sides[0].pokemon.pkmn[1].ability = Abilities::MOLDBREAKER;
    state.sides[0].pokemon.pkmn[1].item = Items::HEARTHFLAMEMASK;
    state.sides[0].pokemon.pkmn[1].id = PokemonName::OGERPONHEARTHFLAME;
    state.sides[0].pokemon.pkmn[1].attack = 181;

    state.sides[0].pokemon.pkmn[0].level = 50;
    state.sides[1].pokemon.pkmn[0].terastallized = true;
    state.sides[1].pokemon.pkmn[0].tera_type = PokemonType::FIRE;
    state.sides[1].pokemon.pkmn[0].hp = 207;
    state.sides[1].pokemon.pkmn[0].defense = 160;

    let choice = MOVES.get(&Choices::IVYCUDGEL).unwrap().clone();

    let damage_rolls = calculate_damage_rolls(
        &mut state,
        SideReference::SideOne,
        &SlotReference::SlotB,
        SideReference::SideTwo,
        &SlotReference::SlotA,
        choice,
        &Choice::default(),
    );

    assert_eq!(damage_rolls, Some(vec![68, 102]));
}
