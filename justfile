
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
