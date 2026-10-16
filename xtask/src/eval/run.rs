//! Running strategies over the eval set.
//!
//! Every base strategy runs once per item, on every selected question.
//! Cascades then replay those answers through the real
//! [`Cascade`](campbooks_ai::decide::strategy::Cascade), so they cost no
//! second model call and still measure the code that ships.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use campbooks_ai::{
    AiConfig, Error, ExternalContent, Gateway, Protocol, ProviderConfig, Role,
    decide::{
        AnswerValue, Question,
        knn::{Embedder, GatewayEmbedder, InMemoryNeighbours, Knn, LexicalEmbedder},
        signals::{email::Envelope, facts::EmailFacts},
        strategy::{BoxFuture, Cascade, Decided, Decisions, Model, Stage, Strategy},
    },
};
use secrecy::SecretString;
use url::Url;

use super::dataset::{Dataset, Split, message_key};

/// What one strategy answered for one item.
#[derive(Debug, Clone)]
pub struct ItemRun {
    /// One entry per selected question.
    pub answers: Vec<Option<Decided>>,
    pub latency: Duration,
    pub remote_calls: u32,
    /// The call failed; every answer is missing.
    pub error: Option<String>,
}

/// One strategy over the set: an entry per dataset item (`None` where the
/// item was filtered out).
#[derive(Debug, Clone)]
pub struct Run {
    pub name: String,
    pub items: Vec<Option<ItemRun>>,
    /// Per stage (cascades only): question id → threshold used.
    pub thresholds: Vec<(String, HashMap<String, f64>)>,
}

