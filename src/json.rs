/// Implement json serialization for the toml_edit data structures
use toml_edit::{Item, Value};

/// Turn a toml_edit::Item into a json Value
pub fn to_json(item: &Item) -> serde_json::Value {
    match item {
        Item::None => serde_json::Value::Null,
        Item::Value(value) => value_to_json(value.clone()),
        Item::Table(table) => table_to_json(table),
        Item::ArrayOfTables(aot) => {
            let items: Vec<serde_json::Value> = aot.iter().map(table_to_json).collect();
            serde_json::Value::Array(items)
        }
    }
}

/// Turn a toml_edit::Table structure into a json object
pub fn table_to_json(table: &toml_edit::Table) -> serde_json::Value {
    let obj: serde_json::Map<String, serde_json::Value> = table
        .iter()
        .map(|(k, v)| (k.to_string(), to_json(v)))
        .collect();
    serde_json::Value::Object(obj)
}

/// Turn a toml_edit::Value into a serde_json::Value
pub fn value_to_json(v: Value) -> serde_json::Value {
    match v {
        Value::String(s) => serde_json::Value::String(s.into_value()),
        Value::Integer(i) => serde_json::Value::Number(i.into_value().into()),
        Value::Float(f) => {
            let val = f.into_value();
            if let Some(num) = serde_json::Number::from_f64(val) {
                serde_json::Value::Number(num)
            } else {
                serde_json::Value::String(val.to_string())
            }
        }
        Value::Boolean(b) => serde_json::Value::Bool(b.into_value()),
        Value::Datetime(dt) => serde_json::Value::String(dt.into_value().to_string()),
        Value::Array(array) => {
            let items: Vec<serde_json::Value> =
                array.iter().map(|xs| value_to_json(xs.clone())).collect();
            serde_json::Value::Array(items)
        }
        Value::InlineTable(table) => {
            let obj: serde_json::Map<String, serde_json::Value> = table
                .iter()
                .map(|(k, v)| (k.to_string(), value_to_json(v.clone())))
                .collect();
            serde_json::Value::Object(obj)
        }
    }
}

/// Given any toml_edit::Item, serialize it to a valid json string
pub fn format_json(item: &Item) -> String {
    let json = to_json(item);
    json.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{get_key, Keyspec};
    use std::str::FromStr;
    use toml_edit::Document;

    #[test]
    fn json_output() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml
            .parse::<Document>()
            .expect("test doc should be valid toml");

        let key = Keyspec::from_str("testcases.hashes.mats").unwrap();
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.hashes.mats");
        let formatted = format_json(&item);
        assert_eq!(formatted, r#"["potatoes","salt","oil","frying"]"#);

        let key = Keyspec::from_str("testcases.numbers").unwrap();
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.numbers");
        let formatted = format_json(&item);
        assert_eq!(formatted, r#"[1,3,5,7,11,13,17,23]"#);

        let key = Keyspec::from_str("testcases.hashes.color").unwrap();
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.numbers");
        let formatted = format_json(&item);
        assert_eq!(formatted, r#""brown""#);

        let key = Keyspec::from_str("testcases.are_passing").unwrap();
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.are_passing");
        let formatted = format_json(&item);
        assert_eq!(formatted, r#"true"#);

        let key = Keyspec::from_str("testcases.are_complete").unwrap();
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.are_complete");
        let formatted = format_json(&item);
        assert_eq!(formatted, r#"false"#);

        let key = Keyspec::from_str("nested").unwrap();
        let item = get_key(&mut doc, &key).expect("expected to find key nested");
        let formatted = format_json(&item);
        assert_eq!(formatted, r#"[{"entry":"one"},{"entry":"two"}]"#);

        let item = doc.as_item();
        let json = format_json(item);
        println!("{json}");
        assert_eq!(json, include_str!("../fixtures/sample.json").trim());
    }
}
