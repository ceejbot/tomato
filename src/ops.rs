//! The document engine: read and write TOML files, and get, set, remove, append,
//! and list values within a parsed document. Everything here operates on validated
//! [`Keyspec`] paths and `toml_edit` types. The CLI layer in `main.rs` is a thin
//! shell around these functions.

use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;

use toml_edit::{DocumentMut, Item, Value};

use crate::errors::{DisplayInfo, TomatoError};
use crate::parser::{KeySegment, Keyspec, resolve_negative_index};

/// Read the toml file and parse it. Respond with an error that gets propagated up
/// if the file is not valid toml.
pub(crate) fn parse_file(maybepath: Option<&String>) -> Result<DocumentMut, TomatoError> {
    let mut data = String::new();
    if let Some(ref fpath) = maybepath {
        let file = File::open(fpath)?;
        let mut reader = BufReader::new(file);
        reader.read_to_string(&mut data)?;
    } else {
        let mut reader = BufReader::new(std::io::stdin());
        reader.read_to_string(&mut data)?;
    }

    Ok(data.parse::<DocumentMut>()?)
}

pub(crate) fn write_file(toml: &DocumentMut, fpath: &str, backup: bool) -> Result<(), TomatoError> {
    if backup {
        std::fs::copy(fpath, format!("{}.bak", fpath))?;
    }
    let mut output = File::create(fpath)?;
    // Note for future work: this won't be great for large files
    write!(output, "{toml}")?;
    Ok(())
}

/// Given a key segment, find that key in this node. Returns None if the key segment is an
/// int but the node is not an array, or if the array index is out of bounds.
fn get_in_node<'a>(key: &'a KeySegment, node: &'a mut Item) -> Result<Option<&'a mut Item>, TomatoError> {
    match key {
        KeySegment::Name(n) => Ok(node.get_mut(n)),
        KeySegment::Index(idx) => {
            if let Some(array) = node.as_array() {
                let array_len = array.len();
                if let Some(resolved_idx) = resolve_negative_index(*idx, array_len) {
                    Ok(node.get_mut(resolved_idx))
                } else {
                    let plural = if array_len == 1 { "" } else { "s" };
                    let valid_range = if array_len > 0 {
                        format!("0 to {}", array_len - 1)
                    } else {
                        "none (empty array)".to_string()
                    };
                    Err(TomatoError::ArrayIndexOutOfBounds {
                        index: *idx,
                        length: array_len,
                        valid_range,
                        plural: plural.to_string(),
                    })
                }
            } else {
                // Cannot index into non-array types
                let (_, value_type) = node_type_info(node);
                Err(TomatoError::CannotIndexIntoNonArray {
                    key: format!("[{}]", idx),
                    value_type,
                })
            }
        }
    }
}

fn node_type_info(node: &Item) -> (bool, String) {
    (node.is_primitive(), node.type_str().to_string())
}

/// Helper function to traverse a key path and handle common error cases
/// Returns the final node or an error
fn traverse_key_path<'a>(
    start_node: &'a mut Item,
    key_path: &'a [KeySegment],
    dotted_key: &Keyspec,
) -> Result<&'a mut Item, TomatoError> {
    let mut node = start_node;

    for k in key_path {
        // Get type information before the mutable borrow
        let (is_primitive, value_type) = node_type_info(node);

        let found = match get_in_node(k, node)? {
            Some(found) => found,
            None => {
                // Check if we're trying to access a property on a primitive value
                if let KeySegment::Name(name) = k
                    && is_primitive
                {
                    return Err(TomatoError::PropertyOnPrimitive {
                        property: name.clone(),
                        value_type,
                    });
                }

                // Missing keys are always errors - this function is for strict path traversal
                return Err(TomatoError::KeyNotFound {
                    key: format!("{}", dotted_key),
                });
            }
        };

        node = found;
    }

    Ok(node)
}

/// Given a full dotted-form key from the command-line, find the matching value
/// in the given document. Returns an error if not found.
pub(crate) fn get_key(toml: &mut DocumentMut, dotted_key: &Keyspec) -> Result<Item, TomatoError> {
    let node = traverse_key_path(toml.as_item_mut(), &dotted_key.subkeys, dotted_key)?;

    if let Item::None = node {
        return Err(TomatoError::KeyNotFound {
            key: format!("{}", dotted_key),
        });
    }

    Ok(node.clone())
}

