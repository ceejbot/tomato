use std::str::FromStr;

use clap::builder::Styles;
use clap::builder::styling::AnsiColor;
use clap::{Parser, Subcommand};
use clap_complete::{Shell, generate};
use toml_edit::{DocumentMut, Item, Value};

use crate::errors::TomatoError;
use crate::format::{Format, format_item, format_keys};
use crate::ops::{append_value, get_key, list_keys, parse_file, remove_key, set_key, write_file};
use crate::parser::Keyspec;

mod bash;
mod errors;
mod format;
mod json;
mod ops;
mod parser;

#[derive(Parser, Debug)]
#[clap(name = "🍅 tomato", version, styles = v3_styles(), max_term_width=100)]
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
struct Args {
    /// How to format the output: json, toml, bash, or raw
    #[clap(short, long, global = true, default_value = "raw")]
    format: Format,
    /// Back up the file to `<filepath>.bak` if we write a new version. This option
    /// is ignored when we're operating on stdin.
    #[clap(long, short, global = true)]
    backup: bool,
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Clone, Debug, Subcommand)]
enum Command {
    /// Get the value of a key from the given file
    #[clap(display_order = 1)]
    Get {
        /// The key to look for. Use dots as path separators.
        key: String,
        /// The toml file to read from. Omit to read from stdin.
        file: Option<String>,
    },
    /// Set a key to the given value, returning the previous value if one existed.
    #[clap(display_order = 2)]
    Set {
        /// The key to set a value for. Use dots as path separators.
        key: String,
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
        key: String,
        /// The toml file to read from. Omit to read from stdin. If you read from stdin,
        /// the normal output of the old value is suppressed. Instead the modified file is written
        /// to stdout in json if you requested json, toml otherwise.
        file: Option<String>,
    },
    /// Append the given value to an array, returning the previous array if one existed.
    #[clap(display_order = 4)]
    Append {
        /// The full key path of the array you want to append to.
        key: String,
        /// The value to append to the array.
        value: String,
        /// The toml file to modify. Omit to read from stdin.
        file: Option<String>,
    },
    /// Exit with status code zero if the key exists in the input file, non-zero if not.
    #[clap(display_order = 5)]
    Exists {
        /// The key to check the existence of.
        key: String,
        /// The toml file to read from. Omit to read from stdin.
        file: Option<String>,
    },
    /// List all keys at a given path, if the value type has sub-keys.
    #[clap(display_order = 6)]
    Keys {
        /// The key to list subkeys for.
        key: String,
        /// The toml file to read from. Omit to read from stdin.
        file: Option<String>,
    },
    /// Generate completions for the named shell.
    Completions {
        #[clap(value_enum)]
        shell: Shell,
    },
}

// A wrapper around toml_edit values to allow us to distinguish between `"true"`
// (a string) and `true` (a boolean) as command-line arguments.
#[derive(Debug, Clone)]
struct TomlVal {
    inner: Value,
}

impl FromStr for TomlVal {
    type Err = TomatoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // A matching pair of surrounding quotes needs at least two characters; a lone
        // `"` or `'` is not a quoted-empty-string, it's a literal one-character value.
        // Guarding on the length keeps the `s[1..s.len() - 1]` slice below from panicking.
        let is_quoted = |q: char| s.len() >= 2 && s.starts_with(q) && s.ends_with(q);
        let inner = if is_quoted('"') || is_quoted('\'') {
            // Extract quoted string content
            let content = &s[1..s.len() - 1];
            content.into()
        } else if s == "true" {
            Value::from(true)
        } else if s == "false" {
            Value::from(false)
        } else if let Ok(v) = i64::from_str(s) {
            Value::from(v)
        } else if let Ok(v) = f64::from_str(s) {
            Value::from(v)
        } else {
            s.into()
        };

        Ok(TomlVal { inner })
    }
}

fn v3_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Yellow.on_default())
        .usage(AnsiColor::Green.on_default())
        .literal(AnsiColor::Green.on_default())
        .placeholder(AnsiColor::Green.on_default())
}

