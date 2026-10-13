//! k nearest neighbours over a workspace's own corrections.
//!
//! When a member corrects a decision ("this is a receipt", "no, this needs a
//! reply"), the corrected message becomes a labelled example of that
//! workspace. [`Knn`] embeds a new message, finds the most similar examples
//! for each question and answers with their weighted vote. It needs no
//! training and learns from the first correction, but it only knows what this
//! workspace has corrected, so it abstains until it has enough close examples.
//!
//! Embeddings come from an [`Embedder`]: the `embed` role ([`GatewayEmbedder`])
//! or [`LexicalEmbedder`], hashed character n-grams with no model at all (a
//! floor to compare against, and a fallback when `embed` is not configured).
//! Examples are looked up through [`Neighbours`], which a module backs with
//! its own store (pgvector) and the eval harness with [`InMemoryNeighbours`].

use std::{collections::HashMap, time::Instant};

use super::{
    AnswerValue, Question, QuestionKind,
    signals::email::Envelope,
    strategy::{BoxFuture, Decided, Decisions, Reason, Strategy, count_f64, local_answer},
};
use crate::{Error, Gateway};

/// Turns texts into vectors.
pub trait Embedder: Send + Sync {
    /// Short name for reports (`lexical`, the embedding model's id).
    fn name(&self) -> &str;
    /// Whether embedding calls a model server.
    fn is_remote(&self) -> bool;
    /// One vector per text, in order.
    fn embed<'a>(&'a self, texts: &'a [String]) -> BoxFuture<'a, Result<Vec<Vec<f32>>, Error>>;
}

/// The `embed` role as an [`Embedder`].
pub struct GatewayEmbedder {
    gateway: Gateway,
    name: String,
}

impl GatewayEmbedder {
    /// Embeds with `gateway`'s `embed` role; `name` is shown in reports.
    pub fn new(gateway: Gateway, name: impl Into<String>) -> Self {
        Self {
            gateway,
            name: name.into(),
        }
    }
}

impl Embedder for GatewayEmbedder {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_remote(&self) -> bool {
        true
    }

    fn embed<'a>(&'a self, texts: &'a [String]) -> BoxFuture<'a, Result<Vec<Vec<f32>>, Error>> {
        Box::pin(async move {
            let mut out = Vec::with_capacity(texts.len());
            for chunk in texts.chunks(32) {
                out.extend(normalised(self.gateway.embed(chunk).await?));
            }
            Ok(out)
        })
    }
}

fn normalised(vectors: Vec<Vec<f32>>) -> impl Iterator<Item = Vec<f32>> {
    vectors.into_iter().map(|mut v| {
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut v {
                *x /= norm;
            }
        }
        v
    })
}

/// Hashed character 3- to 5-grams, sublinear counts, unit length. No model,
/// no download; similar wording gives similar vectors, in any language.
#[derive(Debug, Clone, Copy)]
pub struct LexicalEmbedder {
    dimensions: usize,
}

impl Default for LexicalEmbedder {
    fn default() -> Self {
        Self { dimensions: 4096 }
    }
}

impl LexicalEmbedder {
    /// One text's vector.
    pub fn vector(&self, text: &str) -> Vec<f32> {
        let normal: Vec<char> = text
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .collect();
        let mut counts: HashMap<usize, (f32, u32)> = HashMap::new();
        for n in 3..=5 {
            for gram in normal.windows(n) {
                let hash = fnv1a(gram);
                let bucket = usize::try_from(hash % (self.dimensions as u64)).unwrap_or_default();
                let sign = if hash >> 63 == 0 { 1.0 } else { -1.0 };
                counts.entry(bucket).or_insert((sign, 0)).1 += 1;
            }
        }
        let mut v = vec![0.0_f32; self.dimensions];
        for (bucket, (sign, count)) in counts {
            #[allow(clippy::cast_precision_loss)] // n-gram counts are small
            let weight = 1.0 + (count as f32).ln();
            v[bucket] = sign * weight;
        }
        normalised(vec![v]).next().unwrap_or_default()
    }
}

