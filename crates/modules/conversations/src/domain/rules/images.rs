//! "Always show images from <sender>" (campbooks-cloud#200): whether a
//! smart rule trusts a message's sender with its remote images, and what the
//! reader may offer about them (campbooks-cloud#235). Facts only (ADR 0018):
//! the sender's address, the receiving provider's authentication results and
//! the stored `suspicious` answers, never what the mail says. The images
//! still go through the sanitiser, which removes tracking pixels either way.

use campbooks_ai::decide::{
    signals::facts::{dmarc_refused, sender_authenticated, sender_checks_failed},
    taught::{self, Action, Facts, TaughtRule, images_domain_too_broad},
};
use campbooks_kernel::{Result, binding::Binding};
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
    /// The rule that trusts the sender is paused, or waiting for a yes (a
    /// proposal, or a rule that needs confirming again).
    RulePaused,
    /// The rule that would trust the sender names a whole domain anyone may
    /// send from (a free mail provider's or a shared platform's), so it never
    /// acts; one address there may still be trusted.
    RuleTooBroad,
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
    /// The domain the member may trust instead of the address: the sender's
    /// own host (never a parent of it), owned, not a free mail provider's nor
    /// a shared platform's, when it may be offered.
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
/// never pass), nothing is trusted. Nor does a rule that is not bound to the
/// member who taught it act (campbooks-cloud#242): one planted with SQL, or
/// kept from before rules were bound, counts as waiting for a yes.
pub async fn images(
    conn: &mut PgConnection,
    binding: &Binding,
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
    let (mut acting, mut taught_already, mut too_broad) = (Vec::new(), Vec::new(), Vec::new());
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
        // Stored before its domain joined a list: it never acts, and covers
        // nobody, so one address there may still be trusted.
        if rule.domain_too_broad() {
            too_broad.push(engine);
            continue;
        }
        if rule.effective_state() == RuleState::Active {
            if rule.bound(binding) {
                acting.push(engine.clone());
            } else {
                tracing::warn!(
                    workspace = %rule.workspace,
                    rule = %rule.id,
                    "a taught rule is not bound to the member who taught it; it does not act"
                );
            }
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
        too_broad: &too_broad,
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
    too_broad: &'a [TaughtRule],
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
    // A rule for a domain too broad to act reaches nobody, wherever it is.
    let reaches = |r: &TaughtRule| !r.images_too_broad() && r.matches(&facts);
    let acting = f.acting.iter().any(reaches);
    let covered = acting || f.taught_already.iter().any(reaches);
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
        let refused = f.too_broad.iter().any(|r| r.matches(&facts));
        blocked.or(refused.then_some(ImagesHeld::RuleTooBroad))
    };
    // The sender's own host, never a parent of it: a domain rule reaches its
    // subdomains, so `billing@acme.helpdesk.example` must not be offered
    // anyone at `helpdesk.example`, where every customer sends from.
    let host = f
        .sender
        .rsplit_once('@')
        .map_or("", |(_, host)| host)
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let domain = Some(host).filter(|d| offer && !d.is_empty() && !images_domain_too_broad(d));
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
            too_broad: &[],
        }
    }

    /// Mail from `sender` that its own domain's checks authenticate.
    fn authenticated(sender: &'static str, results: &'static str) -> Seen<'static> {
        Seen {
            sender,
            results: Some(results),
            ..facts(None, &[])
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

    #[test]
    fn a_subdomain_sender_is_offered_its_own_host_never_a_parent() {
        let got = assess(&authenticated(
            "news@mail.brightloop.example",
            "mx.google.com; dkim=pass header.i=@brightloop.example header.s=s1; \
             dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=mail.brightloop.example",
        ));
        assert!(got.offer);
        assert_eq!(got.domain.as_deref(), Some("mail.brightloop.example"));
    }

    #[test]
    fn a_help_desk_customer_is_offered_its_address_only() {
        // Every Zendesk customer sends from its own subdomain, signed by it.
        let got = assess(&authenticated(
            "billing@acme.zendesk.com",
            "mx.google.com; dkim=pass header.i=@zendesk.com header.s=s1; \
             dmarc=pass (p=NONE sp=NONE dis=NONE) header.from=acme.zendesk.com",
        ));
        assert!(got.offer);
        assert_eq!(got.domain, None);
    }

    #[test]
    fn a_relayed_address_is_offered_its_address_only() {
        let got = assess(&authenticated(
            "x7k2m9@privaterelay.appleid.com",
            "mx.google.com; dkim=pass header.i=@privaterelay.appleid.com header.s=s1; \
             dmarc=pass (p=QUARANTINE sp=QUARANTINE dis=NONE) \
             header.from=privaterelay.appleid.com",
        ));
        assert!(got.offer);
        assert_eq!(got.domain, None);
    }

    #[test]
    fn an_old_rule_for_a_free_mail_domain_never_trusts_and_says_why() {
        let rules = [trusting(Condition::SenderDomain("yahoo.fr".into()))];
        let got = assess(&Seen {
            acting: &rules,
            taught_already: &[],
            too_broad: &rules,
            ..authenticated(
                "sam@mail.yahoo.fr",
                "mx.google.com; dkim=pass header.i=@yahoo.fr header.s=s1; \
                 dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=mail.yahoo.fr",
            )
        });
        // Even handed to the engine as acting, it trusts nobody.
        assert!(!got.trusted);
        assert_eq!(got.held, Some(ImagesHeld::RuleTooBroad));
        // One address there may still be trusted.
        assert!(got.offer);
        assert_eq!(got.domain, None);
    }
}
