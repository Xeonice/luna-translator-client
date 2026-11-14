# 首版完整产品实施

## Why
用户于2026-10-19明确指示“开始动工，按照事先规定的架构和设计进行开发，完成后进行对应的测试”。本轮由设计准备进入首版实施，保留FP1-20261019-R6权威来源与全部许可。

## What Changes
按FP-01..12实现Rust业务权威、SQLite、独立Core ABI1/v2 broker、MeCab/UniDic worker、模型及词典适配、React Aria界面与原生多窗。清理旧探针入口与无效草稿，接入真实端口及正式打包。

## Impact
仅本change声明路径；不修改PRD或已确认视觉源迁就代码。技术锁随新架构依赖作显式迁移，保留旧锁证据。整稿评审、数据库字段及发布批准不自动提升；未执行用例保持not_run，外部条件缺失blocked。

## 同源多行收尾

按 `multiline-capture-work-item.md` 修复真实 SR 分行暴露的 REQ-005/025 缺口：在 Rust 和既有 SQLite revisions 中进行有界聚合，保留原始回调及来源；不改 Core、公开契约、DDL、资源或 R12。新包实测独立记录，启发式边界限制不伪装为全部通过。


## 存储源码身份登记

参见 [capture-source-identity-work-item.md](capture-source-identity-work-item.md)。独立审阅后仅登记迁移执行文件的实际完整源码身份与 R13 单条指纹，不改变 SQL DDL、字段关系、批准范围、断言、阈值或产品验收状态。旧失败回执保留；新包与完整门禁重新执行。


## 两项产品验收闭环

用户明确要求落实视觉和完整存储验收，见 [acceptance-closure-work-item.md](acceptance-closure-work-item.md)。先保留原失败现场、补齐真实执行与逐项覆盖，再修复实际实现；不可通过修改批准图、阈值或缺失断言宣称通过。


## 入库与目标核对对齐

见 [library-capture-visual-alignment-work-item.md](library-capture-visual-alignment-work-item.md)。恢复已有设计中的参数、批次审阅/单次原子确认及真实候选字段显示；公开命令/回执的新增影响、去重与CAS决定显式记录，不改变Core/DDL。
