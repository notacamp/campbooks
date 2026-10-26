//! Property tests per type: random inputs from a seeded generator (no
//! external crate), checked against the rules every type shares.
//!
//! - **Canonical**: a value that passes [`validate`] passes again, unchanged.
//! - **Closed**: nothing but what the type allows passes; the wrong JSON
//!   shape is always refused.
//! - **Ordered**: [`eval::compare`] is a total order, nulls last.
//! - **Convertible**: a converted value always passes the new type.
//!
//! The SQL is held to [`eval`] in `domain::table_query`'s tests, against a
//! real database.

use std::{cmp::Ordering, collections::HashMap};

use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    PropertyError, PropertyType as T, Spec,
    config::{self, SelectOption, StatusGroup, default_config},
    convert, eval,
    filter::{self, Direction, Operator},
    value::{self, parse_decimal, parse_when, validate},
};

/// A small deterministic generator (xorshift64*), so a failure replays.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap()
    }

    pub fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    pub fn pick<'a, X>(&mut self, items: &'a [X]) -> &'a X {
        &items[self.below(items.len())]
    }
}

const WORDS: &[&str] = &[
    "alpha", "Beta", "gamma", "delta alpha", "x%y", "a_b", "north", "Zulu", "100%", "o'clock",
];

fn option(id: &str, name: &str, group: Option<StatusGroup>) -> SelectOption {
    SelectOption {
        id: id.into(),
        name: name.into(),
        color: Some("blue".into()),
        group,
    }
}

/// One property of every filterable and storable type, with its config.
pub fn specs() -> Vec<(&'static str, Spec)> {
    let make = |name, ty, config: Value| {
        (name, Spec::new(Uuid::now_v7(), ty, config::normalise(ty, &config).unwrap()))
    };
    vec![
        make("title", T::Title, json!({})),
        make("text", T::Text, json!({})),
        make("number", T::Number, json!({"format": "currency", "currency": "EUR"})),
        (
            "select",
            Spec::new(
                Uuid::now_v7(),
                T::Select,
                config::with_options(
                    T::Select,
                    &[option("s1", "One", None), option("s2", "Two", None), option("s3", "Three", None)],
                )
                .unwrap(),
            ),
        ),
        (
            "multi",
            Spec::new(
                Uuid::now_v7(),
                T::MultiSelect,
                config::with_options(
                    T::MultiSelect,
                    &[option("m1", "Red", None), option("m2", "Green", None), option("m3", "Blue", None)],
                )
                .unwrap(),
            ),
        ),
        (
            "status",
            Spec::new(
                Uuid::now_v7(),
                T::Status,
                config::with_options(
                    T::Status,
                    &[
                        option("t1", "Not started", Some(StatusGroup::Todo)),
                        option("t2", "Doing", Some(StatusGroup::Doing)),
                        option("t3", "Done", Some(StatusGroup::Done)),
                    ],
                )
                .unwrap(),
            ),
        ),
        make("date", T::Date, json!({"time": false})),
        make("datetime", T::Date, json!({"time": true})),
        make("checkbox", T::Checkbox, json!({})),
        make("person", T::Person, json!({})),
        make("files", T::Files, json!({})),
        make("url", T::Url, json!({})),
        make("email", T::Email, json!({})),
        make("phone", T::Phone, json!({})),
        make("relation", T::Relation, json!({"table": Uuid::now_v7().to_string()})),
        make("link", T::CampbooksLink, json!({"types": ["tasks.task", "schedule.event"]})),
        make("created", T::Created, json!({"of": "time"})),
        make("created_by", T::Created, json!({"of": "who"})),
        make("edited", T::Edited, json!({"of": "time"})),
        make("unique", T::UniqueId, json!({"prefix": "CL-"})),
    ]
}

fn members() -> Vec<Uuid> {
    (1..=4).map(|n| Uuid::from_u128(n)).collect()
}

fn day(rng: &mut Rng) -> String {
    format!("2026-{:02}-{:02}", 1 + rng.below(12), 1 + rng.below(28))
}

fn instant(rng: &mut Rng) -> String {
    format!(
        "{}T{:02}:{:02}:00+00:00",
        day(rng),
        rng.below(24),
        rng.below(60)
    )
}

fn decimal(rng: &mut Rng) -> String {
    let sign = if rng.chance(30) { "-" } else { "" };
    let fraction = if rng.chance(50) {
        format!(".{}", rng.below(1000))
    } else {
        String::new()
    };
    format!("{sign}{}{fraction}", rng.below(200))
}

