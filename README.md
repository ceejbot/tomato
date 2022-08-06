# tomato

[![Test the tomato](https://github.com/ceejbot/tomato/actions/workflows/test.yaml/badge.svg)](https://github.com/ceejbot/tomato/actions/workflows/test.yaml)

Get, set, and delete values in TOML files while preserving comments and formatting.

That's it. That's the feature set. I wrote `tomato` to satisfy my own primary use
case, which is to read values of various types from a TOML preferences file,
process those values in bash tooling, and infrequently update those values from
different scripts.

An alternative tool would be [dasel](https://daseldocs.tomwright.me), if you
don't need to preserve comments and formatting when you modify a value. `dasel`
also supports a large variety of file formats.

If you need to convert among JSON, YAML, and TOML, check out
[jyt](https://github.com/ken-matsui/jyt), which does just that.

## Usage

The short version:

* Get a key: `tomato <file> get <dotted.key>`
* Set a key: `tomato <file> set <dotted.key> <value>`
* Delete a key: `tomato <file> rm <dotted.key>`

The `set` and `rm` subcommands modify the input file in place. Thanks to the magic of
[toml_edit](https://lib.rs/crates/toml_edit), they do so without disturbing whitespace and comments.

By default tomato emits data a form suitable for immediate use in bash scripts.
Strings are unquoted, for instance. The `bash` format generates output suitable
for `eval` inside bash. Use this for arrays and associative arrays. If you need
to consume more complex output, you might select `json` format and pipe the
results to `jq`. And of course if you need toml, use `toml`.
The longer version:

```
🍅 tomato 0.1.0
A command-line tool to get and set values in toml files while preserving comments and formatting.

Keys are written using `.` to separate path segments. You can use array[idx] syntax to index into
arrays if you want to. For example, to get the name of the current crate you're working on, you'd
run `tomato Cargo.toml get package.name`.

By default tomato emits data in a form suitable for immediate use in bash scripts if they are
primitive values: strings are unquoted, for instance. If you want to use more complex data types,
consider one of the other output formats. (Json output is not fully implemented yet!)

USAGE:
	tomato [OPTIONS] <FILEPATH> <SUBCOMMAND>

ARGS:
	<FILEPATH>
			The toml file to operate on

OPTIONS:
	-b, --backup
			Back up the file to <filepath>.bak if we write a new version

	-f, --format <FORMAT>
			How to format the output: json, toml, bash, or raw
			[default: raw]

	-h, --help
			Print help information

	-V, --version
			Print version information

SUBCOMMANDS:
	get     Get the value of a key from the given file
	help    Print this message or the help of the given subcommand(s)
	rm      Delete a key from the given file
	set     Set a key to the given value, returning the previous value if one existed
```

`get` and `rm` both print empty string to stdout if the target key is not found. `set`
exits with a non-zero status with a message printed to stderr if the target key is not found.

## Examples

Here are some examples run against the Cargo manifest for this project:

```shell
$ tomato Cargo.toml get package.name
tomato
$ tomato --format json Cargo.toml get package.name
"tomato"
$ tomato Cargo.toml get dependencies.toml_edit.version
0.14.4
$ tomato --format bash Cargo.toml get package.categories
( command-line-utilities toml )
$ tomato Cargo.toml get package.categories
( "command-line-utilities" "toml" )
$ tomato --format toml Cargo.toml get package.categories
["command-line-utilities", "toml"]
$ tomato Cargo.toml get package.categories[0]
command-line-utilities
$ tomato --format json Cargo.toml get package.categories[1]
"toml"

# set examples
$ tomato Cargo.toml set package.name broccoli
tomato
$ tomato Cargo.toml set package.categories[1] yaml
toml

# Keys that don't exist
$ tomato Cargo.toml get dependencies.toml_edit[0]

$ tomato Cargo.toml set dependencies.toml_edit[0] "first!"
Error: unable to index into non-array at dependencies.toml_edit.0

# rm has a number of aliases to prevent user frustration
$ tomato --format json Cargo.toml del package.categories[0]
"command-line-utilities"
```

There are some examples showing consuming array output in bash in `examples/simple.sh`.

## TODO

Remaining work:

- all commands work with the easy cases
- fill out bash output:
	- tables
	- arrays of tables
- complete the tests
	- datetimes
	- tables
	- arrays of tables
- good error output
- tty?
- sweat the help text

## LICENSE

[Blue Oak Model License](https://blueoakcouncil.org/license/1.0.0); text in [LICENSE.md](./LICENSE.md).
