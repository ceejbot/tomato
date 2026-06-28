# tomato

[![Test the tomato](https://github.com/ceejbot/tomato/actions/workflows/test.yaml/badge.svg)](https://github.com/ceejbot/tomato/actions/workflows/test.yaml)
![Crates.io](https://img.shields.io/crates/v/tomato-toml)

Get, set, delete, and inspect values in TOML files while preserving comments and formatting.

I wrote `tomato` to satisfy my own primary use case, which is to read values of various types from a TOML preferences file, process those values in bash tooling, and infrequently update those values from other bash scripts.

You can install from the latest release here, using `cargo install tomato-toml`, or using homebrew like this for Mac and Linux:

```shell
# using homebrew:
brew tap ceejbot/tap
brew install tomato
```

An alternative tool would be [dasel](https://daseldocs.tomwright.me), if you don't need to preserve comments and formatting when you modify a value. `dasel` also supports a large variety of file formats.

If you need to convert among JSON, YAML, and TOML, check out [jyt](https://github.com/ken-matsui/jyt).

## Usage

The short version:

* Get a key: `tomato get <dotted.key> <file>`
* Set a key: `tomato set <dotted.key> <value> <file>`
* Delete a key: `tomato rm <dotted.key> <file>` (with lots of aliases for `rm`)
* Append to array: `tomato append <dotted.key> <value> <file>`
* Check if key exists: `tomato exists <dotted.key> <file>` (exit codes for shell scripts)
* List keys: `tomato keys <dotted.key> <file>`

The `set`, `rm`, and `append` subcommands modify the input file in place. Thanks to the magic of [toml_edit](https://lib.rs/crates/toml_edit), they do so without disturbing whitespace and comments.

By default tomato emits data a form suitable for immediate use in bash scripts. Strings are unquoted, for instance. The `bash` format generates output suitable for `eval` inside bash. Use this for arrays and associative arrays. If you need to consume more complex output, you might select `json` format and pipe the results to `jq`. And of course if you need TOML, use `toml`.

## Keys

What is this dotted key notation, you ask? Sort of jq-ish, but more TOML. Specifically:

- All valid TOML keys are valid `tomato` keys.
- TOML keys may be either bare, quoted, or dotted. (Though shell quoting rules still apply.)
- Bare keys may only contain ASCII letters, ASCII digits, underscores, and dashes `(A-Za-z0-9_-)`.
- `1234` is a valid bare key; it is treated as a string.
- Keys with spaces in them must be quoted; the same for keys with literal dots like `"cats.are.good"`.
- `tomato` treats bare keys with dots the same way TOML does: as implicitly creating tables and referencing things inside the table. (This was the inspiration for the syntax.)

`tomato` does one additional thing that makes sense for its use case but not for TOML's:

- `tomato` supports indexing into arrays; indexing is 0-based.
- Negative number indexes count from the end of an array, with -1 being the last element.

### Output formats

What is this 'bash' output format, you ask? It's my best take on making TOML structures usable in bash scripts. It can be evaluated directly in bash to create native bash data structures:

**Arrays** are formatted as bash array syntax:
```bash
# TOML: fruits = ["apple", "banana", "cherry"]
$ tomato --format bash get fruits config.toml
( 'apple' 'banana' 'cherry' )

# Use in bash:
evaluation=$(tomato --format bash get fruits config.toml)
eval "my_array=$evaluation"
for item in "${my_array[@]}"; do
    echo "Fruit: $item"
done
```

**Inline tables** become bash associative arrays:
```bash
# TOML: person = { name = "Alice", age = 30, active = true }
$ tomato --format bash get person config.toml
declare -A bashval
bashval['name']='Alice'
bashval['age']=30
bashval['active']=1

# Use in bash:
eval "$(tomato --format bash get person config.toml)"
echo "Name: ${bashval[name]}, Age: ${bashval[age]}"
```

**Key lists** (from `keys` command) use the same array format:
```bash
$ tomato --format bash keys person config.toml
( 'age' 'active' 'name' )
```

- The associative array is always named `bashval` for consistency.
- Booleans are expressed as `1` or `0`.
- Strings and keys are single-quoted, so the output is safe to `eval` even when values contain shell metacharacters like `$`, backticks, or `$(...)`.
- For complex nested structures, you should fall back to JSON format and pipe to jq. Sometimes bash just can't do it. Don't tell it I said so.

> **Safety note:** string values and keys are single-quoted, so the `bash` output is safe to `eval` even when a value contains shell metacharacters like `$(...)`, backticks, or `$`. The one exception is deeply nested arrays-of-tables, which can't be represented as bash and fall back to TOML — reach for `--format json` and `jq` there.

### Setting booleans and numbers

`tomato` treats booleans and numbers slightly differently than ordinary values in its `set` command. It's likely that you'll need to set both true-the-boolean and "true"-the-string as values at some point. To distinguish `true` the boolean from `"true"` the string on the command line, use quotes around the string and do not use them around the boolean. Remember that your shell strips the first layer of quoting, so you need to use two layers! That is, use `'"true"'` to get a string, and `true` to get a boolean.

Here are examples that work with the test fixture:

```terminal
# set a value to a boolean
➜ tomato -f toml set testcases.are_complete true fixtures/sample.toml
false
➜ tomato -f toml get testcases.are_complete fixtures/sample.toml
true
# set a value to a string
➜ tomato -f toml set testcases.are_complete '"false"' fixtures/sample.toml
true
# set a value to a number
➜ tomato -f toml set testcases.are_complete 20 fixtures/sample.toml
"false"
# set a value to a string that happens to be a number
➜ tomato -f toml set testcases.are_complete "'20'" fixtures/sample.toml
20
➜ tomato -f toml get testcases.are_complete fixtures/sample.toml
"20"
```

### `stdin` and `stdout`

Omit the file argument to read from `stdin`. This changes how `set`, `rm`, and `append` behave, on the assumption that you're in a shell pipeline: instead of printing the previous value, tomato writes the *modified* document to `stdout` — as JSON if you asked for `--format json`, otherwise as TOML. (The `bash` format is ignored in this mode.) `get`, `exists`, and `keys` read from `stdin` exactly the way they read from a file.

```bash
# read a value from piped TOML
cat Cargo.toml | tomato get package.name

# edit in a pipeline: the whole modified document is written to stdout
cat Cargo.toml | tomato set package.name broccoli
```

## Cli usage

`tomato --help` gets you full help. `tomato [command] --help` gets help for any specific command. Here's the help text, for reference if you're deciding if this tool is useful to you or not.

```text
A command-line tool to get and set values in toml files while preserving comments and formatting.

Keys are written using `.` to separate path segments. You can use `array[idx]` syntax to index into
arrays if you want to. For example, to get the name of the current crate you're working on, you'd
run `tomato get Cargo.toml package.name`.

By default tomato emits data in a form suitable for immediate use in bash scripts if they are
primitive values: strings are unquoted, for instance. If you want to use more complex data types,
consider one of the other output formats.

To read from stdin instead of a file, omit the file argument. Operating on stdin changes the
behavior of set and rm somewhat, under the assumption that you are using this tool in a shell
script. If you read from stdin, normal output (the old value) is suppressed. Instead the modified
file is written to stdout in json if you requested json, toml otherwise. The 'bash' format option is
ignored.

Usage: tomato [OPTIONS] <COMMAND>

Commands:
  get          Get the value of a key from the given file
  set          Set a key to the given value, returning the previous value if one existed
  rm           Delete a key from the given file, returning the previous value if one existed
  append       Append the given value to an array, returning the previous array if one existed
  exists       Exits with 0 status code if the key exists in the input file, non-zero if not
  keys         List all keys at a given path
  completions  Generate completions for the named shell
  help         Print this message or the help of the given subcommand(s)

Options:
  -f, --format <FORMAT>
          How to format the output: json, toml, bash, or raw
          [default: raw]

  -b, --backup
          Back up the file to <filepath>.bak if we write a new version. This option is ignored when
          we're operating on stdin

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```


## Examples

Here are some examples run against the Cargo manifest for this project:

```shell
➜ tomato get package.name Cargo.toml
tomato
➜ tomato --format json get package.name Cargo.toml
"tomato"
➜ cat Cargo.toml | tomato get package.name
tomato

# set examples
➜ tomato set package.name broccoli Cargo.toml
tomato
➜ tomato get package.keywords[0] Cargo.toml
cli
➜ tomato set package.keywords[1] yaml Cargo.toml
toml
➜ tomato get package.keywords[-1] Cargo.toml
bash

# miette provides helpful error reports
➜ tomato get package.keywords[20] Cargo.toml
Error: tomato::array_bounds

  × Array index 20 is out of bounds
  help: The array has 4 elements. Valid indices are 0 to 3 (or negative indices from -1 to -4)

# Keys that don't exist return proper errors
➜ tomato get package.nonexistent Cargo.toml
Error: Key 'package.nonexistent' not found in TOML file

# Trying to index into non-arrays also errors helpfully
➜ tomato get package.name[0] Cargo.toml
Error: Cannot index into non-array at '[0]'

# Check if keys exist (useful in shell scripts)
➜ if tomato exists package.name Cargo.toml; then echo "found!"; fi
found!
➜ tomato exists package.nonexistent Cargo.toml; echo "exit code: $?"
exit code: 1

# List available keys at a path
➜ tomato keys package Cargo.toml
authors
categories
description
edition
license
name
readme
version

# rm has a number of aliases to prevent user frustration
➜ tomato --format json del package.categories[0] Cargo.toml
"command-line-utilities"
```

Look at the `examples/` directory for some sample bash scripts with more varied examples,
including examples of using lists and associative arrays in bash.

## CONTRIBUTING

Heck, yeah! Please keep the major use case in mind: you need to read toml and do stuff
with it in bash. I'm happy to accept anything that improves that use case or makes the Rust
parts better.

## LICENSE

[Blue Oak Model License](https://blueoakcouncil.org/license/1.0.0); text in [LICENSE.md](./LICENSE.md).
