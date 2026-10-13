//! The questions of the decide pass, asked about every inbound message.
//!
//! The ids match the eval set of campbooks-cloud#59
//! (`crates/ai/evals/decide/mail-triage`), so answers recorded here and
//! corrections line up with what is measured there.

use campbooks_ai::decide::Question;

use crate::model::Stream;

/// Which stream the conversation belongs in (`choice`).
pub const STREAM: &str = "stream";
/// Whether the sender expects a reply (`noul`).
pub const NEEDS_REPLY: &str = "needs_reply";
/// Whether the sender asks the recipient to do something (`noul`).
pub const ASKS_ACTION: &str = "asks_action";
/// Whether something has to be done by a date (`noul`).
pub const HAS_DEADLINE: &str = "has_deadline";
/// How soon the recipient needs to act (`score`, 0 to 2).
pub const URGENCY: &str = "urgency";
/// Whether it looks like phishing or a scam (`noul`).
pub const SUSPICIOUS: &str = "suspicious";

/// The six questions, in the order they are asked.
pub fn all() -> Vec<Question> {
    vec![
        Question::choice(
            STREAM,
            "Where does this email belong?",
            Stream::ALL.map(|stream| (stream.as_str(), describe(stream))),
        ),
        Question::noul(
            NEEDS_REPLY,
            "The sender expects a reply from the recipient.",
        ),
        Question::noul(
            ASKS_ACTION,
            "The sender asks the recipient to do something.",
        ),
        Question::noul(
            HAS_DEADLINE,
            "The email gives a date or time by which something must be done.",
        ),
        Question::score(
            URGENCY,
            "How soon does the recipient need to act on this email?",
            ["not soon, or no action", "within a few days", "today"],
        ),
        Question::noul(SUSPICIOUS, "The email looks like phishing or a scam."),
    ]
}

/// What a stream means, as the model reads it.
const fn describe(stream: Stream) -> &'static str {
    match stream {
        Stream::Primary => "A personal or work message written by a person",
        Stream::Updates => "An automatic notification, alert, confirmation or status update",
        Stream::Receipts => "A receipt, invoice, bill or payment confirmation",
        Stream::Newsletters => "A newsletter, mailing list or marketing email",
    }
}

#[cfg(test)]
mod tests {
    use campbooks_ai::decide::QuestionType;

    use super::*;

    #[test]
    fn six_questions_with_the_eval_ids() {
        let questions = all();
        let ids: Vec<&str> = questions.iter().map(|q| q.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                STREAM,
                NEEDS_REPLY,
                ASKS_ACTION,
                HAS_DEADLINE,
                URGENCY,
                SUSPICIOUS
            ]
        );
        assert_eq!(questions[0].question_type(), QuestionType::Choice);
        assert_eq!(questions[0].option_count(), 4);
        assert_eq!(questions[4].question_type(), QuestionType::Score);
    }
}
