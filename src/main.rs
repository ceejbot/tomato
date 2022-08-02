use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::str::FromStr;
use toml_edit::{Document, Item, Value};

#[derive(Parser, Debug)]
#[clap(name = "🍅 tomato", author, version)]
/// A command-line tool to get and set values in toml files while preserving comments and formatting.
///
/// Keys are written using `.` to separate path segments. You can use array[idx] syntax to index into
/// arrays if you want to. For example, to get the name of the current crate you're working on, you'd
/// run `tomato Cargo.toml get package.name`.
///
/// By default tomato emits data in raw mode. Strings are unquoted, for instance. This is
/// most appropriate for primitive types. If you need to consume more complex output, you might
/// want to select an output mode.
struct Args {
    /// The toml file to operate on
    filepath: String,
    /// How to format the output: json, bash, toml, or raw (no formatting)
    #[clap(short, long, default_value = "raw")]
    output: OutputMode,
    /// Back up the file to <filepath>.bak if we write a new version.
    #[clap(long, short)]
    backup: bool,
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Clone, Debug, Subcommand)]
enum Command {
    /// Get the value of a key from the given file
    Get {
        /// The key to look for. Use dots as path separators.
        keyspec: Keyspec,
    },
    /// Delete a key from the given file
    #[clap(aliases = &["del", "delete", "delet", "forget", "regret", "remove", "yank", "yeet"])]
    Rm {
        /// The key to remove from the file. Use dots as path separators.
        keyspec: Keyspec,
    },
    /// Set a key to the given value, returning the previous value if one existed.
    Set {
        /// The key to set a value for
        keyspec: Keyspec,
        /// The new value
        value: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
/// How to format the output of more complex data structures.
enum OutputMode {
    /// Strings are not quoted; suitable for primitive data types; default
    Raw,
    /// Output more complex data structures as valid bash
    Bash,
    /// Output valid JSON
    Json,
    /// Output valid TOML
    Toml,
}

impl FromStr for OutputMode {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_lowercase().as_str() {
            "bash" => Ok(OutputMode::Bash),
            "json" => Ok(OutputMode::Json),
            "raw" => Ok(OutputMode::Raw),
            "toml" => Ok(OutputMode::Toml),
            _ => Err(anyhow::anyhow!("{input} is not a supported output type")),
        }
    }
}

fn parse_file(fpath: &str) -> anyhow::Result<Document, anyhow::Error> {
    let file = File::open(fpath)?;
    let mut buf_reader = BufReader::new(file);
    let mut data = String::new();
    buf_reader.read_to_string(&mut data)?;
    let parsed = data
        .parse::<Document>()
        .unwrap_or_else(|_| panic!("{}", format!("The file {} is not valid toml.", fpath.red())));

    Ok(parsed)
}

#[derive(Clone, Debug, PartialEq)]
enum KeySegment {
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
struct Keyspec {
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

// TODO this can probably be used in clap argument parsing
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
                        anyhow::bail!("{} is not a valid key segment for tomato!", t.red());
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

fn get_in_node<'a>(key: &'a KeySegment, node: &'a mut Item) -> Option<&'a mut Item> {
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

fn get_dotted_key(toml: &mut Document, dotted_key: &Keyspec) -> Result<Item, anyhow::Error> {
    let mut node: &mut Item = toml.as_item_mut();
    let iterator = dotted_key.subkeys.iter();

    for k in iterator {
        let found = get_in_node(k, node);
        if found.is_none() {
            anyhow::bail!("key {} not found in toml file", dotted_key.red());
        }
        node = found.unwrap();
    }

    Ok(node.clone())
}

fn remove_dotted_key(toml: &mut Document, dotted_key: &Keyspec) -> Result<Item, anyhow::Error> {
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
            anyhow::bail!("key {} not found in toml file", dotted_key.red());
        }
        node = found.unwrap();
    }

    if let Some(found) = get_in_node(&target, node) {
        let original = found.clone();
        *found = Item::None;
        return Ok(original);
    }

