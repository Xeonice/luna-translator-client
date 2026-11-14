# 完整窗口核对补齐的既有入口：范围偏好与显示游戏

本批用户已授权按现有PRD/设计实施。实际全窗核对发现：FP-05/11、RW-10/11 的作用范围编辑无法只从当前 effective Preferences 安全恢复；批准 app.js 的“显示游戏”尚无真实效果。此为既有行为接线，不增加产品范围、框架、schema、Core ABI 或 host JSONL。

## 已审窄合同

- Rust `preferences_scope(scope_key)` 返回指定真实 global/game 的 saved/effective/global、已保存 headRevision、逐项来源和 inheritedDraft；复用现有 preferences_for_scope/global/merge，不能把当前其他game effective改scopeKey冒充查询。
- 恢复继承先使用Rust派生的 inheritedDraft（清整个游戏mask，含legacy reading.segmentationEnabled），仍是草稿；明确保存后追加不可变版本，旧数据保留。
- 新增兼容的 `SaveScopedPreferences { preferences, expected_head_revision }`，writer事务核对该scope真实head（None表示应不存在），与已有SavePreferences分开保留兼容。禁止用全库commitRevision代替scope head并发控制：游戏持续取文会产生无关提交；也禁止套最新全库revision使旧草稿伪装成新编辑。
- 两种保存都沿用 native prepare → durable → native commit/rollback；UI读取失败可重试、旧平台回执标过期，保存期间新编辑不因旧成功回执被擦除。
- 对“显示游戏”，只由服务端active binding派生完整 HWND/PID/路径/creation/architecture/class。probe共享identity-only核验以支持最小化/隐藏态，同时原候选发现仍保留可见过滤。先核验、仅显式点击后show/restore、SetForegroundWindow，再核验身份/visible/foreground。
- ShowWindow返回“此前是否可见”，不当成功码；系统拒绝前台激活应返回visible_not_foreground，不能声称已聚焦。不新开游戏、不猜标题/宿主、不申请录屏权限。
- 新增绑定状态查询来自重核验的可信绑定，不按game存在或Hook连接推测窗口仍关联。未关联、过期、无窗口与失败明确返回。

## Ownership与验证

### 独立审阅后的并发前置补强

独立架构审阅发现，显示动作若排队期间绑定已切换，事后检查回执仍可能先激活错误的新游戏。调用方必须冻结 `expectedSessionId` 与 `expectedTargetIdentitySha256`；服务端在同一个绑定 lease 内核对后才实施窗口效果。请求不接受 HWND、路径或任意目标。原生菜单也在用户点击时冻结可信绑定身份；无有效绑定则明确拒绝。

窗口标题挑战会临时写入 nonce，绑定查询与激活应共用同一个短期 proof guard；并发请求明确返回 busy，不投影临时标题，也不与挑战交错激活。增加受控并发负例验证。UI 的解绑、绑定查询及激活回执均绑定发起时的会话与请求代际，旧响应不得清空或覆盖新会话。

这些新增 `GameWindow*` DTO 的 `targetIdentitySha256` 统一取可信 `Candidate::selection_identity()`，即进程身份加 HWND/class 的窗口绑定身份，避免同一进程、同一会话内更换窗口仍误激活。没有有效窗口时该查询字段为空。既有会话、预览和数据库的进程身份语义保持不变，UI 不跨这两种身份作等值比较。

### 解绑效果与持久回执的闭包

独立审阅 CR-SCOPE-04 证明旧 unit 解绑先写回执、再取得窗口 lease，期间重新关联会使旧请求解除新窗口。内部 `Command::UnbindWindow` 改为携带同一 `GameWindowActivationRequest`；遗漏 payload 在编译/反序列化时拒绝。运行时与关联共用 capture lock，并在同一窗口 lease 内完成只读幂等预检、完整窗口身份核对、可补偿的窗口解绑和专门持久提交。普通 repository dispatch 拒绝绕过原生效果直接制造解绑成功；失败恢复绑定，不写成功事件，原文采集保持独立。

解绑以完整窗口期望身份为并发前置，不因无关原文提交阻塞；同 operation 的成功重放先返回旧回执，不再次操作新绑定。事件属于请求的会话，成功 resultId 为此次窗口身份。主界面和 HUD 同时迁移。Tauri 只在非重放成功后撤销发起时同一预览 epoch；陈旧请求、失败、重放与期间新关联均不得停止新窗口预览。增加真实 actor/Client 错误补偿与并发负例后再独立审阅。

backend owner负责新增DTO文件、Command新兼容分支/导出、core ports/runtime、infra queries/database/hook_backend、hook-client/window_binding 与 target_probe 的 activation 局部及测试；Root负责Tauri IPC/role/native事务与TS端口/generated装配；UI两个Owner按既有分工消费契约。不得覆盖其他并行改动。

先跑Rust真实scope查询/隔离/继承mask/CAS/idempotency与binding陈旧身份回归，实际cross compile x86/x64 probes后重新stage。QA独立更新本批v2 source登记，原001/431/旧registry不改。最终正式包实机核对SR开始四游戏的显式显示游戏与作用范围保存。编译/controlled producer不当作最终原生前台与游戏通过。
