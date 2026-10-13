//! Temperature calibration of answer probabilities.
//!
//! Decision models ship over-confident (the multilingual Laya checkpoint ships
//! with no fitted temperature at all). One temperature per (question type,
//! option count) fixes most of that: probabilities are re-tempered as
//! `pᵢ^(1/T) / Σⱼ pⱼ^(1/T)`, which is the same as dividing the logits by `T`.
//! `T > 1` softens, `T < 1` sharpens, `T = 1` leaves them alone. The argmax
//! never changes, only how sure the answer claims to be.
//!
//! Today the temperatures come from configuration
//! (`CAMPBOOKS_AI__DECIDE__CALIBRATION__<TYPE>[_<OPTIONS>]`, default 1.0).
//! Once user corrections are stored as labelled examples (ADR 0009), a refit
//! job will compute them per question from those corrections.

use std::collections::HashMap;

use serde::Deserialize;

use crate::{Error, decide::QuestionType};

/// Temperatures per question type, optionally per option count.
///
/// Keys are `<type>` (every question of that type) or `<type>_<options>`
/// (only questions with that many options, which wins), for `<type>` in
/// `choice`, `score`, `noul`: `choice=1.4`, `choice_2=1.9`, `noul=2.0`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(try_from = "HashMap<String, f64>")]
pub struct Calibration {
    by_type: HashMap<QuestionType, f64>,
    by_options: HashMap<(QuestionType, usize), f64>,
}

impl Calibration {
    /// Build from `(key, temperature)` pairs, as read from configuration.
    pub fn from_pairs<K: AsRef<str>>(
        pairs: impl IntoIterator<Item = (K, f64)>,
    ) -> Result<Self, Error> {
        let mut calibration = Self::default();
        for (key, t) in pairs {
            let key = key.as_ref();
            if !(t.is_finite() && t > 0.0) {
                return Err(Error::Config(format!(
                    "calibration temperature `{key}` must be a positive number, got {t}"
                )));
            }
            let (name, options) = match key.split_once('_') {
                Some((name, count)) => {
                    let count = count.parse::<usize>().map_err(|_| bad_key(key))?;
                    (name, Some(count))
                }
                None => (key, None),
            };
            let qtype = match name {
                "choice" => QuestionType::Choice,
                "score" => QuestionType::Score,
                "noul" => QuestionType::Noul,
                _ => return Err(bad_key(key)),
            };
            match options {
                Some(count) => calibration.by_options.insert((qtype, count), t),
                None => calibration.by_type.insert(qtype, t),
            };
        }
        Ok(calibration)
    }

    /// The temperature for a question of `qtype` with `options` options:
    /// the per-count value, else the per-type value, else 1.0.
    pub fn temperature(&self, qtype: QuestionType, options: usize) -> f64 {
        self.by_options
            .get(&(qtype, options))
            .or_else(|| self.by_type.get(&qtype))
            .copied()
            .unwrap_or(1.0)
    }

    /// Whether no temperature is configured (every temperature is 1.0).
    pub fn is_empty(&self) -> bool {
        self.by_type.is_empty() && self.by_options.is_empty()
    }
}

impl TryFrom<HashMap<String, f64>> for Calibration {
    type Error = Error;

    fn try_from(map: HashMap<String, f64>) -> Result<Self, Error> {
        Self::from_pairs(map)
    }
}

fn bad_key(key: &str) -> Error {
    Error::Config(format!(
        "unknown calibration key `{key}`; use choice, score or noul, optionally followed by \
         _<option count> (e.g. choice_3)"
    ))
}

