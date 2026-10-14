//! `identity.notify_way_in_added` ← `identity.way_in_added`: tell the
//! account that a way to sign in was added (a stolen session adding its own
//! way in is the takeover this catches). The notice goes by mail to the
//! account's verified primary address and to the new way in's verified
//! address, in the person's language, when the instance has a mail server;
//! otherwise it is recorded in the account's audit trail and Settings → Ways
//! to sign in shows recent additions. A sign-up's first way in gets none.

use campbooks_kernel::{
    Event, EventConsumer, JobContext, JobError,
    auth::{Principal, PrincipalKind},
    i18n::{Arg, Locale},
    mail::{Mail, MailError, escape_html},
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    MESSAGES,
    domain::{accounts, notices},
    events::WayInAdded,
};

/// Sends the notice of a new way in.
#[derive(Debug, Clone, Copy, Default)]
pub struct NotifyWayInAdded;

impl EventConsumer for NotifyWayInAdded {
    type Event = WayInAdded;
    const NAME: &'static str = "identity.notify_way_in_added";
    const QUEUE: &'static str = "events";

    async fn handle(&self, ctx: &mut JobContext, event: Event<WayInAdded>) -> Result<(), JobError> {
        let added = &event.payload;
        if added.via == "sign_up" {
            return Ok(());
        }
        let state = ctx.state().clone();
        let now = Utc::now();
        if !state.mailer.is_configured() {
            let tx = ctx.tx().await?;
            accounts::audit(
                tx,
                Some(added.account),
                "way_in_notice",
                serde_json::json!({ "method": added.method, "way_in": added.way_in, "sent": 0, "reason": "no_mail_transport" }),
                now,
            )
            .await?;
            return Ok(());
        }
        let recipients = notices::recipients(&state.db, added.account, added.way_in).await?;
        let locale = locale(&state, added.account).await;
        let mail = Notice {
            what: what(locale, added),
            when: event.occurred_at,
            url: state.config.public_url.trim_end_matches('/'),
        };
        let event_id = *event.id.as_uuid();
        let mut sent = 0_u32;
        let mut refused = 0_u32;
        for (recipient, address) in recipients {
            if notices::sent(&state.db, event_id, recipient).await? {
                sent += 1;
                continue;
            }
            match state.mailer.send(&mail.render(locale, address)).await {
                Ok(()) => {
                    notices::record(&state.db, event_id, added.account, recipient, now).await?;
                    sent += 1;
                }
                // For good (an address the server refuses): not retried.
                Err(MailError::Refused | MailError::BadRecipient) => {
                    tracing::warn!(
                        recipient = recipient.as_str(),
                        "the mail server refused a way-in notice"
                    );
                    refused += 1;
                }
                Err(err @ (MailError::Unavailable | MailError::NotConfigured)) => {
                    return Err(JobError::retry(err.to_string()));
                }
            }
        }
        let tx = ctx.tx().await?;
        accounts::audit(
            tx,
            Some(added.account),
            "way_in_notice",
            serde_json::json!({ "method": added.method, "way_in": added.way_in, "sent": sent, "refused": refused }),
            now,
        )
        .await?;
        Ok(())
    }
}

/// The language the person chose, English when they chose none (or it
/// cannot be read now: a notice in English beats none).
async fn locale(state: &campbooks_kernel::AppState, account: Uuid) -> Locale {
    let principal = Principal {
        subject: account.to_string(),
        kind: PrincipalKind::User,
        client_id: None,
        scopes: std::collections::BTreeSet::new(),
        workspace: None,
        email: None,
        email_verified: false,
        name: None,
        expires_at: None,
        auth_time: None,
    };
    state
        .locales
        .preferred(&state.db, &principal)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// What was added, in the person's words: "A passkey", "A Google account".
fn what(locale: Locale, added: &WayInAdded) -> String {
    match added.provider.as_deref() {
        Some(provider) if added.method == "provider" => MESSAGES.t(
            locale,
            "notices.way_in_added.provider",
            &[("provider", Arg::Text(provider_name(provider).to_owned()))],
        ),
        _ => MESSAGES.t(locale, "notices.way_in_added.passkey", &[]),
    }
}

/// A provider's own name.
fn provider_name(provider: &str) -> &str {
    match provider {
        "google" => "Google",
        "microsoft" => "Microsoft",
        "zoho" => "Zoho",
        other => other,
    }
}

/// The notice, before it is put in one language for one address.
struct Notice<'a> {
    what: String,
    when: DateTime<Utc>,
    url: &'a str,
}

impl Notice<'_> {
    /// The message to `to`: plain text and HTML saying the same thing, no
    /// images, no tracking, one link (to this instance).
    fn render(&self, locale: Locale, to: String) -> Mail {
        let t = |key: &str, args: &[(&str, Arg)]| {
            MESSAGES.t(locale, &format!("notices.way_in_added.{key}"), args)
        };
        let when = self.when.format("%Y-%m-%d %H:%M UTC").to_string();
        let added = t(
            "added",
            &[
                ("what", Arg::Text(self.what.clone())),
                ("when", Arg::Text(when)),
            ],
        );
        let (if_you, if_not, open, why) = (
            t("if_you", &[]),
            t("if_not", &[]),
            t("open", &[]),
            t("why", &[]),
        );
        let text = format!(
            "{added}\n\n{if_you}\n\n{if_not}\n\n{open}: {url}\n\n--\n{why}\n",
            url = self.url
        );
        let html = format!(
            "<!doctype html><html lang=\"{lang}\"><head><meta charset=\"utf-8\">\
             <title>{subject}</title></head>\
             <body style=\"font-family: system-ui, sans-serif; line-height: 1.5; color: #111;\">\
             <p>{added}</p><p>{if_you}</p><p><strong>{if_not}</strong></p>\
             <p><a href=\"{url}\">{open}</a></p>\
             <hr><p style=\"color: #555; font-size: 0.875em;\">{why}</p>\
             </body></html>",
            lang = locale.tag(),
            subject = escape_html(&t("subject", &[])),
            added = escape_html(&added),
            if_you = escape_html(&if_you),
            if_not = escape_html(&if_not),
            url = escape_html(self.url),
            open = escape_html(&open),
            why = escape_html(&why),
        );
        Mail {
            to,
            subject: t("subject", &[]),
            text,
            html,
        }
    }
}

#[cfg(test)]
mod tests;
