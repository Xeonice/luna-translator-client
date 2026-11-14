# R60预览目标盘符修复迭代卡

授权：用户要求完成既定整个重构计划，继续原重构实机回归，暂不实现新紧凑布局。本片修复R59后续现场可复现的无关盘依赖，不改变产品交互或架构决定。

父基线FP1-20261027-R59，SHA80c9f58150fc886371f3af5465f5c67f935cf2a93237d5471bc99fd136eceac7。精确修改前源码1880项，tree d6b8e1e8b4541acffca919a1361b3b347868f1ea17a63cc1372eaa4e93e5e7bd；全map、两产品源原字节、父基线、debt及原验证workitem字节保存在.build/stability-refactor-20261027/preview-drive-resolution-r60/preimage/。此目录不冒称保存全部源字节。

追溯：DRV-001～005 → FP-02/03/05/12 → 当前窗口预览设计docs/design/first-product-v1/interaction-review.html#01-window-preview → M02/M12/M15、C13/C14/C35/C36。Owner：remaining_game_driver_plan27负责目标解析、路径证明及私有wire候选与测试；AP负责目标身份，PL负责私有worker schema；Root负责本卡、源整合、精确authority和实际验证。独立architect-reviewer已指出私有schema应升版本及路径范围未齐，采用前必须闭合；独立审阅者另核对代码和精确packet。所有协作者不独占代码库，不覆盖或回退他人。

允许产品实现路径仅：
- crates/app-infra/src/platform/game_launch.rs
- crates/app-infra/src/hook_backend/preview_identity.rs
- crates/app-infra/src/hook_backend.rs（仅既有私有Frozen wire fixture及严格拒绝测试）
- apps/desktop/src-tauri/src/platform/window_collector/mod.rs（仅既有transport测试fixture）
- apps/desktop/src-tauri/src/platform/window_collector/protocol.rs（私有VERSION 2→3与旧版拒绝测试）

独审范围补充：私有Frozen增加必需canonical image_path闭合freeze→worker及inventory后核验；按既有schema规则同步私有版本，不改变Core ABI1、Hook JSONL2或公开DTO/IPC。后三路径原字节在任何Main产品修改前补存于同一preimage/original并逐项核对原1880项快照，receipt明确记录范围补充；不回写旧R59快照或失败证据。

自身七文件及Root采用必需的精确authority路径见verification.json。无整目录产品授权；不改旧验证workitem、legacy-debt、Core、公开契约、数据库、PRD、批准PNG/token/阈值、原oracle或600秒action预算。候选必须精确匹配前像；未采用的ignored候选不当作Main实现。

依赖与验证：先完成隔离的目标盘解析和身份拒绝测试，再核对保留旧测试、独立代码审阅、精确packet审阅和Root采用。运行npm run spec:validate、spec_contract、technology_contract、实际app-infra测试、注册边界/编译及npm run verify -- --change repair-preview-target-drive-resolution；所有真实命令、退出码、源与回执保存。现有视觉/运行失败不通过删检查解决，baseline drift只按精确审阅采用处理，不刷新旧debt。

完成条件：本片实现和工程检查分别记录；重新冻结新RC并完成后续四游戏与睡眠恢复前，不能称SR6或首版产品通过。R59原运行保持failed（38/50，65/68），其中自然退出Host正常但operation持久终态缺口另片修复；台前调度已恢复1，游戏尺寸不变，不操作另一台Steam会话，无Ever17回归。
