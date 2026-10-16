//! `cargo xtask eval decide`: run `decide` strategies over a labelled eval set
//! and report, per question and locale, how often each one can be trusted to
//! act alone (campbooks-cloud#59).
//!
//! Strategies that need a model server read it from `CAMPBOOKS_EVAL_*`
//! (`LAYA_URL`, `INSTRUCT_URL` + `INSTRUCT_MODEL`, `EMBED_URL` + `EMBED_MODEL`,
//! each with an optional `_API_KEY`; `JEV_API_KEY`, with `JEV_URL`,
//! `JEV_PATH` and `JEV_MODEL` defaulting to the decisions endpoint at `openrouter.ai`
//! and `typesafe/jev-1.13`); without it they are reported as not run.

mod dataset;
mod metrics;
mod report;
mod run;

use std::{collections::HashMap, fs, path::Path, process::Command};

use anyhow::{Context as _, Result, bail};
use campbooks_ai::decide::{Question, knn::LexicalEmbedder, signals::email::triage_questions};
use clap::Args;

use dataset::Dataset;
use report::{
    CurvePoint, NotRun, Report, RunReport, curve_points, markdown, regressions, run_report,
};
use run::{BASE, Context, Endpoints, Run, is_offline, run_base, run_cascade, run_knn};

/// Strategies run when none is named: every base strategy and the cascades
/// worth comparing. Those needing an endpoint run only when it is configured.
const DEFAULT: [&str; 12] = [
    "facts",
    "knn-lexical",
    "facts>knn-lexical",
    "knn-embed",
    "laya",
    "jev",
    "instruct",
    "facts>laya",
    "facts>jev",
    "facts>knn-lexical>laya",
    "facts>instruct",
    "facts>knn-embed>instruct",
];

#[derive(Debug, Args)]
pub struct DecideArgs {
    /// Strategy to run: a base strategy (facts, knn-lexical, knn-embed,
    /// laya, jev, instruct) or a cascade (`facts>knn-lexical>jev`). Repeatable;
    /// default: all of them.
    #[arg(long = "strategy")]
    strategies: Vec<String>,
    /// Only these question ids (repeatable; default: all).
    #[arg(long = "question")]
    questions: Vec<String>,
    /// Only messages in these locales (repeatable; default: all).
    #[arg(long = "lang")]
    langs: Vec<String>,
    /// Eval set under crates/ai/evals/decide/.
    #[arg(long, default_value = "mail-triage")]
    set: String,
    /// Only strategies that need no model server.
    #[arg(long)]
    offline: bool,
    /// Where to write report.md and report.json
    /// (default: target/eval/decide/<set>).
    #[arg(long)]
    out: Option<std::path::PathBuf>,
    /// Write the report as the set's committed baseline
    /// (reports/baseline.{md,json}) instead.
    #[arg(long, conflicts_with = "out")]
    baseline: bool,
    /// Fail if an offline strategy's operating point fell below the committed
    /// baseline.
    #[arg(long)]
    check: bool,
}

pub fn run_decide(root: &Path, args: &DecideArgs) -> Result<String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("starting the async runtime")?;
    runtime.block_on(decide(root, args))
}

async fn decide(root: &Path, args: &DecideArgs) -> Result<String> {
    let set_dir = root.join("crates/ai/evals/decide").join(&args.set);
    let dataset = Dataset::load(&set_dir)?;
    let (questions, specs) = plan(args, &dataset)?;
    let ctx = Context {
        dataset: &dataset,
        questions: &questions,
        selected: dataset
            .items
            .iter()
            .map(|i| args.langs.is_empty() || args.langs.contains(&i.lang))
            .collect(),
    };
    let outcome = run_specs(&ctx, &specs, &Endpoints::from_env()).await?;
    // The check compares operating points only; the curve is for reading.
    let learning_curve = if args.check {
        Vec::new()
    } else {
        learning_curve(&ctx, &specs, &outcome.base).await?
    };
    let report = Report {
        dataset: dataset.manifest.name.clone(),
        version: dataset.manifest.version,
        items: ctx.selected.iter().filter(|s| **s).count(),
        generated: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        commit: commit(root),
        not_run: outcome.not_run,
        runs: outcome.reports,
        learning_curve,
    };
    let reports_dir = set_dir.join("reports");
    if args.check {
        return check(&reports_dir, &report);
    }
    let (dir, stem) = if args.baseline {
        (reports_dir, "baseline")
    } else {
        (
            args.out
                .clone()
                .unwrap_or_else(|| root.join("target/eval/decide").join(&args.set)),
            "report",
        )
    };
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let md = markdown(&report);
    let md_path = dir.join(format!("{stem}.md"));
    fs::write(&md_path, &md).with_context(|| format!("writing {}", md_path.display()))?;
    let json_path = dir.join(format!("{stem}.json"));
    fs::write(&json_path, serde_json::to_string_pretty(&report)? + "\n")
        .with_context(|| format!("writing {}", json_path.display()))?;
    let mut written = vec![md_path, json_path];
    if !args.baseline {
        let path = dir.join("predictions.jsonl");
        fs::write(&path, predictions(&dataset, &questions, &outcome.finished))
            .with_context(|| format!("writing {}", path.display()))?;
        written.push(path);
    }
    let written: Vec<String> = written.iter().map(|p| p.display().to_string()).collect();
    Ok(format!("{md}\nwrote {}", written.join(", ")))
}