/// A valid, canonical value for `spec`, or `Null`.
pub fn generate(rng: &mut Rng, spec: &Spec) -> Value {
    if rng.chance(15) {
        return Value::Null;
    }
    let raw = match spec.ty {
        T::Text => json!([{"text": *rng.pick(WORDS), "bold": rng.chance(30)}, {"text": *rng.pick(WORDS)}]),
        T::Number => json!(decimal(rng)),
        T::Select | T::Status => {
            let options = config::options_of(&spec.config);
            json!(rng.pick(&options).id)
        }
        T::MultiSelect => {
            let options = config::options_of(&spec.config);
            let ids: Vec<&String> = options.iter().filter(|_| rng.chance(50)).map(|o| &o.id).collect();
            json!(ids)
        }
        T::Date => {
            let time = config::date_has_time(&spec.config);
            let start = if time { instant(rng) } else { day(rng) };
            json!({"start": start})
        }
        T::Checkbox => json!(rng.chance(50)),
        T::Person => {
            let people = members();
            let ids: Vec<String> = people.iter().filter(|_| rng.chance(40)).map(Uuid::to_string).collect();
            json!(ids)
        }
        T::Relation => {
            let ids: Vec<String> = (0..rng.below(3)).map(|_| Uuid::from_u128(100 + rng.below(5) as u128).to_string()).collect();
            json!(ids)
        }
        T::Files => json!([{"hash": "ab".repeat(32), "name": "plan.pdf", "size": rng.below(5000), "type": "application/pdf"}]),
        T::Url => json!(format!("https://example.com/{}", rng.below(50))),
        T::Email => json!(format!("user{}@example.com", rng.below(50))),
        T::Phone => json!(format!("+351 {}", 900_000_000 + rng.below(99_999))),
        T::CampbooksLink => {
            let record = *rng.pick(&["tasks.task", "schedule.event"]);
            json!([{"type": record, "id": format!("id-{}", rng.below(5))}])
        }
        _ => return Value::Null,
    };
    validate(spec.ty, &spec.config, &raw).expect("a generated value is valid")
}

#[test]
fn every_generated_value_is_canonical_and_small() {
    let mut rng = Rng(0x00C0_FFEE);
    for (name, spec) in specs().iter().filter(|(_, spec)| spec.ty.editable()) {
        for _ in 0..300 {
            let value = generate(&mut rng, spec);
            let again = validate(spec.ty, &spec.config, &value).unwrap();
            assert_eq!(again, value, "{name}: validate is not idempotent");
            assert!(serde_json::to_vec(&value).unwrap().len() <= super::MAX_VALUE_BYTES);
        }
    }
}

#[test]
fn the_wrong_shape_is_refused_for_every_type() {
    let junk = [
        json!(true),
        json!(12),
        json!({"nope": 1}),
        json!([{"nope": 1}]),
        json!([[1]]),
    ];
    for (name, spec) in specs().iter().filter(|(_, spec)| spec.ty.editable()) {
        for raw in &junk {
            if let Ok(value) = validate(spec.ty, &spec.config, raw) {
                // Only what the type reads leniently may pass (a number
                // from a JSON number, a checkbox from a boolean), and then
                // it is canonical.
                assert_eq!(
                    validate(spec.ty, &spec.config, &value).unwrap(),
                    value,
                    "{name} accepted {raw}"
                );
                assert!(
                    matches!(spec.ty, T::Number | T::Checkbox) || value.is_null(),
                    "{name} accepted {raw}"
                );
            }
        }
    }
}

#[test]
fn computed_and_reserved_types_are_never_written() {
    for ty in T::ALL.iter().copied().filter(|ty| !ty.editable()) {
        let config = default_config(ty);
        for raw in [json!("x"), json!(1), Value::Null, json!([])] {
            assert_eq!(
                validate(ty, &config, &raw),
                Err(PropertyError::NOT_EDITABLE),
                "{ty:?}"
            );
        }
    }
}

#[test]
fn reserved_types_cannot_be_created_by_a_member() {
    for ty in [T::NotionFormula, T::NotionRollup, T::NotionPeople, T::External, T::Title] {
        assert!(!ty.creatable(), "{ty:?}");
    }
    assert!(T::ALL.iter().filter(|ty| ty.creatable()).count() >= 17);
}

