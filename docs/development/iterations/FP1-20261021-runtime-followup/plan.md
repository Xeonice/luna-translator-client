# 首版实机缺口修复工作卡

## 2026-10-21续批：RF-04与RF-05

本节追加于R32/candidate11后的真实回归，不改下文R31/candidate10历史身份与结果。原source-snapshot、基线、批准图及阈值不动。当前产品状态implementation_in_progress/not_run，原静态或API实验结果不提升整包回归。

### RF-04 / FP-03、FP-12：可变标题证明写前安全拒绝

MO非Unicode窗口发生旧W挑战遗留nonce乱码。独立API实验结论不支持换A/W、SetWindowTextW或切同位数helper作为通用修复。仅在hook-client/src/window_binding.rs与lunahook-host/examples/target_probe.rs及同文件测试：任何标题写入前拒绝非Unicode HWND，现有类型明确unavailable；安全Unicode路径保持唯一exact nonce、完整运行身份、有界等待和显式恢复错误。Hook与预览独立，不把native未证明标已绑定，不改变窗口尺寸/焦点或公共DTO。

维护执行器product-hook-client-tests与product-target-probe-tests必须实际运行对应Cargo维护测试，零测试/跳过/过滤/失败/源漂移不得过。资源复用prepare_core_runtime.py双架构实际构建与receipt，精确stage两个target_probe.exe及core-staging-receipt.json后重新freeze/构包；Core、native锁和旧包不偷换。模块M02/M12/M14/M15、C13/C14/C35/C36；需求RW-01～06/REQ-052。Root分配生产及独审，本Owner管治理与producer；完整授权见当前change/implementation-authorization.md。

### RF-05 / FP-03、FP-12：退出后native active占用（独立缺陷）

实机SR已退、UI已断开而native broker仍占active阻挡MO。待只读根因与精确路径后实施session-scoped ProcessDisconnected、有界异步stop/drain后expected SID/target compare-clear，避免consumer等自身DrainBarrier；旧A诊断不得落到latest B。此处登记失败而非宣称已修，不杀容器或以手动重启替代修复。维护测试及新包回归均not_run。

只读定位后Root已分配natural_disconnect_fix，精确生产为hook-client/src/lib.rs、app-infra/src/hook_backend.rs、app-core/src/application/runtime.rs、app-core/src/ports/runtime.rs及ports/mod.rs、app-infra/src/storage/database.rs；现有application_runtime.rs集成测试和内联测试。supervisor停止、drain与compare-clear按事件SID/完整target约束，清理中typed可重试，保护活B与旧历史；不扩公共DTO/DDL/依赖/UI。新增实际producer沿用product-hook-client-tests/core/infra/native-ipc；产品自然退出→下一游戏同包回归仍not_run。

RF-04另有真实WinAPI维护producer：Root授权repair_independent_review新增window_title_proof_api.py/fixtures/window_title_fixture.c，仅自有offscreen非激活窗、现工具链双架构真Rust target_probe；check product-window-title-proof-api-tests需新nonce/source/build/EXE/逐项断言独立回执，不继承API lab或正式游戏通过。

追加真实安全决策：WS_VISIBLE fixture在Wine仍进入前台，原attempt01前置失败保留；改strict hidden且visible0/foreground0，前置失败立即停整轮。Root修proof identity在原两源内复用强bound_identity，独立于discovery visible/title筛选，避免真实窗口挑战后隐藏/最小化无法恢复；原visible枚举与native exact nonce不放宽。helper内部参数增加冻结arch/class，无wire变更；hidden可安全恢复不等于可预览。独立validator消费原始stdout/process/observation复算hash/计数/状态/身份，不只信汇总。

### RF-06待办登记（未授权实施，不扩大本轮源码范围）

candidate11同句翻译重试完成后仍展示旧connection_failed。证据`.build/game-regression-20261021-candidate11/independent-translation/20261021T092135.742999Z/translation-observation.json`。只读定位ReadingPanel以status!='completed'保留旧failed；后续应由Rust按unit/revision/source/purpose/engine/config slot与intent order投影当前状态，保留历史，不在UI hide-all。当前只登记缺陷与后续设计/验证需求，未分配RF-06实现或扩大source-scope；本轮先完成RF-04/05。

## 迭代身份与授权范围

| 字段 | 内容 |
| --- | --- |
| iteration_id / baseline_id | FP1-20261021-runtime-followup；R31 SHA a5f2213291e086c534690bb39db05e31a5cc91b249a199d6b0aa15616bf9abe8 |
| 用户授权范围 | 延续已授权五项产品修复及 SR/MO/Never7/樱之诗实机回归；连接仅针对已运行游戏、本机封面优先及无图首帧已由用户明确选择。 |
| 目标 | 正式关联真正归属已有/新游戏；初扫有真实等待状态且不重复请求；修复真实失败并补齐存储引用证明。 |
| 范围边界 | 不增加依赖/DDL/公共wire DTO，不改游戏窗口或容器，不覆盖旧批准图/阈值，不提交推送。 |
| B门禁 / 前置 | PRD game-library-window-prd.md第14、35～37、46/65/72行及game-library.js 120～122行为；R31精确采用已独审。candidate10实际完整门禁54/59，失败原件保留。 |
| 主责 / 审阅 | asset_preview_architecture负责生产接入与Rust事务及精准集成测试；Root负责组件、适配器必需清单与工作卡；另一个Owner独审最终源码/存储证明，Root精确采用新基线。 |
| 源码与产物身份 | 无Git；实施前源808366a7e108d44b45f6e883c16eed04ecf66d8d943382726695aebe2219de5f/1628项。candidate10 EXE9cf1c6ef61c96cb0e245c4fded614f2ceb7c3f9028c096bc68a5164e51a0babb；不重标旧包。 |

