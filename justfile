default:
    @just --list

build:
    cargo build --locked --bin agent-print-session
    mkdir -p bin
    cp target/debug/agent-print-session bin/agent-print-session

run *args:
    cargo run --locked --bin agent-print-session -- {{args}}

run-ai *args:
    AGENT=1 cargo run --locked --bin agent-print-session -- {{args}}

test:
    cargo test --locked --all-targets

fmt:
    cargo fmt --package agent-print-session-cli

lint:
    cargo fmt --package agent-print-session-cli -- --check
    cargo clippy --locked --all-targets -- -D warnings

coverage:
    cargo llvm-cov --locked --all-targets --fail-under-lines 90

check: lint test coverage
