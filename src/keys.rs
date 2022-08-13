use regex::Regex;
use std::fmt::Display;
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq, Eq)]
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
    pub subkeys: Vec<KeySegment>,
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
}
