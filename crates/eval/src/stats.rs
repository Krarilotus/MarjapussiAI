//! Small statistics for reports: mean with a normal 95 % interval, Wilson intervals, and an
//! anytime-valid confidence sequence for sequential tests.

/// Mean and the half-width of its 95 % normal interval.
pub fn mean_ci(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    if n < 2.0 {
        return (xs.first().copied().unwrap_or(0.0), f64::NAN);
    }
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    (mean, 1.96 * (var / n).sqrt())
}

/// The 95 % Wilson interval of `k` successes out of `n`.
pub fn wilson(k: u64, n: u64) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let (k, n, z) = (k as f64, n as f64, 1.96f64);
    let p = k / n;
    let denom = 1.0 + z * z / n;
    let centre = (p + z * z / (2.0 * n)) / denom;
    let half = z * ((p * (1.0 - p) + z * z / (4.0 * n)) / n).sqrt() / denom;
    ((centre - half).max(0.0), (centre + half).min(1.0))
}

/// "k/n = p % [lo, hi]".
pub fn rate(k: u64, n: u64) -> String {
    let (lo, hi) = wilson(k, n);
    let p = if n > 0 { k as f64 / n as f64 } else { 0.0 };
    format!("{k}/{n} = {:.1} % [{:.1}, {:.1}]", 100.0 * p, 100.0 * lo, 100.0 * hi)
}

/// Mean and the half-width of a two-sided confidence sequence at level `alpha`: valid at every
/// sample size at once, so a run may stop as soon as it excludes 0 (C15 "sequential test").
///
/// Normal-mixture boundary (Robbins 1970; Howard et al. 2021, "Time-uniform, nonparametric,
/// nonasymptotic confidence sequences", eq. 14): with `v = n σ²` and `ρ = σ² n_opt`,
/// `|S_n − n μ| < sqrt((v + ρ) ln((v + ρ) / (ρ α²)))`. The interval is tightest near `n_opt`.
/// σ² is the sample variance (a plug-in, so the guarantee is approximate for small n; runs use
/// at least 100 deal pairs before they may stop).
pub fn conf_seq(xs: &[f64], alpha: f64, n_opt: f64) -> (f64, f64) {
    let n = xs.len() as f64;
    if n < 2.0 {
        return (xs.first().copied().unwrap_or(0.0), f64::INFINITY);
    }
    let mean = xs.iter().sum::<f64>() / n;
    let var = (xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)).max(1e-9);
    let (v, rho) = (n * var, n_opt * var);
    let bound = ((v + rho) * ((v + rho) / (rho * alpha * alpha)).ln()).sqrt();
    (mean, bound / n)
}

/// Least squares `y ≈ w · x` by the normal equations (Gaussian elimination with partial pivoting).
#[allow(clippy::needless_range_loop)] // matrix code reads best with indices
pub fn ols<const K: usize>(xs: &[[f32; K]], ys: &[f64]) -> [f64; K] {
    let mut a = vec![vec![0.0f64; K + 1]; K];
    for (x, &y) in xs.iter().zip(ys) {
        for i in 0..K {
            for j in 0..K {
                a[i][j] += x[i] as f64 * x[j] as f64;
            }
            a[i][K] += x[i] as f64 * y;
        }
    }
    for i in 0..K {
        a[i][i] += 1e-6; // a tiny ridge keeps constant columns solvable
    }
    for c in 0..K {
        let p = (c..K)
            .max_by(|&r, &s| a[r][c].abs().total_cmp(&a[s][c].abs()))
            .unwrap_or(c);
        a.swap(c, p);
        for r in 0..K {
            if r != c && a[c][c] != 0.0 {
                let f = a[r][c] / a[c][c];
                for j in c..=K {
                    a[r][j] -= f * a[c][j];
                }
            }
        }
    }
    std::array::from_fn(|i| if a[i][i] != 0.0 { a[i][K] / a[i][i] } else { 0.0 })
}

/// The linear prediction `w · x`.
pub fn predict<const K: usize>(w: &[f64; K], x: &[f32; K]) -> f64 {
    w.iter().zip(x).map(|(w, &x)| w * x as f64).sum()
}

/// Mean absolute error.
pub fn mae(pred: impl Iterator<Item = f64>, ys: &[f64]) -> f64 {
    pred.zip(ys).map(|(p, y)| (p - y).abs()).sum::<f64>() / ys.len().max(1) as f64
}