/// Endpoints for the strategies that need one, from `CAMPBOOKS_EVAL_*`.
pub struct Endpoints {
    pub laya: Result<Gateway, String>,
    pub jev: Result<Gateway, String>,
    pub instruct: Result<Gateway, String>,
    pub embed: Result<(Gateway, String), String>,
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn provider(prefix: &str, default_model: Option<&str>) -> Result<ProviderConfig, String> {
    let url_var = format!("CAMPBOOKS_EVAL_{prefix}_URL");
    let model_var = format!("CAMPBOOKS_EVAL_{prefix}_MODEL");
    let url = var(&url_var).ok_or_else(|| format!("{url_var} is not set"))?;
    let base_url = Url::parse(&url).map_err(|e| format!("{url_var}: {e}"))?;
    let model = var(&model_var)
        .or_else(|| default_model.map(str::to_owned))
        .ok_or_else(|| format!("{model_var} is not set"))?;
    // The eval set is fictional (crates/ai/evals/decide), so a hosted
    // endpoint may read it: no real mail ever goes through the eval.
    let mut config =
        ProviderConfig::new(base_url, model).with_external_content(ExternalContent::Allow);
    if let Some(key) = var(&format!("CAMPBOOKS_EVAL_{prefix}_API_KEY")) {
        config = config.with_api_key(SecretString::from(key));
    }
    Ok(config)
}

fn gateway(role: Role, config: ProviderConfig) -> Result<Gateway, String> {
    Gateway::new(AiConfig {
        roles: [(role, config)].into(),
        ..AiConfig::default()
    })
    .map_err(|e| e.to_string())
}

/// Jev through the decisions endpoint at `openrouter.ai`, the same protocol as Laya at
/// another path (ADR 0018): `CAMPBOOKS_EVAL_JEV_API_KEY`, and `_URL`
/// (default `https://openrouter.ai/api/alpha`), `_PATH` (default
/// `decisions`) and `_MODEL` (default `typesafe/jev-1.13`).
fn jev() -> Result<ProviderConfig, String> {
    let key = var("CAMPBOOKS_EVAL_JEV_API_KEY")
        .ok_or_else(|| "CAMPBOOKS_EVAL_JEV_API_KEY is not set".to_owned())?;
    let url = var("CAMPBOOKS_EVAL_JEV_URL").unwrap_or_else(|| JEV_URL.to_owned());
    let base_url = Url::parse(&url).map_err(|e| format!("CAMPBOOKS_EVAL_JEV_URL: {e}"))?;
    let model = var("CAMPBOOKS_EVAL_JEV_MODEL").unwrap_or_else(|| JEV_MODEL.to_owned());
    let path = var("CAMPBOOKS_EVAL_JEV_PATH").unwrap_or_else(|| JEV_PATH.to_owned());
    // The eval set is fictional, as for every hosted endpoint here.
    Ok(ProviderConfig::new(base_url, model)
        .with_external_content(ExternalContent::Allow)
        .with_protocol(Protocol::Laya)
        .with_path(path)
        .map_err(|e| format!("CAMPBOOKS_EVAL_JEV_PATH: {e}"))?
        .with_api_key(SecretString::from(key)))
}

/// Jev's endpoint, path and model when `CAMPBOOKS_EVAL_JEV_*` does not say.
const JEV_URL: &str = "https://openrouter.ai/api/alpha";
const JEV_PATH: &str = "decisions";
const JEV_MODEL: &str = "typesafe/jev-1.13";

impl Endpoints {
    pub fn from_env() -> Self {
        let laya = provider("LAYA", Some("convaiinnovations/laya-multilingual"))
            .and_then(|c| gateway(Role::Decide, c.with_protocol(Protocol::Laya)));
        let jev = jev().and_then(|c| gateway(Role::Decide, c));
        let instruct = provider("INSTRUCT", None).and_then(|mut c| {
            c.logprobs = var("CAMPBOOKS_EVAL_INSTRUCT_LOGPROBS").is_none_or(|v| v != "false");
            gateway(Role::Decide, c)
        });
        let embed = provider("EMBED", None).and_then(|c| {
            let model = c.model.clone();
            gateway(Role::Embed, c).map(|g| (g, model))
        });
        Self {
            laya,
            jev,
            instruct,
            embed,
        }
    }
}

/// The strategies `cargo xtask eval decide` knows, and whether each needs a
/// model server.
pub const BASE: [(&str, bool); 6] = [
    ("facts", false),
    ("knn-lexical", false),
    ("knn-embed", true),
    ("laya", true),
    ("jev", true),
    ("instruct", true),
];

pub fn is_offline(spec: &str) -> bool {
    spec.split('>')
        .all(|stage| BASE.iter().any(|(n, remote)| *n == stage && !remote))
}

/// Everything a run needs to know about the set.
pub struct Context<'a> {
    pub dataset: &'a Dataset,
    pub questions: &'a [Question],
    /// Which items to run on.
    pub selected: Vec<bool>,
}

impl Context<'_> {
    fn truth(&self, item: usize) -> HashMap<String, AnswerValue> {
        self.questions
            .iter()
            .filter_map(|q| {
                let v = self.dataset.items[item].labels.value(&q.id)?;
                Some((q.id.clone(), v))
            })
            .collect()
    }
}

async fn run_one(strategy: &dyn Strategy, envelope: &Envelope, questions: &[Question]) -> ItemRun {
    let started = Instant::now();
    let result = strategy.decide(envelope, questions).await;
    let latency = started.elapsed();
    match result {
        Ok(d) => ItemRun {
            answers: d.answers,
            latency,
            remote_calls: d.remote_calls,
            error: None,
        },
        Err(e) => ItemRun {
            answers: vec![None; questions.len()],
            latency,
            remote_calls: 1,
            error: Some(e.to_string()),
        },
    }
}

/// Attempts per item of a strategy whose call can fail (a hosted model
/// rate-limiting), with a growing pause between them.
const ATTEMPTS: u32 = 3;

