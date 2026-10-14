//! The checks, rule by rule: only the vocabulary, only what the input
//! supports, and what `create_rule` would accept.

use super::*;

fn ctx(sentence: &str) -> Context {
    Context {
        sentence: sentence.to_owned(),
        inboxes: vec![
            InboxFacts {
                id: "inbox-1".into(),
                address: "ines@ateliersousa.example".into(),
                teachable: true,
                shared: false,
            },
            InboxFacts {
                id: "inbox-2".into(),
                address: "info@ateliersousa.example".into(),
                teachable: true,
                shared: true,
            },
            InboxFacts {
                id: "inbox-3".into(),
                address: "accounts@ateliersousa.example".into(),
                teachable: false,
                shared: true,
            },
        ],
        labels: vec![
            LabelFacts {
                id: "label-a".into(),
                inbox: "inbox-1".into(),
                name: "Invoices".into(),
            },
            LabelFacts {
                id: "label-b".into(),
                inbox: "inbox-2".into(),
                name: "invoices".into(),
            },
        ],
    }
}

fn rule(who: &str, conditions: &[(&str, &str)], actions: &[(&str, &str)]) -> ModelRule {
    ModelRule {
        who: who.to_owned(),
        conditions: conditions
            .iter()
            .map(|(field, value)| ModelCondition {
                field: (*field).to_owned(),
                value: (*value).to_owned(),
            })
            .collect(),
        actions: actions
            .iter()
            .map(|(kind, value)| ModelAction {
                kind: (*kind).to_owned(),
                value: (*value).to_owned(),
            })
            .collect(),
    }
}

fn output(rules: Vec<ModelRule>) -> ModelOutput {
    ModelOutput {
        outcome: "rules".into(),
        unsupported: "none".into(),
        rules,
    }
}

#[test]
fn a_person_without_an_address_becomes_a_question() {
    let checked = check(
        &output(vec![rule("my accountant", &[], &[("surface", "always")])]),
        &ctx("anything from my accountant always matters"),
    );
    assert_eq!(checked.outcome, CompileOutcome::Rules);
    assert_eq!(checked.proposals.len(), 1);
    let proposal = &checked.proposals[0];
    assert!(proposal.conditions.is_empty());
    assert_eq!(
        proposal.actions,
        [ProposedAction::Surface {
            value: SurfaceKey::Always
        }]
    );
    assert_eq!(proposal.who.as_ref().unwrap().phrase, "my accountant");
    let inboxes: Vec<_> = proposal.inboxes.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(inboxes, ["inbox-1", "inbox-2"], "never the inbox only read");
    assert!(proposal.inboxes[1].shared);
}

#[test]
fn an_address_in_the_sentence_is_used() {
    let checked = check(
        &output(vec![rule(
            "",
            &[("sender_address", "Rita@Silva-Contas.example")],
            &[("surface", "always")],
        )]),
        &ctx("always show mail from rita@silva-contas.example"),
    );
    assert_eq!(
        checked.proposals[0].conditions,
        [ProposedCondition::SenderAddress(
            "rita@silva-contas.example".into()
        )]
    );
}

#[test]
fn an_address_inside_who_needs_no_question() {
    let checked = check(
        &output(vec![rule(
            "Rita (rita@silva-contas.example)",
            &[],
            &[("surface", "always")],
        )]),
        &ctx("mail from Rita (rita@silva-contas.example) always matters"),
    );
    let proposal = &checked.proposals[0];
    assert!(proposal.who.is_none());
    assert_eq!(
        proposal.conditions,
        [ProposedCondition::SenderAddress(
            "rita@silva-contas.example".into()
        )]
    );
}

#[test]
fn invented_values_are_dropped() {
    let sentence = "anything from my accountant always matters";
    for bad in [
        rule(
            "",
            &[("sender_address", "boss@corp.example")],
            &[("surface", "always")],
        ),
        rule("", &[("sender_domain", "corp.example")], &[("archive", "")]),
        rule(
            "",
            &[("subject_contains", "invoice")],
            &[("surface", "always")],
        ),
        rule("my boss", &[], &[("surface", "always")]),
        rule("", &[("label", "Board")], &[("archive", "")]),
    ] {
        let checked = check(&output(vec![bad.clone()]), &ctx(sentence));
        assert!(checked.proposals.is_empty(), "{bad:?}");
        assert_eq!(checked.outcome, CompileOutcome::Unclear);
    }
}

