# Implement: CommandOutput 迁移执行清单

前置:07-07-envelope-err 已完成(`CliEnvelope::err(&ZotError)` 可用,
format.rs 已确认)。工作目录 `src/zot-cli/`。

基线验证(开工前记录):

```bash
grep -rn "if ctx.json" src/zot-cli/src/commands/ | wc -l   # 期望 85
grep -rn "print_enveloped(" src/zot-cli/src/ | wc -l        # 期望 85+定义
cargo test -p zot-cli && cargo clippy -p zot-cli -- -D warnings
```

## 批 1:建模块 + dispatch 适配 + graph.rs 试点

1. 新建 `src/zot-cli/src/output.rs`:`CommandOutput` / `Payload` /
   `new` / `raw` / `silent` / `emit` / `as_json`(形状见 design.md §1);
   在 crate 树注册 `mod output;`。
2. output.rs 单测:
   - `json_ctx` 下 `new(...).as_json()` 与 `print_enveloped` 输出字符串
     全等(对照测试,含 seed 与 `json!` 两种 payload);
   - human ctx 下 `as_json()` 为 None;
   - meta 注入 profile/api_version 断言。
3. 改 `commands/mod.rs`:dispatch 收集 `CommandOutput` 后统一 `emit()`;
   加临时适配 `fn legacy(r: Result<()>) -> Result<CommandOutput>`
   (返回 `silent()`)包住未迁移的臂;Completions 臂返回 `silent()`。
4. 迁移 `graph.rs`(1 处):`handle` 返回 `Result<CommandOutput>`,
   mod.rs 该臂去掉 legacy。

验证:

```bash
cargo test -p zot-cli
grep -rn "if ctx.json" src/zot-cli/src/commands/ | wc -l   # 期望 84
```

回滚点:git commit(批 1 完成)。若对照单测发现字节差异,停止并修 output.rs。

## 批 2:小文件五连(sync/doctor/scite/tag/annotation)

5. 依次迁移 `sync.rs`(1)、`doctor.rs`(1)、`item/scite.rs`(3)、
   `item/tag.rs`(4)、`item/annotation.rs`(4);每文件:handler 签名改
   `Result<CommandOutput>`,机械替换
   `if ctx.json { print_enveloped(ctx,X,S) } else { H }` →
   `CommandOutput::new(ctx, &X, S, move || H)`(注意所有权 move,
   借用改克隆或提前结构调整);item/mod.rs 子分发透传;
   mod.rs 拆对应 legacy。

验证:

```bash
cargo test -p zot-cli && cargo clippy -p zot-cli -- -D warnings
grep -rn "if ctx.json" src/zot-cli/src/commands/ | wc -l   # 期望 71
```

回滚点:git commit(批 2)。

## 批 3:note/config/write

6. 迁移 `item/note.rs`(5)、`config.rs`(5)、`item/write.rs`(9)。
   config 注意不打印未脱敏 key(逻辑不变,仅搬分支)。

验证:同上,计数期望 52。回滚点:git commit(批 3)。

## 批 4:collection + item/read

7. 迁移 `collection.rs`(12)、`item/read.rs`(12)。
   `handle_pdf` 大文本 payload:human 分支闭包 move text,JSON 分支
   构造即序列化,行为与现状一致。

验证:同上,计数期望 28。回滚点:git commit(批 4)。

## 批 5:library.rs + handler 返回值单测

8. 迁移 `library.rs`(17),`handle_saved_search` 透传 CommandOutput。
9. 新增 ≥3 个 handler 返回值单测(不经 stdout):
   - `semantic_status` 字段断言;
   - json ctx 下某 handler 返回的 `CommandOutput::as_json()` 包含
     `"ok": true` 与预期字段(如 saved-search delete payload);
   - 既有测试保持全绿。

验证:同上,计数期望 11。回滚点:git commit(批 5)。

## 批 6:workspace.rs + export 契约修复 + 收尾

10. 迁移 `workspace.rs`(11);`export_workspace`:
    - `"json"`:json 模式 envelope items;非 json 模式
      `CommandOutput::raw(to_pretty_json(&items)?)`(字节不变);
    - `"bibtex"` / markdown:json 模式
      `CommandOutput::new(ctx, &json!({"format":..,"content":..}), None, ...)`,
      非 json 模式 human 闭包输出原文本(现有 println 逻辑搬入闭包)。
11. 为 export bibtex/markdown 增加单测:json ctx 下 `as_json()` 含
    `"format"` 与 `"content"`;非 json 下不含。
12. 删除 `format.rs::print_enveloped` 迁移残留(`EnvelopeMetaSeed` 移入
    output.rs 或原地保留仅供 output 使用);删除 mod.rs 全部 legacy 适配;
    对照单测改为固定字面量断言。
13. 更新 `.trellis/spec/zot-cli/backend/logging-guidelines.md`:
    - 成功输出规则改为「handler 返回 CommandOutput,dispatch emit」;
    - 补记文本导出格式的 `{format, content}` envelope 约定;
    - 代码示例更新。

最终验证(对照验收标准):

```bash
grep -rn "if ctx.json" src/zot-cli/src/commands/ | wc -l     # ≤2(目标 0)
grep -rn "print_enveloped(" src/zot-cli/src/ | wc -l          # 0
cargo test --workspace                                        # 全绿
cargo clippy --workspace -- -D warnings                       # 全绿
```

## 验收标准对照(prd.md)

| 验收项                                       | 覆盖步骤                               |
| -------------------------------------------- | -------------------------------------- |
| `if ctx.json` 在 commands/ ≤2                | 批 1-6 逐批消减,最终 grep 验证(目标 0) |
| CLI 解析与 JSON 契约测试全绿、无格式回归     | 每批 cargo test;批 1 字节对照单测      |
| workspace export --json 走 envelope 或记豁免 | 批 6 步骤 10/11/13(决定:走 envelope)   |
| ≥3 个 handler 返回值直接单测                 | 批 5 步骤 9 + 批 6 步骤 11             |
| clippy / test 全绿                           | 每批验证 + 最终验证                    |

## 回滚点

- 每批一个 git commit;任一批验证失败,`git revert`/`git reset` 回上一批。
- 高风险点:批 1(形状定错全局返工——故先单文件试点+字节对照测试)、
  批 6(export 契约变更——独立 commit,可单独 revert 保留其余成果)。
