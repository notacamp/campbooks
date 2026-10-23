//! Whether someone who writes to an inbox is safe to write to, from facts
//! alone (ADR 0018): addresses, the provider's authentication results,
//! stored answers, needs and member corrections, never what the mail says.
//! Shared by the reads that suggest people to write to:
//! `conversations.list_senders` with `reachable` (Forward's picker,
//! campbooks-cloud#233) and `conversations.list_correspondents` (the
//! composer, campbooks-cloud#304).

use campbooks_ai::decide::signals::facts::{Facts, dmarc_refused, sender_checks_failed};

/// SQL, true when any of `k.address`'s own inbound mail in the inboxes `$1`
/// within the last `$4` days, spam included, was reported (Report and delete
/// moves it to spam), is under review (open, or cleared because a later
/// message replaced it), or was found suspicious and not said safe by a
/// member. Judged by the flagged message's own sender, so the real people in
/// a hijacked thread stay. Each run reads one person's mail through
/// `messages_inbound_sender_idx`, so callers run it row by row after ranking
/// (`OFFSET 0`), only until their answer is full (campbooks-cloud#246).
///
/// A macro so that it can be `concat!`ed into a query's text; the query
/// binds the inboxes as `$1` and the window in days as `$4`, and names the
/// row being checked `k`.
macro_rules! flagged_sender_sql {
    () => {
        r"EXISTS (
           SELECT 1 FROM conversations.messages m
            WHERE m.inbox_id = ANY($1) AND m.direction = 'inbound'
              AND m.sender_address = k.address
              AND m.sent_at > now() - make_interval(days => $4)
              AND (m.in_spam
                   OR EXISTS (SELECT 1 FROM conversations.needs n
                               WHERE n.message_id = m.id AND n.reason = 'review'
                                 AND (n.state = 'open' OR n.cleared_because = 'superseded'))
                   OR (EXISTS (SELECT 1 FROM conversations.decisions d
                                WHERE d.message_id = m.id AND d.question = 'suspicious'
                                  AND d.value = 'true'::jsonb)
                       AND NOT EXISTS (SELECT 1 FROM conversations.corrections c
                                        WHERE c.message_id = m.id AND c.question = 'suspicious'
                                          AND c.to_value = 'false' AND c.withdrawn_at IS NULL))))"
    };
}
pub(crate) use flagged_sender_sql;

/// Rows read beyond `limit` when some are checked with
/// [`flagged_sender_sql`] and [`may_write_to`], so those left out rarely
/// shorten the answer.
pub const SLACK: i64 = 40;

/// Whether `address` is safe to write to, from facts alone: not a no-reply
/// address, and its newest mail (`results`, the provider's authentication
/// results) neither fails its sender's checks nor pretends to be them.
/// Absent results say nothing.
pub fn may_write_to(address: &str, results: Option<&str>) -> bool {
    let no_reply = Facts {
        from: address.to_lowercase(),
        ..Facts::default()
    }
    .no_reply_sender();
    !no_reply && !sender_checks_failed(address, results) && !dmarc_refused(results)
}

#[cfg(test)]
mod tests {
    use super::may_write_to;

    #[test]
    fn who_may_be_written_to_is_read_from_facts() {
        let pass = "mx.google.com; dkim=pass header.i=@silva-contas.example header.s=s1; \
                    dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=silva-contas.example";
        let refused = "mx.google.com; spf=pass smtp.mailfrom=bounce@spoofer.example; \
                       dmarc=fail (p=REJECT sp=REJECT dis=REJECT) header.from=silva-contas.example";
        assert!(may_write_to("rita@silva-contas.example", Some(pass)));
        // No results say nothing (Zoho, IMAP).
        assert!(may_write_to("rita@silva-contas.example", None));
        assert!(!may_write_to("rita@silva-contas.example", Some(refused)));
        for address in [
            "no-reply@accounts.example",
            "noreply@shop.example",
            "do-not-reply@bank.example",
            "mailer-daemon@example.com",
        ] {
            assert!(!may_write_to(address, Some(pass)), "{address}");
        }
    }
}
