# C4 Design

在现有CI加入cron 17 8 * * 1；非security job加github.event_name != 'schedule'条件，security沿用现有audit/deny和固定工具版本。security明确contents:read，避免定期扫描获得写权限。不引入重复workflow或新的依赖检查框架。

事件矩阵：push(main)、pull_request(main)、workflow_dispatch都保持现有stable三平台、msrv、security、machete、udeps、docs；schedule只有security。审核job依赖，不产生scheduled路径的必需上游被跳过问题。

默认分支上的schedule需部署后才实际生效；本地YAML验证不能证明定时执行成功。已有同SHA绿色历史记录继续作为历史证据，不作为新公告之后的安全证明。

C1负责消除已有失败；C4负责发现时效。说明落harnesses.md和现有质量规范，适用五套工具。需要改变扫描频率、分支或远端策略时先修订批准范围。
