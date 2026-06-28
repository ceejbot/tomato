/// Implement serialization into strings that can be eval-ed in bash.
use toml_edit::{Item, Value};

/// Quote a string so it is safe to `eval` in bash: wrap it in single quotes and
/// escape any embedded single quote with the `'\''` idiom. Single-quoted strings
/// undergo no expansion in bash, so `$(...)`, backticks, and `$VAR` stay inert.
pub(crate) fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Format a toml_edit::Item and all child items as eval-able bash, if possible.
pub fn format_bash(item: &Item) -> String {
    // 'ware hackery!
    match item {
        Item::None => "".to_string(),
        Item::Value(v) => format_bash_value(v.clone()),
        Item::Table(table) => {
            let mut lines = vec!["declare -A bashval".to_string()];
            table.iter().for_each(|(k, v)| {
                lines.push(format!("bashval[{}]={}", shell_quote(k), format_bash(v)));
            });
            lines.join("\n")
        }
        // TODO: This bails and emits toml. It might instead emit a lot of
        // more usable bash, but... tbh in this situation the caller should
        // snag json and pass it to jq.
        Item::ArrayOfTables(aot) => aot.to_string(),
    }
}

/// Format a toml_edit::Value as a bash data type, if possible
fn format_bash_value(v: Value) -> String {
    match v {
        // Strings are single-quoted so they are safe to eval even when they
        // contain shell metacharacters. Numbers and booleans are bare literals.
        Value::String(s) => shell_quote(s.value()),
        Value::Integer(i) => i.into_value().to_string(),
        Value::Float(f) => f.into_value().to_string(),
        Value::Boolean(b) => match b.into_value() {
            true => "1".to_string(),
            false => "0".to_string(),
        },
        Value::Datetime(dt) => dt.into_value().to_string(),
        Value::Array(array) => {
            let output = array
                .iter()
                .map(|xs| format_bash_value(xs.clone()))
                .collect::<Vec<String>>()
                .join(" ");
            format!("( {output} )")
        }
        Value::InlineTable(table) => {
            // this could be better. probably should add a keyname param all the way up
            // the chain to make this case work
            let mut lines = vec!["declare -A bashval".to_string()];
            table.iter().for_each(|(k, v)| {
                lines.push(format!("bashval[{}]={}", shell_quote(k), format_bash_value(v.clone())));
            });
            lines.join("\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use toml_edit::DocumentMut;

    use super::*;
    use crate::{Keyspec, get_key};

    #[test]
    fn bash_ouput() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml.parse::<DocumentMut>().expect("test doc should be valid toml");

        let key = Keyspec::from_str("testcases.hashes.mats").expect("test keyspec expected to be valid");
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.hashes.mats");
        let formatted = format_bash(&item);
        assert_eq!(formatted, "( 'potatoes' 'salt' 'oil' 'frying' )");

        let key = Keyspec::from_str("testcases.numbers").expect("test keyspec expected to be valid");
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.numbers");
        let formatted = format_bash(&item);
        assert_eq!(formatted, r#"( 1 3 5 7 11 13 17 23 )"#);

        let key = Keyspec::from_str("testcases.hashes.color").expect("test keyspec expected to be valid");
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.numbers");
        let formatted = format_bash(&item);
        assert_eq!(formatted, "'brown'");

        let key = Keyspec::from_str("testcases.are_passing").expect("test keyspec expected to be valid");
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.are_passing");
        let formatted = format_bash(&item);
        assert_eq!(formatted, r#"1"#);

        let key = Keyspec::from_str("testcases.are_complete").expect("test keyspec expected to be valid");
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.are_complete");
        let formatted = format_bash(&item);
        assert_eq!(formatted, r#"0"#);
    }

    #[test]
    fn bash_assoc_array() {
        let toml = r#"
name = "testtable"
inline_table = { catname = "Kitsune", fruit = "kumquat", "safe_pet" = true, class = "Archaeologist" }"#;
        let expected = r#"declare -A bashval
bashval['catname']='Kitsune'
bashval['fruit']='kumquat'
bashval['safe_pet']=1
bashval['class']='Archaeologist'"#;

        let mut doc = toml.parse::<DocumentMut>().expect("test string should be valid toml");

        let key = Keyspec::from_str("inline_table").expect("test keyspec expected to be valid");
        let item = get_key(&mut doc, &key).expect("expected to get key 'inline_table'");
        let bashified = format_bash(&item);
        assert_eq!(bashified, expected);
    }

    #[test]
    fn bash_output_is_eval_safe() {
        // A string with shell metacharacters must come out single-quoted and inert.
        let toml = r#"danger = "$(touch pwned)""#;
        let mut doc = toml.parse::<DocumentMut>().expect("test string should be valid toml");
        let key = Keyspec::from_str("danger").expect("test keyspec expected to be valid");
        let item = get_key(&mut doc, &key).expect("expected to get key 'danger'");
        assert_eq!(format_bash(&item), r#"'$(touch pwned)'"#);

        // An embedded single quote is escaped with the '\'' idiom.
        let toml = r#"q = "a'b""#;
        let mut doc = toml.parse::<DocumentMut>().expect("test string should be valid toml");
        let key = Keyspec::from_str("q").expect("test keyspec expected to be valid");
        let item = get_key(&mut doc, &key).expect("expected to get key 'q'");
        assert_eq!(format_bash(&item), r#"'a'\''b'"#);
    }
}
