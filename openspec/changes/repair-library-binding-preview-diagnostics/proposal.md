# 修复游戏库封面、关联流程、常驻预览与 Hook 排障信息

## RF07 实机会话记录过期恢复（2026-10-22增量）

Root已在编码前建立[RF07工作卡](rf07-history-range-recovery-work-item.md)。candidate12源`77ea4800b7b917e3006048842b05e3a34bb3af45f5a78b9957bd1cba1059370f`的SR实机已有4条独立TextOutW正文、主窗/HUD一致；当前历史范围返回typed `snapshot_expired`后停在旧记录，是独立失败，不据此推断Hook断流，也不把该局部成功继承为未来candidate13通过。

本增量只恢复FP-09/12与RW-17～23既有完整记录/跟随/失败恢复行为；FP-07正文词法显示保持独立。后端1800秒绝对TTL、固定范围身份、DB/DTO/契约不改。follow=true遇typed snapshot_expired时单次cursor:null重取，成功后才整体替换旧范围；暂停浏览保持原行与锚点，snapshot_expired、invalid_cursor（含GC删除旧range）、cursor_range_changed均经既有“回到最新”显式读取。非过期或一般错误不自动reset，恢复读取失败保留旧数据和可见错误。

UI Owner仅修改SessionRecordPanel.tsx与verify-reviewed-reading.mjs；QA仅登记reading_components.py/test_reading_components.py；本Owner仅持有proposal/design/tasks/delta/verification与.build准备。既有检查保留，旧分页/失败/迟到/多页断言不删；新真实组件用例与最终同证书新包历史恢复分别留证。不改当前R33基线、source-snapshot、批准PNG/token/阈值或产品源，不自动准备或采用R34指纹；精确authority增量须等Root最终源清单与独审。

状态：编码前候选；本 change 先记录范围和可执行追溯，不提升设计、实现、实机或发布状态。

## Why

2026-10-21 用户在签名 09 正式包真实界面指出五项缺口：游戏库无封面、绑定流程偏离原稿、详情三个冗余连接按钮、运行窗口预览不应每次手动开启、Hook 日志缺有效排障信息。只读报告位于 `.build/hook-lexical-retest-20261021-010406/product-gap-review/` 与 `log-diagnosis/`。需要区分恢复既有设计、最新明确产品覆盖和现实现缺陷，不能统一声称原稿已包含全部新行为。

## What Changes

1. REQ-038 / FP-02：接通真实本机封面资产链。用户已明确选择“自动读取本机封面，无图时自动保存首张游戏画面”。优先读取真实本机 Steam/游戏封面资源；正式关联、目标身份有效、已获录屏权限且游戏没有封面时，才把首张有效窗口帧保存为托管静态封面。临时候选不保存；不持续存帧，不自动覆盖已有封面，不复制设计图片冒充实际资源。静态封面与实时预览分别标识。
2. FP-03 / GL-11～20：单个附属面板内选窗口或进程、核对完整身份和预览，一次“关联并接入”即启动 Hook；接入后等待候选、显式选正文。删除产品额外的“核对目标→再确认关联”步骤，不删除必要运行身份检查、候选与选流。
3. FP-02 / REQ-037：用户已明确选择“客户端只连接已经运行的游戏”。首版移除全部游戏启动用户入口，包括详情“运行并连接”“关联并接入”“连接已运行游戏”三个冗余入口；保留页头全局连接入口和必要会话恢复操作。卡片单击/双击/更多均不增加启动手势。必要 broker/helper 管理不属于游戏启动入口，不受此约束。
4. RW-01～06 / FP-03：选中有效目标且权限已具备时自动预览，正式关联后由 session 持有并跨当前会话页签常驻；缺权限显示授权与恢复，授权后恢复所选目标，不再要求每次点“开启”。目标退出、撤权、最小化、切换、取消和迟到帧有明确状态。
5. REQ-052 / FP-12 / RW-17～23：保留已有 Core 原生有界脱敏诊断及结构事实；修正 Warning=1、EmuWarning=2 分级；展示引擎/扫描范围/Hook 地址/错误码/架构及实际 DLL 等已有可证事实。默认抑制逐条 text_unselected 刷屏，保留全部提取可追溯，不删除原始事件或隐藏警告、缺口、存储错误。

## Explicit Authority and Scope

上述两项选择均为用户明确答复，状态 `resolved_user_explicit`，不是推荐默认值。原始 PRD 快照不改；当前首版范围、详规、现行设计和技术文档需要同步覆盖。具体资产端口/字段契约、产品精确路径和新视觉状态须经 Root 审阅后追加到允许范围；本次 sidecar 仅授权此 change 的计划文件。现行 PRD、设计、架构、baseline、已批准 PNG、源码、守卫均不由本工作项编写者修改。

不引入新框架、依赖、DB schema、完整 LunaTranslator UI、自动选流、OCR、模型上传预览帧或在线元数据服务。复用 Rust 业务权威、窄 Tauri 适配、独立 hooks-core、语言 worker、现有分页/导出和封面引用字段；缺乏能力时先审契约，不能伪造已就绪。

## Impact and Acceptance Boundary

原基线 `FP1-20261021-R30`、09 源码 `ee266096a78a703c2c07f8ff48d66c2c6a399da131e266ba3878e624472a0d81` 保留。原 09 工程检查、签名、权限和 SR 局部取文证据不能继承为本修复通过。当前 SR 会话只有中文正文、0 日文 revision、0 lexical snapshot，故日文分词与四游戏回归仍未证明。

完成需新修复包、真实 assets/完整点击链/常驻预览/原生消息贯穿验证、日文分词及 SR→MO→Never7→サクラノ詩10th 四游戏同包回归；不测 Ever17。守卫自测、设计回放、编译或源码映射不能替代产品实测。不提交、不推送。
