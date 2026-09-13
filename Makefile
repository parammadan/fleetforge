# FleetForge — reproducible development commands.
.PHONY: check fmt lint test scripts build clean demo demo-test

check: fmt lint test scripts ## everything CI runs

demo: ## one command: build the UI, serve the replay, print one URL
	./scripts/demo.sh

demo-test: ## regression tests for the demo launcher
	./scripts/tests/test-demo.sh

scripts:
	./scripts/tests/test-eks-down.sh
	./scripts/tests/test-demo.sh

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
