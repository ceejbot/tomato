#!/usr/bin/env bash

# Lots of examples of getting keys, which is the main use case.

cargo build --quiet

target/debug/tomato get Cargo.toml package.name
# tomato

target/debug/tomato --format json get Cargo.toml package.name
# "tomato"

target/debug/tomato get Cargo.toml dependencies.toml_edit.version
# 0.14.4

# Read from stdin
cat Cargo.toml | target/debug/tomato get - package.version
# should print our current version!

target/debug/tomato --format bash get Cargo.toml package.categories
# ( command-line-utilities toml )

target/debug/tomato get Cargo.toml package.categories
# ( "command-line-utilities" "toml" )

target/debug/tomato --format toml get Cargo.toml package.categories
# ["command-line-utilities", "toml"]

target/debug/tomato get Cargo.toml package.categories[0]
# command-line-utilities

target/debug/tomato --format json get Cargo.toml package.keywords[1]
# "toml"
