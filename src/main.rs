use clap::{Parser, Subcommand};
use regex::Regex;
use std::fmt::Display;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::str::FromStr;
use toml_edit::{Document, Item, Value};

pub mod json;
use json::format_json;
pub mod bash;
use bash::format_bash;

#[derive(Parser, Debug)]
#[clap(name = "🍅 tomato", version)]
/// A command-line tool to get and set values in toml files while preserving comments and
/// formatting.
///
/// Keys are written using `.` to separate path segments. You can use `array[idx]` syntax to index
/// into arrays if you want to. For example, to get the name of the current crate you're working on,
/// you'd run `tomato Cargo.toml get package.name`.
///
/// By default tomato emits data in a form suitable for immediate use in bash scripts if they are
/// primitive values: strings are unquoted, for instance. If you want to use more complex data
/// types, consider one of the other output formats.
pub struct Args {
    /// The toml file to operate on
    filepath: String,
    /// How to format the output: json, toml, bash, or raw (NOT FULLY IMPLEMENTED)
    #[clap(short, long, default_value = "raw")]
    format: Format,
    /// Back up the file to <filepath>.bak if we write a new version.
    #[clap(long, short)]
    backup: bool,
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    /// Get the value of a key from the given file
    Get {
        /// The key to look for. Use dots as path separators.
        key: Keyspec,
    },
    /// Delete a key from the given file, returning the previous value if one existed
    #[clap(aliases = &["del", "delete", "delet", "forget", "regret", "remove", "unset", "yank", "yeet"])]
    Rm {
        /// The key to remove from the file. Use dots as path separators.
        key: Keyspec,
    },
    /// Set a key to the given value, returning the previous value if one existed.
    Set {
        /// The key to set a value for
        key: Keyspec,
        /// The new value
        value: String,
    },
}

#[derive(Clone, Debug)]
/// How to format the output of more complex data structures.
pub enum Format {
    /// Strings are not quoted; suitable for primitive data types; default
    Raw,
    /// Suitable for dropping into bash for eval
    Bash,
    /// Output valid JSON
    Json,
    /// Output valid TOML
    Toml,
}

impl FromStr for Format {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_lowercase().as_str() {
            "raw" => Ok(Format::Raw),
            "bash" => Ok(Format::Bash),
            "json" => Ok(Format::Json),
            "toml" => Ok(Format::Toml),
            _ => Err(anyhow::anyhow!("{input} is not a supported output type")),
        }
    }
}

/// Read the toml file and parse it. Respond with an error that gets propagated up
/// if the file is not valid toml.
pub fn parse_file(fpath: &str) -> anyhow::Result<Document, anyhow::Error> {
    let file = File::open(fpath)?;
    let mut buf_reader = BufReader::new(file);
    let mut data = String::new();
    buf_reader.read_to_string(&mut data)?;
    let parsed = data
        .parse::<Document>()
        .unwrap_or_else(|_| panic!("{}", format!("The file {} is not valid toml.", fpath)));

    Ok(parsed)
}

#[derive(Clone, Debug, PartialEq)]
/// Keys can contain either name segments or array indexes.
pub enum KeySegment {
    Name(String),
    Index(usize),
}

impl Display for KeySegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(s) => {
                write!(f, "{s}")
            }
            Self::Index(i) => {
                write!(f, "{i}")
            }
        }
    }
}

#[derive(Debug, Clone)]
/// An internal representation of the dotted key string given on the command-line.
pub struct Keyspec {
    subkeys: Vec<KeySegment>,
}

impl Display for Keyspec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.subkeys
                .iter()
                .map(|k| k.to_string())
                .collect::<Vec<String>>()
                .join(".")
        )
    }
}

impl FromStr for Keyspec {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let tokens: Vec<&str> = input.split('.').collect();
        let mut subkeys: Vec<KeySegment> = Vec::with_capacity(tokens.len() * 2);

        // Tokens that look like "xxx[yyy]" are array references
        // it's the cheesiest thing in the world to implement this with regex, but I am cheesy
        let arraypatt = Regex::new(r"(\w+)\[(\d+)\]").unwrap();

        tokens.iter().try_for_each(|t| {
            let maybe_captures = arraypatt.captures(t);
            match maybe_captures {
                None => {
                    if let Ok(idx) = t.parse::<usize>() {
                        subkeys.push(KeySegment::Index(idx));
                    } else {
                        subkeys.push(KeySegment::Name(t.to_string()));
                    }
                }
                Some(captures) => {
                    if captures.len() != 3 {
                        anyhow::bail!("{} is not a valid key segment for tomato!", t);
                    } else {
                        subkeys.push(KeySegment::Name(captures[1].to_string()));
                        subkeys.push(KeySegment::Index(captures[2].parse()?))
                    }
                }
            };
            Ok(())
        })?;

        Ok(Keyspec { subkeys })
    }
}

/// Given a key segment, find that key in this node. Returns None if the key segment is an
/// int but the node is not an array.
pub fn get_in_node<'a>(key: &'a KeySegment, node: &'a mut Item) -> Option<&'a mut Item> {
    match key {
        KeySegment::Name(n) => node.get_mut(n),
        KeySegment::Index(idx) => {
            if node.as_array().is_some() {
                node.get_mut(*idx)
            } else {
                None
            }
        }
    }
}

