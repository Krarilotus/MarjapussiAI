//! HP1's own bid policy as a likelihood (P3: an HP1 seat is read with the policy it acts with),
//! and the expected-game-points model (EGP, §2) that policy gates on. Moved from
//! `heuristic::bidding` and `heuristic::hand` (D-d).

use engine::{CardSet, SUITS};

use super::bids::{follow_target, message, BidContext, PAIR_LINE};
use super::classes::{len_in, pair_value, HandClass, Rng, N_CLASSES};
use super::Params;

/// Inputs of the expected-game-points estimate, all from one seat's view.
pub struct EgpInput {
    /// Cards our team certainly holds (my hand plus the partner's certain cards).
    pub team_known: CardSet,
    /// My own hand.
    pub mine: CardSet,
    /// P(the partner holds at least one ace outside `team_known`).
    pub partner_ace: f32,
    /// P(our team holds the pair), per suit.
    pub team_pair: [f32; 4],
}

/// The names of the EGP features, in the order of `egp_features` (for reports).
pub const EGP_FEATURES: [&str; 8] = [
    "1",
    "team aces",
    "team tens",
    "team halves",
    "my longest suit",
    "my void suits",
    "expected melds",
    "P(partner ace)",
];

/// The features of the EGP model: a constant, the team's known aces, tens and halves, my
/// longest suit and void suits, the expected meld value (best team pair plus a share of the
/// second), and P(the partner holds an ace).
pub fn egp_features(input: &EgpInput, params: &Params) -> [f32; 8] {
    let mut melds = SUITS.map(|s| input.team_pair[s as usize] * pair_value(s) as f32);
    melds.sort_by(|a, b| b.total_cmp(a));
    let longest = SUITS.iter().map(|&s| len_in(input.mine, s)).max().unwrap_or(0);
    let voids = SUITS.iter().filter(|&&s| len_in(input.mine, s) == 0).count();
    [
        1.0,
        (input.team_known & CardSet::ACES).len() as f32,
        (input.team_known & CardSet::TENS).len() as f32,
        (input.team_known & CardSet::HALVES).len() as f32,
        longest as f32,
        voids as f32,
        melds[0] + params.egp_second_pair_share * melds[1],
        input.partner_ace,
    ]
}

/// The EGP weights, in the order of `egp_features`.
pub fn egp_weights(params: &Params) -> [f32; 8] {
    [
        params.egp_base,
        params.egp_ace,
        params.egp_ten,
        params.egp_half,
        params.egp_long,
        params.egp_void,
        params.egp_meld,
        params.egp_partner_ace,
    ]
}

/// The points our team can expect to make as declarer: the design's linear estimate (§2; the
/// weights in `Params`).
pub fn expected_game_points(input: &EgpInput, params: &Params) -> f32 {
    egp_features(input, params)
        .iter()
        .zip(egp_weights(params))
        .map(|(f, w)| f * w)
        .sum()
}

/// The standard normal distribution function (Abramowitz and Stegun 7.1.26, |error| < 1.5e-7).
pub fn normal_cdf(x: f32) -> f32 {
    let z = (x as f64 / std::f64::consts::SQRT_2).abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * z);
    let poly =
        t * (0.254_829_592 + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let erf = 1.0 - poly * (-z * z).exp();
    (0.5 * (1.0 + if x < 0.0 { -erf } else { erf })) as f32
}

/// Per hand class, the mean and covariance of a lone hand's EGP inputs, over random deals:
/// [1, aces, tens, halves, longest suit, void suits, best own pair, second own pair,
/// P(partner ace) from the seat's own view at the start]. With the EGP weights this gives the
/// distribution of an HP1 seat's estimate when only its class is known (P2, P3).
struct ClassMoments {
    mean: Vec<[f32; 9]>,
    cov: Vec<[[f32; 9]; 9]>,
}

fn class_moments() -> &'static ClassMoments {
    static MOMENTS: std::sync::OnceLock<ClassMoments> = std::sync::OnceLock::new();
    MOMENTS.get_or_init(|| {
        const HANDS: usize = 60_000;
        // Shrink sparse classes towards all hands with this many pseudo-hands.
        const SHRINK: f64 = 5.0;
        // P(the partner holds ≥ 1 of the a aces I don't hold, among the 27 cards I don't hold).
        // C(27 − a, 9) / C(27, 9) = Π_{i<a} (18 − i) / (27 − i).
        let p_partner_ace = |a: u32| 1.0 - (0..a).map(|i| (18 - i) as f64 / (27 - i) as f64).product::<f64>();
        let mut rng = Rng::new(0x5EED_C1A5);
        let mut sum = vec![[0.0f64; 9]; N_CLASSES];
        let mut sq = vec![[[0.0f64; 9]; 9]; N_CLASSES];
        let mut n = vec![0.0f64; N_CLASSES];
        let (mut gsum, mut gsq) = ([0.0f64; 9], [[0.0f64; 9]; 9]);
        for _ in 0..HANDS {
            let mut deck: [u8; 36] = std::array::from_fn(|i| i as u8);
            for i in 0..9 {
                let j = i + (rng.next() % (36 - i) as u64) as usize;
                deck.swap(i, j);
            }
            let hand = deck[..9].iter().fold(CardSet::EMPTY, |a, &c| a | CardSet::single(c));
            let mut pairs: Vec<f64> = hand.pair_suits().map(|s| pair_value(s) as f64).collect();
            pairs.sort_by(|a, b| b.total_cmp(a));
            let lens = SUITS.map(|s| len_in(hand, s));
            let aces = (hand & CardSet::ACES).len();
            let x = [
                1.0,
                aces as f64,
                (hand & CardSet::TENS).len() as f64,
                (hand & CardSet::HALVES).len() as f64,
                *lens.iter().max().unwrap_or(&0) as f64,
                lens.iter().filter(|&&l| l == 0).count() as f64,
                pairs.first().copied().unwrap_or(0.0),
                pairs.get(1).copied().unwrap_or(0.0),
                p_partner_ace(4 - aces),
            ];
            let c = HandClass::of(hand).index();
            n[c] += 1.0;
            for i in 0..9 {
                sum[c][i] += x[i];
                gsum[i] += x[i];
                for j in 0..9 {
                    sq[c][i][j] += x[i] * x[j];
                    gsq[i][j] += x[i] * x[j];
                }
            }
        }
        let g = HANDS as f64;
        let mut mean = vec![[0.0f32; 9]; N_CLASSES];
        let mut cov = vec![[[0.0f32; 9]; 9]; N_CLASSES];
        for c in 0..N_CLASSES {
            let w = n[c] + SHRINK;
            let m: [f64; 9] = std::array::from_fn(|i| (sum[c][i] + SHRINK * gsum[i] / g) / w);
            for i in 0..9 {
                mean[c][i] = m[i] as f32;
                for j in 0..9 {
                    let e = (sq[c][i][j] + SHRINK * gsq[i][j] / g) / w;
                    cov[c][i][j] = (e - m[i] * m[j]) as f32;
                }
            }
        }
        ClassMoments { mean, cov }
    })
}