## 工作项追溯

### RF-01 / FP-03：真实游戏库归属

- 产品/验收：REQ-002/004、FP-02/03、GL-11～20、REQ-038首帧归属；模块M01/M02/M10/M12/M13，组件C11/C13/C14/C15/C35。
- 设计：当前connection-sheet；同面板核对已有记录或新资料，一次确认创建/复用真实GameId并接入；取消、保存失败、接入失败、冲突、迟到和pending分别可观察。
- 视觉依据：现首版风格及既有确认范围不变；新映射状态另存候选并对照，批准PNG与零阈值不动。
- 复用：GameInput/SaveGames/typed GameSaveResult/四字段AssociateRequest；名称与语言草稿可核对，目标真实来源只读；不得用bottleLabel冒充绝对locator。
- Rust权威：native前校验Game和candidate的环境/完整路径；同完整native身份的NULL→GameId补齐需短SQL与active归属一致，失败不留半状态/不误断旧会话。不同非NULL归属明确冲突；不重注入、不换SID，不改既有原文/语言快照。
- 修改范围：LibraryScreen.tsx、CaptureSheet.tsx、capture/styles.module.css、tauri-port.ts；core ports/runtime.rs、application/runtime.rs、infra hook_backend.rs、storage/queries/capture.rs；application_runtime.rs、game_assets.rs精准真实writer测试。Root另管design-port.ts、verify-reviewed-reading.mjs、verify-ui.mjs与reading_components.py/test_reading_components.py。
- 验证：RBC-49已有映射/错目标/显式仅本次、RBC-50新资料typed回执/失败不注入/重试不重建及pending对象冻结；真实Rust拒绝篡改gameId且注入数不增、同会话补齐保持历史且失败回滚、真实首帧归属CAS。正式新包SQL归属、库封面与Hook连续正文单列。
- 完成条件：上述实际通过且独审；当前not_run，不继承candidate10/47项旧通过。

### RF-02 / FP-03：初扫进行中与单飞

- C13/M02/M13/M14；connection-sheet的扫描中/空完成/失败/关闭与重开状态。
- CaptureSheet独立状态与epoch/finally；实例targets单飞只复用进行中的Promise、不缓存旧结果；StrictMode第二setup必须能附着接收，不能runningRef一挡了之。
- RBC-48及真实Tauri适配器校验：延迟时提示与禁并发、空/错可恢复、关闭重开和迟到不污染、成功/失败settle后可新扫。Root必需集合从47扩50，旧47回执不得通过。

### RF-03 / M10/M15：实际失败的存储验收

- 真实candidate10报告.build/quality/20261020T185951.559586Z.json保留；存储carrier旧pin三错、并行聚合profile_in_use故障分别定位，不能刷新哈希或删断言冒通过。
- 已完成只读carrier独审：database.rs仅封面写入转发、queries/mod仅注册；DDL/迁移/原v1保持原件。最终源冻结后再独审，只允许v2/storage-acceptance.json两chain source_sha256及v2/storage-review.json顶层production_migration_source_sha256三个当前指针候选；新storage-carrier-review-r32.json解释完整diff，不改历史嵌套审阅/原v1/22义务/431+29字段/SQL/路由/批准状态。
- profile_in_use先做独立受控复现；未定位不改生产、不延时重试或删除断言。确需文件另补精确授权。
- 加入原存储合同及原守卫的冻结前消费者；新R32原档保留R31，独审后Root精确采用；重新stage实际worker、签名构包及完整verify和rust-storage-complete-v2。

## 变更与审阅记录

当前UI和存储均未判通过。原批准图/设计源冻结不匹配由v4门禁真实拒绝；新状态需独立候选对照与评审，不机械更新批准指纹。candidate10证明新证书DR及现TCC要求匹配，实机候选自动预览可见、日志含native事实、暂停切页保留03:13:11.131原画面；尚无新正文候选，不能称SR/日语回归完成。

## 执行结果与交付

计划状态not_run；逐次结果只写.build独立run，不先勾产品通过。最终必须写新包/源/Core/语言身份、真实检查和局限。SR中文不能替代三款日文取文、分词、注音/词卡验收；Steam其他电脑会话不接管，不测试Ever17。


### 执行时Owner及字段投影澄清

Root追加授权记录见implementation-authorization；RBC48～50测试mjs已独占移交repair_independent_review，Root继续管测试端口/bootstrap和必需清单。RF03两源由Root与gov独立审阅，修复作者不自批。RBC48用独立development构建证明真实StrictMode replay，原47production检查不变。

现schema中reading_units/revisions无独立game_id，旧ReadingUnit.gameId及sourceSummary.gameId是sessions动态JOIN投影。合法补齐同一会话归属后旧句投影会反映新库ID；必须证明原revision/原文/语言/hash/processing_snapshot/tokens/translations未被改写，不为维持旧null投影变更DDL或捏造快照。