/// A strategy that answers the same for every item. An item whose call
/// failed is asked again, up to [`ATTEMPTS`] times.
pub async fn run_plain(ctx: &Context<'_>, strategy: &dyn Strategy) -> Run {
    let mut items = Vec::with_capacity(ctx.dataset.items.len());
    for (i, item) in ctx.dataset.items.iter().enumerate() {
        if !ctx.selected[i] {
            items.push(None);
            continue;
        }
        let mut run = run_one(strategy, &item.message.envelope, ctx.questions).await;
        for attempt in 1..ATTEMPTS {
            if run.error.is_none() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(2_u64.pow(attempt))).await;
            run = run_one(strategy, &item.message.envelope, ctx.questions).await;
        }
        items.push(Some(run));
    }
    Run {
        name: strategy.name().to_owned(),
        items,
        thresholds: Vec::new(),
    }
}

/// Embeddings computed once, looked up per text.
struct CachedEmbedder {
    name: String,
    remote: bool,
    vectors: Arc<HashMap<String, Vec<f32>>>,
}

impl Embedder for CachedEmbedder {
    fn name(&self) -> &str {
        &self.name
    }
    fn is_remote(&self) -> bool {
        self.remote
    }
    fn embed<'a>(&'a self, texts: &'a [String]) -> BoxFuture<'a, Result<Vec<Vec<f32>>, Error>> {
        Box::pin(async move {
            texts
                .iter()
                .map(|t| {
                    self.vectors
                        .get(t)
                        .cloned()
                        .ok_or_else(|| Error::InvalidResponse("text was not embedded".into()))
                })
                .collect()
        })
    }
}

/// k-NN per workspace: an item's corrections are the `dev` items of its
/// workspace (all locales), itself excluded, optionally only the first
/// `limit` of them by date.
pub async fn run_knn(
    ctx: &Context<'_>,
    embedder: &dyn Embedder,
    limit: Option<usize>,
) -> Result<Run> {
    let items = &ctx.dataset.items;
    let texts: Vec<String> = items
        .iter()
        .map(|i| i.message.envelope.embedding_text())
        .collect();
    let vectors = embedder
        .embed(&texts)
        .await
        .map_err(|e| anyhow!("embedding the eval set with {}: {e}", embedder.name()))?;
    let cache: Arc<HashMap<String, Vec<f32>>> =
        Arc::new(texts.iter().cloned().zip(vectors.iter().cloned()).collect());

    let mut by_workspace: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, item) in items.iter().enumerate() {
        if item.split == Split::Dev {
            by_workspace
                .entry(item.workspace.as_str())
                .or_default()
                .push(i);
        }
    }
    for list in by_workspace.values_mut() {
        list.sort_by(|a, b| items[*a].message.date.cmp(&items[*b].message.date));
    }

    let name = format!("knn-{}", embedder.name());
    let mut out = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        if !ctx.selected[i] {
            out.push(None);
            continue;
        }
        let mut store = InMemoryNeighbours::default();
        let corrections = by_workspace
            .get(item.workspace.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .filter(|&&j| j != i)
            .take(limit.unwrap_or(usize::MAX));
        for &j in corrections {
            store.push(vectors[j].clone(), ctx.truth(j));
        }
        let strategy = Knn::new(
            CachedEmbedder {
                name: embedder.name().to_owned(),
                remote: embedder.is_remote(),
                vectors: Arc::clone(&cache),
            },
            store,
        );
        out.push(Some(
            run_one(&strategy, &item.message.envelope, ctx.questions).await,
        ));
    }
    Ok(Run {
        name,
        items: out,
        thresholds: Vec::new(),
    })
}

