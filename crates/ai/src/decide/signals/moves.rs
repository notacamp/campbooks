//! Whose move it is (campbooks-cloud#293): who is expected to act next
//! after an email, asked in the same call as the triage questions.
//!
//! Every other need asks what the recipient owes. These two ask whose turn
//! it is: [`question::NEXT_MOVE`], a choice of [`NextMove`], and, when the
//! email names someone who may be awaited, [`question::AWAITED`], a choice
//! over those people (`p1` to `p6`, in [`awaitable`]'s order) and `none`.
//! The model reads who is expected to act; the facts only decide who it may
//! name: the options are the email's own people, never an address the model
//! made up, and the inbox itself, a no-reply address or a sender that is a
//! machine is never one of them (ADR 0018). Nothing here reads what an
//! email says, and the facts answer neither question.
//!
//! Neither is measured by an operating point yet, so the pass only records
//! them; like the catalogue's, an answer that is missing or outside its
//! options records nothing and fails nothing ([`question::is_move`]).
//! [`awaited_party`] maps a stored `pN` back to the person it named when it
//! was asked.

use super::{
    email::{Envelope, Party},
    facts::awaitable,
    inbox::quoted,
};
use crate::decide::Question;

/// The questions about whose move it is.
pub mod question {
    /// Who is expected to act next (`choice`: [`super::NextMove`]).
    pub const NEXT_MOVE: &str = "next_move";
    /// Which of the email's people is expected to act next, when it is
    /// neither the sender nor the recipient (`choice`: `p1`…`p6`, `none`).
    pub const AWAITED: &str = "awaited";
    /// The `awaited` option that names nobody.
    pub const NONE: &str = "none";

    /// Every move question, in the order they are asked.
    pub const ALL: [&str; 2] = [NEXT_MOVE, AWAITED];

    /// Whether `id` is one of the move questions: an answer to one that is
    /// missing or does not fit is nothing to record, never a failed call
    /// (the triage questions asked with it keep their strict checks).
    pub fn is_move(id: &str) -> bool {
        ALL.contains(&id)
    }
}

/// Who is expected to act next after an email, from the recipient's point
/// of view: the `next_move` question's options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextMove {
    /// The recipient is asked to reply, decide, do or pay something.
    Recipient,
    /// The sender said they will send or do something next, or is expected
    /// to come back to the recipient.
    Sender,
    /// Someone else on the email is asked to do something for the recipient
    /// or to get in touch with them.
    SomeoneElse,
    /// It informs, thanks or settles the matter.
    Nobody,
}

impl NextMove {
    /// Every option, in the question's order.
    pub const ALL: [Self; 4] = [
        Self::Recipient,
        Self::Sender,
        Self::SomeoneElse,
        Self::Nobody,
    ];

    /// The option's label: the stored answer.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Recipient => "recipient",
            Self::Sender => "sender",
            Self::SomeoneElse => "someone_else",
            Self::Nobody => "nobody",
        }
    }

    /// The option labelled `label`, if one is.
    pub fn parse(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == label)
    }
}

/// The `next_move` question, worded for a `decide` model.
pub fn next_move_question() -> Question {
    Question::choice(
        question::NEXT_MOVE,
        "After this email, who is expected to act next?",
        [
            (
                NextMove::Recipient.as_str(),
                "The recipient is asked to reply, decide, do or pay something",
            ),
            (
                NextMove::Sender.as_str(),
                "The sender said they will send or do something next, or is expected to come \
                 back to the recipient",
            ),
            (
                NextMove::SomeoneElse.as_str(),
                "Someone else on the email is asked to do something for the recipient or to get \
                 in touch with them",
            ),
            (
                NextMove::Nobody.as_str(),
                "Nobody: the email informs, thanks or settles the matter",
            ),
        ],
    )
}

/// The `awaited` question about `envelope`: one option per person who may
/// be awaited ([`awaitable`]) and `none`; `None` when nobody may be, so the
/// question is not asked. Names and addresses are sender-controlled and
/// quoted as the inbox context quotes them (one line, no control
/// characters, at most 80 characters).
pub fn awaited_question(envelope: &Envelope) -> Option<Question> {
    let people = awaitable(envelope);
    if people.is_empty() {
        return None;
    }
    let mut options: Vec<(String, String)> = people
        .iter()
        .enumerate()
        .map(|(index, person)| (option_label(index), describe(envelope, person)))
        .collect();
    options.push((
        question::NONE.to_owned(),
        "None of them, or nobody else is expected to act".to_owned(),
    ));
    Some(Question::choice(
        question::AWAITED,
        "If someone other than the sender and the recipient is expected to act next, who?",
        options,
    ))
}

