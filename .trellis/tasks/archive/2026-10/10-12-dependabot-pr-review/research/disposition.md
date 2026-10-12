# 处置结果

日期：2026-10-12。分支：本地 `dev`。这些提交尚未推送，也没有合并进 `main`。

| PR | 结论 | 本地提交 |
| --- | --- | --- |
| #17 clap 4.6.6 | 接受，关闭 | `974334dda8280691b6a1665f244132d76132660f` |
| #18 thiserror 2.0.20 | 接受，关闭 | `974334dda8280691b6a1665f244132d76132660f` |
| #20 tar 0.4.46 | 接受，关闭。GHSA-3cv2-h65g-fgmm | `974334dda8280691b6a1665f244132d76132660f` |
| #21 http 1.5.0 | 接受，关闭 | `974334dda8280691b6a1665f244132d76132660f` |
| #19 dirs 7.0.0 | 接受，关闭 | `e1221155bc522497c9b99939d042c0ad1f1e8cca` |
| #22 actions/checkout v7 | 接受，关闭 | `588e2bf65051177a6a35eb031bd715f09ee9e747` |
| #23 actions/setup-node v7 | 接受，关闭 | `588e2bf65051177a6a35eb031bd715f09ee9e747` |
| #24 rust-toolchain 1.120.0 | 拒绝，关闭。MSRV 保持 1.85.0 | `815cd97215d456146a1b44cb615ab21498315d39` |

验收时的锁版本：clap 4.6.6、thiserror 2.0.20 与 1.0.69、dirs 7.0.0、dirs-sys 0.5.0、tar 0.4.46、http 1.5.0、rustls 0.23.45。`just ci` 在提交前通过。`node-version` 仍是 20。MSRV job 仍是 `dtolnay/rust-toolchain@1.85.0`。
