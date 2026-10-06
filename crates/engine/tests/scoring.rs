//! Guards for `game::scoring`: one case per scoring rule of marjapussi.de (the source of truth,
//! owner decision D2), the series rules of the site's series table, and
//! `GameFinishedInfo::scores` on random games against a line-by-line port of the site's code.

mod common;

use common::{new_game, pick_action, site_scores};
use marjapussi_engine::GameFinishedInfo;
use marjapussi_engine::{
    game_scores, round5, team_points, GameOutcome, SeriesLength, SeriesSettings, SeriesTally, TeamPoints,
    SCHWARZ_FACTOR_FIFTHS,
};
use marjapussi_engine::{points_card, LAST_TRICK_BONUS, PAIR_POINTS};
use rand::rngs::StdRng;
use rand::SeedableRng;

fn outcome(playing: Option<u8>, value: i32, points: [i32; 2], tricks: [u8; 2]) -> GameOutcome {
    GameOutcome {
        playing_team: playing,
        game_value: value,
        team_points: points,
        team_tricks: tricks,
    }
}

fn score(o: GameOutcome) -> [i32; 2] {
    game_scores(&o, SCHWARZ_FACTOR_FIFTHS)
}

#[test]
fn rounding_to_five() {
    assert_eq!(
        [0, 2, 3, 137, 138, 140, 142, 143].map(round5),
        [0, 0, 5, 135, 140, 140, 140, 145]
    );
}

#[test]
fn contract_made_and_defence_rounded() {
    assert_eq!(score(outcome(Some(1), 150, [37, 183], [3, 6])), [35, 150]);
    assert_eq!(score(outcome(Some(0), 150, [150, 10], [8, 1])), [150, 10]);
}

#[test]
fn contract_failed() {
    assert_eq!(score(outcome(Some(0), 150, [122, 38], [6, 3])), [-150, 40]);
    assert_eq!(score(outcome(Some(1), 300, [12, 268], [1, 8])), [10, -300]);
}

#[test]
fn schwarz_doubles_only_when_the_contract_is_made() {
    assert_eq!(score(outcome(Some(0), 200, [240, 0], [9, 0])), [200, -400]);
    assert_eq!(score(outcome(Some(0), 300, [240, 0], [9, 0])), [-300, 0]);
}

#[test]
fn schwarz_of_the_playing_party_doubles_its_loss() {
    // Owner decision D2 (the site's table): declarers played schwarz lose −2 × value.
    assert_eq!(score(outcome(Some(0), 120, [0, 140], [0, 9])), [-240, 140]);
    assert_eq!(score(outcome(Some(1), 300, [180, 0], [9, 0])), [180, -600]);
    // Failing with at least one trick stays −value.
    assert_eq!(score(outcome(Some(0), 150, [2, 138], [1, 8])), [-150, 140]);
}

#[test]
fn all_passed_scores_rounded_points() {
    // `team_points` include announced pairs in all-pass games (owner decision D2).
    assert_eq!(score(outcome(None, 115, [112, 68], [5, 4])), [110, 70]);
    assert_eq!(score(outcome(None, 115, [72, 68], [5, 4])), [70, 70]);
    assert_eq!(score(outcome(None, 115, [0, 140], [0, 9])), [0, 140]);
}

#[test]
fn schwarz_factor_is_a_setting() {
    assert_eq!(game_scores(&outcome(Some(1), 200, [0, 240], [0, 9]), 15), [-600, 200]);
}

fn tally(length: SeriesLength, games: &[[i32; 2]]) -> SeriesTally {
    let mut t = SeriesTally::new(SeriesSettings {
        length,
        ..SeriesSettings::default()
    });
    for &g in games {
        t.add_scores(g);
    }
    t
}

