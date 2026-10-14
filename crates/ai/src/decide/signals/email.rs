//! Email-envelope signals: the deterministic triage rules.
//!
//! [`EmailSignals`] answers the triage questions ([`triage_questions`]) from
//! what an email shows on its face: bulk-mail headers (`List-Unsubscribe`,
//! `List-Id`, `Precedence`, `Auto-Submitted`), the sender's address, the
//! attachments, and cue phrases in English, European Portuguese, Spanish and
//! French (requests, deadlines, payments, phishing tells). Every answer
//! carries the [`Reason`]s that produced it, for the UI's "Why?"; it abstains
//! where the signals don't settle a question.
//!
//! The confidences are fixed per rule. They were set from each rule's
//! precision on the `dev` split of the `mail-triage` eval set
//! (`crates/ai/evals/decide/mail-triage/`), and `cargo xtask eval decide`
//! reports how they hold on `test`.

use std::{
    collections::BTreeMap,
    fmt::{self, Write as _},
    sync::LazyLock,
    time::Instant,
};

use campbooks_kernel::text::displayable;
use regex::Regex;
use serde::Deserialize;

use crate::{
    Error,
    decide::{
        DecisionState, Question, QuestionKind,
        strategy::{BoxFuture, Decided, Decisions, Reason, Strategy, local_answer, peaked},
    },
};

/// Question ids of the email triage pass (`conversations.triage`).
pub mod triage {
    /// Which stream (`choice`: primary, updates, receipts, newsletters).
    pub const STREAM: &str = "stream";
    /// The sender expects a written reply (`noul`).
    pub const NEEDS_REPLY: &str = "needs_reply";
    /// The sender asks the recipient to do something (`noul`).
    pub const ASKS_ACTION: &str = "asks_action";
    /// There is a time by which the recipient must act (`noul`).
    pub const HAS_DEADLINE: &str = "has_deadline";
    /// How soon (`score`: none, within days, today).
    pub const URGENCY: &str = "urgency";
    /// Phishing, scam or impersonation (`noul`).
    pub const SUSPICIOUS: &str = "suspicious";
    /// The built-in streams, in option order.
    pub const STREAMS: [&str; 4] = ["primary", "updates", "receipts", "newsletters"];
}

/// The triage questions, worded for a `decide` model. The labels and their
/// meaning follow `crates/ai/evals/decide/mail-triage/LABELLING.md`.
pub fn triage_questions() -> Vec<Question> {
    vec![
        Question::choice(
            triage::STREAM,
            "Which stream does this email belong to?",
            [
                (
                    "primary",
                    "A person writing to the recipient personally (a client, colleague, \
                     supplier's staff, relative), including a person sending an invoice by hand",
                ),
                (
                    "updates",
                    "An automated notification that is not about money: shipping, account or \
                     security alerts, appointments, password resets, auto-replies",
                ),
                (
                    "receipts",
                    "Automated money mail from a system: receipts, order confirmations with an \
                     amount, invoices or bills from a service, payment confirmations or failures",
                ),
                (
                    "newsletters",
                    "Content or marketing sent to a list: newsletters, promotions, digests, \
                     announcements",
                ),
            ],
        ),
        Question::noul(
            triage::NEEDS_REPLY,
            "The sender expects a written reply from the recipient.",
        ),
        Question::noul(
            triage::ASKS_ACTION,
            "The sender asks the recipient to do something, such as reply, pay, sign, send, \
             review, confirm or renew. Marketing calls to action and \"if this wasn't you\" notes \
             do not count.",
        ),
        Question::noul(
            triage::HAS_DEADLINE,
            "The email gives a time by which the recipient must act, or after which something \
             affecting them lapses. A meeting time alone or a sale ending does not count.",
        ),
        Question::score(
            triage::URGENCY,
            "How soon does the recipient need to act?",
            [
                "No action, or no time pressure",
                "Within the next days",
                "Today: within 24 hours, overdue, or urgent",
            ],
        ),
        Question::noul(
            triage::SUSPICIOUS,
            "The email is phishing, a scam or an impersonation (a look-alike sender domain, \
             credentials or payment asked for through a link, an urgent transfer or gift cards).",
        ),
    ]
}

/// A sender or recipient.
#[derive(Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Party {
    /// Display name; may be empty.
    #[serde(default)]
    pub name: String,
    /// Email address.
    pub address: String,
}

/// An attachment's name and media type (never its content).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Attachment {
    /// File name.
    pub name: String,
    /// Media type (`application/pdf`).
    pub content_type: String,
}

/// What triage reads of an email: the envelope, the headers that matter and
/// the plain-text body. Its `Debug` output never includes content.
#[derive(Clone, Default, PartialEq, Deserialize)]
pub struct Envelope {
    /// Sender.
    pub from: Party,
    /// Recipients.
    #[serde(default)]
    pub to: Vec<Party>,
    /// Copied recipients.
    #[serde(default)]
    pub cc: Vec<Party>,
    /// Subject.
    #[serde(default)]
    pub subject: String,
    /// Headers the provider exposed (`List-Unsubscribe`, `Auto-Submitted`,
    /// `Reply-To`, …). Missing means not given, not "no".
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// Attachments.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// Plain-text body, quoted history included.
    #[serde(default)]
    pub body: String,
}

impl fmt::Debug for Party {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Party(..)")
    }
}

impl fmt::Debug for Envelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Envelope({} headers, {} attachments, {} body chars)",
            self.headers.len(),
            self.attachments.len(),
            self.body.chars().count()
        )
    }
}

/// Headers that say a message went to a list or was generated by a machine.
const BULK_HEADERS: [&str; 4] = [
    "list-unsubscribe",
    "list-id",
    "precedence",
    "auto-submitted",
];
/// Body characters a `decide` state keeps.
const STATE_BODY_CHARS: usize = 1500;

impl Envelope {
    /// A header's value, by case-insensitive name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The compact state a `decide` model reads (ADR 0009): sender,
    /// recipients, subject, the bulk headers present, the attachment list and
    /// the first part of the body.
    pub fn decision_state(&self) -> DecisionState {
        let mut out = format!("From: {}\n", party(&self.from));
        let to: Vec<String> = self.to.iter().chain(&self.cc).map(party).collect();
        if !to.is_empty() {
            let _ = writeln!(out, "To: {}", to.join(", "));
        }
        let _ = writeln!(out, "Subject: {}", self.subject);
        for name in BULK_HEADERS.iter().chain(&["reply-to"]) {
            if let Some(value) = self.header(name) {
                let _ = writeln!(out, "{name}: {}", truncate(value, 120));
            }
        }
        if !self.attachments.is_empty() {
            let list: Vec<String> = self
                .attachments
                .iter()
                .map(|a| format!("{} ({})", a.name, a.content_type))
                .collect();
            let _ = writeln!(out, "Attachments: {}", list.join(", "));
        }
        out.push('\n');
        out.push_str(&truncate(&self.body, STATE_BODY_CHARS));
        DecisionState::Text(out)
    }

    /// The text a k-NN embedder reads: sender, subject, header names,
    /// attachment names and the new (unquoted) text.
    pub fn embedding_text(&self) -> String {
        let headers: Vec<&str> = self.headers.keys().map(String::as_str).collect();
        let attachments: Vec<&str> = self.attachments.iter().map(|a| a.name.as_str()).collect();
        format!(
            "{}\n{}\n{}\n{}\n{}",
            party(&self.from),
            self.subject,
            headers.join(" "),
            attachments.join(" "),
            truncate(&new_text(&self.body), STATE_BODY_CHARS)
        )
    }
}

fn party(p: &Party) -> String {
    if p.name.is_empty() {
        p.address.clone()
    } else {
        format!("{} <{}>", p.name, p.address)
    }
}

fn truncate(text: &str, chars: usize) -> String {
    match text.char_indices().nth(chars) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}

