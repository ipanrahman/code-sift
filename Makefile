# CodeSift Makefile
# Token-efficient code intelligence engine for AI agents

.PHONY: all build release clean test clippy fmt help

# Default target
all: release

# Build in debug mode
build:
	@cargo build

# Release build (optimized)
release:
	@cargo build --release

# Copy release binary to bin folder
bin: release
	@mkdir -p bin
	@cp target/release/codesift bin/codesift

# Run tests
test:
	@cargo test

# Run clippy linter
clippy:
	@cargo clippy

# Format code
fmt:
	@cargo fmt

# Run all checks (build + clippy + test)
check: build clippy test

# Clean build artifacts
clean:
	@cargo clean

# Show this help message
help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'