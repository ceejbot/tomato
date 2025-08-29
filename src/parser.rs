use std::fmt::Display;
use std::str::FromStr;

use crate::errors::TomatoError;

#[derive(Clone, Debug, PartialEq, Eq)]
/// Keys can contain either name segments or array indexes (positive or negative).
pub enum KeySegment {
    Name(String),
    Index(isize), // Changed from usize to isize to support negative indices
}

impl Display for KeySegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(s) => write!(f, "{s}"),
            Self::Index(i) => write!(f, "{i}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// An internal representation of the dotted key string given on the command-line.
pub struct Keyspec {
    pub subkeys: Vec<KeySegment>,
}

impl Display for Keyspec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parts: Vec<String> = self
            .subkeys
            .iter()
            .map(|segment| match segment {
                KeySegment::Name(name) => {
                    // If the name contains dots or spaces, we should quote it for display
                    if name.contains('.') || name.contains(' ') || name.contains('-') {
                        format!("\"{}\"", name)
                    } else {
                        name.clone()
                    }
                }
                KeySegment::Index(idx) => format!("[{}]", idx),
            })
            .collect();

        // Combine segments appropriately
        let mut result = String::new();
        for (i, part) in parts.iter().enumerate() {
            if part.starts_with('[') {
                // Array index - append directly to previous segment
                result.push_str(part);
            } else if i == 0 {
                // First segment
                result.push_str(part);
            } else {
                // Regular name segment - add dot separator
                result.push('.');
                result.push_str(part);
            }
        }
        write!(f, "{}", result)
    }
}

/// Tokenizer for key parsing
#[derive(Debug)]
struct Tokenizer {
    input: Vec<char>,
    position: usize,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Name(String),
    Dot,
    LeftBracket,
    RightBracket,
    Number(isize),
    QuotedString(String),
    End,
}

