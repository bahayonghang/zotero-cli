set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

default:
  @just --list

help:
  @just --list

version-check:
  cargo test -p zot-cli --test workspace_version_guard --locked

check: version-check
  cargo check --workspace --locked

fmt:
  cargo fmt --all --check

fmt-fix:
  cargo fmt --all

clippy:
  cargo clippy --workspace --all-targets --locked -- -D warnings

test:
  cargo test --workspace --locked

build:
  cargo build --release -p zot-cli --locked

docs:
  npm --prefix docs install
  npm --prefix docs run dev

[script("python")]
version-sync:
  import re
  from pathlib import Path

  cargo = Path("Cargo.toml").read_text(encoding="utf-8")
  m = re.search(r'^version = "(\d+\.\d+\.\d+)"', cargo, re.MULTILINE)
  version = m.group(1)
  for sk in sorted(Path("skills").rglob("SKILL.md")):
      lines = sk.read_text(encoding="utf-8").splitlines(keepends=True)
      for i, line in enumerate(lines):
          if line.startswith("description: "):
              line = re.sub(r'\s*\(v\d+\.\d+\.\d+\)', "", line)
              lines[i] = f"{line.rstrip()} (v{version})\n"
      sk.write_text("".join(lines), encoding="utf-8")
  print(f"version-sync: synced v{version} to skills/*/SKILL.md")

install: install-local skills-sync

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

skills-sync: _install-skills

skills-check:
  python scripts/check_skill_mirrors.py
  python -m unittest discover -s scripts/tests -p "test_*.py"

ci-check: fmt check clippy test skills-check

ci: ci-check