/// The body without quoted history: stops at the first reply or forward
/// marker and drops `>` lines.
fn new_text(body: &str) -> String {
    let mut out = String::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if QUOTE_MARKER.is_match(trimmed) {
            break;
        }
        if trimmed.starts_with('>') {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

// --- Cue phrases -----------------------------------------------------------

fn regex(pattern: &str) -> Regex {
    match Regex::new(pattern) {
        Ok(re) => re,
        Err(err) => panic!("built-in signal pattern does not compile: {err}"),
    }
}

/// A case-insensitive match of any phrase, on word boundaries.
fn phrases(list: &[&str]) -> Regex {
    let alternatives: Vec<String> = list.iter().map(|p| regex::escape(p)).collect();
    regex(&format!(
        r"(?i)(?:^|\W)(?:{})(?:$|\W)",
        alternatives.join("|")
    ))
}

static QUOTE_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(?i)^(?:on .{3,80} wrote:|em .{3,80} escreveu:|el .{3,80} escribió:|le .{3,80} a écrit ?:|-{2,}\s*(?:original message|mensagem original|mensaje original|message d'origine|forwarded message|mensagem encaminhada|mensaje reenviado|message transféré)\s*-{2,}|(?:from|de|von)\s*:\s.+@.+)$",
    )
});

static AUTOMATED_LOCAL: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(?i)^(?:no-?reply|do-?not-?reply|nao-?responder|naoresponder|no-?responder|ne-?pas-?repondre|notifica(?:tions?|coes|ciones)|alerts?|alertas?|info|news(?:letter)?|novidades|noticias|actualites|billing|invoices?|faturas?|facturas?|factur(?:acion|ation)|faturacao|mailer|mailer-daemon|accounts?|conta|cuenta|compte|security|seguranca|seguridad|securite|updates?|marketing|promo(?:tions?|s)?|hello|ola|hola|bonjour|team|equipa|equipo|equipe|orders?|encomendas|pedidos|commandes|receipts?|recibos|recus|payments?|pagamentos|pagos|paiements|support|suporte|soporte|service|servicos|servicios|clientes?|customer\w*|events?|eventos|evenements|digest|reminders?|lembretes|recordatorios|rappels|calendar|calendario|agenda|shipping|envios|expedition|livraison|entregas|citas|rdv|rendez-?vous|marcacoes|avisos|cobran(?:zas|cas)|cobros|contas|cuentas|comptes|alertes|factures|reservas|reservations|bookings?|tickets|bilheteira|entradas|monitor(?:ing|izacao)?|status|system|sistema|systeme)(?:[._+-].*)?$",
    )
});

static OUT_OF_OFFICE: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "out of office",
        "automatic reply",
        "auto-reply",
        "autoreply",
        "away from the office",
        "ausência",
        "resposta automática",
        "fora do escritório",
        "ausente",
        "fuera de la oficina",
        "respuesta automática",
        "absence",
        "réponse automatique",
        "absent du bureau",
    ])
});

static CALENDAR_INVITE: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(?i)^(?:invitation|updated invitation|convite|convite atualizado|invitación|invitación actualizada|invitation mise à jour)\s?:",
    )
});

static UNSUBSCRIBE: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "unsubscribe",
        "manage your preferences",
        "manage preferences",
        "you are receiving this",
        "you received this email because",
        "cancelar subscrição",
        "cancelar a subscrição",
        "anular subscrição",
        "deixar de receber",
        "gerir preferências",
        "darse de baja",
        "darte de baja",
        "date de baja",
        "cancelar suscripción",
        "dejar de recibir",
        "gestionar preferencias",
        "se désabonner",
        "se désinscrire",
        "désinscription",
        "désabonnement",
        "gérer vos préférences",
        "ne plus recevoir",
    ])
});

static MONEY_DOC: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "receipt",
        "invoice",
        "bill",
        "order",
        "your order",
        "order confirmation",
        "paid",
        "payment",
        "refund",
        "statement",
        "renewal",
        "recibo",
        "fatura",
        "factura",
        "encomenda",
        "pagamento",
        "reembolso",
        "extrato",
        "pedido",
        "pago",
        "extracto",
        "cobro",
        "reçu",
        "facture",
        "commande",
        "paiement",
        "remboursement",
        "relevé",
        "prélèvement",
        "débito",
        "cobrança",
    ])
});

static AMOUNT: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"(?i)(?:[€£$]\s?\d|\d[\d.,\u{a0} ]*\s?(?:€|eur\b|euros?\b|£|gbp\b|usd\b))")
});

static PAID: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "thank you for your payment",
        "thanks for your payment",
        "payment received",
        "we received your payment",
        "we have received your payment",
        "has been paid",
        "payment successful",
        "paid in full",
        "thank you for your order",
        "thanks for your order",
        "thank you for your purchase",
        "obrigado pelo seu pagamento",
        "obrigado pelo pagamento",
        "pagamento recebido",
        "recebemos o seu pagamento",
        "pagamento efetuado com sucesso",
        "pagamento confirmado",
        "obrigado pela sua encomenda",
        "obrigado pela sua compra",
        "gracias por tu pago",
        "gracias por su pago",
        "pago recibido",
        "hemos recibido tu pago",
        "hemos recibido su pago",
        "pago realizado",
        "pago confirmado",
        "gracias por tu pedido",
        "gracias por tu compra",
        "merci pour votre paiement",
        "paiement reçu",
        "nous avons bien reçu votre paiement",
        "paiement confirmé",
        "paiement effectué",
        "merci pour votre commande",
        "merci pour votre achat",
    ])
});

static DUE: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "amount due",
        "due date",
        "due on",
        "due by",
        "payment due",
        "is due",
        "payment failed",
        "could not process your payment",
        "was declined",
        "outstanding",
        "overdue",
        "please pay",
        "valor em dívida",
        "data de vencimento",
        "data limite de pagamento",
        "vence a",
        "vence no dia",
        "a pagar até",
        "pagamento falhou",
        "não foi possível processar",
        "foi recusado",
        "em atraso",
        "por pagar",
        "importe pendiente",
        "fecha de vencimiento",
        "vence el",
        "a pagar antes",
        "pago fallido",
        "no hemos podido procesar",
        "ha sido rechazado",
        "pendiente de pago",
        "vencida",
        "montant dû",
        "date d'échéance",
        "à régler avant",
        "à payer avant",
        "à régler",
        "échec du paiement",
        "n'a pas pu être traité",
        "a été refusé",
        "impayé",
        "en attente de paiement",
    ])
});

/// A subject that says something of yours needs fixing: a security alert, a
/// vulnerability found, an action required. Read only in the subject of a
/// service's mail that names its own domain (see [`security_alert`]).
static SECURITY_ALERT: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "security alert",
        "security advisory",
        "vulnerability",
        "vulnerabilities",
        "vulnerable dependency",
        "dependabot alert",
        "action required",
        "alerta de segurança",
        "aviso de segurança",
        "vulnerabilidade",
        "vulnerabilidades",
        "ação necessária",
        "alerta de seguridad",
        "aviso de seguridad",
        "vulnerabilidad",
        "acción requerida",
        "acción necesaria",
        "alerte de sécurité",
        "avis de sécurité",
        "vulnérabilité",
        "vulnérabilités",
        "action requise",
    ])
});

static NOTIFICATION: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "shipped",
        "on its way",
        "delivery",
        "delivered",
        "tracking",
        "password",
        "sign-in",
        "new login",
        "verification code",
        "appointment",
        "reminder",
        "your account",
        "is ready",
        "enviada",
        "a caminho",
        "entrega",
        "entregue",
        "palavra-passe",
        "início de sessão",
        "código de verificação",
        "consulta",
        "marcação",
        "lembrete",
        "a sua conta",
        "está disponível",
        "envío",
        "en camino",
        "entregado",
        "contraseña",
        "inicio de sesión",
        "código de verificación",
        "cita",
        "recordatorio",
        "tu cuenta",
        "ya está disponible",
        "expédié",
        "en cours de livraison",
        "livraison",
        "livré",
        "mot de passe",
        "connexion",
        "code de vérification",
        "rendez-vous",
        "rappel",
        "votre compte",
        "est disponible",
    ])
});

static MARKETING: LazyLock<Regex> = LazyLock::new(|| {
    regex(&format!(
        r"(?i)(?:-\s?\d{{1,2}}\s?%|\d{{1,2}}\s?%\s?(?:off|de desconto|de descuento|de réduction)|{})",
        phrases(&[
            "sale",
            "discount",
            "offer",
            "promo",
            "newsletter",
            "webinar",
            "new collection",
            "limited time",
            "shop now",
            "desconto",
            "promoção",
            "oferta",
            "novidades",
            "campanha",
            "descuento",
            "rebajas",
            "promoción",
            "novedades",
            "réduction",
            "soldes",
            "offre",
            "nouveautés",
        ])
        .as_str()
    ))
});