/// Re-temper a probability distribution: `pᵢ^(1/T)`, renormalised.
///
/// Zero stays zero. A distribution with no mass, a non-positive or non-finite
/// `t`, or `t == 1` is returned unchanged (renormalised).
pub fn temper(probabilities: &[f64], t: f64) -> Vec<f64> {
    let total: f64 = probabilities.iter().sum();
    if !(total.is_finite() && total > 0.0) {
        return probabilities.to_vec();
    }
    if !(t.is_finite() && t > 0.0) || (t - 1.0).abs() < f64::EPSILON {
        return probabilities.iter().map(|p| p / total).collect();
    }
    // Work in log space so tiny probabilities do not underflow before scaling.
    let logs: Vec<f64> = probabilities
        .iter()
        .map(|p| {
            if *p > 0.0 {
                p.ln() / t
            } else {
                f64::NEG_INFINITY
            }
        })
        .collect();
    let max = logs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let exps: Vec<f64> = logs.iter().map(|l| (l - max).exp()).collect();
    let sum: f64 = exps.iter().sum();
    exps.into_iter().map(|e| e / sum).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &[f64], b: &[f64]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
    }

    #[test]
    fn identity_at_one() {
        let p = [0.7, 0.2, 0.1];
        assert!(close(&temper(&p, 1.0), &p));
    }

    #[test]
    fn matches_logit_scaling() {
        // softmax(z / T) from the logits behind p = softmax(z).
        let z: [f64; 3] = [2.0, 1.0, -0.5];
        let softmax = |zs: &[f64]| {
            let m = zs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let e: Vec<f64> = zs.iter().map(|x| (x - m).exp()).collect();
            let s: f64 = e.iter().sum();
            e.into_iter().map(|x| x / s).collect::<Vec<_>>()
        };
        let p = softmax(&z);
        for t in [0.5, 1.7, 3.0] {
            let scaled: Vec<f64> = z.iter().map(|x| x / t).collect();
            assert!(close(&temper(&p, t), &softmax(&scaled)), "T = {t}");
        }
    }

    #[test]
    fn softens_and_sharpens_without_moving_the_argmax() {
        let p = [0.9, 0.1];
        let soft = temper(&p, 2.0);
        let sharp = temper(&p, 0.5);
        assert!(soft[0] < 0.9 && soft[0] > 0.5);
        assert!(sharp[0] > 0.9);
        // 0.9^0.5 / (0.9^0.5 + 0.1^0.5) = 0.75
        assert!((soft[0] - 0.75).abs() < 1e-12);
    }

    #[test]
    fn keeps_zeros_and_degenerate_input() {
        assert!(close(&temper(&[0.0, 1.0], 2.0), &[0.0, 1.0]));
        assert!(close(&temper(&[0.0, 0.0], 2.0), &[0.0, 0.0]));
        assert!(close(
            &temper(&[0.5, 0.25, 0.25], f64::NAN),
            &[0.5, 0.25, 0.25]
        ));
        // Renormalises input that does not sum to one (rounded provider output).
        assert!(close(
            &temper(&[0.5, 0.49], 1.0),
            &[0.5 / 0.99, 0.49 / 0.99]
        ));
    }

    #[test]
    fn lookup_prefers_option_count_then_type_then_one() {
        let c =
            Calibration::from_pairs([("choice", 1.5), ("choice_2", 1.9), ("noul", 2.0)]).unwrap();
        assert!((c.temperature(QuestionType::Choice, 2) - 1.9).abs() < f64::EPSILON);
        assert!((c.temperature(QuestionType::Choice, 7) - 1.5).abs() < f64::EPSILON);
        assert!((c.temperature(QuestionType::Noul, 2) - 2.0).abs() < f64::EPSILON);
        assert!((c.temperature(QuestionType::Score, 5) - 1.0).abs() < f64::EPSILON);
        assert!(Calibration::default().is_empty());
        assert!(!c.is_empty());
    }

    #[test]
    fn rejects_bad_keys_and_temperatures() {
        for key in ["vibes", "choice_x", "choice_", "_3"] {
            let err = Calibration::from_pairs([(key, 1.0)])
                .unwrap_err()
                .to_string();
            assert!(err.contains("unknown calibration key"), "{key}: {err}");
        }
        for t in [0.0, -1.0, f64::INFINITY, f64::NAN] {
            assert!(Calibration::from_pairs([("noul", t)]).is_err(), "{t}");
        }
    }
}