/// Mean and spread of an HP1 seat's EGP when only its hand class is known (P2, P3).
fn class_egp(class: HandClass, p: &Params) -> (f32, f32) {
    let m = class_moments();
    let w = [
        p.egp_base,
        p.egp_ace,
        p.egp_ten,
        p.egp_half,
        p.egp_long,
        p.egp_void,
        p.egp_meld,
        p.egp_meld * p.egp_second_pair_share,
        p.egp_partner_ace,
    ];
    let c = class.index();
    let mean: f32 = (0..9).map(|i| w[i] * m.mean[c][i]).sum();
    let var: f32 = (0..9)
        .map(|i| (0..9).map(|j| w[i] * w[j] * m.cov[c][i][j]).sum::<f32>())
        .sum();
    (mean, var.max(1.0).sqrt())
}

/// `P(bid | class, ctx)` for a seat that plays HP1: the message of its class at its shifted
/// target (HB-1…5, HB-140a) if its EGP clears HB-7 and, above 140, it holds a pair (HB-140b),
/// or, up to 140, by HB-9's information bid (its EGP within `hp1_info_margin` of the target);
/// with nothing to tell, +5 when its EGP clears the next bid by
/// `take_margin` (HB-8); else a pass.
pub fn bid_prob(bid: Option<i32>, class: HandClass, ctx: &BidContext, p: &Params) -> f32 {
    let (mu, sd) = class_egp(class, p);
    let clears = |need: f32| 1.0 - normal_cdf((need - mu) / sd);
    let (target, p_bid) = match message(class, ctx, p) {
        Some(m) => {
            let t = follow_target(m, ctx, p);
            let gate = clears(t as f32 - p.bid_overreach);
            let send = if t <= PAIR_LINE {
                gate.max(clears(t as f32 - p.hp1_info_margin))
            } else if class.any_pair() {
                gate
            } else {
                0.0
            };
            (t, send)
        }
        None => {
            let next = next_bid(ctx.value);
            let ok = next <= PAIR_LINE || class.any_pair();
            (next, if ok { clears(next as f32 + p.take_margin) } else { 0.0 })
        }
    };
    match bid {
        None => 1.0 - p_bid,
        Some(b) if b == target => p_bid,
        Some(_) => 0.0,
    }
}

/// HB-8's plain +5 step, which skips 140 itself (HB-140a).
pub fn next_bid(value: i32) -> i32 {
    if value + 5 == PAIR_LINE {
        PAIR_LINE + 5
    } else {
        value + 5
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conventions::next_context;

    #[test]
    fn hp1_bid_model_tells_a_red_pair_and_passes_an_empty_hand() {
        let p = Params::default();
        let ctx = next_context(&[], 0, &p);
        let red = HandClass {
            ace: false,
            halves: [0, 0, 0, 2],
        };
        let empty = HandClass {
            ace: false,
            halves: [0; 4],
        };
        assert!(bid_prob(Some(135), red, &ctx, &p) > 0.9);
        assert!(bid_prob(None, empty, &ctx, &p) > 0.95);
        // A Rot pair goes before the ace (C43); with only a Grün pair the ace first.
        let red_ace = HandClass { ace: true, ..red };
        assert!(bid_prob(Some(135), red_ace, &ctx, &p) > 0.9);
        let green_ace = HandClass {
            ace: true,
            halves: [2, 0, 0, 0],
        };
        // (Its only bid is the ace's 120; how often it bids is the EGP gate's.)
        assert!(bid_prob(Some(120), green_ace, &ctx, &p) > 0.0);
        assert_eq!(bid_prob(Some(125), green_ace, &ctx, &p), 0.0);
        let total: f32 = [None, Some(120), Some(135)]
            .iter()
            .map(|&b| bid_prob(b, red, &ctx, &p))
            .sum();
        assert!((total - 1.0).abs() < 1e-5);
    }

    #[test]
    fn normal_cdf_matches_known_values() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-6);
        assert!((normal_cdf(1.96) - 0.975).abs() < 1e-4);
        assert!((normal_cdf(-1.0) - 0.158_655).abs() < 1e-4);
    }
}