#[test]
fn numbers_are_canonical_decimals_with_a_total_order() {
    let mut rng = Rng(7);
    for _ in 0..500 {
        let a = decimal(&mut rng);
        let canonical = parse_decimal(&a).unwrap();
        assert_eq!(parse_decimal(&canonical).as_deref(), Some(canonical.as_str()));
        let (x, y) = (eval::scaled(&a).unwrap(), eval::scaled(&decimal(&mut rng)).unwrap());
        assert_eq!(x.cmp(&y), y.cmp(&x).reverse());
    }
    for bad in ["", "-", "1e5", "1,5", "+1", ".5", "5.", "lots", "1 2", "--1", "1.2.3"] {
        assert_eq!(parse_decimal(bad), None, "{bad}");
        assert!(validate(T::Number, &json!({"format": "number"}), &json!(bad)).is_err() || bad.is_empty());
    }
    assert_eq!(parse_decimal("-0.0").as_deref(), Some("0"));
    assert_eq!(parse_decimal("007.500").as_deref(), Some("7.5"));
    assert_eq!(parse_decimal("1234567890123456"), None);
    assert_eq!(parse_decimal("0.1234567891"), None);
    let number = json!({"format": "number"});
    assert_eq!(validate(T::Number, &number, &json!(12)).unwrap(), json!("12"));
    assert_eq!(validate(T::Number, &number, &json!(0.5)).unwrap(), json!("0.5"));
    assert_eq!(validate(T::Number, &number, &json!("")).unwrap(), Value::Null);
}

#[test]
fn text_is_cleaned_bounded_and_links_are_safe() {
    let text = json!({});
    let value = validate(T::Text, &text, &json!("  a\tb\u{200b}\u{202e}c ")).unwrap();
    assert_eq!(value::runs_text(&value), "  a bc ");
    assert_eq!(validate(T::Text, &text, &json!("")).unwrap(), Value::Null);
    assert!(validate(T::Text, &text, &json!("x".repeat(2001))).is_err());
    assert!(validate(T::Text, &text, &json!("x".repeat(2000))).is_ok());
    for href in ["https://example.com", "http://example.com/a", "mailto:a@example.com"] {
        let run = json!([{"text": "link", "href": href}]);
        assert!(validate(T::Text, &text, &run).is_ok(), "{href}");
    }
    for href in ["javascript:alert(1)", "data:text/html,x", "//example.com", "https://", "ftp://x.test", "https:///x"] {
        let run = json!([{"text": "link", "href": href}]);
        assert!(validate(T::Text, &text, &run).is_err(), "{href}");
    }
    assert!(validate(T::Url, &text, &json!("javascript:alert(1)")).is_err());
    assert!(validate(T::Url, &text, &json!("mailto:a@example.com")).is_err());
    assert!(validate(T::Email, &text, &json!("a@b")).is_err());
    assert!(validate(T::Email, &text, &json!("a b@example.com")).is_err());
    assert!(validate(T::Email, &text, &json!("a@example.com")).is_ok());
    assert!(validate(T::Phone, &text, &json!("12")).is_err());
    assert!(validate(T::Phone, &text, &json!("+351 912 345 678")).is_ok());
    assert!(validate(T::Phone, &text, &json!("call me")).is_err());
}

#[test]
fn select_values_are_option_ids() {
    let (_, select) = specs().into_iter().find(|(name, _)| *name == "select").unwrap();
    let (_, multi) = specs().into_iter().find(|(name, _)| *name == "multi").unwrap();
    assert_eq!(validate(T::Select, &select.config, &json!("s2")).unwrap(), json!("s2"));
    // A name is not an id.
    assert!(validate(T::Select, &select.config, &json!("Two")).is_err());
    assert!(validate(T::Select, &select.config, &json!(["s1"])).is_err());
    assert_eq!(
        validate(T::MultiSelect, &multi.config, &json!(["m1", "m3"])).unwrap(),
        json!(["m1", "m3"])
    );
    assert!(validate(T::MultiSelect, &multi.config, &json!(["m1", "m1"])).is_err());
    assert!(validate(T::MultiSelect, &multi.config, &json!(["nope"])).is_err());
    assert_eq!(validate(T::MultiSelect, &multi.config, &json!([])).unwrap(), Value::Null);
}