/// Build and run a base strategy, or say why it can't run.
pub async fn run_base(
    ctx: &Context<'_>,
    name: &str,
    endpoints: &Endpoints,
) -> Result<std::result::Result<Run, String>> {
    Ok(Ok(match name {
        "facts" => run_plain(ctx, &EmailFacts).await,
        "knn-lexical" => run_knn(ctx, &LexicalEmbedder::default(), None).await?,
        "knn-embed" => match &endpoints.embed {
            Ok((gateway, model)) => {
                let mut run =
                    run_knn(ctx, &GatewayEmbedder::new(gateway.clone(), model), None).await?;
                run.name = "knn-embed".into();
                run
            }
            Err(reason) => return Ok(Err(reason.clone())),
        },
        "laya" | "jev" | "instruct" => {
            let endpoint = match name {
                "laya" => &endpoints.laya,
                "jev" => &endpoints.jev,
                _ => &endpoints.instruct,
            };
            match endpoint {
                Ok(gateway) => {
                    run_plain(
                        ctx,
                        &Model::new(name, gateway.clone(), "conversations.triage"),
                    )
                    .await
                }
                Err(reason) => return Ok(Err(reason.clone())),
            }
        }
        other => return Err(anyhow!("unknown strategy `{other}`")),
    }))
}

/// Replays a finished run's answers as a strategy, adding up the latency of
/// the calls the cascade actually makes.
struct Replay {
    name: String,
    questions: Vec<String>,
    by_message: HashMap<String, ItemRun>,
    spent: Mutex<Duration>,
}

impl Strategy for Replay {
    fn name(&self) -> &str {
        &self.name
    }

    fn decide<'a>(
        &'a self,
        envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Decisions, Error>> {
        Box::pin(async move {
            let run = self
                .by_message
                .get(&message_key(envelope))
                .ok_or_else(|| Error::InvalidResponse("no recorded answer".into()))?;
            if let Ok(mut spent) = self.spent.lock() {
                *spent += run.latency;
            }
            if let Some(error) = &run.error {
                return Err(Error::InvalidResponse(error.clone()));
            }
            Ok(Decisions {
                answers: questions
                    .iter()
                    .map(|q| {
                        let i = self.questions.iter().position(|id| *id == q.id)?;
                        run.answers.get(i).cloned().flatten()
                    })
                    .collect(),
                remote_calls: run.remote_calls,
            })
        })
    }
}

/// A cascade over finished runs. `thresholds[s]` gives, per question, the
/// confidence from which stage `s` is trusted (the last stage takes any
/// answer).
pub async fn run_cascade(
    ctx: &Context<'_>,
    stages: &[&Run],
    thresholds: &[HashMap<String, f64>],
) -> Run {
    let question_ids: Vec<String> = ctx.questions.iter().map(|q| q.id.clone()).collect();
    let replays: Vec<Arc<Replay>> = stages
        .iter()
        .map(|run| {
            Arc::new(Replay {
                name: run.name.clone(),
                questions: question_ids.clone(),
                by_message: ctx
                    .dataset
                    .items
                    .iter()
                    .zip(&run.items)
                    .filter_map(|(item, r)| Some((message_key(&item.message.envelope), r.clone()?)))
                    .collect(),
                spent: Mutex::new(Duration::ZERO),
            })
        })
        .collect();
    let cascade = Cascade::new(
        replays
            .iter()
            .zip(thresholds)
            .map(|(replay, t)| Stage {
                strategy: Arc::clone(replay) as Arc<dyn Strategy>,
                thresholds: t.clone(),
            })
            .collect(),
    );
    let mut items = Vec::with_capacity(ctx.dataset.items.len());
    for (i, item) in ctx.dataset.items.iter().enumerate() {
        if !ctx.selected[i] {
            items.push(None);
            continue;
        }
        for r in &replays {
            if let Ok(mut spent) = r.spent.lock() {
                *spent = Duration::ZERO;
            }
        }
        let mut run = run_one(&cascade, &item.message.envelope, ctx.questions).await;
        run.latency = replays
            .iter()
            .map(|r| r.spent.lock().map(|s| *s).unwrap_or_default())
            .sum();
        items.push(Some(run));
    }
    Run {
        name: cascade.name().to_owned(),
        items,
        thresholds: stages
            .iter()
            .map(|s| s.name.clone())
            .zip(thresholds.iter().cloned())
            .collect(),
    }
}
