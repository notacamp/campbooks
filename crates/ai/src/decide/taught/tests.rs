use std::collections::BTreeMap;

use super::*;
use crate::decide::signals::email::{Party, subject_template, triage_questions};

fn rule(
    id: &str,
    conditions: Vec<Condition>,
    actions: Vec<Action>,
    confirmed_at: i64,
) -> TaughtRule {
    TaughtRule {
        id: id.into(),
        conditions,
        actions,
        confirmed_at,
    }
}

fn to_stream(stream: &str) -> Action {
    Action::MoveToStream {
        stream: stream.into(),
    }
}

fn facts<'a>(sender: &'a str, subject: &'a str, template: Option<&'a str>) -> Facts<'a> {
    Facts {
        sender_address: sender,
        subject,
        subject_template: template,
        stream: "primary",
        ..Facts::default()
    }
}

#[test]
fn a_sender_rule_moves_its_mail() {
    let rules = [rule(
        "r1",
        vec![Condition::SenderAddress("notifications@github.com".into())],
        vec![to_stream("updates")],
        1,
    )];
    let out = evaluate(
        &rules,
        &facts("Notifications@GitHub.com", "[acme/web] PR merged", None),
    );
    assert_eq!(
        out.stream,
        Some(Applied {
            value: "updates".into(),
            rule: "r1".into()
        })
    );
    assert!(evaluate(&rules, &facts("marta@example.com", "Hi", None)).is_empty());
}

#[test]
fn the_more_specific_rule_wins_its_slot() {
    let shop = rule(
        "domain",
        vec![Condition::SenderDomain("shop.example".into())],
        vec![to_stream("receipts")],
        9,
    );
    let shipped = rule(
        "template",
        vec![
            Condition::SenderDomain("shop.example".into()),
            Condition::SubjectTemplate("your order # has shipped".into()),
        ],
        vec![to_stream("updates"), Action::MarkRead],
        1,
    );
    let rules = [shop, shipped];
    let notice = evaluate(
        &rules,
        &facts(
            "order-1@mail.shop.example",
            "Your order 12 has shipped",
            Some("your order # has shipped"),
        ),
    );
    assert_eq!(notice.stream.unwrap().rule, "template");
    assert_eq!(notice.read.as_deref(), Some("template"));
    let invoice = evaluate(&rules, &facts("billing@shop.example", "Invoice", None));
    assert_eq!(invoice.stream.unwrap().value, "receipts");
    assert!(invoice.read.is_none());
}

#[test]
fn ties_go_to_the_latest_confirmed_then_the_smallest_id() {
    let cond = || vec![Condition::SenderAddress("a@b.example".into())];
    let older = rule("b", cond(), vec![to_stream("receipts")], 1);
    let newer = rule("c", cond(), vec![to_stream("updates")], 2);
    let same_time = rule("a", cond(), vec![to_stream("newsletters")], 2);
    let out = evaluate(
        &[older.clone(), newer.clone()],
        &facts("a@b.example", "x", None),
    );
    assert_eq!(out.stream.unwrap().rule, "c");
    let out = evaluate(&[older, newer, same_time], &facts("a@b.example", "x", None));
    assert_eq!(out.stream.unwrap().rule, "a");
}

#[test]
fn the_same_input_always_gives_the_same_outcome() {
    let rules: Vec<TaughtRule> = (0..20)
        .map(|i| {
            rule(
                &format!("r{i:02}"),
                vec![Condition::SenderDomain("github.com".into())],
                vec![
                    to_stream(if i % 2 == 0 { "updates" } else { "receipts" }),
                    Action::AddLabel {
                        label: format!("l{}", i % 3),
                    },
                ],
                i64::from(i % 4),
            )
        })
        .collect();
    let f = facts("notifications@github.com", "x", None);
    let first = evaluate(&rules, &f);
    let mut reversed = rules.clone();
    reversed.reverse();
    assert_eq!(evaluate(&reversed, &f), first);
    assert_eq!(first.labels.len(), 3, "labels add up, once each");
}

#[test]
fn suspicious_mail_is_never_hidden() {
    let rules = [rule(
        "hide",
        vec![Condition::SenderAddress("notifications@github.com".into())],
        vec![
            to_stream("updates"),
            Action::Archive,
            Action::MarkRead,
            Action::NeedsReply { value: false },
        ],
        1,
    )];
    let mut f = facts("notifications@github.com", "Verify your account", None);
    f.suspicious = true;
    let out = evaluate(&rules, &f);
    assert!(out.stream.is_none() && out.archive.is_none() && out.read.is_none());
    assert!(out.needs_reply.is_none());
}

#[test]
fn always_show_keeps_other_rules_from_hiding() {
    let hide = rule(
        "hide",
        vec![Condition::SenderDomain("mfiscal.example".into())],
        vec![to_stream("newsletters"), Action::Archive],
        5,
    );
    let show = rule(
        "show",
        vec![Condition::SenderAddress("ines@mfiscal.example".into())],
        vec![Action::Surface {
            value: Surface::Always,
        }],
        1,
    );
    let out = evaluate(&[hide, show], &facts("ines@mfiscal.example", "IRS", None));
    assert!(out.archive.is_none() && out.stream.is_none());
    assert_eq!(out.surface.unwrap().value, Surface::Always);
}

