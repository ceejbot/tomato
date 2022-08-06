/// Implement json serialization for the toml_edit data structures
use toml_edit::{Item, Value};

/// Turn a toml_edit::Item into a json Value
fn to_json(item: Item) -> serde_json::Value {
    match item {
        Item::None => serde_json::Value::Null,
        Item::Value(value) => value_to_json(value),
        Item::Table(table) => table_to_json(&table),
        Item::ArrayOfTables(aot) => {
            let items: Vec<serde_json::Value> = aot.iter().map(table_to_json).collect();
            serde_json::Value::Array(items)
        }
    }
}

fn table_to_json(table: &toml_edit::Table) -> serde_json::Value {
    let obj: serde_json::Map<String, serde_json::Value> = table
        .iter()
        .map(|(k, v)| (k.to_string(), to_json(v.clone())))
        .collect();
    serde_json::Value::Object(obj)
}

/// Turn a toml_edit::Value into a serde_json::Value
fn value_to_json(v: Value) -> serde_json::Value {
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
        // TODO needs pretty-printing
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
pub fn format_json(item: Item) -> String {
    let json = to_json(item);
    json.to_string()
}
