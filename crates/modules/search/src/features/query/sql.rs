//! The SQL of a query, built from a parsed [`Query`] with every piece of the
//! member's text bound as a parameter. Only the statement's shape (which
//! filters, how many terms) depends on the query.

use chrono::{DateTime, Utc};
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

use crate::domain::{
    language::Language,
    query::{Clause, Filter, Flag, Mailbox, PersonField, Query, Term},
    text::{HIT_END, HIT_START},
};

/// Field weights for `ts_rank_cd`: D, C (body), B (people, files), A (title).
const WEIGHTS: &str = "{0.1, 0.3, 0.6, 1.0}";
/// Matching entries considered per query, newest groups first: bounds the
/// cost of very common words.
pub const CANDIDATES: i64 = 5_000;
/// Seconds of age that halve a match's score.
const HALF_LIFE_SECONDS: f64 = 180.0 * 86_400.0;

/// How results are ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// Text relevance with recency; pages by offset.
    Relevance {
        /// Groups to skip.
        offset: i64,
    },
    /// Newest first; pages by the last group shown.
    Newest {
        /// The last group of the previous page.
        after: Option<(DateTime<Utc>, Uuid)>,
    },
}

/// Where the query may look: these groups types, in these containers.
#[derive(Debug, Clone)]
pub struct Scope {
    /// The workspace.
    pub workspace: Uuid,
    /// Group record types the caller may read.
    pub group_types: Vec<String>,
    /// `<container type>:<container id>` the caller may see.
    pub containers: Vec<String>,
    /// The caller's time zone for dates (a name Postgres knows).
    pub tz: String,
}

/// `ts_headline` options: hits delimited by private-use characters.
pub fn headline_options(all: bool) -> String {
    if all {
        format!("StartSel={HIT_START}, StopSel={HIT_END}, HighlightAll=true")
    } else {
        format!(
            "StartSel={HIT_START}, StopSel={HIT_END}, MaxWords=24, MinWords=12, ShortWord=2, \
             MaxFragments=2, FragmentDelimiter=\" … \""
        )
    }
}

/// Push the `tsquery` of `clauses` (non-empty).
pub fn push_tsquery(builder: &mut QueryBuilder<Postgres>, clauses: &[Clause]) {
    builder.push("(");
    for (i, clause) in clauses.iter().enumerate() {
        if i > 0 {
            builder.push(" && ");
        }
        if clause.negated {
            builder.push("!!");
        }
        builder.push("(");
        for (j, term) in clause.any.iter().enumerate() {
            if j > 0 {
                builder.push(" || ");
            }
            push_term(builder, term);
        }
        builder.push(")");
    }
    builder.push(")");
}

/// One term, looked up in every configuration.
fn push_term(builder: &mut QueryBuilder<Postgres>, term: &Term) {
    builder.push("(");
    for (i, language) in Language::ALL.iter().enumerate() {
        if i > 0 {
            builder.push(" || ");
        }
        let config = format!("'search.{}'::regconfig", language.as_str());
        match term {
            Term::Word(word) => {
                builder.push(format!("plainto_tsquery({config}, "));
                builder.push_bind(word.clone());
            }
            Term::Phrase(phrase) => {
                builder.push(format!("phraseto_tsquery({config}, "));
                builder.push_bind(phrase.clone());
            }
            Term::Prefix(start) => {
                // Letters and digits only (the parser's guarantee), so the
                // bound text is a single lexeme in `tsquery` syntax.
                builder.push(format!("to_tsquery({config}, "));
                builder.push_bind(start.clone());
                builder.push(" || ':*'");
            }
        }
        builder.push(")");
    }
    builder.push(")");
}

