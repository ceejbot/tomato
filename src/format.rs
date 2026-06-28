//! Output formatting. The [`Format`] enum selects how a `toml_edit` item is
//! rendered; `raw` and `toml` rendering live here, while the `json` and `bash`
//! renderers live in the sibling [`crate::json`] and [`crate::bash`] modules.

use std::str::FromStr;

use toml_edit::{Item, Value};

use crate::bash::format_bash;
use crate::errors::TomatoError;
use crate::json::{self, format_json};

#[derive(Clone, Copy, Debug)]
/// How to format the output of more complex data structures.
pub(crate) enum Format {
    /// Strings are not quoted; suitable for primitive data types; default
    Raw,
    /// Suitable for dropping into bash for eval; might not be suitable for complex structures
    Bash,
    /// Output valid JSON
    Json,
    /// Output valid TOML
    Toml,
}

impl FromStr for Format {
    type Err = TomatoError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_lowercase().as_str() {
            "raw" => Ok(Format::Raw),
            "bash" => Ok(Format::Bash),
            "json" => Ok(Format::Json),
            "toml" => Ok(Format::Toml),
            _ => Err(TomatoError::UnsupportedOutputType {
                format: input.to_string(),
            }),
        }
    }
}

/// Format the given toml_edit item for the desired kind of output.
pub(crate) fn format_item(item: &Item, output: Format) -> String {
    match output {
        Format::Raw => format_raw(item),
        Format::Bash => format_bash(item),
        Format::Json => format_json(item),
        Format::Toml => format_toml(item),
    }
}

/// Format a list of keys according to the output format
pub(crate) fn format_keys(keys: &[String], output: Format) -> String {
    match output {
        Format::Raw => keys.join("\n"),
        Format::Json => {
            // Create a TOML array and use the existing json formatter
            let toml_keys: Vec<toml_edit::Value> = keys.iter().map(|k| toml_edit::Value::from(k.as_str())).collect();
            let array = toml_edit::Array::from_iter(toml_keys);
            let item = Item::Value(Value::Array(array));
            format_json(&item)
        }
        Format::Bash => {
            let quoted_keys: Vec<String> = keys.iter().map(|k| format!("\"{}\"", k)).collect();
            format!("( {} )", quoted_keys.join(" "))
        }
        Format::Toml => {
            let toml_keys: Vec<toml_edit::Value> = keys.iter().map(|k| toml_edit::Value::from(k.as_str())).collect();
            let array = toml_edit::Array::from_iter(toml_keys);
            array.to_string().trim().to_string()
        }
    }
}

/// Format the item as toml.
pub(crate) fn format_toml(item: &Item) -> String {
    item.to_string().trim().to_string()
}

/// Format the item as a primitive type ready to use in bash. Falls back to
/// json format for complex items, which might not be what you want.
pub(crate) fn format_raw(item: &Item) -> String {
    match item {
        Item::None => "".to_string(),
        Item::Value(v) => format_raw_value(v.clone()),
        Item::Table(_) => format_json(item),
        Item::ArrayOfTables(_) => format_json(item),
    }
}

/// Format the value in a way useful immediately in bash scripts. This option
/// falls back to json for anything that doesn't make sense in that context,
/// such as toml tables.
pub(crate) fn format_raw_value(v: Value) -> String {
    match v {
        Value::String(s) => s.into_value(),
        Value::Integer(i) => i.into_value().to_string(),
        Value::Float(f) => f.into_value().to_string(),
        Value::Boolean(b) => match b.into_value() {
            true => "1".to_string(),
            false => "0".to_string(),
        },
        Value::Datetime(dt) => dt.into_value().to_string(),
        Value::Array(array) => array
            .iter()
            .map(|xs| format_raw_value(xs.clone()))
            .collect::<Vec<String>>()
            .join("\n"),
        Value::InlineTable(_) => json::value_to_json(v).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use toml_edit::DocumentMut;

    use super::*;
    use crate::ops::get_key;
    use crate::parser::Keyspec;

    #[test]
    fn toml_output() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml.parse::<DocumentMut>().expect("test doc should be valid toml");

        let key = Keyspec::from_str("testcases.hashes.mats").expect("test key should be valid");
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.hashes.mats");
        let formatted = format_toml(&item);
        assert_eq!(formatted, r#"[ "potatoes", "salt", "oil", "frying" ]"#);

        let key = Keyspec::from_str("testcases.numbers").expect("test key should be valid");
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.numbers");
        let formatted = format_toml(&item);
        assert_eq!(formatted, r#"[1, 3, 5, 7, 11, 13, 17, 23]"#);

        let key = Keyspec::from_str("testcases.hashes.color").expect("test key should be valid");
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.numbers");
        let formatted = format_toml(&item);
        assert_eq!(formatted, r#""brown""#);

        let key = Keyspec::from_str("testcases.are_passing").expect("test key should be valid");
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.are_passing");
        let formatted = format_toml(&item);
        assert_eq!(formatted, r#"true"#);

        let key = Keyspec::from_str("testcases.are_complete").expect("test key should be valid");
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.are_complete");
        let formatted = format_toml(&item);
        assert_eq!(formatted, r#"false"#);
    }
}
