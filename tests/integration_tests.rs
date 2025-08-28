use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use tempfile::NamedTempFile;

fn run_tomato_stdin(args: &[&str], stdin_data: &str) -> (String, String, bool) {
    let mut cmd = Command::new("cargo")
        .args(["run", "--quiet", "--"])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn tomato process");

    if let Some(mut stdin) = cmd.stdin.take() {
        stdin
            .write_all(stdin_data.as_bytes())
            .expect("Failed to write to stdin");
    }

    let output = cmd.wait_with_output().expect("Failed to read output");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.success(),
    )
}

fn run_tomato(args: &[&str]) -> (String, String, bool) {
    let output = Command::new("cargo")
        .args(["run", "--quiet", "--"])
        .args(args)
        .output()
        .expect("Failed to run tomato");

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.success(),
    )
}

#[test]
fn stdin_get() {
    let toml_data = r#"
[package]
name = "test-package"
version = "1.0.0"
"#;

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "package.name"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "test-package");
}

#[test]
fn stdin_set() {
    let toml_data = r#"
[package]
name = "test-package"
version = "1.0.0"
"#;

    let (stdout, _stderr, success) = run_tomato_stdin(&["set", "package.name", "new-name"], toml_data);
    assert!(success);
    assert!(stdout.contains("new-name"));
    assert!(stdout.contains("version = \"1.0.0\""));
}

#[test]
fn stdin_rm() {
    let toml_data = r#"
[package]
name = "test-package"
version = "1.0.0"
"#;

    let (stdout, _stderr, success) = run_tomato_stdin(&["rm", "package.name"], toml_data);
    assert!(success);
    assert!(!stdout.contains("name ="));
    assert!(stdout.contains("version = \"1.0.0\""));
}

#[test]
fn stdin_format_json() {
    let toml_data = r#"
[package]
name = "test-package"
"#;

    let (stdout, _stderr, success) = run_tomato_stdin(&["--format", "json", "get", "package.name"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "\"test-package\"");
}

#[test]
fn backup_feature() {
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");

    let toml_data = r#"
[package]
name = "test-package"
version = "1.0.0"
"#;

    temp_file
        .write_all(toml_data.as_bytes())
        .expect("Failed to write to temp file");
    temp_file.flush().expect("Failed to flush temp file");

    let file_path = temp_file
        .path()
        .to_str()
        .expect("Temp file path should be valid UTF-8")
        .to_string();

    // Run with backup flag
    let (_stdout, _stderr, success) = run_tomato(&["--backup", "set", "package.name", "new-name", &file_path]);
    assert!(success);

    // Check that backup file exists
    let backup_path = format!("{}.bak", file_path);
    assert!(std::path::Path::new(&backup_path).exists(), "Backup file should exist");

    // Check backup content
    let backup_content = fs::read_to_string(&backup_path).expect("Failed to read backup file");
    assert!(
        backup_content.contains("test-package"),
        "Backup should contain original content"
    );

    // Clean up backup file
    fs::remove_file(&backup_path).ok();
}

#[test]
fn missing_key_error() {
    let toml_data = r#"
[package]
name = "test-package"
"#;

    let (_stdout, stderr, success) = run_tomato_stdin(&["get", "package.nonexistent"], toml_data);
    assert!(!success); // get now errors on missing keys
    assert!(stderr.contains("Key 'package.nonexistent' not found in TOML file"));
    assert!(stderr.contains("tomato::key_not_found"));
}

#[test]
fn invalid_toml_error() {
    let invalid_toml = r#"
[package
name = "test-package"
"#;

    let (_stdout, stderr, success) = run_tomato_stdin(&["get", "package.name"], invalid_toml);
    assert!(!success);
    assert!(stderr.contains("TOML parse error") || stderr.contains("unclosed table"));
}

#[test]
fn array_of_tables() {
    // Note: tomato currently doesn't support accessing array of tables with index notation
    // This test documents the current behavior
    let toml_data = r#"
[[servers]]
name = "alpha"
ip = "10.0.0.1"

[[servers]]
name = "beta"
ip = "10.0.0.2"
"#;

    // This returns the entire array of tables
    let (stdout, _stderr, success) = run_tomato_stdin(&["--format", "json", "get", "servers"], toml_data);
    assert!(success);
    assert!(stdout.contains("alpha"));
    assert!(stdout.contains("10.0.0.1"));
    assert!(stdout.contains("beta"));
    assert!(stdout.contains("10.0.0.2"));
}

#[test]
fn deeply_nested_structure() {
    let toml_data = r#"
[a.b.c.d.e]
value = "deeply nested"
"#;

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "a.b.c.d.e.value"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "deeply nested");
}

#[test]
fn special_characters_in_values() {
    let toml_data = r#"
[test]
multiline = """
Line 1
Line 2
Line 3"""
with_quotes = 'He said "Hello"'
with_newline = "First\nSecond"
"#;

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "test.multiline"], toml_data);
    assert!(success);
    assert!(stdout.contains("Line 1"));
    assert!(stdout.contains("Line 2"));
    assert!(stdout.contains("Line 3"));

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "test.with_quotes"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), r#"He said "Hello""#);
}

