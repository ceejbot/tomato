/// Implement serialization into strings that can be eval-ed in bash.

use toml_edit::{Item, Value};

pub fn format_bash(item: Item) -> String {
	// 'ware hackery!
	match item {
		Item::None => "".to_string(),
		Item::Value(v) => format_bash_value(v),
		Item::Table(table) => {
			let mut lines = vec!["declare -A bashval".to_string()];
			table.iter().for_each(|(k, v)| {
				lines.push(format!("bashval[{k}]={}", format_bash(v.clone())));
			});
			lines.join("\n")
		}
		// TODO unimplemented
		Item::ArrayOfTables(aot) => aot.to_string(),
	}
}

fn format_bash_value(v: Value) -> String {
	match v {
		Value::String(s) => s.to_string().trim().to_string(),
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
				.map(|xs| format_bash_value(xs.clone()).trim().to_owned())
				.collect::<Vec<String>>()
				.join(" ");
			format!("( {output} )")
		}
		Value::InlineTable(table) => {
			// this could be better. probably should add a keyname param all the way up
			// the chain to make this case work
			let mut lines = vec!["declare -A bashval".to_string()];
			table.iter().for_each(|(k, v)| {
				lines.push(format!("bashval[{k}]={}", format_bash_value(v.clone())));
			});
			lines.join("\n")
		},
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{Keyspec, get_key};
	use toml_edit::Document;
	use std::str::FromStr;

	#[test]
	fn bash_assoc_array() {
		let toml = r#"
name = "testtable"
clap = { version = "3.2.16", features = ["derive"] }"#;
        let expected = r#"declare -A bashval
bashval[version]="3.2.16"
bashval[features]=( "derive" )"#;

		let mut doc = toml
			.parse::<Document>()
			.expect("test string should be valid toml");

		let key = Keyspec::from_str("clap").unwrap();
		let item = get_key(&mut doc, &key).expect("expected to get key 'clap'");
		let bashified = format_bash(item);
		assert_eq!(bashified, expected);
	}
}
