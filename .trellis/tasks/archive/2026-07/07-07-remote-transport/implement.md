# Implement: transport seam 执行清单

基线:`grep -rn "fn remote_err" src/zot-remote/src/ | wc -l` = 6;
zot-remote 无 dev-dependencies。

## 批 1:http.rs 共享响应层 + zotero.rs 迁移

1. http.rs 新增 remote_err / http_hint / ensure_status / read_json /
   ensure_empty(签名见 design.md §1),各配直接单测(remote_err 映射
   code/status;http_hint 各状态段)。
2. zotero.rs:删除自有 ensure_empty(:737)/ensure_json(:752)/http_hint(:895)/
   remote_err(:886),全部调用点改共享函数;错误码字符串逐个保留。
3. 验证:`cargo test -p zot-remote`、`cargo clippy -p zot-remote --all-targets
-- -D warnings`、`grep -rn "fn remote_err" src/zot-remote/src/ | wc -l` = 1。
   回滚点:commit。

## 批 2:其余 5 client 迁移

4. better_bibtex.rs:100 / oa.rs:598 / scite.rs:256 / embedding.rs:107 /
   semantic_scholar.rs:203 的 remote_err 删除,内联 send→status→json 改
   ensure_status/read_json(oa 5 处 send、scite 4 处等,逐个机械替换;
   错误码不变)。
5. 验证:同上;grep fn remote_err 仍 = 1(仅 http.rs);抽查无内联
   status→json 残留(`grep -n "status()" src/zot-remote/src/*.rs` 复核)。
   回滚点:commit。

## 批 3:base_url 收敛 + fake adapter + zotero 测试

6. zotero.rs / semantic_scholar.rs 增加 env 覆写(ZOT_ZOTERO_API_BASE /
   ZOT_SEMANTIC_SCHOLAR_API_BASE,默认不变)+ 测试注入构造(with_base_url,
   #[cfg(test)] 或 pub(crate))。
7. zot-remote Cargo.toml 加 `[dev-dependencies] tiny_http.workspace = true`。
8. 新增 tests(design.md §4):版本前置条件 header 断言、412/404/500 错误
   映射、update-item 204 写路径、create 流 read_json 解码。
9. 验证:`cargo test --workspace`、clippy 全绿;无持久化(fake server 内存态)。
   回滚点:commit。

## 批 4(移交 07-07-pdf-http 任务执行)

10. zot-core 增 net 常量模块;http.rs 与 zot-local/pdf.rs 改用;
    pdf.rs 下载路径冒烟(单测常量一致性即可,不真下载);
    spec 记录 pdf.rs 保留 blocking 栈的边界说明。

## 验收对照(prd.md)

| 验收项                              | 覆盖             |
| ----------------------------------- | ---------------- |
| fn remote_err 定义 = 1              | 批 1-2,grep 验证 |
| 6 client 无内联 status→json         | 批 2 抽查        |
| zotero 版本前置/错误映射/写路径测试 | 批 3 步骤 8      |
| 既有纯 helper 测试全绿、无持久化    | 每批 cargo test  |
| clippy / test 全绿                  | 每批验证         |
