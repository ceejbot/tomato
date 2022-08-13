
# List available recipes.
help:
	@just -l

# Run all tests with nextest
test:
	@cargo nextest run

# Lint and stuff.
ci:
	@cargo fmt
	@cargo clippy
	@cargo nextest run

# Build docs and open them in your browser.
docs:
	@cargo doc --no-deps --open

# Install the tool into .cargo/bin
install:
	@cargo install --path .

# Use tomato to set the crate version to the passed in version, commit,
# and create a git tag `v{version}`. Will not act if there are uncommitted
# changes extant.
tag VERSION:
	#!/usr/bin/env bash
	status=$(git status --porcelain)
	if [ "$status" != ""  ]; then
		echo "There are uncommitted changes! Cowardly refusing to act."
		exit 1
	fi
	cargo run --quiet -- set Cargo.toml package.version {{VERSION}}
	git commit Cargo.toml -m "v{{VERSION}}"
	git tag "v{{VERSION}}"
	echo "Release tagged for version v{{VERSION}}"
