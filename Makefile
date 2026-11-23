.PHONY: check test run exercise-book exercise-strategy exercise-risk

check:
	cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace

test:
	cargo test --workspace

ARGS ?=

run:
	cargo run --release -p app -- $(ARGS)

exercise-book:
	cargo test -p book --test mine -- --include-ignored

exercise-strategy:
	cargo test -p strategy --test mine -- --include-ignored

exercise-risk:
	cargo test -p risk --test mine -- --include-ignored
