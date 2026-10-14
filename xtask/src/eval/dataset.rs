//! The eval set on disk: `manifest.json` and `items/<locale>.jsonl`.

use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
};

use anyhow::{Context as _, Result, bail};
use campbooks_ai::decide::{
    AnswerValue, Question,
    signals::email::{Envelope, Recipient},
};
use serde::Deserialize;

/// `manifest.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: u32,
    pub locales: Vec<String>,
    pub workspaces: BTreeMap<String, String>,
    pub questions: Vec<QuestionSpec>,
}

/// One question of the set and the precision an automatic action needs.
#[derive(Debug, Clone, Deserialize)]
pub struct QuestionSpec {
    pub id: String,
    #[serde(default)]
    pub labels: Vec<String>,
    pub target_precision: f64,
}

/// One labelled message.
#[derive(Debug, Clone, Deserialize)]
pub struct Item {
    pub id: String,
    pub lang: String,
    pub split: Split,
    pub workspace: String,
    #[serde(default)]
    pub hard: Vec<String>,
    pub message: Message,
    pub labels: Labels,
}

/// The envelope plus the date (which the rules don't read).
#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    pub date: String,
    #[serde(flatten)]
    pub envelope: Envelope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Split {
    Dev,
    Test,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // the labels are four yes/no questions
pub struct Labels {
    pub stream: String,
    pub needs_reply: bool,
    pub asks_action: bool,
    pub has_deadline: bool,
    pub urgency: u8,
    pub suspicious: bool,
}

impl Labels {
    /// The true answer to question `id`.
    pub fn value(&self, id: &str) -> Option<AnswerValue> {
        Some(match id {
            "stream" => AnswerValue::Choice(self.stream.clone()),
            "needs_reply" => AnswerValue::Noul(self.needs_reply),
            "asks_action" => AnswerValue::Noul(self.asks_action),
            "has_deadline" => AnswerValue::Noul(self.has_deadline),
            "urgency" => AnswerValue::Score(f64::from(self.urgency)),
            "suspicious" => AnswerValue::Noul(self.suspicious),
            _ => return None,
        })
    }
}

/// The index of the true option of `question` for this item.
pub fn truth_index(question: &Question, labels: &Labels) -> Option<usize> {
    match labels.value(&question.id)? {
        AnswerValue::Choice(label) => match &question.kind {
            campbooks_ai::decide::QuestionKind::Choice { options, .. } => {
                options.iter().position(|o| o.label == label)
            }
            _ => None,
        },
        AnswerValue::Noul(holds) => Some(usize::from(holds)),
        AnswerValue::Score(_) => Some(usize::from(labels.urgency)),
    }
}

/// A loaded set.
#[derive(Debug, Clone)]
pub struct Dataset {
    pub manifest: Manifest,
    pub items: Vec<Item>,
}

impl Dataset {
    pub fn load(dir: &Path) -> Result<Self> {
        let manifest: Manifest = serde_json::from_str(
            &fs::read_to_string(dir.join("manifest.json")).context("reading manifest.json")?,
        )
        .context("parsing manifest.json")?;
        let mut items = Vec::new();
        for locale in &manifest.locales {
            let path = dir.join("items").join(format!("{locale}.jsonl"));
            let text =
                fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
            for (n, line) in text.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                let mut item: Item = serde_json::from_str(line)
                    .with_context(|| format!("{}:{}", path.display(), n + 1))?;
                // Every message of the set arrived in its workspace owner's
                // personal inbox, the first `To`: what the decide pass tells
                // a model about the recipient.
                let envelope = &mut item.message.envelope;
                if envelope.recipient.is_none()
                    && let Some(owner) = envelope.to.first()
                {
                    envelope.recipient = Some(Recipient {
                        name: owner.name.clone(),
                        addresses: vec![owner.address.clone()],
                        shared: false,
                    });
                }
                items.push(item);
            }
        }
        let dataset = Self { manifest, items };
        let problems = dataset.problems();
        if !problems.is_empty() {
            bail!("the eval set is inconsistent:\n  {}", problems.join("\n  "));
        }
        Ok(dataset)
    }

    /// Everything wrong with the set: unknown values, broken consistency
    /// rules, non-reserved domains, duplicate ids or messages.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut ids = HashSet::new();
        let mut keys = HashSet::new();
        let streams: Vec<&str> = self
            .manifest
            .questions
            .iter()
            .find(|q| q.id == "stream")
            .map(|q| q.labels.iter().map(String::as_str).collect())
            .unwrap_or_default();
        for item in &self.items {
            let mut bad = |msg: &str| out.push(format!("{}: {msg}", item.id));
            if !ids.insert(item.id.as_str()) {
                bad("duplicate id");
            }
            if !keys.insert(message_key(&item.message.envelope)) {
                bad("duplicate message");
            }
            if !self.manifest.locales.contains(&item.lang) {
                bad("unknown locale");
            }
            if !self.manifest.workspaces.contains_key(&item.workspace) {
                bad("unknown workspace");
            }
            if item.id
                != format!(
                    "{}-{}-{}",
                    item.lang,
                    item.workspace,
                    item.id.rsplit('-').next().unwrap_or_default()
                )
            {
                bad("id is not <locale>-<workspace>-<nn>");
            }
            let env = &item.message.envelope;
            for party in std::iter::once(&env.from).chain(&env.to).chain(&env.cc) {
                if !is_reserved(party.address.rsplit('@').next().unwrap_or_default()) {
                    bad(&format!(
                        "address on a non-reserved domain: {}",
                        party.address
                    ));
                }
            }
            for host in link_hosts(&env.body) {
                if !is_reserved(&host) {
                    bad(&format!("link to a non-reserved host: {host}"));
                }
            }
            let l = &item.labels;
            if !streams.contains(&l.stream.as_str()) {
                bad("unknown stream");
            }
            if l.urgency > 2 {
                bad("urgency is 0, 1 or 2");
            }
            if l.needs_reply && l.stream != "primary" {
                bad("needs_reply outside primary");
            }
            if l.needs_reply && !l.asks_action {
                bad("needs_reply without asks_action");
            }
            if l.urgency > 0 && !l.asks_action {
                bad("urgency without asks_action");
            }
            if l.urgency == 2 && !l.has_deadline {
                bad("urgency 2 without has_deadline");
            }
            if l.suspicious && (l.needs_reply || l.urgency != 0) {
                bad("suspicious with needs_reply or urgency");
            }
        }
        out
    }
}

