#!/usr/bin/env bash

cargo build --quiet

# Read from stdin
cat Cargo.toml | target/debug/tomato get package.version

sample=$(cat ./fixtures/sample.toml)

echo ''
echo "Reading from stdin with the default format:"
target/debug/tomato get testcases.fruits <<< "$sample"

echo ''
echo "Reading from stdin with format json:"
target/debug/tomato --format json get testcases.fruits <<< "$sample"

echo ''
echo "Removing a key from stdin with default format (toml):"
target/debug/tomato yeet testcases.fruits[1] <<< "$sample"

echo ''
echo "Now we'll remove and emit json:"
target/debug/tomato --format json unset testcases.fruits[1] <<< "$sample"
