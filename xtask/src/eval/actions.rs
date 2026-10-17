//! `cargo xtask eval actions`: how the action catalogue's ratings
//! (campbooks-cloud#166) match what the owner says they would do with their
//! own mail, at one or more floors.
//!
//! The set is an owner's export (`campbooks-server labelling export --set
//! actions`), labelled by the owner, never by an agent: one column per
//! action in `labels.csv`. The decide model (Jev by default) answers the
//! questions the catalogue reads (`needs_reply`, `suspicious` and its own),
//! [`likelihoods`] turns them into one rating per action, exactly as the
//! model pass does, and each floor is scored per action: precision (of the
//! actions it would offer, how many the owner would do), recall (of the
//! actions the owner would do, how many it would offer) and support. Each
//! pair of actions is then counted from the labels alone (how often the
//! owner would do both), so the combo table can be pruned or extended.

use std::{collections::BTreeMap, fmt::Write as _, fs, path::PathBuf};

use anyhow::{Context as _, Result, bail};
use campbooks_ai::decide::{
    Answer, Question,
    signals::{
        actions::{ActionId, COMBOS, action_questions, likelihoods},
        email::{Envelope, triage, triage_questions},
    },
};
use clap::Args;

use super::{
    dataset::Dataset,
    run::{Context, Endpoints, Run, run_base},
};

#[derive(Debug, Args)]
pub struct ActionsArgs {
    /// The owner's labelled export (`campbooks-server labelling export --set
    /// actions`), wherever it is; it never enters the repository.
    #[arg(long)]
    set_dir: PathBuf,
    /// A floor to score, as a likelihood between 0 and 1 (repeatable;
    /// default 0.5, 0.6, 0.7, 0.8 and 0.9).
    #[arg(long = "floor")]
    floors: Vec<f64>,
    /// The strategy that rates (default `jev`, from `CAMPBOOKS_EVAL_JEV_*`).
    #[arg(long, default_value = "jev")]
    strategy: String,
}

/// Floors scored when none is named.
const FLOORS: [f64; 5] = [0.5, 0.6, 0.7, 0.8, 0.9];

pub fn run_actions(args: &ActionsArgs) -> Result<String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("starting the async runtime")?;
    runtime.block_on(actions(args))
}

async fn actions(args: &ActionsArgs) -> Result<String> {
    let floors: Vec<f64> = if args.floors.is_empty() {
        FLOORS.to_vec()
    } else {
        args.floors.clone()
    };
    if let Some(bad) = floors.iter().find(|f| !(0.0..=1.0).contains(*f)) {
        bail!("--floor {bad}: a likelihood between 0 and 1");
    }
    let dataset = Dataset::load(&args.set_dir)?;
    let questions = asked();
    // Only what the owner labelled goes to the model.
    let selected: Vec<bool> = dataset
        .items
        .iter()
        .map(|i| !i.labels.actions.is_empty())
        .collect();
    let labelled = selected.iter().filter(|s| **s).count();
    if labelled == 0 {
        bail!(
            "no item is labelled for actions: fill the action columns of {}",
            args.set_dir.join("labels.csv").display()
        );
    }
    let ctx = Context {
        dataset: &dataset,
        questions: &questions,
        selected,
    };
    let run = match run_base(&ctx, &args.strategy, &Endpoints::from_env()).await? {
        Ok(run) => run,
        Err(reason) => bail!("`{}` did not run: {reason}", args.strategy),
    };
    let rated = ratings(&dataset, &run);
    let labels: Vec<&BTreeMap<ActionId, bool>> =
        dataset.items.iter().map(|i| &i.labels.actions).collect();
    let report = report(&rated, &labels, &floors, labelled, &args.strategy);
    let dir = args.set_dir.join("report");
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join("actions.md");
    fs::write(&path, &report).with_context(|| format!("writing {}", path.display()))?;
    Ok(format!("{report}\nwrote {}", path.display()))
}

