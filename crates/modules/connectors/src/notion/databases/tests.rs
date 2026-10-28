//! A Notion database in the neutral form: properties, values, hashes and
//! the payloads written back.

use serde_json::{Value, json};
use uuid::Uuid;

use super::*;

fn rich(text: &str) -> Value {
    json!([{"type": "text", "text": {"content": text}, "plain_text": text,
            "annotations": {"bold": true, "italic": false, "strikethrough": false,
                            "code": false, "underline": false, "color": "default"},
            "href": null}])
}

fn data_source() -> Value {
    json!({
        "object": "data_source",
        "properties": {
            "Name": {"id": "title", "name": "Name", "type": "title", "title": {}},
            "Stage": {"id": "p1", "name": "Stage", "type": "select",
                      "select": {"options": [
                          {"id": "a", "name": "Lead", "color": "blue"},
                          {"id": "b", "name": "Won", "color": "green"}]}},
            "Value": {"id": "p2", "name": "Value", "type": "number", "number": {"format": "euro"}},
            "Total": {"id": "p3", "name": "Total", "type": "formula", "formula": {}},
            "Owner": {"id": "p4", "name": "Owner", "type": "people", "people": {}},
            "Deals": {"id": "p5", "name": "Deals", "type": "relation", "relation": {}},
            "Weird": {"id": "p6", "name": "Weird", "type": "brand_new_kind", "brand_new_kind": {}},
        }
    })
}

#[test]
fn the_schema_has_the_title_first_and_kinds_it_knows() {
    let schema = schema_of(&data_source());
    assert_eq!(schema.properties[0].kind, "title");
    let kinds: Vec<&str> = schema.properties.iter().map(|p| p.kind.as_str()).collect();
    assert!(kinds.contains(&"select") && kinds.contains(&"formula") && kinds.contains(&"other"));
    let stage = schema.properties.iter().find(|p| p.id == "p1").unwrap();
    assert_eq!(stage.options.len(), 2);
    assert_eq!(stage.options[0].name, "Lead");
    let value = schema.properties.iter().find(|p| p.id == "p2").unwrap();
    assert_eq!(value.format.as_deref(), Some("euro"));
}

