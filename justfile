set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# 列出全部配方
default:
  @just --list

# 列出全部配方及中文说明
help:
  @just --list

# 检查工作区版本守卫：内部依赖、lint 继承、CHANGELOG 版本
version-check:
  cargo test -p zot-cli --test workspace_version_guard --locked

# 跑版本守卫，再对锁定依赖做全工作区编译检查
check: version-check
  cargo check --workspace --locked

# 检查 Rust 格式，不修改文件
fmt:
  cargo fmt --all --check

# 按 rustfmt 格式化全部 Rust 代码
fmt-fix:
  cargo fmt --all

# 以警告即错误运行 Clippy（锁定依赖，含全部 target）
clippy:
  cargo clippy --workspace --all-targets --locked -- -D warnings

# 运行锁定依赖的全工作区测试
test:
  cargo test --workspace --locked

# 编译 zot-cli 的 release 二进制
build:
  cargo build --release -p zot-cli --locked

# 安装文档依赖并启动 VitePress 本地预览
docs:
  npm --prefix docs install
  npm --prefix docs run dev

# 把 Cargo.toml 版本写入 skills/*/SKILL.md（会改文件）
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

# 安装本机 zot，并同步 skill 镜像
install: install-local skills-sync

# 用 cargo install 强制安装本机 zot
install-local:
  cargo install --path src/zot-cli --locked --force

# 把 skills/ 复制到 .agents/skills 与 .claude/skills（会改文件）
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

# 把 skills/ 同步到 .agents/skills 与 .claude/skills（会改文件）
skills-sync: _install-skills

# 检查 canonical skill 镜像，并运行 scripts 单测
skills-check:
  python scripts/check_skill_mirrors.py --skip-if-all-missing
  python -m unittest discover -s scripts/tests -p "test_*.py"

# 只读门禁：fmt、check、clippy、test、skill 镜像检查
ci-check: fmt check clippy test skills-check

# 本地验证入口，等价于 ci-check，不改文件
ci: ci-check
