//! Where an inbox's mail goes out (campbooks-cloud#242): the connection,
//! stream and mailbox that feed it, as the kernel's outbound-destinations
//! port answers it. The conversations module binds what this answers into a
//! send's MACs when the member asks and when the send is released; the
//! sender and the change recorder resolve it again here and pass it when
//! they ask whether a send or a change stands, so a stream repointed by SQL
//! after the member asked does not verify.
//!
//! `connectors.destinations_at_deploy` holds each inbox's destination as
//! it was when destinations were first bound, written by that migration and
//! never by the runtime role: a binding made before (which names none)
//! verifies only while the destination is still that one
//! ([`Resolved::as_at_deploy`]).

use std::sync::Arc;

use campbooks_kernel::{
    Result,
    outbound::{Destination, DestinationFuture, DestinationsPort, OutboundDestinations, Resolved},
};
use sqlx::PgConnection;
use uuid::Uuid;

use super::model::{ConnectionId, InboxId, Provider, StreamId};
use crate::microsoft::graph_write::MAX_SEND_BODY;

/// The port, for the server to register.
pub fn outbound_destinations() -> DestinationsPort {
    Arc::new(Destinations)
}

#[derive(Debug, Clone, Copy)]
struct Destinations;

impl OutboundDestinations for Destinations {
    fn destination<'a>(&'a self, conn: &'a mut PgConnection, inbox: Uuid) -> DestinationFuture<'a> {
        Box::pin(async move { of_inbox(conn, InboxId::from_uuid(inbox)).await })
    }
}

/// The destination of `inbox`, read in `conn` (a transaction working in its
/// workspace): its `messages` stream, that stream's connection and mailbox.
/// `None` when no stream feeds it, or when more than one claims it (only
/// SQL can make two: none is trusted).
pub async fn of_inbox(conn: &mut PgConnection, inbox: InboxId) -> Result<Option<Resolved>> {
    let rows: Vec<(StreamId, ConnectionId, String, bool, String)> = sqlx::query_as(
        "SELECT s.id, s.connection_id, s.mailbox_key,
                EXISTS (SELECT 1 FROM connectors.destinations_at_deploy d
                         WHERE d.stream_id = s.id AND d.inbox_id = s.inbox_id
                           AND d.connection_id = s.connection_id
                           AND d.mailbox_key = s.mailbox_key),
                c.provider
           FROM connectors.streams s
           JOIN connectors.connections c ON c.id = s.connection_id
          WHERE s.inbox_id = $1 AND s.capability = 'messages'
          ORDER BY s.id
          LIMIT 2",
    )
    .bind(inbox)
    .fetch_all(conn)
    .await?;
    if rows.len() > 1 {
        tracing::warn!(
            %inbox,
            "more than one stream claims an inbox; nothing goes out from it"
        );
        return Ok(None);
    }
    let Some((stream, connection, mailbox, as_at_deploy, provider)) = rows.into_iter().next()
    else {
        return Ok(None);
    };
    Ok(Some(Resolved {
        destination: destination(stream, connection, mailbox),
        as_at_deploy,
        max_attachment_bytes: Provider::parse(&provider)
            .ok()
            .and_then(max_attachment_bytes),
    }))
}

/// Headroom in a sent message for its headers and text, in bytes.
const MESSAGE_HEADROOM: u64 = 64 * 1024;

/// The most bytes of attachments one message through `provider` can carry,
/// when the provider takes less than a draft's own 25 MiB (campbooks-cloud#391).
/// Microsoft Graph takes at most [`MAX_SEND_BODY`] bytes in a `sendMail`,
/// and the MIME message goes as base64 (3 bytes in 4), its attachments inside
/// it as base64 again in 76-character lines (57 bytes in 78): what is left
/// once the headers and text have their room.
pub const fn max_attachment_bytes(provider: Provider) -> Option<u64> {
    match provider {
        Provider::Microsoft => Some((MAX_SEND_BODY as u64 / 4 * 3 - MESSAGE_HEADROOM) / 78 * 57),
        Provider::Google | Provider::Zoho | Provider::Imap | Provider::Notion => None,
    }
}

/// `stream`, its connection `connection` and its mailbox `mailbox` as the
/// destination of a send from `inbox`, read as recorded at the deploy or not
/// (in `conn`).
pub async fn resolved(
    conn: &mut PgConnection,
    inbox: InboxId,
    stream: StreamId,
    connection: ConnectionId,
    mailbox: &str,
) -> Result<Resolved> {
    Ok(Resolved {
        as_at_deploy: as_at_deploy(conn, Some(inbox), stream, connection, mailbox).await?,
        destination: destination(stream, connection, mailbox.to_owned()),
        // Read only where a draft is written (`of_inbox`); a send checks
        // its size itself.
        max_attachment_bytes: None,
    })
}

/// Whether `stream` fed the inbox it feeds now (and `inbox`, when given)
/// through `connection` and `mailbox` when destinations were first bound
/// (`connectors.destinations_at_deploy`). The inbox is part of it: a
/// binding from before names the inbox but not the stream, so another
/// inbox's stream pointed at it never counts as recorded.
pub async fn as_at_deploy(
    conn: &mut PgConnection,
    inbox: Option<InboxId>,
    stream: StreamId,
    connection: ConnectionId,
    mailbox: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM connectors.destinations_at_deploy d
                          JOIN connectors.streams s ON s.id = d.stream_id
                         WHERE d.stream_id = $1 AND d.connection_id = $2 AND d.mailbox_key = $3
                           AND d.inbox_id = s.inbox_id
                           AND ($4::uuid IS NULL OR d.inbox_id = $4))",
    )
    .bind(stream)
    .bind(connection)
    .bind(mailbox)
    .bind(inbox)
    .fetch_one(conn)
    .await?)
}

fn destination(stream: StreamId, connection: ConnectionId, mailbox: String) -> Destination {
    Destination {
        connection: *connection.as_uuid(),
        stream: *stream.as_uuid(),
        mailbox,
    }
}

#[cfg(test)]
mod limit_tests {
    use super::{Provider, max_attachment_bytes};

    #[test]
    fn microsoft_carries_about_two_megabytes_of_files() {
        let limit = max_attachment_bytes(Provider::Microsoft).unwrap();
        // Twice base64 inside 4 MB, less the headroom: about 2.1 MB.
        assert!((2_000_000..2_200_000).contains(&limit), "{limit}");
        assert_eq!(max_attachment_bytes(Provider::Google), None);
        assert_eq!(max_attachment_bytes(Provider::Imap), None);
    }
}
