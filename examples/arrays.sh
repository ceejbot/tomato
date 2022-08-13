#!/opt/homebrew/bin/bash

# Some examples of arrays in bash. Note the hashbang above!
# This is to make sure I'm running a modern bash on MacOS for
# the readarray example.

categories=$(cargo run --quiet -- get ./fixtures/sample.toml package.categories)
for c in $categories; do
	echo "Marvelous category '$c'!"
done

# If you have a modern bash:
echo ''
echo "modern bash:"
readarray -t phrases < <(cargo run --quiet -- get ./fixtures/sample.toml testcases.phrases)
for i in "${phrases[@]}"; do
	echo "readarray: $i"
done

# If you don't have a modern bash:
echo ''
echo "we have an old bash:"
IFS=$'\n' read -r -d '' -a phrases < <( cargo run --quiet  -- get ./fixtures/sample.toml testcases.phrases )
for i in "${phrases[@]}"; do
	echo "old bash: $i"
done

echo ''
echo "let's use some eval!"
evaluation=$(cargo run --quiet -- --format bash get ./fixtures/sample.toml testcases.phrases)
eval "result=$evaluation"
for i in "${result[@]}"; do
	echo "eval: $i"
done