#[test]
fn empty_string_value() {
    let toml_data = r#"
[test]
empty = ""
"#;

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "test.empty"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "");

    // Set empty string - tomato outputs '""' with single quotes
    let (stdout, _stderr, success) = run_tomato_stdin(&["set", "test.empty", r#"'""'"#], toml_data);
    assert!(success);
    assert!(stdout.contains(r#"empty = '""'"#));
}

#[test]
fn empty_array() {
    let toml_data = r#"
[test]
empty_array = []
"#;

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "test.empty_array"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "");

    let (stdout, _stderr, success) = run_tomato_stdin(&["--format", "json", "get", "test.empty_array"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "[]");
}

#[test]
fn datetime_handling() {
    let toml_data = r#"
[dates]
odt1 = 1979-05-27T07:32:00Z
odt2 = 1979-05-27T00:32:00-07:00
odt3 = 1979-05-27T00:32:00.999999-07:00
ldt1 = 1979-05-27T07:32:00
ldt2 = 1979-05-27T00:32:00.999999
ld1 = 1979-05-27
"#;

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "dates.odt1"], toml_data);
    assert!(success);
    assert!(stdout.contains("1979-05-27"));

    let (stdout, _stderr, success) = run_tomato_stdin(&["--format", "toml", "get", "dates.ldt2"], toml_data);
    assert!(success);
    assert!(stdout.contains("1979-05-27"));
    assert!(stdout.contains("00:32:00.999999"));

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "dates.ld1"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "1979-05-27");
}

#[test]
fn append_to_array() {
    let toml_data = r#"
[test]
fruits = ["apple", "banana"]
"#;

    let (stdout, _stderr, success) = run_tomato_stdin(&["append", "test.fruits", "orange"], toml_data);
    assert!(success);
    assert!(stdout.contains("apple"));
    assert!(stdout.contains("banana"));
    assert!(stdout.contains("orange"));
}