fn fnv1a(chars: &[char]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for c in chars {
        hash ^= u64::from(u32::from(*c));
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

impl Embedder for LexicalEmbedder {
    fn name(&self) -> &'static str {
        "lexical"
    }

    fn is_remote(&self) -> bool {
        false
    }

    fn embed<'a>(&'a self, texts: &'a [String]) -> BoxFuture<'a, Result<Vec<Vec<f32>>, Error>> {
        Box::pin(async move { Ok(texts.iter().map(|t| self.vector(t)).collect()) })
    }
}

/// A labelled example close to the query.
#[derive(Debug, Clone, PartialEq)]
pub struct Neighbour {
    /// Cosine similarity to the query, in `[-1, 1]`.
    pub similarity: f32,
    /// The corrected answer.
    pub value: AnswerValue,
}

/// A workspace's labelled examples, searchable by similarity.
pub trait Neighbours: Send + Sync {
    /// The `k` examples most similar to `vector` that carry an answer to
    /// `question_id`, most similar first.
    fn nearest<'a>(
        &'a self,
        vector: &'a [f32],
        question_id: &'a str,
        k: usize,
    ) -> BoxFuture<'a, Result<Vec<Neighbour>, Error>>;
}

/// Examples held in memory, searched exhaustively (fine for one workspace's
/// corrections in the eval harness).
#[derive(Debug, Clone, Default)]
pub struct InMemoryNeighbours {
    examples: Vec<(Vec<f32>, HashMap<String, AnswerValue>)>,
}

impl InMemoryNeighbours {
    /// Add an example: its unit-length vector and its answers by question id.
    pub fn push(&mut self, vector: Vec<f32>, answers: HashMap<String, AnswerValue>) {
        self.examples.push((vector, answers));
    }

    /// How many examples there are.
    pub fn len(&self) -> usize {
        self.examples.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.examples.is_empty()
    }
}

impl Neighbours for InMemoryNeighbours {
    fn nearest<'a>(
        &'a self,
        vector: &'a [f32],
        question_id: &'a str,
        k: usize,
    ) -> BoxFuture<'a, Result<Vec<Neighbour>, Error>> {
        Box::pin(async move {
            let mut found: Vec<Neighbour> = self
                .examples
                .iter()
                .filter_map(|(v, answers)| {
                    let value = answers.get(question_id)?.clone();
                    let similarity = v.iter().zip(vector).map(|(a, b)| a * b).sum();
                    Some(Neighbour { similarity, value })
                })
                .collect();
            found.sort_by(|a, b| b.similarity.total_cmp(&a.similarity));
            found.truncate(k);
            Ok(found)
        })
    }
}

/// Answers each question with the similarity-weighted vote of the `k` most
/// similar corrections; abstains with fewer than `min_neighbours` examples at
/// `min_similarity` or more.
pub struct Knn<E, N> {
    name: String,
    embedder: E,
    neighbours: N,
    /// Examples consulted per question.
    pub k: usize,
    /// Fewest qualifying examples needed to answer.
    pub min_neighbours: usize,
    /// Lowest similarity an example needs to count.
    pub min_similarity: f32,
}

/// Additive smoothing per option, so a single neighbour is never certainty.
const SMOOTHING: f64 = 0.5;

impl<E: Embedder, N: Neighbours> Knn<E, N> {
    /// A k-NN strategy named `knn-<embedder>`, with k = 5, at least 2
    /// neighbours of similarity 0.1 or more.
    pub fn new(embedder: E, neighbours: N) -> Self {
        Self {
            name: format!("knn-{}", embedder.name()),
            embedder,
            neighbours,
            k: 5,
            min_neighbours: 2,
            min_similarity: 0.1,
        }
    }
}

fn option_index(question: &Question, value: &AnswerValue) -> Option<usize> {
    match (&question.kind, value) {
        (QuestionKind::Choice { options, .. }, AnswerValue::Choice(label)) => {
            options.iter().position(|o| &o.label == label)
        }
        (QuestionKind::Score { levels, .. }, AnswerValue::Score(level)) => {
            let rounded = level.round();
            (rounded >= 0.0 && rounded < count_f64(levels.len())).then(|| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                // checked to be a small non-negative whole number
                let index = rounded as usize;
                index
            })
        }
        (QuestionKind::Noul { .. }, AnswerValue::Noul(holds)) => Some(usize::from(*holds)),
        _ => None,
    }
}

impl<E: Embedder, N: Neighbours> Strategy for Knn<E, N> {
    fn name(&self) -> &str {
        &self.name
    }

