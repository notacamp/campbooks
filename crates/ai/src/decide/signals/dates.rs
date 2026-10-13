//! Resolving a deadline's words to a date.
//!
//! The email signals answer `has_deadline` with the words they matched
//! ("Thursday", "30/10", "end of the month") and the sentence around them.
//! [`resolve_date`] turns those into a calendar date relative to the day the
//! message was sent, in English, European Portuguese, Spanish and French.
//! It is deterministic and never guesses: words it cannot place give `None`,
//! and the caller keeps the words.

use std::sync::LazyLock;

use chrono::{Datelike as _, Days, NaiveDate, Weekday};
use regex::Regex;

/// Month names (and their number) in the four languages.
const MONTHS: [(&str, u32); 49] = [
    ("january", 1),
    ("february", 2),
    ("march", 3),
    ("april", 4),
    ("may", 5),
    ("june", 6),
    ("july", 7),
    ("august", 8),
    ("september", 9),
    ("october", 10),
    ("november", 11),
    ("december", 12),
    ("janeiro", 1),
    ("fevereiro", 2),
    ("março", 3),
    ("abril", 4),
    ("maio", 5),
    ("junho", 6),
    ("julho", 7),
    ("agosto", 8),
    ("setembro", 9),
    ("outubro", 10),
    ("novembro", 11),
    ("dezembro", 12),
    ("enero", 1),
    ("febrero", 2),
    ("marzo", 3),
    ("mayo", 5),
    ("junio", 6),
    ("julio", 7),
    ("septiembre", 9),
    ("setiembre", 9),
    ("octubre", 10),
    ("noviembre", 11),
    ("diciembre", 12),
    ("janvier", 1),
    ("février", 2),
    ("mars", 3),
    ("avril", 4),
    ("mai", 5),
    ("juin", 6),
    ("juillet", 7),
    ("août", 8),
    ("septembre", 9),
    ("octobre", 10),
    ("novembre", 11),
    ("décembre", 12),
    ("aout", 8),
    ("fevrier", 2),
];

/// Weekday names in the four languages (Portuguese also as `<day>-feira`).
const WEEKDAYS: [(&str, Weekday); 28] = [
    ("monday", Weekday::Mon),
    ("tuesday", Weekday::Tue),
    ("wednesday", Weekday::Wed),
    ("thursday", Weekday::Thu),
    ("friday", Weekday::Fri),
    ("saturday", Weekday::Sat),
    ("sunday", Weekday::Sun),
    ("segunda", Weekday::Mon),
    ("terça", Weekday::Tue),
    ("quarta", Weekday::Wed),
    ("quinta", Weekday::Thu),
    ("sexta", Weekday::Fri),
    ("sábado", Weekday::Sat),
    ("domingo", Weekday::Sun),
    ("lunes", Weekday::Mon),
    ("martes", Weekday::Tue),
    ("miércoles", Weekday::Wed),
    ("jueves", Weekday::Thu),
    ("viernes", Weekday::Fri),
    ("sábado", Weekday::Sat),
    ("domingo", Weekday::Sun),
    ("lundi", Weekday::Mon),
    ("mardi", Weekday::Tue),
    ("mercredi", Weekday::Wed),
    ("jeudi", Weekday::Thu),
    ("vendredi", Weekday::Fri),
    ("samedi", Weekday::Sat),
    ("dimanche", Weekday::Sun),
];

const TODAY: [&str; 6] = ["today", "tonight", "hoje", "hoy", "aujourd'hui", "ce soir"];
const TOMORROW: [&str; 4] = ["tomorrow", "amanhã", "mañana", "demain"];
const END_OF_WEEK: [&str; 13] = [
    "end of the week",
    "end of week",
    "end of this week",
    "this week",
    "fim da semana",
    "fim desta semana",
    "esta semana",
    "final de esta semana",
    "fin de semana",
    "fin de la semaine",
    "fin de semaine",
    "fin de cette semaine",
    "cette semaine",
];
const NEXT_WEEK: [&str; 9] = [
    "next week",
    "end of next week",
    "próxima semana",
    "fim da próxima semana",
    "la semana que viene",
    "la próxima semana",
    "final de la próxima semana",
    "semaine prochaine",
    "fin de la semaine prochaine",
];
const END_OF_MONTH: [&str; 14] = [
    "end of the month",
    "end of month",
    "end of this month",
    "this month",
    "fim do mês",
    "fim deste mês",
    "final do mês",
    "este mês",
    "fin de mes",
    "final de mes",
    "finales de mes",
    "a finales de mes",
    "este mes",
    "fin du mois",
];

fn regex(pattern: &str) -> Regex {
    match Regex::new(pattern) {
        Ok(re) => re,
        Err(err) => panic!("built-in date pattern does not compile: {err}"),
    }
}

fn month_alternation() -> String {
    MONTHS
        .iter()
        .map(|(name, _)| regex::escape(name))
        .collect::<Vec<_>>()
        .join("|")
}

