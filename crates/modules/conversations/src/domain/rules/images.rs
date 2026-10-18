//! "Always show images from <sender>" (campbooks-cloud#200): whether a
//! smart rule trusts a message's sender with its remote images, and what the
//! reader may offer about them (campbooks-cloud#235). Facts only (ADR 0018):
//! the sender's address, the receiving provider's authentication results and
//! the stored `suspicious` answers, never what the mail says. The images
//! still go through the sanitiser, which removes tracking pixels either way.

use campbooks_ai::decide::{
    signals::facts::{
        dmarc_refused, registrable_domain, sender_authenticated, sender_checks_failed,
    },
    taught::{self, Action, Facts, TaughtRule, is_free_mail, is_owned_domain, is_shared_sender},
};
use campbooks_kernel::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

use super::of_inboxes;
use crate::{
    ingest::Headers,
    model::{InboxId, MessageId, RuleState},
};

/// Why a message's remote images stay held back although its sender is, or
/// could be, trusted (campbooks-cloud#235).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImagesHeld {
    /// The email or its conversation is under review.
    UnderReview,
    /// The email fails its sender's checks, or pretends to be them.
    ChecksFail,
    /// The provider checked nothing that could confirm the sender (its
    /// results say `none`).
    Unverified,
    /// The provider that received it shares no sender checks, so no sender
    /// can be trusted here.
    CannotAuthenticate,
    /// Scout has not checked the email yet: images wait for it, and stay held
    /// back while Scout is off or past its daily limit.
    NotChecked,
    /// The rule that trusts the sender is paused or waiting for a yes.
    RulePaused,
}

/// What a message's sender is trusted with, and what the reader may offer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Images {
    /// A rule trusts the sender and everything about the email allows it:
    /// its remote images are kept without asking.
    pub trusted: bool,
    /// "Always show images from …" can take effect: no rule trusts the
    /// sender yet, and nothing about the email would keep its images back.
    pub offer: bool,
    /// The domain the member may trust instead of the address (registrable,
    /// owned, not a free mail provider's), when it may be offered.
    pub domain: Option<String>,
    /// Why the images stay held back when a rule trusts the sender, or why
    /// trusting it could not take effect.
    pub held: Option<ImagesHeld>,
}

/// The images of `message`, one of `inboxes` (those the member may see), as
/// the rules of its inbox decide them. An outbound message, or one not in
/// `inboxes`, is trusted with nothing and offers nothing.
///
/// Trust fails closed: until the message has been judged not suspicious (a
/// stored `suspicious` answer of `false`), while anything in its
/// conversation looks suspicious, and unless the provider's results
/// authenticate the sender (a spoofed From, failed checks or no results
/// never pass), nothing is trusted.
pub async fn images(
    conn: &mut PgConnection,
    inboxes: &[InboxId],
    message: MessageId,
) -> Result<Images> {
    let row: Option<(InboxId, String, serde_json::Value, bool, bool, bool)> = sqlx::query_as(
        "SELECT m.inbox_id, m.sender_address, m.headers,
                EXISTS (SELECT 1 FROM conversations.decisions d
                         WHERE d.message_id = m.id AND d.question = 'suspicious'
                           AND d.value = 'false'::jsonb),
                (EXISTS (SELECT 1 FROM conversations.decisions d
                          WHERE d.conversation_id = m.conversation_id
                            AND d.question = 'suspicious' AND d.value = 'true'::jsonb)
                 OR EXISTS (SELECT 1 FROM conversations.needs n
                             WHERE n.conversation_id = m.conversation_id
                               AND n.state = 'open' AND n.reason = 'review')),
                -- The inbox's rules are read only when one shows images.
                EXISTS (SELECT 1 FROM conversations.rules r
                         WHERE r.inbox_id = m.inbox_id AND r.state <> 'removed'
                           AND r.actions @> '[{\"action\": \"show_images\"}]'::jsonb)
           FROM conversations.messages m
          WHERE m.id = $1 AND m.inbox_id = ANY($2) AND m.direction = 'inbound'",
    )
    .bind(message)
    .bind(inboxes)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((inbox, sender, headers, judged, suspicious, image_rules)) = row else {
        return Ok(Images::default());
    };
    // The inbox's rules that show images: acting ones trust, and any not
    // removed (paused or waiting for a yes too) means it is taught already.
    let (mut acting, mut taught_already) = (Vec::new(), Vec::new());
    let stored = if image_rules {
        of_inboxes(conn, &[inbox]).await?
    } else {
        Vec::new()
    };
    for rule in stored {
        let engine = rule.engine();
        if !engine.actions.contains(&Action::ShowImages) {
            continue;
        }
        if rule.effective_state() == RuleState::Active {
            acting.push(engine.clone());
        }
        taught_already.push(engine);
    }
    let headers: Headers = serde_json::from_value(headers).unwrap_or_default();
    Ok(assess(&Seen {
        sender: &sender,
        results: headers.authentication_results.as_deref(),
        judged,
        suspicious,
        acting: &acting,
        taught_already: &taught_already,
    }))
}

/// What [`assess`] reads about one message.
struct Seen<'a> {
    sender: &'a str,
    results: Option<&'a str>,
    judged: bool,
    suspicious: bool,
    acting: &'a [TaughtRule],
    taught_already: &'a [TaughtRule],
}