/// Remove the node corresponding to the given key. If the key is a valid path that
/// points to something that's already gone, we treat that as a non-error: running
/// this is idempotent.
pub(crate) fn remove_key(toml: &mut DocumentMut, dotted_key: &Keyspec) -> Result<Item, TomatoError> {
    let mut parent_key: Keyspec = dotted_key.clone();

    let Some(target) = parent_key.subkeys.pop() else {
        return Err(TomatoError::NoKeyToRemove);
    };

    // Traverse to the parent node - for rm, missing parent paths mean the key doesn't exist
    let node = if parent_key.subkeys.is_empty() {
        toml.as_item_mut()
    } else {
        match traverse_key_path(toml.as_item_mut(), &parent_key.subkeys, &parent_key) {
            Ok(parent_node) => parent_node,
            Err(_) => return Ok(Item::None), // Parent path invalid, key doesn't exist (idempotent)
        }
    };

    // Check the final key
    let (is_primitive, value_type) = node_type_info(node);

    if let Some(found) = get_in_node(&target, node)? {
        let original = found.clone();
        *found = Item::None;
        return Ok(original);
    }

    // If we couldn't find the key, check if it was because of invalid access
    if let KeySegment::Name(name) = &target
        && is_primitive
    {
        // Trying to remove a property from a primitive is an error
        return Err(TomatoError::PropertyOnPrimitive {
            property: name.clone(),
            value_type,
        });
    }

    // Valid path but key doesn't exist - this is OK for rm (idempotent)
    Ok(Item::None)
}

