# R26：实际预览图片被正式 CSP 阻断的窄修复

日期：2026-10-20。范围沿用用户已授权的六项缺口实施及真实游戏回归；本项修复用户在候选05实机发现的预览破图，不新增产品能力或变更设计。

## 追溯、职责与允许范围

- 需求：FP-03、FP-12；RW-01～05 的实际窗口帧、权限及不可用边界；既有 reviewed-reading 场景和 proposal/delta specs/design/tasks/verification.json 继续适用。
- 设计：`docs/design/first-product-v1/index.html?scenario=window-preview#window-preview`、当前会话运行窗口预览；保留全部批准设计源、PNG、token、尺寸和零差异阈值。
- 模块/组件：M12 平台窗口、M13 主界面、M15 验证；C13/C14/C35 的既有预览职责。Rust/IPC/DTO/Core/SQL/语言 worker 边界不变。
- Owner：preview_csp_fix 负责下列四文件的最小实现及红/绿实际执行；root 负责工作项、精确采用、封包与实机；preview_render_trace 的独立诊断和另一独立审阅者复核修复/测试/配置锁。
- 产品修改仅 `apps/desktop/src-tauri/tauri.conf.json`：在既有 `img-src` 中允许组件实际使用的 `blob:`；其余 directive 与安全开关不变，不增加 framework/依赖/HTTP 权限。
- 测试修改仅 `apps/desktop/tests/visual/verify-reviewed-reading.mjs`、`apps/desktop/tests/visual/verify-ui.mjs`、`apps/desktop/tests/visual/capture-reviewed-states.mjs`：真实产品页面在生产配置的完整 CSP 下加载，保留既有38 ID和原断言，并验证 JPEG `decode()`、complete、非零及符合样本的自然尺寸，不能仅等待 img/src。`verify-ui.mjs` 的唯一共享 pageFor 在首次导航前安装并核对完整 CSP，以覆盖全部79行为及原v3/v4画面，避免在单个v4模块事后注入或重复初始化。
- 规范登记的必要同步：独立审阅后才更新 technology-lock 中该配置的精确 hash、现有状态源 pin 与实际 producer 身份，以及本 OpenSpec 的 design/tasks/verification 和 R26 基线历史记录。禁止自动刷新未审变更、批准图、旧债务或阈值。

## 当前证据与依赖

- R25/candidate05 的开发门禁已实际61/61通过且源码/ZIP不漂移；该事实保留，不作为本项或新包通过。
- 真实系统授权已从旧04的要求更换为05的要求；用户提供和实际客户端截图均显示 image 破图而状态 Live。
- 独立诊断 `.build/next-batch-20261020/context/preview-blob-csp-diagnosis-05/`：同一真实 React 组件及合法 JPEG，原正式 CSP 阻止 blob、自然宽度0且解码失败；只追加 img-src blob 后解码成功。非真实游戏验收。
- 旧包、原失败图/日志/系统授权取证原样保留。所有新输出使用独立目录。

## 验证与完成条件

1. 原正式 CSP 的受控负例实际失败，阻断来源必须是 img-src/blob，不能改断言或换预期绕过。
2. 修补后用实际 Rust 四 DTO producer 与新 nonce 执行全部38项组件检查，保留暂停、旧租约/帧隔离、历史刷新等已有断言。
3. 全部79行为、37状态77变体在完整正式 CSP 下重新加载并解码图片，按原冻结 PNG/条件/阈值独立复算；所有加载/解码失败必须失败。
4. 独立审阅四文件精确差异、technology-lock 和必要登记，R26 仅采用已审输入；原始 PRD/License/批准16源37图/420保护项/字段关系/旧债务无变化。
5. 生成新候选06，验证实际签名、EXE、源 seal、ZIP；运行完整统一开发门禁，不继承05产品通过。
6. 新包实机完成正确代码身份的系统授权、真实 SR 窗口内容及两次变化帧的 Game/客户端图核对，再按本期四游戏目标继续逐款回归。浏览器状态、时间戳、img 节点或辅助探针不能代替新包可见像素。

执行状态：计划，尚未因本卡关闭任何产品用例或发布门禁。不提交、不推送。
