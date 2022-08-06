#!/usr/bin/env bash

echo ''
echo "first, this is what the data we're fetching looks like in json:"
cargo run --quiet -- --format json ./fixtures/sample.toml get testcases.inline_table
echo ''
echo "this is what our bash output looks like:"
cargo run --quiet -- --format bash ./fixtures/sample.toml get testcases.inline_table
echo ''
echo "let's evaluate that text!"
evaluation=$(cargo run --quiet -- --format bash ./fixtures/sample.toml get testcases.inline_table)
eval "$evaluation"
for i in "${!bashval[@]}"; do
	echo "key: $i	value: ${bashval[$i]}"
done
