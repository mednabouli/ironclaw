# IronClaw task runner — install: cargo install just
# Usage: just <recipe>

# Default: show available recipes
default:
    @just --list

# ── Quality ───────────────────────────────────────────────────────────────

# Check all crates compile
check:
    cargo check --workspace --all-features

# Run the full test suite
test:
    cargo test --workspace --all-features

# Run tests excluding E2E (no Ollama required)
test-fast:
    cargo test --workspace --all-features -- --skip e2e

# Lint with clippy — zero warnings
lint:
    cargo clippy --workspace --all-features -- -D warnings

# Format all code
fmt:
    cargo fmt --all

# Check formatting without changing files (used in CI)
fmt-check:
    cargo fmt --all -- --check

# License + CVE check
deny:
    cargo deny check

# Generate and open docs
docs:
    cargo doc --workspace --no-deps --open

# Code coverage report
cov:
    cargo llvm-cov --workspace --all-features --html
    open target/llvm-cov/html/index.html

# ── Build ─────────────────────────────────────────────────────────────────

# Debug build
build:
    cargo build --workspace

# Release build (optimised, stripped)
release:
    cargo build --release

# Show release binary size
size: release
    @ls -lh target/release/ironclaw | awk '{print "Binary size: " $5}'
    @file target/release/ironclaw

# Binary size audit — top crate contributors (requires: cargo install cargo-bloat)
bloat:
    cargo bloat --release --crates -p ironclaw-cli -n 20

# Detailed binary size audit with per-function breakdown
bloat-full:
    cargo bloat --release -p ironclaw-cli -n 40

# Cross-compile for x86_64 Linux musl (static)
cross-x86:
    cross build --release --target x86_64-unknown-linux-musl -p ironclaw-cli

# Cross-compile for ARM64 Linux (requires: cargo install cross)
cross-arm:
    cross build --release --target aarch64-unknown-linux-musl -p ironclaw-cli

# Cross-compile for ARMv7 (Raspberry Pi 2/3)
cross-armv7:
    cross build --release --target armv7-unknown-linux-gnueabihf -p ironclaw-cli

# Cross-compile macOS x86_64 (native on Intel Mac, requires target on Apple Silicon)
cross-macos-x86:
    cargo build --release --target x86_64-apple-darwin -p ironclaw-cli

# Cross-compile macOS Apple Silicon (native on AS, requires target on Intel Mac)
cross-macos-arm:
    cargo build --release --target aarch64-apple-darwin -p ironclaw-cli

# Cross-compile all 5 targets (Linux via cross, macOS native)
cross-all: cross-x86 cross-arm cross-armv7 cross-macos-x86 cross-macos-arm
    @echo "✅ All 5 targets built"

# Package all cross-compiled binaries into release archives
package-all: cross-all
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p dist
    for target in x86_64-unknown-linux-musl aarch64-unknown-linux-musl armv7-unknown-linux-gnueabihf; do
        strip "target/$target/release/ironclaw" 2>/dev/null || true
        tar czf "dist/ironclaw-$target.tar.gz" -C "target/$target/release" ironclaw
        shasum -a 256 "dist/ironclaw-$target.tar.gz" > "dist/ironclaw-$target.tar.gz.sha256"
    done
    for target in x86_64-apple-darwin aarch64-apple-darwin; do
        strip "target/$target/release/ironclaw" 2>/dev/null || true
        tar czf "dist/ironclaw-$target.tar.gz" -C "target/$target/release" ironclaw
        shasum -a 256 "dist/ironclaw-$target.tar.gz" > "dist/ironclaw-$target.tar.gz.sha256"
    done
    ls -lh dist/
    @echo "✅ All archives in dist/"

# ── Run ───────────────────────────────────────────────────────────────────

# Interactive chat
chat:
    cargo run --bin ironclaw -- chat

# Health check
health:
    cargo run --bin ironclaw -- health

# List configuration
list:
    cargo run --bin ironclaw -- list

# One-shot prompt
run PROMPT:
    cargo run --bin ironclaw -- run "{{PROMPT}}"

# Doctor check
doctor:
    cargo run --bin ironclaw -- doctor

# ── Dev helpers ───────────────────────────────────────────────────────────

# Pull recommended Ollama models
pull-models:
    ollama pull llama3.2
    ollama pull nomic-embed-text

# Start Ollama (macOS)
ollama:
    ollama serve &

# Start with Docker Compose (Ollama + IronClaw)
docker-up:
    docker compose up --build

# ── CI simulation ─────────────────────────────────────────────────────────

# Run everything CI runs — use before pushing
ci: fmt-check lint test deny
    @echo "✅ All CI checks passed"

# Full pre-release check
pre-release: ci docs size
    @echo "✅ Ready to release"

# ── Maintenance ───────────────────────────────────────────────────────────

# Remove build artifacts
clean:
    cargo clean

# Update all dependencies
update:
    cargo update

# Audit dependencies for known vulnerabilities
audit:
    cargo audit

# Generate CHANGELOG.md from conventional commits using git-cliff
changelog:
    git cliff --output CHANGELOG.md

# Preview changelog for unreleased changes
changelog-preview:
    git cliff --unreleased
