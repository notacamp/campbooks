//! Settling a conflict with each choice, and taking the choice back.

use super::*;
use crate::domain::notion::tests::{Synced, at, para, snapshot};

/// A page whose one block "Hello" has a conflict: ours "Mine", Notion's
/// "Theirs" (edited at minute 7).
fn conflicted() -> (Synced, Uuid) {
    let mut s = Synced::new();
    s.apply(&snapshot(vec![para(1, "Hello", 1), para(2, "Other", 1)], 5, true));
    let bid = s.bid_of("Hello");
    s.edit_text(bid, "Mine");
    s.apply(&snapshot(vec![para(1, "Theirs", 7), para(2, "Other", 1)], 8, true));
    assert_eq!(s.tree().conflicts.len(), 1);
    (s, bid)
}

fn mapping(s: &Synced, bid: Uuid) -> Mapping {
    s.mappings.iter().find(|m| m.bid == bid).cloned().unwrap()
}

#[test]
fn keep_mine_keeps_the_block_moves_the_base_and_so_sends_it() {
    let (s, bid) = conflicted();
    let resolution = resolve(
        &s.doc,
        &s.resolver,
        bid,
        Choice::Mine,
        Some(&mapping(&s, bid)),
    )
    .unwrap();
    assert_eq!(s.texts(), ["Mine", "Other"]);
    assert_eq!(resolution.conflicts, 0);
    let upsert = resolution.upsert.expect("the base moves");
    assert_eq!(upsert.base_hash, hash(&para(1, "Theirs", 7)));
    assert_eq!(upsert.base_edited, at(7));
    // Ours now differs from the base: it reads as changed, and is sent.
    assert_ne!(hash(&s.tree().find(bid).unwrap().block), upsert.base_hash);
    assert_eq!(resolution.ours.text(), "Mine");
    assert_eq!(resolution.theirs.as_ref().map(Block::text).as_deref(), Some("Theirs"));
    assert!(resolution.update.is_some());
}

#[test]
fn use_notions_takes_its_version_and_leaves_nothing_to_send() {
    let (s, bid) = conflicted();
    let resolution = resolve(
        &s.doc,
        &s.resolver,
        bid,
        Choice::Notion,
        Some(&mapping(&s, bid)),
    )
    .unwrap();
    assert_eq!(s.texts(), ["Theirs", "Other"]);
    assert_eq!(resolution.conflicts, 0);
    let upsert = resolution.upsert.unwrap();
    assert_eq!(hash(&s.tree().find(bid).unwrap().block), upsert.base_hash);
}

#[test]
fn keep_both_adds_notions_version_after_ours_as_a_block_of_ours() {
    let (s, bid) = conflicted();
    let resolution = resolve(
        &s.doc,
        &s.resolver,
        bid,
        Choice::Both,
        Some(&mapping(&s, bid)),
    )
    .unwrap();
    assert_eq!(s.texts(), ["Mine", "Theirs", "Other"]);
    let extra = resolution.extra.expect("a block was made");
    assert_eq!(s.tree().find(extra).unwrap().block.text(), "Theirs");
    assert!(
        s.mappings.iter().all(|m| m.bid != extra),
        "it has no counterpart at Notion: it is to be added there"
    );
}

#[test]
fn when_notion_removed_the_block_each_choice_means_what_it_says() {
    let removed = || {
        let mut s = Synced::new();
        s.apply(&snapshot(vec![para(1, "Hello", 1), para(2, "Other", 1)], 5, true));
        let bid = s.bid_of("Hello");
        s.edit_text(bid, "Mine");
        s.apply(&snapshot(vec![para(2, "Other", 1)], 6, true));
        assert!(s.tree().conflict_of(bid).unwrap().removed);
        (s, bid)
    };
    // Keep mine: the block is ours alone now, no longer the same at Notion.
    let (s, bid) = removed();
    let kept = resolve(&s.doc, &s.resolver, bid, Choice::Mine, Some(&mapping(&s, bid))).unwrap();
    assert_eq!(s.texts(), ["Mine", "Other"]);
    assert_eq!(kept.deletes, vec![bid]);
    assert!(kept.upsert.is_none());
    // Keep both is the same as keep mine: there is no version of theirs.
    let (s, bid) = removed();
    let both = resolve(&s.doc, &s.resolver, bid, Choice::Both, Some(&mapping(&s, bid))).unwrap();
    assert_eq!(s.texts(), ["Mine", "Other"]);
    assert_eq!(both.deletes, vec![bid]);
    assert!(both.extra.is_none());
    // Use Notion's: ours goes too.
    let (s, bid) = removed();
    let theirs = resolve(&s.doc, &s.resolver, bid, Choice::Notion, Some(&mapping(&s, bid))).unwrap();
    assert_eq!(s.texts(), ["Other"]);
    assert_eq!(theirs.deletes, vec![bid]);
}

#[test]
fn a_choice_with_no_conflict_or_no_block_is_refused() {
    let mut s = Synced::new();
    s.apply(&snapshot(vec![para(1, "Hello", 1)], 5, true));
    let bid = s.bid_of("Hello");
    assert_eq!(
        resolve(&s.doc, &s.resolver, bid, Choice::Mine, None).unwrap_err(),
        Refused::NoConflict
    );
    assert_eq!(
        resolve(&s.doc, &s.resolver, Uuid::now_v7(), Choice::Mine, None).unwrap_err(),
        Refused::NoConflict
    );
}

#[test]
fn undoing_puts_the_conflict_back_as_it_was() {
    for choice in [Choice::Mine, Choice::Notion, Choice::Both] {
        let (s, bid) = conflicted();
        let before = s.texts();
        let conflict_before = s.tree().conflict_of(bid).cloned().unwrap();
        let resolution = resolve(&s.doc, &s.resolver, bid, choice, Some(&mapping(&s, bid))).unwrap();
        let theirs = resolution.theirs.clone();
        let taken = Taken {
            bid,
            choice,
            ours: &resolution.ours,
            theirs: theirs.as_ref(),
            theirs_edited: resolution.theirs_edited,
            extra: resolution.extra,
        };
        let undone = undo(&s.doc, &s.resolver, &taken).unwrap();
        assert_eq!(s.texts(), before, "{choice:?}");
        assert_eq!(undone.conflicts, 1, "{choice:?}");
        let conflict_after = s.tree().conflict_of(bid).cloned().unwrap();
        assert_eq!(conflict_after.hash, conflict_before.hash, "{choice:?}");
        assert_eq!(conflict_after.edited, conflict_before.edited);
        assert_eq!(
            conflict_after.theirs.as_ref().map(Block::text),
            conflict_before.theirs.as_ref().map(Block::text)
        );
    }
}

#[test]
fn undoing_twice_or_after_the_block_went_is_refused() {
    let (s, bid) = conflicted();
    let resolution = resolve(&s.doc, &s.resolver, bid, Choice::Mine, Some(&mapping(&s, bid))).unwrap();
    let taken = Taken {
        bid,
        choice: Choice::Mine,
        ours: &resolution.ours,
        theirs: resolution.theirs.as_ref(),
        theirs_edited: resolution.theirs_edited,
        extra: None,
    };
    undo(&s.doc, &s.resolver, &taken).unwrap();
    assert!(matches!(
        undo(&s.doc, &s.resolver, &taken),
        Err(Refused::Body(_))
    ));
    s.delete(bid);
    // The conflict is gone with its block, so the block cannot be found.
    let gone = undo(&s.doc, &s.resolver, &taken);
    assert!(gone.is_err());
}
