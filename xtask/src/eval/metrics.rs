//! Metrics over predictions: pure functions, checked by hand-computed cases.

use campbooks_ai::decide::temper;

/// One answered prediction: the option the strategy chose, its probabilities
/// and the true option.
#[derive(Debug, Clone, PartialEq)]
pub struct Scored {
    pub chosen: usize,
    pub probabilities: Vec<f64>,
    pub confidence: f64,
    pub truth: usize,
}

impl Scored {
    pub fn correct(&self) -> bool {
        self.chosen == self.truth
    }
}

/// Counts are small; the conversion is exact.
#[allow(clippy::cast_precision_loss)]
pub fn f(n: usize) -> f64 {
    n as f64
}

pub fn ratio(a: usize, b: usize) -> Option<f64> {
    (b > 0).then(|| f(a) / f(b))
}

/// Precision and coverage of the answers at or above `threshold`, over
/// `total` items (abstentions included in `total`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtThreshold {
    pub threshold: f64,
    pub covered: usize,
    pub correct: usize,
    pub total: usize,
}

impl AtThreshold {
    pub fn precision(&self) -> Option<f64> {
        ratio(self.correct, self.covered)
    }
    pub fn coverage(&self) -> Option<f64> {
        ratio(self.covered, self.total)
    }
}

pub fn at_threshold(scored: &[Scored], total: usize, threshold: f64) -> AtThreshold {
    let covered: Vec<&Scored> = scored
        .iter()
        .filter(|s| s.confidence >= threshold)
        .collect();
    AtThreshold {
        threshold,
        covered: covered.len(),
        correct: covered.iter().filter(|s| s.correct()).count(),
        total,
    }
}

/// The lowest threshold whose precision reaches `target` with at least
/// `min_support` answers covered, or `None` when no threshold does.
pub fn operating_point(scored: &[Scored], target: f64, min_support: usize) -> Option<f64> {
    let mut candidates: Vec<f64> = scored.iter().map(|s| s.confidence).collect();
    candidates.sort_by(f64::total_cmp);
    candidates.dedup();
    candidates.into_iter().find(|&t| {
        let at = at_threshold(scored, scored.len(), t);
        at.covered >= min_support && at.precision().is_some_and(|p| p >= target)
    })
}

/// 95 % Wilson score interval for `successes` out of `n`.
pub fn wilson(successes: usize, n: usize) -> Option<(f64, f64)> {
    if n == 0 {
        return None;
    }
    let z = 1.959_964;
    let n = f(n);
    let p = f(successes) / n;
    let denom = 1.0 + z * z / n;
    let centre = (p + z * z / (2.0 * n)) / denom;
    let half = z * ((p * (1.0 - p) / n) + z * z / (4.0 * n * n)).sqrt() / denom;
    Some(((centre - half).max(0.0), (centre + half).min(1.0)))
}

/// Expected calibration error over 10 equal-width confidence bins.
pub fn ece(scored: &[Scored]) -> Option<f64> {
    if scored.is_empty() {
        return None;
    }
    let mut bins = [(0.0_f64, 0.0_f64, 0_usize); 10];
    for s in scored {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // confidence is in [0, 1]: the bin is 0..=9
        let bin = ((s.confidence * 10.0).floor() as usize).min(9);
        bins[bin].0 += s.confidence;
        bins[bin].1 += if s.correct() { 1.0 } else { 0.0 };
        bins[bin].2 += 1;
    }
    let n = f(scored.len());
    Some(
        bins.iter()
            .filter(|b| b.2 > 0)
            .map(|(conf, acc, count)| (f(*count) / n) * ((conf - acc) / f(*count)).abs())
            .sum(),
    )
}

/// Multi-class Brier score (sum of squared errors over options, averaged).
pub fn brier(scored: &[Scored]) -> Option<f64> {
    if scored.is_empty() {
        return None;
    }
    let total: f64 = scored
        .iter()
        .map(|s| {
            s.probabilities
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let y = if i == s.truth { 1.0 } else { 0.0 };
                    (p - y).powi(2)
                })
                .sum::<f64>()
        })
        .sum();
    Some(total / f(scored.len()))
}

/// The same predictions with their probabilities re-tempered by `t` (as
/// production calibration does).
pub fn tempered(scored: &[Scored], t: f64) -> Vec<Scored> {
    scored
        .iter()
        .map(|s| {
            let probabilities = temper(&s.probabilities, t);
            let confidence = probabilities.get(s.chosen).copied().unwrap_or(s.confidence);
            Scored {
                probabilities,
                confidence,
                ..s.clone()
            }
        })
        .collect()
}

fn nll(scored: &[Scored]) -> f64 {
    scored
        .iter()
        .map(|s| {
            -s.probabilities
                .get(s.truth)
                .copied()
                .unwrap_or(0.0)
                .max(1e-9)
                .ln()
        })
        .sum()
}