#[test]
fn every_condition_must_hold() {
    let labels = vec!["press".to_owned()];
    let r = rule(
        "r",
        vec![
            Condition::ListId("<News.Studio.Example>".into()),
            Condition::Label("press".into()),
            Condition::Header(HeaderTest {
                name: HeaderName::Precedence,
                value: Some("bulk".into()),
            }),
            Condition::SubjectContains("Pull Request".into()),
        ],
        vec![Action::MarkRead],
        1,
    );
    let mut f = facts("news@studio.example", "Nova pull-request aberta", None);
    f.list_id = Some("Studio news <news.studio.example>");
    f.precedence = Some(" BULK ");
    f.labels = &labels;
    assert!(r.matches(&f));
    f.precedence = None;
    assert!(!r.matches(&f));
}

#[test]
fn subject_words_ignore_case_and_accents() {
    assert!(contains_words("Fatura N.º 12 — Água e Luz", "agua e luz"));
    assert!(!contains_words("Pullrequest merged", "pull request"));
    assert!(contains_words(
        "Your PULL request was merged",
        "pull request"
    ));
}

#[test]
fn validation() {
    let ok = |c: Vec<Condition>, a: Vec<Action>| TaughtRule::validate(&c, &a);
    let sender = || Condition::SenderAddress("a@b.example".into());
    assert_eq!(ok(vec![sender()], vec![to_stream("updates")]), Ok(()));
    assert_eq!(ok(vec![], vec![Action::Archive]), Err(Invalid::Conditions));
    assert_eq!(ok(vec![sender()], vec![]), Err(Invalid::Actions));
    assert_eq!(
        ok(
            vec![sender()],
            vec![to_stream("updates"), to_stream("receipts")]
        ),
        Err(Invalid::SameSlot)
    );
    assert_eq!(
        ok(vec![sender()], vec![to_stream("spam")]),
        Err(Invalid::Value)
    );
    assert_eq!(
        ok(
            vec![Condition::SenderDomain("gmail.com".into())],
            vec![Action::Archive]
        ),
        Err(Invalid::FreeMailDomain)
    );
    assert_eq!(
        ok(
            vec![
                Condition::SenderDomain("gmail.com".into()),
                Condition::SubjectTemplate("weekly report #".into())
            ],
            vec![Action::Archive]
        ),
        Ok(())
    );
    for suffix in ["co.uk", "com.br", "com"] {
        assert_eq!(
            ok(
                vec![
                    Condition::SenderDomain(suffix.into()),
                    Condition::SubjectTemplate("weekly report #".into())
                ],
                vec![Action::Archive]
            ),
            Err(Invalid::Value),
            "{suffix}"
        );
    }
    assert_eq!(
        ok(
            vec![Condition::SenderDomain("hotmail.co.uk".into())],
            vec![Action::Archive]
        ),
        Err(Invalid::FreeMailDomain)
    );
    assert_eq!(
        ok(
            vec![Condition::Stream("updates".into())],
            vec![Action::MarkRead]
        ),
        Err(Invalid::TooBroad)
    );
    assert_eq!(
        ok(
            vec![sender()],
            vec![
                Action::AddLabel { label: "a".into() },
                Action::AddLabel { label: "b".into() }
            ]
        ),
        Ok(()),
        "labels add up"
    );
}

#[test]
fn conditions_and_actions_have_a_stable_json_shape() {
    let c = serde_json::to_value(Condition::SenderAddress("a@b.example".into())).unwrap();
    assert_eq!(
        c,
        serde_json::json!({"field": "sender_address", "value": "a@b.example"})
    );
    let h = serde_json::to_value(Condition::Header(HeaderTest {
        name: HeaderName::ListId,
        value: None,
    }))
    .unwrap();
    assert_eq!(
        h,
        serde_json::json!({"field": "header", "value": {"name": "list_id"}})
    );
    let a = serde_json::to_value(Action::NeedsReply { value: false }).unwrap();
    assert_eq!(
        a,
        serde_json::json!({"action": "needs_reply", "value": false})
    );
    let s = serde_json::to_value(Action::Surface {
        value: Surface::Never,
    })
    .unwrap();
    assert_eq!(
        s,
        serde_json::json!({"action": "surface", "value": "never"})
    );
}

#[test]
fn templates() {
    let t = |s: &str| subject_template(s);
    assert_eq!(
        t("Re: Your order #48213 has shipped").as_deref(),
        Some("your order # has shipped")
    );
    assert_eq!(
        t("Fwd: RE: Your order 77 has shipped").as_deref(),
        Some("your order # has shipped")
    );
    assert_eq!(t("Invoice 2026-10-03"), None);
    assert_eq!(
        t("ENC: A sua fatura 2026/10 está disponível").as_deref(),
        Some("a sua fatura # está disponível")
    );
    assert_eq!(
        t("RV: Su pedido AB12CD34 ha sido enviado").as_deref(),
        Some("su pedido * ha sido enviado")
    );
    assert_eq!(
        t("TR: Votre colis 3 sur 5 arrive le 14/10/2026").as_deref(),
        Some("votre colis # sur # arrive le #")
    );
    assert_eq!(
        t("[acme/web] Fix toast offset (#4182)").as_deref(),
        Some("[acme/web] fix toast offset (#)")
    );
    let long = "word ".repeat(60);
    assert!(t(&long).unwrap().chars().count() <= 120);
}

