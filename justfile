default:
  @just --list

version-check:
  cargo test -p zot-cli --test workspace_version_guard

check: version-check
  cargo check --workspace

fmt:
  cargo fmt --all --check

fmt-fix:
  cargo fmt --all

clippy:
  cargo clippy --workspace --all-targets -- -D warnings

test:
  cargo test --workspace

build:
  cargo build --release -p zot-cli

docs:
  npm --prefix docs install
  npm --prefix docs run dev

install:
  cargo install --path src/zot-cli --locked --force

install-local:
  cargo install --path src/zot-cli --locked --force

ci: fmt check clippy test