#[test]
fn append_creates_new_array() {
    let toml_data = r#"
[test]
value = "something"
"#;

    let (stdout, _stderr, success) = run_tomato_stdin(&["append", "test.new_array", "first"], toml_data);
    assert!(success);
    assert!(stdout.contains(r#"new_array = ["first"]"#));
}

#[test]
fn booleans() {
    let toml_data = r#"
[flags]
enabled = true
disabled = false
"#;

    // Raw format returns 1/0 for booleans
    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "flags.enabled"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "1");

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "flags.disabled"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "0");

    // TOML format returns true/false
    let (stdout, _stderr, success) = run_tomato_stdin(&["--format", "toml", "get", "flags.enabled"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "true");
}

#[test]
fn bash_arrays() {
    let toml_data = r#"
[test]
numbers = [1, 2, 3, 4, 5]
strings = ["foo", "bar", "baz"]
"#;

    let (stdout, _stderr, success) = run_tomato_stdin(&["--format", "bash", "get", "test.numbers"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "( 1 2 3 4 5 )");

    let (stdout, _stderr, success) = run_tomato_stdin(&["--format", "bash", "get", "test.strings"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), r#"( "foo" "bar" "baz" )"#);
}

#[test]
fn inline_table_bash_format() {
    let toml_data = r#"
[test]
inline = { name = "test", value = 42, enabled = true }
"#;

    let (stdout, _stderr, success) = run_tomato_stdin(&["--format", "bash", "get", "test.inline"], toml_data);
    assert!(success);
    assert!(stdout.contains("declare -A bashval"));
    assert!(stdout.contains(r#"bashval[name]="test""#));
    assert!(stdout.contains("bashval[value]=42"));
    assert!(stdout.contains("bashval[enabled]=1"));
}

#[test]
fn unicode_in_values() {
    let toml_data = r#"
[test]
emoji = "🍅"
chinese = "你好"
arabic = "مرحبا"
"#;

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "test.emoji"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "🍅");

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "test.chinese"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "你好");

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "test.arabic"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "مرحبا");
}

#[test]
fn the_points_that_float() {
    let toml_data = r#"
[numbers]
pi = 3.14159
negative = -273.15
scientific = 6.022e23
inf = inf
nan = nan
"#;

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "numbers.pi"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "3.14159");

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "numbers.negative"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "-273.15");

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "numbers.inf"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "inf");
}

#[test]
fn negative_array_indexing() {
    let toml_data = r#"
[test]
fruits = ["apple", "banana", "cherry", "date", "elderberry"]
"#;

    // Test last element
    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "test.fruits[-1]"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "elderberry");

    // Test second to last
    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "test.fruits[-2]"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "date");

    // Test first element using negative index
    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "test.fruits[-5]"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "apple");

    // Test out of bounds negative index (should return error)
    let (_stdout, stderr, success) = run_tomato_stdin(&["get", "test.fruits[-10]"], toml_data);
    assert!(!success);
    assert!(stderr.contains("Array index -10 is out of bounds"));
}

#[test]
fn negative_array_set() {
    let toml_data = r#"
[test]
fruits = ["apple", "banana", "cherry"]
"#;

    // Set last element using negative index
    let (stdout, _stderr, success) = run_tomato_stdin(&["set", "test.fruits[-1]", "orange"], toml_data);
    assert!(success);
    assert!(stdout.contains("apple"));
    assert!(stdout.contains("banana"));
    assert!(stdout.contains("orange"));
    assert!(!stdout.contains("cherry"));

    // Set first element using negative index
    let (stdout, _stderr, success) = run_tomato_stdin(&["set", "test.fruits[-3]", "pear"], toml_data);
    assert!(success);
    assert!(stdout.contains("pear"));
    assert!(stdout.contains("banana"));
    assert!(stdout.contains("cherry"));
    assert!(!stdout.contains("apple"));
}

#[test]
fn negative_array_remove() {
    let toml_data = r#"
[test]
fruits = ["apple", "banana", "cherry", "date"]
"#;

    // Remove last element using negative index
    let (stdout, _stderr, success) = run_tomato_stdin(&["rm", "test.fruits[-1]"], toml_data);
    assert!(success);
    assert!(stdout.contains("apple"));
    assert!(stdout.contains("banana"));
    assert!(stdout.contains("cherry"));
    assert!(!stdout.contains("date"));
}

#[test]
fn single_quote_key_parsing() {
    let toml_data = r#"
[test]
"key with spaces" = "double quoted"
normalkey = "normal"
"key.with.dots" = "dotted"
"#;

    // Test single quotes for keys with spaces
    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "test.'key with spaces'"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "double quoted");

    // Test mixed quoting
    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "'test'.'key with spaces'"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "double quoted");

    // Test single quotes for keys with dots
    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "test.'key.with.dots'"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "dotted");
}

