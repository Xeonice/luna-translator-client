# 单窗原生取证修复

用户已授权完成 SR0–SR6 重构并做同包游戏回归。本片只修 QA 只读截图工具，不增加产品能力、不实现已延期的紧凑预览。R59 MO 的 HUD 与词卡截图实际为同一附属窗口 union，既有 `screencapture -x -o -l` 没有请求排除 attached windows，两个相同PNG不能作为独立窗口证据。主窗20次截图并无同样证据，不泛化为所有窗口都错误。

只允许 observer 与现有 RuntimeStabilityGuards 测试两源。保留原 runtime 同句 oracle、原失败、PNG/token/视觉阈值、时间预算、48 个 guard 方法及既有16个 native case；新增纯检查不得硬编码产品通过。孤立候选 48 guards/27 native 真实通过，Root独立采用与正式新包回归尚未执行。本片不修复小图OCR精确同句失败，不将尺寸一致、不同hash或请求 `-a` 等同窗口内容归属证明。

## 显式R62范围补充：合法新终验登记

原final checker固定旧verify-refactor-stability-rc，新名own-only workitem会在development空diff选择0义务。用户要求保留原失败工作项，Root已授权在尚未Main的本QA片追加三个确切路径：lifecycle-impact-contract.json、lifecycle_impact.py、原test_lifecycle_impact.py。合同升级strict schema2，只追加受精确pin/独审约束的additional_final_change_ids；旧primary/所有phase/九义务/14links/cases不变。新名不在代码硬编码，未登记声明runtime明确拒绝。不是用release绕过。原attempt01保留，1900/R61真实前像补存3原文件，不重拍旧map。
