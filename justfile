# Default recipe (runs when just is called without arguments)
default:
    @just --list

# Format Rust code
format:
    cargo +nightly fmt --all

# Check if code is formatted
format-check:
    cargo +nightly fmt -- --check

# Lint Rust code using clippy
lint:
    cargo clippy --bins --tests --benches --examples --all-features --all-targets -- -D warnings

# Build the project
build:
    cargo build

# Build in release mode
build-release:
    cargo build --release

# Build Debian package for the current architecture
deb: build-release
    cargo deb -p moltis-cli --no-build

# Build Debian package for amd64
deb-amd64:
    cargo build --release --target x86_64-unknown-linux-gnu
    cargo deb -p moltis-cli --no-build --target x86_64-unknown-linux-gnu

# Build Debian package for arm64
deb-arm64:
    cargo build --release --target aarch64-unknown-linux-gnu
    cargo deb -p moltis-cli --no-build --target aarch64-unknown-linux-gnu

# Build Debian packages for all architectures
deb-all: deb-amd64 deb-arm64
