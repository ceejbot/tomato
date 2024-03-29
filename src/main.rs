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
/// To read from stdin instead of a file, omit the file argument. Operating on stdin changes
/// the behavior of set and rm somewhat, under the assumption that you are using this tool in
/// a shell script. If you read from stdin, normal output (the old value) is suppressed. Instead
/// the modified file is written to stdout in json if you requested json, toml otherwise.
/// The 'bash' format option is ignored.
pub struct Args {
    /// How to format the output: json, toml, bash, or raw
    #[clap(short, long, global = true, default_value = "raw")]
    format: Format,
    /// Back up the file to <filepath>.bak if we write a new version. This option
    /// is ignored when we're operating on stdin.
    #[clap(long, short, global = true)]
    backup: bool,
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    /// Get the value of a key from the given file
    #[clap(display_order = 1)]
    Get {
        /// The key to look for. Use dots as path separators.
        key: Keyspec,
        /// The toml file to read from. Omit to read from stdin.
        file: Option<String>,
    },
    /// Set a key to the given value, returning the previous value if one existed.
    #[clap(display_order = 2)]
    Set {
        /// The key to set a value for. Use dots as path separators.
        key: Keyspec,
        /// The new value.
        value: TomlVal,
        /// The toml file to read from. Omit to read from stdin. If you read from stdin,
        /// the normal output of the old value is suppressed. Instead the modified file is written
        /// to stdout in json if you requested json, toml otherwise.
        file: Option<String>,
    },
    /// Delete a key from the given file, returning the previous value if one existed
    #[clap(aliases = &["del", "delete", "delet", "forget", "regret", "remove", "unset", "yank", "yeet"], display_order=3)]
    Rm {
        /// The key to remove from the file. Use dots as path separators.
        key: Keyspec,
        /// The toml file to read from. Omit to read from stdin. If you read from stdin,
        /// the normal output of the old value is suppressed. Instead the modified file is written
        /// to stdout in json if you requested json, toml otherwise.
        file: Option<String>,
    },
    /// Append the given value to an array, returning the previous array if one existed.
    #[clap(display_order = 1)]
    Append {
        /// The key to look for. Use dots as path separators. Must
        key: Keyspec,
        /// The new value.
        value: String,
        /// The toml file to read from. Omit to read from stdin.
        file: Option<String>,
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

// A wrapper around toml_edit values to allow us to distinguish between `"true"`
// (a string) and `true` (a boolean) as command-line arguments.
#[derive(Debug, Clone)]
pub struct TomlVal {
    inner: Value,
}

impl FromStr for TomlVal {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let quoted_string = regex::Regex::new(r#"^"(.+)"|'(.+)'$"#).unwrap();
        let inner = if let Some(captures) = quoted_string.captures(s) {
            let core = if let Some(_c) = captures.get(1) {
                captures[1].to_string()
            } else if let Some(_c) = captures.get(2) {
                captures[2].to_string()
            } else {
                s.to_string()
            };
            core.into()
        } else if s == "true" {
            Value::try_from(true).unwrap()
        } else if s == "false" {
            Value::try_from(false).unwrap()
        } else if let Ok(v) = i64::from_str(s) {
            Value::try_from(v).unwrap()
        } else if let Ok(v) = f64::from_str(s) {
            Value::try_from(v).unwrap()
        } else {
            s.into()
        };

        Ok(TomlVal { inner })
    }
}

/// Read the toml file and parse it. Respond with an error that gets propagated up
/// if the file is not valid toml.
pub fn parse_file(maybepath: Option<&String>) -> anyhow::Result<Document, anyhow::Error> {
    let mut data = String::new();
    if let Some(ref fpath) = maybepath {
        let file = File::open(fpath)?;
        let mut reader = BufReader::new(file);
        reader.read_to_string(&mut data)?;
    } else {
        let mut reader = BufReader::new(std::io::stdin());
        reader.read_to_string(&mut data)?;
    }
    let parsed = data
        .parse::<Document>()
        .unwrap_or_else(|_| panic!("{}", format!("The file {:?} is not valid toml.", maybepath)));

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
/// document. Responds with an error if the key included an index into an
/// array for a non-array node in the document.
pub fn set_key(
    toml: &mut Document,
    dotted_key: &Keyspec,
    value: &Value,
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

/// Append the given value to the array at the given key and respond with
/// the original array value.
/// Replaces null nodes if the parent was found, adding a new key to the
/// document. Responds with an error if the key exists and is not an array
/// or if the key included an index into an array for a non-array node in
/// the document.
pub fn append_value(
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

    node.or_insert(Item::Value(Value::Array(toml_edit::Array::new())))
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("unable to append to a non-array at {}", dotted_key))?
        .push(value);

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
        Command::Get { key, file } => {
            let mut toml = parse_file(file.as_ref())?;
            let item = get_key(&mut toml, &key)?;
            println!("{}", format_item(&item, args.format));
        }
        Command::Rm { key, file } => {
            let mut toml = parse_file(file.as_ref())?;
            let original = remove_key(&mut toml, &key)?;
            match file {
                None => {
                    match args.format {
                        Format::Json => println!("{}", format_item(toml.as_item(), args.format)),
                        _ => println!("{toml}"),
                    };
                }
                Some(filepath) => {
                    write_file(&toml, &filepath, args.backup)?;
                    println!("{}", format_item(&original, args.format));
                }
            }
        }
        Command::Set { key, value, file } => {
            let mut toml = parse_file(file.as_ref())?;
            let inner = value.inner;
            let original = set_key(&mut toml, &key, &inner)?;
            match file {
                None => {
                    match args.format {
                        Format::Json => println!("{}", format_item(toml.as_item(), args.format)),
                        _ => println!("{toml}"),
                    };
                }
                Some(filepath) => {
                    write_file(&toml, &filepath, args.backup)?;
                    println!("{}", format_item(&original, args.format));
                }
            }
        }
        Command::Append { key, value, file } => {
            let mut toml = parse_file(file.as_ref())?;
            let original = append_value(&mut toml, &key, &value)?;
            match file {
                None => {
                    match args.format {
                        Format::Json => println!("{}", format_item(toml.as_item(), args.format)),
                        _ => println!("{toml}"),
                    };
                }
                Some(filepath) => {
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
        let taupe = Value::from("taupe");
        let item = set_key(&mut doc, &key, &taupe).expect("expected to find key 'hashes.color'");
        assert_eq!("brown", format_item(&item, Format::Raw));
        assert!(doc.to_string().contains("color = \"taupe\""));

        let key =
            Keyspec::from_str("testcases.hashes.mats[3]").expect("expected this key to be valid");
        let bacon = Value::from("bacon");
        let item = set_key(&mut doc, &key, &bacon).expect("could not find this key");
        assert_eq!("frying", format_item(&item, Format::Raw));
        assert!(doc.to_string().contains("bacon"));
    }

    #[test]
    fn append() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml
            .parse::<Document>()
            .expect("test doc should be valid toml");

        let key = Keyspec::from_str("testcases.fruits").expect("test key should be valid");
        let item = append_value(&mut doc, &key, "orange")
            .expect("expected to be able to insert value 'orange'");
        let formatted = format_toml(&item);
        assert_eq!(
            formatted,
            r#"[ "tomato", "plum", "pluot", "kumquat", "persimmon" ]"#
        );
        assert!(doc.to_string().contains(
            r#"fruits = [ "tomato", "plum", "pluot", "kumquat", "persimmon" , "orange"]"#
        ));
    }

    #[test]
    fn append_to_non_existing_key_creates_array() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml
            .parse::<Document>()
            .expect("test doc should be valid toml");

        let key =
            Keyspec::from_str("testcases.these.are.not.fruits").expect("test key should be valid");
        let item = append_value(&mut doc, &key, "leek")
            .expect("expected to be able to insert value 'leek'");
        assert!(item.is_none());
        assert!(doc
            .to_string()
            .contains(r#"these = { are = { not = { fruits = ["leek"] } } }"#));

        let item = append_value(&mut doc, &key, "artichoke")
            .expect("expected to be able to insert value 'artichoke'");
        assert_eq!(format_toml(&item), r#"["leek"]"#);
        assert!(doc
            .to_string()
            .contains(r#"these = { are = { not = { fruits = ["leek", "artichoke"] } } }"#));

        let key = Keyspec::from_str("testcases.these.are.maybe.fruits")
            .expect("test key should be valid");
        let item = append_value(&mut doc, &key, "banana")
            .expect("expected to be able to insert value 'banana'");
        eprintln!("{}", doc.to_string());
        assert!(item.is_none());
        assert!(doc
            .to_string()
            .contains(r#"these = { are = { not = { fruits = ["leek", "artichoke"] }, maybe = { fruits = ["banana"] } } }"#));
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

    #[test]
    fn tomlval_parser_handles_booleans() {
        let quoted = r#""false""#;
        let tval = TomlVal::from_str(quoted).expect("conversion should work");
        match tval.inner {
            Value::String(s) => {
                assert_eq!(*s.value(), "false");
            }
            _ => {
                eprintln!("{:?}", tval.inner);
                assert!(false, "should have been a string");
            }
        }

        let singlequoted = "'true'";
        let tval = TomlVal::from_str(singlequoted).expect("conversion should work");
        match tval.inner {
            Value::String(s) => {
                assert_eq!(*s.value(), "true");
            }
            _ => {
                eprintln!("{:?}", tval.inner);
                assert!(false, "should have been a string");
            }
        }

        let unquoted = "false";
        let tval2 = TomlVal::from_str(unquoted).expect("conversion should work");
        match tval2.inner {
            Value::Boolean(b) => {
                assert_eq!(*b.value(), false);
            }
            _ => {
                eprintln!("{:?}", tval2.inner);
                assert!(false, "should have been a boolean");
            }
        }
    }

    #[test]
    fn tomlval_parser_handles_numbers() {
        let quoted = r#""1""#;
        let tval = TomlVal::from_str(quoted).expect("conversion should work");
        match tval.inner {
            Value::String(s) => {
                assert_eq!(*s.value(), "1");
            }
            _ => {
                eprintln!("{:?}", tval.inner);
                assert!(false, "should have been a string");
            }
        }

        let inty = "1";
        let tval2 = TomlVal::from_str(inty).expect("conversion should work");
        match tval2.inner {
            Value::Integer(n) => {
                assert_eq!(*n.value(), 1);
            }
            _ => {
                eprintln!("{:?}", tval2.inner);
                assert!(false, "should have been an integer");
            }
        }

        let floaty = "1.5";
        let floatyval = TomlVal::from_str(floaty).expect("conversion should work");
        match floatyval.inner {
            Value::Float(n) => {
                assert_eq!(*n.value(), 1.5);
            }
            _ => {
                eprintln!("{:?}", floatyval.inner);
                assert!(false, "should have been an integer");
            }
        }
    }

    #[test]
    fn can_set_booleans() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml
            .parse::<Document>()
            .expect("test doc should be valid toml");

        let key = Keyspec::from_str("testcases.are_passing").expect("test key should be valid");
        let newval = Value::from(false);
        let previous =
            set_key(&mut doc, &key, &newval).expect("test fixture known to contain the test key");
        let prevval = previous
            .as_value()
            .expect("the previous value should be a valid toml value");
        match prevval {
            Value::Boolean(b) => {
                assert!(*b.value());
            }
            _ => panic!("fetched value was supposed to be a boolean!"),
        }

        let current = get_key(&mut doc, &key).expect("test fixture known to contain the test key");
        let curval = current
            .as_value()
            .expect("the new value should be a valid toml value");
        match curval {
            Value::Boolean(b) => {
                assert_eq!(*b.value(), false);
            }
            _ => panic!("fetched value was supposed to be a boolean!"),
        }
    }
}
