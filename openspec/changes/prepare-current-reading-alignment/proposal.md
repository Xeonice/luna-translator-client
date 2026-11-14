# CP00-A：当前阅读对齐的只读取证

## Why
用户授权开始实施并要求阶段门禁通过后再进入下一步。R80已确认61源，但缺当前独立参照及真实捕获，不能直接改生产UI。先建立可复跑的设计源独立参考候选和既有正式React组件只读诊断，显式暴露差异。

## What Changes
- 工具变更；冻结当前阅读输入、视口、主题、source tree、nonce、命令、原始DOM和独立PNG。复用原生产WindowRoot/ApplicationStore/DesignPort受控运输，不复制产品组件。
- 设计源与生产组件分别构建/加载；输出到新.build目录，参考候选不自动批准。
- 独立程序重算PNG和语义事实，实际差异不能写成产品通过；准出仍blocked。

## Impact
FP-04/09/11、RW-17～23/26～30、GD-01/02/03、CP-00/08；M15/QA工具，M13/C16/C17/C36只读。产品/Rust/Core/采集器/DDL/技术锁/61源/旧PNG/阈值/冻结计划不变。SR6未核实退出前不启动剩余功能。精确修改范围由verification与预冻结ownership列出。
