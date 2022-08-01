# tomato

Get, set, and delete values in TOML files while preserving comments and formatting.

That's it. That's the feature set. `tomato` is intended to be used in shell scripts
that need to read toml values and occasionally set them.

An alternative tool would be [dasel](https://daseldocs.tomwright.me), if you don't
need to preserve comments and formatting when you modify a value. Dasel also
supports a large variety of file formats.

## Usage

The short version:

* Get a key: `tomato <file> get <dotted.key>`
* Set a key: `tomato <file> set <dotted.key> <value>`
* Delete a key: `tomato <file> rm <dotted.key>`

The longer version:

```
tomato 0.1.0
C J Silverio <ceejceej@gmail.com>
🍅 A command-line tool to get and set values in toml files while preserving comments and formatting

USAGE:
	tomato [OPTIONS] <FILEPATH> <SUBCOMMAND>

ARGS:
	<FILEPATH>    The toml file to operate on

OPTIONS:
	-b, --backup     Back up the file to <filepath>.bak if we write a new version
	-h, --help       Print help information
	-r, --raw        Raw mode; if set does not quote strings when printing values
	-V, --version    Print version information

SUBCOMMANDS:
	get     Get the value of a key from the given file
	help    Print this message or the help of the given subcommand(s)
	rm      Delete a key from the given file
	set     Set a key to the given value, returning the previous value if one existed
```


All error text is printed to stderr. (or will be, anyway)

## Examples

Here are some examples run against the Cargo manifest for this project:

```sh
$ tomato Cargo.toml get package.name
"tomato"
$ tomato -r Cargo.toml get package.name
tomato
$ tomato Cargo.toml get dependencies.toml_edit.version
0.14.4
$ tomato Cargo.toml get package.categories
["command-line-utilities", "toml"]
$ tomato -r Cargo.toml get package.categories[0]
command-line-utilities
$ tomato Cargo.toml get package.categories[1]
"toml"
$ tomato Cargo.toml get dependencies.toml_edit[0]
value at dependencies.toml_edit is not an array and cannot be indexed into
$ tomato Cargo.toml del package.categories[1]
$ tomato Cargo.toml set package.name broccoli
tomato
$ tomato Cargo.toml set package.categories[0] yaml
toml
```

## implementation notes

TODO

- all commands work with the easy case; need testing with the array cases
- need to fill out the tests
	- datetimes
	- bools
- raw output vs quoted (does anybody ever need quoted?)
- good error output
- tty?
- sweat the help text

## LICENSE

[Blue Oak Model License](https://blueoakcouncil.org/license/1.0.0); text in [LICENSE.md](./LICENSE.md).
