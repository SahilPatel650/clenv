.PHONY: build release release-linux test check clean install pin-deps help

# Default target
help:
	@echo "clenv build targets:"
	@echo "  make build         - Debug build"
	@echo "  make release       - Release build (macOS)"
	@echo "  make release-linux - Static release build for Linux (musl)"
	@echo "  make test          - Run all tests"
	@echo "  make check         - Type-check without building"
	@echo "  make clean         - Remove build artifacts"
	@echo "  make install       - Install binary to ~/.local/bin"
	@echo "  make pin-deps      - Pin transitive deps for Rust 1.85 compat"

build:
	cargo build

release:
	cargo build --release

release-linux:
	rustup target add x86_64-unknown-linux-musl
	cargo build --release --target x86_64-unknown-linux-musl

test:
	cargo test

check:
	cargo check

clean:
	cargo clean

install: release
	mkdir -p ~/.local/bin
	cp target/release/clenv ~/.local/bin/clenv

# Pin transitive deps that require Rust > 1.85 down to compatible versions
pin-deps:
	cargo update instability --precise 0.3.7
	cargo update image --precise 0.25.5
