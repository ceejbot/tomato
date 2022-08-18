# tomato

[![Test the tomato](https://github.com/ceejbot/tomato/actions/workflows/test.yaml/badge.svg)](https://github.com/ceejbot/tomato/actions/workflows/test.yaml)
![Crates.io](https://img.shields.io/crates/v/tomato-toml)

Get, set, and delete values in TOML files while preserving comments and formatting.

That's it. That's the feature set. I wrote `tomato` to satisfy my own primary use
case, which is to read values of various types from a TOML preferences file,
process those values in bash tooling, and infrequently update those values from
other bash scripts.

To install:

```shell
# using homebrew:
brew tap ceejbot/tap
brew install tomato

# if you have rust installed and prefer to build from source:
cargo install tomato-toml

# once installed:
tomato --help
```

Or snag a pre-built executable from [the releases](https://github.com/ceejbot/tomato/releases/latest).

An alternative tool would be [dasel](https://daseldocs.tomwright.me), if you
don't need to preserve comments and formatting when you modify a value. `dasel`
also supports a large variety of file formats.

If you need to convert among JSON, YAML, and TOML, check out
[jyt](https://github.com/ken-matsui/jyt).

## Usage

The short version:

* Get a key: `tomato get <dotted.key> <file>`
* Set a key: `tomato set <dotted.key> <value> <file>`
* Delete a key: `tomato rm <dotted.key> <file>` (with lots of aliases for `rm`)

The `set` and `rm` subcommands modify the input file in place. Thanks to the magic of
[toml_edit](https://lib.rs/crates/toml_edit), they do so without disturbing whitespace
and comments.

By default tomato emits data a form suitable for immediate use in bash scripts.
Strings are unquoted, for instance. The `bash` format generates output suitable
for `eval` inside bash. Use this for arrays and associative arrays. If you need
to consume more complex output, you might select `json` format and pipe the
results to `jq`. And of course if you need TOML, use `toml`.

The longer version:

```text
🍅 tomato 0.2.0
A command-line tool to get and set values in toml files while preserving comments and formatting.

Keys are written using `.` to separate path segments. You can use `array[idx]` syntax to index into
arrays if you want to. For example, to get the name of the current crate you're working on, you'd
run `tomato get Cargo.toml package.name`.

By default tomato emits data in a form suitable for immediate use in bash scripts if they are
primitive values: strings are unquoted, for instance. If you want to use more complex data types,
consider one of the other output formats.

To read from stdin instead of a file, omit the file from the arguments. Operating on stdin changes
the behavior of set and rm somewhat, under the assumption that you are using this tool in a shell
script. If you read from stdin, normal output (the old value) is suppressed. Instead the modified
file is written to stdout in json if you requested json, toml otherwise. The 'bash' format option is
ignored.

USAGE:
	tomato [OPTIONS] <SUBCOMMAND>

OPTIONS:
	-b, --backup
			Back up the file to <filepath>.bak if we write a new version. This option is ignored
			when we're operating on stdin

	-f, --format <FORMAT>
			How to format the output: json, toml, bash, or raw
			[default: raw]

	-h, --help
			Print help information

	-V, --version
			Print version information

SUBCOMMANDS:
	get     Get the value of a key from the given file
	set     Set a key to the given value, returning the previous value if one existed
	rm      Delete a key from the given file, returning the previous value if one existed
	completions
			Generate completions for the named shell
	help    Print this message or the help of the given subcommand(s)
```

`get` and `rm` both print empty string to stdout if the target key is not found. `set`
exits with a non-zero status with a message printed to stderr if the target key is not found.

## Examples

Here are some examples run against the Cargo manifest for this project:

```shell
$ tomato get package.name Cargo.toml
tomato
$ tomato --format json get package.name Cargo.toml
"tomato"
$ cat Cargo.toml | tomato get package.name
tomato

# set examples
$ tomato set package.name broccoli Cargo.toml
tomato
$ tomato set package.keywords[1] yaml Cargo.toml
toml

# Keys that don't exist
$ tomato get dependencies.toml_edit[0] Cargo.toml

$ tomato set dependencies.toml_edit[0] "first!" Cargo.toml
Error: unable to index into non-array at dependencies.toml_edit.0

# rm has a number of aliases to prevent user frustration
$ tomato --format json del package.categories[0] Cargo.toml
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