/// A markdown table: predicted value buckets against the realised mean with its 95 % interval.
pub fn calibration(pred: &[f64], ys: &[f64]) -> String {
    // Point buckets: below 100, four of 15 points from 100, then 165 and more.
    let step = |i: u8| 100.0 + 15.0 * i as f64;
    let edges = [f64::MIN, step(0), step(1), step(2), step(3), 165.0, f64::MAX];
    let names = ["< 100", "100–114", "115–129", "130–144", "145–164", "≥ 165"];
    let mut out = String::from("| predicted | n | mean predicted | realised (95 % CI) |\n|---|---|---|---|\n");
    for b in 0..6 {
        let idx: Vec<usize> = (0..pred.len())
            .filter(|&i| pred[i] >= edges[b] && pred[i] < edges[b + 1])
            .collect();
        if idx.is_empty() {
            continue;
        }
        let p: Vec<f64> = idx.iter().map(|&i| pred[i]).collect();
        let y: Vec<f64> = idx.iter().map(|&i| ys[i]).collect();
        let (m, ci) = mean_ci(&y);
        out += &format!(
            "| {} | {} | {:.0} | {m:.0} ± {ci:.0} |\n",
            names[b],
            idx.len(),
            p.iter().sum::<f64>() / p.len() as f64
        );
    }
    out
}

/// Out-of-fold predictions of `ols` with `folds`-fold cross-validation (row i in fold i mod folds).
pub fn ols_cv<const K: usize>(xs: &[[f32; K]], ys: &[f64], folds: usize) -> Vec<f64> {
    let mut cv = vec![0.0; ys.len()];
    for fold in 0..folds {
        let (train_x, train_y): (Vec<[f32; K]>, Vec<f64>) = (0..ys.len())
            .filter(|i| i % folds != fold)
            .map(|i| (xs[i], ys[i]))
            .unzip();
        let w = ols(&train_x, &train_y);
        for i in (0..ys.len()).filter(|i| i % folds == fold) {
            cv[i] = predict(&w, &xs[i]);
        }
    }
    cv
}

/// Reliability of probability forecasts against what happened: 10 bins of the forecast, the
/// Brier score and the expected calibration error (ECE).
#[derive(Clone, Debug, Default)]
pub struct Reliability {
    /// Per bin: (count, sum of forecasts, sum of outcomes).
    bins: [(u64, f64, f64); 10],
    brier: f64,
    n: u64,
}

impl Reliability {
    pub fn add(&mut self, p: f64, happened: bool) {
        let y = happened as u8 as f64;
        let b = &mut self.bins[((p * 10.0) as usize).min(9)];
        b.0 += 1;
        b.1 += p;
        b.2 += y;
        self.brier += (p - y).powi(2);
        self.n += 1;
    }

    pub fn n(&self) -> u64 {
        self.n
    }

    pub fn brier(&self) -> f64 {
        self.brier / self.n.max(1) as f64
    }

    /// Σ over bins of |mean forecast − observed frequency| × the bin's share.
    pub fn ece(&self) -> f64 {
        self.bins
            .iter()
            .filter(|b| b.0 > 0)
            .map(|b| (b.1 - b.2).abs())
            .sum::<f64>()
            / self.n.max(1) as f64
    }

    /// Markdown rows `| bin | n | mean forecast | observed |`.
    pub fn table(&self) -> String {
        let mut out = String::from("| forecast | n | mean forecast | observed |\n|---|---|---|---|\n");
        for (i, b) in self.bins.iter().enumerate().filter(|(_, b)| b.0 > 0) {
            let n = b.0 as f64;
            out += &format!(
                "| {:.1}–{:.1} | {} | {:.3} | {:.3} |\n",
                i as f64 / 10.0,
                (i + 1) as f64 / 10.0,
                b.0,
                b.1 / n,
                b.2 / n
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reliability_of_a_calibrated_forecast() {
        let mut r = Reliability::default();
        for i in 0..1000 {
            r.add(0.25, i % 4 == 0);
        }
        assert!((r.brier() - 0.1875).abs() < 1e-9);
        assert!(r.ece() < 1e-9);
        assert_eq!(r.n(), 1000);
    }

    #[test]
    fn the_confidence_sequence_is_wider_than_the_fixed_interval_and_shrinks() {
        let xs: Vec<f64> = (0..2000).map(|i| ((i * 7919) % 101) as f64 - 50.0).collect();
        let (_, fixed) = mean_ci(&xs[..500]);
        let (_, seq500) = conf_seq(&xs[..500], 0.05, 500.0);
        let (_, seq2000) = conf_seq(&xs, 0.05, 500.0);
        assert!(seq500 > fixed && seq500 < 2.5 * fixed);
        assert!(seq2000 < seq500);
    }

    #[test]
    fn ols_recovers_a_linear_model() {
        let xs: Vec<[f32; 3]> = (0..50).map(|i| [1.0, i as f32, (i * i % 7) as f32]).collect();
        let ys: Vec<f64> = xs.iter().map(|x| 3.0 + 2.0 * x[1] as f64 - x[2] as f64).collect();
        let w = ols(&xs, &ys);
        assert!((w[0] - 3.0).abs() < 1e-3 && (w[1] - 2.0).abs() < 1e-3 && (w[2] + 1.0).abs() < 1e-3);
        assert!(mae(ols_cv(&xs, &ys, 5).into_iter(), &ys) < 1e-3);
    }

    #[test]
    fn wilson_matches_a_known_value() {
        let (lo, hi) = wilson(50, 100);
        assert!((lo - 0.4038).abs() < 1e-3 && (hi - 0.5962).abs() < 1e-3);
    }
}