/// `%value%` for `LIKE`, with `value`'s own wildcards escaped.
fn like(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

/// Push the conditions of `query`'s filters (each starts with ` AND `).
fn push_filters(builder: &mut QueryBuilder<Postgres>, query: &Query, tz: &str) {
    let mut mailbox_chosen = false;
    for filter in &query.filters {
        match filter {
            Filter::Person(field, value) => {
                let column = match field {
                    PersonField::From => "e.from_folded",
                    PersonField::To => "e.to_folded",
                    PersonField::Cc => "e.cc_folded",
                };
                builder.push(format!(" AND {column} LIKE lower(unaccent("));
                builder.push_bind(like(value));
                builder.push(")) ESCAPE '\\'");
            }
            Filter::Subject(value) => {
                builder.push(" AND g.title_folded LIKE lower(unaccent(");
                builder.push_bind(like(value));
                builder.push(")) ESCAPE '\\'");
            }
            Filter::HasAttachment => {
                builder.push(" AND e.facets @? '$.attachments ? (@ > 0)'");
            }
            Filter::Mailbox(mailbox) => {
                mailbox_chosen = true;
                let name = match mailbox {
                    Mailbox::Inbox => "inbox",
                    Mailbox::Archive => "archive",
                    Mailbox::Spam => "spam",
                    Mailbox::Trash => "trash",
                    Mailbox::Anywhere => continue,
                };
                builder.push(" AND e.facets->>'mailbox' = ");
                builder.push_bind(name);
            }
            Filter::Stream(stream) => {
                builder.push(" AND g.facets->>'stream' = ");
                builder.push_bind(stream.clone());
            }
            Filter::Is(flag) => {
                builder.push(match flag {
                    Flag::Unread => " AND g.facets @> '{\"unread\": true}'",
                    Flag::Read => " AND g.facets @> '{\"unread\": false}'",
                    Flag::Starred => " AND g.facets @> '{\"starred\": true}'",
                });
            }
            Filter::Label(name) => {
                builder.push(" AND lower(unaccent(");
                builder.push_bind(name.clone());
                builder.push(")) = ANY(g.keywords)");
            }
            Filter::After(day) => {
                builder.push(" AND e.occurred_at >= (");
                builder.push_bind(*day);
                builder.push("::timestamp AT TIME ZONE ");
                builder.push_bind(tz.to_owned());
                builder.push(")");
            }
            Filter::Before(day) => {
                builder.push(" AND e.occurred_at < (");
                builder.push_bind(*day);
                builder.push("::timestamp AT TIME ZONE ");
                builder.push_bind(tz.to_owned());
                builder.push(")");
            }
            Filter::NewerThanDays(days) => {
                builder.push(" AND e.occurred_at > now() - make_interval(days => ");
                builder.push_bind(i32::try_from(*days).unwrap_or(i32::MAX));
                builder.push(")");
            }
            Filter::OlderThanDays(days) => {
                builder.push(" AND e.occurred_at < now() - make_interval(days => ");
                builder.push_bind(i32::try_from(*days).unwrap_or(i32::MAX));
                builder.push(")");
            }
        }
    }
    if !mailbox_chosen {
        builder.push(" AND coalesce(e.facets->>'mailbox', '') NOT IN ('spam', 'trash')");
    }
}

/// The statement returning one page (plus one row) of groups:
/// `id, record_type, record_id, container_type, container_id, occurred_at,
/// facets, score, best`.
pub fn page(query: &Query, scope: &Scope, order: Order, fetch: i64) -> QueryBuilder<Postgres> {
    let text = !query.clauses.is_empty();
    let mut b = QueryBuilder::new("WITH matched AS (SELECT e.id, e.group_id, g.occurred_at, ");
    if text {
        b.push(format!("ts_rank_cd('{WEIGHTS}'::real[], e.tsv, "));
        push_tsquery(&mut b, &query.clauses);
        b.push(", 1)");
    } else {
        b.push("0::real");
    }
    b.push(
        " AS rank FROM search.entries e JOIN search.groups g ON g.id = e.group_id \
         WHERE e.workspace_id = ",
    );
    b.push_bind(scope.workspace);
    b.push(" AND g.workspace_id = ");
    b.push_bind(scope.workspace);
    b.push(" AND g.record_type = ANY(");
    b.push_bind(scope.group_types.clone());
    b.push(") AND (g.container_type || ':' || g.container_id) = ANY(");
    b.push_bind(scope.containers.clone());
    b.push(")");
    if text {
        b.push(" AND e.tsv @@ ");
        push_tsquery(&mut b, &query.clauses);
    }
    push_filters(&mut b, query, &scope.tz);
    if let Order::Newest {
        after: Some((at, id)),
    } = order
    {
        b.push(" AND (g.occurred_at, g.id) < (");
        b.push_bind(at);
        b.push(", ");
        b.push_bind(id);
        b.push(")");
    }
    b.push(" ORDER BY g.occurred_at DESC, g.id DESC LIMIT ");
    b.push_bind(CANDIDATES);
    b.push(
        "), best AS (SELECT DISTINCT ON (group_id) group_id, id, rank FROM matched \
         ORDER BY group_id, rank DESC, occurred_at DESC) \
         SELECT g.id, g.record_type, g.record_id, g.container_type, g.container_id, \
                g.occurred_at, g.facets, best.id AS best, \
                (best.rank / (1.0 + greatest(extract(epoch FROM now() - g.occurred_at), 0) / ",
    );
    b.push(format!("{HALF_LIFE_SECONDS:.1}"));
    b.push(
        "))::float8 AS score \
         FROM best JOIN search.groups g ON g.id = best.group_id ORDER BY ",
    );
    match order {
        Order::Relevance { offset } => {
            b.push("score DESC, g.occurred_at DESC, g.id DESC LIMIT ");
            b.push_bind(fetch);
            b.push(" OFFSET ");
            b.push_bind(offset);
        }
        Order::Newest { .. } => {
            b.push("g.occurred_at DESC, g.id DESC LIMIT ");
            b.push_bind(fetch);
        }
    }
    b
}

/// The statement returning, for the page's best entries, `id, people,
/// title, snippet`: highlighted when the query has text, else plain (the
/// start of the body).
pub fn highlights(query: &Query, entries: Vec<Uuid>) -> QueryBuilder<Postgres> {
    let mut b = QueryBuilder::new("SELECT e.id, e.record_type, e.record_id, e.people, ");
    if query.clauses.is_empty() {
        b.push("g.title AS title, left(regexp_replace(e.body, '\\s+', ' ', 'g'), 240) AS snippet");
    } else {
        b.push("ts_headline(('search.' || e.language)::regconfig, g.title, ");
        push_tsquery(&mut b, &query.clauses);
        b.push(", ");
        b.push_bind(headline_options(true));
        b.push(") AS title, ts_headline(('search.' || e.language)::regconfig, regexp_replace(e.body, '\\s+', ' ', 'g'), ");
        push_tsquery(&mut b, &query.clauses);
        b.push(", ");
        b.push_bind(headline_options(false));
        b.push(") AS snippet");
    }
    b.push(" FROM search.entries e JOIN search.groups g ON g.id = e.group_id WHERE e.id = ANY(");
    b.push_bind(entries);
    b.push(")");
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::query::parse;

    #[test]
    fn user_text_is_bound_never_spliced() {
        let query = parse(
            r#"o'brien "x'); DROP TABLE y; --" from:a%b_c\ label:L' is:unread"#,
            true,
        )
        .unwrap();
        let scope = Scope {
            workspace: Uuid::nil(),
            group_types: vec!["conversations.conversation".into()],
            containers: vec![],
            tz: "UTC".into(),
        };
        let built = page(&query, &scope, Order::Relevance { offset: 0 }, 21).into_sql();
        let sql = built.as_str();
        for fragment in ["o'brien", "DROP TABLE", "a%b", "L'"] {
            assert!(!sql.contains(fragment), "{fragment} in {sql}");
        }
        assert_eq!(like(r"a%b_c\"), r"%a\%b\_c\\%");
    }
}