/// The decision itself, from the facts alone.
fn assess(f: &Seen<'_>) -> Images {
    let auth_failed = sender_checks_failed(f.sender, f.results);
    let impersonated = dmarc_refused(f.results);
    let authenticated = sender_authenticated(f.sender, f.results);
    let facts = Facts {
        sender_address: f.sender,
        suspicious: f.suspicious,
        auth_failed,
        impersonated,
        ..Facts::default()
    };
    let trusted = f.judged && taught::shows_images(f.acting, &facts, authenticated).is_some();
    let acting = f.acting.iter().any(|r| r.matches(&facts));
    let covered = acting || f.taught_already.iter().any(|r| r.matches(&facts));
    // What would keep images back whatever a rule says.
    let blocked = if f.suspicious {
        Some(ImagesHeld::UnderReview)
    } else if f.results.is_none() {
        Some(ImagesHeld::CannotAuthenticate)
    } else if auth_failed || impersonated {
        Some(ImagesHeld::ChecksFail)
    } else if !authenticated {
        Some(ImagesHeld::Unverified)
    } else {
        None
    };
    let offer = !covered && blocked.is_none();
    let held = if trusted {
        None
    } else if acting {
        blocked.or((!f.judged).then_some(ImagesHeld::NotChecked))
    } else if covered {
        blocked.or(Some(ImagesHeld::RulePaused))
    } else {
        blocked
    };
    let domain = Some(registrable_domain(f.sender)).filter(|d| {
        offer && !d.is_empty() && is_owned_domain(d) && !is_free_mail(d) && !is_shared_sender(d)
    });
    Images {
        trusted,
        offer,
        domain,
        held,
    }
}

#[cfg(test)]
mod tests {
    use campbooks_ai::decide::taught::Condition;

    use super::*;

    const PASS: &str = "mx.google.com; dkim=pass header.i=@brightloop.example header.s=s1; \
                        dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=brightloop.example";
    const FAIL: &str = "mx.google.com; spf=pass smtp.mailfrom=bounce@spoofer.example; \
                        dmarc=fail (p=REJECT sp=REJECT dis=REJECT) header.from=brightloop.example";

    fn trusting(condition: Condition) -> TaughtRule {
        TaughtRule {
            id: "r1".into(),
            conditions: vec![condition],
            actions: vec![Action::ShowImages],
            confirmed_at: 1,
        }
    }

    fn facts<'a>(results: Option<&'a str>, rules: &'a [TaughtRule]) -> Seen<'a> {
        Seen {
            sender: "news@brightloop.example",
            results,
            judged: true,
            suspicious: false,
            acting: rules,
            taught_already: rules,
        }
    }

    #[test]
    fn an_untrusted_authenticated_sender_is_offered_with_its_domain() {
        let got = assess(&facts(Some(PASS), &[]));
        assert!(got.offer && !got.trusted);
        assert_eq!(got.domain.as_deref(), Some("brightloop.example"));
        assert_eq!(got.held, None);
    }

    #[test]
    fn a_trusted_sender_is_offered_nothing_more() {
        let rules = [trusting(Condition::SenderDomain(
            "brightloop.example".into(),
        ))];
        let got = assess(&facts(Some(PASS), &rules));
        assert!(got.trusted && !got.offer);
        assert_eq!(got.domain, None);
        // Not judged yet: held back, and why.
        let waiting = assess(&Seen {
            judged: false,
            ..facts(Some(PASS), &rules)
        });
        assert!(!waiting.trusted && !waiting.offer);
        assert_eq!(waiting.held, Some(ImagesHeld::NotChecked));
        // A paused rule still counts as taught: no second offer, and why.
        let paused = assess(&Seen {
            acting: &[],
            ..facts(Some(PASS), &rules)
        });
        assert!(!paused.trusted && !paused.offer);
        assert_eq!(paused.held, Some(ImagesHeld::RulePaused));
    }

    #[test]
    fn nothing_is_offered_where_it_could_not_take_effect() {
        let failing = assess(&facts(Some(FAIL), &[]));
        assert!(!failing.offer);
        assert_eq!(failing.held, Some(ImagesHeld::ChecksFail));
        let unsigned = assess(&facts(Some("mx.google.com; none"), &[]));
        assert!(!unsigned.offer);
        assert_eq!(unsigned.held, Some(ImagesHeld::Unverified));
        let unchecked = assess(&facts(None, &[]));
        assert!(!unchecked.offer);
        assert_eq!(unchecked.held, Some(ImagesHeld::CannotAuthenticate));
        let review = assess(&Seen {
            suspicious: true,
            ..facts(Some(PASS), &[])
        });
        assert!(!review.offer);
        assert_eq!(review.held, Some(ImagesHeld::UnderReview));
    }

    #[test]
    fn a_stored_suspicious_answer_blocks_a_trusted_sender() {
        let rules = [trusting(Condition::SenderAddress(
            "news@brightloop.example".into(),
        ))];
        let got = assess(&Seen {
            suspicious: true,
            ..facts(Some(PASS), &rules)
        });
        assert!(!got.trusted);
        assert_eq!(got.held, Some(ImagesHeld::UnderReview));
    }

    #[test]
    fn a_free_mail_sender_is_offered_its_address_only() {
        let got = assess(&Seen {
            sender: "sam@mail.yahoo.fr",
            results: Some(
                "mx.google.com; dkim=pass header.i=@yahoo.fr header.s=s1; \
                 dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=mail.yahoo.fr",
            ),
            ..facts(None, &[])
        });
        assert!(got.offer);
        assert_eq!(got.domain, None);
    }

    #[test]
    fn a_platform_anyone_sends_from_is_offered_its_address_only() {
        let got = assess(&Seen {
            sender: "ledger@mail.beehiiv.com",
            results: Some(
                "mx.google.com; dkim=pass header.i=@beehiiv.com header.s=s1; \
                 dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=mail.beehiiv.com",
            ),
            ..facts(None, &[])
        });
        assert!(got.offer);
        assert_eq!(got.domain, None);
    }
}
