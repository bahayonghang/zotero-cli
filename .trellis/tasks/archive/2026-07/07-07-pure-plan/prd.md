# 按 merge.rs 范本拆出 pure plan 函数

## Goal

把困在「验证 + I/O + 变换 + 打印」handler 体内的纯决策逻辑拆成 deep pure plan 函数 + 薄 async 壳,使 interface = test surface。范本在库内:`item/merge.rs` 的 `build_merge_execution_plan`(:69-182,Value 进/计划出,113 行)+ 3 个测试(:331-457),壳 `merge_item_set`(:28-67)只做 I/O。

## Problem & Evidence(2026-07-07 核实)

- `write.rs::add_item_by_url`(:219-261):DOI/arXiv 规范化 → 3 种 remote payload 构造 → 条件附 PDF;纯 mapper `build_crossref_item_payload` / `build_arxiv_item_payload` / `crossref_type_to_zotero`(:394-455)**零测试**
- `write.rs::maybe_attach_pdf_url`(:354-389)在 CLI 层直接 `runtime.client().get(url)` + 临时文件 + 上传——疑似越过 database-guidelines 的「remote mutation 属于 zot-remote」线,归属在设计时定夺
- `sync.rs:34-52`:`PublicationStatus`→fields map,纯、承重、零测试(sync 整个 handler :13-65 一体)
- `scite.rs:94-157`:zip/filter 纯逻辑缠绕 2 次 I/O,零测试
- 9 个 handler 文件零测试的大背景见父任务与 07-07-library-seam

## Requirements

- write / sync / scite 各拆出纯 plan/map 函数(输入数据 → 输出计划/载荷),壳按 `merge_item_set` 形状重写
- 每个拆出的纯函数获得直接单元测试(含边界:无 DOI、非 preprint、空 tallies 等)
- 行为不变:载荷字节级等价,或差异在测试中显式断言
- `--confirm` / `--apply` 预览门语义不变(quality-guidelines mutation gates:preview 永不谎称已应用)

## Acceptance Criteria

- [ ] write.rs 三个 payload mapper、sync.rs 字段映射、scite.rs zip/filter 均有单元测试
- [ ] 壳函数不再包含载荷决策(抽查:壳内无字段名字符串拼装)
- [ ] 既有测试全绿;`cargo clippy` / `cargo test` 全绿

## Notes

- 中等复杂度:PRD + 简短 design.md 即可(范本在库内,风格零分歧)。
- 顺序:建议在 07-07-cmd-output 之后(壳更薄);`maybe_attach_pdf_url` 若需下沉 zot-remote,与 07-07-remote-transport 协调。
- 父任务:07-07-arch-deepening(评审候选 E)。
