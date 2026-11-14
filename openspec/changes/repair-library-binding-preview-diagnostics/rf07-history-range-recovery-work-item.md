# RF07 会话历史过期范围恢复修复

2026-10-22；已授权首版及本轮实机修复范围，编码前工作卡。

- 现场：candidate12 SR SID `1792604088217-42741-0`；TextOutW selection2 后4条独立完整正文/模型归属已核实，主窗与HUD最新句一致；历史仅旧2条，跟随开启但 `snapshot_expired` 后永久停止刷新。证据位于 `.build/game-regression-20261021-candidate12/sr/after-user-confirmed-change/`。这不是Hook断流的证据。
- 需求/验收：FP-07/09/12、RW-17～23（会话完整记录、冻结范围、迟到记录提示、失败保留与恢复）；既有C36 SessionRecordPanel、M10会话记录、M13 UI适配。设计为当前整稿 `index.html#main-content` 的正文/全部提取/Hook日志页签与错误恢复状态。
- 原因：后端固定范围1800秒绝对过期；UI先刷新旧cursor才能检查newerCount，异常后停轮询，重试仍使用过期cursor。
- Owner：Root统筹、UI worker仅持有 SessionRecordPanel.tsx 与 verify-reviewed-reading.mjs；QA仅持有 reading_components.py/test_reading_components.py 登记及独立审阅；治理Owner只持有本change及精确authority增量；并行不覆盖他人。
- 修改范围：前述4个现有文件；本change工作卡/设计/任务/verification映射；经独立审阅的R33→下一revision身份增量。无新框架、端口、协议、数据库字段/关系/TTL/快照身份政策；不动批准PNG/token/阈值，不改Core或游戏设置。
- 语义：跟随模式遇到后端typed snapshot_expired时仅发起一次新的cursor:null读取，成功才整体替换；暂停浏览保留旧行/锚点且经既有“回到最新”显式重新读取。非过期错误不得自动换范围；任何旧session/filter/dataset/迟到请求不得污染新状态，维持single-flight。重试必须可恢复，不掩盖真实失败。
- 验证：现有分页/失败/迟到/多页断言全保留；增加真实React控件的typed过期、跟随与暂停、范围切换/迟到、非过期错误及新读取失败恢复用例；实际类型检查及组件Playwright，独立测试登记/回执核对；完整verify仍独立记录canonical视觉source drift，禁止更新参照绕过。
- 完成条件：源/包身份经审阅、同证书打包；实际游戏回归与历史恢复按新包重新留证。candidate12局部成功不继承新包四游戏完成，不循环重启当前Luna/游戏。

## 编码前补充：回收后的失效游标

Explorer核对 new_range 会清除旧非导出范围，重放时可能是 typed `invalid_cursor`。该错误与 `cursor_range_changed` 只保留旧行和错误信息，提供清楚的显式“回到最新范围”以 cursor:null 重新读取；不得把它们自动当作合法过期范围、不得解析/伪造opaque cursor，也不改变后端拒绝条件。普通消息相似/错误stage等负例不触发自动恢复。
