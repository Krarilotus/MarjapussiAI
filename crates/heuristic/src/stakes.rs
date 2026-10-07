//! What points are worth to my team: the expected score difference (ours − theirs) at the end
//! of the game, from the engine's scoring (`engine::game_scores`, the one owner of the rules).
//! The series is decided by that difference, so it is the one objective every valuation term
//! reads its point worth from ([D]). The worth is not symmetric: as declarers our points count
//! only through P(made) × 2 · value, every point the defenders take costs us one, and a team's
//! first trick matters while the scoring punishes taking none.
//!
//! The model of the rest of the game is a smooth "fair share": of the points still out (live
//! cards and the last-trick bonus), my team takes X ~ Normal(out / 2, out / (2 √n)), clipped to
//! [0, out] and summed over whole points, over the n tricks left (each trick a fair share, won
//! with probability ½). A team without a trick yet takes none of the n tricks with
//! probability ½ⁿ. Everything else (made or failed, rounding to 5, schwarz) is `game_scores`.

use engine::{game_scores, GameOutcome, CARD_POINTS, LAST_TRICK_BONUS, SCHWARZ_FACTOR_FIFTHS, TRICKS};
use knowledge::Knowledge;
use observation::Seat;

/// The state the score depends on, by team (0 = my team, 1 = theirs).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stakes {
    /// My team's index in the engine's team order (seat % 2).
    mine: usize,
    playing_team: Option<u8>,
    value: i32,
    points: [f64; 2],
    tricks: [u8; 2],
    /// Points still out and tricks left.
    out: f64,
    n: u8,
}

impl Stakes {
    pub fn new(k: &Knowledge, me: Seat) -> Stakes {
        let mine = (me % 2) as usize;
        let by_team = |x: [i32; 2]| [x[mine] as f64, x[1 - mine] as f64];
        let won = |t: usize| k.trick_winners().iter().filter(|&&w| w as usize % 2 == t).count() as u8;
        let done = k.trick_winners().len() as u8;
        let bonus = if done < TRICKS { LAST_TRICK_BONUS } else { 0 };
        let out = (!k.played())
            .iter()
            .map(|c| CARD_POINTS[c as usize] as i32)
            .sum::<i32>()
            + bonus;
        Stakes {
            mine,
            playing_team: k.declarer().map(|d| d % 2),
            value: k.value(),
            points: by_team(k.team_points()),
            tricks: [won(mine), won(1 - mine)],
            out: out as f64,
            n: TRICKS - done,
        }
    }

    /// Whether my team plays the contract.
    pub fn declaring(&self) -> bool {
        self.playing_team == Some(self.mine as u8)
    }

    /// The final score difference (ours − theirs) for final points and tricks by team.
    fn score(&self, points: [f64; 2], tricks: [u8; 2]) -> f64 {
        let mut o = GameOutcome {
            playing_team: self.playing_team,
            game_value: self.value,
            team_points: [0; 2],
            team_tricks: [0; 2],
        };
        o.team_points[self.mine] = points[0].round() as i32;
        o.team_points[1 - self.mine] = points[1].round() as i32;
        o.team_tricks[self.mine] = tricks[0];
        o.team_tricks[1 - self.mine] = tricks[1];
        let s = game_scores(&o, SCHWARZ_FACTOR_FIFTHS);
        (s[self.mine] - s[1 - self.mine]) as f64
    }

    /// The expected final score difference from points and tricks by team, `out` points still
    /// out and `n` tricks left.
    fn expected(&self, points: [f64; 2], tricks: [u8; 2], out: f64, n: u8) -> f64 {
        if n == 0 {
            return self.score(points, tricks);
        }
        let none = 0.5f64.powi(n as i32);
        let (mut total, mut mass) = (0.0, 1.0);
        for t in 0..2 {
            if tricks[t] == 0 {
                // Team t takes none of the n tricks: the other team takes everything out.
                let (mut p, mut w) = (points, tricks);
                p[1 - t] += out;
                w[1 - t] += n;
                total += none * self.score(p, w);
                mass -= none;
            }
        }
        // Final points are whole numbers: sum over every split of what is out, weighted by the
        // normal density (renormalised), so the scoring's rounding to 5 averages out smoothly.
        let sd = (out / (2.0 * (n as f64).sqrt())).max(0.5);
        let mean = out / 2.0;
        let (mut sum, mut norm) = (0.0, 0.0);
        for x in 0..=out.round() as i32 {
            let x = x as f64;
            let w = (-0.5 * ((x - mean) / sd).powi(2)).exp();
            sum += w * self.score([points[0] + x, points[1] + out - x], tricks.map(|t| t.max(1)));
            norm += w;
        }
        let split = sum / norm;
        total + mass * split
    }

    /// The expected score difference now.
    pub fn now(&self) -> f64 {
        self.expected(self.points, self.tricks, self.out, self.n)
    }

    /// A trick with `t` points, `from_out` of them still out now: the expected score
    /// difference if my team takes it and if the other team does, relative to now.
    pub fn trick(&self, t: f64, from_out: f64) -> (f64, f64) {
        let out = (self.out - from_out).max(0.0);
        let n = self.n.saturating_sub(1);
        let now = self.now();
        let take = self.expected(
            [self.points[0] + t, self.points[1]],
            [self.tricks[0] + 1, self.tricks[1]],
            out,
            n,
        );
        let give = self.expected(
            [self.points[0], self.points[1] + t],
            [self.tricks[0], self.tricks[1] + 1],
            out,
            n,
        );
        (take - now, give - now)
    }

    /// What `points` more for my team now (a meld) are worth.
    pub fn meld(&self, points: f64) -> f64 {
        self.expected([self.points[0] + points, self.points[1]], self.tricks, self.out, self.n) - self.now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stakes(playing_team: Option<u8>, value: i32, points: [f64; 2], tricks: [u8; 2], out: f64, n: u8) -> Stakes {
        Stakes {
            mine: 0,
            playing_team,
            value,
            points,
            tricks,
            out,
            n,
        }
    }

    #[test]
    fn a_made_contract_leaves_the_declarers_only_the_defenders_points() {
        // Declarers with 130 of 120 and a trick: our own meld is worth nothing more, a trick is
        // worth what the defenders don't get.
        let s = stakes(Some(0), 120, [130.0, 40.0], [4, 2], 50.0, 3);
        assert!(s.meld(40.0).abs() < 1e-9);
        let (take, give) = s.trick(20.0, 20.0);
        assert!((take - give - 20.0).abs() < 3.0, "{take} {give}");
    }

    #[test]
    fn defenders_meld_counts_one_to_one_and_declarers_meld_only_near_the_line() {
        let defenders = stakes(Some(1), 120, [30.0, 60.0], [2, 3], 90.0, 4);
        assert!((defenders.meld(40.0) - 40.0).abs() < 5.0);
        let far = stakes(Some(0), 200, [20.0, 60.0], [1, 3], 50.0, 3);
        let near = stakes(Some(0), 120, [90.0, 30.0], [3, 2], 60.0, 4);
        assert!(near.meld(40.0) > far.meld(40.0));
        assert!(near.meld(40.0) > 40.0, "the swing of a close contract is 2 × value");
    }

    #[test]
    fn the_first_trick_matters_while_schwarz_threatens() {
        // Defenders without a trick, the declarers have made it: one trick avoids schwarz.
        let s = stakes(Some(1), 120, [0.0, 150.0], [0, 7], 30.0, 2);
        let (take, give) = s.trick(0.0, 0.0);
        assert!(take - give > 100.0, "{take} {give}");
    }
}
