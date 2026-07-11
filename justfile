set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

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

install: install-local _install-skills

install-local:
  cargo install --path src/zot-cli --locked --force

[private]
[script("python")]
_install-skills:
  import shutil
  from pathlib import Path

  source = Path("skills")
  for target in (Path(".agents/skills"), Path(".claude/skills")):
      target.mkdir(parents=True, exist_ok=True)
      for skill in source.iterdir():
          if skill.is_dir():
              destination = target / skill.name
              if destination.exists():
                  shutil.rmtree(destination)
              shutil.copytree(skill, destination)

ci: fmt check clippy test
