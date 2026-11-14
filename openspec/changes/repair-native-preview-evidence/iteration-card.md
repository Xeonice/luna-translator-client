# SR6取证修复迭代卡

日期：2026-10-27。授权：用户要求完成整个既定重构，且已明确继续原重构实机回归、暂不实现新紧凑客户端布局。本片是已授权范围内的取证缺陷修正；此准备任务只写本change，Root稍后独立审阅后才允许Main源整合，不自动更新baseline。

父authority：FP1-20261027-R58／SHA `267df69126b263000382b58066e0d398d03e33cfc7d7d6d3db500c724454608c`。本change修改前源1870项／tree `1d2292a1216f370e6972856d7cccff887c690bab1666ae1656c5db5fe4d89e6a`；全map及observer/test原字节保存于`.build/stability-refactor-20261027/runtime-observer-repair-preimage/`，该目录不冒称保存全部1870源字节。

类型：`tooling`（当前框架只有tooling／product，不自创guard类型）。本片检查属于QA工具／纯测试，不冒充product-runtime。追溯：FP-03／04／12 → 当前预览设计`docs/design/first-product-v1/interaction-review.html#01-window-preview` → M12／C35 → OBS-001～004。现有产品交互和已确认设计不变。

Owner：本工作卡由上下文准备Owner只写自身七文件；代码Owner后续仅拥有下面两个工具源，Root拥有整合／authority／最终包与GUI，AP独立审阅。所有协作者不独占代码库，不覆盖／回退他人。

允许实现源：
- `scripts/quality/runtime_observer.swift`
- `scripts/quality/test_runtime_stability.py`（仅沿现`RuntimeStabilityGuards`补真正Swift纯测试，不新增checkId）

自身文档：`openspec/changes/repair-native-preview-evidence/**`。后续Root精确采用所需authority允许路径见verification.json；它们不是本准备Owner可修改范围。无apps／crates／native／产品资源或整目录授权；不改`runtime_stability.py`、`runtime_stability_evidence.py`、verify registry、PRD、PNG、token、阈值、Core/DB或600秒action预算。ignored driver别名修复另行冻结／独审，不扩大Main源码范围。

验证命令（每次保存真实argv／exit／source／日志，不预填通过）：
```sh
PYTHONPATH=scripts/quality python3 -m unittest test_runtime_stability.RuntimeStabilityGuards -v
npm run spec:validate
python3 scripts/quality/technology_contract.py --json
npm run verify -- --change repair-native-preview-evidence
```

源码修正前可出现baseline drift并必须保留；精确独审及Root新revision采用后再执行适用完整门禁，不为绿灯自行改pin。真正Swift纯测试须旧实现实际red、修后green，原方法／断言完整；缺macOS编译工具明确blocked，不skip视为通过。

依赖与完成条件：原3次runtime超时／缺裁剪原证据全部保留，不能借本片继承通过；工具／规范／代码审阅／精确采用与真实新RC分别留证。完整source seal改变后必须重新freeze/build同证书包，游戏SR中文／MO日文／Never7日文／樱之诗10th日文与真实SleepWake另跑；无Ever17、不动另一台Steam会话、不改gamegeometry。Stage Manager原临时0现已由Root恢复1，此处不再标pending也不操作桌面。