/// The questions the catalogue reads: `needs_reply` and `suspicious` from
/// triage, then its own (all of them: an item without an attachment simply
/// gets no Open attachment rating).
fn asked() -> Vec<Question> {
    let mut questions: Vec<Question> = triage_questions()
        .into_iter()
        .filter(|q| q.id == triage::NEEDS_REPLY || q.id == triage::SUSPICIOUS)
        .collect();
    let attached: Envelope = serde_json::from_value(serde_json::json!({
        "from": {"name": "", "address": "someone@example.com"},
        "attachments": [{"name": "a.pdf", "content_type": "application/pdf"}]
    }))
    .expect("a valid envelope");
    questions.extend(action_questions(&attached));
    questions
}

/// Each item's rating per action (empty where it was not run).
fn ratings(dataset: &Dataset, run: &Run) -> Vec<BTreeMap<ActionId, f64>> {
    dataset
        .items
        .iter()
        .zip(&run.items)
        .map(|(item, ran)| {
            let Some(ran) = ran else {
                return BTreeMap::new();
            };
            let given: Vec<&Answer> = ran.answers.iter().flatten().map(|d| &d.answer).collect();
            likelihoods(&item.message.envelope, &given)
                .into_iter()
                .collect()
        })
        .collect()
}

/// One action's score at one floor.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Score {
    /// Offered and labelled yes.
    hits: usize,
    /// Offered.
    offered: usize,
    /// Labelled yes.
    wanted: usize,
}

impl Score {
    fn precision(self) -> Option<f64> {
        ratio(self.hits, self.offered)
    }

    fn recall(self) -> Option<f64> {
        ratio(self.hits, self.wanted)
    }
}

#[allow(clippy::cast_precision_loss)]
fn ratio(part: usize, whole: usize) -> Option<f64> {
    (whole > 0).then(|| part as f64 / whole as f64)
}

/// `action` at `floor` over every item labelled for actions: an item the
/// owner labelled is offered the action when its rating is at least the
/// floor (no rating: not offered).
fn score(
    rated: &[BTreeMap<ActionId, f64>],
    labels: &[&BTreeMap<ActionId, bool>],
    action: ActionId,
    floor: f64,
) -> Score {
    let mut score = Score {
        hits: 0,
        offered: 0,
        wanted: 0,
    };
    for (ratings, labels) in rated.iter().zip(labels) {
        let Some(&wanted) = labels.get(&action) else {
            continue;
        };
        let offered = ratings.get(&action).is_some_and(|r| *r >= floor);
        score.offered += usize::from(offered);
        score.wanted += usize::from(wanted);
        score.hits += usize::from(offered && wanted);
    }
    score
}

/// How often the owner would do both actions of a pair, from the labels.
fn together(
    labels: &[&BTreeMap<ActionId, bool>],
    a: ActionId,
    b: ActionId,
) -> (usize, usize, usize) {
    let yes = |l: &BTreeMap<ActionId, bool>, x: ActionId| l.get(&x).copied().unwrap_or(false);
    let mut counts = (0, 0, 0);
    for l in labels {
        counts.0 += usize::from(yes(l, a) && yes(l, b));
        counts.1 += usize::from(yes(l, a));
        counts.2 += usize::from(yes(l, b));
    }
    counts
}

/// Pairs that are two answers to one question (Report / It's safe, Already
/// paid / Still to pay): never done together, never counted.
fn one_question(a: ActionId, b: ActionId) -> bool {
    let pair = |x, y| (a == x && b == y) || (a == y && b == x);
    pair(ActionId::Report, ActionId::Safe) || pair(ActionId::AlreadyPaid, ActionId::StillToPay)
}

