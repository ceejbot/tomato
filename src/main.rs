use clap::{Parser, Subcommand};
use clap_complete::{generate, Shell};
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::str::FromStr;
use toml_edit::{Document, Item, Value};

mod json;
use json::format_json;
mod bash;
use bash::format_bash;
mod keys;
use keys::*;

#[derive(Parser, Debug)]
#[clap(name = "🍅 tomato", version)]
/// A command-line tool to get and set values in toml files while preserving comments and
/// formatting.
///
/// Keys are written using `.` to separate path segments. You can use `array[idx]` syntax to index
/// into arrays if you want to. For example, to get the name of the current crate you're working on,
/// you'd run `tomato get Cargo.toml package.name`.
///
/// By default tomato emits data in a form suitable for immediate use in bash scripts if they are
/// primitive values: strings are unquoted, for instance. If you want to use more complex data
/// types, consider one of the other output formats.
///
/// To read from stdin instead of a file, pass '-' as the filename. Operating on stdin changes
/// the behavior of set and rm somewhat, under the assumption that you are using this tool in
/// a shell script. If you read from stdin, normal output (the old value) is suppressed. Instead
/// the modified file is written to stdout in json if you requested json, toml otherwise.
/// The 'bash' format option is ignored.
pub struct Args {
    /// How to format the output: json, toml, bash, or raw
    #[clap(short, long, default_value = "raw")]
    format: Format,
    /// Back up the file to <filepath>.bak if we write a new version. This option is ignored
    /// when we're operating on stdin.
    #[clap(long, short)]
    backup: bool,
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    /// Get the value of a key from the given file
    #[clap(display_order = 1)]
    Get {
        /// The toml file to read from. Pass '-' to read from stdin.
        filepath: String,
        /// The key to look for. Use dots as path separators.
        key: Keyspec,
    },
    /// Set a key to the given value, returning the previous value if one existed.
    #[clap(display_order = 2)]
    Set {
        /// The toml file to read from. Pass '-' to read from stdin. If you read from stdin,
        /// the normal output of the old value is suppressed. Instead the modified file is written
        /// to stdout in json if you requested json, toml otherwise.
        filepath: String,
        /// The key to set a value for. Use dots as path separators.
        key: Keyspec,
        /// The new value.
        value: String,
    },
    /// Delete a key from the given file, returning the previous value if one existed
    #[clap(aliases = &["del", "delete", "delet", "forget", "regret", "remove", "unset", "yank", "yeet"], display_order=3)]
    Rm {
        /// The toml file to read from. Pass '-' to read from stdin. If you read from stdin,
        /// the normal output of the old value is suppressed. Instead the modified file is written
        /// to stdout in json if you requested json, toml otherwise.
        filepath: String,
        /// The key to remove from the file. Use dots as path separators.
        key: Keyspec,
    },
    /// Generate completions for the named shell.
    #[clap(display_order = 4)]
    Completions {
        #[clap(arg_enum)]
        shell: Shell,
    },
}

#[derive(Clone, Debug)]
/// How to format the output of more complex data structures.
pub enum Format {
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
    let mut data = String::new();
    match fpath {
        "-" => {
            let mut reader = BufReader::new(std::io::stdin());
            reader.read_to_string(&mut data)?;
        }
        _ => {
            let file = File::open(fpath)?;
            let mut reader = BufReader::new(file);
            reader.read_to_string(&mut data)?;
        }
    };
    let parsed = data
        .parse::<Document>()
        .unwrap_or_else(|_| panic!("{}", format!("The file {} is not valid toml.", fpath)));

    Ok(parsed)
}

pub fn write_file(toml: &Document, fpath: &str, backup: bool) -> anyhow::Result<(), anyhow::Error> {
    if backup {
        std::fs::copy(fpath, format!("{}.bak", fpath))?;
    }
    let mut output = File::create(fpath)?;
    // Note for future work: this won't be great for large files
    write!(output, "{toml}")?;
    Ok(())
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
/// Replaces null nodes if the parent was found, adding a new key to the
/// document. Reponds with an error if the key included an index into an
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

    match args.cmd {
        Command::Get { filepath, key } => {
            let mut toml = parse_file(&filepath)?;
            let item = get_key(&mut toml, &key)?;
            println!("{}", format_item(&item, args.format));
        }
        Command::Rm { filepath, key } => {
            let mut toml = parse_file(&filepath)?;
            let original = remove_key(&mut toml, &key)?;
            match filepath.as_str() {
                "-" => {
                    match args.format {
                        Format::Json => println!("{}", format_item(toml.as_item(), args.format)),
                        _ => println!("{toml}"),
                    };
                }
                _ => {
                    write_file(&toml, &filepath, args.backup)?;
                    println!("{}", format_item(&original, args.format));
                }
            }
        }
        Command::Set {
            filepath,
            key,
            value,
        } => {
            let mut toml = parse_file(&filepath)?;
            let original = set_key(&mut toml, &key, &value)?;
            match filepath.as_str() {
                "-" => {
                    match args.format {
                        Format::Json => println!("{}", format_item(toml.as_item(), args.format)),
                        _ => println!("{toml}"),
                    };
                }
                _ => {
                    write_file(&toml, &filepath, args.backup)?;
                    println!("{}", format_item(&original, args.format));
                }
            }
        }
        Command::Completions { shell } => {
            use clap::CommandFactory;
            let mut app = Args::command();
            generate(shell, &mut app, "tomato", &mut std::io::stdout())
        }
    };

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