#[test]
fn single_quote_edge_cases() {
    let toml_data = r#"
[test]
"" = "empty key"
"true" = "boolean string key"
"false" = "another boolean string key"
"#;

    // Test empty single-quoted key
    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "test.''"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "empty key");

    // Test single-quoted boolean string keys
    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "test.'true'"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "boolean string key");

    let (stdout, _stderr, success) = run_tomato_stdin(&["get", "test.'false'"], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "another boolean string key");
}

#[test]
fn single_quote_with_double_quotes_inside() {
    let toml_data = r#"
[test]
'key "with" quotes' = "value with embedded quotes"
"#;

    // Single quotes should handle double quotes inside without escaping
    let (stdout, _stderr, success) = run_tomato_stdin(&["get", r#"test.'key "with" quotes'"#], toml_data);
    assert!(success);
    assert_eq!(stdout.trim(), "value with embedded quotes");
}

// Error handling integration tests
#[test]
fn array_bounds_error() {
    let toml_data = r#"
[test]
fruits = ["apple", "banana", "cherry"]
"#;

    // Test positive index out of bounds
    let (_stdout, stderr, success) = run_tomato_stdin(&["get", "test.fruits[5]"], toml_data);
    assert!(!success);
    assert!(stderr.contains("Array index 5 is out of bounds"));
    assert!(stderr.contains("The array has 3 elements"));
    assert!(stderr.contains("Valid indices are 0 to 2"));
    assert!(stderr.contains("tomato::array_bounds"));

    // Test negative index out of bounds
    let (_stdout, stderr, success) = run_tomato_stdin(&["get", "test.fruits[-10]"], toml_data);
    assert!(!success);
    assert!(stderr.contains("Array index -10 is out of bounds"));
    assert!(stderr.contains("from -1 to -3"));
}

#[test]
fn cannot_append_to_non_array_error() {
    let toml_data = r#"
[test]
name = "string value"
count = 42
flag = true
"#;

    // Test appending to string
    let (_stdout, stderr, success) = run_tomato_stdin(&["append", "test.name", "value"], toml_data);
    assert!(!success);
    assert!(stderr.contains("Cannot append to non-array at 'test.name'"));
    assert!(stderr.contains("is a string, not an array"));
    assert!(stderr.contains("Use 'set' to replace"));

    // Test appending to integer
    let (_stdout, stderr, success) = run_tomato_stdin(&["append", "test.count", "value"], toml_data);
    assert!(!success);
    assert!(stderr.contains("is a integer, not an array"));

    // Test appending to boolean
    let (_stdout, stderr, success) = run_tomato_stdin(&["append", "test.flag", "value"], toml_data);
    assert!(!success);
    assert!(stderr.contains("is a boolean, not an array"));
}

#[test]
fn invalid_key_syntax_errors() {
    let toml_data = r#"
[test]
key = "value"
"#;

    // Test unterminated quote
    let (_stdout, stderr, success) = run_tomato_stdin(&["get", r#"test."unterminated"#], toml_data);
    assert!(!success);
    assert!(stderr.contains("Unterminated quoted string"));

    // Test invalid bracket syntax
    let (_stdout, stderr, success) = run_tomato_stdin(&["get", "test.key["], toml_data);
    assert!(!success);
    assert!(stderr.contains("Expected number in array index"));

    // Test empty key
    let (_stdout, stderr, success) = run_tomato_stdin(&["get", ""], toml_data);
    assert!(!success);
    assert!(stderr.contains("Empty key specification"));
}

#[test]
fn unsupported_format_error() {
    // Test unsupported format
    let (_stdout, stderr, success) = run_tomato(&["get", "--format", "xml", "test.key", "fixtures/sample.toml"]);
    assert!(!success);
    assert!(stderr.contains("Unsupported output format 'xml'"));
}
