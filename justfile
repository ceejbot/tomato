
# List available recipes.
help:
	just -l

# Run all tests with nextest
test:
	cargo nextest run

# Lint and stuff.
ci:
	cargo fmt
	cargo clippy
	cargo test
