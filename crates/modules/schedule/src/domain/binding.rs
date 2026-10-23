//! What binds a change request to the authenticated request that made it
//! (campbooks-cloud#227): a MAC under the kernel's [`Binding`], whose key the
//! database never holds. `domain::outbound::request` signs what it records
//! (workspace, request, member, calendar, event, connection, the kind of
//! change and the `calendar-change` blob); the kernel's calendar-requests
//! port verifies it before it tells connectors the change stands. SQL in the
//! workspace can edit or plant a row; it cannot make the tag verify.

use campbooks_kernel::{
    binding::{Binding, Fields},
    blobs::BlobRef,
    tenancy::{UserId, WorkspaceId},
};

use crate::model::{CalendarId, ConnectionId, EventId, RequestId};

/// A change request, as it is recorded.
const CHANGE_REQUEST: &str = "campbooks/schedule.change-request/v1";

/// A recorded change request.
pub struct ChangeRequest<'a> {
    /// The workspace.
    pub workspace: WorkspaceId,
    /// The request.
    pub request: RequestId,
    /// The member who asked.
    pub member: UserId,
    /// The calendar.
    pub calendar: CalendarId,
    /// The event.
    pub event: EventId,
    /// The connection whose grant makes the change.
    pub connection: ConnectionId,
    /// `create`, `update`, `update_occurrence`, `delete` or `respond`.
    pub change: &'a str,
    /// The `calendar-change` document.
    pub body: &'a BlobRef,
}

impl ChangeRequest<'_> {
    fn fields(&self) -> Fields {
        let mut fields = Fields::new(CHANGE_REQUEST);
        fields
            .uuid(self.workspace.as_uuid())
            .uuid(self.request.as_uuid())
            .uuid(self.member.as_uuid())
            .uuid(self.calendar.as_uuid())
            .uuid(self.event.as_uuid())
            .uuid(self.connection.as_uuid())
            .str(self.change)
            .bytes(self.body.hash.as_bytes())
            .uint(self.body.size)
            .str(&self.body.content_type);
        fields
    }

    /// The tag of the request; `None` when the instance has no secret.
    pub fn sign(&self, binding: &Binding) -> Option<Vec<u8>> {
        binding.tag(&self.fields())
    }

    /// Whether `mac` is the tag of this request.
    pub fn holds(&self, binding: &Binding, mac: Option<&[u8]>) -> bool {
        binding.verify(&self.fields(), mac)
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::{
        blobs::BlobHash,
        crypto::{SECRET_KEY_LEN, SecretKey},
    };

    use super::*;

    fn request<'a>(body: &'a BlobRef, change: &'a str) -> ChangeRequest<'a> {
        ChangeRequest {
            workspace: WorkspaceId::new(),
            request: RequestId::new(),
            member: UserId::new(),
            calendar: CalendarId::new(),
            event: EventId::new(),
            connection: ConnectionId::new(),
            change,
            body,
        }
    }

    #[test]
    fn a_tag_holds_for_its_request_only() {
        let binding = Binding::new(&SecretKey::from_bytes([3; SECRET_KEY_LEN]));
        let body = BlobRef {
            hash: BlobHash::of(b"change"),
            size: 6,
            content_type: "application/json".into(),
        };
        let asked = request(&body, "update");
        let mac = asked.sign(&binding).unwrap();
        assert!(asked.holds(&binding, Some(&mac)));
        // Another change, another event: not the same request.
        let other = ChangeRequest {
            change: "delete",
            ..request(&body, "delete")
        };
        assert!(!other.holds(&binding, Some(&mac)));
        assert!(!asked.holds(&binding, None));
        assert!(!asked.holds(&Binding::disabled(), Some(&mac)));
    }
}
