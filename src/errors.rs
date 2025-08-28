use miette::Diagnostic;
use thiserror::Error;

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
    KeyNotFound { key: String, suggestion: Option<String> },

    #[error("Cannot access property '{property}' on {actual_type} value")]
    #[diagnostic(
        code(tomato::property_on_primitive),
        help("Only tables and inline tables have named properties. {actual_type} values don't have properties to access.")
    )]
    PropertyOnPrimitive { property: String, actual_type: String },

    #[error("Cannot index into non-array at '{key}'")]
    #[diagnostic(
        code(tomato::not_an_array),
        help("The key '{key}' refers to a {actual_type}, not an array. Remove the array index notation.")
    )]
    CannotIndexIntoNonArray { key: String, actual_type: String },

    #[error("Cannot append to non-array at '{key}'")]
    #[diagnostic(
        code(tomato::cannot_append),
        help("The key '{key}' is a {actual_type}, not an array. Use 'set' to replace the value instead.")
    )]
    CannotAppendToNonArray { key: String, actual_type: String },

    #[error("Invalid key syntax: {message}")]
    #[diagnostic(
        code(tomato::invalid_key),
        help("Check your key syntax. Keys can be bare (abc), quoted (\"a.b.c\"), or use array indices (arr[0])")
    )]
    InvalidKeySegment { message: String, help_text: Option<String> },

    #[error("Array index {index} is out of bounds")]
    #[diagnostic(
        code(tomato::array_bounds),
        help("The array has {length} element{plural}. Valid indices are {valid_range} (or negative indices from -1 to -{length})")
    )]
    ArrayIndexOutOfBounds {
        index: isize,
        length: usize,
        valid_range: String,
        plural: String,
    },

    #[error("Quoted string regex cell is unset")]
    QuotedStringCellUnset,

    #[error("Array regex cell is unset")]
    ArrayRegexCellUnset,

    #[error("File is not valid TOML: {0:#?}")]
    InvalidToml(String),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    TomlError(#[from] toml_edit::TomlError),

    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),
}
