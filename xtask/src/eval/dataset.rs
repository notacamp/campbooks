//! The eval set on disk: `manifest.json` and `items/<locale>.jsonl`, and
//! for a private set (an owner's own mail, never committed) the labels the
//! owner filled in `labels.csv` (campbooks-cloud#167).

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
    /// Real mail (an owner's export, `campbooks-server labelling export`):
    /// the fictional set's reserved-domain and id checks do not apply, and
    /// items may be unlabelled (left out of the scores).
    #[serde(default)]
    pub private: bool,
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

/// One value per question; a question the set does not label is `None`
/// (the fictional set labels every question but `money`, an owner's export
/// only `money`).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Labels {
    #[serde(default)]
    pub stream: Option<String>,
    #[serde(default)]
    pub needs_reply: Option<bool>,
    #[serde(default)]
    pub asks_action: Option<bool>,
    #[serde(default)]
    pub has_deadline: Option<bool>,
    #[serde(default)]
    pub urgency: Option<u8>,
    #[serde(default)]
    pub suspicious: Option<bool>,
    /// `owed`, `paid` or `neither` (campbooks-cloud#167).
    #[serde(default)]
    pub money: Option<String>,
}

impl Labels {
    /// The true answer to question `id`, when labelled.
    pub fn value(&self, id: &str) -> Option<AnswerValue> {
        match id {
            "stream" => self.stream.clone().map(AnswerValue::Choice),
            "needs_reply" => self.needs_reply.map(AnswerValue::Noul),
            "asks_action" => self.asks_action.map(AnswerValue::Noul),
            "has_deadline" => self.has_deadline.map(AnswerValue::Noul),
            "urgency" => self.urgency.map(|u| AnswerValue::Score(f64::from(u))),
            "suspicious" => self.suspicious.map(AnswerValue::Noul),
            "money" => self.money.clone().map(AnswerValue::Choice),
            _ => None,
        }
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
        AnswerValue::Score(level) => {
            // Levels are small whole numbers (`urgency` is 0, 1 or 2).
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Some(level as usize)
        }
    }
}

/// The money question's labels.
const MONEY: [&str; 3] = ["owed", "paid", "neither"];

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
        let csv = dir.join("labels.csv");
        if manifest.private && csv.exists() {
            let text =
                fs::read_to_string(&csv).with_context(|| format!("reading {}", csv.display()))?;
            apply_sheet(&text, &mut items)?;
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
            // An owner's export is real mail: its ids and domains are what
            // they are, and an unlabelled item is simply left out.
            if !self.manifest.private {
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
                for question in &self.manifest.questions {
                    if item.labels.value(&question.id).is_none() {
                        bad(&format!("no `{}` label", question.id));
                    }
                }
            }
            let l = &item.labels;
            if let Some(stream) = &l.stream
                && !streams.contains(&stream.as_str())
            {
                bad("unknown stream");
            }
            if let Some(money) = &l.money
                && !MONEY.contains(&money.as_str())
            {
                bad("money is owed, paid or neither");
            }
            let yes = |v: Option<bool>| v == Some(true);
            let urgency = l.urgency.unwrap_or(0);
            if urgency > 2 {
                bad("urgency is 0, 1 or 2");
            }
            if yes(l.needs_reply) && l.stream.as_deref().is_some_and(|s| s != "primary") {
                bad("needs_reply outside primary");
            }
            if yes(l.needs_reply) && l.asks_action == Some(false) {
                bad("needs_reply without asks_action");
            }
            if urgency > 0 && l.asks_action == Some(false) {
                bad("urgency without asks_action");
            }
            if urgency == 2 && l.has_deadline == Some(false) {
                bad("urgency 2 without has_deadline");
            }
            if yes(l.suspicious) && (yes(l.needs_reply) || urgency != 0) {
                bad("suspicious with needs_reply or urgency");
            }
        }
        out
    }
}

/// Fill each item's `money` label from the owner's sheet (`labels.csv`:
/// a header row naming `id` and `money` columns, then one row per item;
/// an empty `money` cell leaves the item unlabelled).
fn apply_sheet(text: &str, items: &mut [Item]) -> Result<()> {
    let mut rows = text.lines().map(csv_row);
    let header = rows.next().unwrap_or_default();
    let column = |name: &str| {
        header
            .iter()
            .position(|h| h.trim() == name)
            .with_context(|| format!("labels.csv has no `{name}` column"))
    };
    let (id, money) = (column("id")?, column("money")?);
    for (n, row) in rows.enumerate() {
        let (Some(row_id), Some(label)) = (row.get(id), row.get(money)) else {
            continue;
        };
        let label = label.trim().to_lowercase();
        if label.is_empty() {
            continue;
        }
        let item = items
            .iter_mut()
            .find(|i| i.id == row_id.trim())
            .with_context(|| format!("labels.csv row {}: no item `{row_id}`", n + 2))?;
        item.labels.money = Some(label);
    }
    Ok(())
}

/// The cells of one CSV line: comma-separated, `"`-quoted cells with `""`
/// for a quote (the sheet the export writes has no line breaks in cells).
fn csv_row(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            ('"', _) => quoted = !quoted,
            (',', false) => cells.push(std::mem::take(&mut cell)),
            _ => cell.push(c),
        }
    }
    cells.push(cell);
    cells
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
        bad.labels.suspicious = Some(true);
        bad.labels.needs_reply = Some(true);
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
    fn the_owners_sheet_fills_the_money_labels() {
        let set = set();
        let mut items = vec![set.items[0].clone(), set.items[1].clone()];
        let (a, b) = (items[0].id.clone(), items[1].id.clone());
        let sheet = format!(
            "id,date,from,subject,attachments,money\n\"{a}\",\"x\",\"A, B\",\"Re: \"\"hi\"\"\",\"\",\"Owed\"\n\"{b}\",,,,,\n"
        );
        apply_sheet(&sheet, &mut items).unwrap();
        assert_eq!(items[0].labels.money.as_deref(), Some("owed"));
        assert_eq!(items[1].labels.money, None, "left blank: unlabelled");
        assert!(apply_sheet("id,money\nnope,paid\n", &mut items).is_err());
        assert!(apply_sheet("id,label\n", &mut items).is_err());
        assert_eq!(csv_row(r#""a,b","c""d",e"#), ["a,b", "c\"d", "e"]);
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
