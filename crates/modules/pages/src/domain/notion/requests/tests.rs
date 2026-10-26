//! A stored request keeps ids, kinds, times and a hash, and hands the
//! connector exactly its changes.

use campbooks_kernel::outbound::PageItemKind;

use super::*;
use crate::domain::notion::tests::{at, id};

fn item(action: &str) -> StoredItem {
    StoredItem {
        seq: 3,
        action: action.to_owned(),
        bid: id(1),
        kind: "paragraph".to_owned(),
        external: Some(id(2)),
        parent: Some(id(3)),
        parent_block: None,
        after: Some(id(4)),
        after_block: Some(id(5)),
        base_edited: Some(at(7)),
        hash: "ab".repeat(32),
    }
}

#[test]
fn each_action_reaches_the_connector_as_its_kind() {
    for (action, kind) in [
        ("update", PageItemKind::Update),
        ("append", PageItemKind::Append),
        ("delete", PageItemKind::Delete),
    ] {
        let sent = item(action).for_connector().unwrap();
        assert_eq!(sent.kind, kind);
        assert_eq!(sent.block, id(1));
        assert_eq!(sent.external, Some(id(2)));
        assert_eq!(sent.parent, Some(id(3)));
        assert_eq!(sent.after, Some(id(4)));
        assert_eq!(sent.after_block, Some(id(5)));
        assert_eq!(sent.base_edited_at, Some(at(7)));
    }
    // An action this module does not make is not passed on.
    assert!(item("rearrange").for_connector().is_none());
}

#[test]
fn what_is_stored_has_no_text() {
    let stored = serde_json::to_string(&item("update")).unwrap();
    assert!(stored.contains("\"hash\""));
    assert!(!stored.contains("text") && !stored.contains("runs"));
}

#[test]
fn the_binding_covers_every_field_of_every_change() {
    let body = campbooks_kernel::blobs::BlobRef {
        hash: campbooks_kernel::blobs::BlobHash::of(b"x"),
        size: 1,
        content_type: "application/json".to_owned(),
    };
    let (workspace, page) = (WorkspaceId::new(), PageId::new());
    let digest = |items: &[StoredItem], request: Uuid, body: &campbooks_kernel::blobs::BlobRef| {
        Bound {
            workspace,
            request,
            page,
            connection: id(8),
            notion_page: id(9),
            items,
            body,
        }
        .fields()
        .digest()
    };
    let base = [item("update")];
    let same = digest(&base, id(1), &body);
    assert_eq!(same, digest(&base, id(1), &body), "it is deterministic");
    assert_ne!(same, digest(&base, id(2), &body), "the request");
    let other_body = campbooks_kernel::blobs::BlobRef {
        hash: campbooks_kernel::blobs::BlobHash::of(b"y"),
        ..body.clone()
    };
    assert_ne!(same, digest(&base, id(1), &other_body), "the document");
    assert_ne!(
        same,
        digest(&[item("update"), item("delete")], id(1), &body),
        "one more change"
    );
    let edits: [fn(&mut StoredItem); 9] = [
        |i| i.action = "delete".to_owned(),
        |i| i.bid = id(99),
        |i| i.kind = "heading".to_owned(),
        |i| i.external = Some(id(99)),
        |i| i.parent = None,
        |i| i.parent_block = Some(id(99)),
        |i| i.after = None,
        |i| i.base_edited = Some(at(99)),
        |i| i.hash = "cd".repeat(32),
    ];
    for (n, edit) in edits.iter().enumerate() {
        let mut changed = item("update");
        edit(&mut changed);
        assert_ne!(same, digest(&[changed], id(1), &body), "field {n}");
    }
}
