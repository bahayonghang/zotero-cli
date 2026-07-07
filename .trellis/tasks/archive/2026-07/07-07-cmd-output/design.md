# Design: 收敛 85 处输出分支为 CommandOutput 模块

## 背景

`if ctx.json` 与 `print_enveloped(` 各 85 处,散布在 13 个命令文件中(library 17、
item/read 12、collection 12、workspace 11、item/write 9、note/config 各 5、
tag/annotation 各 4、scite 3、sync/graph/doctor 各 1)。envelope meta 组装一半在
`format.rs:print_enveloped`、一半涂抹在各调用点(`EnvelopeMetaSeed`)。
`workspace.rs:185-206` 的 bibtex/markdown 导出在 `--json` 下直接 `println!`,
违反 envelope 契约。

## 1. CommandOutput 的类型形状

### 推荐方案:惰性双路 struct,构造时接收 ctx,内部一次分支

新建 `src/zot-cli/src/output.rs`:

```rust
use crate::context::AppContext;
use crate::format::{EnvelopeMetaSeed, to_pretty_json, ENVELOPE_API_VERSION};
use zot_core::{CliEnvelope, EnvelopeMeta};

/// 命令成功输出。handler 返回它,dispatch 层调用 `emit()` 打印。
pub struct CommandOutput(Payload);

enum Payload {
    /// 完整渲染好的 pretty JSON envelope(仅 ctx.json 时构造)
    Json(String),
    /// 人类可读渲染闭包(仅非 json 时构造;捕获所有权数据,延迟到 emit 执行)
    Human(Box<dyn FnOnce() + Send>),
    /// 模式无关的原样文本(workspace export bibtex/markdown、item export 等
    /// 非 json 模式下的纯文本;json 模式下不使用此变体)
    Raw(String),
    /// 无输出(completions、mcp serve 等自行写 stdout 的命令)
    Silent,
}

impl CommandOutput {
    /// 唯一的 json/human 决策点 + 唯一的 meta 组装点。
    pub fn new<T, F>(
        ctx: &AppContext,
        data: &T,
        seed: Option<EnvelopeMetaSeed>,
        human: F,
    ) -> anyhow::Result<Self>
    where
        T: serde::Serialize,
        F: FnOnce() + Send + 'static,
    {
        if ctx.json {
            let seed = seed.unwrap_or_default();
            let meta = EnvelopeMeta {
                count: seed.count,
                total: seed.total,
                profile: ctx.profile.clone(),
                api_version: Some(ENVELOPE_API_VERSION),
            };
            Ok(Self(Payload::Json(to_pretty_json(
                &CliEnvelope::ok_with_meta(data, meta),
            )?)))
        } else {
            Ok(Self(Payload::Human(Box::new(human))))
        }
    }

    pub fn raw(text: String) -> Self { Self(Payload::Raw(text)) }
    pub fn silent() -> Self { Self(Payload::Silent) }

    /// dispatch 层唯一的打印动作。
    pub fn emit(self) {
        match self.0 {
            Payload::Json(json) | Payload::Raw(json) => println!("{json}"),
            Payload::Human(render) => render(),
            Payload::Silent => {}
        }
    }

    /// 测试专用:直接断言 JSON envelope 内容,不经 stdout。
    pub fn as_json(&self) -> Option<&str> {
        match &self.0 {
            Payload::Json(json) => Some(json),
            _ => None,
        }
    }
}
```

> **批 1 落地修正**:`new` 的 data 改为按值传入(`T: Serialize + Send + 'static`),
> human 闭包签名为 `FnOnce(&T)`——原设计「借 data + move 同一数据进闭包」无法通过
> 借用检查。JSON 分支序列化 `&data` 后丢弃;human 分支 move data 进 boxed 闭包、
> 渲染时传引用。后续批迁移一律按此签名(借用改克隆或提前取所有权)。

handler 侧调用形状(以 library search 为例):

```rust
LibraryCommand::Search(args) => {
    let result = library.search(...)?;
    let seed = EnvelopeMetaSeed {
        count: Some(result.items.len()),
        total: Some(result.total),
    };
    let items = result.items;
    CommandOutput::new(ctx, &items, Some(seed), move || print_items(&items))
}
```

注意:human 闭包 move 捕获数据所有权;JSON 分支下闭包被丢弃、零渲染成本;
human 分支下不做任何序列化,与现状成本一致。

### 关键设计判断

**human 渲染用闭包,不用 trait、不用预渲染 String:**

