use miette::Diagnostic;
use thiserror::Error;
use toml_edit::{Item, Value};

#[derive(Debug, Error, Diagnostic)]
pub enum TomatoError {
    #[error("Unsupported output format '{format}'")]
    #[diagnostic(code(tomato::unsupported_format), help("Valid formats are: raw, json, toml, bash"))]
    UnsupportedOutputType { format: String },

    #[error("No key specified for removal")]
    #[diagnostic(
        code(tomato::no_key),
        help("Specify a key to remove, e.g., 'tomato rm package.name file.toml'")
    )]
    NoKeyToRemove,

    #[error("Key '{key}' not found in TOML file")]
    #[diagnostic(
        code(tomato::key_not_found),
        help("Check that the key exists and is spelled correctly. Use 'tomato get' to explore available keys.")
    )]
    KeyNotFound { key: String },

    #[error("Cannot access property '{property}' on {value_type} value")]
    #[diagnostic(
        code(tomato::property_on_primitive),
        help(
            "Only tables and inline tables have named properties. {value_type} values don't have properties to access."
        )
    )]
    PropertyOnPrimitive { property: String, value_type: String },

    #[error("Cannot index into non-array at '{key}'")]
    #[diagnostic(
        code(tomato::not_an_array),
        help(
            "The key '{key}' refers to {} {value_type}, not an array. Remove the array index notation.",
            indefinite_article_for(value_type)
        )
    )]
    CannotIndexIntoNonArray { key: String, value_type: String },

    #[error("Cannot append to non-array at '{key}'")]
    #[diagnostic(
        code(tomato::cannot_append),
        help(
            "The value at key '{key}' is {} {value_type}, not an array. Use 'set' to replace the value instead.",
            indefinite_article_for(value_type)
        )
    )]
    CannotAppendToNonArray { key: String, value_type: String },

    #[error("Cannot list keys on {value_type} at '{key}'")]
    #[diagnostic(
        code(tomato::not_a_table),
        help(
            "The value at '{key}' is {} {value_type}, not a table or inline table. Only tables have keys to list.",
            indefinite_article_for(value_type)
        )
    )]
    CannotListKeysOnNonTable { key: String, value_type: String },

    #[error("Invalid key syntax: {message}")]
    #[diagnostic(
        code(tomato::invalid_key),
        help("Check your key syntax. Keys can be bare (abc), quoted (\"a.b.c\"), or use array indices (arr[0])")
    )]
    InvalidKeySegment { message: String, help_text: Option<String> },

    #[error("Array index {index} is out of bounds")]
    #[diagnostic(
        code(tomato::array_bounds),
        help(
            "The array has {length} element{plural}. Valid indices are {valid_range} (or negative indices from -1 to -{length})"
        )
    )]
    ArrayIndexOutOfBounds {
        index: isize,
        length: usize,
        valid_range: String,
        plural: String,
    },

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    TomlError(#[from] toml_edit::TomlError),
}

fn indefinite_article_for(input: &str) -> &str {
    match input {
        "inline table" => "an",
        "array" => "an",
        "array of tables" => "an",
        _ => "a",
    }
}

pub trait DisplayInfo {
    fn is_primitive(&self) -> bool;
    fn type_str(&self) -> &str;
}

impl DisplayInfo for &Item {
    fn is_primitive(&self) -> bool {
        match self {
            Item::Value(v) => match v {
                Value::InlineTable(_) => false,
                Value::String(_) => true,
                Value::Integer(_) => true,
                Value::Float(_) => true,
                Value::Boolean(_) => true,
                Value::Datetime(_) => true,
                Value::Array(_) => true,
            },
            Item::Table(_) => false,
            Item::ArrayOfTables(_) => false,
            Item::None => false,
        }
    }

    fn type_str(&self) -> &str {
        match self {
            Item::Value(v) => match v {
                Value::InlineTable(_) => "inline table",
                Value::String(_) => "string",
                Value::Integer(_) => "integer",
                Value::Float(_) => "float",
                Value::Boolean(_) => "boolean",
                Value::Datetime(_) => "date-time",
                Value::Array(_) => "array",
            },
            Item::Table(_) => "table",
            Item::ArrayOfTables(_) => "array of tables",
            Item::None => "null",
        }
    }
}