static REQUEST: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "please",
        "could you",
        "can you",
        "would you",
        "would you mind",
        "kindly",
        "let me know",
        "i need you to",
        "make sure",
        "remember to",
        "don't forget",
        "you need to",
        "you must",
        "action required",
        "please confirm",
        "send me",
        "por favor",
        "pode",
        "poderia",
        "podia",
        "consegue",
        "conseguia",
        "podes",
        "agradecia",
        "agradeço que",
        "peço que",
        "preciso que",
        "não se esqueça",
        "não te esqueças",
        "confirme",
        "envie",
        "envia",
        "assine",
        "pague",
        "diga-me",
        "diz-me",
        "avise",
        "tem de",
        "terá de",
        "é necessário",
        "quando puder",
        "quando puderes",
        "puedes",
        "podrías",
        "podéis",
        "podría",
        "te agradecería",
        "le agradecería",
        "os agradecería",
        "necesito que",
        "no olvides",
        "recuerda",
        "confirma",
        "confírmame",
        "envía",
        "envíame",
        "firma",
        "paga",
        "dime",
        "avísame",
        "tienes que",
        "hay que",
        "es necesario",
        "cuando puedas",
        "merci de",
        "pourriez-vous",
        "pouvez-vous",
        "peux-tu",
        "pourrais-tu",
        "tu peux",
        "je vous prie",
        "veuillez",
        "n'oubliez pas",
        "n'oublie pas",
        "il faut",
        "vous devez",
        "tu dois",
        "confirmez",
        "envoyez",
        "envoie",
        "signez",
        "réglez",
        "payez",
        "dites-moi",
        "dis-moi",
        "tenez-moi au courant",
        "si tu as un moment",
        "quand tu peux",
        "quand vous pouvez",
    ])
});

static REPLY_ASK: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "let me know",
        "please confirm",
        "can you confirm",
        "could you confirm",
        "what do you think",
        "get back to me",
        "are you available",
        "does that work",
        "your thoughts",
        "diga-me",
        "diz-me",
        "confirme",
        "confirma-me",
        "o que acha",
        "o que achas",
        "está disponível",
        "estás disponível",
        "dá-me notícias",
        "aguardo resposta",
        "aguardo a sua resposta",
        "dime",
        "dígame",
        "confírmame",
        "qué te parece",
        "qué opinas",
        "estás disponible",
        "me dices",
        "quedo a la espera",
        "espero tu respuesta",
        "dites-moi",
        "dis-moi",
        "confirmez-moi",
        "qu'en pensez-vous",
        "qu'en penses-tu",
        "êtes-vous disponible",
        "tu es dispo",
        "tiens-moi au courant",
        "dans l'attente de votre réponse",
        "j'attends ta réponse",
    ])
});

static CONDITIONAL: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "if this wasn't you",
        "if this was not you",
        "if you did not request",
        "if you didn't request",
        "if you have any questions",
        "if you did not make",
        "se não foi você",
        "se não foi o senhor",
        "caso não tenha sido",
        "se não pediu",
        "se não reconhece",
        "se tiver alguma dúvida",
        "si no has sido tú",
        "si no fuiste tú",
        "si no lo solicitaste",
        "si no has solicitado",
        "si no reconoces",
        "si tienes alguna duda",
        "si ce n'était pas vous",
        "si vous n'êtes pas à l'origine",
        "si vous n'avez pas demandé",
        "si vous ne reconnaissez pas",
        "si vous avez des questions",
    ])
});

static CONDITIONAL_START: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"(?i)^(?:if|should|in case|se|caso|si|en caso de|au cas où|s'il|s'ils|s'elle)\b")
});

static POLITE_MODAL: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "could you",
        "would you",
        "can you",
        "pourriez-vous",
        "pouvez-vous",
        "peux-tu",
        "pourrais-tu",
        "tu peux",
        "vous pouvez",
        "poderia",
        "podia",
        "pode",
        "podes",
        "consegue",
        "podrías",
        "puedes",
        "podría",
        "podéis",
    ])
});

static NOTHING_NEEDED: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "no need",
        "nothing to do",
        "no action needed",
        "no action required",
        "não é necessário",
        "não é preciso",
        "não precisa",
        "não há nada a fazer",
        "no hace falta",
        "no es necesario",
        "no hay que",
        "no tienes que hacer nada",
        "pas besoin",
        "rien à faire",
        "aucune action",
        "n'avez rien à faire",
        "inutile de",
        "not urgent",
        "no rush",
        "nada de muito urgente",
        "nada urgente",
        "sem pressa",
        "no es urgente",
        "sin prisa",
        "pas urgent",
        "rien d'urgent",
        "aucune urgence",
        "pas pressé",
    ])
});

/// Payment terms on a bill, from a person or a system.
static PAYMENT_TERMS: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(?i)(?:amount to pay|payment within|terms are \d+|falls due|montant à payer|date limite de paiement|échéance|paiement sous|merci de procéder au règlement|pagamento a \d+ dias|vencimento|data limite|pague até|vencimiento|a pagar|el pago antes|te agradezco el pago|valor a pagar)",
    )
});

/// Imperatives that ask for an action now (pay, renew, verify, update).
static ACTION_NOW: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "pay now",
        "settle",
        "renew",
        "update your card",
        "update your payment",
        "verify your",
        "confirm your",
        "action required",
        "pague",
        "regularize",
        "renove",
        "atualize",
        "confirme a sua",
        "paga ahora",
        "regulariza",
        "renueva",
        "actualiza",
        "confirma tu",
        "réglez",
        "régularisez",
        "renouvelez",
        "pour renouveler",
        "mettez à jour",
        "confirmez",
        "action requise",
    ])
});

static DEADLINE_CUE: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "by",
        "until",
        "before",
        "no later than",
        "due",
        "deadline",
        "expires",
        "expiry",
        "at the latest",
        "até",
        "antes de",
        "o mais tardar",
        "prazo",
        "vence",
        "vencimento",
        "expira",
        "data limite",
        "hasta",
        "antes del",
        "a más tardar",
        "plazo",
        "vencimiento",
        "caduca",
        "fecha límite",
        "avant",
        "jusqu'au",
        "au plus tard",
        "d'ici",
        "échéance",
        "expire",
        "date limite",
    ])
});

static DATE_EXPR: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(?i)(?:\b\d{1,2}[/.-]\d{1,2}(?:[/.-]\d{2,4})?\b|\b\d{4}-\d{2}-\d{2}\b|\b(?:january|february|march|april|may|june|july|august|september|october|november|december|janeiro|fevereiro|março|abril|maio|junho|julho|agosto|setembro|outubro|novembro|dezembro|enero|febrero|marzo|mayo|junio|julio|septiembre|octubre|noviembre|diciembre|janvier|février|mars|avril|mai|juin|juillet|août|septembre|octobre|novembre|décembre)\b|\b(?:monday|tuesday|wednesday|thursday|friday|saturday|sunday|segunda|terça|quarta|quinta|sexta|sábado|domingo|lunes|martes|miércoles|jueves|viernes|lundi|mardi|mercredi|jeudi|vendredi|samedi|dimanche)\b|\b(?:today|tomorrow|tonight|end of (?:the )?(?:day|week|month)|next week|this week|hoje|amanhã|fim do dia|fim da semana|fim do mês|final do mês|próxima semana|esta semana|hoy|mañana|esta semana|la semana que viene|la próxima semana|final de mes|fin de mes|finales de mes|aujourd'hui|demain|ce soir|fin de (?:la )?(?:journée|semaine)|fin du mois|semaine prochaine|cette semaine)\b|\b(?:end of (?:this|next) week|end of (?:this|next) month|this month|before the (?:holidays|break)|fim (?:desta|da próxima) semana|fim deste mês|este mês|antes das férias|final de (?:esta|la próxima) semana|finales de (?:este|mes)|a finales de mes|este mes|antes de (?:las|las próximas) vacaciones|fin de (?:cette|la) semaine(?: prochaine)?|ce mois-ci|avant les vacances)\b|\b(?:within|in|dentro de|nas próximas|em|en|en las próximas|sous|dans|dans les|d'ici)\s+\d+\s*(?:h\b|hours?|days?|horas|dias|días|heures|jours))",
    )
});