/// What identifies a message's content (the eval set has no duplicates).
pub fn message_key(env: &Envelope) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}",
        env.from.address, env.subject, env.body
    )
}

/// Domains reserved for examples (RFC 2606, RFC 6761).
fn is_reserved(domain: &str) -> bool {
    let d = domain.trim_end_matches('.').to_ascii_lowercase();
    let d = d.split(':').next().unwrap_or_default();
    ["example.com", "example.org", "example.net"]
        .iter()
        .any(|r| d == *r || d.ends_with(&format!(".{r}")))
        || [".example", ".test", ".invalid"]
            .iter()
            .any(|tld| d.ends_with(tld))
}

fn link_hosts(text: &str) -> Vec<String> {
    text.match_indices("://")
        .filter_map(|(i, _)| {
            let scheme = &text[..i];
            if !(scheme.ends_with("http") || scheme.ends_with("https")) {
                return None;
            }
            let host: String = text[i + 3..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | ':'))
                .collect();
            Some(host.trim_end_matches(['.', ':']).to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set() -> Dataset {
        let dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/ai/evals/decide/mail-triage");
        match Dataset::load(&dir) {
            Ok(set) => set,
            Err(e) => panic!("{e:#}"),
        }
    }

    #[test]
    fn the_eval_set_is_consistent() {
        let set = set();
        assert!(set.items.len() >= 300, "{} items", set.items.len());
        for locale in &set.manifest.locales {
            let n = set.items.iter().filter(|i| &i.lang == locale).count();
            assert!(n >= 60, "{locale}: {n} items");
        }
        for question in &set.manifest.questions {
            assert!(question.target_precision > 0.5 && question.target_precision < 1.0);
        }
    }

    #[test]
    fn planted_problems_are_caught() {
        let mut set = set();
        let mut bad = set.items[0].clone();
        bad.id = format!("{}-{}-99", bad.lang, bad.workspace);
        bad.message
            .envelope
            .body
            .push_str("\nhttps://real-bank.com/login");
        bad.message.envelope.from.address = "ceo@real-company.com".into();
        bad.labels.suspicious = true;
        bad.labels.needs_reply = true;
        set.items.push(bad);
        let problems = set.problems().join("\n");
        assert!(
            problems.contains("non-reserved domain: ceo@real-company.com"),
            "{problems}"
        );
        assert!(
            problems.contains("non-reserved host: real-bank.com"),
            "{problems}"
        );
        assert!(
            problems.contains("suspicious with needs_reply"),
            "{problems}"
        );
    }

    #[test]
    fn reserved_domains() {
        assert!(is_reserved("bancoatlantico.example"));
        assert!(is_reserved("news.atelier.example.com"));
        assert!(is_reserved("login-atlantico.test"));
        assert!(!is_reserved("example.co"));
        assert!(!is_reserved("notexample.com"));
    }
}
