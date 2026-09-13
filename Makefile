# FleetForge — reproducible development commands.
.PHONY: check fmt lint test scripts build clean

check: fmt lint test scripts ## everything CI runs

scripts:
	./scripts/tests/test-eks-down.sh

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