static TODAY: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "today",
        "urgent",
        "urgently",
        "asap",
        "as soon as possible",
        "immediately",
        "right away",
        "tonight",
        "end of day",
        "overdue",
        "hoje",
        "urgente",
        "com urgência",
        "o quanto antes",
        "imediatamente",
        "ainda hoje",
        "em atraso",
        "hoy",
        "cuanto antes",
        "inmediatamente",
        "lo antes posible",
        "esta tarde",
        "vencida",
        "aujourd'hui",
        "urgence",
        "au plus vite",
        "immédiatement",
        "dès que possible",
        "ce soir",
        "en retard",
    ])
});

static PHISHING_ASK: LazyLock<Regex> = LazyLock::new(|| {
    phrases(&[
        "verify your account",
        "verify your identity",
        "confirm your identity",
        "confirm your details",
        "update your payment",
        "update your details",
        "unusual activity",
        "suspended",
        "locked",
        "gift card",
        "gift cards",
        "wire transfer",
        "click here",
        "log in here",
        "small fee",
        "customs fee",
        "verifique a sua conta",
        "confirme os seus dados",
        "atualize os seus dados",
        "atividade invulgar",
        "atividade suspeita",
        "suspensa",
        "bloqueada",
        "cartão-oferta",
        "cartões-oferta",
        "cartões-presente",
        "transferência urgente",
        "clique aqui",
        "taxa alfandegária",
        "verifica tu cuenta",
        "confirma tus datos",
        "actualiza tus datos",
        "actividad inusual",
        "actividad sospechosa",
        "suspendida",
        "bloqueado",
        "tarjetas regalo",
        "tarjetas de regalo",
        "transferencia urgente",
        "haz clic aquí",
        "pulsa aquí",
        "tasa de aduana",
        "vérifiez votre compte",
        "confirmez vos informations",
        "mettez à jour vos informations",
        "activité inhabituelle",
        "activité suspecte",
        "suspendu",
        "bloqué",
        "cartes cadeaux",
        "virement urgent",
        "cliquez ici",
        "frais de douane",
    ])
});

/// Sender domains shaped like an impersonation: digits for letters
/// (`atlant1co`), or a brand followed by a service word
/// (`bancoatlantico-seguranca`, `mercadopan-invoices`), which genuine senders
/// put in a subdomain instead.
static LOOKALIKE_DOMAIN: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(?i)(?:[a-z][0-9][a-z]|^[a-z0-9]+(?:-[a-z0-9]+)*-(?:invoices?|billing|payments?|pay|pagos?|pagamentos?|faturas?|facturas?|factures?|faturacao|facturacion|facturation|suivi|tracking|rastreio|seguimiento|envios|direccion|direcao|direction|secure|security|seguranca|seguridad|securite|verify|verificacao|verificacion|verification|login|alerts?|alertas|alertes|account|accounts|conta|cuenta|compte|support|suporte|soporte|online|taxa|tasas|frais|douane|aduana|notices?|avisos|cobros|orders|pedidos|clientes?|customer|services?|servicos|servicios)(?:\.|$))",
    )
});

static WITHIN_HOURS: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(?i)\b(?:within|in|en|em|dentro de|nas próximas|en las próximas|sous|dans|dans les|d'ici)\s+(\d+)\s*(?:h\b|hours?|horas|heures)",
    )
});

