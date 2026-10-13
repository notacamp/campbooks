//! From runs to numbers: the JSON report, its markdown rendering and the
//! baseline check.

use std::{collections::BTreeMap, fmt::Write as _};

use campbooks_ai::decide::{AnswerValue, Question, QuestionKind};
use serde::{Deserialize, Serialize};

use super::{
    dataset::{Dataset, Split, truth_index},
    metrics::{
        Scored, at_threshold, brier, ece, f, fit_temperature, macro_f1, operating_point, quantile,
        ratio, tempered, wilson,
    },
    run::{Run, is_offline},
};

/// Fewest `dev` answers an operating point must cover.
pub const MIN_SUPPORT: usize = 10;
/// Thresholds listed in the JSON report.
const GRID: [f64; 7] = [0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 0.99];
/// How far an offline strategy may fall below the baseline before `--check`
/// fails.
pub const TOLERANCE: f64 = 0.02;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub dataset: String,
    pub version: u32,
    pub items: usize,
    pub generated: String,
    pub commit: String,
    pub not_run: Vec<NotRun>,
    pub runs: Vec<RunReport>,
    #[serde(default)]
    pub learning_curve: Vec<CurvePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotRun {
    pub strategy: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReport {
    pub name: String,
    pub offline: bool,
    pub errors: usize,
    pub latency_p50_ms: Option<f64>,
    pub latency_p95_ms: Option<f64>,
    pub remote_calls_per_item: Option<f64>,
    #[serde(default)]
    pub stage_thresholds: Vec<(String, BTreeMap<String, f64>)>,
    pub questions: Vec<QuestionReport>,
    /// Hard-case tag → `(test items, accuracy over their answered questions)`.
    #[serde(default)]
    pub hard_cases: BTreeMap<String, (usize, Option<f64>)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionReport {
    pub id: String,
    pub target_precision: f64,
    /// Operating point found on `dev`; `None`: no threshold reaches the target.
    pub threshold: Option<f64>,
    /// Temperature fitted on `dev`.
    pub temperature: Option<f64>,
    /// `all` and each locale, on `test`.
    pub groups: BTreeMap<String, GroupReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupReport {
    pub n: usize,
    pub answered: usize,
    pub accuracy: Option<f64>,
    pub at_threshold: Option<AtOp>,
    pub macro_f1: Option<f64>,
    pub mae: Option<f64>,
    pub within_one: Option<f64>,
    pub ece_raw: Option<f64>,
    pub ece_fitted: Option<f64>,
    pub brier: Option<f64>,
    /// `(threshold, precision, coverage)`; for `all` only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grid: Vec<(f64, Option<f64>, Option<f64>)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtOp {
    pub precision: Option<f64>,
    pub precision_low: Option<f64>,
    pub precision_high: Option<f64>,
    pub coverage: Option<f64>,
    pub covered: usize,
    /// `noul` only: share of the true cases answered "true" at the threshold.
    pub positive_recall: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurvePoint {
    pub corrections: String,
    pub question: String,
    pub answered: Option<f64>,
    pub accuracy: Option<f64>,
}

/// The option an answer chose.
fn chosen(question: &Question, value: &AnswerValue, probabilities: Option<&Vec<f64>>) -> usize {
    if let Some(p) = probabilities {
        return p
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map_or(0, |(i, _)| i);
    }
    match (value, &question.kind) {
        (AnswerValue::Choice(label), QuestionKind::Choice { options, .. }) => options
            .iter()
            .position(|o| &o.label == label)
            .unwrap_or_default(),
        (AnswerValue::Noul(holds), _) => usize::from(*holds),
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // a level in 0..20
        (AnswerValue::Score(level), _) => level.round().max(0.0) as usize,
        _ => 0,
    }
}

/// Per item of `split` (and `lang`, if given): the locale and the scored
/// answer, `None` when the strategy abstained.
fn scored(
    dataset: &Dataset,
    run: &Run,
    qi: usize,
    question: &Question,
    split: Split,
) -> Vec<(String, usize, Option<Scored>)> {
    dataset
        .items
        .iter()
        .zip(&run.items)
        .filter(|(item, r)| item.split == split && r.is_some())
        .filter_map(|(item, r)| {
            let truth = truth_index(question, &item.labels)?;
            let answer = r.as_ref()?.answers.get(qi)?.as_ref().map(|d| {
                let a = &d.answer;
                Scored {
                    chosen: chosen(question, &a.value, a.probabilities.as_ref()),
                    probabilities: a.probabilities.clone().unwrap_or_default(),
                    // No probability: never passes a threshold.
                    confidence: a.confidence.unwrap_or(-1.0),
                    truth,
                }
            });
            Some((item.lang.clone(), truth, answer))
        })
        .collect()
}

fn with_probabilities(scored: &[Scored]) -> Vec<Scored> {
    scored
        .iter()
        .filter(|s| !s.probabilities.is_empty())
        .cloned()
        .collect()
}

fn group(
    rows: &[(String, usize, Option<Scored>)],
    question: &Question,
    threshold: Option<f64>,
    temperature: Option<f64>,
) -> GroupReport {
    let answered: Vec<Scored> = rows.iter().filter_map(|r| r.2.clone()).collect();
    let n = rows.len();
    let probs = with_probabilities(&answered);
    let at_op = threshold.map(|t| {
        let at = at_threshold(&answered, n, t);
        let interval = wilson(at.correct, at.covered);
        let positive_recall = matches!(question.kind, QuestionKind::Noul { .. })
            .then(|| {
                let positives = rows.iter().filter(|r| r.1 == 1).count();
                let found = answered
                    .iter()
                    .filter(|s| s.truth == 1 && s.chosen == 1 && s.confidence >= t)
                    .count();
                ratio(found, positives)
            })
            .flatten();
        AtOp {
            precision: at.precision(),
            precision_low: interval.map(|i| i.0),
            precision_high: interval.map(|i| i.1),
            coverage: at.coverage(),
            covered: at.covered,
            positive_recall,
        }
    });
    let is_score = matches!(question.kind, QuestionKind::Score { .. });
    let distance = |s: &Scored| s.chosen.abs_diff(s.truth);
    GroupReport {
        n,
        answered: answered.len(),
        accuracy: ratio(
            answered.iter().filter(|s| s.correct()).count(),
            answered.len(),
        ),
        at_threshold: at_op,
        macro_f1: matches!(question.kind, QuestionKind::Choice { .. })
            .then(|| macro_f1(&answered, question.option_count()))
            .flatten(),
        mae: is_score
            .then(|| ratio(answered.iter().map(distance).sum(), answered.len()))
            .flatten(),
        within_one: is_score
            .then(|| {
                ratio(
                    answered.iter().filter(|s| distance(s) <= 1).count(),
                    answered.len(),
                )
            })
            .flatten(),
        ece_raw: ece(&probs),
        ece_fitted: temperature.and_then(|t| ece(&tempered(&probs, t))),
        brier: brier(&probs),
        grid: Vec::new(),
    }
}

/// The group with precision and coverage at every [`GRID`] threshold.
fn with_grid(mut group: GroupReport, rows: &[(String, usize, Option<Scored>)]) -> GroupReport {
    let answered: Vec<Scored> = rows.iter().filter_map(|r| r.2.clone()).collect();
    group.grid = GRID
        .iter()
        .map(|&t| {
            let at = at_threshold(&answered, rows.len(), t);
            (t, at.precision(), at.coverage())
        })
        .collect();
    group
}

/// Per hard-case tag: `test` messages and accuracy over their answered
/// questions.
fn hard_cases(
    dataset: &Dataset,
    questions: &[Question],
    run: &Run,
) -> BTreeMap<String, (usize, Option<f64>)> {
    let mut hard: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    for (item, r) in dataset.items.iter().zip(&run.items) {
        let Some(r) = r else { continue };
        if item.split != Split::Test {
            continue;
        }
        let mut answered = 0;
        let mut correct = 0;
        for (q, a) in questions.iter().zip(&r.answers) {
            let (Some(d), Some(truth)) = (a, truth_index(q, &item.labels)) else {
                continue;
            };
            answered += 1;
            let a = &d.answer;
            correct += usize::from(chosen(q, &a.value, a.probabilities.as_ref()) == truth);
        }
        for tag in &item.hard {
            let entry = hard.entry(tag.clone()).or_default();
            entry.0 += 1;
            entry.1 += answered;
            entry.2 += correct;
        }
    }
    hard.into_iter()
        .map(|(tag, (items, answered, correct))| (tag, (items, ratio(correct, answered))))
        .collect()
}

pub fn run_report(dataset: &Dataset, questions: &[Question], run: &Run) -> RunReport {
    let mut question_reports = Vec::new();
    for (qi, question) in questions.iter().enumerate() {
        let target = dataset
            .manifest
            .questions
            .iter()
            .find(|q| q.id == question.id)
            .map_or(0.95, |q| q.target_precision);
        let dev: Vec<Scored> = scored(dataset, run, qi, question, Split::Dev)
            .into_iter()
            .filter_map(|r| r.2)
            .collect();
        let threshold = operating_point(&dev, target, MIN_SUPPORT);
        let temperature = fit_temperature(&with_probabilities(&dev));
        let test = scored(dataset, run, qi, question, Split::Test);
        let mut groups = BTreeMap::new();
        groups.insert(
            "all".to_owned(),
            with_grid(group(&test, question, threshold, temperature), &test),
        );
        for locale in &dataset.manifest.locales {
            let rows: Vec<_> = test.iter().filter(|r| &r.0 == locale).cloned().collect();
            if !rows.is_empty() {
                groups.insert(
                    locale.clone(),
                    group(&rows, question, threshold, temperature),
                );
            }
        }
        question_reports.push(QuestionReport {
            id: question.id.clone(),
            target_precision: target,
            threshold,
            temperature,
            groups,
        });
    }
    let test_runs: Vec<_> = dataset
        .items
        .iter()
        .zip(&run.items)
        .filter(|(item, _)| item.split == Split::Test)
        .filter_map(|(_, r)| r.as_ref())
        .collect();
    let latencies: Vec<f64> = test_runs
        .iter()
        .map(|r| r.latency.as_secs_f64() * 1000.0)
        .collect();
    RunReport {
        hard_cases: hard_cases(dataset, questions, run),
        name: run.name.clone(),
        offline: is_offline(&run.name),
        errors: run
            .items
            .iter()
            .flatten()
            .filter(|r| r.error.is_some())
            .count(),
        latency_p50_ms: quantile(&latencies, 0.5),
        latency_p95_ms: quantile(&latencies, 0.95),
        remote_calls_per_item: (!test_runs.is_empty()).then(|| {
            f(test_runs.iter().map(|r| r.remote_calls as usize).sum()) / f(test_runs.len())
        }),
        stage_thresholds: run
            .thresholds
            .iter()
            .map(|(name, t)| {
                (
                    name.clone(),
                    t.iter().map(|(k, v)| (k.clone(), *v)).collect(),
                )
            })
            .collect(),
        questions: question_reports,
    }
}

/// Answered share and accuracy on `test`, all locales, per question.
pub fn curve_points(
    dataset: &Dataset,
    questions: &[Question],
    run: &Run,
    corrections: &str,
) -> Vec<CurvePoint> {
    questions
        .iter()
        .enumerate()
        .map(|(qi, q)| {
            let rows = scored(dataset, run, qi, q, Split::Test);
            let answered: Vec<&Scored> = rows.iter().filter_map(|r| r.2.as_ref()).collect();
            CurvePoint {
                corrections: corrections.to_owned(),
                question: q.id.clone(),
                answered: ratio(answered.len(), rows.len()),
                accuracy: ratio(
                    answered.iter().filter(|s| s.correct()).count(),
                    answered.len(),
                ),
            }
        })
        .collect()
}

fn pct(v: Option<f64>) -> String {
    v.map_or_else(|| "–".to_owned(), |v| format!("{:.0}%", v * 100.0))
}

fn num(v: Option<f64>, digits: usize) -> String {
    v.map_or_else(|| "–".to_owned(), |v| format!("{v:.digits$}"))
}

/// The report as markdown.
pub fn markdown(report: &Report) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# `decide` eval — {} v{}\n\n{} items · test split · generated {} at `{}` by \
         `cargo xtask eval decide`.\n",
        report.dataset, report.version, report.items, report.generated, report.commit
    );
    let _ = writeln!(
        out,
        "How to read it: each strategy's **threshold** is the lowest confidence whose \
         precision reached the question's target on the `dev` split (with at least {MIN_SUPPORT} \
         answers). **Precision** (95% Wilson interval) and **coverage** are measured at that \
         threshold on `test`: coverage is the share of messages the strategy could act on \
         alone. `–` means no threshold reaches the target, so that strategy must not act on \
         that question. Accuracy is over the answers given (abstentions excluded); ECE is the \
         calibration error before → after the temperature fitted on `dev`.\n"
    );
    if !report.not_run.is_empty() {
        let _ = writeln!(out, "## Not run\n");
        for n in &report.not_run {
            let _ = writeln!(out, "- `{}`: {}", n.strategy, n.reason);
        }
        out.push('\n');
    }
    let _ = writeln!(out, "## Strategies\n");
    let _ = writeln!(
        out,
        "| Strategy | Latency p50 / p95 | Model calls per message | Errors |\n| --- | --- | --- | --- |"
    );
    for r in &report.runs {
        let _ = writeln!(
            out,
            "| `{}` | {} / {} ms | {} | {} |",
            r.name,
            num(r.latency_p50_ms, 2),
            num(r.latency_p95_ms, 2),
            num(r.remote_calls_per_item, 2),
            r.errors
        );
    }
    out.push('\n');
    let questions: Vec<(String, f64)> = report
        .runs
        .first()
        .map(|r| {
            r.questions
                .iter()
                .map(|q| (q.id.clone(), q.target_precision))
                .collect()
        })
        .unwrap_or_default();
    for (qid, target) in &questions {
        markdown_question(&mut out, report, qid, *target);
    }
    markdown_hard_cases(&mut out, report);
    markdown_learning_curve(&mut out, report, &questions);
    out
}

/// One question's section: the table over every strategy, then by locale.
fn markdown_question(out: &mut String, report: &Report, qid: &str, target: f64) {
    let _ = writeln!(
        out,
        "## `{qid}` — target precision {:.0}%\n",
        target * 100.0
    );
    let _ = writeln!(
        out,
        "| Strategy | Threshold | Precision | Holds | Coverage | Recall (true) | Accuracy | Answered | ECE | T |\n\
         | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    );
    for r in &report.runs {
        let Some(q) = r.questions.iter().find(|q| q.id == qid) else {
            continue;
        };
        let Some(g) = q.groups.get("all") else {
            continue;
        };
        let at = g.at_threshold.as_ref();
        let precision = at.map_or_else(
            || "–".to_owned(),
            |a| match (a.precision_low, a.precision_high) {
                (Some(low), Some(high)) => format!(
                    "{} ({}–{})",
                    pct(a.precision),
                    pct(Some(low)),
                    pct(Some(high))
                ),
                _ => pct(a.precision),
            },
        );
        let holds = at.map_or("–", |a| match (a.precision, a.precision_high) {
            (Some(p), _) if p >= target => "yes",
            (_, Some(high)) if high >= target => "unclear",
            _ => "no",
        });
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} → {} | {} |",
            r.name,
            num(q.threshold, 2),
            precision,
            holds,
            pct(at.and_then(|a| a.coverage)),
            pct(at.and_then(|a| a.positive_recall)),
            pct(g.accuracy),
            pct(ratio(g.answered, g.n)),
            num(g.ece_raw, 2),
            num(g.ece_fitted, 2),
            num(q.temperature, 2),
        );
    }
    let _ = writeln!(out, "\nBy locale: precision / coverage at the threshold.\n");
    let locales: Vec<String> = report
        .runs
        .iter()
        .flat_map(|r| r.questions.iter())
        .find(|q| q.id == qid)
        .map(|q| q.groups.keys().filter(|k| *k != "all").cloned().collect())
        .unwrap_or_default();
    let _ = writeln!(
        out,
        "| Strategy | {} |\n| --- |{}",
        locales.join(" | "),
        " --- |".repeat(locales.len())
    );
    for r in &report.runs {
        let Some(q) = r.questions.iter().find(|q| q.id == qid) else {
            continue;
        };
        let cells: Vec<String> = locales
            .iter()
            .map(|l| {
                q.groups
                    .get(l)
                    .and_then(|g| g.at_threshold.as_ref())
                    .map_or_else(
                        || "–".to_owned(),
                        |a| format!("{} / {}", pct(a.precision), pct(a.coverage)),
                    )
            })
            .collect();
        let _ = writeln!(out, "| `{}` | {} |", r.name, cells.join(" | "));
    }
    out.push('\n');
}

/// Accuracy per hard-case tag.
fn markdown_hard_cases(out: &mut String, report: &Report) {
    let tags: Vec<String> = report
        .runs
        .iter()
        .flat_map(|r| r.hard_cases.keys().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    if !tags.is_empty() {
        let _ = writeln!(
            out,
            "## Hard cases\n\nAccuracy over the answered questions of `test` messages with \
             each tag (all six questions pooled).\n"
        );
        let names: Vec<String> = report
            .runs
            .iter()
            .map(|r| format!("`{}`", r.name))
            .collect();
        let _ = writeln!(
            out,
            "| Tag | Messages | {} |\n| --- | --- |{}",
            names.join(" | "),
            " --- |".repeat(names.len())
        );
        for tag in &tags {
            let items = report
                .runs
                .iter()
                .find_map(|r| r.hard_cases.get(tag).map(|h| h.0))
                .unwrap_or_default();
            let cells: Vec<String> = report
                .runs
                .iter()
                .map(|r| pct(r.hard_cases.get(tag).and_then(|h| h.1)))
                .collect();
            let _ = writeln!(out, "| `{tag}` | {items} | {} |", cells.join(" | "));
        }
        out.push('\n');
    }
}

/// The k-NN learning curve, when it was run.
fn markdown_learning_curve(out: &mut String, report: &Report, questions: &[(String, f64)]) {
    if !report.learning_curve.is_empty() {
        let _ = writeln!(
            out,
            "## k-NN learning curve (lexical)\n\nAccuracy / answered share on `test` with \
             the first N corrections of each workspace.\n"
        );
        let mut steps: Vec<&str> = Vec::new();
        for p in &report.learning_curve {
            if !steps.contains(&p.corrections.as_str()) {
                steps.push(&p.corrections);
            }
        }
        let _ = writeln!(
            out,
            "| Question | {} |\n| --- |{}",
            steps.join(" | "),
            " --- |".repeat(steps.len())
        );
        for (qid, _) in questions {
            let cells: Vec<String> = steps
                .iter()
                .map(|s| {
                    report
                        .learning_curve
                        .iter()
                        .find(|p| &p.question == qid && p.corrections == *s)
                        .map_or_else(
                            || "–".to_owned(),
                            |p| format!("{} / {}", pct(p.accuracy), pct(p.answered)),
                        )
                })
                .collect();
            let _ = writeln!(out, "| `{qid}` | {} |", cells.join(" | "));
        }
        out.push('\n');
    }
}

/// Offline strategies whose operating point got worse than the baseline.
pub fn regressions(baseline: &Report, current: &Report) -> Vec<String> {
    let mut out = Vec::new();
    for base in baseline.runs.iter().filter(|r| r.offline) {
        let Some(now) = current.runs.iter().find(|r| r.name == base.name) else {
            out.push(format!(
                "`{}` is in the baseline but was not run",
                base.name
            ));
            continue;
        };
        for bq in &base.questions {
            let Some(nq) = now.questions.iter().find(|q| q.id == bq.id) else {
                continue;
            };
            let at = |q: &QuestionReport| q.groups.get("all").and_then(|g| g.at_threshold.clone());
            let (Some(b), n) = (at(bq), at(nq)) else {
                continue;
            };
            let Some(n) = n else {
                out.push(format!(
                    "`{}` / `{}`: no longer reaches its target precision",
                    base.name, bq.id
                ));
                continue;
            };
            for (what, before, after) in [
                ("precision", b.precision, n.precision),
                ("coverage", b.coverage, n.coverage),
            ] {
                if let (Some(before), Some(after)) = (before, after)
                    && after < before - TOLERANCE
                {
                    out.push(format!(
                        "`{}` / `{}`: {what} {:.1}% → {:.1}%",
                        base.name,
                        bq.id,
                        before * 100.0,
                        after * 100.0
                    ));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(precision: f64, coverage: f64) -> Report {
        let at = AtOp {
            precision: Some(precision),
            precision_low: None,
            precision_high: None,
            coverage: Some(coverage),
            covered: 10,
            positive_recall: None,
        };
        let group = GroupReport {
            n: 10,
            answered: 10,
            accuracy: None,
            at_threshold: Some(at),
            macro_f1: None,
            mae: None,
            within_one: None,
            ece_raw: None,
            ece_fitted: None,
            brier: None,
            grid: Vec::new(),
        };
        Report {
            dataset: "d".into(),
            version: 1,
            items: 10,
            generated: String::new(),
            commit: String::new(),
            not_run: Vec::new(),
            learning_curve: Vec::new(),
            runs: vec![RunReport {
                name: "signals".into(),
                offline: true,
                errors: 0,
                latency_p50_ms: None,
                latency_p95_ms: None,
                remote_calls_per_item: None,
                stage_thresholds: Vec::new(),
                hard_cases: BTreeMap::new(),
                questions: vec![QuestionReport {
                    id: "stream".into(),
                    target_precision: 0.95,
                    threshold: Some(0.9),
                    temperature: None,
                    groups: [("all".to_owned(), group)].into(),
                }],
            }],
        }
    }

    #[test]
    fn regressions_beyond_the_tolerance_are_reported() {
        let base = report(0.97, 0.5);
        assert!(regressions(&base, &report(0.96, 0.49)).is_empty());
        let worse = regressions(&base, &report(0.9, 0.5));
        assert_eq!(worse.len(), 1);
        assert!(worse[0].contains("precision 97.0% → 90.0%"), "{worse:?}");
        let mut gone = report(0.97, 0.5);
        gone.runs[0].questions[0]
            .groups
            .get_mut("all")
            .unwrap()
            .at_threshold = None;
        assert!(regressions(&base, &gone)[0].contains("no longer reaches"));
    }

    #[test]
    fn markdown_lists_runs_and_questions() {
        let md = markdown(&report(0.97, 0.5));
        assert!(md.contains("## `stream` — target precision 95%"), "{md}");
        assert!(
            md.contains("| `signals` | 0.90 | 97% | yes | 50% |"),
            "{md}"
        );
        let md = markdown(&report(0.9, 0.5));
        assert!(md.contains("| 90% | no |"), "{md}");
    }
}