#[test]
fn outside_the_vocabulary_is_dropped() {
    let sentence = "forward mail from rita@silva-contas.example";
    for bad in [
        rule(
            "",
            &[("sender_address", "rita@silva-contas.example")],
            &[("forward", "rita@silva-contas.example")],
        ),
        rule("", &[("body_contains", "rita")], &[("surface", "always")]),
        rule(
            "",
            &[("sender_address", "rita@silva-contas.example")],
            &[("surface", "sometimes")],
        ),
        rule(
            "",
            &[("sender_address", "rita@silva-contas.example")],
            &[("move_to_stream", "spam")],
        ),
        rule("", &[("header_present", "x_mailer")], &[("archive", "")]),
    ] {
        assert!(
            check(&output(vec![bad.clone()]), &ctx(sentence))
                .proposals
                .is_empty(),
            "{bad:?}"
        );
    }
}

#[test]
fn free_mail_domains_are_refused() {
    let checked = check(
        &output(vec![rule(
            "",
            &[("sender_domain", "gmail.com")],
            &[("surface", "never")],
        )]),
        &ctx("anything from gmail.com never needs me"),
    );
    assert!(checked.proposals.is_empty());
    let who = check(
        &output(vec![rule("gmail.com people", &[], &[("surface", "never")])]),
        &ctx("gmail.com people never need me"),
    );
    assert!(who.proposals.is_empty());
}

#[test]
fn what_create_rule_rejects_is_never_shown() {
    let sentence = "archive everything in updates";
    // Stream only: it doesn't say which mail.
    let broad = check(
        &output(vec![rule("", &[("stream", "updates")], &[("archive", "")])]),
        &ctx(sentence),
    );
    assert!(broad.proposals.is_empty());
    // Two actions of the same kind.
    let twice = check(
        &output(vec![rule(
            "",
            &[("header_present", "list_id")],
            &[("archive", ""), ("archive", "")],
        )]),
        &ctx("archive mailing lists"),
    );
    assert!(twice.proposals.is_empty());
}

#[test]
fn newsletters_never_need_me() {
    let checked = check(
        &output(vec![rule(
            "",
            &[("header_present", "list_id")],
            &[("surface", "never")],
        )]),
        &ctx("newsletters never need me"),
    );
    assert_eq!(
        checked.proposals[0].conditions,
        [ProposedCondition::Header(ProposedHeader {
            name: HeaderKey::ListId
        })]
    );
}

#[test]
fn labels_resolve_per_inbox() {
    let checked = check(
        &output(vec![rule(
            "",
            &[("subject_contains", "invoice")],
            &[("add_label", "Invoices")],
        )]),
        &ctx("label mail with invoice in the subject as Invoices"),
    );
    assert_eq!(checked.proposals.len(), 2, "label ids differ per inbox");
    let labels: Vec<_> = checked
        .proposals
        .iter()
        .map(|p| (p.inboxes[0].id.clone(), p.actions[0].clone()))
        .collect();
    assert!(labels.contains(&(
        "inbox-1".into(),
        ProposedAction::AddLabel {
            label: "label-a".into()
        }
    )));
    assert!(labels.contains(&(
        "inbox-2".into(),
        ProposedAction::AddLabel {
            label: "label-b".into()
        }
    )));
}

#[test]
fn unsupported_keeps_the_closest_rule() {
    let checked = check(
        &ModelOutput {
            outcome: "unsupported".into(),
            unsupported: "sends_mail".into(),
            rules: vec![rule(
                "",
                &[("subject_contains", "invoice")],
                &[("add_label", "Invoices")],
            )],
        },
        &ctx("forward every invoice to my accountant"),
    );
    assert_eq!(checked.outcome, CompileOutcome::Unsupported);
    assert_eq!(checked.unsupported, Some(UnsupportedReason::SendsMail));
    assert!(!checked.proposals.is_empty());
}

#[test]
fn at_most_three_rules() {
    let many: Vec<_> = (0..6)
        .map(|_| {
            rule(
                "",
                &[("header_present", "list_id")],
                &[("surface", "never")],
            )
        })
        .collect();
    let checked = check(&output(many), &ctx("newsletters never need me"));
    assert!(checked.proposals.len() <= MAX_RULES);
}

#[test]
fn own_addresses_count_as_the_members() {
    let checked = check(
        &output(vec![rule(
            "",
            &[("sender_domain", "ateliersousa.example")],
            &[("move_to_stream", "updates")],
        )]),
        &ctx("mail from my own studio goes to updates"),
    );
    assert_eq!(
        checked.proposals[0].actions,
        [ProposedAction::MoveToStream {
            stream: StreamKey::Updates
        }]
    );
}