static LINK_HOST: LazyLock<Regex> = LazyLock::new(|| regex(r#"(?i)https?://([^/\s"'<>)]+)"#));

/// Two-label public suffixes the registrable-domain guess knows.
const SUFFIXES: [&str; 6] = [
    "example.com",
    "example.org",
    "example.net",
    "co.uk",
    "com.pt",
    "com.es",
];

/// A best-effort registrable domain (`conta.bancoatlantico.example` →
/// `bancoatlantico.example`).
fn registrable(host: &str) -> String {
    let host = host.trim_end_matches('.').to_lowercase();
    let host = host.split(':').next().unwrap_or_default();
    let labels: Vec<&str> = host.split('.').collect();
    let keep = if SUFFIXES
        .iter()
        .any(|s| host == *s || host.ends_with(&format!(".{s}")))
    {
        3
    } else {
        2
    };
    labels[labels.len().saturating_sub(keep)..].join(".")
}

fn domain(address: &str) -> &str {
    address.rsplit_once('@').map_or("", |(_, d)| d)
}

fn local_part(address: &str) -> &str {
    address.split_once('@').map_or(address, |(l, _)| l)
}

fn first_match(re: &Regex, text: &str) -> Option<String> {
    re.find(text).map(|m| {
        m.as_str()
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_owned()
    })
}

/// The sentences of `text` that can carry a request: drops conditional ones
/// ("if you have questions, let me know") unless they hold a polite request
/// ("if you have a moment, could you…"), and ones that say nothing is needed
/// ("no need to reply").
fn actionable_text(text: &str) -> String {
    let mut out = String::new();
    for sentence in text.split_inclusive(['.', '!', '?', '\n']) {
        let trimmed = sentence.trim_start().to_lowercase();
        let polite = POLITE_MODAL.is_match(&trimmed);
        if (CONDITIONAL_START.is_match(&trimmed) && !polite) || NOTHING_NEEDED.is_match(&trimmed) {
            continue;
        }
        out.push_str(sentence);
    }
    out
}

// --- What the signals read --------------------------------------------------

/// Everything the rules look at, read once per envelope.
struct Read {
    bulk: Option<String>,
    auto_reply: Option<String>,
    automated_sender: Option<String>,
    invite: Option<String>,
    unsubscribe: Option<String>,
    money: Option<String>,
    amount: bool,
    paid: Option<String>,
    due: Option<String>,
    notification: Option<String>,
    security_alert: Option<String>,
    marketing: Option<String>,
    request: Option<String>,
    reply_ask: Option<String>,
    question: bool,
    conditional: Option<String>,
    phishing_ask: Option<String>,
    deadline: Option<String>,
    date: bool,
    today: Option<String>,
    phishing: Vec<Reason>,
}

/// The header that marks list mail, if any.
fn bulk_header(env: &Envelope) -> Option<String> {
    env.headers
        .iter()
        .find(|(k, v)| match k.to_ascii_lowercase().as_str() {
            "list-unsubscribe" | "list-id" => true,
            "precedence" => {
                let v = v.to_ascii_lowercase();
                v.contains("bulk") || v.contains("list") || v.contains("junk")
            }
            _ => false,
        })
        .map(|(k, _)| k.clone())
}

/// Whether a machine wrote it: `(auto-reply, automated sender)`.
fn automation(env: &Envelope, subject: &str) -> (Option<String>, Option<String>) {
    let auto_submitted = env
        .header("auto-submitted")
        .map(str::to_ascii_lowercase)
        .filter(|v| v != "no");
    let auto_reply = auto_submitted
        .as_deref()
        .filter(|v| v.contains("auto-replied"))
        .map(|_| "Auto-Submitted: auto-replied".to_owned())
        .or_else(|| first_match(&OUT_OF_OFFICE, subject));
    let automated_sender = if AUTOMATED_LOCAL.is_match(local_part(&env.from.address)) {
        Some(env.from.address.clone())
    } else {
        auto_submitted.map(|v| format!("Auto-Submitted: {v}"))
    };
    (auto_reply, automated_sender)
}

/// Phishing tells, plus the action and payment asks they look at:
/// `(tells, action asked, payment due)`.
fn phishing_tells(
    env: &Envelope,
    body: &str,
    head: &str,
) -> (Vec<Reason>, Option<String>, Option<String>) {
    let from_domain = registrable(domain(&env.from.address));
    let mut phishing = Vec::new();
    if let Some(reply_to) = env.header("reply-to") {
        let reply_domain = registrable(domain(reply_to.trim_matches(['<', '>', ' '])));
        if !reply_domain.is_empty() && reply_domain != from_domain {
            phishing.push(Reason::new("signals.reply_to_mismatch", reply_domain));
        }
    }
    if LOOKALIKE_DOMAIN.is_match(&from_domain) {
        phishing.push(Reason::new("signals.lookalike_domain", from_domain.clone()));
    }
    let phishing_ask = first_match(&PHISHING_ASK, head);
    let due = first_match(&DUE, head).or_else(|| first_match(&PAYMENT_TERMS, head));
    let urgent = first_match(&TODAY, head).or_else(|| first_match(&WITHIN_HOURS, head));
    let foreign_link = LINK_HOST
        .captures_iter(body)
        .filter_map(|c| c.get(1))
        .map(|m| registrable(m.as_str()))
        .find(|host| *host != from_domain);
    if let Some(host) = &foreign_link
        && (phishing_ask.is_some() || due.is_some())
    {
        phishing.push(Reason::new("signals.link_elsewhere", host.clone()));
    }
    if let Some(ask) = &phishing_ask {
        phishing.push(Reason::new("signals.phishing_phrase", ask.clone()));
    }
    let phishing_ask = phishing_ask.or_else(|| first_match(&ACTION_NOW, head));
    if let (Some(urgent), Some(_)) = (&urgent, phishing_ask.as_ref().or(due.as_ref())) {
        phishing.push(Reason::new("signals.pressure", urgent.clone()));
    }

    (phishing, phishing_ask, due)
}

/// Whether the sender's display name names its own domain: "GitHub" at
/// `github.com`, not "GitHub" at `github-alerts.example`. Names and domain
/// labels compare letters and digits only.
fn names_own_domain(from: &Party) -> bool {
    let squash = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    };
    let host = registrable(domain(&from.address));
    let label = squash(host.split('.').next().unwrap_or_default());
    if label.len() < 3 {
        return false;
    }
    squash(&from.name) == label || from.name.split_whitespace().any(|w| squash(w) == label)
}

/// The security alert words of a service's notification about something of
/// the recipient's (a vulnerability in their code, an advisory on their
/// account): only from a service sender (list or automated address) whose
/// display name names its own domain, with no phishing tell and no
/// marketing words ("action required: claim your discount" stays a
/// newsletter).
fn security_alert(
    env: &Envelope,
    subject: &str,
    service: bool,
    marketing: Option<&str>,
    phishing: &[Reason],
) -> Option<String> {
    if !service || marketing.is_some() || !phishing.is_empty() || !names_own_domain(&env.from) {
        return None;
    }
    first_match(&SECURITY_ALERT, subject)
}

/// A "today" cue in the subject or the actionable text, or a window of 24
/// hours or less.
fn today(subject: &str, actionable: &str) -> Option<String> {
    first_match(&TODAY, &format!("{subject}\n{actionable}")).or_else(|| {
        WITHIN_HOURS
            .captures_iter(actionable)
            .find(|c| {
                c.get(1)
                    .and_then(|n| n.as_str().parse::<u32>().ok())
                    .is_some_and(|n| n <= 24)
            })
            .map(|c| c[0].to_owned())
    })
}

impl Read {
    fn new(env: &Envelope) -> Self {
        let body = env.body.replace('\u{2019}', "'");
        let fresh = new_text(&body);
        let subject = env.subject.replace('\u{2019}', "'");
        let head = format!("{subject}\n{fresh}");
        let attachments: String = env
            .attachments
            .iter()
            .map(|a| a.name.replace(['_', '-', '.'], " "))
            .collect::<Vec<_>>()
            .join(" ");

        let bulk = bulk_header(env);
        let (auto_reply, automated_sender) = automation(env, &subject);
        let (phishing, phishing_ask, due) = phishing_tells(env, &body, &head);

        let actionable = actionable_text(&fresh);
        let deadline = fresh
            .split(['.', '!', '?', '\n'])
            .chain(std::iter::once(subject.as_str()))
            .find(|sentence| DEADLINE_CUE.is_match(sentence) && DATE_EXPR.is_match(sentence))
            .and_then(|sentence| first_match(&DATE_EXPR, sentence));

        let invite = env
            .attachments
            .iter()
            .find(|a| a.content_type.eq_ignore_ascii_case("text/calendar"))
            .map(|a| a.name.clone())
            .or_else(|| first_match(&CALENDAR_INVITE, &subject));
        let marketing = first_match(&MARKETING, &head);
        let security_alert = security_alert(
            env,
            &subject,
            bulk.is_some() || automated_sender.is_some(),
            marketing.as_deref(),
            &phishing,
        );
        Self {
            bulk,
            auto_reply,
            automated_sender,
            invite,
            unsubscribe: first_match(&UNSUBSCRIBE, &body),
            money: first_match(&MONEY_DOC, &format!("{subject}\n{attachments}")),
            amount: AMOUNT.is_match(&fresh),
            paid: first_match(&PAID, &head),
            due,
            notification: first_match(&NOTIFICATION, &subject),
            security_alert,
            marketing,
            request: first_match(&REQUEST, &actionable),
            reply_ask: first_match(&REPLY_ASK, &actionable),
            question: actionable.contains('?'),
            phishing_ask,
            conditional: first_match(&CONDITIONAL, &fresh),
            deadline,
            date: DATE_EXPR.is_match(&head),
            today: today(&subject, &actionable),
            phishing,
        }
    }

    fn automated(&self) -> bool {
        self.bulk.is_some()
            || self.automated_sender.is_some()
            || self.invite.is_some()
            || self.auto_reply.is_some()
            || self.unsubscribe.is_some()
    }
}

// --- The rules ----------------------------------------------------------------

/// Rule confidences: each rule's precision on the `dev` split of
/// `mail-triage` v1, smoothed as (correct + 1) / (answers + 2) so a rule seen
/// a few times is not trusted blindly. `cargo xtask eval decide` checks them
/// on `test`.
mod conf {
    pub const AUTO_REPLY: f64 = 0.8;
    pub const BULK_RECEIPT: f64 = 0.7;
    pub const BULK_UPDATE: f64 = 0.7;
    pub const BULK_NEWSLETTER: f64 = 0.97;
    pub const AUTOMATED_RECEIPT: f64 = 0.91;
    pub const AUTOMATED_UPDATE: f64 = 0.93;
    pub const INVITE: f64 = 0.83;
    pub const PERSON: f64 = 0.98;
    pub const SERVICE_NO_REPLY: f64 = 0.99;
    pub const REPLY_ASKED: f64 = 0.9;
    pub const REPLY_QUESTION: f64 = 0.88;
    pub const NO_REPLY_ASKED: f64 = 0.96;
    pub const MARKETING_NO_ACTION: f64 = 0.97;
    pub const PAID_NO_ACTION: f64 = 0.92;
    pub const DUE_ACTION: f64 = 0.89;
    pub const SERVICE_ASKS: f64 = 0.93;
    pub const CONDITIONAL_NO_ACTION: f64 = 0.75;
    pub const NOTICE_NO_ACTION: f64 = 0.95;
    pub const PERSON_ASKS: f64 = 0.97;
    pub const PERSON_QUESTION: f64 = 0.83;
    pub const PERSON_NO_ASK: f64 = 0.95;
    pub const DEADLINE: f64 = 0.96;
    pub const NO_DATE: f64 = 0.8;
    pub const NOTHING_ASKED: f64 = 0.95;
    pub const MARKETING_NO_DEADLINE: f64 = 0.93;
    pub const NOT_URGENT: f64 = 0.95;
    /// Not in `mail-triage` v1 yet (no labelled security alerts): a prior,
    /// to be replaced by its measured precision once the set has them.
    pub const SECURITY_ALERT: f64 = 0.9;
    pub const TODAY: f64 = 0.9;
    pub const SUSPICIOUS_CALM: f64 = 0.93;
    pub const DAYS: f64 = 0.92;
    pub const SUSPICIOUS: f64 = 0.93;
    pub const NOT_SUSPICIOUS: f64 = 0.99;
}

type Ruled = Option<(usize, f64, Vec<Reason>)>;

fn stream(r: &Read) -> (usize, f64, Vec<Reason>) {
    if let Some(ooo) = &r.auto_reply {
        return (
            1,
            conf::AUTO_REPLY,
            vec![Reason::new("signals.auto_reply", ooo)],
        );
    }
    let list = r
        .bulk
        .as_ref()
        .map(|h| Reason::new("signals.list_header", h))
        .or_else(|| {
            r.unsubscribe
                .as_ref()
                .map(|u| Reason::new("signals.unsubscribe_text", u))
        });
    let money = r
        .money
        .as_ref()
        .map(|m| Reason::new("signals.money_words", m));
    let notice = r
        .notification
        .as_ref()
        .map(|n| Reason::new("signals.notification_words", n));
    if let Some(alert) = &r.security_alert {
        let alert = Reason::new("signals.security_alert", alert);
        return (
            1,
            conf::SECURITY_ALERT,
            list.into_iter().chain([alert]).collect(),
        );
    }
    if let Some(list) = list {
        return match (money, notice) {
            (Some(money), _) if r.marketing.is_none() => (2, conf::BULK_RECEIPT, vec![list, money]),
            (_, Some(notice)) if r.marketing.is_none() => {
                (1, conf::BULK_UPDATE, vec![list, notice])
            }
            _ => (3, conf::BULK_NEWSLETTER, vec![list]),
        };
    }
    if let Some(invite) = &r.invite {
        return (
            1,
            conf::INVITE,
            vec![Reason::new("signals.calendar_invite", invite)],
        );
    }
    if let Some(sender) = &r.automated_sender {
        let sender = Reason::new("signals.automated_sender", sender);
        // Money words alone ("your order is on its way") are not enough: a
        // receipt shows an amount, or says it is paid or due.
        let money_mail = r.amount || r.paid.is_some() || r.due.is_some();
        return match money {
            Some(money) if money_mail => (2, conf::AUTOMATED_RECEIPT, vec![sender, money]),
            _ => (1, conf::AUTOMATED_UPDATE, vec![sender]),
        };
    }
    (
        0,
        conf::PERSON,
        vec![Reason::new("signals.person_sender", "")],
    )
}

fn needs_reply(r: &Read) -> Ruled {
    if r.automated() {
        return Some((
            0,
            conf::SERVICE_NO_REPLY,
            vec![Reason::new("signals.service_sender", "")],
        ));
    }
    if let Some(ask) = &r.reply_ask {
        return Some((
            1,
            conf::REPLY_ASKED,
            vec![Reason::new("signals.reply_asked", ask)],
        ));
    }
    if r.question {
        return Some((
            1,
            conf::REPLY_QUESTION,
            vec![Reason::new("signals.question", "?")],
        ));
    }
    if r.request.is_some() {
        return None;
    }
    Some((
        0,
        conf::NO_REPLY_ASKED,
        vec![Reason::new("signals.no_question", "")],
    ))
}

fn asks_action(r: &Read) -> Ruled {
    // A service telling you something of yours needs fixing is a to-do,
    // even when it comes through a list.
    if let Some(alert) = &r.security_alert {
        return Some((
            1,
            conf::SECURITY_ALERT,
            vec![Reason::new("signals.security_alert", alert)],
        ));
    }
    let newsletter = (r.bulk.is_some() || r.unsubscribe.is_some()) && r.money.is_none();
    if newsletter && r.notification.is_none() {
        return Some((
            0,
            conf::MARKETING_NO_ACTION,
            vec![Reason::new("signals.list_mail", "")],
        ));
    }
    if let Some(due) = &r.due {
        return Some((
            1,
            conf::DUE_ACTION,
            vec![Reason::new("signals.payment_due", due)],
        ));
    }
    if r.automated() {
        if let Some(paid) = &r.paid {
            return Some((
                0,
                conf::PAID_NO_ACTION,
                vec![Reason::new("signals.already_paid", paid)],
            ));
        }
        if let Some(ask) = &r.phishing_ask {
            return Some((
                1,
                conf::SERVICE_ASKS,
                vec![Reason::new("signals.action_asked", ask)],
            ));
        }
        if let Some(cond) = &r.conditional {
            return Some((
                0,
                conf::CONDITIONAL_NO_ACTION,
                vec![Reason::new("signals.conditional_ask", cond)],
            ));
        }
        if r.request.is_none() {
            return Some((
                0,
                conf::NOTICE_NO_ACTION,
                vec![Reason::new("signals.notice", "")],
            ));
        }
        return None;
    }
    if let Some(ask) = r.reply_ask.as_ref().or(r.request.as_ref()) {
        return Some((
            1,
            conf::PERSON_ASKS,
            vec![Reason::new("signals.request", ask)],
        ));
    }
    if r.question {
        return Some((
            1,
            conf::PERSON_QUESTION,
            vec![Reason::new("signals.question", "?")],
        ));
    }
    Some((
        0,
        conf::PERSON_NO_ASK,
        vec![Reason::new("signals.no_request", "")],
    ))
}

fn has_deadline(r: &Read, asks: &Ruled) -> Ruled {
    if (r.bulk.is_some() || r.unsubscribe.is_some()) && r.money.is_none() && r.marketing.is_some() {
        return Some((
            0,
            conf::MARKETING_NO_DEADLINE,
            vec![Reason::new(
                "signals.marketing",
                r.marketing.clone().unwrap_or_default(),
            )],
        ));
    }
    // A deadline is a time by which the recipient must act: no ask, no
    // deadline (a meeting on Friday in an FYI is not one).
    if let Some((0, c, reasons)) = asks {
        return Some((0, c.min(conf::NOTHING_ASKED), reasons.clone()));
    }
    if let Some(date) = &r.deadline {
        return Some((
            1,
            conf::DEADLINE,
            vec![Reason::new("signals.deadline", date)],
        ));
    }
    if !r.date && r.today.is_none() {
        return Some((0, conf::NO_DATE, vec![Reason::new("signals.no_date", "")]));
    }
    None
}

fn urgency(r: &Read, asks: &Ruled) -> Ruled {
    // Campbooks never rushes anyone on a suspicious message; with a single
    // tell it can't tell either way.
    match r.phishing.len() {
        0 => {}
        1 => return None,
        _ => {
            return Some((0, conf::SUSPICIOUS_CALM, r.phishing.clone()));
        }
    }
    match asks {
        Some((0, c, reasons)) => Some((0, c.min(conf::NOT_URGENT), reasons.clone())),
        Some((1, _, _)) => Some(match &r.today {
            Some(today) => (2, conf::TODAY, vec![Reason::new("signals.today", today)]),
            None => (1, conf::DAYS, vec![Reason::new("signals.no_today", "")]),
        }),
        _ => None,
    }
}

fn suspicious(r: &Read) -> Ruled {
    match r.phishing.len() {
        0 => Some((
            0,
            conf::NOT_SUSPICIOUS,
            vec![Reason::new("signals.no_phishing_tell", "")],
        )),
        1 => None,
        _ => Some((1, conf::SUSPICIOUS, r.phishing.clone())),
    }
}

/// The deterministic email triage rules as a [`Strategy`]. Answers the
/// [`triage`] question ids when the question has the expected shape (the four
/// built-in streams, three urgency levels, `noul` otherwise) and abstains on
/// anything else.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmailSignals;

impl EmailSignals {
    /// Answer synchronously; same result as [`Strategy::decide`].
    pub fn answer(envelope: &Envelope, questions: &[Question]) -> Decisions {
        let started = Instant::now();
        let read = Read::new(envelope);
        let fresh = new_text(&envelope.body);
        let asks = asks_action(&read);
        let answers = questions
            .iter()
            .map(|q| {
                let ruled = match (q.id.as_str(), &q.kind) {
                    (triage::STREAM, QuestionKind::Choice { options, .. }) => {
                        let (i, c, why) = stream(&read);
                        let label = triage::STREAMS[i];
                        options
                            .iter()
                            .position(|o| o.label == label)
                            .map(|index| (index, c, why))
                    }
                    (triage::NEEDS_REPLY, QuestionKind::Noul { .. }) => needs_reply(&read),
                    (triage::ASKS_ACTION, QuestionKind::Noul { .. }) => asks.clone(),
                    (triage::HAS_DEADLINE, QuestionKind::Noul { .. }) => has_deadline(&read, &asks),
                    (triage::URGENCY, QuestionKind::Score { levels, .. }) if levels.len() == 3 => {
                        urgency(&read, &asks)
                    }
                    (triage::SUSPICIOUS, QuestionKind::Noul { .. }) => suspicious(&read),
                    _ => None,
                };
                ruled.map(|(index, confidence, mut reasons)| {
                    for reason in &mut reasons {
                        reason.sentence = quoted_sentence(&fresh, reason);
                    }
                    Decided {
                        answer: local_answer(
                            q,
                            peaked(q.option_count(), index, confidence),
                            "signals",
                            started,
                        ),
                        reasons,
                    }
                })
            })
            .collect();
        Decisions {
            answers,
            remote_calls: 0,
        }
    }
}

/// Rules whose evidence is a phrase from the message's text, so the
/// sentence around it is what the sender wrote.
const QUOTED_RULES: [&str; 10] = [
    "signals.reply_asked",
    "signals.request",
    "signals.question",
    "signals.payment_due",
    "signals.action_asked",
    "signals.deadline",
    "signals.today",
    "signals.phishing_phrase",
    "signals.pressure",
    "signals.security_alert",
];

/// Longest quoted sentence, in characters.
pub const SENTENCE_CHARS: usize = 240;

/// The sentence of `fresh` (the new text, quoted history dropped) holding
/// `reason`'s evidence, whitespace collapsed and cut at a word past
/// [`SENTENCE_CHARS`]; `None` for rules that read headers or addresses, or
/// when the evidence is not in the text (it came from the subject).
fn quoted_sentence(fresh: &str, reason: &Reason) -> Option<String> {
    if !QUOTED_RULES.contains(&reason.rule) || reason.evidence.is_empty() {
        return None;
    }
    let needle = reason.evidence.to_lowercase().replace('\u{2019}', "'");
    let mut start = 0;
    let mut chars = fresh.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        // A full stop ends a sentence only before a space or the end, so
        // "€148.00" and "v3.2" stay whole.
        let next_is_space = chars.peek().is_none_or(|(_, next)| next.is_whitespace());
        let ends = matches!(c, '!' | '?' | '\n') || (c == '.' && next_is_space);
        if !ends {
            continue;
        }
        let end = i + c.len_utf8();
        if let Some(found) = sentence_at(&fresh[start..end], &needle) {
            return Some(found);
        }
        start = end;
    }
    sentence_at(&fresh[start..], &needle)
}

