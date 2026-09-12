# FleetForge — reproducible development commands.
.PHONY: check fmt lint test build clean

check: fmt lint test ## everything CI runs

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test --workspace

build:
	cargo build --workspace

clean:
	cargo clean