/// Given a full dotted-form key from the command-line, find the matching value
/// in the given document. Responds with Item::None if not found.
pub fn get_key(toml: &mut Document, dotted_key: &Keyspec) -> Result<Item, anyhow::Error> {
    let mut node: &mut Item = toml.as_item_mut();
    let iterator = dotted_key.subkeys.iter();

    for k in iterator {
        let found = get_in_node(k, node);
        if found.is_none() {
            return Ok(Item::None);
        }
        node = found.unwrap();
    }

    Ok(node.clone())
}

/// Remove the node corresponding to the given key. If the key was not found, we
/// return an error saying so. Otherwise, we respond with the value that the key
/// used to point to.
pub fn remove_key(toml: &mut Document, dotted_key: &Keyspec) -> Result<Item, anyhow::Error> {
    let mut node: &mut Item = toml.as_item_mut();
    let mut parent_key: Keyspec = dotted_key.clone();
    let target = parent_key.subkeys.pop();
    if target.is_none() {
        anyhow::bail!("You must pass a key to remove!!");
    }
    let target = target.unwrap();
    let iterator = parent_key.subkeys.iter();

    for k in iterator {
        let found = get_in_node(k, node);
        if found.is_none() {
            anyhow::bail!("key {} not found in toml file", dotted_key);
        }
        node = found.unwrap();
        if let Item::None = node {
            anyhow::bail!("key {} not found in toml file", dotted_key);
        }
    }

    if let Some(found) = get_in_node(&target, node) {
        let original = found.clone();
        *found = Item::None;
        return Ok(original);
    }

    Ok(Item::None)
}

/// Set the given key to the new value, and respond with the original value.
/// Will replace null nodes if the parent was found, adding a new key to the
/// document. Will repond with an error if the key included an index into an
/// array for a non-array node in the document.
pub fn set_key(
    toml: &mut Document,
    dotted_key: &Keyspec,
    value: &str,
) -> Result<Item, anyhow::Error> {
    let mut node: &mut Item = toml.as_item_mut();
    let iterator = dotted_key.subkeys.iter();
    let mut found: Option<&mut Item>;

    for k in iterator {
        found = get_in_node(k, node);
        if found.is_none() {
            anyhow::bail!("unable to index into non-array at {}", dotted_key);
        }
        node = found.unwrap();
    }

    let original = node.clone();
    let existing: &mut Item = &mut *node;

    // Straight outta cargo-edit
    let existing_decor = existing
        .as_value()
        .map(|v| v.decor().clone())
        .unwrap_or_default();
    let mut new_value: Value = value.into();
    *new_value.decor_mut() = existing_decor;
    *existing = toml_edit::Item::Value(new_value);

    Ok(original)
}

/// Format the given toml_edit item for the desired kind of output.
pub fn format_item(item: &Item, output: Format) -> String {
    match output {
        Format::Raw => format_raw(item),
        Format::Bash => format_bash(item),
        Format::Json => format_json(item),
        Format::Toml => format_toml(item),
    }
}

/// Format the item as toml.
pub fn format_toml(item: &Item) -> String {
    item.to_string().trim().to_string()
}