/// "16 October", "30 de outubro", "le 3 mars 2027".
static DAY_MONTH: LazyLock<Regex> = LazyLock::new(|| {
    regex(&format!(
        r"(?i)\b(\d{{1,2}})(?:st|nd|rd|th|º|er)?\s+(?:de\s+|of\s+)?({})\b(?:,?\s+(?:de\s+)?(\d{{4}}))?",
        month_alternation()
    ))
});

/// "October 16", "October 16th, 2026".
static MONTH_DAY: LazyLock<Regex> = LazyLock::new(|| {
    regex(&format!(
        r"(?i)\b({})\s+(\d{{1,2}})(?:st|nd|rd|th)?\b(?:,?\s+(\d{{4}}))?",
        month_alternation()
    ))
});

/// `16/10`, `16.10.2026`, `16-10-26` (day first, the four locales' order).
static NUMERIC: LazyLock<Regex> =
    LazyLock::new(|| regex(r"^(\d{1,2})[/.-](\d{1,2})(?:[/.-](\d{2}|\d{4}))?$"));

/// `2026-10-16`.
static ISO: LazyLock<Regex> = LazyLock::new(|| regex(r"^(\d{4})-(\d{2})-(\d{2})$"));

/// "within 3 days", "in 48 hours", "dentro de 5 dias", "sous 2 jours".
static WITHIN: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?i)(\d+)\s*(h|hours?|horas|heures|days?|dias|días|jours)\b"));

/// The date the deadline words `evidence` name, for a message sent on
/// `sent_on`, reading the sentence they came from (`context`) for a fuller
/// date ("Friday 16 October", where the evidence is only "Friday").
pub fn resolve_date(
    evidence: &str,
    context: Option<&str>,
    sent_on: NaiveDate,
) -> Option<NaiveDate> {
    let words = evidence.trim().to_lowercase().replace('\u{2019}', "'");
    if words.is_empty() {
        return None;
    }
    if let Some(date) = context.and_then(|sentence| named_date(sentence, sent_on)) {
        return Some(date);
    }
    if let Some(date) = named_date(&words, sent_on) {
        return Some(date);
    }
    if let Some(date) = numeric_date(&words, sent_on) {
        return Some(date);
    }
    if TODAY.contains(&words.as_str()) {
        return Some(sent_on);
    }
    if TOMORROW.contains(&words.as_str()) {
        return sent_on.checked_add_days(Days::new(1));
    }
    if NEXT_WEEK.contains(&words.as_str()) {
        return next_or_same(sent_on, Weekday::Fri)?.checked_add_days(Days::new(7));
    }
    if END_OF_WEEK.contains(&words.as_str()) {
        return next_or_same(sent_on, Weekday::Fri);
    }
    if END_OF_MONTH.contains(&words.as_str()) {
        return last_of_month(sent_on);
    }
    if let Some(day) = weekday(&words) {
        return next_after(sent_on, day);
    }
    within(&words, sent_on)
}

/// A day and a month name, in either order, in `text`.
fn named_date(text: &str, sent_on: NaiveDate) -> Option<NaiveDate> {
    let (day, month, year) = if let Some(c) = DAY_MONTH.captures(text) {
        (c.get(1)?.as_str(), c.get(2)?.as_str(), c.get(3))
    } else if let Some(c) = MONTH_DAY.captures(text) {
        (c.get(2)?.as_str(), c.get(1)?.as_str(), c.get(3))
    } else {
        return None;
    };
    let day: u32 = day.parse().ok()?;
    let month = month_number(month)?;
    match year {
        Some(year) => NaiveDate::from_ymd_opt(year.as_str().parse().ok()?, month, day),
        None => in_coming_year(day, month, sent_on),
    }
}

fn numeric_date(words: &str, sent_on: NaiveDate) -> Option<NaiveDate> {
    if let Some(c) = ISO.captures(words) {
        return NaiveDate::from_ymd_opt(c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?);
    }
    let c = NUMERIC.captures(words)?;
    let day: u32 = c[1].parse().ok()?;
    let month: u32 = c[2].parse().ok()?;
    match c.get(3) {
        Some(year) => {
            let mut year: i32 = year.as_str().parse().ok()?;
            if year < 100 {
                year += 2000;
            }
            NaiveDate::from_ymd_opt(year, month, day)
        }
        None => in_coming_year(day, month, sent_on),
    }
}

/// `day`/`month` of the sent year, or of the next one when that has passed.
fn in_coming_year(day: u32, month: u32, sent_on: NaiveDate) -> Option<NaiveDate> {
    let this_year = NaiveDate::from_ymd_opt(sent_on.year(), month, day)?;
    if this_year >= sent_on {
        Some(this_year)
    } else {
        NaiveDate::from_ymd_opt(sent_on.year() + 1, month, day)
    }
}

fn month_number(name: &str) -> Option<u32> {
    let name = name.to_lowercase();
    MONTHS
        .iter()
        .find(|(month, _)| *month == name)
        .map(|(_, n)| *n)
}