#[test]
fn options_have_unique_names_known_colours_and_groups_only_on_status() {
    let ok = config::normalise(
        T::Select,
        &json!({"options": [{"name": "A"}, {"id": "keep", "name": "B", "color": "red"}]}),
    )
    .unwrap();
    let options = config::options_of(&ok);
    assert_eq!(options.len(), 2);
    assert_eq!(options[1].id, "keep");
    assert!(!options[0].id.is_empty() && options[0].id != options[1].id);
    for bad in [
        json!({"options": [{"name": "A"}, {"name": "a"}]}),
        json!({"options": [{"id": "x", "name": "A"}, {"id": "x", "name": "B"}]}),
        json!({"options": [{"name": "A", "color": "mauve"}]}),
        json!({"options": [{"name": "A", "group": "todo"}]}),
        json!({"options": [{"name": ""}]}),
        json!({"options": [{"name": "A", "extra": 1}]}),
        json!({"options": (0..101).map(|n| json!({"name": format!("o{n}")})).collect::<Vec<_>>()}),
        json!({"nope": 1}),
    ] {
        assert!(config::normalise(T::Select, &bad).is_err(), "{bad}");
    }
    let status = config::normalise(T::Status, &json!({"options": [{"name": "A"}, {"name": "B", "group": "done"}]})).unwrap();
    let groups: Vec<_> = config::options_of(&status).iter().map(|o| o.group).collect();
    assert_eq!(groups, [Some(StatusGroup::Todo), Some(StatusGroup::Done)]);
    assert!(config::normalise(T::Status, &json!({"options": [{"name": "A", "group": "later"}]})).is_err());
}

#[test]
fn configs_are_canonical_and_closed() {
    for ty in T::ALL.iter().copied().filter(|ty| ty.creatable()) {
        let default = default_config(ty);
        if matches!(ty, T::Relation) {
            assert!(config::normalise(ty, &default).is_err());
            continue;
        }
        let once = config::normalise(ty, &default).unwrap();
        assert_eq!(config::normalise(ty, &once).unwrap(), once, "{ty:?}");
        let mut extra = once.clone();
        extra.as_object_mut().unwrap().insert("extra".into(), json!(1));
        assert!(config::normalise(ty, &extra).is_err(), "{ty:?}");
    }
    let table = Uuid::now_v7().to_string();
    assert!(config::normalise(T::Relation, &json!({"table": table})).is_ok());
    assert!(config::normalise(T::Relation, &json!({"table": "nope"})).is_err());
    assert!(config::normalise(T::Relation, &json!({"table": table, "pair": "nope"})).is_err());
    assert!(config::normalise(T::Number, &json!({"format": "currency"})).is_err());
    assert!(config::normalise(T::Number, &json!({"format": "currency", "currency": "eur"})).is_err());
    assert!(config::normalise(T::Number, &json!({"format": "percent", "currency": "EUR"})).is_err());
    assert!(config::normalise(T::UniqueId, &json!({"prefix": "a b"})).is_err());
    assert!(config::normalise(T::CampbooksLink, &json!({"types": ["tasks.task", "tasks.task"]})).is_err());
    assert!(config::normalise(T::CampbooksLink, &json!({"types": ["Tasks"]})).is_err());
}

#[test]
fn dates_follow_their_config() {
    let day_only = json!({"time": false});
    let with_time = json!({"time": true});
    assert_eq!(
        validate(T::Date, &day_only, &json!({"start": "2026-10-26"})).unwrap(),
        json!({"start": "2026-10-26"})
    );
    assert!(validate(T::Date, &day_only, &json!({"start": "2026-10-26T10:00:00+01:00"})).is_err());
    assert!(validate(T::Date, &with_time, &json!({"start": "2026-10-26T10:00:00+01:00", "time_zone": "Europe/Lisbon"})).is_ok());
    // A day is allowed where times are.
    assert!(validate(T::Date, &with_time, &json!({"start": "2026-10-26"})).is_ok());
    for bad in [
        json!({"start": "2026-13-01"}),
        json!({"start": "2026-02-30"}),
        json!({"start": "26-10-2026"}),
        json!({"start": "2026-10-26", "end": "2026-10-25"}),
        json!({"start": "2026-10-26", "end": "2026-10-27T10:00:00+00:00"}),
        json!({"end": "2026-10-27"}),
        json!({"start": "2026-10-26", "time_zone": "Europe/Lisbon"}),
        json!({"start": "2026-10-26", "other": 1}),
    ] {
        assert!(validate(T::Date, &with_time, &bad).is_err(), "{bad}");
    }
    assert!(validate(T::Date, &day_only, &json!({"start": "2026-10-26", "end": "2026-10-28"})).is_ok());
    let mut rng = Rng(11);
    for _ in 0..200 {
        let text = instant(&mut rng);
        assert_eq!(parse_when(&text).unwrap().text(), text);
    }
}

