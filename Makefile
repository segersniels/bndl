.DEFAULT_GOAL := build

clean:
	cargo clean

build: clean
	cargo build --release

dev:
	cargo watch -- cargo build

lint:
	cargo clippy
	cargo fmt --check

lint-fix:
	cargo clippy --fix --allow-dirty
	cargo fmt

version:
	@cargo pkgid | sed 's/.*[#@]\(.*\)/\1/'

name:
	@cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select( .name == "bndl_convert") | .targets[] | select( .kind | map(. == "bin") | any ) | .name'
