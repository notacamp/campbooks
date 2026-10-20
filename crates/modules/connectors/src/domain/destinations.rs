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

use super::model::{ConnectionId, InboxId, StreamId};

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
    let rows: Vec<(StreamId, ConnectionId, String, bool)> = sqlx::query_as(
        "SELECT s.id, s.connection_id, s.mailbox_key,
                EXISTS (SELECT 1 FROM connectors.destinations_at_deploy d
                         WHERE d.stream_id = s.id AND d.inbox_id = s.inbox_id
                           AND d.connection_id = s.connection_id
                           AND d.mailbox_key = s.mailbox_key)
           FROM connectors.streams s
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
    let Some((stream, connection, mailbox, as_at_deploy)) = rows.into_iter().next() else {
        return Ok(None);
    };
    Ok(Some(Resolved {
        destination: destination(stream, connection, mailbox),
        as_at_deploy,
    }))
}

/// `stream`, its connection `connection` and its mailbox `mailbox` as a
/// destination, read as recorded at the deploy or not (in `conn`).
pub async fn resolved(
    conn: &mut PgConnection,
    stream: StreamId,
    connection: ConnectionId,
    mailbox: &str,
) -> Result<Resolved> {
    Ok(Resolved {
        as_at_deploy: as_at_deploy(conn, stream, connection, mailbox).await?,
        destination: destination(stream, connection, mailbox.to_owned()),
    })
}

/// Whether `stream` went through `connection` and `mailbox` when
/// destinations were first bound (`connectors.destinations_at_deploy`).
pub async fn as_at_deploy(
    conn: &mut PgConnection,
    stream: StreamId,
    connection: ConnectionId,
    mailbox: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM connectors.destinations_at_deploy
                         WHERE stream_id = $1 AND connection_id = $2 AND mailbox_key = $3)",
    )
    .bind(stream)
    .bind(connection)
    .bind(mailbox)
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