#[tokio::test]
async fn as_a_strategy_it_answers_stream_and_needs() {
    let rules = TaughtRules::new(vec![rule(
        "r1",
        vec![Condition::SenderAddress("notifications@github.com".into())],
        vec![to_stream("updates"), Action::NeedsReply { value: false }],
        1,
    )]);
    let envelope = Envelope {
        from: Party {
            name: "GitHub".into(),
            address: "notifications@github.com".into(),
        },
        subject: "[acme/web] PR merged".into(),
        headers: BTreeMap::new(),
        ..Envelope::default()
    };
    let qs = triage_questions();
    let out = rules.decide(&envelope, &qs).await.unwrap();
    let stream = out.get(&qs, triage::STREAM).unwrap();
    assert_eq!(
        stream.answer.value,
        crate::decide::AnswerValue::Choice("updates".into())
    );
    assert_eq!(stream.answer.confidence, Some(1.0));
    assert_eq!(stream.reasons[0].rule, REASON);
    assert_eq!(stream.reasons[0].evidence, "r1");
    assert_eq!(
        out.get(&qs, triage::NEEDS_REPLY).unwrap().answer.noul(),
        Some(false)
    );
    assert!(out.get(&qs, triage::SUSPICIOUS).is_none());
    assert_eq!(out.remote_calls, 0);
}

#[test]
fn a_stored_rule_naming_a_public_suffix_matches_nothing() {
    let broad = [rule(
        "r1",
        vec![Condition::SenderDomain("github.io".into())],
        vec![to_stream("updates")],
        1,
    )];
    assert!(evaluate(&broad, &facts("a@a.github.io", "Hi", None)).is_empty());
    assert!(evaluate(&broad, &facts("a@github.io", "Hi", None)).is_empty());
    let owned = [rule(
        "r2",
        vec![Condition::SenderDomain("github.com".into())],
        vec![to_stream("updates")],
        1,
    )];
    assert!(!evaluate(&owned, &facts("a@mail.github.com", "Hi", None)).is_empty());
}

#[test]
fn a_rule_against_mail_failing_its_sender_checks_reports_it_even_when_suspicious() {
    let rules = [rule(
        "fake",
        vec![
            Condition::SenderDomain("paypal.com".into()),
            Condition::SenderChecksFail,
        ],
        vec![Action::Spam],
        1,
    )];
    let fake = Facts {
        suspicious: true,
        auth_failed: true,
        ..facts("service@paypal.com", "Your account has been limited", None)
    };
    let out = evaluate(&rules, &fake);
    assert_eq!(
        out.bin,
        Some(Applied {
            value: Bin::Spam,
            rule: "fake".into()
        })
    );
    // The real sender passes its checks: the rule leaves it alone.
    let real = facts("service@paypal.com", "Your receipt", None);
    assert!(evaluate(&rules, &real).is_empty());
}

#[test]
fn always_show_outranks_the_trash() {
    let rules = [
        rule(
            "bin",
            vec![Condition::SenderDomain("shop.example".into())],
            vec![Action::Trash],
            1,
        ),
        rule(
            "keep",
            vec![Condition::SenderAddress("ines@shop.example".into())],
            vec![Action::Surface {
                value: Surface::Always,
            }],
            2,
        ),
    ];
    let out = evaluate(&rules, &facts("ines@shop.example", "Hello", None));
    assert_eq!(out.bin, None);
    let other = evaluate(&rules, &facts("deals@shop.example", "Sale", None));
    assert_eq!(other.bin.map(|b| b.value), Some(Bin::Trash));
}

#[test]
fn paid_is_its_own_slot_and_never_for_suspicious_mail() {
    let rules = [rule(
        "edp",
        vec![Condition::SenderAddress("faturas@edp.example".into())],
        vec![Action::MarkPaid],
        1,
    )];
    let bill = facts("faturas@edp.example", "Fatura outubro", None);
    assert_eq!(evaluate(&rules, &bill).paid.as_deref(), Some("edp"));
    let suspicious = Facts {
        suspicious: true,
        ..bill
    };
    assert_eq!(evaluate(&rules, &suspicious).paid, None);
    assert_eq!(
        TaughtRule::validate(
            &[Condition::SenderAddress("a@b.example".into())],
            &[Action::Trash, Action::Spam]
        ),
        Err(Invalid::SameSlot)
    );
    // Failed checks alone say nothing about which mail.
    assert_eq!(
        TaughtRule::validate(&[Condition::SenderChecksFail], &[Action::Spam]),
        Err(Invalid::TooBroad)
    );
    let json = serde_json::to_value(Condition::SenderChecksFail).unwrap();
    assert_eq!(json, serde_json::json!({"field": "sender_checks_fail"}));
}