/// The temperature in [0.05, 20] (log grid) that minimises the negative
/// log-likelihood of the true options.
pub fn fit_temperature(scored: &[Scored]) -> Option<f64> {
    if scored.is_empty() {
        return None;
    }
    let steps = 240;
    (0..=steps)
        .map(|i| {
            let x = f(i) / f(steps);
            (0.05_f64.ln() + x * (20.0_f64.ln() - 0.05_f64.ln())).exp()
        })
        .map(|t| (t, nll(&tempered(scored, t))))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(t, _)| t)
}

/// Macro-averaged F1 over `options` classes (answered predictions only).
pub fn macro_f1(scored: &[Scored], options: usize) -> Option<f64> {
    if scored.is_empty() {
        return None;
    }
    let mut sum = 0.0;
    let mut classes = 0;
    for c in 0..options {
        let tp = scored
            .iter()
            .filter(|s| s.chosen == c && s.truth == c)
            .count();
        let fp = scored
            .iter()
            .filter(|s| s.chosen == c && s.truth != c)
            .count();
        let fn_ = scored
            .iter()
            .filter(|s| s.chosen != c && s.truth == c)
            .count();
        if tp + fp + fn_ == 0 {
            continue;
        }
        classes += 1;
        sum += 2.0 * f(tp) / f(2 * tp + fp + fn_);
    }
    (classes > 0).then(|| sum / f(classes))
}

/// The value at quantile `q` (nearest rank) of `values`.
pub fn quantile(values: &[f64], q: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // q in [0, 1] and len small: the rank fits
    let rank = ((q * f(v.len())).ceil() as usize).clamp(1, v.len());
    Some(v[rank - 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(chosen: usize, confidence: f64, truth: usize) -> Scored {
        let mut probabilities = vec![1.0 - confidence; 2];
        probabilities[chosen] = confidence;
        Scored {
            chosen,
            probabilities,
            confidence,
            truth,
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn threshold_precision_and_coverage() {
        let v = [s(1, 0.9, 1), s(1, 0.8, 0), s(0, 0.6, 0)];
        let at = at_threshold(&v, 4, 0.7);
        assert_eq!((at.covered, at.correct), (2, 1));
        assert_eq!(at.precision(), Some(0.5));
        assert_eq!(at.coverage(), Some(0.5));
    }

    #[test]
    fn operating_point_is_the_lowest_threshold_reaching_the_target() {
        let v = [s(1, 0.95, 1), s(1, 0.9, 1), s(1, 0.8, 0), s(0, 0.7, 0)];
        assert_eq!(operating_point(&v, 0.99, 1), Some(0.9));
        assert_eq!(operating_point(&v, 0.99, 3), None, "not enough support");
        assert_eq!(operating_point(&v, 0.7, 1), Some(0.7));
    }

    #[test]
    fn wilson_interval() {
        let (lo, hi) = wilson(8, 10).unwrap();
        assert!(
            (lo - 0.490).abs() < 0.002 && (hi - 0.943).abs() < 0.002,
            "{lo} {hi}"
        );
        assert_eq!(wilson(0, 0), None);
    }

    #[test]
    fn calibration_error_and_brier() {
        // All at 0.9 confidence, half right: ECE = |0.9 - 0.5| = 0.4.
        let v = [s(1, 0.9, 1), s(1, 0.9, 0)];
        assert!(close(ece(&v).unwrap(), 0.4));
        // Brier: right → 2·0.01 = 0.02, wrong → 0.81 + 0.81 = 1.62; mean 0.82.
        assert!(close(brier(&v).unwrap(), 0.82));
    }

    #[test]
    fn temperature_fit_softens_overconfidence() {
        let v: Vec<Scored> = (0..10)
            .map(|i| s(1, 0.99, usize::from(i % 2 == 0)))
            .collect();
        let t = fit_temperature(&v).unwrap();
        assert!(
            t > 5.0,
            "half right at 0.99 needs a high temperature, got {t}"
        );
        assert!(ece(&tempered(&v, t)).unwrap() < ece(&v).unwrap());
    }

    #[test]
    fn macro_f1_and_quantiles() {
        let v = [s(0, 0.9, 0), s(1, 0.9, 1), s(1, 0.9, 0)];
        // class 0: tp1 fn1 → 2/3; class 1: tp1 fp1 → 2/3.
        assert!(close(macro_f1(&v, 2).unwrap(), 2.0 / 3.0));
        assert_eq!(quantile(&[3.0, 1.0, 2.0, 4.0], 0.5), Some(2.0));
        assert_eq!(quantile(&[3.0, 1.0, 2.0, 4.0], 0.95), Some(4.0));
    }
}
