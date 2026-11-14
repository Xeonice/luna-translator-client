# 逻辑修正迭代卡

| 字段 | 内容 |
|---|---|
| 身份 | repair-design-gate-selection-logic；父R82，完整before snapshot |
| 授权 | 用户“进行逻辑修正”；不含提交/推送或准出豁免 |
| 需求/设计 | FP-11；GLC-001/002/003；complete-guided-history-20261031 61源与794义务保留 |
| 模块/组件 | M03/M13/M15；C16/C35/C36；本轮仅测试和质量执行逻辑 |
| Owner | gate_selection_fix：影响/能力/assessment与对应guards；historical_capture_fix：v4捕获与回执判读；Root：lifecycle、OpenSpec、文档和身份；独立非作者审核 |
| 允许路径 | verification.json逐项列出；产品TSX/Rust/设计源/PNG不可改 |
| 验证 | 原断言保留；纯测试/文档与生产导入反例、未知native全量、假adapter/preflight替代、历史phase/新final选择、条件事实逐字段；实际browser及统一verify |
| 完成条件 | 逻辑修正实际检查通过，独审绑定完整源；产品参照/映射/正式包缺口分别保持blocked |

捕获回执集成补充（修改前登记）：verify-ui.mjs与compare_product_capture.mjs需正确呈现/处理无PNG的历史退役blocked行；不将其视作比较通过，37项库存与原图保留。

影响分析补充（修改前登记）：page-inventory.mjs复用既有TypeScript解析器提供独立production依赖读取模式，覆盖bootstrap/index.html入口导入的测试资源；原页面合同输出不变。

独审补充（修改前登记）：current-product-readiness同样只核对报告自洽、未重算实际缺口；current_product_coverage.py及其原tests纳入GLC-001窄修，三个readiness均须真实事实核验，现行产品义务不变。比较器compare_product_capture.mjs是明确代码拥有的质量工具身份，允许准确登记，不豁免整个scripts目录。首轮冻结/失败证据保留。

统一检查收尾补充（改动前登记）：第一轮真实verify为89项中87通过、2失败。M03声明触发真实存储验收，因此明确登记并执行既有rust-storage-complete-v2，不移除M03或修改规则；另对本轮经独审/实际回归的3个捕获脚本product source closure及M15两条工具来源SHA精确更新实现状态目录，保留全部功能/对齐/实机状态，导出只允许四份既有状态文件。原失败回执独立保留，新树重新审核及统一运行。