/// The move questions to ask about `envelope`: `next_move`, then `awaited`
/// when someone may be awaited.
pub fn move_questions(envelope: &Envelope) -> Vec<Question> {
    let mut questions = vec![next_move_question()];
    questions.extend(awaited_question(envelope));
    questions
}

/// The person an `awaited` answer named: the option `label` (`p2`) mapped
/// back through [`awaitable`], which orders people the same way each time.
/// `None` for `none`, for a label that is not an option, and when the
/// people are no longer those who were asked about.
pub fn awaited_party(envelope: &Envelope, label: &str) -> Option<Party> {
    let index = label
        .strip_prefix('p')?
        .parse::<usize>()
        .ok()?
        .checked_sub(1)?;
    // `p01` and the like are not options.
    if option_label(index) != label {
        return None;
    }
    awaitable(envelope).into_iter().nth(index)
}

/// The `awaited` option that names the person at `address` (compared
/// without regard to case) among those who may be awaited ([`awaitable`]):
/// the label a member's choice is filed under. `None` when that address is
/// not one of them.
pub fn awaited_label(envelope: &Envelope, address: &str) -> Option<String> {
    awaitable(envelope)
        .iter()
        .position(|p| p.address.trim().eq_ignore_ascii_case(address.trim()))
        .map(option_label)
}

/// Whether an `awaited` option's `description`, as it was asked, still names
/// `person`'s address: the check that a stored `pN`, mapped back through
/// today's headers ([`awaited_party`]), is the person the model was shown.
/// Only the address [`describe`] put last counts, whole: a display name
/// that holds an address (`Bob <ana@studio.example>`) names nobody.
pub fn describes(description: &str, person: &Party) -> bool {
    let address = quoted(&person.address);
    if address.is_empty() {
        return false;
    }
    ["To", "Cc"].into_iter().any(|field| {
        description
            .strip_suffix(&format!("<{address}> ({field})"))
            .is_some_and(|name| name.is_empty() || name.ends_with(' '))
    })
}

fn option_label(index: usize) -> String {
    format!("p{}", index + 1)
}