#[test]
fn ids_files_and_links_are_unique_bounded_and_shaped() {
    let none = json!({});
    let a = Uuid::now_v7().to_string();
    assert_eq!(validate(T::Person, &none, &json!([a, a])).unwrap(), json!([a]));
    assert!(validate(T::Person, &none, &json!(["not-a-uuid"])).is_err());
    let many: Vec<String> = (0..101).map(|n| Uuid::from_u128(n + 1).to_string()).collect();
    assert!(validate(T::Relation, &none, &json!(many)).is_err());
    assert!(validate(T::Relation, &none, &json!(many[..100])).is_ok());
    let file = json!({"hash": "0".repeat(64), "name": " a.pdf ", "size": 10, "type": "application/pdf"});
    assert_eq!(validate(T::Files, &none, &json!([file])).unwrap()[0]["name"], "a.pdf");
    for bad in [
        json!({"hash": "G".repeat(64), "name": "a", "size": 1, "type": "text/plain"}),
        json!({"hash": "0".repeat(64), "name": "", "size": 1, "type": "text/plain"}),
        json!({"hash": "0".repeat(64), "name": "a", "size": 99_999_999, "type": "text/plain"}),
        json!({"hash": "0".repeat(64), "name": "a", "size": 1, "type": "nonsense"}),
    ] {
        assert!(validate(T::Files, &none, &json!([bad])).is_err());
    }
    let link = json!({"types": ["tasks.task"]});
    assert!(validate(T::CampbooksLink, &link, &json!([{"type": "tasks.task", "id": "abc"}])).is_ok());
    assert!(validate(T::CampbooksLink, &link, &json!([{"type": "schedule.event", "id": "abc"}])).is_err());
    assert!(validate(T::CampbooksLink, &link, &json!([{"type": "tasks.task", "id": "a b"}])).is_err());
    assert!(validate(T::CampbooksLink, &link, &json!([{"type": "tasks.task", "id": "abc", "title": "x"}])).is_err());
    assert!(validate(T::CampbooksLink, &json!({"types": []}), &json!([{"type": "x", "id": "1"}])).is_err());
}

fn row(rng: &mut Rng, specs: &[(&str, Spec)], number: i64) -> eval::RowFacts {
    let base = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap().to_utc();
    let at = |rng: &mut Rng| base + chrono::TimeDelta::hours(i64::try_from(rng.below(5000)).unwrap());
    let people = members();
    let cells = specs
        .iter()
        .filter(|(_, spec)| spec.ty.stored())
        .map(|(_, spec)| (spec.id, generate(rng, spec)))
        .collect();
    eval::RowFacts {
        title: if rng.chance(10) { String::new() } else { rng.pick(WORDS).to_string() },
        number,
        created_at: at(rng),
        created_by: *rng.pick(&people),
        edited_at: at(rng),
        edited_by: *rng.pick(&people),
        cells,
    }
}

#[test]
fn ordering_is_total_with_empty_cells_last() {
    let all = specs();
    let mut rng = Rng(99);
    let rows: Vec<_> = (1..=40).map(|n| row(&mut rng, &all, n)).collect();
    for (name, spec) in all.iter().filter(|(_, spec)| spec.ty.sortable()) {
        for direction in [Direction::Asc, Direction::Desc] {
            let cmp = |a: &eval::RowFacts, b: &eval::RowFacts| eval::compare(spec, direction, a, b);
            for a in &rows {
                assert_eq!(cmp(a, a), Ordering::Equal, "{name}");
                for b in &rows {
                    assert_eq!(cmp(a, b), cmp(b, a).reverse(), "{name}: not antisymmetric");
                    for c in rows.iter().take(8) {
                        if cmp(a, b) != Ordering::Greater && cmp(b, c) != Ordering::Greater {
                            assert_ne!(cmp(a, c), Ordering::Greater, "{name}: not transitive");
                        }
                    }
                }
            }
        }
    }
}