#[test]
fn a_property_hash_moves_with_the_name_the_kind_and_the_options() {
    let before = schema_of(&data_source());
    let mut renamed = data_source();
    let stage = renamed["properties"]
        .as_object_mut()
        .unwrap()
        .remove("Stage")
        .unwrap();
    renamed["properties"]["Phase"] = stage;
    let after = schema_of(&renamed);
    assert_ne!(before.hash, after.hash);
    let stage = |s: &Schema| {
        s.properties
            .iter()
            .find(|p| p.id == "p1")
            .unwrap()
            .hash
            .clone()
    };
    assert_ne!(stage(&before), stage(&after));
    // The same schema read twice hashes the same, whatever the key order.
    assert_eq!(before.hash, schema_of(&data_source()).hash);
    let mut added = data_source();
    added["properties"]["Stage"]["select"]["options"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": "c", "name": "Lost", "color": "red"}));
    assert_ne!(before.hash, schema_of(&added).hash);
}

#[test]
fn values_are_neutral_and_hashed() {
    let schema = schema_of(&data_source());
    let page = json!({
        "id": "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9d01",
        "last_edited_time": "2026-10-27T10:00:00.000Z",
        "properties": {
            "Name": {"id": "title", "type": "title", "title": rich("Acme")},
            "Stage": {"id": "p1", "type": "select", "select": {"id": "a", "name": "Lead", "color": "blue"}},
            "Value": {"id": "p2", "type": "number", "number": 1200.5},
            "Total": {"id": "p3", "type": "formula", "formula": {"type": "number", "number": 7}},
            "Owner": {"id": "p4", "type": "people", "people": [{"object": "user", "name": "Rui"}]},
            "Deals": {"id": "p5", "type": "relation", "relation": [{"id": "x"}], "has_more": false},
            "Weird": {"id": "p6", "type": "brand_new_kind", "brand_new_kind": {"a": 1}},
        }
    });
    let entry = entry_of(&page, &schema).unwrap();
    let cell = |id: &str| entry.cells.iter().find(|c| c.property == id).unwrap();
    assert_eq!(cell("title").value[0]["text"], "Acme");
    assert_eq!(cell("title").value[0]["bold"], true);
    assert_eq!(cell("p1").value, json!({"id": "a", "name": "Lead"}));
    assert_eq!(cell("p2").value, json!(1200.5));
    assert_eq!(cell("p3").value, json!("7"));
    assert_eq!(cell("p4").value, json!(["Rui"]));
    // A relation and a kind not known keep nothing of their content.
    assert_eq!(cell("p5").value, Value::Null);
    assert_eq!(cell("p6").value, Value::Null);
    assert_eq!(
        cell("p1").hash,
        hash_of(&json!({"id": "a", "name": "Lead"}))
    );
    assert_eq!(
        entry.id,
        Uuid::parse_str("0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9d01").unwrap()
    );
}

#[test]
fn a_long_list_is_known_to_be_possibly_short() {
    let twenty_five: Vec<Value> = (0..25)
        .map(|i| json!({"plain_text": i.to_string()}))
        .collect();
    assert!(may_be_truncated("title", &json!({"title": twenty_five})));
    assert!(!may_be_truncated(
        "title",
        &json!({"title": [{"plain_text": "x"}]})
    ));
    assert!(!may_be_truncated("select", &json!({})));
}

#[test]
fn what_is_written_is_checked_against_the_kind() {
    let runs = json!([{"text": "Acme", "bold": true}]);
    let title = value_payload("title", &runs, &[]).unwrap();
    assert_eq!(title["title"][0]["text"]["content"], "Acme");
    assert_eq!(title["title"][0]["annotations"]["bold"], true);
    assert_eq!(
        value_payload("select", &json!({"name": "Won"}), &[]).unwrap(),
        json!({"select": {"name": "Won"}})
    );
    assert_eq!(
        value_payload("select", &Value::Null, &[]).unwrap(),
        json!({"select": null})
    );
    // A status can only be one of its options, and cannot be cleared.
    let options = vec![OptionRef {
        id: "s".into(),
        name: "Done".into(),
        color: "green".into(),
        group: Some("done".into()),
    }];
    assert!(value_payload("status", &json!({"name": "Done"}), &options).is_some());
    assert!(value_payload("status", &json!({"name": "Nope"}), &options).is_none());
    assert!(value_payload("status", &Value::Null, &options).is_none());
    // Kinds that are Notion's alone are never written.
    for kind in [
        "formula",
        "rollup",
        "people",
        "relation",
        "files",
        "created_time",
        "other",
    ] {
        assert!(value_payload(kind, &json!("x"), &[]).is_none(), "{kind}");
    }
    // The wrong shape is not written.
    assert!(value_payload("number", &json!("12"), &[]).is_none());
    assert!(value_payload("multi_select", &json!("x"), &[]).is_none());
}

#[test]
fn options_are_only_ever_added() {
    let keep = vec![OptionRef {
        id: "a".into(),
        name: "Lead".into(),
        color: "blue".into(),
        group: None,
    }];
    let wanted = vec![
        NewOption {
            name: "lead ".into(),
            color: Some("red".into()),
            group: None,
        },
        NewOption {
            name: "Won".into(),
            color: Some("green".into()),
            group: None,
        },
    ];
    let settings = column_settings("select", None, &wanted, &keep).unwrap();
    let listed = settings["select"]["options"].as_array().unwrap();
    // The one Notion has stays by id (not renamed, not recoloured); the new
    // one is added by name.
    assert_eq!(listed[0], json!({"id": "a"}));
    assert_eq!(listed[1]["name"], "Won");
    assert_eq!(listed.len(), 2);
    // A status cannot be made or have options added by the API.
    assert!(column_settings("status", None, &wanted, &keep).is_none());
    assert!(!CREATABLE.contains(&"status"));
}
