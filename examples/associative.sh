#!/usr/bin/env bash

cargo build --quiet

echo "Associative arrays exist in bash, but are a bit limited."

echo ''
echo "first, this is what the data we're fetching looks like in json:"
target/debug/tomato --format json get testcases.inline_table  ./fixtures/sample.toml
echo ''
echo "this is what our bash output looks like:"
target/debug/tomato --format bash get testcases.inline_table ./fixtures/sample.toml
echo ''
echo "let's evaluate that text!"
evaluation=$(target/debug/tomato --format bash get testcases.inline_table ./fixtures/sample.toml)
eval "$evaluation"
for i in "${!bashval[@]}"; do
	echo "key: $i	value: ${bashval[$i]}"
done