fn spec_of<'a>(all: &'a [(&str, Spec)], name: &str) -> &'a Spec {
    &all.iter().find(|(n, _)| *n == name).unwrap().1
}

#[test]
fn filters_are_checked_against_the_tables_properties() {
    let all = specs();
    let map: HashMap<Uuid, Spec> = all.iter().map(|(_, s)| (s.id, s.clone())).collect();
    let number = spec_of(&all, "number").id;
    let select = spec_of(&all, "select").id;
    let ok = json!({"and": [
        {"property": number, "operator": "gt", "value": "10"},
        {"or": [{"property": select, "operator": "is_not", "value": "s1"}]},
    ]});
    assert!(filter::parse(&ok, &map).is_ok());
    for bad in [
        json!({"property": number, "operator": "contains", "value": "1"}),
        json!({"property": number, "operator": "gt", "value": "lots"}),
        json!({"property": number, "operator": "gt"}),
        json!({"property": Uuid::now_v7(), "operator": "is", "value": "x"}),
        json!({"property": select, "operator": "is_any_of", "value": "s1"}),
        json!({"and": [], "or": []}),
        json!({"property": number, "operator": "is", "value": "1", "extra": 1}),
        json!({"and": [{"and": [{"and": [{"and": []}]}]}]}),
        json!("nope"),
    ] {
        assert!(filter::parse(&bad, &map).is_err(), "{bad}");
    }
    // Three levels deep is the limit.
    assert!(filter::parse(&json!({"and": [{"or": [{"and": []}]}]}), &map).is_ok());
    let many: Vec<Value> = (0..51)
        .map(|_| json!({"property": number, "operator": "is", "value": "1"}))
        .collect();
    assert!(filter::parse(&json!({"and": many}), &map).is_err());
    // Operators exist per type, and nothing is filtered by a reserved type.
    for (_, spec) in &all {
        assert!(!filter::operators(spec).is_empty(), "{:?}", spec.ty);
    }
    assert!(filter::operators(&Spec::new(Uuid::now_v7(), T::External, json!({}))).is_empty());
}

#[test]
fn sorts_and_summaries_are_checked() {
    let all = specs();
    let map: HashMap<Uuid, Spec> = all.iter().map(|(_, s)| (s.id, s.clone())).collect();
    let number = spec_of(&all, "number").id;
    let person = spec_of(&all, "person").id;
    let checkbox = spec_of(&all, "checkbox").id;
    assert!(filter::parse_sorts(&json!([{"property": number, "direction": "desc"}]), &map).is_ok());
    assert!(filter::parse_sorts(&json!([{"property": person, "direction": "asc"}]), &map).is_err());
    assert!(filter::parse_sorts(&json!([{"property": number, "direction": "asc"}, {"property": number, "direction": "desc"}]), &map).is_err());
    let six: Vec<Value> = ["number", "title", "text", "date", "checkbox", "select"]
        .iter()
        .map(|n| json!({"property": spec_of(&all, n).id, "direction": "asc"}))
        .collect();
    assert!(filter::parse_sorts(&json!(six), &map).is_err());
    assert!(filter::parse_summaries(&json!([{"property": number, "kind": "sum"}, {"property": checkbox, "kind": "percent_checked"}]), &map).is_ok());
    assert!(filter::parse_summaries(&json!([{"property": checkbox, "kind": "sum"}]), &map).is_err());
    assert!(filter::parse_summaries(&json!([{"property": person, "kind": "count"}]), &map).is_ok());
}

#[test]
fn the_oracle_agrees_with_itself_on_complements() {
    // `is_not` is exactly the complement of `is`, an empty cell included.
    let all = specs();
    let mut rng = Rng(5);
    let rows: Vec<_> = (1..=60).map(|n| row(&mut rng, &all, n)).collect();
    let map: HashMap<Uuid, Spec> = all.iter().map(|(_, s)| (s.id, s.clone())).collect();
    for (name, spec) in &all {
        let ops = filter::operators(spec);
        for (positive, negative) in [(Operator::Is, Operator::IsNot), (Operator::Contains, Operator::DoesNotContain), (Operator::IsEmpty, Operator::IsNotEmpty)] {
            if !ops.contains(&positive) || !ops.contains(&negative) {
                continue;
            }
            let operand = match spec.ty {
                T::Select | T::Status => json!("s1"),
                T::MultiSelect => json!("m1"),
                T::Person => json!(Uuid::from_u128(1).to_string()),
                T::Relation => json!(Uuid::from_u128(100).to_string()),
                T::Number | T::UniqueId => json!("5"),
                T::CampbooksLink => json!({"type": "tasks.task", "id": "id-1"}),
                T::Created | T::Edited if matches!(*name, "created_by") => json!(Uuid::from_u128(1).to_string()),
                _ => json!("alpha"),
            };
            let condition = |op: Operator| {
                filter::parse(&json!({"property": spec.id, "operator": op, "value": operand}), &map)
            };
            let (Ok(p), Ok(n)) = (condition(positive), condition(negative)) else {
                continue;
            };
            for r in &rows {
                assert_ne!(
                    eval::matches(&p, &map, r),
                    eval::matches(&n, &map, r),
                    "{name} {positive:?}"
                );
            }
        }
    }
}

