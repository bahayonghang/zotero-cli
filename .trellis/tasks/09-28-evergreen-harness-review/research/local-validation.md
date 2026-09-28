# 本地验证记录

2026-09-28，Windows/PowerShell，基线 ba4e10e987e7afe65dccb8450445d41bc9e08d79。rustc 1.98.0，Node 26.7.0，actionlint 1.7.12，cargo-audit 0.22.2，cargo-deny 0.20.2。

| 检查 | 结果 | 证据/边界 |
| --- | --- | --- |
| just ci | exit 0 | just-ci.log / result.json；fmt、locked check、clippy、测试、镜像门通过 |
| Rust | 281 个不同测试，0 失败/忽略 | CLI126+JSON4+version2+core14+desktop11+local64+search8+semantic8+remote44；version guard 执行两次，合计执行283次，不重复计数 |
| Python | 13 通过 | just-ci.log；两个 published skills 各两套镜像一致 |
| npm --prefix docs run build | exit 0 | docs-build.log；现有 node_modules，无安装升级；本机 Node26 与 hosted Node20 不同 |
| actionlint | exit 0 | actionlint.result.json，日志空 |
| cargo audit | exit 1 | cargo-audit.log；305 依赖中报告1个 rustls 漏洞 |
| cargo deny check | exit 1 | cargo-deny.log；advisories FAILED；bans/licenses/sources ok；重复依赖只为 warning |
| 平台渲染探针 | 全部命令 exit0，三者语义失败 | probe_platforms.py、platform-probe.json、phase-2.1-*.txt；claude/kimi/omp 各45字符，仅标题；codex851/grok843有正文 |
| grok inspect --json | exit 0，发现已核实 | grok-discovery.json 保存去敏来源；不证明模型执行 |
| zot --version / --json doctor | 1.0.1 / exit0 | runtime 固定使用已安装 zot；能力摘要如下 |
| 本机 MSRV/nightly/udeps/machete | NOT RUN | 对应工具未安装；同 SHA 历史 hosted 证据另列 |

## Doctor
SQLite available=true，schema129，legacy_tables=false，sidecar_present=true；Pdfium available/cached=true。connector/local_http=false，connector-unreachable；Web configured/verified=false；embedding configured=false。索引200 chunks、0 embeddings。未运行 PDF 提取、embedding、导入或 Web 写。doctor 可产生读取快照/缓存，不写 Zotero 源数据库。

环境缺少能力不计作 Rust 测试失败。没有保存密钥或文献内容。本轮只新增5个任务目录；Cargo.lock、源码、正式说明、CI、canonical skill 与远端不变。research 诊断脚本不冒充正式回归。