- 被否:`trait HumanRender { fn render(&self) -> String }` 按 payload 类型实现
  —— 85 处输出对应几十种 payload 形状(大量匿名 `serde_json::json!` 对象),
  每种都要 newtype + impl,样板爆炸;且 `json!` 值走 trait 需二次建模。
- 被否:预渲染 `String` —— 现有 `format.rs` 的 `print_items` /
  `print_item` / `print_collections` 等 helper 全部基于 `println!`,改成
  `fmt::Write` 累积字符串是一次大范围重写,回归风险高且与本任务目标
  (收敛分支)无关;此外 human 分支在 JSON 模式下会白白渲染。闭包让这些
  helper 一行不改。
- 采纳:`Box<dyn FnOnce() + Send>` —— 迁移 diff 最小(else 分支原样搬进闭包),
  惰性执行,JSON 模式零成本。

**JSON 在构造时立刻渲染成 String,不保存 `serde_json::Value`:**

- 被否:`data: serde_json::Value` —— workspace 未启用 serde_json 的
  `preserve_order` feature(见根 Cargo.toml),`Value::Object` 是 BTreeMap,
  经 `to_value` 中转会把 struct 字段按字母序重排,**破坏字节级不变**。
- 被否:泛型 `CommandOutput<T>` —— dispatch 层 match 各臂返回类型不一,
  需要装箱或 enum,复杂度回到原点;`erased_serde` 可行但引新依赖,收益不抵。
- 采纳:构造时用与今日 `print_enveloped` 完全相同的代码路径
  (同一 `CliEnvelope::ok_with_meta` + `to_pretty_json`,直接序列化原始
  typed 值)渲染成 String,序列化路径逐字节等价。

**meta 组装集中一处:** `EnvelopeMetaSeed`(count/total)由 handler 提供——只有
handler 知道 count/total 语义;profile 与 api_version 由 `CommandOutput::new`
从 ctx 统一注入。即 meta 的「环境部分」集中在 output.rs 一处,「数据部分」随
数据给出。`format.rs::print_enveloped` 迁移完成后删除。

**构造时需要 `&AppContext`:** 分支发生在 `CommandOutput::new` 内部
(output.rs),不在 commands/ 下,满足验收标准 grep `if ctx.json` ≤2。
被否方案「handler 返回纯数据、dispatch 层拿 ctx 再分支」要求 dispatch 能拿到
每个 payload 的类型,回到泛型困境;且 handler 本就持有 ctx,不增加耦合。

## 2. dispatch 层的唯一分支与错误路径

- 全部 handler 签名从 `-> Result<()>` 改为 `-> Result<CommandOutput>`;
  嵌套子分发(`item::handle` → read/write/... 、`handle_saved_search` 等)
  同样透传 `Result<CommandOutput>`。
- `commands/mod.rs::dispatch`:

```rust
pub(crate) async fn dispatch(ctx: &AppContext, command: Commands) -> Result<()> {
    let output = match command {
        Commands::Doctor => doctor::handle(ctx).await?,
        ...
        Commands::Completions { shell } => {
            clap_complete::generate(...);
            CommandOutput::silent()
        }
    };
    output.emit();
    Ok(())
}
```

- 分支物理位置:`output.rs::CommandOutput::new`(一处);打印动作:
  `dispatch` 末尾 `emit()`(一处)。commands/ 下 `if ctx.json` 归零。
- **错误路径不变**:handler 内 `?` 早退,`ZotError` 经 anyhow 下沉到
  main.rs,由 `format::print_error`(内部用新的 `CliEnvelope::err(&ZotError)`)
  统一打印。`print_error` 保留在 format.rs,不纳入 CommandOutput——错误
  发生在 dispatch 之外(context 构建等)也要能打印。
- doctor 的人类模式进度输出:若存在检查过程中的即时打印,保持现状
  (logging-guidelines 允许非 JSON 模式的过程文本),仅末尾汇总走 CommandOutput。

## 3. workspace export bibtex/markdown 在 --json 下的决定

**决定:走 envelope,payload 形状对齐 `item export`:
`{"format": "bibtex", "content": "..."}`(markdown 同理)。**

判断依据:

1. **管道用法不受影响**:纯文本管道场景
   (`zot workspace export -f bibtex > refs.bib`)本就不加 `--json`;
   默认(非 json)模式输出保持纯文本不变。`--json` 是调用方显式 opt-in,
   此时返回裸文本反而违背调用方预期(agent 按 envelope 契约解析会失败)。
2. **一致性**:`item/read.rs::handle_export` 对同类 bibtex 导出已在 `--json`
   下走 envelope(`{"format", "content"}`),workspace export 是孤例违约,
   记豁免会让契约出现「看格式而定」的例外,增加 agent 侧解析复杂度。