impl Tokenizer {
    fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            position: 0,
        }
    }

    fn current(&self) -> Option<char> {
        self.input.get(self.position).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.current();
        self.position += 1;
        ch
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.position + 1).copied()
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn is_bare_key_char(ch: char) -> bool {
        ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
    }

    fn read_bare_key(&mut self) -> String {
        let mut result = String::new();
        while let Some(ch) = self.current() {
            if Self::is_bare_key_char(ch) {
                result.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        result
    }

    fn read_quoted_string(&mut self, quote_char: char) -> Result<String, TomatoError> {
        // Expect opening quote
        if self.advance() != Some(quote_char) {
            return Err(TomatoError::InvalidKeySegment {
                message: format!("Expected opening {}", quote_char),
                help_text: Some("If you meant to use a literal quote, escape it with a backslash".to_string()),
            });
        }

        let mut result = String::new();
        let mut escaped = false;

        while let Some(ch) = self.current() {
            self.advance();

            if escaped {
                match ch {
                    c if c == quote_char => result.push(c), // Handle the specific quote character
                    '\\' => result.push('\\'),
                    'n' if quote_char == '"' => result.push('\n'), // Only in double quotes
                    't' if quote_char == '"' => result.push('\t'), // Only in double quotes
                    'r' if quote_char == '"' => result.push('\r'), // Only in double quotes
                    _ => {
                        result.push('\\');
                        result.push(ch);
                    }
                }
                escaped = false;
            } else if ch == '\\' && quote_char == '"' {
                // Only double quotes support escaping in TOML
                escaped = true;
            } else if ch == quote_char {
                return Ok(result);
            } else {
                result.push(ch);
            }
        }

        Err(TomatoError::InvalidKeySegment {
            message: "Unterminated quoted string".to_string(),
            help_text: Some(format!("Add a closing {} quote to complete the string", quote_char)),
        })
    }

    fn read_number(&mut self) -> Result<isize, TomatoError> {
        let mut result = String::new();

        // Handle optional negative sign
        if self.current() == Some('-') {
            result.push('-');
            self.advance();
        }

        // Read digits
        while let Some(ch) = self.current() {
            if ch.is_ascii_digit() {
                result.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        result.parse::<isize>().map_err(|_| TomatoError::InvalidKeySegment {
            message: format!("Invalid number: {}", result),
            help_text: Some("Array indices must be integers".to_string()),
        })
    }

    fn next_token(&mut self) -> Result<Token, TomatoError> {
        self.skip_whitespace();

        match self.current() {
            None => Ok(Token::End),
            Some('.') => {
                self.advance();
                Ok(Token::Dot)
            }
            Some('[') => {
                self.advance();
                Ok(Token::LeftBracket)
            }
            Some(']') => {
                self.advance();
                Ok(Token::RightBracket)
            }
            Some('"') => {
                let quoted = self.read_quoted_string('"')?;
                Ok(Token::QuotedString(quoted))
            }
            Some('\'') => {
                let quoted = self.read_quoted_string('\'')?;
                Ok(Token::QuotedString(quoted))
            }
            Some(ch) if ch.is_ascii_digit() || (ch == '-' && self.peek().is_some_and(|p| p.is_ascii_digit())) => {
                let number = self.read_number()?;
                Ok(Token::Number(number))
            }
            Some(ch) if Self::is_bare_key_char(ch) => {
                let name = self.read_bare_key();
                Ok(Token::Name(name))
            }
            Some(ch) => Err(TomatoError::InvalidKeySegment {
                message: format!("Unexpected character: '{}'", ch),
                help_text: Some("Expected a key name, array index, or quoted string".to_string()),
            }),
        }
    }
}

/// Parser for tomato key specifications
struct Parser {
    tokenizer: Tokenizer,
    current_token: Token,
}

impl Parser {
    fn new(input: &str) -> Result<Self, TomatoError> {
        let mut tokenizer = Tokenizer::new(input);
        let current_token = tokenizer.next_token()?;
        Ok(Self {
            tokenizer,
            current_token,
        })
    }

    fn advance(&mut self) -> Result<(), TomatoError> {
        self.current_token = self.tokenizer.next_token()?;
        Ok(())
    }

    fn parse_key_segment(&mut self) -> Result<KeySegment, TomatoError> {
        match &self.current_token {
            Token::Name(name) => {
                let segment = KeySegment::Name(name.clone());
                self.advance()?;
                Ok(segment)
            }
            Token::QuotedString(name) => {
                let segment = KeySegment::Name(name.clone());
                self.advance()?;
                Ok(segment)
            }
            Token::Number(num) => {
                // Numbers outside of brackets are treated as key names (e.g., "123" key)
                let segment = KeySegment::Name(num.to_string());
                self.advance()?;
                Ok(segment)
            }
            _ => Err(TomatoError::InvalidKeySegment {
                message: "Expected key name or number".to_string(),
                help_text: Some(
                    "Valid key segments are: bare names (abc), quoted strings (\"a b\"), or numbers (123)".to_string(),
                ),
            }),
        }
    }

    fn parse_array_index(&mut self) -> Result<KeySegment, TomatoError> {
        // Expect left bracket
        if !matches!(self.current_token, Token::LeftBracket) {
            return Err(TomatoError::InvalidKeySegment {
                message: "Expected '['".to_string(),
                help_text: Some("Array indices must be enclosed in brackets, e.g., array[0]".to_string()),
            });
        }
        self.advance()?;

        // Parse the index
        let index = match &self.current_token {
            Token::Number(num) => *num,
            _ => {
                return Err(TomatoError::InvalidKeySegment {
                    message: "Expected number in array index".to_string(),
                    help_text: Some("Array indices must be integers, e.g., [0], [-1], [42]".to_string()),
                });
            }
        };
        self.advance()?;

        // Expect right bracket
        if !matches!(self.current_token, Token::RightBracket) {
            return Err(TomatoError::InvalidKeySegment {
                message: "Expected ']'".to_string(),
                help_text: Some("Array indices must be properly closed with ']'".to_string()),
            });
        }
        self.advance()?;

        Ok(KeySegment::Index(index))
    }

    fn parse(&mut self) -> Result<Keyspec, TomatoError> {
        let mut subkeys = Vec::new();

        // Parse first segment (required)
        if matches!(self.current_token, Token::End) {
            return Err(TomatoError::InvalidKeySegment {
                message: "Empty key specification".to_string(),
                help_text: Some("Provide a key name, e.g., 'package.name' or 'array[0]'".to_string()),
            });
        }

        subkeys.push(self.parse_key_segment()?);

        // Parse optional array indices and additional segments
        loop {
            match &self.current_token {
                Token::LeftBracket => {
                    subkeys.push(self.parse_array_index()?);
                }
                Token::Dot => {
                    self.advance()?;
                    subkeys.push(self.parse_key_segment()?);
                }
                Token::End => break,
                _ => {
                    return Err(TomatoError::InvalidKeySegment {
                        message: format!("Unexpected token: {:?}", self.current_token),
                        help_text: Some("Expected '.', '[index]', or end of key specification".to_string()),
                    });
                }
            }
        }

        Ok(Keyspec { subkeys })
    }
}

impl FromStr for Keyspec {
    type Err = TomatoError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let mut parser = Parser::new(input)?;
        parser.parse()
    }
}

/// Convert a negative array index to positive index given array length
pub fn resolve_negative_index(index: isize, array_length: usize) -> Option<usize> {
    if index >= 0 {
        let idx = index as usize;
        if idx < array_length { Some(idx) } else { None }
    } else {
        let abs_index = (-index) as usize;
        if abs_index <= array_length {
            Some(array_length - abs_index)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_key() {
        let key = Keyspec::from_str("simple").expect("Should parse simple key");
        assert_eq!(key.subkeys.len(), 1);
        assert_eq!(key.subkeys[0], KeySegment::Name("simple".to_string()));
    }

    #[test]
    fn dotted_key() {
        let key = Keyspec::from_str("a.b.c").expect("Should parse dotted key");
        assert_eq!(key.subkeys.len(), 3);
        assert_eq!(key.subkeys[0], KeySegment::Name("a".to_string()));
        assert_eq!(key.subkeys[1], KeySegment::Name("b".to_string()));
        assert_eq!(key.subkeys[2], KeySegment::Name("c".to_string()));
    }

    #[test]
    fn days_since_last_shell_quoting_incident() {
        let key0 = Keyspec::from_str("\"zero\"").expect("should parse double-quoted key");
        assert_eq!(key0.subkeys.len(), 1);
        assert_eq!(key0.subkeys[0], KeySegment::Name("zero".to_string()));

        let key1 = Keyspec::from_str("'zero'").expect("should parse single-quoted key");
        assert_eq!(key1.subkeys[0], KeySegment::Name("zero".to_string()));

        let key2 = Keyspec::from_str("zero").expect("Bare 'zero' should be a valid key");
        assert_eq!(key2.subkeys[0], KeySegment::Name("zero".to_string()));

        assert_eq!(key0, key1, "quoting style should matter not");
        assert_eq!(key1, key2, "quoting style should matter not");

        let key = Keyspec::from_str(r#""quoted key""#).expect("Should parse double-quoted key");
        assert_eq!(key.subkeys.len(), 1);
        assert_eq!(key.subkeys[0], KeySegment::Name("quoted key".to_string()));
    }

    #[test]
    fn positive_array_indices() {
        let key = Keyspec::from_str("array[42]").expect("Should parse array index");
        assert_eq!(key.subkeys.len(), 2);
        assert_eq!(key.subkeys[0], KeySegment::Name("array".to_string()));
        assert_eq!(key.subkeys[1], KeySegment::Index(42));
    }

    #[test]
    fn negative_array_indices() {
        let key = Keyspec::from_str("array[-1]").expect("Should parse negative array index");
        assert_eq!(key.subkeys.len(), 2);
        assert_eq!(key.subkeys[0], KeySegment::Name("array".to_string()));
        assert_eq!(key.subkeys[1], KeySegment::Index(-1));
    }

    #[test]
    fn negative_index_resolver() {
        // Array length 5: [0, 1, 2, 3, 4]
        assert_eq!(resolve_negative_index(-1, 5), Some(4));
        assert_eq!(resolve_negative_index(-2, 5), Some(3));
        assert_eq!(resolve_negative_index(-5, 5), Some(0));
        assert_eq!(resolve_negative_index(-6, 5), None); // Out of bounds
        assert_eq!(resolve_negative_index(0, 5), Some(0));
        assert_eq!(resolve_negative_index(4, 5), Some(4));
        assert_eq!(resolve_negative_index(5, 5), None); // Out of bounds
    }

    #[test]
    fn quoted_keys_with_dots() {
        let key = Keyspec::from_str(r#""key.with.dots""#).expect("Should parse double-quoted key with dots");
        assert_eq!(key.subkeys.len(), 1);
        assert_eq!(key.subkeys[0], KeySegment::Name("key.with.dots".to_string()));

        let key = Keyspec::from_str("'key.with.dots'").expect("Should parse single-quoted key with dots");
        assert_eq!(key.subkeys.len(), 1);
        assert_eq!(key.subkeys[0], KeySegment::Name("key.with.dots".to_string()));
    }

    #[test]
    fn dots_and_boxes() {
        let key = Keyspec::from_str(r#"a."b.c"[0].d[-1]"#).expect("Should parse mixed notation");
        assert_eq!(key.subkeys.len(), 5);
        assert_eq!(key.subkeys[0], KeySegment::Name("a".to_string()));
        assert_eq!(key.subkeys[1], KeySegment::Name("b.c".to_string()));
        assert_eq!(key.subkeys[2], KeySegment::Index(0));
        assert_eq!(key.subkeys[3], KeySegment::Name("d".to_string()));
        assert_eq!(key.subkeys[4], KeySegment::Index(-1));
    }

    #[test]
    fn underscores_and_dashes() {
        let key = Keyspec::from_str("some_key.with-dashes").expect("Should parse keys with underscores and dashes");
        assert_eq!(key.subkeys.len(), 2);
        assert_eq!(key.subkeys[0], KeySegment::Name("some_key".to_string()));
        assert_eq!(key.subkeys[1], KeySegment::Name("with-dashes".to_string()));
    }

    #[test]
    fn numeric_keys() {
        let key = Keyspec::from_str("123.456").expect("Should parse numeric key names");
        assert_eq!(key.subkeys.len(), 2);
        assert_eq!(key.subkeys[0], KeySegment::Name("123".to_string()));
        assert_eq!(key.subkeys[1], KeySegment::Name("456".to_string()));
    }

    #[test]
    fn error_cases() {
        assert!(Keyspec::from_str("").is_err());
        assert!(Keyspec::from_str("[42]").is_err()); // Can't start with array index
        assert!(Keyspec::from_str("key[").is_err()); // Unclosed bracket
        assert!(Keyspec::from_str("key]").is_err()); // Unexpected bracket
        assert!(Keyspec::from_str(r#"key[abc]"#).is_err()); // Non-numeric index
        assert!(Keyspec::from_str(r#""unterminated"#).is_err()); // Unterminated quote
    }

    #[test]
    fn keys_str_round_trip() {
        let key = Keyspec::from_str("a.b[0]").expect("Should parse");
        assert_eq!(key.to_string(), "a.b[0]");
        let key2 = Keyspec::from_str("a.b.0").expect("Should parse");
        assert!(key != key2); // the dot 0 treats the zero as a bare key aka a string

        let key = Keyspec::from_str(r#""key with spaces""#).expect("Should parse");
        assert_eq!(key.to_string(), r#""key with spaces""#);

        let key = Keyspec::from_str("array[-1]").expect("Should parse");
        assert_eq!(key.to_string(), "array[-1]");
    }

    #[test]
    fn quote_escaping_double() {
        // Double quotes support escaping
        let key = Keyspec::from_str(r#""key \"with\" quotes""#).expect("Should parse escaped double quotes");
        assert_eq!(key.subkeys[0], KeySegment::Name(r#"key "with" quotes"#.to_string()));

        let key = Keyspec::from_str(r#""key\nwith\nnewlines""#).expect("Should parse escaped newlines");
        assert_eq!(key.subkeys[0], KeySegment::Name("key\nwith\nnewlines".to_string()));
    }

    #[test]
    fn quote_escaping_single() {
        // Single quotes don't support escaping in TOML (like in TOML spec)
        let key = Keyspec::from_str("'key with \"double\" quotes'")
            .expect("Should parse single quotes with double quotes inside");
        assert_eq!(
            key.subkeys[0],
            KeySegment::Name(r#"key with "double" quotes"#.to_string())
        );

        // Single quotes cannot contain single quotes (no escaping)
        let result = Keyspec::from_str("'can't contain single quotes'");
        assert!(result.is_err(), "Single quotes should not support escaping");
    }

    #[test]
    fn mixed_quote_types() {
        let key = Keyspec::from_str(r#"'single'."double".bare"#).expect("Should parse mixed quote types");
        assert_eq!(key.subkeys.len(), 3);
        assert_eq!(key.subkeys[0], KeySegment::Name("single".to_string()));
        assert_eq!(key.subkeys[1], KeySegment::Name("double".to_string()));
        assert_eq!(key.subkeys[2], KeySegment::Name("bare".to_string()));
    }

    #[test]
    fn empty_quoted_keys() {
        let key = Keyspec::from_str(r#""""#).expect("Should parse empty double-quoted key");
        assert_eq!(key.subkeys[0], KeySegment::Name("".to_string()));

        let key = Keyspec::from_str("''").expect("Should parse empty single-quoted key");
        assert_eq!(key.subkeys[0], KeySegment::Name("".to_string()));
    }
}
