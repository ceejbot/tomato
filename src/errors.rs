use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum TomatoError {
    #[error("Unsupported output type: {0}")]
    UnsupportedOutputType(String),

    #[error("remove: must specify a key to remove!")]
    NoKeyToRemove,

    #[error("key {0} not found in toml file")]
    KeyNotFound(String),

    #[error("unable to index into non-array at {0}")]
    CannotIndexIntoNonArray(String),

    #[error("unable to append to a non-array at {0}")]
    CannotAppendToNonArray(String),

    #[error("{0} is not a valid key segment for tomato")]
    InvalidKeySegment(String),

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