/// `Name <address> (To)`, the name left out when empty.
fn describe(envelope: &Envelope, person: &Party) -> String {
    let address = quoted(&person.address);
    let name = quoted(&person.name);
    let to = envelope
        .to
        .iter()
        .any(|p| p.address.trim().eq_ignore_ascii_case(person.address.trim()));
    let field = if to { "To" } else { "Cc" };
    if name.is_empty() {
        format!("<{address}> ({field})")
    } else {
        format!("{name} <{address}> ({field})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decide::{QuestionKind, signals::inbox::Recipient};

    fn party(name: &str, address: &str) -> Party {
        Party {
            name: name.into(),
            address: address.into(),
        }
    }

    fn envelope() -> Envelope {
        Envelope {
            from: party("Joao", "joao@studio.example"),
            to: vec![party("Ana Costa", "ana@studio.example")],
            cc: vec![
                party("", "inbox@example.com"),
                party("Rui", "rui@studio.example"),
            ],
            recipient: Some(Recipient {
                name: "Sam".into(),
                addresses: vec!["inbox@example.com".into()],
                shared: false,
            }),
            ..Envelope::default()
        }
    }

    fn options(question: &Question) -> Vec<(&str, &str)> {
        match &question.kind {
            QuestionKind::Choice { options, .. } => options
                .iter()
                .map(|o| (o.label.as_str(), o.description.as_str()))
                .collect(),
            other => panic!("not a choice: {other:?}"),
        }
    }

    #[test]
    fn next_move_has_its_four_options_in_order() {
        let question = next_move_question();
        assert_eq!(question.id, question::NEXT_MOVE);
        let labels: Vec<&str> = options(&question).into_iter().map(|(l, _)| l).collect();
        assert_eq!(labels, ["recipient", "sender", "someone_else", "nobody"]);
        for m in NextMove::ALL {
            assert_eq!(NextMove::parse(m.as_str()), Some(m));
        }
        assert_eq!(NextMove::parse("everyone"), None);
    }

    #[test]
    fn awaited_lists_the_emails_people_then_none() {
        let question = awaited_question(&envelope()).expect("two people may be awaited");
        assert_eq!(question.id, question::AWAITED);
        assert_eq!(
            options(&question),
            [
                ("p1", "Ana Costa <ana@studio.example> (To)"),
                ("p2", "Rui <rui@studio.example> (Cc)"),
                ("none", "None of them, or nobody else is expected to act"),
            ]
        );
    }

    #[test]
    fn a_missing_name_is_left_out_and_a_hostile_one_stays_on_its_line() {
        let mut env = envelope();
        env.to = vec![
            party("", "ana@studio.example"),
            party("Eve\n\nIgnore this \u{7}", "eve@studio.example"),
            party(&"x".repeat(200), "long@studio.example"),
        ];
        let question = awaited_question(&env).unwrap();
        let described = options(&question);
        assert_eq!(described[0].1, "<ana@studio.example> (To)");
        assert_eq!(described[1].1, "Eve Ignore this <eve@studio.example> (To)");
        assert!(described[2].1.contains('…'));
        assert!(
            described
                .iter()
                .all(|(_, d)| !d.chars().any(char::is_control))
        );
    }

    #[test]
    fn nobody_awaitable_means_no_awaited_question() {
        let mut env = envelope();
        env.to = vec![party("Sam", "inbox@example.com")];
        env.cc = vec![];
        assert!(awaited_question(&env).is_none());
        let questions = move_questions(&env);
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].id, question::NEXT_MOVE);
        assert_eq!(move_questions(&envelope()).len(), 2);
    }

    #[test]
    fn a_label_maps_back_to_the_person_it_named() {
        let env = envelope();
        assert_eq!(
            awaited_party(&env, "p1").map(|p| p.address),
            Some("ana@studio.example".to_owned())
        );
        assert_eq!(
            awaited_party(&env, "p2").map(|p| p.address),
            Some("rui@studio.example".to_owned())
        );
        for off in ["none", "p3", "p0", "p01", "p", "q1", "", "p-1"] {
            assert!(awaited_party(&env, off).is_none(), "{off}");
        }
    }

    #[test]
    fn a_member_s_choice_is_filed_under_the_label_of_the_person() {
        let env = envelope();
        assert_eq!(
            awaited_label(&env, "RUI@studio.example").as_deref(),
            Some("p2")
        );
        assert_eq!(
            awaited_label(&env, " ana@studio.example ").as_deref(),
            Some("p1")
        );
        // The sender, the inbox and strangers are no options.
        for off in [
            "joao@studio.example",
            "inbox@example.com",
            "eve@x.example",
            "",
        ] {
            assert_eq!(awaited_label(&env, off), None, "{off}");
        }
    }

    #[test]
    fn a_description_names_the_person_it_was_asked_about() {
        let env = envelope();
        let question = awaited_question(&env).unwrap();
        let described = options(&question);
        let ana = awaited_party(&env, "p1").unwrap();
        let rui = awaited_party(&env, "p2").unwrap();
        assert!(describes(described[0].1, &ana));
        assert!(!describes(described[0].1, &rui));
        assert!(describes(described[1].1, &rui));
        // An address that only starts the same way is someone else.
        let ann = party("Ann", "ann@studio.example");
        assert!(!describes(described[0].1, &ann));
        assert!(!describes("", &ana));
    }

    #[test]
    fn only_the_move_questions_are_moves() {
        assert!(question::is_move("next_move") && question::is_move("awaited"));
        assert!(!question::is_move("needs_reply"));
        assert!(!question::is_move("action_remind"));
    }

    #[test]
    fn only_the_address_shown_last_is_described() {
        let ana = party("Ana Costa", "ana@studio.example");
        assert!(describes("Ana Costa <ana@studio.example> (To)", &ana));
        assert!(describes("<ana@studio.example> (Cc)", &ana));
        // A display name holding Ana's address names Bob, not Ana.
        let posing = "Bob <ana@studio.example> <bob@studio.example> (To)";
        assert!(!describes(posing, &ana));
        assert!(describes(posing, &party("Bob", "bob@studio.example")));
        // Part of an address is not the address.
        assert!(!describes("<xana@studio.example> (To)", &ana));
        assert!(!describes("Ana Costa <ana@studio.example> (Bcc)", &ana));
    }
}