/// The questions and strategy specs to run, checked.
fn plan(args: &DecideArgs, dataset: &Dataset) -> Result<(Vec<Question>, Vec<String>)> {
    let questions: Vec<_> = triage_questions()
        .into_iter()
        .filter(|q| args.questions.is_empty() || args.questions.contains(&q.id))
        .collect();
    if questions.is_empty() {
        bail!("no question matches {:?}", args.questions);
    }
    for lang in &args.langs {
        if !dataset.manifest.locales.contains(lang) {
            bail!(
                "unknown locale `{lang}`; the set has {:?}",
                dataset.manifest.locales
            );
        }
    }
    let mut specs: Vec<String> = if args.strategies.is_empty() {
        DEFAULT.iter().map(|s| (*s).to_owned()).collect()
    } else {
        args.strategies.clone()
    };
    if args.offline || args.check {
        specs.retain(|s| is_offline(s));
    }
    for spec in &specs {
        for stage in spec.split('>') {
            if !BASE.iter().any(|(name, _)| *name == stage) {
                bail!("unknown strategy `{stage}` in `{spec}`");
            }
        }
    }
    Ok((questions, specs))
}

/// What running the specs produced.
struct Outcome {
    reports: Vec<RunReport>,
    finished: Vec<Run>,
    not_run: Vec<NotRun>,
    base: HashMap<String, Result<Run, String>>,
}

/// Run every base strategy once, then each spec (a base strategy or a
/// cascade replaying them).
async fn run_specs(ctx: &Context<'_>, specs: &[String], endpoints: &Endpoints) -> Result<Outcome> {
    let (dataset, questions) = (ctx.dataset, ctx.questions);
    // Base strategies first, each once.
    let mut base: HashMap<String, Result<Run, String>> = HashMap::new();
    for spec in specs {
        for stage in spec.split('>') {
            if !base.contains_key(stage) {
                base.insert(stage.to_owned(), run_base(ctx, stage, endpoints).await?);
            }
        }
    }

    let mut reports = Vec::new();
    let mut finished: Vec<Run> = Vec::new();
    let mut not_run = Vec::new();
    for spec in specs {
        let stages: Vec<&str> = spec.split('>').collect();
        let missing: Vec<String> = stages
            .iter()
            .filter_map(|s| base.get(*s).and_then(|r| r.as_ref().err()).cloned())
            .collect();
        if !missing.is_empty() {
            not_run.push(NotRun {
                strategy: spec.clone(),
                reason: missing.join("; "),
            });
            continue;
        }
        let stage_runs: Vec<&Run> = stages
            .iter()
            .filter_map(|s| base.get(*s).and_then(|r| r.as_ref().ok()))
            .collect();
        if let [single] = stage_runs.as_slice() {
            reports.push(run_report(dataset, questions, single));
            finished.push((*single).clone());
            continue;
        }
        // Each stage but the last is trusted from its own operating point on
        // `dev`; a stage with none for a question is skipped for it.
        let thresholds: Vec<HashMap<String, f64>> = stage_runs
            .iter()
            .enumerate()
            .map(|(i, r)| {
                if i + 1 == stage_runs.len() {
                    questions.iter().map(|q| (q.id.clone(), 0.0)).collect()
                } else {
                    run_report(dataset, questions, r)
                        .questions
                        .into_iter()
                        .filter_map(|q| Some((q.id, q.threshold?)))
                        .collect()
                }
            })
            .collect();
        let cascade = run_cascade(ctx, &stage_runs, &thresholds).await;
        reports.push(run_report(dataset, questions, &cascade));
        finished.push(cascade);
    }
    Ok(Outcome {
        reports,
        finished,
        not_run,
        base,
    })
}

