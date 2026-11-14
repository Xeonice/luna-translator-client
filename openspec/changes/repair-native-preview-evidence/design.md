# 原生取证修复技术设计

## 边界与复用

职责归属QA原生证据设施，追溯FP-03／FP-04／FP-12与M12／C35；设计来源仅用于解释预览可观察结果，不改`docs/design/first-product-v1/`或产品行为。重用R54的只读OS观察器及`RuntimeStabilityGuards`注册入口，不引入debug IPC、产品控制口、截图fallback或第二套runtime oracle。Core ABI1／七回调／JSONL2、DB、UI、collector、Host和翻译／语言worker均不在本片实现范围。

## 原生帧及截图几何

从AX获得的原生CGFloat坐标必须经明确的数值解码／有限数验证形成同一纯几何路径，不能依赖`Any`中`[CGFloat] as? [Double]`的偶然桥接。以真实AX frame、同一窗口CG bounds、实际PNG像素宽高计算裁剪；支持负屏幕原点、独立x/y比例及Retina，拒绝非法类型、错误元素数、NaN／Inf、非正或越界尺寸。必要的纯函数与编译入口隔离只可留在已授权observer文件中；若实现需要额外源文件，先由Root精确补scope并审阅，不能暗加路径。

窗口图像采集须排除窗口阴影，使逻辑bounds与截图内容边界一致；截图仍只取已指定窗口，不扩大到桌面／其他窗口。裁剪只能来自本次真实PNG及实际AXImage范围，不能用整张主窗OCR、服务元数据、缓存旧图或预造preview_crops代替。现有关键字段与原始画面/OCR保持，不能修改`runtime_stability_evidence.py`或放宽其验收条件。

## 失败诊断

对AX帧解码、bounds／像素几何、截图进程失败／超时、图像解码、OCR、裁剪或写入失败分别记录真实阶段与原因；缺值按缺失记录，不虚构frame／时间／OCR。保持有界遍历、fresh输出、当前PID/window角色选择、只读系统调用及既有时间／容量预算；不改600秒barrier预算或7200秒总预算。诊断不能变成传入pass开关。

## 纯测试与真实回归分层

现有`RuntimeStabilityGuards`新增测试须实际编译调用生产Swift纯函数，使用真实原生CGFloat／受支持数值容器建立输入，复现旧转换red，再验证相同路径green。不得仅用Python复制算法、源码字符串匹配或预置crop对象。覆盖数值类型／非法类型与形状、1x／2x、非同x/y比例、负原点、边界与越界；保持原测试方法／断言，禁止skip／expectedFailure替代通过。该纯测试不读取AX／屏幕、不启动游戏、不改变权限或几何；实际截图／OCR与睡醒仍由新RC回归证明。

## 源身份、采用与旧运行

前像见`.build/stability-refactor-20261027/runtime-observer-repair-preimage/receipt.json`。Root拥有authority、代码整合与GUI执行；实现Owner仅拥有两个明确源，AP独立审阅。代码/测试冻结和独审后，沿R55／R58精确packet→AP审阅→Root采用新revision；不直接刷新旧pin。新change或工具修改会改变包内全source seal，须保存最终验证计划前像再显式rebase／冻结，固定C514证书新建`--no-install`候选并实际检查／正常安装，不能用旧fd78包冒充新源。旧nonce、失败attempt和局部匹配OCR均不能拼接为新run通过。

Stage Manager此前临时关闭已由Root于本轮正常恢复为1并核实；该环境恢复是历史运行记录，不由本片执行。若后续另需临时环境变更，仍单独登记和恢复，不把环境调整当产品pass。