fn report(
    rated: &[BTreeMap<ActionId, f64>],
    labels: &[&BTreeMap<ActionId, bool>],
    floors: &[f64],
    labelled: usize,
    strategy: &str,
) -> String {
    let pct = |v: Option<f64>| v.map_or_else(|| "—".to_owned(), |v| format!("{:.0}%", v * 100.0));
    let mut out = format!(
        "# Action catalogue ({strategy}) on {labelled} labelled emails\n\n\
         Precision: of the actions offered at the floor, how many the owner would do. \
         Recall: of the actions the owner would do, how many were offered.\n"
    );
    for floor in floors {
        let _ = write!(
            out,
            "\n## Floor {floor:.2}\n\n| Action | Precision | Recall | Offered | Wanted |\n| --- | --- | --- | --- | --- |\n"
        );
        for action in ActionId::ALL {
            let s = score(rated, labels, action, *floor);
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                action.as_str(),
                pct(s.precision()),
                pct(s.recall()),
                s.offered,
                s.wanted
            );
        }
    }
    out.push_str(
        "\n## Done together (from the labels)\n\n\
         | Pair | Both | First | Second | In combos |\n| --- | --- | --- | --- | --- |\n",
    );
    for (index, first) in ActionId::ALL.iter().enumerate() {
        for second in &ActionId::ALL[index + 1..] {
            if one_question(*first, *second) {
                continue;
            }
            let (both, firsts, seconds) = together(labels, *first, *second);
            let combo = COMBOS.iter().any(|(one, other)| {
                (one == first && other == second) || (one == second && other == first)
            });
            let _ = writeln!(
                out,
                "| {} + {} | {both} | {firsts} | {seconds} | {} |",
                first.as_str(),
                second.as_str(),
                if combo { "yes" } else { "" }
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floors_are_scored_per_action_and_pairs_counted_from_labels() {
        let rated = vec![
            BTreeMap::from([(ActionId::Reply, 0.9), (ActionId::Remind, 0.65)]),
            BTreeMap::from([(ActionId::Reply, 0.55), (ActionId::Remind, 0.8)]),
            BTreeMap::from([(ActionId::Reply, 0.2)]),
        ];
        let owner = [
            BTreeMap::from([(ActionId::Reply, true), (ActionId::Remind, true)]),
            BTreeMap::from([(ActionId::Reply, true), (ActionId::Remind, false)]),
            BTreeMap::from([(ActionId::Reply, false), (ActionId::Remind, false)]),
        ];
        let labels: Vec<&BTreeMap<ActionId, bool>> = owner.iter().collect();
        let reply = score(&rated, &labels, ActionId::Reply, 0.5);
        assert_eq!(
            reply,
            Score {
                hits: 2,
                offered: 2,
                wanted: 2
            }
        );
        assert_eq!(reply.precision(), Some(1.0));
        let remind = score(&rated, &labels, ActionId::Remind, 0.7);
        assert_eq!((remind.hits, remind.offered, remind.wanted), (0, 1, 1));
        assert_eq!(remind.precision(), Some(0.0));
        // Never offered: no precision to speak of.
        assert_eq!(
            score(&rated, &labels, ActionId::Forward, 0.5).precision(),
            None
        );
        assert_eq!(
            together(&labels, ActionId::Reply, ActionId::Remind),
            (1, 2, 1)
        );
        let report = report(&rated, &labels, &[0.5], 3, "test");
        assert!(
            report.contains("| reply | 100% | 100% | 2 | 2 |"),
            "{report}"
        );
        assert!(
            report.contains("| reply + remind | 1 | 2 | 1 |"),
            "{report}"
        );
        assert!(!report.contains("report + safe"), "{report}");
    }

    #[test]
    fn the_questions_asked_are_the_catalogue_s() {
        let ids: Vec<String> = asked().into_iter().map(|q| q.id).collect();
        assert!(ids.contains(&triage::NEEDS_REPLY.to_owned()));
        assert!(ids.contains(&triage::SUSPICIOUS.to_owned()));
        for id in campbooks_ai::decide::signals::actions::question::ALL {
            assert!(ids.contains(&id.to_owned()), "{id}");
        }
    }
}