fn sentence_at(candidate: &str, needle: &str) -> Option<String> {
    // Whitespace collapsed first (so lines do not run together), then what
    // could reorder or hide text dropped: bidi controls, zero-width and
    // control characters (`campbooks_kernel::text`).
    let flat = displayable(&candidate.split_whitespace().collect::<Vec<_>>().join(" "));
    let normalised = flat.to_lowercase().replace('\u{2019}', "'");
    if flat.is_empty() || !normalised.contains(needle) {
        return None;
    }
    if flat.chars().count() <= SENTENCE_CHARS {
        return Some(flat);
    }
    let mut cut = String::new();
    for word in flat.split(' ') {
        if cut.chars().count() + word.chars().count() + 1 > SENTENCE_CHARS {
            break;
        }
        if !cut.is_empty() {
            cut.push(' ');
        }
        cut.push_str(word);
    }
    cut.push('…');
    Some(cut)
}

impl Strategy for EmailSignals {
    fn name(&self) -> &'static str {
        "signals"
    }

    fn decide<'a>(
        &'a self,
        envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Decisions, Error>> {
        Box::pin(async move { Ok(Self::answer(envelope, questions)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decide::AnswerValue;

    fn mail(from: &str, subject: &str, body: &str) -> Envelope {
        Envelope {
            from: Party {
                name: String::new(),
                address: from.into(),
            },
            subject: subject.into(),
            body: body.into(),
            ..Envelope::default()
        }
    }

    fn with_header(mut env: Envelope, k: &str, v: &str) -> Envelope {
        env.headers.insert(k.into(), v.into());
        env
    }

    fn ask(env: &Envelope, id: &str) -> Option<Decided> {
        let qs = triage_questions();
        EmailSignals::answer(env, &qs).get(&qs, id).cloned()
    }

    fn value(env: &Envelope, id: &str) -> Option<AnswerValue> {
        ask(env, id).map(|d| d.answer.value)
    }

    fn choice(label: &str) -> AnswerValue {
        AnswerValue::Choice(label.into())
    }

    #[test]
    fn reasons_quote_the_senders_sentence() {
        let env = mail(
            "marta@casa-lume.example",
            "Walkthrough",
            "Hope the move went well. Could you confirm Thursday\u{2019}s walkthrough? Thanks!\n\n\
             On Mon, 12 Oct 2026 Ana wrote:\n> Could you send the plan?",
        );
        let d = ask(&env, triage::NEEDS_REPLY).unwrap();
        assert_eq!(
            d.reasons[0].sentence.as_deref(),
            Some("Could you confirm Thursday\u{2019}s walkthrough?")
        );
    }

    #[test]
    fn amounts_do_not_split_a_sentence() {
        let env = mail(
            "billing@cloudhost.example",
            "Invoice",
            "Your invoice CH-20418 for €148.00 is due on 16 October. Thank you.",
        );
        let d = ask(&env, triage::ASKS_ACTION).unwrap();
        assert_eq!(
            d.reasons[0].sentence.as_deref(),
            Some("Your invoice CH-20418 for €148.00 is due on 16 October.")
        );
    }

    #[test]
    fn quotes_drop_what_reorders_or_hides_text() {
        // An RLO inside an IBAN would show its digits reversed.
        let env = mail(
            "billing@supplier.example",
            "Invoice",
            "Payment due: please pay to IBAN PT50\u{202E}0002 0123\u{200B}4567 by Friday. Thanks.",
        );
        let d = ask(&env, triage::ASKS_ACTION).unwrap();
        let sentence = d.reasons[0].sentence.clone().unwrap();
        assert!(!sentence.contains('\u{202E}') && !sentence.contains('\u{200B}'));
        assert!(sentence.contains("PT500002 01234567"), "{sentence}");
    }

    #[test]
    fn header_reasons_quote_nothing() {
        let env = with_header(
            mail("news@shop.example", "Autumn sale", "Big savings this week."),
            "List-Unsubscribe",
            "<mailto:u@shop.example>",
        );
        let d = ask(&env, triage::STREAM).unwrap();
        assert!(d.reasons.iter().all(|r| r.sentence.is_none()));
    }

    #[test]
    fn long_sentences_are_cut_at_a_word() {
        let long = format!("Could you {} confirm?", "please ".repeat(60));
        let env = mail("jonas@example.com", "Lunch", &long);
        let d = ask(&env, triage::NEEDS_REPLY).unwrap();
        let sentence = d.reasons[0].sentence.clone().unwrap();
        assert!(sentence.chars().count() <= SENTENCE_CHARS + 1);
        assert!(sentence.ends_with('…'));
    }

    #[test]
    fn every_pattern_compiles() {
        for re in [
            &QUOTE_MARKER,
            &AUTOMATED_LOCAL,
            &CALENDAR_INVITE,
            &OUT_OF_OFFICE,
            &UNSUBSCRIBE,
            &MONEY_DOC,
            &AMOUNT,
            &PAID,
            &DUE,
            &NOTIFICATION,
            &MARKETING,
            &REQUEST,
            &REPLY_ASK,
            &CONDITIONAL,
            &DEADLINE_CUE,
            &DATE_EXPR,
            &TODAY,
            &PHISHING_ASK,
            &LOOKALIKE_DOMAIN,
            &LINK_HOST,
            &WITHIN_HOURS,
            &CONDITIONAL_START,
            &NOTHING_NEEDED,
            &POLITE_MODAL,
            &PAYMENT_TERMS,
            &ACTION_NOW,
            &SECURITY_ALERT,
        ] {
            assert!(!re.as_str().is_empty());
        }
    }

    fn named(mut env: Envelope, name: &str) -> Envelope {
        env.from.name = name.into();
        env
    }

    /// A code host's security alert about the recipient's repository: list
    /// mail from the host's own domain.
    fn security_alert_mail(name: &str, from: &str, subject: &str) -> Envelope {
        with_header(
            named(
                mail(
                    from,
                    subject,
                    "Dependabot found a high severity vulnerability in a dependency of \
                     example-org/demo-shop.\n\nReview: https://github.com/example-org/demo-shop/\
                     security/dependabot/42\nUpgrade to 2.4.1",
                ),
                name,
            ),
            "List-Id",
            "<demo-shop.example-org.github.com>",
        )
    }

    #[test]
    fn a_security_alert_from_the_platform_is_a_to_do() {
        let env = security_alert_mail(
            "GitHub",
            "notifications@github.com",
            "[example-org/demo-shop] High severity vulnerability in tiny-template",
        );
        let asks = ask(&env, triage::ASKS_ACTION).unwrap();
        assert_eq!(asks.answer.value, AnswerValue::Noul(true));
        assert_eq!(asks.reasons[0].rule, "signals.security_alert");
        assert!(
            asks.reasons[0]
                .sentence
                .as_deref()
                .is_some_and(|s| s.starts_with("Dependabot found")),
            "{asks:?}"
        );
        assert_eq!(value(&env, triage::STREAM), Some(choice("updates")));
        assert_eq!(
            value(&env, triage::NEEDS_REPLY),
            Some(AnswerValue::Noul(false))
        );
    }

    #[test]
    fn a_security_alert_from_another_domain_is_not_one() {
        // The name claims the platform, the domain is someone else's.
        for (name, from) in [
            ("GitHub", "notifications@github-alerts.example"),
            ("GitHub", "alerts@g1thub.example"),
            ("Security Team", "notifications@github.com"),
        ] {
            let env = security_alert_mail(
                name,
                from,
                "[example-org/demo-shop] High severity vulnerability in tiny-template",
            );
            let asks = ask(&env, triage::ASKS_ACTION);
            assert!(
                asks.as_ref()
                    .is_none_or(|d| d.reasons.iter().all(|r| r.rule != "signals.security_alert")),
                "{name} <{from}>: {asks:?}"
            );
        }
    }

    #[test]
    fn action_required_marketing_stays_a_newsletter() {
        let env = with_header(
            named(
                mail(
                    "news@paperkite.example",
                    "Action required: claim your discount",
                    "Your 20% discount expires soon. Shop now.",
                ),
                "Paper Kite",
            ),
            "List-Unsubscribe",
            "<https://paperkite.example/u>",
        );
        assert_eq!(
            value(&env, triage::ASKS_ACTION),
            Some(AnswerValue::Noul(false))
        );
        assert_eq!(value(&env, triage::STREAM), Some(choice("newsletters")));
    }

    #[test]
    fn a_comment_on_a_watched_issue_asks_nothing() {
        let env = with_header(
            named(
                mail(
                    "notifications@github.com",
                    "Re: [example-org/demo-shop] Checkout fails on Safari (Issue #17)",
                    "Same here on Safari 18. Any idea when this lands?\n\n\
                     You are receiving this because you are subscribed to this thread.",
                ),
                "Rui Tavares",
            ),
            "List-Id",
            "<demo-shop.example-org.github.com>",
        );
        let asks = ask(&env, triage::ASKS_ACTION).unwrap();
        assert_eq!(asks.answer.value, AnswerValue::Noul(false));
        assert_eq!(asks.reasons[0].rule, "signals.list_mail");
    }

    #[test]
    fn list_mail_is_a_newsletter_with_no_ask() {
        let env = with_header(
            mail(
                "news@livraria.example",
                "Novidades de outubro",
                "Descubra os livros do mês.",
            ),
            "List-Unsubscribe",
            "<https://livraria.example/u>",
        );
        let d = ask(&env, triage::STREAM).unwrap();
        assert_eq!(d.answer.value, AnswerValue::Choice("newsletters".into()));
        assert_eq!(d.reasons[0].rule, "signals.list_header");
        assert_eq!(
            value(&env, triage::ASKS_ACTION),
            Some(AnswerValue::Noul(false))
        );
        assert_eq!(
            value(&env, triage::NEEDS_REPLY),
            Some(AnswerValue::Noul(false))
        );
    }

    #[test]
    fn automated_money_mail_is_a_receipt() {
        let env = mail(
            "no-reply@nuvem.example",
            "Recibo do seu pagamento",
            "Obrigado pelo seu pagamento de 12,00 €.",
        );
        assert_eq!(value(&env, triage::STREAM), Some(choice("receipts")));
        let asks = ask(&env, triage::ASKS_ACTION).unwrap();
        assert_eq!(asks.answer.value, AnswerValue::Noul(false));
        assert_eq!(asks.reasons[0].rule, "signals.already_paid");
    }

    #[test]
    fn a_bill_due_asks_and_has_a_deadline() {
        let env = mail(
            "facturacion@aguas.example",
            "Tu factura de septiembre",
            "Importe pendiente: 48,20 €. Fecha de vencimiento: hasta el 15/10/2026.",
        );
        assert_eq!(value(&env, triage::STREAM), Some(choice("receipts")));
        assert_eq!(
            value(&env, triage::ASKS_ACTION),
            Some(AnswerValue::Noul(true))
        );
        assert_eq!(
            value(&env, triage::HAS_DEADLINE),
            Some(AnswerValue::Noul(true))
        );
    }

    #[test]
    fn a_person_asking_a_question_needs_a_reply() {
        let env = mail(
            "claire@atelier-morel.example",
            "Plans",
            "Bonjour, pourriez-vous m'envoyer les plans avant vendredi ? Merci !\n\n\
             Le 2 oct. 2026, Rui a écrit :\n> Voici la version 2.",
        );
        assert_eq!(value(&env, triage::STREAM), Some(choice("primary")));
        assert_eq!(
            value(&env, triage::NEEDS_REPLY),
            Some(AnswerValue::Noul(true))
        );
        assert_eq!(
            value(&env, triage::ASKS_ACTION),
            Some(AnswerValue::Noul(true))
        );
        assert_eq!(
            value(&env, triage::HAS_DEADLINE),
            Some(AnswerValue::Noul(true))
        );
    }

    #[test]
    fn quoted_history_is_ignored() {
        let env = mail(
            "sam@example.com",
            "Re: draft",
            "Thanks, all good.\n\nOn Tue, 1 Sep 2026, Ana wrote:\n> Could you send it by Friday?",
        );
        assert_eq!(
            value(&env, triage::NEEDS_REPLY),
            Some(AnswerValue::Noul(false))
        );
        assert_eq!(
            value(&env, triage::HAS_DEADLINE),
            Some(AnswerValue::Noul(false))
        );
    }

    #[test]
    fn out_of_office_is_an_update() {
        let env = mail(
            "marta@example.com",
            "Resposta automática: Orçamento",
            "Estou ausente até dia 20.",
        );
        let d = ask(&env, triage::STREAM).unwrap();
        assert_eq!(d.answer.value, AnswerValue::Choice("updates".into()));
        assert_eq!(d.reasons[0].rule, "signals.auto_reply");
    }

    #[test]
    fn phishing_needs_two_tells() {
        let env = with_header(
            mail(
                "alertas@bancoatlantico-seguranca.test",
                "Conta bloqueada",
                "Detetámos atividade invulgar. Clique aqui: https://login-atlantico.test/x",
            ),
            "Reply-To",
            "suporte@outro.test",
        );
        let d = ask(&env, triage::SUSPICIOUS).unwrap();
        assert_eq!(d.answer.value, AnswerValue::Noul(true));
        assert!(d.reasons.len() >= 2);

        let plain = mail("ana@example.com", "Olá", "Tudo bem? Até logo.");
        assert_eq!(
            value(&plain, triage::SUSPICIOUS),
            Some(AnswerValue::Noul(false))
        );
    }

    #[test]
    fn conditional_asks_do_not_count() {
        let env = mail(
            "security@conta.example",
            "New sign-in to your account",
            "We noticed a new sign-in. If this wasn't you, reset your password.",
        );
        assert_eq!(value(&env, triage::STREAM), Some(choice("updates")));
        assert_eq!(
            value(&env, triage::ASKS_ACTION),
            Some(AnswerValue::Noul(false))
        );
        let urgency = ask(&env, triage::URGENCY).unwrap().answer;
        let p = urgency.probabilities.unwrap();
        assert!(p[0] > p[1] && p[0] > p[2], "most likely no urgency: {p:?}");
    }

    #[test]
    fn unknown_or_reshaped_questions_are_abstained() {
        let env = mail("ana@example.com", "Olá", "Tudo bem?");
        let qs = [
            Question::noul("other", "Something else."),
            Question::choice("stream", "Which?", [("work", ""), ("home", "")]),
        ];
        assert_eq!(EmailSignals::answer(&env, &qs).answers, vec![None, None]);
    }

    #[test]
    fn decision_state_is_compact_and_debug_hides_content() {
        let env = with_header(
            mail("ana@example.com", "Fatura", &"x".repeat(5000)),
            "List-Id",
            "<news.example>",
        );
        let DecisionState::Text(state) = env.decision_state() else {
            unreachable!()
        };
        assert!(state.contains("Subject: Fatura"));
        assert!(state.contains("list-id: <news.example>"));
        assert!(state.chars().count() < 1700);
        assert!(!format!("{env:?}").contains("Fatura"));
    }

    #[test]
    fn registrable_domains() {
        assert_eq!(
            registrable("conta.bancoatlantico.example"),
            "bancoatlantico.example"
        );
        assert_eq!(
            registrable("news.atelier.example.com"),
            "atelier.example.com"
        );
        assert_eq!(registrable("example.com"), "example.com");
    }
}