/// Emit the result of a mutating command (set/rm/append). When reading from stdin
/// (`file` is `None`) we print the whole modified document so it can be piped onward;
/// when writing to a file we persist it (optionally backing it up first) and print the
/// previous value instead.
fn emit(
    toml: &DocumentMut,
    original: &Item,
    file: Option<String>,
    format: Format,
    backup: bool,
) -> Result<(), TomatoError> {
    match file {
        None => match format {
            Format::Json => println!("{}", format_item(toml.as_item(), format)),
            _ => println!("{toml}"),
        },
        Some(filepath) => {
            write_file(toml, &filepath, backup)?;
            println!("{}", format_item(original, format));
        }
    }
    Ok(())
}

/// Parse command-line args and do whatever our user wants!
fn main() -> miette::Result<()> {
    let args = Args::parse();

    match args.cmd {
        Command::Get { key, file } => {
            let key: Keyspec = key.parse()?;
            let mut toml = parse_file(file.as_ref())?;
            let item = get_key(&mut toml, &key)?;
            println!("{}", format_item(&item, args.format));
        }
        Command::Rm { key, file } => {
            let key: Keyspec = key.parse()?;
            let mut toml = parse_file(file.as_ref())?;
            let original = remove_key(&mut toml, &key)?;
            emit(&toml, &original, file, args.format, args.backup)?;
        }
        Command::Set { key, value, file } => {
            let key: Keyspec = key.parse()?;
            let mut toml = parse_file(file.as_ref())?;
            let original = set_key(&mut toml, &key, &value.inner)?;
            emit(&toml, &original, file, args.format, args.backup)?;
        }
        Command::Append { key, value, file } => {
            let key: Keyspec = key.parse()?;
            let mut toml = parse_file(file.as_ref())?;
            let original = append_value(&mut toml, &key, &value)?;
            emit(&toml, &original, file, args.format, args.backup)?;
        }
        Command::Exists { key, file } => {
            let key: Keyspec = key.parse()?;
            let mut toml = parse_file(file.as_ref())?;
            match get_key(&mut toml, &key) {
                Ok(_) => {} // found, fall through and exit with Ok below
                Err(e) => {
                    match e {
                        // For key not found errors, we exit quietly with a non-zero exit code,
                        // bypassing miette's processing.
                        TomatoError::KeyNotFound { .. } => {
                            std::process::exit(1);
                        }
                        // For all other errors, we print the same error report as usual.
                        _ => {
                            Err(e)?;
                        }
                    }
                }
            }
        }
        Command::Keys { key, file } => {
            let key: Keyspec = key.parse()?;
            let mut toml = parse_file(file.as_ref())?;
            let parent = get_key(&mut toml, &key)?;
            let keys = list_keys(&parent, &key)?;
            println!("{}", format_keys(&keys, args.format));
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
    fn verify_cli() {
        use clap::CommandFactory;
        Args::command().debug_assert();
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
                unreachable!("should have been a string");
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
                unreachable!("should have been a string");
            }
        }

        let unquoted = "false";
        let tval2 = TomlVal::from_str(unquoted).expect("conversion should work");
        match tval2.inner {
            Value::Boolean(b) => {
                assert!(!*b.value());
            }
            _ => {
                eprintln!("{:?}", tval2.inner);
                unreachable!("should have been a boolean");
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
                unreachable!("should have been a string");
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
                unreachable!("should have been an integer");
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
                unreachable!("should have been an integer");
            }
        }
    }

    #[test]
    fn tomlval_parser_handles_lone_quotes() {
        // A single quote character must not be mistaken for an empty quoted string:
        // it should round-trip as a literal one-character value, and must never panic.
        for lone in ["\"", "'"] {
            let tval = TomlVal::from_str(lone).expect("a lone quote should parse as a literal string");
            match tval.inner {
                Value::String(s) => assert_eq!(*s.value(), lone),
                _ => unreachable!("a lone quote should be a string, got {:?}", tval.inner),
            }
        }
    }
}
