# 已删除的空分类

2026-11-10，Zotero 开着，SQLite 快照为 `zotero-db-busy`。空分类判断用 Local API：`GET /collections` 和 `GET /collections/KEY/items`、`/items/top`。Server ID 为 `2ZxitCcOtvOu`。只删除整理请求里出现过、且不是落点的来源分类，外加方案里本来就是空的 `D5MI5QAE`、`2ND8GB6R`、`A3974ZQE`、`X6RWBDRM`。

94 把 key 的直接文献数和全部 item 数都是 0，子孙也同属这批。按 84 个最外层分类做本地预览并确认，子孙随父分类一起删除。报告在 `research/previews/empty-delete-report.json`。抽查 `ZME8XQMS`、`ZE3Z2YJK`、`GCFFVCIF` 已是 404。新书架 `FQZ9GPPN`、`XRF5WWX2`、`U4ZWG29U`、`ID2KTFJG`、`7AI4CBF3`、`UMQV286S` 以及 Forecasting、00Basic、02Optimization 仍在。

未删除：

- `MSKRR445` TimeSeriesForecasting 还挂着一条顶层附件 `5P4SVVVK`（WPMixer PDF）。它的空子类 Linear、bench 已删除。
- `HIW8XDF7` 不在当时的分类列表里，这次没有对它发删除。
