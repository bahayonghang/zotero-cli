# 收敛边界错误构造:require_item 等命名构造器

## Goal

zot-cli 中 63 处内联 `ZotError` 构造里的重复语义收敛:同一个「条目不存在」错误手写了 6 遍,「取附件 + 拒绝非 PDF」校验块逐字复制 2 处。提供命名构造器/helper,让同一行为只写一次;错误码对外保持稳定。

## Problem & Evidence(2026-07-07 grep 复核)

- 内联 `ZotError` 构造共 63 处(48 InvalidInput / 8 Io / 4 Remote / 2 Pdf / 1 Unsupported)
- `item-not-found` ×6:read.rs:19、:54、:257、:278;scite.rs:62;tag.rs:13
- 「fetch attachment + 拒绝非 PDF」块逐字复制:annotation.rs:84-98 与 :149-166
- 其余高频码:`item-no-pdf` ×3、`attachment-not-found` ×3、`invalid-doi` ×3、`attachment-not-pdf` ×2
- spec 约束:错误码是对外契约(`.trellis/spec/zot-cli/backend/error-handling.md`)——收敛构造不得改变 code / message / hint 语义

## Requirements

- 提供 `require_item` / `require_pdf_attachment`(或 `ZotError::not_found` 类命名构造器);放置层级(zot-local vs zot-cli util)按 spec 的层职责在实施时判定
- 全部重复点迁移;错误码、message、hint 字节不变

## Acceptance Criteria

- [ ] grep `"item-not-found"` 字面构造 ≤ 1 处(构造器内部)
- [ ] annotation.rs 两个复制块合一
- [ ] JSON 错误输出不变(契约测试);`cargo clippy` / `cargo test` 全绿

## Notes

- 轻量:PRD-only;可与 07-07-envelope-err 同批完成。
- 父任务:07-07-arch-deepening(评审小信号 2)。
