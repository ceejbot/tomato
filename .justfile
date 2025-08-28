BINNAME := "tomato"
RELATIVE_TAP_PATH := "../../../homebrew-tap/"

_help:
	just -l

# Run all tests using nextest.
test:
	cargo nextest run

# Format the code.
fmt:
	cargo +nightly fmt

# Run the same checks we run in CI. Requires nightly.
ci: test fmt
	cargo clippy --all-targets

# Ask for clippy's opinion.
lint: fmt
	cargo clippy --fix

# Install required tools
setup:
	#!/usr/bin/env bash
	if [[ -z $(which cargo) ]]; then
		printf "Installing 🦀 {{BOLD}}{{RED}}Rust{{RESET}}...\n"
		curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
	else
	    rustup update
	fi
	rustup target add x86_64-apple-darwin
	rustup install nightly
	brew tap --quiet ceejbot/tap
	brew install --quiet fzf semver-bump formulaic cargo-nextest

# Use tomato to set the crate version to the passed in version, commit,
# and create a git tag `v{version}`. Will not act if there are uncommitted
# changes extant.

# Tag a new version for release.
version BUMP:
	#!/usr/bin/env bash
	set -e
	current=$(cargo run --quiet -- get package.version Cargo.toml)
	version=$(echo "$current" | semver-bump {{BUMP}})
	cargo run --quiet -- set package.version "$version" Cargo.toml &> /dev/null
	cargo generate-lockfile
	git commit Cargo.toml Cargo.lock -m "v${version}"
	git tag "v${version}"
	printf "Release tagged for version {{BOLD_YELLOW}}v${version}{{RESET}}\n"

# Release by hand instead of in action.
release:
	#!/usr/bin/env bash
	set -e

	mkdir -p dist
	cd dist
	tag=$(git describe --tags --abbrev=0)
	# fails if this already exists
	release_url=$(gh release create "$tag" --generate-notes)

	for target in "aarch64-apple-darwin" "x86_64-apple-darwin"; do
		cargo +stable build --release --target $target
		tar czf {{ BINNAME }}-$target.tar.gz --strip-components=2  target/$target/release/{{ BINNAME }}
		gh release upload "$tag" "{{ BINNAME }}-$target.tar.gz"
		sha256sum {{ BINNAME }}-$target.tar.gz > {{ BINNAME }}-"$target".tar.gz.sha256
		gh release upload "$tag" "{{ BINNAME }}-$target.tar.gz.sha256"
	done
	formula_file=$(formulaic ../Cargo.toml)
	mv dist/$formula_file {{RELATIVE_TAP_PATH}}/Formula/
	cd {{RELATIVE_TAP_PATH}} || exit
	git add Formula/$(basename $formula_file)
	git commit -m "$(basename -s .rb $formula_file) $tag"

RESET := "\\e[0m"
BOLD := "\\e[1m"
BOLD_YELLOW := "\\e[1;33m"
BOLD_BLUE := "\\e[1;34m"
BLACK := "\\e[30m"
BLACK_BG := "\\e[40m"
RED := "\\e[31m"
RED_BG := "\\e[41m"
GREEN := "\\e[32m"
GREEN_BG := "\\e[42m"
YELLOW := "\\e[33m"
YELLOW_BG := "\\e[43"
BLUE := "\\e[34m"
BLUE_BG:= "\\e[44m"
MAGENTA := "\\e[35m"
MAGENTA_BG := "\\e[45m"
CYAN := "\\e[36m"
CYAN_BG := "\\e[46m"
WHITE := "\\e[37m"
WHITE_BG := "\\e[47m"
DEFAULT := "\\e[39m"
DEFAULT_BG := "\\e[49m"
