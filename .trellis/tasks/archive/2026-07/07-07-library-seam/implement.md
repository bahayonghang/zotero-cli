# library-seam 实施计划

定案:D1 数据域 / D2 首轮 19 方法(5 新 trait)/ D3 混合分发 / D4 remote 另立 / D5 doctor 保留 / D6 复用 RagLibrary。详见 design.md「定案后事实刷新」。

## W0 地基(zot-local)

- [ ] 新建 `src/zot-local/src/library_traits.rs`:5 个 trait,方法签名与 db.rs inherent fn 同形
  - ItemReader:get_item / list_items / search / get_recent_items / get_recent_items_by_count
  - CollectionNav:get_collections / get_collection / get_subcollections / search_collections
  - CollectionContent:get_collection_items / get_collection_item_count / get_collection_tags
  - NoteReader:get_notes / search_notes
  - AttachmentSource:get_attachments / get_attachment_by_key / get_pdf_attachment / attachment_path / pdf_path
- [ ] `impl <trait> for LocalLibrary` 薄委托(同 rag_engine.rs:32 模式,inherent 优先解析,无递归)
- [ ] pdf.rs:blanket impl `impl<T: PdfBackend + ?Sized> PdfBackend for Arc<T>`(全方法显式转发,含默认方法 extract_doi,避免旁路 override)
- [ ] lib.rs:`pub mod library_traits;` + re-export 5 trait
- 验证:`cargo check -p zot-local`

## W1 Pdfium 收敛(zot-cli,AC3)

- [ ] context.rs:AppContext 加字段 `pdf: Arc<dyn PdfBackend + Send + Sync>`;from_cli 默认 `Arc::new(PdfiumBackend)`;derive(Debug) 改手写 impl Debug(Clone 保留 derive);新增 `pdf_backend()` 返回 Arc clone
- [ ] 7 处构造点改 `ctx.pdf_backend()`:library.rs:301、workspace.rs:196、annotation.rs:90/:141、read.rs:71/:192、write.rs:234;随手清 6 文件的 PdfiumBackend import
- [ ] doctor.rs:27 保留具体构造(D5 甲,依赖 inherent `status()`)
- [ ] 既有测试 ctx 构造补 pdf 字段:output.rs:87、workspace.rs:251
- 验证:`cargo clippy -p zot-cli --all-targets`;grep PdfiumBackend 构造 = context.rs 1 + doctor.rs 1
- 回滚点:commit(W0+W1 可合并)

## W2 semantic 对等(zot-local,AC2)

- [ ] semantic.rs:126 `search(&self, library: &LocalLibrary, ...)` → `search<L: ItemReader + CollectionContent>(&self, library: &L, ...)`(调用方 library.rs:147 传 &LocalLibrary 不变)
- [ ] semantic.rs 单测:FakeLibrary(impl RagLibrary + ItemReader + CollectionContent)+ FakePdfBackend + tempfile 索引,覆盖 reindex→search 回路与 allowed_collection 过滤
- 验证:`cargo test -p zot-local`
- 回滚点:commit

## W3 handler fake 测试(zot-cli,AC1)

- [ ] collection.rs:7 个读 arm 拆 `fn handle_read<L: CollectionNav + CollectionContent>(ctx, &L, cmd)`(写 arm 留外层走 ctx.remote(),fallthrough `read => handle_read(...)`)
- [ ] collection 测试:List/Items/Tags JSON envelope(as_json());Get 未命中 → `collection-not-found`;ItemCount payload {collection_key, item_count};非 json 模式 as_json()==None
- [ ] note.rs:List/Search 拆 `fn handle_read<L: NoteReader>`;测试:envelope 含 note key/content;Search limit 传抵 fake(交互断言);fake Err 透传
- [ ] util.rs:require_item → `&impl ItemReader`;require_item_pdf / require_pdf_attachment → `&impl AttachmentSource`(调用方传 &LocalLibrary 均不变)
- 验证:`cargo test -p zot-cli`
- 回滚点:commit

## W4 收尾

- [ ] AppContext 收敛 library md-cache 路径拼装(library.rs:290-297;最小动作:context.rs 加 `library_md_cache_path()`,store 构造保持参数注入)
- [ ] spec 更新:`.trellis/spec/zot-cli/backend/database-guidelines.md` 补「窄 trait 消费」小节(access boundary 不变)
- [ ] 全量 gate:`cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace`
- [ ] grep 复核 AC3;新增代码 rustfmt 干净(以任务前 `cargo fmt --check` 漂移清单为基线,不触碰 8 处预存漂移)
- 回滚点:commit

## 验证命令汇总

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
grep -rn "PdfiumBackend" src --include=*.rs   # 构造点 ≤2(context/doctor)
```

## 风险与回退

- run_pdf 闭包捕获:Arc 非 Copy,分支互斥处直接 move,同分支复用处 Arc::clone;若泛型推导出问题,可临时以 `&*backend`(&dyn)传参兜底。
- 每波次独立 commit;任一波次失败 `git checkout -- <files>` 回该波次前状态,不影响已归档的 9 个兄弟任务。