3. **不违反字节级不变约束**:该处当前在 `--json` 下输出的不是 JSON,
   属于契约缺陷修复,不在「既有 JSON 契约」保护范围;prd 已明示
   「workspace export 例外按设计决定处理」。

实施:`export_workspace` 三臂统一为——json 模式
`CommandOutput::new(ctx, &payload, seed, ...)`;非 json 模式经 human 闭包
输出原文本(bibtex 拼接串 / markdown 拼接串)。`"json"` 格式臂维持现状语义
(json 模式 envelope 包 items;非 json 模式裸 pretty JSON items,用
`CommandOutput::raw` 承载以保持字节不变)。
同步在 `.trellis/spec/zot-cli/backend/logging-guidelines.md` 补记:
文本导出格式在默认模式输出裸文本、在 `--json` 下包 `{format, content}`
envelope 的约定。

> **批 6 落地修正**:(1)export "json" 格式臂不用 `raw`——那需要调用点分支,
> 违反 grep=0;改为预渲染 `to_pretty_json(&items)?` 后
> `CommandOutput::new(ctx, items, None, move |_| println!("{rendered}"))`,
> json 模式闭包被丢弃、非 json 模式输出裸 pretty 数组,字节不变。
> (2)`Payload::Raw` 与 `raw()` 因此全程无消费者,已整体删除,
> Payload 只留 Json/Human/Silent。

## 4. JSON 成功输出字节级不变的保障

1. **同一序列化路径**:`CommandOutput::new` 的 JSON 分支是
   `print_enveloped` 函数体的逐字搬移(同一 `EnvelopeMeta` 构造、同一
   `CliEnvelope::ok_with_meta`、同一 `to_pretty_json`),直接序列化 handler
   手里的原始 typed 值/`json!` 值,不经 `serde_json::Value` 中转,无键序风险。
2. **迁移期对照测试**:在 output.rs 新增单测,对代表性 payload
   (含 meta seed 的 items 列表、`json!` 匿名对象)断言
   `CommandOutput::new(json_ctx, ...).as_json()` 与旧
   `print_enveloped` 等价字符串完全相等(`assert_eq!` 全串),该测试在
   `print_enveloped` 删除前持续存在,删除时改为对固定字面量断言
   (形如 format.rs 现有 `serializes_error_envelope_byte_exact`)。
3. **既有契约测试全绿**:CLI 解析测试、JSON 契约测试、
   `tests/workspace_version_guard.rs` 每批迁移后运行。
4. **各批迁移是机械变换**:`if ctx.json { print_enveloped(ctx, X, S) } else { H }`
   → `CommandOutput::new(ctx, &X, S, move || H)`,X、S 表达式不动。

## 5. 13 个文件的渐进迁移策略

每批独立可编译、可测试、可提交;grep 计数单调下降。签名改动
(`Result<()>` → `Result<CommandOutput>`)按文件为单位整体切换。
采用**自底向上、按 dispatch 臂切换**:第 1 批同时改 mod.rs 与 graph.rs,
其余臂暂时包一层 `legacy(handler_result)` 适配器
(`.map(|()| CommandOutput::silent())`),每迁一个文件拆一个适配器,
最后一批删适配器。

| 批  | 内容                                                                                                           | 分支数消减 |
| --- | -------------------------------------------------------------------------------------------------------------- | ---------- |
| 1   | 新建 output.rs(+对照单测);改 mod.rs dispatch;迁移 graph.rs 验证形状                                            | 1          |
| 2   | sync.rs、doctor.rs、item/scite.rs、item/tag.rs、item/annotation.rs                                             | 1+1+3+4+4  |
| 3   | item/note.rs、config.rs、item/write.rs                                                                         | 5+5+9      |
| 4   | collection.rs、item/read.rs                                                                                    | 12+12      |
| 5   | library.rs(17 处,含 3 个 handler 返回值单测)                                                                   | 17         |
| 6   | workspace.rs(11 处 + export 契约修复 + spec 契约补记);删除 `format::print_enveloped` 与 legacy 适配器;doc 收尾 | 11         |

单元测试目标(≥3 个 handler 返回值直接测试,不经 stdout):

- `library::semantic_status`(已返回 `SemanticIndexStatus`,直接断言字段)
- `library.rs` 中构造 json ctx 的 `CommandOutput::as_json()` 断言
  (如 saved-search create/delete payload)
- `workspace::export_workspace` 的 bibtex/markdown payload
  (断言 `{format, content}` envelope)
- output.rs 自身的 new/emit/meta 组装测试
