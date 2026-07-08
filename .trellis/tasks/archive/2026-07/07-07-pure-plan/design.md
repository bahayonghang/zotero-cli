# Design: 按 merge.rs 范本拆出 pure plan 函数(简短)

范本:`item/merge.rs` —— `build_merge_execution_plan`(纯,Value 进/计划出)+
`merge_item_set` 薄壳(只做 I/O)。本任务把同形状施加到 write / sync / scite。

## 拆分形状

### write.rs(add_item_by_url 路径)

- 纯函数 `plan_add_item(source: &ItemSource, meta: ...) -> AddItemPlan`:
  DOI/arXiv 规范化 + 三种 remote payload 构造(既有纯 mapper
  `build_crossref_item_payload` / `build_arxiv_item_payload` /
  `crossref_type_to_zotero` 保持原位,纳入测试)+ 是否附 PDF 的**决策**
  (输出 `pdf_url: Option<String>`,不执行下载)。
- 壳 `add_item_by_url`:取 meta(I/O)→ 调 plan → 按 plan 执行 remote 写入
  与可选 PDF 附加(I/O)。
- **`maybe_attach_pdf_url` 归属决定:本任务不动**。它的 HTTP 执行体留在原处,
  只把「要不要附、附哪个 url」的判断进 plan;下沉 zot-remote 的问题归
  07-07-remote-transport 处理(该任务 PRD 已覆盖 transport seam)。

### sync.rs

- 纯函数 `publication_status_fields(status: &PublicationStatus) -> Vec<(String, String)>`
  (或等价 map 形状):`:34-52` 的字段映射原样搬出。
- 壳 `handle`:读状态(I/O)→ 调映射 → 写字段(I/O)→ CommandOutput。

### scite.rs

- 纯函数 `pair_tallies(items: &[...], tallies: &[...]) -> Vec<...>`(zip/filter
  逻辑,`:94-157` 中缠绕在两次 I/O 之间的部分)。
- 壳:fetch items(I/O)→ fetch tallies(I/O)→ 纯 pair → 输出。

## 测试计划(每个纯函数直接单测)

- write:无 DOI、arXiv id 带版本号、crossref type 未知回退、有/无 PDF url。
- sync:非 preprint(空 map)、preprint 全字段、部分字段缺失。
- scite:空 tallies、条目多于 tallies、完全匹配。
- 载荷等价:对既有 mapper 的输出用固定输入断言 JSON 字段(行为不变基线)。

## 不变式

- `--confirm`/`--apply` 预览门语义不变(纯函数只产计划,壳负责门控与执行)。
- 壳内不得出现字段名字符串拼装(验收抽查点)。
