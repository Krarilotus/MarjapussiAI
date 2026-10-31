//! Golden complete evaluations captured on afe2ebb before search optimisations.
//! Hash every criterion, move and mean (including metadata) over 300 fixed positions.
#[path = "support/positions.rs"]
mod positions;

use knowledge::Uniform;
use rand::{rngs::StdRng, SeedableRng};
use solver::{evaluate, Objective, Solver};

#[test]
fn fixed_evaluation_means() {
    let mut solver = Solver::with_tt_bits(16);
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for k in [3, 4, 5] {
        for (i, (know, table, _, _)) in positions::decisions(k, 100).iter().enumerate() {
            solver.clear();
            let team = i % 2;
            let objective = Objective::Contract {
                playing_team: team as u8,
                game_value: (table.card_points[team] + table.pair_points[team]) as i32 + 30,
            };
            let d = evaluate(
                &mut solver,
                know,
                table,
                objective,
                64,
                &Uniform,
                &mut StdRng::seed_from_u64(64),
            )
            .unwrap();
            assert!(d.complete);
            for byte in format!("{d:?}").bytes() {
                hash = (hash ^ byte as u64).wrapping_mul(0x100_0000_01b3);
            }
        }
    }
    assert_eq!(hash, 0x3e5d_d952_31f4_c68d, "complete evaluation means changed");
}