fn weekday(words: &str) -> Option<Weekday> {
    let word = words.trim_end_matches("-feira");
    WEEKDAYS
        .iter()
        .find(|(name, _)| *name == word)
        .map(|(_, day)| *day)
}

/// The first `day` strictly after `from`.
fn next_after(from: NaiveDate, day: Weekday) -> Option<NaiveDate> {
    let ahead = (7 + day.num_days_from_monday() - from.weekday().num_days_from_monday()) % 7;
    let ahead = if ahead == 0 { 7 } else { ahead };
    from.checked_add_days(Days::new(u64::from(ahead)))
}

/// `from` itself when it is `day`, else the next one.
fn next_or_same(from: NaiveDate, day: Weekday) -> Option<NaiveDate> {
    let ahead = (7 + day.num_days_from_monday() - from.weekday().num_days_from_monday()) % 7;
    from.checked_add_days(Days::new(u64::from(ahead)))
}

fn last_of_month(day: NaiveDate) -> Option<NaiveDate> {
    let (year, month) = if day.month() == 12 {
        (day.year() + 1, 1)
    } else {
        (day.year(), day.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1)?.pred_opt()
}

fn within(words: &str, sent_on: NaiveDate) -> Option<NaiveDate> {
    let c = WITHIN.captures(words)?;
    let n: u64 = c[1].parse().ok()?;
    let unit = c[2].to_lowercase();
    let days = if unit.starts_with('h') {
        n.div_ceil(24)
    } else {
        n
    };
    if days > 366 {
        return None;
    }
    sent_on.checked_add_days(Days::new(days))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    // Monday 12 October 2026.
    const MONDAY: (i32, u32, u32) = (2026, 10, 12);

    fn sent() -> NaiveDate {
        day(MONDAY.0, MONDAY.1, MONDAY.2)
    }

    #[test]
    fn every_pattern_compiles() {
        for re in [&DAY_MONTH, &MONTH_DAY, &NUMERIC, &ISO, &WITHIN] {
            assert!(!re.as_str().is_empty());
        }
    }

    #[test]
    fn weekdays_are_the_next_one_after_sending() {
        assert_eq!(
            resolve_date("Thursday", None, sent()),
            Some(day(2026, 10, 15))
        );
        assert_eq!(
            resolve_date("Monday", None, sent()),
            Some(day(2026, 10, 19))
        );
        assert_eq!(
            resolve_date("quinta-feira", None, sent()),
            Some(day(2026, 10, 15))
        );
        assert_eq!(
            resolve_date("viernes", None, sent()),
            Some(day(2026, 10, 16))
        );
        assert_eq!(
            resolve_date("mercredi", None, sent()),
            Some(day(2026, 10, 14))
        );
    }

    #[test]
    fn relative_words() {
        assert_eq!(resolve_date("today", None, sent()), Some(sent()));
        assert_eq!(
            resolve_date("amanhã", None, sent()),
            Some(day(2026, 10, 13))
        );
        assert_eq!(
            resolve_date("end of the week", None, sent()),
            Some(day(2026, 10, 16))
        );
        assert_eq!(
            resolve_date("fin du mois", None, sent()),
            Some(day(2026, 10, 31))
        );
        assert_eq!(
            resolve_date("next week", None, sent()),
            Some(day(2026, 10, 23))
        );
        assert_eq!(
            resolve_date("within 3 days", None, sent()),
            Some(day(2026, 10, 15))
        );
        assert_eq!(
            resolve_date("in 48 hours", None, sent()),
            Some(day(2026, 10, 14))
        );
    }

    #[test]
    fn numbers_are_day_first() {
        assert_eq!(resolve_date("30/10", None, sent()), Some(day(2026, 10, 30)));
        assert_eq!(
            resolve_date("03/10/2027", None, sent()),
            Some(day(2027, 10, 3))
        );
        assert_eq!(
            resolve_date("2026-11-02", None, sent()),
            Some(day(2026, 11, 2))
        );
        // Already past this year: next year's.
        assert_eq!(resolve_date("05/01", None, sent()), Some(day(2027, 1, 5)));
        assert_eq!(resolve_date("31/02", None, sent()), None);
    }

    #[test]
    fn the_sentence_gives_the_full_date() {
        let sentence = "Please sign the attached lease and send it back by Friday 16 October.";
        assert_eq!(
            resolve_date("Friday", Some(sentence), sent()),
            Some(day(2026, 10, 16))
        );
        let pt = "O pagamento deve ser feito até 30 de outubro.";
        assert_eq!(
            resolve_date("outubro", Some(pt), sent()),
            Some(day(2026, 10, 30))
        );
        let en = "Payment due by October 30th, 2026.";
        assert_eq!(
            resolve_date("October", Some(en), sent()),
            Some(day(2026, 10, 30))
        );
    }

    #[test]
    fn words_it_cannot_place_stay_words() {
        assert_eq!(resolve_date("before the holidays", None, sent()), None);
        assert_eq!(resolve_date("October", None, sent()), None);
        assert_eq!(resolve_date("", None, sent()), None);
    }
}
