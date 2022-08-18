#!/usr/bin/env bash

# Lots of examples of getting keys, which is the main use case.

cargo build --quiet

target/debug/tomato get package.name Cargo.toml
# tomato

target/debug/tomato --format json get package.name Cargo.toml
# "tomato"

target/debug/tomato get dependencies.toml_edit.version Cargo.toml
# 0.14.4

# Read from stdin
cat Cargo.toml | target/debug/tomato get package.version
# should print our current version!

target/debug/tomato --format bash get package.categories Cargo.toml
# ( command-line-utilities toml )

target/debug/tomato get package.categories Cargo.toml
# ( "command-line-utilities" "toml" )

target/debug/tomato --format toml get package.categories Cargo.toml
# ["command-line-utilities", "toml"]

target/debug/tomato get package.categories[0] Cargo.toml
# command-line-utilities

target/debug/tomato --format json get package.keywords[1] Cargo.toml
# "toml"
