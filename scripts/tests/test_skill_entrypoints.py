from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SKILLS = ROOT / "skills"
MAX_ENTRY_LINES = 250
DESCRIPTIONS = {
    "zot": (
        "当用户要直接查询、提取、整理或安全更新本机已有的 Zotero 库或 reading workspace 时使用，"
        "即使用户没说 zot。覆盖库内搜索、citation key、PDF/批注/笔记、collection、saved search、"
        "feed、附件、语义索引、Scite、配置排障、Zotero 10+ Local API 字段/作者/分类创建与单 key 删除/单条标签、"
        "connector 导入 BibTeX/RIS，以及明确授权的 Web API 写入。不要用于泛化找论文、引用格式教学、"
        "非 Zotero PDF，也不要用于文献缺陷分析、创新点或 brainstorm 报告；后者使用 zot-brainstorm。"
    ),
    "zot-brainstorm": (
        "当用户要基于已有 Zotero collection、workspace 或明确 item key，做研究 brainstorming、"
        "缺陷/局限/gap analysis、创新方向或下一步选题，并生成可追溯的 Markdown/HTML 报告时使用，"
        "即使用户没说 zot-brainstorm。不要用于普通库内检索、单篇摘要、标签/笔记/分类维护、"
        "引用格式教学、脱离 Zotero 的网页找论文，或未经单独授权写入 Zotero/Obsidian；"
        "这些分别交给 zot 或停止。"
    ),
}
REQUIRED_ENTRY_TERMS = {
    "zot": ("## 调用顺序", "## 诊断", "## 安全", "## 输出契约"),
    "zot-brainstorm": ("read-only", "Evidence levels", "## Analysis Sequence", "## Report Output"),
}
LINK_RE = re.compile(r"\[([^\]]+)\]\(([^)]+)\)")


def cargo_version() -> str:
    match = re.search(
        r'^version = "(\d+\.\d+\.\d+)"',
        (ROOT / "Cargo.toml").read_text(encoding="utf-8"),
        re.MULTILINE,
    )
    if match is None:
        raise AssertionError("workspace package version not found")
    return match.group(1)


def skill_entries() -> dict[str, Path]:
    entries = {
        path.parent.name: path
        for path in SKILLS.glob("*/SKILL.md")
        if path.is_file()
    }
    nested = [
        path.relative_to(ROOT).as_posix()
        for path in SKILLS.rglob("SKILL.md")
        if path.parent.parent != SKILLS
    ]
    if nested:
        raise AssertionError(f"nested SKILL.md files found: {nested}")
    return entries


class SkillEntrypointTests(unittest.TestCase):
    def test_only_two_canonical_entries_exist(self):
        self.assertEqual(set(skill_entries()), {"zot", "zot-brainstorm"})

    def test_entries_match_description_and_routing_contract(self):
        version = cargo_version()
        for name, expected in DESCRIPTIONS.items():
            with self.subTest(skill=name):
                path = skill_entries()[name]
                lines = path.read_text(encoding="utf-8").splitlines()
                self.assertLessEqual(len(lines), MAX_ENTRY_LINES)
                descriptions = [
                    line for line in lines if line.startswith("description: ")
                ]
                self.assertEqual(len(descriptions), 1)
                self.assertEqual(
                    descriptions[0],
                    f"description: {expected} (v{version})",
                )
                body = "\n".join(lines)
                for term in REQUIRED_ENTRY_TERMS[name]:
                    self.assertIn(term, body)

    def test_relative_links_resolve_inside_each_package(self):
        for name, path in skill_entries().items():
            with self.subTest(skill=name):
                text = path.read_text(encoding="utf-8")
                links = [match.group(2) for match in LINK_RE.finditer(text)]
                self.assertGreaterEqual(len(links), 3)
                for link in links:
                    self.assertFalse(link.startswith(("http://", "https://", "/")))
                    target = (path.parent / link).resolve()
                    self.assertTrue(
                        target.is_relative_to(path.parent.resolve()),
                        f"{link} escapes {name}",
                    )
                    self.assertTrue(target.is_file(), f"missing link target: {link}")
                    self.assertGreater(target.stat().st_size, 0)


if __name__ == "__main__":
    unittest.main()
