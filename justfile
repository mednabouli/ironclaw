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

# Cross-compile for ARM64 Linux (requires: cargo install cross)
cross-arm:
    cross build --release --target aarch64-unknown-linux-musl

# Cross-compile for ARMv7 (Raspberry Pi 2/3)
cross-armv7:
    cross build --release --target armv7-unknown-linux-musleabihf

# Cross-compile for all targets
cross-all: cross-arm cross-armv7

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