    fn decide<'a>(
        &'a self,
        envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Decisions, Error>> {
        Box::pin(async move {
            let started = Instant::now();
            let text = [envelope.embedding_text()];
            let vector = self.embedder.embed(&text).await?.pop().unwrap_or_default();
            let mut answers = Vec::with_capacity(questions.len());
            for question in questions {
                let close: Vec<Neighbour> = self
                    .neighbours
                    .nearest(&vector, &question.id, self.k)
                    .await?
                    .into_iter()
                    .filter(|n| n.similarity >= self.min_similarity)
                    .collect();
                if close.len() < self.min_neighbours {
                    answers.push(None);
                    continue;
                }
                let n = question.option_count();
                let mut weights = vec![SMOOTHING; n];
                for neighbour in &close {
                    if let Some(i) = option_index(question, &neighbour.value) {
                        weights[i] += f64::from(neighbour.similarity);
                    }
                }
                let total: f64 = weights.iter().sum();
                let probabilities = weights.iter().map(|w| w / total).collect();
                answers.push(Some(Decided {
                    answer: local_answer(question, probabilities, &self.name, started),
                    reasons: vec![Reason::new("knn.neighbours", close.len().to_string())],
                }));
            }
            Ok(Decisions {
                answers,
                remote_calls: u32::from(self.embedder.is_remote()),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decide::signals::email::Party;

    fn env(subject: &str, body: &str) -> Envelope {
        Envelope {
            from: Party {
                name: String::new(),
                address: "x@example.com".into(),
            },
            subject: subject.into(),
            body: body.into(),
            ..Envelope::default()
        }
    }

    async fn knn(examples: &[(&Envelope, bool)]) -> Knn<LexicalEmbedder, InMemoryNeighbours> {
        let embedder = LexicalEmbedder::default();
        let mut store = InMemoryNeighbours::default();
        for (e, holds) in examples {
            let v = embedder
                .embed(&[e.embedding_text()])
                .await
                .unwrap()
                .remove(0);
            store.push(v, [("asks".to_owned(), AnswerValue::Noul(*holds))].into());
        }
        Knn::new(embedder, store)
    }

    #[test]
    fn lexical_vectors_are_unit_length_and_similar_for_similar_text() {
        let e = LexicalEmbedder::default();
        let a = e.vector("Your invoice for October is attached");
        let b = e.vector("Your invoice for November is attached");
        let c = e.vector("Bonjour, on se voit demain au parc ?");
        let dot = |x: &[f32], y: &[f32]| x.iter().zip(y).map(|(p, q)| p * q).sum::<f32>();
        assert!((dot(&a, &a) - 1.0).abs() < 1e-4);
        assert!(dot(&a, &b) > dot(&a, &c) + 0.3);
    }

    #[tokio::test]
    async fn votes_with_the_nearest_corrections() {
        let pay = env("Invoice 12", "Please pay invoice 12 by Friday.");
        let pay2 = env("Invoice 13", "Please pay invoice 13 by Monday.");
        let news = env("Our autumn news", "Read our autumn stories and recipes.");
        let strategy = knn(&[(&pay, true), (&pay2, true), (&news, false)]).await;
        assert_eq!(strategy.name(), "knn-lexical");
        let q = [Question::noul("asks", "Asks for something.")];
        let out = strategy
            .decide(&env("Invoice 14", "Please pay invoice 14 by Tuesday."), &q)
            .await
            .unwrap();
        let decided = out.answers[0].as_ref().unwrap();
        assert_eq!(decided.answer.noul(), Some(true));
        assert!(decided.answer.confidence.unwrap() < 1.0, "smoothed");
        assert_eq!(decided.reasons[0].rule, "knn.neighbours");
        assert_eq!(out.remote_calls, 0);
    }

    #[tokio::test]
    async fn abstains_without_enough_close_examples() {
        let one = env("Invoice 12", "Please pay invoice 12 by Friday.");
        let strategy = knn(&[(&one, true)]).await;
        let q = [
            Question::noul("asks", "Asks for something."),
            Question::noul("other", "Never corrected."),
        ];
        let out = strategy.decide(&one, &q).await.unwrap();
        assert_eq!(out.answers, vec![None, None]);
    }

    #[test]
    fn maps_answers_to_options() {
        let score = Question::score("u", "How?", ["a", "b", "c"]);
        assert_eq!(option_index(&score, &AnswerValue::Score(1.6)), Some(2));
        assert_eq!(option_index(&score, &AnswerValue::Score(3.0)), None);
        let choice = Question::choice("s", "Which?", [("x", ""), ("y", "")]);
        assert_eq!(
            option_index(&choice, &AnswerValue::Choice("y".into())),
            Some(1)
        );
        assert_eq!(option_index(&choice, &AnswerValue::Noul(true)), None);
    }
}
