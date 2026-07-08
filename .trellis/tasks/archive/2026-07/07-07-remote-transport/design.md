# Design: 补完 zot-remote transport seam

## 1. transport 形状:http.rs 增设共享响应处理层(不新建 struct)

HttpRuntime 保持连接池角色不变(超时/UA 缺省不变)。请求构造留在各 client
(API 形状:路径、query、header——含 zotero 的 `If-Unmodified-Since-Version`);
**响应半程**收敛到 http.rs 的自由函数:

```rust
// http.rs 新增(pub(crate)):
fn remote_err(code: &'static str) -> impl Fn(reqwest::Error) -> ZotError  // 唯一定义
fn http_hint(status: Option<StatusCode>) -> Option<String>                // 唯一定义
async fn ensure_status(resp: Response, code: &'static str) -> ZotResult<Response>
async fn read_json<T: DeserializeOwned>(resp: Response, code: &'static str) -> ZotResult<T>
async fn ensure_empty(resp: Response, code: &'static str) -> ZotResult<()>
```

- 被否:Transport struct 包 send——6 个 client 请求形状差异大(multipart、
  自定义 header、GET/POST/PATCH/DELETE),包装 send 只会复刻 reqwest builder。
  深模块的收敛点是「status→错误映射→JSON 解码」这段六处重复的响应逻辑。
- zotero.rs 的 ensure_empty/ensure_json/http_hint/remote_err 方法体迁走,
  调用点改为共享函数(错误码参数原样保留,payload 字节不变)。

## 2. base URL:6 client 统一「env 覆写 + 常量默认 + 测试注入」

> **批 1+2 落地修正**:①ensure_status/read_json/ensure_empty 的 code 参数为
> `&str`(zotero create_items 传非 'static code);remote_err 保持 &'static str。
> ②zotero library_version 内联状态检查顺带迁移(字节等价)。③bbt 的连接提示
> 仅保留在 JSON-RPC error 分支,send/json/status 路径改共享映射(probe 上游
> 已拦截不可达,错误码 4 个全保留)。④oa/embedding/ss 的状态错误 message
> 统一为通用文案(错误码与 status 不变,grep 证实无测试/下游依赖)。

- 现状:bbt(ZOT_BBT_URL/PORT)、scite(ZOT_SCITE_API_BASE)、oa(4 个 env)
  已可覆写;**zotero.rs:13 与 semantic_scholar.rs:8 硬编码**。
- 补齐:`ZOT_ZOTERO_API_BASE`、`ZOT_SEMANTIC_SCHOLAR_API_BASE`(默认值不变;
  oa.rs 的 ZOT_SEMANTIC_SCHOLAR_GRAPH_BASE 是另一端点,名字不冲突)。
- 测试注入用 `#[cfg(test)] fn with_base_url(...)`(或既有构造参数),
  避免并行测试的 env 竞态;env 覆写面向用户,与兄弟 client 模式一致。

## 3. fake adapter:tiny_http dev-dep 本地 server

- tiny_http 0.12 已是 workspace 依赖(zot-local graph server 在用)——
  zot-remote 加 dev-dependency 零新增外部依赖;绑 127.0.0.1:0 随机端口,
  base_url 注入指向它。不打 live service,符合 quality-guidelines
  network-test pattern(fake server 属本地 adapter)。
- 被否:手写 TcpListener(重造 chunked/keep-alive 轮子)、wiremock(新依赖)。

## 4. 测试范围(zotero.rs 907 行首次可验证)

经 fake server 覆盖:

1. **写前置条件**:update_item 请求带 `If-Unmodified-Since-Version: <version>`
   (server 端断言 header 值);
2. **错误映射**:412 → ZotError::Remote{code, status: Some(412), hint 含版本
   冲突提示};404/500 各一例断言 http_hint;
3. **一条写路径**:update-item 204 → Ok(());另加 read_json 走 create 流
   (MultiWriteResponse 解码)。
   transport 纯函数(remote_err/http_hint)另有直接单测。不测:live service、
   重试语义(现无)、鉴权真值。

## 5. pdf-http 定案(07-07-pdf-http):**选「对齐」**

- 超时/UA 常量抽到 zot-core(如 `zot_core::net::{CONNECT_TIMEOUT,
REQUEST_TIMEOUT, USER_AGENT}`),zot-remote HttpRuntime 与 zot-local pdf.rs
  两栈共用常量、各持栈。
- 理由:①zot-local 不得依赖 zot-remote(既定约束),「统一」需经 composition
  root 注入下载闭包,为一次性引导下载(首次运行缺 Pdfium 才触发)引入跨 crate
  接缝,收益不抵复杂度;②blocking 在同步 crate 合理(PRD 已认可);③漂移的
  实害仅在超时/UA 不一致——对齐常量即消除。豁免不选:可修的漂移不该只记档。
- 落地在 07-07-pdf-http 任务内完成(本任务批 3 后接续),spec 记录写入
  zot-local backend(pdf.rs 保留 blocking 栈的边界说明)。
