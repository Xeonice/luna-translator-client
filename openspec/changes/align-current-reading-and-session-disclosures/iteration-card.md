# CP-01迭代卡

| 字段 | 本轮内容 |
|---|---|
| 身份 | align-current-reading-and-session-disclosures；父R81，完整source snapshot |
| 用户授权 | “进入下一产品阶段，开始做正事修改”；直接实施产品，不停留在工具；不开准出例外 |
| 需求/设计 | FP-04/09/11 → GD01/02/03、GUI04、RW17～23/26～30；61源complete-guided-history-20261031，guided当前句和后置details |
| 模块/组件 | M03/M10/M13；C16/C17/C18/C36；C35保持原预览租约/订阅 |
| Owner | cp01_reading_implementation生产3文件及精确补充的2个infra租约文件；cp01_reading_tests测试3文件；Root治理/架构状态；独立非作者审查 |
| 允许路径 | 编码前ownership.json冻结，verification逐项列出；不含Core/DB/公共DTO/设计源；后续真实回归故障精确补充两个infra Rust租约文件，见CPR-004 |
| 依赖与限制 | 现有DTO足够，复用既有端口；产品独立参照/adapter缺口保持真实，不以此停止已授权实施或伪造通过 |
| 验证 | 实际typecheck/build、100项既有reading行为及新增折叠独立/状态/词法/固定对象/预览不重建；Playwright四变体；独立审阅；完整verify真实记录 |
| 完成 | 产品修改和适用行为通过；候选包真实构建；未满足的完整视觉/正式包/四游戏门禁明确blocked，不宣称发布 |

打包补充：首轮预备输入检查失败后，派生language-worker暂存文件单独纳入精确范围，M07/JA协作；原像和失败日志保留，经当前生产者/二进制/stdio实证独审后再冻结构建，不更改Rust源码或语言协议。
