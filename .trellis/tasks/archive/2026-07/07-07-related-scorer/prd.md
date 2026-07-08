# 统一相关性评分为单一 scorer

## Goal

消除「两条文献有多相关」的双实现:`db.rs::get_related_items` 的 SQL 内联权重与 `graph.rs` 的 pair model 已分叉,`zot related` 与 `zot graph` 对同一对条目可能给出不同排名(潜在一致性 bug,非风格差异)。收敛为一个纯 scorer,两条命令共享一套权重。

## Problem & Evidence(2026-07-07 逐行核实)

| 信号 | graph.rs(:16-19, :92-130) | db.rs::get_related_items(:1154-1235) |
|---|---|---|
| RELATED | 100 | +100(:1176) |
| COAUTHOR | 8 | **✗ 完全缺失** |
| TAG | 5 | count×5,且 `HAVING cnt>=2`(:1213,:1229) |
| COLLECTION | 1 | count×1(:1205) |

- graph.rs 是纯 pair model(PairAccum :26-31);db.rs 是 SQL 内联计分
- 既有决定(务必遵守):`.trellis/spec/zot-local/backend/directory-structure.md:34-40`(a0a11ee)——**graph.rs 不开 SQLite,db.rs 负责读**。本任务在边界之内工作:db.rs 仍是唯一取数方,graph.rs 拥有权重与打分。这是完成该决定,不是重开它。

## Requirements

- 一套权重、一个纯 scorer(落在 graph.rs 侧);`get_related_items` 取数后委托打分
- 明确决定并记录:`HAVING cnt>=2` 阈值保留与否、coauthor 信号是否进入 related——行为会变,须在 design 定案并写入测试
- db.rs 不新增图谱语义;graph.rs 不新增 SQL——职责边界不动

## Acceptance Criteria

- [ ] 相关性权重常量定义仅 1 处
- [ ] 新测试:同一 fixture 下 `related` 与 `graph` 对同一对条目给出一致相对排序
- [ ] 行为变化(若有)在测试中显式断言,并在 spec 记录
- [ ] 既有 db.rs fixture 测试(:2621 起)与图谱测试全绿
- [ ] `cargo clippy` / `cargo test` 全绿

## Notes

- 轻-中量级:PRD + 简短 design(权重决策一页)即可。
- 独立任务,可随时并行。父任务:07-07-arch-deepening(评审候选 F)。