#[test]
fn tournament_round_difference_and_bonus() {
    let t = tally(SeriesLength::Games(4), &[[150, 35], [140, -200], [-150, 40], [300, 0]]);
    assert!(t.is_over());
    assert_eq!(t.totals(), [440, -125]);
    assert_eq!(t.bonus_team(), None);
    let r = t.result();
    assert_eq!(r.difference, [113, -113]);
    assert_eq!(r.bonus, [0, 0]);
    assert_eq!(r.total, [553, -238]);

    let t = tally(SeriesLength::Games(4), &[[300, 0], [0, 260], [210, 20], [-150, 300]]);
    assert_eq!(t.bonus_team(), Some(0), "team 0 reached 510 first");
    let r = t.result();
    assert_eq!(r.games, [360, 580]);
    assert_eq!(r.difference, [-44, 44]);
    assert_eq!(r.bonus, [300, -300]);
    assert_eq!(r.total, [616, 324]);
}

#[test]
fn bonus_when_both_cross_in_the_same_game() {
    assert_eq!(
        tally(SeriesLength::Games(2), &[[400, 450], [150, 60]]).bonus_team(),
        Some(0)
    );
    assert_eq!(
        tally(SeriesLength::Games(2), &[[400, 450], [140, 140]]).bonus_team(),
        Some(1)
    );
    // Tie: the site's table gives the bonus to team 1.
    assert_eq!(
        tally(SeriesLength::Games(2), &[[400, 400], [100, 100]]).bonus_team(),
        Some(1)
    );
}

#[test]
fn playing_to_500_has_no_end_bonuses() {
    let t = tally(SeriesLength::UntilPoints(500), &[[300, 0], [140, 40]]);
    assert!(!t.is_over());
    assert_eq!(t.games_left(), None);
    let t = tally(SeriesLength::UntilPoints(500), &[[300, 0], [140, 40], [150, 10]]);
    assert!(t.is_over());
    let r = t.result();
    assert_eq!((r.difference, r.bonus, r.total), ([0, 0], [0, 0], [590, 50]));
}

#[test]
fn random_games_score_like_the_site() {
    let mut series = SeriesTally::new(SeriesSettings::default());
    let mut schwarz = 0;
    for seed in 0..3_000 {
        let mut rng = StdRng::seed_from_u64(7_000_000 + seed);
        let mut game = new_game(&mut rng);
        while !game.ended() {
            let a = pick_action(&game, &mut rng);
            game.apply_action_mut(a);
        }
        let info = GameFinishedInfo::from(game);
        assert_eq!(info.scores(), site_scores(&info), "seed {seed}");
        assert_eq!(info.declarer.map(|p| p.0 % 2), info.playing_party.map(|p| p.0));
        schwarz += info.outcome().team_tricks.contains(&0) as u32;
        series.add(&info.outcome());
    }
    eprintln!("3,000 random games, {schwarz} schwarz");
    assert!(schwarz >= 30, "schwarz games covered: {schwarz}");
    assert_eq!(series.games_played(), 3_000);
}

#[test]
fn running_team_points_at_every_state() {
    let mut pairs_seen = 0;
    for seed in 0..1_000 {
        let mut rng = StdRng::seed_from_u64(9_000_000 + seed);
        let mut game = new_game(&mut rng);
        let mut last = TeamPoints::default();
        while !game.ended() {
            let a = pick_action(&game, &mut rng);
            game.apply_action_mut(a);
            let p = team_points(&game.state.all_tricks, &game.all_events);
            let mut cards = [0; 2];
            for (i, t) in game.state.all_tricks.iter().enumerate() {
                let bonus = if i == 8 { LAST_TRICK_BONUS } else { 0 };
                cards[(t.winner.0 % 2) as usize] += t.cards.iter().map(|c| points_card(*c).0).sum::<i32>() + bonus;
            }
            assert_eq!(p.cards, cards, "seed {seed}");
            assert_eq!(
                p.tricks.iter().map(|&t| t as usize).sum::<usize>(),
                game.state.all_tricks.len()
            );
            for team in 0..2 {
                let added = p.pairs[team] - last.pairs[team];
                assert!(
                    added == 0 || PAIR_POINTS.contains(&added),
                    "seed {seed}: pair step {added}"
                );
                pairs_seen += (added > 0) as u32;
            }
            last = p;
        }
        let info = GameFinishedInfo::from(game);
        let expected = if info.no_one_played { last.cards } else { last.total() };
        assert_eq!(expected, info.team_points, "seed {seed}");
    }
    assert!(pairs_seen >= 50, "pairs covered: {pairs_seen}");
}
