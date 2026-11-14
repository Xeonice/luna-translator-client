# 修复原生预览取证路径

本工作项属于用户已授权的整个稳定性重构及SR6实机回归中的取证工具缺陷修正，不增加产品能力、不实现新紧凑客户端布局。kind采用当前框架支持的`tooling`，不自创`guard`类别；执行身份沿用注册的`runtime-stability-guard-tests`。

当前冻结源为R58验证计划rebase后的1870项，tree `1d2292a1216f370e6972856d7cccff887c690bab1666ae1656c5db5fe4d89e6a`。独立OS观察器把AX原生帧写入Any字典时实际为`[CGFloat]`，后续按`[Double]`读取会静默缺失；真实现场虽有AXImage及游戏／主窗／HUD匹配OCR，仍无法生成所需预览裁剪。窗口截图的阴影范围与CG逻辑bounds也须保持可核验映射，不能用尺寸近似掩盖偏移。上述诊断不证明产品预览或所有SR6场景已通过。

本轮只允许修`runtime_observer.swift`及在现有`test_runtime_stability.py`中补真正执行Swift生产纯函数的红→绿测试；保留原有所有测试、失败与runtime oracle。只读取证保持不注入、不改几何、不请求权限。所有拒绝／失败必须留下有界明确诊断，不能静默当成功。

原retry及缺裁剪证据保留，不回填通过；修后须精确源独审、合法authority采用、重新冻结与同固定证书新RC，再独立重跑同包四游戏。原v4失败、PRD／产品合同／PNG／token／阈值／600秒action预算均保持。