#[test]
fn a_changed_type_converts_what_it_can_and_the_result_is_always_valid() {
    let all = specs();
    let mut rng = Rng(2026);
    for (from_name, from) in all.iter().filter(|(_, s)| s.ty.editable()) {
        let cells: Vec<(usize, Value)> = (0..60).map(|k| (k, generate(&mut rng, from))).collect();
        for to in T::ALL.iter().copied().filter(|ty| ty.editable() && ty.creatable()) {
            if to == T::Relation {
                continue;
            }
            let plan = convert::plan(from, to, None, &cells).unwrap();
            assert_eq!(plan.values.len(), cells.len());
            for ((key, original), (same, converted)) in cells.iter().zip(&plan.values) {
                assert_eq!(key, same);
                match converted {
                    // An empty cell stays empty.
                    Some(Value::Null) => {}
                    Some(value) => assert_eq!(
                        validate(to, &plan.config, value).as_ref(),
                        Ok(value),
                        "{from_name} -> {to:?}: {original} became {value}"
                    ),
                    None => assert!(!original.is_null()),
                }
                if original.is_null() {
                    assert_eq!(converted, &Some(Value::Null));
                }
            }
        }
    }
}

#[test]
fn text_becomes_a_select_with_one_option_per_distinct_name() {
    let all = specs();
    let text = spec_of(&all, "text");
    let cells: Vec<(usize, Value)> = ["Red", "red", "Blue", "Red"]
        .iter()
        .enumerate()
        .map(|(k, word)| (k, validate(T::Text, &json!({}), &json!(word)).unwrap()))
        .collect();
    let plan = convert::plan(text, T::Select, None, &cells).unwrap();
    let options = config::options_of(&plan.config);
    assert_eq!(options.len(), 2);
    let ids: Vec<_> = plan.values.iter().map(|(_, v)| v.clone().unwrap()).collect();
    assert_eq!(ids[0], ids[1]);
    assert_eq!(ids[0], ids[3]);
    assert_ne!(ids[0], ids[2]);
    // And back: the names are the text.
    let select = Spec::new(Uuid::now_v7(), T::Select, plan.config.clone());
    let back_cells: Vec<(usize, Value)> = plan.values.iter().map(|(k, v)| (*k, v.clone().unwrap())).collect();
    let back = convert::plan(&select, T::Text, None, &back_cells).unwrap();
    assert_eq!(value::runs_text(back.values[2].1.as_ref().unwrap()), "Blue");
}

#[test]
fn what_cannot_be_converted_is_reported_not_invented() {
    let all = specs();
    let person = spec_of(&all, "person");
    let cells = vec![(1, json!([Uuid::from_u128(1).to_string()]))];
    for to in [T::Text, T::Number, T::Date, T::Select, T::Checkbox] {
        let plan = convert::plan(person, to, None, &cells).unwrap();
        assert_eq!(plan.values[0].1, None, "{to:?}");
    }
    // Neither side may be a computed or reserved type.
    assert!(convert::plan(person, T::Created, None, &cells).is_err());
    assert!(convert::plan(person, T::NotionFormula, None, &cells).is_err());
    let title = spec_of(&all, "title");
    assert!(convert::plan(title, T::Text, None, &cells).is_err());
    // A number becomes text and back.
    let number = spec_of(&all, "number");
    let cells = vec![(1, json!("12.5")), (2, Value::Null)];
    let text = convert::plan(number, T::Text, None, &cells).unwrap();
    assert_eq!(value::runs_text(text.values[0].1.as_ref().unwrap()), "12.5");
    assert_eq!(text.values[1].1, Some(Value::Null));
}
