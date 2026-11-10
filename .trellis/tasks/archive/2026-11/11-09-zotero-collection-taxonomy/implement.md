# 执行计划

范围是 TimeSeries、Deep Learning、Optimization。用户已批准按预览改库。2026-11-10 的 doctor 显示 Local API 不可达，执行停在预览之前。不要运行 `task.py start`，直到一次 doctor 里 `local_api_write.reachable` 为 true。

## 开始前

1. 再跑 `zot --json doctor`。
2. `local_api_write.reachable` 必须为真。否则停止，请用户打开 Zotero 并允许本地通信。
3. 不因为 Local API 失败改走 Web。Web 凭据现在也没有。
4. 用 `collection list` 核对 design.md 里的 key 仍指向同一名字。key 变了就停止。

## 顺序

批准后按这个顺序做完三个领域。不要先做 Optimization 再做 Forecasting。

1. TimeSeries：先建还不存在的落点，包括上移后的 Soft Sensor。创建用 `collection create --write-backend local`，先预览。
2. 按 design.md 的 Forecasting 与 TimeSeries 表，逐个源分类预览、加标签、移动。
3. Deep Learning：落点用现有 `00Basic`、Attention、Transformer、KAN、Diffusion、Mamba。MOE 进 Basic，不新建 MOE 类。
4. Optimization：落点用现有父类和六个保留子类。调度与 Schedule 都进父类。
5. 每个源分类单独预览。条目先加该行规定的标签，再 `collection move-item`。已经在落点的只加标签。
6. 每批最多 50 条。超过时拆批，每批重新预览。
7. 一个源分类回读确认后，才开始下一个。
8. 旧类变空后列出 key 和名字，请用户在 Zotero 里删除。CLI 本地路径不能删除分类。

## 校验

- 预览响应里没有写入成功字段时，库必须仍是原计数。
- 每批回读：落点包含这些 item key，源分类不再包含它们，规定标签存在，`/unread` 与 `CodeAvailable` 等流程标签还在。
- 三个领域都结束后跑 `collection list` 和 `library stats`。Forecasting 子类为 7 个，外加 Diffusion。Deep Learning 子类为 Basic、Attention、Transformer、KAN、Diffusion、Mamba。Optimization 子类为 DecisionMaking、IndustryOperationOptimization、优化控制、RL、RTO、Multi-objective。
- 部分失败时列出成功、失败、未执行的 key。不把 envelope 成功说成整批完成。

## 回退点

每个源分类的预览 JSON 留在本任务 `research/previews/`。回退只移动该批 item key，并移除该批新增的 `ts/`、`dl/` 或 `opt/` 标签。不删除用户原有标签，不碰 Archive、Industry 和 Control。