    Ok(Item::None)
}

fn set_dotted_key(
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
            anyhow::bail!("key {} not found in toml file", dotted_key);
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

fn format_toml_item(item: Item, output: OutputMode) -> String {
    match output {
        OutputMode::Raw => format_raw(item),
        OutputMode::Bash => format_raw(item), // TODO
        OutputMode::Json => format_raw(item), // TODO
        OutputMode::Toml => item.to_string().trim().to_string(), // the easy case
    }
}

fn format_raw(item: Item) -> String {
    // It's very possible there's an easier way to do this but I haven't
    // been able to find it yet in the toml_edit api.
    match item {
        Item::None => "".to_string(),
        Item::Value(v) => {
            match v {
                Value::String(s) => s.into_value(),
                Value::Integer(i) => i.into_value().to_string(),
                Value::Float(f) => f.into_value().to_string(),
                Value::Boolean(b) => b.into_value().to_string(),
                // TODO needs pretty-printing
                Value::Datetime(dt) => dt.into_value().to_string(),
                // TODO needs pretty-printing
                Value::Array(array) => array.to_string(),
                // TODO needs pretty-printing
                Value::InlineTable(table) => table.to_string(),
            }
        }
        // TODO needs pretty-printing
        Item::Table(t) => t.to_string(),
        // TODO needs pretty-printing
        Item::ArrayOfTables(aot) => aot.to_string(),
    }
}

fn main() -> anyhow::Result<(), anyhow::Error> {
    let args = Args::parse();
    let mut toml = parse_file(&args.filepath)?;

    match args.cmd {
        Command::Get { keyspec } => {
            let item = get_dotted_key(&mut toml, &keyspec)?;
            println!("{}", format_toml_item(item, args.output));
        }
        Command::Rm { keyspec } => {
            let original = remove_dotted_key(&mut toml, &keyspec)?;
            if args.backup {
                std::fs::copy(&args.filepath, format!("{}.bak", args.filepath))?;
            }
            let mut output = File::create(args.filepath)?;
            // TODO this won't be great for large files
            write!(output, "{toml}")?;
            println!("{}", format_toml_item(original, args.output));
        }
        Command::Set { keyspec, value } => {
            let original = set_dotted_key(&mut toml, &keyspec, &value)?;
            if args.backup {
                std::fs::copy(&args.filepath, format!("{}.bak", args.filepath))?;
            }
            let mut output = File::create(args.filepath)?;
            // TODO this won't be great for large files
            write!(output, "{toml}")?;
            println!("{}", format_toml_item(original, args.output));
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
        let toml = r#"[testcases]
        fruits = [ "tomato", "plum", "pluot", "kumquat", "persimmon" ]
        numbers = [1, 3, 5, 7, 11, 13, 17, 23]
        # food not algorithms
        [testcases.hashes]
        color = "brown"
        # I want some now!
        favorite = "Hobees DeAnza"
        mats = [ "potatoes", "salt", "oil", "frying" ]"#;
        let mut doc = toml
            .parse::<Document>()
            .expect("test doc should be valid ???");

        let key = Keyspec::from_str("testcases.hashes.color").unwrap();
        let item = get_dotted_key(&mut doc, &key).expect("expected to get key 'hashes.color'");
        assert_eq!("brown", format_toml_item(item.clone(), OutputMode::Raw));
        assert_eq!("\"brown\"", format_toml_item(item, OutputMode::Toml));

        let key = Keyspec::from_str("testcases.hashes.mats[1]").unwrap();
        let item = get_dotted_key(&mut doc, &key).expect("expected this key to be valid");
        assert_eq!("salt", format_toml_item(item, OutputMode::Raw));
    }

    #[test]
    fn set() {
        let toml = r#"[testcases]
    fruits = [ "tomato", "plum", "pluot", "kumquat", "persimmon" ]
    numbers = [1, 3, 5, 7, 11, 13, 17, 23]
    # food not algorithms
    [testcases.hashes]
    color = "brown"
    # I want some now!
    favorite = "Hobees DeAnza"
    mats = [ "potatoes", "salt", "oil", "frying" ]"#;
        let mut doc = toml
            .parse::<Document>()
            .expect("test doc should be valid ???");

        let key = Keyspec::from_str("testcases.hashes.color").expect("test key should be valid");
        let item =
            set_dotted_key(&mut doc, &key, "taupe").expect("expected to find key 'hashes.color'");
        assert_eq!("brown", format_toml_item(item, OutputMode::Raw));
        assert!(doc.to_string().contains("color = \"taupe\""));

        let key =
            Keyspec::from_str("testcases.hashes.mats[3]").expect("expected this key to be valid");
        let item = set_dotted_key(&mut doc, &key, "bacon").expect("could not find this key");
        assert_eq!("frying", format_toml_item(item, OutputMode::Raw));
        assert!(doc.to_string().contains("bacon"));
    }

    #[test]
    fn yeet() {
        let toml = r#"[testcases]
    fruits = [ "tomato", "plum", "pluot", "kumquat", "persimmon" ]
    numbers = [1, 3, 5, 7, 11, 13, 17, 23]
    # food not algorithms
    [testcases.hashes]
    color = "brown"
    # I want some now!
    favorite = "Hobees DeAnza"
    mats = [ "potatoes", "salt", "oil", "frying" ]"#;
        let mut doc = toml
            .parse::<Document>()
            .expect("test doc should be valid ???");

        let key = Keyspec::from_str("testcases.hashes.color").unwrap();
        let item = remove_dotted_key(&mut doc, &key).expect("expected to find key 'hashes.color'");
        assert_eq!("brown", format_toml_item(item, OutputMode::Raw));
        assert!(!doc.to_string().contains("color = \"brown\""));

        let key = Keyspec::from_str("testcases.hashes.mats[1]").unwrap();
        let item = remove_dotted_key(&mut doc, &key).expect("could not find this key");
        assert_eq!("salt", format_toml_item(item, OutputMode::Raw));
        assert!(doc
            .to_string()
            .contains(r#"mats = [ "potatoes", "oil", "frying" ]"#));
    }

    #[test]
    fn bash_ouput() {}

    #[test]
    fn json_output() {}

    #[test]
    fn toml_output() {}
}