/// k-NN (lexical) accuracy with the first 5, 10, 20 and all corrections.
async fn learning_curve(
    ctx: &Context<'_>,
    specs: &[String],
    base: &HashMap<String, Result<Run, String>>,
) -> Result<Vec<CurvePoint>> {
    let (dataset, questions) = (ctx.dataset, ctx.questions);
    let mut learning_curve: Vec<CurvePoint> = Vec::new();
    if specs.iter().any(|s| s == "knn-lexical") {
        for limit in [5, 10, 20] {
            let run = run_knn(ctx, &LexicalEmbedder::default(), Some(limit)).await?;
            learning_curve.extend(curve_points(dataset, questions, &run, &limit.to_string()));
        }
        if let Some(Ok(all)) = base.get("knn-lexical") {
            learning_curve.extend(curve_points(dataset, questions, all, "all"));
        }
    }
    Ok(learning_curve)
}

/// Compare offline strategies with the committed baseline. Online runs in
/// the baseline (a hosted model's) are not run here and not compared.
fn check(reports_dir: &Path, report: &Report) -> Result<String> {
    let path = reports_dir.join("baseline.json");
    let baseline: Report = serde_json::from_str(
        &fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?,
    )
    .with_context(|| format!("parsing {}", path.display()))?;
    if baseline.version != report.version {
        bail!(
            "the baseline was made on {} v{}, the set is v{}: regenerate it with \
                 `cargo xtask eval decide --baseline` (with CAMPBOOKS_EVAL_JEV_API_KEY set, \
                 so the model run the policy is pinned to is in it)",
            baseline.dataset,
            baseline.version,
            report.version
        );
    }
    let worse = regressions(&baseline, report);
    if !worse.is_empty() {
        bail!(
            "offline decide strategies fell below the baseline (tolerance {:.0} points):\n  {}\n\
                 Fix the regression, or regenerate the baseline with \
                 `cargo xtask eval decide --baseline` (with CAMPBOOKS_EVAL_JEV_API_KEY set) \
                 if it is intended.",
            report::TOLERANCE * 100.0,
            worse.join("\n  ")
        );
    }
    Ok(format!(
        "decide eval: {} offline strategies hold the baseline ({} v{})",
        report.runs.len(),
        report.dataset,
        report.version
    ))
}

/// One line per (strategy, item): the truth and each answer with its
/// confidence and reasons, for error analysis. Never committed: run it on the
/// fictional set, or on a private export kept out of the repository.
fn predictions(dataset: &Dataset, questions: &[Question], runs: &[Run]) -> String {
    let mut out = String::new();
    for run in runs {
        for (item, r) in dataset.items.iter().zip(&run.items) {
            let Some(r) = r else { continue };
            let answers: serde_json::Map<String, serde_json::Value> = questions
                .iter()
                .zip(&r.answers)
                .map(|(q, a)| {
                    let value = a.as_ref().map_or(serde_json::Value::Null, |d| {
                        serde_json::json!({
                            "value": format!("{:?}", d.answer.value),
                            "confidence": d.answer.confidence,
                            "probabilities": d.answer.probabilities,
                            "reasons": d.reasons.iter().map(ToString::to_string).collect::<Vec<_>>(),
                        })
                    });
                    (q.id.clone(), value)
                })
                .collect();
            let line = serde_json::json!({
                "strategy": run.name,
                "id": item.id,
                "split": format!("{:?}", item.split).to_lowercase(),
                "hard": item.hard,
                "truth": questions
                    .iter()
                    .map(|q| {
                        let truth = item.labels.value(&q.id).map(|v| format!("{v:?}"));
                        (q.id.clone(), serde_json::json!(truth))
                    })
                    .collect::<serde_json::Map<_, _>>(),
                "answers": answers,
                "error": r.error,
            });
            out.push_str(&line.to_string());
            out.push('\n');
        }
    }
    out
}

fn commit(root: &Path) -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map_or_else(|| "unknown".to_owned(), |s| s.trim().to_owned())
}