/// Set the given key to the new value, and respond with the original value.
/// Replaces null nodes if the parent was found, adding a new key to the
/// document. Responds with an error if the key included an index into an
/// array for a non-array node in the document.
pub(crate) fn set_key(toml: &mut DocumentMut, dotted_key: &Keyspec, value: &Value) -> Result<Item, TomatoError> {
    let node = traverse_key_path(toml.as_item_mut(), &dotted_key.subkeys, dotted_key)?;

    let original = node.clone();
    let existing: &mut Item = &mut *node;

    // Straight outta cargo-edit
    let existing_decor = existing.as_value().map(|v| v.decor().clone()).unwrap_or_default();
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
pub(crate) fn append_value(toml: &mut DocumentMut, dotted_key: &Keyspec, value: &str) -> Result<Item, TomatoError> {
    let node = traverse_key_path(toml.as_item_mut(), &dotted_key.subkeys, dotted_key)?;

    let original = node.clone();

    // Get type info before mutable operations using consistent helper
    let (_, value_type) = node_type_info(&original);

    node.or_insert(Item::Value(Value::Array(toml_edit::Array::new())))
        .as_array_mut()
        .ok_or_else(|| TomatoError::CannotAppendToNonArray {
            key: dotted_key.to_string(),
            value_type,
        })?
        .push(value);

    Ok(original)
}

/// Extract keys from a TOML item if it's a table or inline table
/// Returns an error if the item is not a table type
pub(crate) fn list_keys(item: &Item, key_path: &Keyspec) -> Result<Vec<String>, TomatoError> {
    match item {
        Item::Table(table) => {
            let mut keys: Vec<String> = table.iter().map(|(k, _)| k.to_string()).collect();
            keys.sort();
            Ok(keys)
        }
        Item::Value(Value::InlineTable(inline_table)) => {
            let mut keys: Vec<String> = inline_table.iter().map(|(k, _)| k.to_string()).collect();
            keys.sort();
            Ok(keys)
        }
        _ => {
            let (_, value_type) = node_type_info(item);
            Err(TomatoError::CannotListKeysOnNonTable {
                key: key_path.to_string(),
                value_type,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::format::{Format, format_item, format_toml};

    #[test]
    fn get() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml.parse::<DocumentMut>().expect("test doc should be valid toml");

        let key = Keyspec::from_str("testcases.hashes.color").expect("test key should be valid");
        let item = get_key(&mut doc, &key).expect("expected to get key 'hashes.color'");
        assert_eq!("brown", format_item(&item, Format::Raw));
        assert_eq!("\"brown\"", format_item(&item, Format::Toml));

        let key = Keyspec::from_str("testcases.hashes.mats[1]").expect("test key should be valid");
        let item = get_key(&mut doc, &key).expect("expected this key to be valid");
        assert_eq!("salt", format_item(&item, Format::Raw));
    }

    #[test]
    fn get_key_errs_correctly() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml.parse::<DocumentMut>().expect("test doc should be valid toml");

        let no_exists = Keyspec::from_str("cats").expect("test key should be valid");
        let maybe_cats = get_key(&mut doc, &no_exists);
        assert!(maybe_cats.is_err());
    }

    #[test]
    fn set() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml.parse::<DocumentMut>().expect("test doc should be valid toml");

        let key = Keyspec::from_str("testcases.hashes.color").expect("test key should be valid");
        let taupe = Value::from("taupe");
        let item = set_key(&mut doc, &key, &taupe).expect("expected to find key 'hashes.color'");
        assert_eq!("brown", format_item(&item, Format::Raw));
        assert!(doc.to_string().contains("color = \"taupe\""));

        let key = Keyspec::from_str("testcases.hashes.mats[3]").expect("expected this key to be valid");
        let bacon = Value::from("bacon");
        let item = set_key(&mut doc, &key, &bacon).expect("could not find this key");
        assert_eq!("frying", format_item(&item, Format::Raw));
        assert!(doc.to_string().contains("bacon"));
    }

    #[test]
    fn append() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml.parse::<DocumentMut>().expect("test doc should be valid toml");

        let key = Keyspec::from_str("testcases.fruits").expect("test key should be valid");
        let item = append_value(&mut doc, &key, "orange").expect("expected to be able to insert value 'orange'");
        let formatted = format_toml(&item);
        assert_eq!(formatted, r#"[ "tomato", "plum", "pluot", "kumquat", "persimmon" ]"#);
        assert!(
            doc.to_string()
                .contains(r#"fruits = [ "tomato", "plum", "pluot", "kumquat", "persimmon" , "orange"]"#)
        );
    }

    #[test]
    fn append_to_non_existing_key_creates_array() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml.parse::<DocumentMut>().expect("test doc should be valid toml");

        let key = Keyspec::from_str("testcases.these.are.not.fruits").expect("test key should be valid");
        let item = append_value(&mut doc, &key, "leek").expect("expected to be able to insert value 'leek'");
        assert!(item.is_none());
        assert!(
            doc.to_string()
                .contains(r#"these = { are = { not = { fruits = ["leek"] } } }"#)
        );

        let item = append_value(&mut doc, &key, "artichoke").expect("expected to be able to insert value 'artichoke'");
        assert_eq!(format_toml(&item), r#"["leek"]"#);
        assert!(
            doc.to_string()
                .contains(r#"these = { are = { not = { fruits = ["leek", "artichoke"] } } }"#)
        );

        let key = Keyspec::from_str("testcases.these.are.maybe.fruits").expect("test key should be valid");
        let item = append_value(&mut doc, &key, "banana").expect("expected to be able to insert value 'banana'");
        eprintln!("{doc}");
        assert!(item.is_none());
        assert!(doc.to_string().contains(
            r#"these = { are = { not = { fruits = ["leek", "artichoke"] }, maybe = { fruits = ["banana"] } } }"#
        ));
    }

    #[test]
    fn yeet() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml.parse::<DocumentMut>().expect("test doc should be valid toml");

        let key = Keyspec::from_str("testcases.hashes.color").expect("test key should be valid");
        let item = remove_key(&mut doc, &key).expect("expected to find key 'hashes.color'");
        assert_eq!("brown", format_item(&item, Format::Raw));
        assert!(!doc.to_string().contains("color = \"brown\""));

        let key = Keyspec::from_str("testcases.hashes.mats[1]").expect("test key should be valid");
        let item = remove_key(&mut doc, &key).expect("expected to find key testcases.hashes.mats[1]");
        assert_eq!("salt", format_item(&item, Format::Raw));
        assert!(doc.to_string().contains(r#"mats = [ "potatoes", "oil", "frying" ]"#));
    }

    #[test]
    fn can_set_booleans() {
        let toml = include_str!("../fixtures/sample.toml");
        let mut doc = toml.parse::<DocumentMut>().expect("test doc should be valid toml");

        let key = Keyspec::from_str("testcases.are_passing").expect("test key should be valid");
        let newval = Value::from(false);
        let previous = set_key(&mut doc, &key, &newval).expect("test fixture known to contain the test key");
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
        let curval = current.as_value().expect("the new value should be a valid toml value");
        match curval {
            Value::Boolean(b) => {
                assert!(!*b.value());
            }
            _ => panic!("fetched value was supposed to be a boolean!"),
        }
    }
}
