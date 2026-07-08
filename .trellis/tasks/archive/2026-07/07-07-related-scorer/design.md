# Design: 统一相关性评分为单一 scorer(一页)

## 权重定案

采用 graph.rs 现有权重作为唯一真相(它是显式 pair model,且已被图谱输出消费):

| 信号       | 权重    | 说明                                                |
| ---------- | ------- | --------------------------------------------------- |
| RELATED    | 100     | 双方一致,不变                                       |
| COAUTHOR   | 8       | **进入 related**(db.rs 原缺失——修复分叉,行为变化 1) |
| TAG        | 5×count | **取消 `HAVING cnt>=2` 阈值**(行为变化 2,见下)      |
| COLLECTION | 1×count | 双方一致,不变                                       |

### 决策理由

1. **coauthor 进入 related**:`zot related` 缺 coauthor 是 db.rs 实现滞后,
   不是产品决定;图谱与 related 对「相关」应同一语义。
2. **取消 `HAVING cnt>=2`**:该阈值是评分语义泄漏进 SQL(单个共享 tag 记 0 分、
   两个记 10 分,跳变无由)。统一后每个共享 tag 记 5 分;单 tag 对(得分 5)
   会新出现在 related 尾部,排序影响限于低分段。若未来需要噪声过滤,
   应作为 scorer 的显式 min_score 参数,而非埋在取数 SQL。

两个行为变化均写入测试显式断言,并记录到 zot-local spec。

## 形状(在既定边界内:db.rs 取数,graph.rs 打分)

- graph.rs:`pub(crate) const` 权重仅此一处;新增/复用纯函数
  `score_pair(signals: &PairSignals) -> u32`(或沿用 PairAccum 既有形状,
  提出可被 db.rs 路径调用的纯入口)。
- db.rs::get_related_items:SQL 退化为**取信号**(related 对、共同作者数、
  共享 tag 数、共享 collection 数——不再在 SQL 里乘权重/设阈值),
  然后委托 graph.rs 的 scorer 排序。db.rs 不新增图谱语义,graph.rs 不新增 SQL。

## 测试

- 同一 fixture:`related` 与 `graph` 对同一对条目相对排序一致(验收核心)。
- 行为变化断言:coauthor-only 对在 related 中得分 8;单共享 tag 对得分 5
  (旧行为 0,测试注释标明变化)。
- 既有 db.rs fixture 测试(:2621 起)与图谱测试全绿(权重语义不变部分)。