/// Format the item as a primitive type ready to use in bash. Falls back to
/// json format for complex items, which might not be what you want.
pub fn format_raw(item: &Item) -> String {
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
pub fn format_raw_value(v: Value) -> String {
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

/// Parse command-line args and do whatever our user wants!
fn main() -> anyhow::Result<(), anyhow::Error> {
    let args = Args::parse();
    let mut toml = parse_file(&args.filepath)?;

    match args.cmd {
        Command::Get { key } => {
            let item = get_key(&mut toml, &key)?;
            println!("{}", format_item(&item, args.format));
        }
        Command::Rm { key } => {
            let original = remove_key(&mut toml, &key)?;
            if args.backup {
                std::fs::copy(&args.filepath, format!("{}.bak", args.filepath))?;
            }
            let mut output = File::create(args.filepath)?;
            // Note for future work: this won't be great for large files
            write!(output, "{toml}")?;
            println!("{}", format_item(&original, args.format));
        }
        Command::Set { key, value } => {
            let original = set_key(&mut toml, &key, &value)?;
            if args.backup {
                std::fs::copy(&args.filepath, format!("{}.bak", args.filepath))?;
            }
            let mut output = File::create(args.filepath)?;
            // Note for future work: this won't be great for large files
            write!(output, "{toml}")?;
            println!("{}", format_item(&original, args.format));
        }
    };

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_parsing_easy() {
        let mut key = Keyspec::from_str("a").unwrap();
        assert!(key.subkeys.len() == 1);
        assert_eq!(key.subkeys[0], KeySegment::Name("a".to_string()));

        key = Keyspec::from_str("a.b.c").unwrap();
        assert!(key.subkeys.len() == 3);
        assert_eq!(key.subkeys[2], KeySegment::Name("c".to_string()));
    }

    #[test]
    fn key_parsing_arrays() {
        let mut key = Keyspec::from_str("a[1]").unwrap();
        assert!(key.subkeys.len() == 2);
        assert_eq!(key.subkeys[0], KeySegment::Name("a".to_string()));
        assert_eq!(key.subkeys[1], KeySegment::Index(1));

        key = Keyspec::from_str("a[1].b[2]").unwrap();
        assert!(key.subkeys.len() == 4);
        assert_eq!(key.subkeys[2], KeySegment::Name("b".to_string()));
        assert_eq!(key.subkeys[3], KeySegment::Index(2));

        key = Keyspec::from_str("a[1].b.c[3]").unwrap();
        assert!(key.subkeys.len() == 5);
        assert_eq!(key.subkeys[2], KeySegment::Name("b".to_string()));
        assert_eq!(key.subkeys[3], KeySegment::Name("c".to_string()));
        assert_eq!(key.subkeys[4], KeySegment::Index(3));

        let identical = Keyspec::from_str("a.1.b.c.3").unwrap();
        assert!(identical.subkeys.len() == 5);
        assert_eq!(identical.subkeys[2], KeySegment::Name("b".to_string()));
        assert_eq!(identical.subkeys[3], KeySegment::Name("c".to_string()));
        assert_eq!(identical.subkeys[4], KeySegment::Index(3));
    }

    #[test]
    fn key_parsing_bad() {
        // Basically, my key parsing is _not good enough_
        // This should be an error but it is not.
        match Keyspec::from_str("a[bbbbb[bb]") {
            Ok(k) => {
                assert_eq!(k.to_string(), "a[bbbbb[bb]");
            }
            Err(e) => {
                assert!(e.to_string().contains("bbbb"));
            }
        };
    }

    #[test]
    fn get() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml
            .parse::<Document>()
            .expect("test doc should be valid toml");

        let key = Keyspec::from_str("testcases.hashes.color").unwrap();
        let item = get_key(&mut doc, &key).expect("expected to get key 'hashes.color'");
        assert_eq!("brown", format_item(&item, Format::Raw));
        assert_eq!("\"brown\"", format_item(&item, Format::Toml));

        let key = Keyspec::from_str("testcases.hashes.mats[1]").unwrap();
        let item = get_key(&mut doc, &key).expect("expected this key to be valid");
        assert_eq!("salt", format_item(&item, Format::Raw));
    }

    #[test]
    fn set() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml
            .parse::<Document>()
            .expect("test doc should be valid toml");

        let key = Keyspec::from_str("testcases.hashes.color").expect("test key should be valid");
        let item = set_key(&mut doc, &key, "taupe").expect("expected to find key 'hashes.color'");
        assert_eq!("brown", format_item(&item, Format::Raw));
        assert!(doc.to_string().contains("color = \"taupe\""));

        let key =
            Keyspec::from_str("testcases.hashes.mats[3]").expect("expected this key to be valid");
        let item = set_key(&mut doc, &key, "bacon").expect("could not find this key");
        assert_eq!("frying", format_item(&item, Format::Raw));
        assert!(doc.to_string().contains("bacon"));
    }

    #[test]
    fn yeet() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml
            .parse::<Document>()
            .expect("test doc should be valid toml");

        let key = Keyspec::from_str("testcases.hashes.color").unwrap();
        let item = remove_key(&mut doc, &key).expect("expected to find key 'hashes.color'");
        assert_eq!("brown", format_item(&item, Format::Raw));
        assert!(!doc.to_string().contains("color = \"brown\""));

        let key = Keyspec::from_str("testcases.hashes.mats[1]").unwrap();
        let item =
            remove_key(&mut doc, &key).expect("expected to find key testcases.hashes.mats[1]");
        assert_eq!("salt", format_item(&item, Format::Raw));
        assert!(doc
            .to_string()
            .contains(r#"mats = [ "potatoes", "oil", "frying" ]"#));
    }

    #[test]
    fn toml_output() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml
            .parse::<Document>()
            .expect("test doc should be valid toml");

        let key = Keyspec::from_str("testcases.hashes.mats").unwrap();
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.hashes.mats");
        let formatted = format_toml(&item);
        assert_eq!(formatted, r#"[ "potatoes", "salt", "oil", "frying" ]"#);

        let key = Keyspec::from_str("testcases.numbers").unwrap();
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.numbers");
        let formatted = format_toml(&item);
        assert_eq!(formatted, r#"[1, 3, 5, 7, 11, 13, 17, 23]"#);

        let key = Keyspec::from_str("testcases.hashes.color").unwrap();
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.numbers");
        let formatted = format_toml(&item);
        assert_eq!(formatted, r#""brown""#);

        let key = Keyspec::from_str("testcases.are_passing").unwrap();
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.are_passing");
        let formatted = format_toml(&item);
        assert_eq!(formatted, r#"true"#);

        let key = Keyspec::from_str("testcases.are_complete").unwrap();
        let item = get_key(&mut doc, &key).expect("expected to find key testcases.are_complete");
        let formatted = format_toml(&item);
        assert_eq!(formatted, r#"false"#);
    }
}
